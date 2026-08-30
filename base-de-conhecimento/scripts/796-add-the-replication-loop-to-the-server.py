# Add the replication loop to the server
# 28/08 20:17

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()

antigo = """            eprintln!(
                "ATENCAO: o transporte de eventos ainda nao esta implementado \\
                 (ver docs/REPLICACAO.md). As portas sao configuracao, nao servico."
            );
        }

        self.subir_web();
        self.subir_backup_agendado();"""
novo = """            if self.config.replicacao.papel == crate::config::Papel::Source
                && !self.config.replicacao.imagem_da_linha
            {
                eprintln!(
                    "ATENCAO: source com replicacao.imagem_da_linha DESLIGADA. O \\
                     diario grava que a linha mudou, nao grava para que, e as \\
                     replicas nao terao o que aplicar."
                );
            }
        }

        self.subir_web();
        self.subir_replicacao();
        self.subir_backup_agendado();"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    fn subir_backup_agendado(self: &Arc<Self>) {"""
novo = """    /// Uma thread por origem, puxando os eventos do source.
    ///
    /// Uma por origem e nao uma so: multi-source e varias conexoes
    /// independentes, e uma origem lenta ou caida nao pode segurar as outras.
    fn subir_replicacao(self: &Arc<Self>) {
        if self.config.replicacao.papel != crate::config::Papel::Replica {
            return;
        }
        if self.config.replicacao.origens.is_empty() {
            eprintln!(
                "replicacao: papel replica sem nenhuma origem em \\
                 replicacao.origens -- nada a puxar"
            );
            return;
        }
        if !self.config.somente_leitura {
            // Nao e erro, e e uma pedra no caminho conhecida: uma replica
            // escrita pela aplicacao quebra a numeracao dos rowids, e a
            // proxima inclusao vinda do source para a replicacao inteira.
            eprintln!(
                "ATENCAO: replica sem somente_leitura. Se a aplicacao escrever \\
                 aqui, os rowids divergem e a replicacao para."
            );
        }
        for origem in self.config.replicacao.origens.clone() {
            if !origem.senha.is_empty() && origem.senha_hash.is_empty() {
                eprintln!(
                    "AVISO: origem {} com a SENHA EM TEXTO PURO no config.json. \\
                     Troque por senha_hash: phxsqld --senha",
                    origem.nome
                );
            }
            eprintln!(
                "replicacao: puxando de {} ({}:{}) a cada {}s",
                origem.nome, origem.host, origem.porta, origem.reconectar_em
            );
            let servidor = Arc::clone(self);
            std::thread::spawn(move || servidor.laco_da_replica(origem));
        }
    }

    /// O laco de uma origem: conectar, puxar, aplicar, dormir, repetir.
    ///
    /// Erro nao mata a thread -- ele escreve e espera. Um source que caiu volta
    /// e a replica retoma do numero em que parou; matar a thread exigiria
    /// reiniciar a replica para religar a replicacao.
    fn laco_da_replica(self: Arc<Self>, origem: crate::config::Origem) {
        let espera = Duration::from_secs(origem.reconectar_em);
        loop {
            match self.rodada_da_replica(&origem) {
                Ok(0) => {}
                Ok(n) => eprintln!("replicacao [{}]: {n} evento(s) aplicado(s)", origem.nome),
                Err(e) => eprintln!("replicacao [{}]: {e}", origem.nome),
            }
            std::thread::sleep(espera);
        }
    }

    /// Uma passada por todas as tabelas de todos os databases da origem.
    ///
    /// Devolve quantos eventos aplicou.
    fn rodada_da_replica(&self, origem: &crate::config::Origem) -> Result<u64> {
        let mut cliente = crate::replica::ligar(origem)?;
        let databases = if origem.databases.is_empty() {
            cliente.databases()?
        } else {
            origem.databases.clone()
        };

        let mut aplicados = 0u64;
        for database in databases {
            let (com_imagem, tabelas) = crate::replica::posicao(&mut cliente, &database)?;
            if !com_imagem {
                return Err(PhxError::Esquema(format!(
                    "o source de {} esta com replicacao.imagem_da_linha desligada: \\
                     o diario dele nao carrega a linha, e nao ha o que aplicar",
                    origem.nome
                )));
            }
            for no in tabelas {
                aplicados += self.alcancar_tabela(&mut cliente, &database, &no)?;
            }
        }
        Ok(aplicados)
    }

    /// Traz UMA tabela ate a posicao do source.
    fn alcancar_tabela(
        &self,
        cliente: &mut crate::replica::Cliente,
        database: &str,
        no: &crate::replica::NoSource,
    ) -> Result<u64> {
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = _trava.garantir_database(database)?;

        // Tabela que ainda nao existe aqui nasce do MESMO bloco de esquema que
        // o source tem, e nao de uma remontagem a partir de JSON: e assim que
        // o payload da imagem cai byte a byte no lugar certo.
        let mut tabela = match db.abrir_qualificada(&no.nome) {
            Ok(t) => t,
            Err(_) => match &no.esquema {
                Some(e) => {
                    let (schema, nome) = match no.nome.split_once('.') {
                        Some((s, n)) => (Some(s.to_string()), n.to_string()),
                        None => (None, no.nome.clone()),
                    };
                    let _ = nome;
                    eprintln!("replicacao: criando {database}.{} aqui", no.nome);
                    db.criar_tabela(schema.as_deref(), e.clone())?
                }
                None => return Ok(0),
            },
        };

        let mut posicao = tabela.eventos()?;
        if posicao >= no.eventos {
            return Ok(0);
        }
        let mut aplicados = 0u64;
        while posicao < no.eventos {
            let eventos = crate::replica::puxar(cliente, database, &no.nome, posicao)?;
            if eventos.is_empty() {
                break;
            }
            for e in &eventos {
                tabela.aplicar_evento(e.operacao, e.rowid, &e.imagem)?;
                aplicados += 1;
            }
            // A posicao LOCAL, e nao `posicao + eventos.len()`: aplicar gera
            // eventos no diario daqui, e e por ele que a proxima rodada se
            // orienta. Contar do lado do source deixaria os dois numeros
            // andarem separados no primeiro evento que nao gerasse outro.
            posicao = tabela.eventos()?;
        }
        tabela.sincronizar()?;
        Ok(aplicados)
    }

    fn subir_backup_agendado(self: &Arc<Self>) {"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
