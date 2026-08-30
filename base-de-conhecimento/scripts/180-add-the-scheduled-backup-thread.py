# Add the scheduled backup thread
# 27/08 21:16

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s=s.replace('        self.subir_web();','        self.subir_web();\n        self.subir_backup_agendado();')
s=s.replace('''    // ----------------------------------------------------------- interface web''',
'''    /// Sobe o relogio do backup agendado, se ligado.
    ///
    /// Confere de minuto em minuto em vez de dormir ate a hora certa: dormir
    /// horas seguidas e frageil -- a maquina suspende, o relogio anda, e o
    /// backup nao acontece sem ninguem notar.
    fn subir_backup_agendado(self: &Arc<Self>) {
        if !self.config.backup.agendado {
            return;
        }
        let b = &self.config.backup;
        eprintln!(
            "backup agendado: {} | destino {} | {} | guarda {}",
            if b.hora.is_empty() {
                format!("a cada {} h", b.cada_horas)
            } else {
                format!("todo dia as {}", b.hora)
            },
            b.destino.display(),
            if b.zip { "um zip por vez" } else { "arvore de diretorios" },
            if b.manter == 0 {
                "tudo".to_string()
            } else {
                format!("os {} mais novos", b.manter)
            }
        );
        let servidor = Arc::clone(self);
        std::thread::spawn(move || {
            let mut ultimo = 0i64;
            loop {
                let agora = crate::agora_ms();
                if servidor.config.backup.hora_de_rodar(agora, ultimo) {
                    ultimo = agora;
                    match servidor.rodar_backup_agendado(agora) {
                        Ok(onde) => eprintln!("backup agendado: {onde}"),
                        Err(e) => eprintln!("backup agendado FALHOU: {e}"),
                    }
                }
                std::thread::sleep(Duration::from_secs(60));
            }
        });
    }

    fn rodar_backup_agendado(&self, quando: i64) -> Result<String> {
        let b = &self.config.backup;
        let (onde, r) = {
            let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
            if b.zip {
                let (caminho, r) = phxsql_store::backup::executar_zip(
                    &self.config.base,
                    &b.destino,
                    &b.database,
                    &b.admin,
                    quando,
                )?;
                (caminho.display().to_string(), r)
            } else {
                let pasta = b
                    .destino
                    .join(phxsql_core::datahora::instante_iso(quando).replace([' ', ':', ','], "-"));
                let r = phxsql_store::backup::executar(
                    &self.config.base,
                    &pasta,
                    &phxsql_core::datahora::instante_iso(quando),
                )?;
                (pasta.display().to_string(), r)
            }
        };

        // O log de acessos guarda tambem o que o servidor faz sozinho: senao,
        // a unica prova de que o backup rodou seria o arquivo existir.
        self.anotar(&Acesso {
            quando_ms: quando,
            ip: "(local)".into(),
            porta_origem: 0,
            op: "backup_agendado".into(),
            usuario: b.admin.clone(),
            autenticado: true,
            ok: true,
            duracao_ms: 0,
            erro: None,
        });

        let apagados = self.limpar_backups_velhos();
        Ok(format!(
            "{onde} ({} arquivos, {} bytes{}{})",
            r.arquivos.len(),
            r.bytes,
            if b.zip {
                format!(", zip de {} bytes", r.comprimido)
            } else {
                String::new()
            },
            if apagados > 0 {
                format!(", {apagados} antigo(s) apagado(s)")
            } else {
                String::new()
            }
        ))
    }

    /// Guarda so os `manter` mais novos. Zero nao apaga nada.
    ///
    /// Olha apenas os `.zip` cujo nome tem a cara dos nossos. Backup nao
    /// apaga arquivo que nao criou -- alguem pode ter guardado outra coisa
    /// nessa pasta.
    fn limpar_backups_velhos(&self) -> usize {
        let b = &self.config.backup;
        if b.manter == 0 || !b.zip {
            return 0;
        }
        let Ok(dir) = std::fs::read_dir(&b.destino) else {
            return 0;
        };
        let mut nossos: Vec<std::path::PathBuf> = dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().and_then(|e| e.to_str()) == Some("zip")
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.matches('_').count() >= 3)
                        .unwrap_or(false)
            })
            .collect();
        if nossos.len() <= b.manter {
            return 0;
        }
        // O nome ja ordena por data: Banco_Admin_AAAA-MM-DD_HHMM.zip.
        nossos.sort();
        let sobra = nossos.len() - b.manter;
        let mut apagados = 0;
        for velho in nossos.iter().take(sobra) {
            if std::fs::remove_file(velho).is_ok() {
                apagados += 1;
            }
        }
        apagados
    }

    // ----------------------------------------------------------- interface web''')
open(p,'w').write(s)
