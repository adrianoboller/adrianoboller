# Escrever as operacoes ligar e sincronizar
# 29/08 11:39

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

anc='''    // ----------------------------------------------------- a maquina embaixo'''
assert s.count(anc)==1
novo='''    /// `dblink_ligar`: o assistente liga tabelas primas — cria a tabela local
    /// espelhando a remota e registra a sincronia na definicao da ligacao.
    fn op_dblink_ligar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::dblink::sincronia;
        let pedidos = p
            .campo("tabelas")
            .and_then(Json::lista)
            .ok_or_else(|| PhxError::Esquema("informe \\"tabelas\\" como lista".into()))?;
        if pedidos.is_empty() {
            return Err(PhxError::Esquema("a lista \\"tabelas\\" veio vazia".into()));
        }

        let (mut d, mut c) = self.ligar(p)?;
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut ligadas = Vec::new();
        for t in pedidos {
            let mut sinc = sincronia::Sincronia::de_json(t)?;
            crate::dblink::nome_seguro(&sinc.remota)?;
            if sinc.local_database.trim().is_empty() {
                return Err(PhxError::Esquema(format!(
                    "sincronia de {:?} sem \\"local_database\\"",
                    sinc.remota
                )));
            }
            // O portao por tabela, no alvo LOCAL: esta operacao nao tem o
            // campo "tabela" que o portao comum le -- o mesmo furo do juntar.
            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Criar) {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de criar {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
            }
            // So os metadados: LIMIT 0 traz as colunas tipadas sem uma linha.
            let r = c.consultar(
                &format!(
                    "SELECT * FROM {} LIMIT 0",
                    crate::dblink::entre_crases(&sinc.remota)
                ),
                1,
            )?;
            let (esquema, chave) = sincronia::esquema_local_de(&sinc.local_tabela, &r.colunas)?;
            let db = dados.garantir_database(&sinc.local_database)?;
            let criada = match db.abrir_qualificada(&sinc.local_tabela) {
                Ok(existente) => {
                    // Tabela ja existente serve, desde que a chave case; o
                    // resto o mapa por nome confere a cada rodada.
                    drop(existente);
                    false
                }
                Err(_) => {
                    db.criar_tabela(None, esquema)?;
                    true
                }
            };
            sinc.chave = chave;
            d.sincronias.retain(|x| {
                !(x.remota.eq_ignore_ascii_case(&sinc.remota)
                    && x.local_database.eq_ignore_ascii_case(&sinc.local_database)
                    && x.local_tabela.eq_ignore_ascii_case(&sinc.local_tabela))
            });
            ligadas.push(Json::objeto(vec![
                ("remota", Json::texto_de(&sinc.remota)),
                ("local_database", Json::texto_de(&sinc.local_database)),
                ("local_tabela", Json::texto_de(&sinc.local_tabela)),
                ("chave", Json::texto_de(&sinc.chave)),
                ("sentido", Json::texto_de(sinc.sentido.nome())),
                ("dono", Json::texto_de(sinc.dono.nome())),
                ("tabela_criada", Json::Bool(criada)),
            ]));
            d.sincronias.push(sinc);
        }
        c.encerrar();
        drop(dados);
        let mut r = self.dblink.lock().map_err(|_| trava_envenenada())?;
        r.salvar(d)?;
        Ok(Json::objeto(vec![("ligadas", Json::Lista(ligadas))]))
    }

    /// `dblink_sincronizar`: uma rodada de convergencia das tabelas ligadas.
    ///
    /// E a operacao que o job agenda. Exclusao nao viaja, o conflito e por
    /// linha e quem vence e o dono -- o porque de cada limite esta no modulo
    /// `dblink::sincronia`.
    fn op_dblink_sincronizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::dblink::sincronia::{self, Sentido};
        use std::collections::HashMap;

        let so = p.texto_ou("tabela", "").trim().to_string();
        let (d, mut c) = self.ligar(p)?;
        if d.sincronias.is_empty() {
            return Err(PhxError::Esquema(format!(
                "a ligacao {:?} nao tem tabela ligada: rode o assistente do DbLink",
                d.nome
            )));
        }

        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut relatorio = Vec::new();
        for sinc in &d.sincronias {
            if !so.is_empty()
                && !so.eq_ignore_ascii_case(&sinc.remota)
                && !so.eq_ignore_ascii_case(&sinc.local_tabela)
            {
                continue;
            }
            // Os portoes locais desta sincronia, conforme o que ela faz.
            if let Some(u) = &sessao.usuario {
                if !u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Ler) {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de ler {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
                if sinc.sentido != Sentido::Empurrar
                    && !(u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Inserir)
                        && u.pode_em(&sinc.local_database, &sinc.local_tabela, Atividade::Alterar))
                {
                    return Err(PhxError::Autorizacao(format!(
                        "sem direito de gravar em {}.{}",
                        sinc.local_database, sinc.local_tabela
                    )));
                }
            }
            if sinc.sentido != Sentido::Puxar && d.somente_leitura {
                return Err(PhxError::Autorizacao(format!(
                    "a ligacao {:?} esta em somente leitura e a sincronia de {:?} \\
                     empurra: tire o somente_leitura ou mude o sentido para puxar",
                    d.nome, sinc.remota
                )));
            }

            let db = dados.abrir_database(&sinc.local_database)?;
            let mut t = db.abrir_qualificada(&sinc.local_tabela).map_err(|_| {
                PhxError::NaoEncontrado(format!(
                    "a tabela local {}.{} nao existe: rode o assistente do DbLink",
                    sinc.local_database, sinc.local_tabela
                ))
            })?;
            t.definir_usuario(sessao.id());
            t.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);
            let esquema = t.esquema().clone();

            // O lado de la, inteiro -- com uma linha de sobra para saber se o
            // teto cortou. Sincronizar metade e fingir que acabou seria pior
            // que recusar.
            let teto = d.max_linhas;
            let r = c.consultar(
                &format!("SELECT * FROM {}", crate::dblink::entre_crases(&sinc.remota)),
                teto + 1,
            )?;
            if r.truncado || r.linhas.len() as u64 > teto {
                return Err(PhxError::LimiteExcedido(format!(
                    "a tabela remota {:?} passa das {} linhas da ligacao: suba o \\
                     max_linhas ou sincronize por outra estrategia",
                    sinc.remota, teto
                )));
            }

            let negocio = sincronia::posicoes_de_negocio(&esquema);
            let mapa = sincronia::mapa_de_colunas(&esquema, &r.colunas)?;
            let chave_biz = negocio
                .iter()
                .position(|p| esquema.colunas()[*p].nome.eq_ignore_ascii_case(&sinc.chave))
                .ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "a chave {:?} sumiu da tabela local {}.{}",
                        sinc.chave, sinc.local_database, sinc.local_tabela
                    ))
                })?;
            let indice_da_chave = esquema
                .indices()
                .iter()
                .find(|i| {
                    i.unico
                        && i.colunas.len() == 1
                        && i.colunas[0].coluna == negocio[chave_biz]
                })
                .map(|i| i.nome.clone())
                .ok_or_else(|| {
                    PhxError::Esquema(format!(
                        "{}.{} nao tem indice UNICO na chave {:?} -- e ele que \\
                         faz o upsert sem varrer",
                        sinc.local_database, sinc.local_tabela, sinc.chave
                    ))
                })?;

            let mut remotas = HashMap::new();
            for lr in &r.linhas {
                let lv = sincronia::linha_remota_para_negocio(&esquema, &negocio, &mapa, lr)?;
                remotas.insert(sincronia::chave_canonica(&lv[chave_biz]), lv);
            }
            let mut locais = HashMap::new();
            for (_rowid, linha) in t.varrer()? {
                let lv: Vec<_> = negocio.iter().map(|p| linha[*p].clone()).collect();
                locais.insert(sincronia::chave_canonica(&lv[chave_biz]), lv);
            }

            let plano = sincronia::plano(sinc.sentido, sinc.dono, &remotas, &locais);
            let (inseridas, alteradas) =
                sincronia::aplicar_para_ca(&mut t, &indice_da_chave, chave_biz, &plano.para_ca)?;
            t.sincronizar()?;

            let colunas_sql: Vec<(String, phxsql_core::types::ColumnType)> = negocio
                .iter()
                .map(|p| (esquema.colunas()[*p].nome.clone(), esquema.colunas()[*p].ty))
                .collect();
            let mut empurradas = 0u64;
            for sql in
                sincronia::sql_do_empurrao(&sinc.remota, &colunas_sql, &plano.para_la, 500)?
            {
                let r = c.consultar(&sql, 1)?;
                empurradas += r.afetadas;
            }

            relatorio.push(Json::objeto(vec![
                ("remota", Json::texto_de(&sinc.remota)),
                (
                    "local",
                    Json::texto_de(format!(
                        "{}.{}",
                        sinc.local_database, sinc.local_tabela
                    )),
                ),
                ("sentido", Json::texto_de(sinc.sentido.nome())),
                ("puxadas_novas", Json::de_u64(inseridas)),
                ("puxadas_alteradas", Json::de_u64(alteradas)),
                ("empurradas", Json::de_u64(plano.para_la.len() as u64)),
                ("linhas_afetadas_la", Json::de_u64(empurradas)),
                ("iguais", Json::de_u64(plano.iguais)),
                ("conflitos", Json::de_u64(plano.conflitos)),
            ]));
        }
        c.encerrar();
        if relatorio.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhuma sincronia casa com {so:?} na ligacao {:?}",
                d.nome
            )));
        }
        Ok(Json::objeto(vec![("sincronizadas", Json::Lista(relatorio))]))
    }

    // ----------------------------------------------------- a maquina embaixo'''
s=s.replace(anc,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ops escritas')
