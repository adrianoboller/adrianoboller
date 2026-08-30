# Add backup ops to server
# 27/08 20:48

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('''            "memoria" => self.op_memoria(),''','''            "memoria" => self.op_memoria(),
            "backup" => self.op_backup(p),
            "conferir_backup" => self.op_conferir_backup(p),''')
s=s.replace('''    // ------------------------------------------------------ tabela em memoria''','''    /// Copia de seguranca, com a trava de dados segurada do inicio ao fim.
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
                &phxsql_core::datahora::formatar_ms(quando),
            )?
        };
        Ok(Json::objeto(vec![
            ("destino", Json::texto_de(&destino)),
            ("arquivos", Json::de_u64(r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
        ]))
    }

    fn op_conferir_backup(&self, p: &Json) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \\"destino\\"".into()));
        }
        let r = phxsql_store::backup::conferir(std::path::Path::new(&destino))?;
        Ok(Json::objeto(vec![
            ("destino", Json::texto_de(&destino)),
            ("integro", Json::Bool(r.ok())),
            ("arquivos", Json::de_u64(r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            (
                "divergencias",
                Json::Lista(r.divergencias.iter().map(Json::texto_de).collect()),
            ),
        ]))
    }

    // ------------------------------------------------------ tabela em memoria''')
open(p,'w').write(s)
