# Add zip option to the backup operation
# 27/08 21:14

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
velho='''    /// Copia de seguranca, com a trava de dados segurada do inicio ao fim.
    fn op_backup(&self, p: &Json) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \\"destino\\"".into()));
        }
        let quando = crate::agora_ms();
        let inicio = Instant::now();
        // A trava fica presa a copia inteira. E o que "consistente" quer dizer
        // sem transacao: nenhuma escrita acontece no meio.
        let r = {
            let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
            phxsql_store::backup::executar(
                &self.config.base,
                std::path::Path::new(&destino),
                &phxsql_core::datahora::instante_iso(quando),
            )?
        };
        Ok(Json::objeto(vec![
            ("destino", Json::texto_de(&destino)),
            ("arquivos", Json::de_u64(r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
        ]))
    }'''
novo='''    /// Copia de seguranca, com a trava de dados segurada do inicio ao fim.
    ///
    /// `"zip": true` faz um arquivo unico chamado
    /// `Banco_Admin_Data_HoraMin.zip`, com o manifesto dentro. Sem isso,
    /// copia a arvore de diretorios como antes.
    fn op_backup(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \\"destino\\"".into()));
        }
        let quando = crate::agora_ms();
        let inicio = Instant::now();
        let em_zip = p.booleano_ou("zip", false);
        let banco = p.texto_ou("database", "").trim().to_string();
        // Quem fez entra no nome do arquivo. Sem login, entrou pelo token de
        // servico -- e o nome diz isso, em vez de fingir um usuario.
        let quem = if sessao.login().is_empty() {
            "servico".to_string()
        } else {
            sessao.login().to_string()
        };

        // A trava fica presa a copia inteira. E o que "consistente" quer dizer
        // sem transacao: nenhuma escrita acontece no meio.
        let (arquivo, r) = {
            let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
            if em_zip {
                let (caminho, r) = phxsql_store::backup::executar_zip(
                    &self.config.base,
                    std::path::Path::new(&destino),
                    &banco,
                    &quem,
                    quando,
                )?;
                (Some(caminho.display().to_string()), r)
            } else {
                (
                    None,
                    phxsql_store::backup::executar(
                        &self.config.base,
                        std::path::Path::new(&destino),
                        &phxsql_core::datahora::instante_iso(quando),
                    )?,
                )
            }
        };

        let mut campos = vec![
            ("destino", Json::texto_de(&destino)),
            ("arquivos", Json::de_u64(r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
        ];
        if let Some(a) = arquivo {
            campos.push(("arquivo", Json::texto_de(a)));
            campos.push(("comprimido", Json::de_u64(r.comprimido)));
            campos.push((
                "reducao_pct",
                Json::de_u64(if r.bytes > 0 {
                    100 - (r.comprimido * 100 / r.bytes).min(100)
                } else {
                    0
                }),
            ));
        }
        Ok(Json::objeto(campos))
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)
s=s.replace('"backup" => self.op_backup(p),','"backup" => self.op_backup(p, sessao),')
open(p,'w').write(s)
