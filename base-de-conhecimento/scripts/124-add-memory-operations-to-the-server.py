# Add memory operations to the server
# 27/08 20:23

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
ancora = '    fn op_diario(&self, p: &Json, sessao: &Sessao) -> Result<Json> {'
assert s.count(ancora)==1
bloco = '''    // ------------------------------------------------------ tabela em memoria

    /// Chave de residencia. Inclui o database porque duas bases podem ter
    /// tabela de mesmo nome -- e teriam, se ninguem cuidasse disso.
    fn chave_residente(p: &Json) -> String {
        format!(
            "{}/{}",
            p.texto_ou("database", ""),
            p.texto_ou("tabela", "")
        )
    }

    /// Mexe na copia residente, se a tabela deste pedido estiver carregada.
    fn residente_mut(&self, p: &Json, f: impl FnOnce(&mut TabelaMemoria)) {
        if let Ok(mut r) = self.residentes.lock() {
            if let Some(m) = r.get_mut(&Self::chave_residente(p)) {
                f(m);
            }
        }
    }

    /// Le a tabela inteira para a RAM.
    fn op_memoria_carregar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let esquema = t.esquema().clone();

        // As colunas com mapa de igualdade. Sem pedido, mapeia as que ja sao
        // primeira coluna de algum indice: quem indexou no disco costuma
        // filtrar pelo mesmo campo na memoria.
        let mapear: Vec<usize> = match p.campo("mapear").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, &esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => {
                let mut v: Vec<usize> = esquema
                    .indices()
                    .iter()
                    .filter_map(|i| i.colunas.first().map(|c| c.coluna))
                    .collect();
                v.sort_unstable();
                v.dedup();
                v
            }
        };

        let inicio = Instant::now();
        let m = {
            let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
            TabelaMemoria::carregar(&mut t, &mapear, crate::agora_ms())?
        };
        let ficha = ficha_residente(&Self::chave_residente(p), &m);
        let ms = inicio.elapsed().as_millis() as u64;
        self.residentes
            .lock()
            .map_err(|_| trava_envenenada())?
            .insert(Self::chave_residente(p), m);

        let mut campos = ficha;
        campos.push(("carregou_em_ms", Json::de_u64(ms)));
        Ok(Json::objeto(campos))
    }

    fn op_memoria_liberar(&self, p: &Json) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let saiu = self
            .residentes
            .lock()
            .map_err(|_| trava_envenenada())?
            .remove(&chave)
            .is_some();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            ("estava_carregada", Json::Bool(saiu)),
        ]))
    }

    /// O que esta residente agora.
    fn op_memoria(&self) -> Result<Json> {
        let r = self.residentes.lock().map_err(|_| trava_envenenada())?;
        let mut chaves: Vec<&String> = r.keys().collect();
        chaves.sort();
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            ("tabelas", Json::de_u64(r.len() as u64)),
            (
                "bytes",
                Json::de_u64(r.values().map(|m| m.bytes() as u64).sum()),
            ),
            (
                "residentes",
                Json::Lista(
                    chaves
                        .into_iter()
                        .map(|c| {
                            let m = &r[c];
                            let mut f = ficha_residente(c, m);
                            f.push((
                                "carregada_ha_s",
                                Json::de_u64(((agora - m.carregada_ms()) / 1000).max(0) as u64),
                            ));
                            Json::objeto(f)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// `SelectMemory`: a consulta que nao toca em disco.
    ///
    /// Recusa em vez de adivinhar quando a tabela nao esta carregada. Carregar
    /// uma tabela grande sem ninguem ter pedido seria a operacao rapida virando
    /// a operacao lenta, calada, na hora errada.
    fn op_selecionar_memoria(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let r = self.residentes.lock().map_err(|_| trava_envenenada())?;
        let m = r.get(&chave).ok_or_else(|| {
            PhxError::NaoEncontrado(format!(
                "{chave} nao esta em memoria; carregue antes com {{\\"op\\":\\"memoria_carregar\\",\\"database\\":...,\\"tabela\\":...}}"
            ))
        })?;
        let esquema = m.esquema();

        // O poder vale igual na memoria e no disco. O portao ja passou pelo
        // despachar; isto e o cinto: quem chegar aqui por outro caminho para.
        if let Some(u) = &sessao.usuario {
            if !u.pode(p.texto_ou("database", ""), Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {}",
                    u.login,
                    p.texto_ou("database", "")
                )));
            }
        }

        let mut onde = Vec::new();
        if let Some(l) = p.campo("onde").and_then(Json::lista) {
            for f in l {
                let coluna = coluna_de(
                    f.campo("coluna")
                        .ok_or_else(|| PhxError::Esquema("filtro sem \\"coluna\\"".into()))?,
                    esquema,
                )?;
                let op = Operador::de_texto(f.texto_ou("op", "="))?;
                let valor = match f.campo("valor") {
                    Some(v) => crate::valores::json_para_valor(v, &esquema.colunas()[coluna].tipo)?,
                    None => phxsql_core::value::Value::Null,
                };
                onde.push(Filtro { coluna, op, valor });
            }
        }

        let mut ordenar = Vec::new();
        if let Some(l) = p.campo("ordenar").and_then(Json::lista) {
            for o in l {
                ordenar.push(Ordem {
                    coluna: coluna_de(
                        o.campo("coluna")
                            .ok_or_else(|| PhxError::Esquema("ordem sem \\"coluna\\"".into()))?,
                        esquema,
                    )?,
                    desc: o.booleano_ou("desc", false),
                });
            }
        }

        let colunas = match p.campo("colunas").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => Vec::new(),
        };

        let consulta = Consulta {
            onde,
            ordenar,
            colunas,
            pular: p.inteiro_ou("pular", 0).max(0) as u64,
            max: self.limite(p),
        };

        let inicio = Instant::now();
        let saida = m.selecionar(&consulta)?;
        let us = inicio.elapsed().as_micros() as u64;

        // A projecao muda as colunas, entao os nomes vem com o resultado --
        // senao quem le nao sabe qual campo e qual.
        let nomes: Vec<String> = if consulta.colunas.is_empty() {
            esquema.colunas().iter().map(|c| c.nome.clone()).collect()
        } else {
            consulta
                .colunas
                .iter()
                .map(|i| esquema.colunas()[*i].nome.clone())
                .collect()
        };

        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            ("colunas", Json::Lista(nomes.iter().map(Json::texto_de).collect())),
            ("achadas", Json::de_u64(saida.achadas)),
            ("devolvidas", Json::de_u64(saida.linhas.len() as u64)),
            ("examinadas", Json::de_u64(saida.examinadas)),
            (
                "por_mapa",
                match &saida.por_mapa {
                    Some(c) => Json::texto_de(c),
                    None => Json::Nulo,
                },
            ),
            ("us", Json::de_u64(us)),
            (
                "linhas",
                Json::Lista(
                    saida
                        .linhas
                        .iter()
                        .map(|(rowid, l)| {
                            let mut campos = vec![("rowid", Json::de_u64(*rowid))];
                            for (n, v) in nomes.iter().zip(l.iter()) {
                                campos.push((n.as_str(), crate::valores::valor_para_json(v)));
                            }
                            Json::objeto(campos)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

'''
s = s.replace(ancora, bloco + ancora)

