# Add the dashboard aggregate operation
# 27/08 22:34

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''            "memoria" => self.op_memoria(),''','''            "memoria" => self.op_memoria(),
            "painel" => self.op_painel(sessao),''')

s=s.replace('''    // ------------------------------------------------------ tabela em memoria''','''    // -------------------------------------------------------------- o painel

    /// Tudo que o painel mostra, numa chamada so.
    ///
    /// Poderia ser dez chamadas do navegador, e o painel ficaria dez vezes
    /// mais lento por causa da ida e volta. Agregar aqui tambem deixa a conta
    /// do que o usuario PODE VER acontecer de um lado so: o painel nunca
    /// mostra numero de base que quem esta olhando nao poderia abrir.
    fn op_painel(&self, sessao: &Sessao) -> Result<Json> {
        let agora = crate::agora_ms();

        // ---------------------------------------------------------- bancos
        let (mut bancos, mut tabelas_total, mut registros_total, mut bytes_total) =
            (Vec::new(), 0u64, 0u64, 0u64);
        let mut maiores: Vec<(String, u64, u64)> = Vec::new();
        {
            let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
            for nome in dados.databases()? {
                // O painel so conta o que quem esta olhando poderia abrir.
                if let Some(u) = &sessao.usuario {
                    if !u.pode(&nome, Atividade::Ler) {
                        continue;
                    }
                }
                let db = dados.abrir_database(&nome)?;
                let lista = db.todas_as_tabelas()?;
                let schemas = db.schemas()?.len() as u64;
                let mut registros_db = 0u64;
                for t in &lista {
                    if let Ok(mut tab) = db.abrir_qualificada(t) {
                        let regs = tab.registros();
                        registros_db += regs;
                        let bytes: u64 = tab
                            .volumes_por_arquivo()
                            .0
                            .iter()
                            .map(|v| {
                                std::fs::metadata(tab.diretorio().join(format!(
                                    "{}{}.reg",
                                    t.rsplit('.').next().unwrap_or(t),
                                    if *v == 1 { String::new() } else { format!("_{v:03}") }
                                )))
                                .map(|m| m.len())
                                .unwrap_or(0)
                            })
                            .sum();
                        bytes_total += bytes;
                        maiores.push((format!("{nome}/{t}"), regs, bytes));
                    }
                }
                tabelas_total += lista.len() as u64;
                registros_total += registros_db;
                bancos.push(Json::objeto(vec![
                    ("nome", Json::texto_de(&nome)),
                    ("tabelas", Json::de_u64(lista.len() as u64)),
                    ("schemas", Json::de_u64(schemas)),
                    ("registros", Json::de_u64(registros_db)),
                ]));
            }
        }
        // As dez maiores, por registro. Mais que isso vira lista, nao grafico.
        maiores.sort_by(|a, b| b.1.cmp(&a.1));
        maiores.truncate(10);

        // --------------------------------------------------------- acessos
        //
        // Uma passada so sobre o log, alimentando todas as contagens de uma
        // vez. Ler o arquivo cinco vezes para responder cinco perguntas seria
        // o painel ficando lento com o log crescendo.
        let acessos = LogAcessos::ler(&self.config.log_acessos).unwrap_or_default();
        let dia_ms = 86_400_000i64;
        let desde = agora - dia_ms;
        let mut por_hora = [0u64; 24];
        let mut recusadas_por_hora = [0u64; 24];
        let mut por_op: HashMap<String, (u64, u64)> = HashMap::new();
        let mut por_usuario: HashMap<String, u64> = HashMap::new();
        let (mut ok, mut falhas, mut soma_ms) = (0u64, 0u64, 0u64);
        for a in &acessos {
            if a.ok {
                ok += 1;
            } else {
                falhas += 1;
            }
            soma_ms += a.duracao_ms;
            let e = por_op.entry(a.op.clone()).or_insert((0, 0));
            if a.ok {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
            if !a.usuario.is_empty() {
                *por_usuario.entry(a.usuario.clone()).or_insert(0) += 1;
            }
            if a.quando_ms >= desde {
                // Balde por hora, contando de tras para frente a partir de
                // agora: o balde 23 e a hora corrente.
                let atras = ((agora - a.quando_ms) / 3_600_000) as usize;
                if atras < 24 {
                    let i = 23 - atras;
                    por_hora[i] += 1;
                    if !a.ok {
                        recusadas_por_hora[i] += 1;
                    }
                }
            }
        }
        let mut ops: Vec<(String, u64, u64)> =
            por_op.into_iter().map(|(k, (a, b))| (k, a, b)).collect();
        ops.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
        ops.truncate(12);
        let mut usuarios_ativos: Vec<(String, u64)> = por_usuario.into_iter().collect();
        usuarios_ativos.sort_by(|a, b| b.1.cmp(&a.1));
        usuarios_ativos.truncate(8);

        let ips = LogAcessos::resumo_por_ip(&self.config.log_acessos).unwrap_or_default();
        let mut top_ips: Vec<&crate::acesso::ResumoIp> = ips.iter().collect();
        top_ips.sort_by(|a, b| b.acessos.cmp(&a.acessos));
        top_ips.truncate(8);

        // -------------------------------------------------------- usuarios
        let cadastro = &self.config.cadastro;
        let mut por_nivel: HashMap<&'static str, u64> = HashMap::new();
        for u in cadastro.root.iter().chain(cadastro.usuarios.iter()) {
            *por_nivel.entry(u.nivel.nome()).or_insert(0) += 1;
        }
        let ordem_nivel = ["admin", "dono", "operador", "leitor", "nenhum"];

        // --------------------------------------------------------- estado
        let bloqueios = self
            .lista_negra
            .lock()
            .map(|l| l.ativos(agora).len() as u64)
            .unwrap_or(0);
        let (residentes, bytes_ram) = self
            .residentes
            .lock()
            .map(|r| {
                (
                    r.len() as u64,
                    r.values().map(|m| m.bytes() as u64).sum::<u64>(),
                )
            })
            .unwrap_or((0, 0));
        let sessoes_web = self.sessoes.lock().map(|s| s.quantas() as u64).unwrap_or(0);

        Ok(Json::objeto(vec![
            ("quando", Json::texto_de(phxsql_core::datahora::instante_iso(agora))),
            ("versao", Json::texto_de(VERSAO)),
            ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
            (
                "resumo",
                Json::objeto(vec![
                    ("bancos", Json::de_u64(bancos.len() as u64)),
                    ("tabelas", Json::de_u64(tabelas_total)),
                    ("registros", Json::de_u64(registros_total)),
                    ("bytes_reg", Json::de_u64(bytes_total)),
                    (
                        "usuarios",
                        Json::de_u64(
                            (cadastro.usuarios.len() + usize::from(cadastro.root.is_some())) as u64,
                        ),
                    ),
                    ("conexoes", Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64)),
                    ("sessoes_web", Json::de_u64(sessoes_web)),
                    ("bloqueios", Json::de_u64(bloqueios)),
                    ("tabelas_em_ram", Json::de_u64(residentes)),
                    ("bytes_em_ram", Json::de_u64(bytes_ram)),
                    ("acessos", Json::de_u64(ok + falhas)),
                    ("acessos_ok", Json::de_u64(ok)),
                    ("acessos_recusados", Json::de_u64(falhas)),
                    (
                        "ms_medio",
                        Json::de_u64(if ok + falhas > 0 { soma_ms / (ok + falhas) } else { 0 }),
                    ),
                    ("espelho", Json::Bool(self.config.espelho)),
                    ("somente_leitura", Json::Bool(self.config.somente_leitura)),
                ]),
            ),
            ("bancos", Json::Lista(bancos)),
            (
                "maiores_tabelas",
                Json::Lista(
                    maiores
                        .iter()
                        .map(|(n, r, b)| {
                            Json::objeto(vec![
                                ("tabela", Json::texto_de(n)),
                                ("registros", Json::de_u64(*r)),
                                ("bytes", Json::de_u64(*b)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_hora",
                Json::Lista(por_hora.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            (
                "recusadas_por_hora",
                Json::Lista(recusadas_por_hora.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            (
                "por_operacao",
                Json::Lista(
                    ops.iter()
                        .map(|(o, a, r)| {
                            Json::objeto(vec![
                                ("op", Json::texto_de(o)),
                                ("ok", Json::de_u64(*a)),
                                ("recusados", Json::de_u64(*r)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_nivel",
                Json::Lista(
                    ordem_nivel
                        .iter()
                        .filter_map(|n| {
                            por_nivel.get(n).map(|q| {
                                Json::objeto(vec![
                                    ("nivel", Json::texto_de(*n)),
                                    ("quantos", Json::de_u64(*q)),
                                ])
                            })
                        })
                        .collect(),
                ),
            ),
            (
                "usuarios_ativos",
                Json::Lista(
                    usuarios_ativos
                        .iter()
                        .map(|(u, q)| {
                            Json::objeto(vec![
                                ("usuario", Json::texto_de(u)),
                                ("acessos", Json::de_u64(*q)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "top_ips",
                Json::Lista(
                    top_ips
                        .iter()
                        .map(|r| {
                            Json::objeto(vec![
                                ("ip", Json::texto_de(&r.ip)),
                                ("acessos", Json::de_u64(r.acessos)),
                                ("recusados", Json::de_u64(r.recusados)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    // ------------------------------------------------------ tabela em memoria''')
open(p,'w').write(s)
