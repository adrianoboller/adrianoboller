# Add the replication ops
# 28/08 20:13

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

antigo = """            "diario" => self.op_diario(p, sessao),"""
novo = """            "diario" => self.op_diario(p, sessao),
            "posicao" => self.op_posicao(p, sessao),
            "replicar" => self.op_replicar(p, sessao),
            "aplicar" => self.op_aplicar(p, sessao),"""
assert antigo in s
s = s.replace(antigo, novo)

# As tres operacoes, logo depois de op_diario.
antigo = """        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("eventos", Json::Lista(recentes)),
        ]))
    }"""
novo = """        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("eventos", Json::Lista(recentes)),
        ]))
    }

    // ----------------------------------------------------------- replicacao

    /// `posicao`: quantos eventos cada tabela do database ja tem.
    ///
    /// E o equivalente do `SHOW MASTER STATUS`, e o que a replica compara com
    /// a propria posicao para saber o que falta. Sai do cabecalho de cada
    /// volume do `.log`, sem ler evento nenhum.
    ///
    /// Por que POR TABELA e nao por servidor: o PhxSql ainda nao tem transacao
    /// entre tabelas, entao nao existe ordem global a preservar -- e um numero
    /// por tabela deixa as tabelas replicarem em paralelo.
    fn op_posicao(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let database = p.texto_ou("database", "").to_string();
        if database.is_empty() {
            return Err(PhxError::Esquema("informe \\"database\\"".into()));
        }
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = _trava.abrir_database(&database)?;
        let mut posicoes = Vec::new();
        for nome in db.todas_as_tabelas()? {
            let mut t = db.abrir_qualificada(&nome)?;
            posicoes.push((nome, Json::de_u64(t.eventos()?)));
        }
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(database)),
            ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
            // Sem a imagem ligada o diario existe mas nao replica, e a replica
            // precisa saber disso ANTES de puxar mil eventos inaplicaveis.
            (
                "imagem_da_linha",
                Json::Bool(self.config.replicacao.imagem_da_linha),
            ),
            ("tabelas", Json::Objeto(posicoes)),
            ("usuario", Json::de_u64(sessao.id() as u64)),
        ]))
    }

    /// `replicar`: os eventos a partir da posicao `desde`, com a imagem.
    ///
    /// A imagem vai em hexadecimal porque o transporte e JSON e JSON nao tem
    /// bytes. Dobra o tamanho -- e a alternativa seria acrescentar um formato
    /// binario ao protocolo, que e uma decisao maior do que esta.
    fn op_replicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let desde = p.inteiro_ou("desde", 0).max(0) as u64;
        let max = p.inteiro_ou("max", 500).max(0) as u64;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let total = t.eventos()?;
        let eventos = t.diario_com_imagem(desde, max)?;
        let lidos = eventos.len() as u64;

        let lista: Vec<Json> = eventos
            .into_iter()
            .map(|(e, imagem)| {
                Json::objeto(vec![
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                    ("imagem", Json::texto_de(bytes_para_hex(&imagem))),
                ])
            })
            .collect();

        Ok(Json::objeto(vec![
            ("desde", Json::de_u64(desde)),
            ("ate", Json::de_u64(desde + lidos)),
            ("total", Json::de_u64(total)),
            // `fim` verdadeiro quer dizer "por enquanto acabou": a replica
            // espera e pergunta de novo, em vez de girar em falso.
            ("fim", Json::Bool(desde + lidos >= total)),
            ("eventos", Json::Lista(lista)),
        ]))
    }

    /// `aplicar`: grava na tabela LOCAL os eventos que vieram do source.
    ///
    /// Para no primeiro erro e devolve onde parou. Seguir depois de um erro
    /// espalharia a divergencia -- e o rowid que nao bate ja e o sinal de que
    /// a replica divergiu.
    fn op_aplicar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let eventos = p
            .campo("eventos")
            .and_then(Json::lista)
            .ok_or_else(|| PhxError::Esquema("informe \\"eventos\\" como lista".into()))?
            .clone();
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;

        let mut aplicados = 0u64;
        let mut erro = None;
        for e in &eventos {
            let operacao = match e.texto_ou("operacao", "") {
                "inclusao" => Operacao::Inclusao,
                "alteracao" => Operacao::Alteracao,
                "exclusao" => Operacao::Exclusao,
                outro => {
                    erro = Some(format!("operacao desconhecida no evento: {outro:?}"));
                    break;
                }
            };
            let rowid = e.inteiro_ou("rowid", 0).max(0) as u64;
            let imagem = match hex_para_bytes(e.texto_ou("imagem", "")) {
                Ok(b) => b,
                Err(x) => {
                    erro = Some(x.to_string());
                    break;
                }
            };
            match t.aplicar_evento(operacao, rowid, &imagem) {
                Ok(_) => aplicados += 1,
                Err(x) => {
                    erro = Some(x.to_string());
                    break;
                }
            }
        }
        self.gravar_de_verdade(&mut t, p)?;

        Ok(Json::objeto(vec![
            ("recebidos", Json::de_u64(eventos.len() as u64)),
            ("aplicados", Json::de_u64(aplicados)),
            ("posicao", Json::de_u64(t.eventos()?)),
            (
                "erro",
                match erro {
                    Some(e) => Json::texto_de(e),
                    None => Json::Nulo,
                },
            ),
        ]))
    }"""
assert antigo in s
s = s.replace(antigo, novo, 1)
p.write_text(s)
print("ok")