# ajudantes no fim do arquivo
s += '''
/// Uma coluna, pelo nome ou pelo numero. Aceitar os dois e o que deixa a
/// consulta legivel a mao e barata pela interface.
fn coluna_de(j: &Json, esquema: &phxsql_core::schema::Schema) -> Result<usize> {
    if let Some(n) = j.inteiro() {
        let i = n as usize;
        if n < 0 || i >= esquema.colunas().len() {
            return Err(PhxError::Esquema(format!("coluna {n} nao existe")));
        }
        return Ok(i);
    }
    let nome = j.texto().unwrap_or("");
    esquema
        .colunas()
        .iter()
        .position(|c| c.nome == nome)
        .ok_or_else(|| PhxError::Esquema(format!("coluna {nome:?} nao existe")))
}

fn ficha_residente(chave: &str, m: &TabelaMemoria) -> Vec<(&'static str, Json)> {
    let nomes: Vec<Json> = m
        .colunas_mapeadas()
        .iter()
        .map(|i| Json::texto_de(&m.esquema().colunas()[*i].nome))
        .collect();
    vec![
        ("tabela", Json::texto_de(chave)),
        ("linhas", Json::de_u64(m.vivos())),
        ("slots", Json::de_u64(m.slots())),
        ("bytes", Json::de_u64(m.bytes() as u64)),
        ("mapas", Json::Lista(nomes)),
    ]
}
'''
open(p,'w').write(s)
