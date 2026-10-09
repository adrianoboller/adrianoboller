//! Backup: o agendado, as operacoes, restaurar e o PITR.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// O estado de uma corrida de backup entre a fase 1 e a fase 2 -- ver
/// `Servidor::fazer_backup`.
enum Pronto {
    Arvore(phxsql_store::backup::Fase1),
    Zip(phxsql_store::backup::Fase1),
    /// Sem espaco para a arvore temporaria: a fase 2 faz a passada unica.
    ZipSemEspaco {
        livre: u64,
    },
    ZipInteiro {
        zip: phxsql_store::backup::ZipParcial,
        r: phxsql_store::backup::Relatorio,
        livre: u64,
    },
}

/// O que uma corrida de backup deixou -- a resposta do `backup` e a linha do
/// agendado saem daqui, um motor so.
#[derive(Default)]
struct BackupFeito {
    destino: PathBuf,
    /// O zip, quando a copia foi para arquivo unico.
    arquivo: Option<PathBuf>,
    r: phxsql_store::backup::Relatorio,
    ms: u64,
    fase_1_ms: u64,
    /// `None` so' no retrato inteiro (zip sem espaco).
    acerto: Option<phxsql_store::backup::Acerto>,
    /// Bytes livres medidos quando a arvore temporaria do zip nao coube.
    sem_espaco: Option<u64>,
}

impl BackupFeito {
    fn onde(&self) -> &Path {
        self.arquivo.as_deref().unwrap_or(&self.destino)
    }

    fn para_json(&self) -> Vec<(&'static str, Json)> {
        let mut campos = vec![
            (
                "destino",
                Json::texto_de(self.destino.display().to_string()),
            ),
            ("arquivos", Json::de_u64(self.r.arquivos.len() as u64)),
            ("bytes", Json::de_u64(self.r.bytes)),
            ("ms", Json::de_u64(self.ms)),
            (
                "modo",
                Json::texto_de(if self.acerto.is_some() {
                    "duas_passadas"
                } else {
                    "retrato_inteiro"
                }),
            ),
            (
                "retrato_ms",
                match self.r.retrato_ms {
                    Some(t) => Json::Numero(t as f64),
                    None => Json::Nulo,
                },
            ),
            ("fase_1_ms", Json::de_u64(self.fase_1_ms)),
        ];
        if let Some(a) = &self.acerto {
            campos.push((
                "fase_2",
                Json::objeto(vec![
                    ("ms", Json::de_u64(a.ms)),
                    ("arquivos", Json::de_u64(a.arquivos)),
                    ("bytes", Json::de_u64(a.bytes)),
                    ("conferidos", Json::de_u64(a.conferidos)),
                    ("fora_do_bloqueio", Json::de_u64(a.fora_do_bloqueio)),
                ]),
            ));
        }
        if let Some(livre) = self.sem_espaco {
            campos.push((
                "motivo",
                Json::texto_de(format!(
                    "sem espaco para a arvore temporaria do zip ({livre} bytes livres): a \
                     copia rodou inteira sob a trava, como antes do passo 2 do pedido 513"
                )),
            ));
        }
        if let Some(a) = &self.arquivo {
            campos.push(("arquivo", Json::texto_de(a.display().to_string())));
            campos.push(("comprimido", Json::de_u64(self.r.comprimido)));
            campos.push((
                "reducao_pct",
                Json::de_u64(if self.r.bytes > 0 {
                    100 - (self.r.comprimido * 100 / self.r.bytes).min(100)
                } else {
                    0
                }),
            ));
        }
        campos
    }
}

/// A ficha com que a fase 2 de uma copia em duas passadas roda -- ver
/// [`Servidor::copiar_o_retrato`].
pub(super) enum FichaDaFase2<'a> {
    Leitura(#[allow(dead_code)] &'a Raiz),
    Exclusiva(&'a Instancia),
}

impl Servidor {
    pub(super) fn subir_backup_agendado(self: &Arc<Self>) {
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
            if b.zip {
                "um zip por vez"
            } else {
                "arvore de diretorios"
            },
            if b.manter == 0 {
                "tudo".to_string()
            } else {
                format!("os {} mais novos", b.manter)
            }
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "backup-agendado",
            "confere de minuto em minuto se chegou a hora do backup e o executa; \
             dormir ate a hora certa seria fragil -- a maquina suspende e o \
             relogio anda",
            "servico",
            crate::agora_ms(),
            move |fio| {
                // Zero, a nao ser que a corrida anterior tenha derrubado o
                // processo -- ai ela conta como a ultima (pedido 502).
                let mut ultimo = servidor.lapide_do_backup_no_arranque();
                loop {
                    let agora = crate::agora_ms();
                    if servidor.config.backup.hora_de_rodar(agora, ultimo) {
                        ultimo = agora;
                        fio.fazendo("copiando e conferindo o SHA-256");
                        servidor.uma_corrida_do_backup_agendado(agora);
                    } else {
                        fio.fazendo("esperando a hora marcada");
                    }
                    std::thread::sleep(Duration::from_secs(60));
                }
            },
        );
    }

    /// Uma corrida do backup agendado: a lapide, a filha, e o aviso.
    ///
    /// # A filha (pedido 502)
    ///
    /// O backup segura a trava de ESCRITA por segundos, e na thread de servico
    /// um panico ali abortava o processo -- e o backup rodava de novo no
    /// arranque, e abortava de novo. Na filha (familia `corrida`), o panico
    /// repara a trava e volta como falha. A lapide cobre o que sobra: o reparo
    /// que falha ainda aborta (H5), e o arranque seguinte a acha e nao roda o
    /// backup de novo na partida.
    ///
    /// # O aviso (pedido 510)
    ///
    /// Falha que so ia ao erro padrao era descoberta no dia de restaurar. Vai
    /// ao carteiro da saude do disco -- e-mail e SMS, com silencio proprio --,
    /// e o sucesso seguinte devolve o direito de avisar na hora.
    fn uma_corrida_do_backup_agendado(&self, agora: i64) {
        let lapide = self.gravar_lapide_do_backup(agora);
        let corrida = self.telemetria.rodar_em_filha(
            "backup-corrida",
            "executa UMA corrida do backup agendado e morre: o panico dela volta \
             pelo `join` e vira backup que falhou, em vez de derrubar o processo \
             e rodar de novo no arranque (pedido 502)",
            "corrida",
            agora,
            |fio| {
                fio.fazendo("copiando e conferindo o SHA-256");
                self.rodar_backup_agendado(agora)
            },
        );
        let resultado = corrida.unwrap_or_else(|panico| Err(corrida_em_panico(&panico)));
        if let Some(l) = lapide {
            let _ = std::fs::remove_file(l);
        }
        match resultado {
            Ok(onde) => {
                eprintln!("backup agendado: {onde}");
                self.saude.backup_voltou();
            }
            Err(e) => {
                eprintln!("backup agendado FALHOU: {e}");
                self.avisar_backup_que_falhou(agora, &e.to_string());
            }
        }
    }

    /// Entrega ao carteiro a falha do backup agendado, se o silencio deixar --
    /// pedido 510. Sem rede aqui: quem fala com o rele e a thread da saude.
    fn avisar_backup_que_falhou(&self, agora: i64, texto: &str) {
        if let Some(evento) = self.saude.falha_do_backup(agora, texto) {
            self.saude.entregar(evento);
        }
    }

    /// Onde mora a lapide do backup agendado: no DESTINO, que e dele. Na base
    /// ela iria para dentro do proprio backup (o `backup::listar` copia tudo
    /// o que ha la), e um banco restaurado nasceria achando que caiu no meio
    /// de um backup.
    fn caminho_da_lapide_do_backup(&self) -> PathBuf {
        self.config.backup.destino.join(LAPIDE_DO_BACKUP)
    }

    /// Anota que a corrida do backup COMECOU -- pedido 502. Sem `fsync`, pelo
    /// motivo do `jobs::Registro::registrar_inicio`. `None` quando nao deu para
    /// escrever: o backup roda do mesmo jeito, e a falha dele, se vier, e a
    /// que avisa.
    fn gravar_lapide_do_backup(&self, agora: i64) -> Option<PathBuf> {
        let caminho = self.caminho_da_lapide_do_backup();
        let texto = Json::objeto(vec![
            ("quando_ms", Json::de_i64(agora)),
            (
                "base",
                Json::texto_de(self.config.base.display().to_string()),
            ),
        ])
        .escrever();
        // Pelo motor da permissao do banco (pedido 542): o destino do backup
        // nasce 0700 aqui como nasce no `backup.rs`, e a lapide 0600.
        let _ = phxsql_store::permissao::criar_diretorio_do_banco(&self.config.backup.destino);
        phxsql_store::permissao::escrever_do_banco(&caminho, texto)
            .ok()
            .map(|_| caminho)
    }

    /// A corrida do backup que derrubou o processo anterior: se a lapide esta
    /// la, e desta base, o backup NAO roda de novo na partida -- a corrida
    /// morta conta como a ultima --, e o carteiro avisa. Pedido 502, com a
    /// decisao escrita em `jobs::Registro::fechar_interrompidas`.
    pub(super) fn lapide_do_backup_no_arranque(&self) -> i64 {
        let caminho = self.caminho_da_lapide_do_backup();
        let Ok(texto) = std::fs::read_to_string(&caminho) else {
            return 0;
        };
        let Ok(j) = Json::analisar(&texto) else {
            let _ = std::fs::remove_file(&caminho);
            return 0;
        };
        // Outra base no mesmo destino: a lapide e do vizinho, e nao se mexe.
        if j.texto_ou("base", "") != self.config.base.display().to_string() {
            return 0;
        }
        let quando = j.inteiro_ou("quando_ms", 0);
        let _ = std::fs::remove_file(&caminho);
        if quando <= 0 {
            return 0;
        }
        let agora = crate::agora_ms();
        let texto = format!(
            "a corrida de {}{} nunca terminou: o servidor caiu no meio dela. Ela NAO \
             roda de novo neste arranque; o backup volta na proxima hora da agenda, \
             contada desta corrida (pedido 502)",
            phxsql_core::datahora::instante_iso(quando),
            if quando > agora {
                " (no FUTURO: o relogio voltou depois da queda, e a conta vale a \
                 partir de agora)"
            } else {
                ""
            }
        );
        eprintln!("backup agendado FALHOU: {texto}");
        self.avisar_backup_que_falhou(agora, &texto);
        // Nunca depois de agora -- C1 do parecer do DBA, pelo motivo escrito
        // no `jobs::Registro::fechar_interrompidas`: sem o `min`, uma lapide
        // do futuro deixava o backup sem rodar ate aquela data.
        quando.min(agora)
    }

    /// A COPIA de um backup -- o protocolo e o agendado passam por aqui, e e
    /// o UNICO lugar que decide sob que trava cada fase roda (pedido 513).
    ///
    /// # Duas passadas -- passo 2
    ///
    /// A FASE 1 (`fase_1`) corre SEM trava nenhuma: copia a raiz inteira e
    /// anota a ficha de cada arquivo. So o retrato do armazem esta ligado
    /// (`congelamento::comecar_retrato`), e ele serve a duas coisas: recusar
    /// a manutencao que reescreveria tabela inteira no meio, e somar os
    /// eventos por tabela tocada -- a terceira rede da fase 2.
    ///
    /// A FASE 2 (`fase_2`) e o passo 1 de antes: sob a ficha de LEITURA mais
    /// o portao do retrato, que exclui so o escritor. Ela recebe os eventos
    /// somados e acerta o destino: `stat` de tudo, recopia do que mudou. O
    /// retrato -- o instante em que a copia e consistente -- e o da fase 2,
    /// como o `BLOCK_COMMIT` do `mariabackup`.
    ///
    /// Antes do passo 2 a copia inteira rodava sob a ficha de leitura: 100 GB
    /// eram 12-18 min (arvore) ou 50-64 min (zip) com a ESCRITA parada. Agora
    /// a escrita espera so a fase 2.
    ///
    /// # Ficha compartilhada, e o portao antes dela
    ///
    /// A copia so LE `raiz`; o que ela escreve vai para o destino, e destino
    /// dentro da raiz e recusado antes do primeiro byte (`backup.rs`). Entao
    /// basta excluir quem ESCREVE -- e e o portao do retrato que exclui,
    /// segurando os escritores antes da fila do `RwLock` (`crate::retrato`
    /// diz por que na fila nao serve). Quem mais tem a ficha compartilhada
    /// sao o `varrer` e o `coletar_rowids`, que pelo tipo `Legivel` nao
    /// gravam nada (`docs/CONCORRENCIA.md` §16); o recuo deles para a
    /// exclusiva espera no portao, como qualquer escritor.
    ///
    /// # A reentrancia, ANTES do portao
    ///
    /// Uma thread com a ficha exclusiva na mao esta na conta do portao: se
    /// ela fechasse o portao, esperaria a si mesma para sempre. A pergunta
    /// vem antes, e vira o mesmo erro comum da `COM_A_TRAVA`.
    ///
    /// # Os ganchos de teste tem nome PROPRIO
    ///
    /// O `despachar` arma o panico de teste pelo nome da OPERACAO, antes de
    /// qualquer trava. Com o gancho chamado `backup`, a pausa da prova do 513
    /// disparava ali, com trava nenhuma na mao -- e a escrita passava em 1 ms.
    /// `backup_fase_1` dispara DEPOIS da fase 1 e antes da trava (a janela em
    /// que o escritor anda); o `gancho` de quem chama dispara com a ficha da
    /// fase 2 na mao.
    ///
    /// # Dois chamadores, um motor -- pedido 729
    ///
    /// O backup e o retrato da replica (`retrato_da_replica`, pedido 706)
    /// pagam a MESMA copia em duas passadas. O retrato copiava o database
    /// inteiro com a ficha EXCLUSIVA na mao; uma segunda copia em duas fases
    /// ao lado desta seria a mesma decisao escrita duas vezes. O que muda
    /// entre os dois e a ficha da FASE 2, e so ela ([`FichaDaFase2`]):
    ///
    /// - o **backup** acerta com a ficha de LEITURA e o portao fechado aos
    ///   escritores -- a copia restaurada se cura na abertura;
    /// - o **retrato** acerta com a ficha EXCLUSIVA, porque so vale com as
    ///   tabelas do database descarregadas (cabecalhos do `.log` e do `.reg`
    ///   escritos), e descarregar e escrever. Pela leitura ele precisava de
    ///   tentar ate achar o database limpo -- e um escritor em laco na mesma
    ///   tabela o sujava entre a descarga e o portao 50 vezes em 50 (medido).
    ///   Com a exclusiva a descarga e o acerto sao uma tomada so; o leitor
    ///   espera junto, e e a fase 2 -- `stat` e o que mudou --, nao a copia.
    ///
    /// Devolve tambem quanto tempo a fase 2 segurou a ficha -- o quanto um
    /// escritor espera por esta copia.
    pub(super) fn copiar_o_retrato<F, T>(
        &self,
        #[allow(unused_variables)] gancho: &str,
        raiz: &Path,
        exclusiva: bool,
        fase_1: impl FnOnce() -> Result<F>,
        fase_2: impl FnOnce(F, &std::collections::BTreeMap<PathBuf, u64>, FichaDaFase2<'_>) -> Result<T>,
    ) -> Result<(T, Duration)> {
        if COM_A_TRAVA.with(std::cell::Cell::get) {
            return Err(trava_reentrante());
        }
        let em_curso = phxsql_store::congelamento::comecar_retrato(raiz)?;
        let feito_1 = fase_1()?;
        #[cfg(test)]
        self.armar_panico_de_teste("backup_fase_1");
        let fechado = Instant::now();
        let feito = if exclusiva {
            let trava = self.travar_dados()?;
            #[cfg(test)]
            self.armar_panico_de_teste(gancho);
            let eventos = em_curso.eventos();
            let feito = fase_2(feito_1, &eventos, FichaDaFase2::Exclusiva(&trava));
            drop(em_curso);
            feito
        } else {
            let _retrato = self.retrato.tirar_retrato();
            let trava = self.travar_dados_para_ler()?;
            // So nos testes: o panico (pedido 502) ou a pausa (pedido 513)
            // com a ficha da copia na mao.
            #[cfg(test)]
            self.armar_panico_de_teste(gancho);
            let eventos = em_curso.eventos();
            let feito = fase_2(feito_1, &eventos, FichaDaFase2::Leitura(&trava));
            // O bloqueio de manutencao solta ANTES da trava e do portao, de
            // proposito: o escritor que esperou no portao a fase 2 inteira
            // entraria com o retrato ainda ligado por microssegundos e levaria
            // um 4006 de um backup que ja acabou.
            drop(em_curso);
            feito
        };
        let fechado = fechado.elapsed();
        feito.map(|f| (f, fechado))
    }

    /// O que uma corrida de backup deixou, para a resposta do protocolo e
    /// para o log do agendado -- um motor so para os dois chamadores.
    fn fazer_backup(
        &self,
        gancho: &str,
        destino: &Path,
        em_zip: bool,
        banco: &str,
        quem: &str,
        quando: i64,
    ) -> Result<BackupFeito> {
        // Fase 1: a arvore (ou a arvore temporaria do zip) sem trava; fase 2:
        // o acerto sob a ficha de leitura. O `fsync`, o manifesto e o rename
        // final ficam FORA das duas: so tocam o destino, nunca leem `raiz`,
        // e a catraca `alcancam-fsync-3` proibe alcancar `sync_all` com a
        // trava na mao (pedidos 513/524) -- ver `backup.rs`, condicoes C1 e
        // C2. O zip volta com o descritor de quem o escreveu (pedido 552).
        let inicio = Instant::now();
        let ((fase_1_ms, pronto, acerto), _) = self.copiar_o_retrato(
            gancho,
            &self.config.base,
            false,
            || {
                let t = Instant::now();
                let pronto = if em_zip {
                    // Sem espaco para a arvore temporaria, a copia cai na
                    // passada unica DIZENDO (`modo: retrato_inteiro`): quem
                    // tem zip funcionando hoje nao passa a falhar.
                    let livre = self.espaco_livre_em(destino);
                    match phxsql_store::backup::copiar_fase_1_para_zip(
                        &self.config.base,
                        destino,
                        banco,
                        quem,
                        quando,
                        livre,
                    )? {
                        Some(fase) => Pronto::Zip(fase),
                        None => Pronto::ZipSemEspaco {
                            livre: livre.unwrap_or(0),
                        },
                    }
                } else {
                    // Sem `fsync` aqui (pedido 646): sincronizar o grosso com o
                    // escritor andando deu picos de ~0,4 s a 1 GB e nada
                    // depois; o `concluir` sincroniza tudo, fora da trava.
                    Pronto::Arvore(phxsql_store::backup::copiar_fase_1(
                        &self.config.base,
                        destino,
                    )?)
                };
                Ok((t.elapsed().as_millis() as u64, pronto))
            },
            |(fase_1_ms, pronto), eventos, _| {
                let (pronto, acerto) = match pronto {
                    Pronto::Arvore(mut fase) => {
                        let acerto = phxsql_store::backup::acertar_fase_2(&mut fase, eventos)?;
                        (Pronto::Arvore(fase), Some(acerto))
                    }
                    Pronto::Zip(mut fase) => {
                        let acerto = phxsql_store::backup::acertar_fase_2(&mut fase, eventos)?;
                        (Pronto::Zip(fase), Some(acerto))
                    }
                    Pronto::ZipSemEspaco { livre } => {
                        let (zip, r) = phxsql_store::backup::executar_zip(
                            &self.config.base,
                            destino,
                            banco,
                            quem,
                            quando,
                        )?;
                        (Pronto::ZipInteiro { zip, r, livre }, None)
                    }
                    inteiro @ Pronto::ZipInteiro { .. } => (inteiro, None),
                };
                Ok((fase_1_ms, pronto, acerto))
            },
        )?;
        let mut feito = BackupFeito {
            destino: destino.to_path_buf(),
            fase_1_ms,
            acerto,
            ..BackupFeito::default()
        };
        match pronto {
            Pronto::Arvore(fase) => {
                let (r, copias) = phxsql_store::backup::terminar(fase);
                phxsql_store::backup::concluir(destino, quando, &r, &copias)?;
                feito.r = r;
            }
            Pronto::Zip(fase) => {
                let (zip, r) = phxsql_store::backup::concluir_zip(fase)?;
                phxsql_store::backup::finalizar_zip(&zip)?;
                feito.arquivo = Some(zip.to_path_buf());
                feito.r = r;
            }
            Pronto::ZipInteiro { zip, r, livre } => {
                phxsql_store::backup::finalizar_zip(&zip)?;
                feito.arquivo = Some(zip.to_path_buf());
                feito.r = r;
                feito.sem_espaco = Some(livre);
            }
            Pronto::ZipSemEspaco { .. } => unreachable!("a fase 2 troca este estado"),
        }
        feito.ms = inicio.elapsed().as_millis() as u64;
        Ok(feito)
    }

    /// O espaco livre, em bytes, no disco de `caminho` -- pelo `df`, o mesmo
    /// motor do painel (`sistema::espaco`). `None` quando nao se mede (fora
    /// do Linux, ou pasta que ainda nao existe e cujo pai tampouco).
    pub(super) fn espaco_livre_em(&self, caminho: &Path) -> Option<u64> {
        let mut existente = caminho;
        while !existente.exists() {
            existente = existente.parent()?;
        }
        crate::sistema::espaco(&[existente])
            .first()
            .map(|e| e.livre_kb.saturating_mul(1024))
    }

    fn rodar_backup_agendado(&self, quando: i64) -> Result<String> {
        let b = &self.config.backup;
        // A trava de cada fase e decidida no `copiar_o_retrato`, o MESMO do
        // `op_backup` -- e la que mora o gancho de teste do panico (pedido
        // 502), com a ficha COMPARTILHADA da fase 2 na mao (pedido 513).
        let destino = if b.zip {
            b.destino.clone()
        } else {
            b.destino
                .join(phxsql_core::datahora::instante_iso(quando).replace([' ', ':', ','], "-"))
        };
        let feito = self.fazer_backup(
            "backup_agendado",
            &destino,
            b.zip,
            &b.database,
            &b.admin,
            quando,
        )?;
        let onde = feito.onde().display().to_string();

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
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
        });

        let apagados = self.limpar_backups_velhos();
        Ok(format!(
            "{onde} ({} arquivos, {} bytes{}{}{}{})",
            feito.r.arquivos.len(),
            feito.r.bytes,
            if b.zip {
                format!(", zip de {} bytes", feito.r.comprimido)
            } else {
                String::new()
            },
            match &feito.acerto {
                Some(a) => format!(
                    ", fase 2 em {} ms: {} arquivo(s), {} bytes",
                    a.ms, a.arquivos, a.bytes
                ),
                None => String::new(),
            },
            match feito.sem_espaco {
                Some(livre) => format!(
                    ", retrato inteiro sob a trava: sem espaco para a arvore temporaria \
                     ({livre} bytes livres)"
                ),
                None => String::new(),
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
        let nomes: Vec<String> = dir
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();
        let mut apagados = 0;
        for nome in phxsql_store::backup::escolher_para_apagar(&nomes, b.manter) {
            if std::fs::remove_file(b.destino.join(&nome)).is_ok() {
                apagados += 1;
            }
        }
        apagados
    }

    /// Copia de seguranca: a escrita espera a copia inteira, a leitura nao
    /// (pedido 513 -- ver [`Servidor::copiar_o_retrato`]).
    ///
    /// `"zip": true` faz um arquivo unico chamado
    /// `Banco_Admin_Data_HoraMin.zip`, com o manifesto dentro. Sem isso,
    /// copia a arvore de diretorios como antes.
    pub(super) fn op_backup(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \"destino\"".into()));
        }
        let quando = crate::agora_ms();
        let em_zip = p.booleano_ou("zip", false);
        let banco = p.texto_ou("database", "").trim().to_string();
        // Quem fez entra no nome do arquivo. Sem login, entrou pelo token de
        // servico -- e o nome diz isso, em vez de fingir um usuario.
        let quem = if sessao.login().is_empty() {
            "servico".to_string()
        } else {
            sessao.login().to_string()
        };

        // As duas fases, a trava de cada uma e o `fsync` fora delas sao
        // decididos no `fazer_backup`, o MESMO do backup agendado.
        let feito = self.do_caminho_pedido(
            self.fazer_backup(
                "backup_copia",
                std::path::Path::new(&destino),
                em_zip,
                &banco,
                &quem,
                quando,
            ),
            &destino,
            false,
        )?;
        Ok(Json::objeto(feito.para_json()))
    }

    /// Separa o `Io` que o CAMINHO DO PEDIDO causou do `Io` do disco do banco
    /// (pedido 641). `anotar` dispara o aviso de saude do disco -- e o gancho
    /// que EXECUTA um programa do operador -- por todo `PhxError::Io` (5001);
    /// um `destino` inexistente ou sem permissao, digitado por um usuario
    /// autenticado, nao e disco doente, e gastaria o aviso (e o SMS pago) a
    /// cada erro de digitacao.
    ///
    /// `qualquer_io`: a operacao SO le o caminho pedido (conferir, restaurar
    /// a leitura do manifesto), entao todo `Io` e dele. Em `false` a operacao
    /// tambem le o banco (o backup), e so as formas de erro que descrevem
    /// CAMINHO ou PERMISSAO -- nao existe, sem permissao, nao e diretorio --
    /// saem do alerta; `EIO`, `ENOSPC`, `EROFS` e `EDQUOT` continuam `Io`,
    /// porque sao exatamente o que o aviso existe para pegar.
    ///
    /// A frase sai da tabela de mensagens (`erro.caminho_inutilizavel`,
    /// pedido 673): e o servidor quem a monta inteira, como os portoes. O
    /// `{motivo}` e o texto do sistema operacional e fica como veio -- e dado.
    pub(super) fn do_caminho_pedido<T>(
        &self,
        r: Result<T>,
        caminho: &str,
        qualquer_io: bool,
    ) -> Result<T> {
        r.map_err(|e| match e {
            PhxError::Io(io)
                if qualquer_io
                    || matches!(
                        io.kind(),
                        std::io::ErrorKind::NotFound
                            | std::io::ErrorKind::PermissionDenied
                            | std::io::ErrorKind::NotADirectory
                    ) =>
            {
                PhxError::Esquema(self.msg(
                    "erro.caminho_inutilizavel",
                    &[
                        ("caminho", &format!("{caminho:?}")),
                        ("motivo", &io.to_string()),
                    ],
                ))
            }
            outro => outro,
        })
    }

    /// Confere `.reg` contra `.bkp` e conserta o que der.
    pub(super) fn op_reparar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let _trava = self.travar_dados()?;
        let mut t = self.abrir_travada(&_trava, p, sessao)?;
        let (conferidos, reparados, perdidos) = t.reparar()?;
        self.gravar_de_verdade(&_trava, &mut t, p)?;
        Ok(Json::objeto(vec![
            ("conferidos", Json::de_u64(conferidos)),
            ("reparados", Json::de_u64(reparados)),
            ("perdidos", Json::de_u64(perdidos)),
            ("integro", Json::Bool(perdidos == 0)),
        ]))
    }

    pub(super) fn op_conferir_backup(&self, p: &Json) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \"destino\"".into()));
        }
        let r = self.do_caminho_pedido(
            phxsql_store::backup::conferir(std::path::Path::new(&destino)),
            &destino,
            true,
        )?;
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

    /// `backups`: o que ha na pasta, com o que cada arquivo traz dentro.
    ///
    /// Sem esta operacao, restaurar comeca por digitar um caminho de cabeca --
    /// e o backup que se acha digitando e o que a gente lembra, nao o que
    /// existe. De cada arquivo le SO o manifesto: num ZIP isso e o fim do
    /// arquivo mais uma entrada, entao listar dez copias de um gigabyte nao
    /// custa dez gigabytes.
    ///
    /// Arquivo ilegivel entra na lista dizendo que e ilegivel. Sumir com ele
    /// seria esconder justamente o backup que precisa de atencao.
    pub(super) fn op_backups(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let pedida = p.texto_ou("pasta", "").trim().to_string();
        let caminho = if pedida.is_empty() {
            self.config.backup.destino.clone()
        } else {
            std::path::PathBuf::from(&pedida)
        };
        let pasta = caminho.display().to_string();
        let mut itens = Vec::new();
        let mut escondidos = 0u64;
        if caminho.is_dir() {
            let mut achados: Vec<std::path::PathBuf> = std::fs::read_dir(&caminho)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|c| {
                    // O que tem cara de backup nosso: um `.zip` ou uma pasta
                    // com manifesto dentro. O resto da pasta nao e da nossa
                    // conta -- alguem pode guardar outra coisa ali.
                    c.extension().and_then(|e| e.to_str()) == Some("zip")
                        || c.join(phxsql_store::backup::MANIFESTO).is_file()
                })
                .collect();
            // Do mais novo para o mais velho: o nome ja ordena por data, e o
            // backup que alguem procura e quase sempre o ultimo.
            achados.sort();
            achados.reverse();

            for arquivo in achados {
                let nome = arquivo
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let tamanho = std::fs::metadata(&arquivo).map(|m| m.len()).unwrap_or(0);
                let mut campos = vec![
                    ("nome", Json::texto_de(&nome)),
                    ("caminho", Json::texto_de(arquivo.display().to_string())),
                    ("no_disco", Json::de_u64(tamanho)),
                ];
                match phxsql_store::restaurar::conteudo(&arquivo) {
                    Ok(c) => {
                        // Banco que esta sessao nao administra nao aparece na
                        // lista de restauraveis: oferecer o que o portao vai
                        // negar e mandar montar um pedido para ouvir nao.
                        let (visiveis, ocultos): (Vec<String>, Vec<String>) = c
                            .databases
                            .into_iter()
                            .partition(|db| self.poder_no_backup(sessao, db).is_ok());
                        escondidos += ocultos.len() as u64;
                        campos.extend([
                            ("legivel", Json::Bool(true)),
                            ("zip", Json::Bool(c.zip)),
                            ("quando", Json::texto_de(&c.quando)),
                            ("phxsql", Json::texto_de(&c.versao)),
                            ("arquivos", Json::de_u64(c.arquivos as u64)),
                            ("bytes", Json::de_u64(c.bytes)),
                            (
                                "escopo",
                                Json::texto_de(match c.escopo {
                                    phxsql_store::restaurar::Escopo::Raiz => "raiz",
                                    phxsql_store::restaurar::Escopo::Database(_) => "database",
                                }),
                            ),
                            // Diz se o escopo veio ESCRITO ou foi deduzido: o
                            // backup mais velho que a restauracao nao traz o
                            // campo, e a tela nao deve afirmar o que deduziu.
                            ("escopo_declarado", Json::Bool(c.declarado)),
                            (
                                "databases",
                                Json::Lista(visiveis.iter().map(Json::texto_de).collect()),
                            ),
                        ]);
                    }
                    Err(e) => campos.extend([
                        ("legivel", Json::Bool(false)),
                        ("erro", Json::texto_de(e.to_string())),
                    ]),
                }
                itens.push(Json::objeto(campos));
            }
        }
        Ok(Json::objeto(vec![
            ("pasta", Json::texto_de(&pasta)),
            ("existe", Json::Bool(caminho.is_dir())),
            ("total", Json::de_u64(itens.len() as u64)),
            ("escondidos", Json::de_u64(escondidos)),
            ("backups", Json::Lista(itens)),
        ]))
    }

    /// O portao PROPRIO da restauracao: o poder sobre o que vem de DENTRO do
    /// backup.
    ///
    /// O portao geral confere o campo `"database"` do pedido -- que aqui e o
    /// DESTINO, o nome novo. O database que vem de dentro do backup nao tem
    /// campo nenhum no pedido, e e ele que carrega o dado. Sem esta
    /// conferencia bastaria administrar um banco de rascunho para restaurar
    /// dentro dele o backup da folha de pagamento e ler tudo: a mesma porta
    /// dos fundos do `juntar` e do `unir`, num campo que o portao geral nao
    /// tem como enxergar.
    fn poder_no_backup(&self, sessao: &Sessao, database: &str) -> Result<()> {
        let Some(usuario) = sessao.usuario.as_ref() else {
            // Sem usuario e o token de servico, que ja passou pelo portao 1.
            return Ok(());
        };
        if usuario.pode_em(database, "", Atividade::Administrar) {
            return Ok(());
        }
        Err(PhxError::Autorizacao(format!(
            "{} nao tem permissao de administrar em {database}, que e o \
             database que vem dentro deste backup",
            usuario.login
        )))
    }

    /// `restaurar_backup`: um database de dentro de um backup vira um database
    /// deste servidor.
    ///
    /// # Os dois modos
    ///
    /// * `"modo":"novo"` (o padrao) grava com OUTRO nome. Nao destroi nada,
    ///   nao precisa da porta de dados parada, e a copia inteira acontece fora
    ///   da trava -- so a troca final, que e um `rename`, entra nela.
    /// * `"modo":"por_cima"` substitui um database que ja existe. Exige
    ///   `"confirmar":true` e a **porta de dados parada**, porque o que ela
    ///   troca e o chao debaixo de quem esta lendo. O database anterior nao e
    ///   apagado: sai da raiz e o caminho volta na resposta.
    ///
    /// `"simular":true` le e confere o manifesto e devolve o que ha dentro,
    /// sem escrever nada -- e o que a tela usa para mostrar o conteudo antes
    /// de alguem decidir.
    pub(super) fn op_restaurar_backup(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use phxsql_store::restaurar::{Escopo, Preparada};

        let origem = p.texto_ou("origem", "").trim().to_string();
        if origem.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"origem\": o .zip ou a pasta do backup".into(),
            ));
        }
        let caminho = std::path::PathBuf::from(&origem);
        let conteudo =
            self.do_caminho_pedido(phxsql_store::restaurar::conteudo(&caminho), &origem, true)?;

        // O PITR e lido AQUI, antes de o pedido tocar em disco. Todas as
        // recusas que dao para conferir sem restaurar acontecem antes da
        // restauracao -- ver `reaplicar_diario_ate`.
        let ate_ms = Self::instante_pedido(p, "ate", "ate_ms")?;

        // Qual database de dentro do backup. Quando so ha um, nao ha o que
        // escolher -- pedir o nome de qualquer jeito seria burocracia.
        let pedido_de = p.texto_ou("de", "").trim().to_string();
        let de = match (&conteudo.escopo, pedido_de.as_str()) {
            (Escopo::Database(n), "") => n.clone(),
            (_, "") if conteudo.databases.len() == 1 => conteudo.databases[0].clone(),
            (_, "") => {
                return Err(PhxError::Esquema(format!(
                    "este backup tem {} databases dentro; diga em \"de\" qual restaurar: {}",
                    conteudo.databases.len(),
                    conteudo.databases.join(", ")
                )))
            }
            (_, escolhido) => escolhido.to_string(),
        };
        if phxsql_store::catalogo::nome_hostil(&de) {
            return Err(PhxError::Esquema(format!("database {de:?} invalido")));
        }
        self.poder_no_backup(sessao, &de)?;

        let simular = p.booleano_ou("simular", false);
        let tabelas = phxsql_store::restaurar::tabelas_de(&caminho, &de)?;
        if simular {
            return Ok(Json::objeto(vec![
                ("simulado", Json::Bool(true)),
                ("origem", Json::texto_de(&origem)),
                ("zip", Json::Bool(conteudo.zip)),
                ("de", Json::texto_de(&de)),
                ("quando", Json::texto_de(&conteudo.quando)),
                ("phxsql", Json::texto_de(&conteudo.versao)),
                ("arquivos", Json::de_u64(conteudo.arquivos as u64)),
                ("bytes", Json::de_u64(conteudo.bytes)),
                ("escopo_declarado", Json::Bool(conteudo.declarado)),
                (
                    "databases",
                    Json::Lista(conteudo.databases.iter().map(Json::texto_de).collect()),
                ),
                (
                    "tabelas",
                    Json::Lista(tabelas.iter().map(Json::texto_de).collect()),
                ),
                (
                    "aviso",
                    Json::texto_de(
                        "nada foi escrito: isto e so a leitura do manifesto. O SHA-256 \
                         de cada arquivo e conferido na restauracao de verdade, antes \
                         de o destino ser tocado",
                    ),
                ),
            ]));
        }

        let destino = p.texto_ou("database", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\": o nome com que o backup vai ser restaurado".into(),
            ));
        }
        let modo = p.texto_ou("modo", "novo").trim().to_lowercase();
        let por_cima = match modo.as_str() {
            "novo" | "" => false,
            "por_cima" | "porcima" | "substituir" => true,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "modo {outro:?} nao existe: use \"novo\" ou \"por_cima\""
                )))
            }
        };

        if por_cima {
            // Confirmacao explicita, como no `esvaziar_lixeira`: e a unica
            // operacao aqui que substitui um database inteiro, e um cliente
            // que erre o campo `modo` nao pode substituir nada por acidente.
            if !p.booleano_ou("confirmar", false) {
                return Err(PhxError::Esquema(format!(
                    "restaurar POR CIMA substitui o database {destino} inteiro. \
                     Mande \"confirmar\":true junto se e isso mesmo"
                )));
            }
            // A porta de dados parada e o que torna a troca honesta: com ela no
            // ar, um cliente pode estar no meio de uma leitura do database que
            // sai debaixo dele. A interface web NAO para junto -- e por ela que
            // se restaura e por ela que a porta volta.
            if self.porta_no_ar.load(Ordering::SeqCst) {
                return Err(PhxError::Esquema(
                    "para restaurar POR CIMA, pare a porta de dados antes \
                     ({\"op\":\"servico_parar\"}, ou o botao Start/Stop). A interface \
                     web continua no ar, e e por ela que a porta volta. Restaurar com \
                     OUTRO nome nao exige nada disso"
                        .into(),
                ));
            }
            let abertas = self.permissoes_de_dados.em_uso();
            if abertas > 0 {
                return Err(PhxError::Esquema(format!(
                    "a porta de dados esta parada, mas {abertas} conexao(oes) continuam \
                     abertas e podem estar lendo {destino}. Espere elas acabarem ou \
                     encerre pela tela de conexoes"
                )));
            }
        }

        // ------------------------------------------------------------ PITR
        //
        // As cinco recusas que se conferem SEM tocar em disco. Uma restauracao
        // que criasse o database e so entao descobrisse que nao consegue
        // reaplicar deixaria o pior estado possivel: um banco novo com o nome
        // pedido, no instante errado, e um erro na resposta.
        if let Some(ate_ms) = ate_ms {
            if por_cima {
                // A restauracao por cima TIRA o database vivo da raiz de dados
                // -- e e o diario dele que o PITR reaplica. As duas coisas na
                // mesma operacao se cancelam.
                return Err(PhxError::Esquema(format!(
                    "restaurar POR CIMA tira o database {destino} do lugar, e e o \
                     diario VIVO dele que o \"ate\" reaplica. Restaure com OUTRO \
                     nome, confira, e so entao decida o que fazer com o original"
                )));
            }
            let Some(copia_ms) = conteudo.quando_ms else {
                return Err(PhxError::Esquema(
                    "o manifesto deste backup nao diz em que instante a copia foi \
                     tirada: falta o \"quando_ms\" do backup.json, e o \"quando\" \
                     que ele traz nao e um instante legivel. Sem isso nao da para \
                     saber de ONDE reaplicar o diario. O campo entrou com o PITR, \
                     e uma copia nova feita por este servidor o traz. Sem \"ate\", \
                     a restauracao simples continua funcionando"
                        .into(),
                ));
            };
            if ate_ms < copia_ms {
                return Err(PhxError::Esquema(format!(
                    "\"ate\" ({}) e ANTES do instante da copia ({}): o diario so \
                     sabe andar para a frente. Escolha um backup mais antigo",
                    phxsql_core::datahora::instante_iso(ate_ms),
                    phxsql_core::datahora::instante_iso(copia_ms)
                )));
            }
            // O interruptor da imagem, conferido no config e nao no diario: sem
            // ele o evento diz que o rowid 42 mudou e nao diz PARA QUE. O
            // `aplicar_evento` recusa evento a evento com a mesma mensagem, mas
            // descobrir isso depois de restaurar seria descobrir tarde.
            if !self.config.replicacao.imagem_da_linha {
                return Err(PhxError::Esquema(
                    "o diario deste servidor nao guarda a imagem da linha, e sem ela \
                     um evento nao da para reaplicar. Ligue \
                     \"replicacao\": {\"imagem_da_linha\": true} no config.json -- \
                     ela vale para o que for gravado DAQUI EM DIANTE, e nao para o \
                     diario que ja esta no disco"
                        .into(),
                ));
            }
            // O diario vivo mora no database de ORIGEM. Se ele nao existe mais
            // aqui, nao ha o que reaplicar -- e restaurar a copia inteira
            // chamando aquilo de PITR seria mentir sobre o instante.
            let existe = {
                let trava = self.travar_dados()?;
                trava.abrir_database(&de).is_ok()
            };
            if !existe {
                return Err(PhxError::NaoEncontrado(format!(
                    "o database {de} nao existe mais neste servidor, e e o diario \
                     VIVO dele que o \"ate\" reaplica. Sem \"ate\", a restauracao \
                     simples continua funcionando"
                )));
            }
        }

        // O caro acontece FORA da trava: ler o backup, conferir o SHA-256 de
        // cada arquivo e escrever o palco. Segurar a trava por esse tempo
        // pararia o servidor inteiro pela duracao da copia.
        let inicio = Instant::now();
        let preparada = Preparada::preparar(
            &caminho,
            &self.config.base,
            &de,
            politica_do_diario(&self.config),
        )?;

        // O PITR acontece NO PALCO, antes de o database entrar na raiz.
        //
        // A ordem era a inversa -- confirmar e so entao reaplicar --, e ela
        // custava as duas coisas que esta troca compra:
        //
        //  * uma JANELA em que outra sessao enxerga o database restaurado no
        //    instante da copia, sem os eventos reaplicados. Restaurar deixava
        //    de ser atomico para quem le, e ninguem tinha como saber se o que
        //    estava vendo era o fim ou o meio;
        //  * o `fsync` da reaplicacao com a TRAVA GLOBAL na mao, porque depois
        //    do `rename` a tabela restaurada tem segundo dono possivel e
        //    escrever nela sem a ficha seria corrupcao. No palco nao ha segundo
        //    dono: o `fsync` sai da trava sem abrir mao de nada.
        //
        // E um terceiro, de graca: erro DURO na reaplicacao (o disco que
        // acabou, o `.log` ilegivel) nao deixa mais um database criado ao lado
        // de uma resposta de fracasso -- o `Drop` da `Preparada` leva o palco
        // junto. A recusa POR TABELA continua saindo em `parou_em`, como
        // antes: ela nao e fracasso da restauracao.
        let pitr = match ate_ms {
            // O `copia_ms` volta do manifesto -- foi conferido la em cima.
            Some(ate_ms) => Some(self.reaplicar_diario_ate(
                &de,
                preparada.palco(),
                conteudo.quando_ms.unwrap_or_default(),
                ate_ms,
            )?),
            None => None,
        };

        // A troca entra na trava: e um `rename`, e o que ela impede e dois
        // pedidos criarem o mesmo database ao mesmo tempo.
        let r = {
            let _trava = self.travar_dados()?;
            preparada.confirmar(&self.config.base, &destino, por_cima)?
        };

        // O `ms` passa a contar a reaplicacao junto, e e o certo: ele responde
        // «quanto demorou este pedido», e a reaplicacao e parte dele.
        let mut campos = vec![
            ("database", Json::texto_de(&r.database)),
            ("de", Json::texto_de(&r.de)),
            ("origem", Json::texto_de(&origem)),
            (
                "modo",
                Json::texto_de(if por_cima { "por_cima" } else { "novo" }),
            ),
            ("arquivos", Json::de_u64(r.arquivos as u64)),
            ("bytes", Json::de_u64(r.bytes)),
            (
                "tabelas",
                Json::Lista(r.tabelas.iter().map(Json::texto_de).collect()),
            ),
            ("substituiu", Json::Bool(r.substituiu)),
            ("ms", Json::de_u64(inicio.elapsed().as_millis() as u64)),
        ];
        // So quando houve (pedido 522): o indice que chegou marcado na copia
        // e foi reconstruido, e o que nao reconstruiu -- este, alto, porque a
        // tabela restaurada recusa ate alguem mandar `reindexar`.
        if r.indices_reconstruidos > 0 {
            campos.push((
                "indices_reconstruidos",
                Json::de_u64(r.indices_reconstruidos as u64),
            ));
        }
        if !r.indices_pendentes.is_empty() {
            campos.push((
                "indices_pendentes",
                Json::Lista(r.indices_pendentes.iter().map(Json::texto_de).collect()),
            ));
        }
        if let Some(pitr) = pitr {
            campos.push(("pitr", pitr));
        }
        if let Some(onde) = &r.anterior_em {
            campos.push(("anterior_em", Json::texto_de(onde)));
            campos.push((
                "aviso",
                Json::texto_de(
                    "o database que estava aqui NAO foi apagado: esta em \"anterior_em\", \
                     fora da raiz de dados. Confira o restaurado antes de apagar aquilo",
                ),
            ));
        }
        Ok(Json::objeto(campos))
    }

    /// Le um instante do pedido, por texto (`ate`) ou por numero (`ate_ms`).
    ///
    /// # Por que os dois, e por que o texto e o principal
    ///
    /// O numero e exato e nao se digita: quem escreve um pedido a mao escreve
    /// `2026-09-08T15:00:00Z`, que e a forma do contrato e a que uma pessoa
    /// consegue conferir olhando. O numero existe para quem ja tem o instante
    /// na mao -- o `carimbo_ms` que o proprio `diario` devolveu, por exemplo --
    /// e assim nao precisa formatar para o servidor voltar a ler.
    ///
    /// Os dois juntos e RECUSA, e nao "um deles ganha": dois campos de tempo
    /// no mesmo pedido querendo dizer coisas diferentes e um pedido que quem
    /// escreveu nao entende, e escolher por ele esconderia o engano.
    pub(super) fn instante_pedido(p: &Json, campo: &str, campo_ms: &str) -> Result<Option<i64>> {
        let texto = p.texto_ou(campo, "").trim().to_string();
        let numero = p.campo(campo_ms).and_then(Json::inteiro);
        if !texto.is_empty() && numero.is_some() {
            return Err(PhxError::Esquema(format!(
                "mande \"{campo}\" OU \"{campo_ms}\", nao os dois"
            )));
        }
        if let Some(n) = numero {
            return Ok(Some(n));
        }
        if texto.is_empty() {
            return Ok(None);
        }
        phxsql_core::datahora::ms_de_instante_iso(&texto)
            .map(Some)
            .ok_or_else(|| {
                // O valor pelo `citar`: curto e o diagnostico, longo vira o
                // tamanho -- sem teto, um megabyte ia inteiro ao `acessos.log`
                // (parecer SEC do 497, P1; a familia do 453).
                PhxError::Esquema(format!(
                    "\"{campo}\": {} nao e um instante. Escreva \
                     2026-09-08T15:00:00Z (tudo em UTC -- fuso escrito na mao e \
                     recusado em vez de ignorado), ou mande os milissegundos em \
                     \"{campo_ms}\"",
                    phxsql_core::error::citar(&texto)
                ))
            })
    }

    /// **PITR** -- a copia restaurada vira REPLICA do diario vivo, da hora da
    /// copia ate o instante pedido.
    ///
    /// # A ideia inteira, em uma linha
    ///
    /// Um backup e um retrato do instante T0. O diario de cada tabela guarda
    /// tudo que aconteceu depois. Entao restaurar a um instante T e restaurar
    /// o retrato e **reaplicar o diario de T0 ate T** -- e isso ja existe
    /// pronto nesta casa com outro nome: e o que uma replica faz. O PITR nao
    /// escreve um segundo aplicador; ele chama o mesmo
    /// [`Table::aplicar_evento`] da replicacao, que **aplica e nao julga**. Um
    /// segundo caminho seria o caminho que um dia esquece uma conferencia.
    ///
    /// # De onde sai o COMECO, e por que ele nao sai do relogio
    ///
    /// O `.log` viaja dentro do backup -- medido: uma copia tirada com um
    /// evento no diario tem `c.log` com um evento, e o `.log` vivo passa a ter
    /// dois, com o primeiro **byte a byte igual** ao copiado. Entao o diario
    /// da copia diz, sozinho, a POSICAO em que o mundo estava na hora da
    /// copia: `td.eventos()`. Comecar dali e exato; comecar por «o primeiro
    /// evento cujo carimbo passou de T0» dependeria do relogio para achar o
    /// comeco, e o relogio e justamente a parte fraca (ver abaixo).
    ///
    /// O relogio entra **so no corte de cima**, o `ate`.
    ///
    /// # O carimbo NAO e monotonico -- medido
    ///
    /// Dois motivos, os dois no codigo: o `agora_ms` e relogio de parede
    /// (`SystemTime::now`, `store/util.rs`), que anda para tras num acerto de
    /// NTP; e o caminho bidirecional carimba o evento com o relogio de OUTRO
    /// servidor (`Table::forcar_proximo_evento`), porque e o instante do
    /// NASCIMENTO da escrita que decide o conflito la. Medido aqui, num diario
    /// de tres eventos: `[1788884516705, 1000000000000, 1788884516705]` -- o
    /// do meio vinte e cinco anos atras, com `origem: 7`.
    ///
    /// Por isso o filtro e **evento a evento** (`carimbo <= ate`), e nunca
    /// «corte a lista no primeiro que passou». Cortar por posicao jogaria fora
    /// os eventos bons que vem depois de um carimbo torto.
    ///
    /// E a consequencia de pular um evento no meio e uma GUARDA, nao um
    /// defeito: o `aplicar_evento` confere o rowid, entao pular uma inclusao e
    /// aplicar a seguinte para na hora, com «o source diz rowid 2 e aqui saiu
    /// 1». O `parou_em` da resposta diz isso, em vez de gravar a linha errada
    /// no slot errado.
    ///
    /// # O que ele NAO refaz, e por que
    ///
    /// * **Cascata.** O `aplicar_evento` acende `como_replica`, e com ela o
    ///   `julga_integridade` cala. A origem ja cascateou quando aceitou a
    ///   escrita, e os eventos que a cascata dela gerou estao no diario das
    ///   FILHAS -- refazer aqui criaria evento que o original nunca teve.
    /// * **Chave estrangeira.** Pelo mesmo portao e pelo mesmo motivo: a
    ///   reaplicacao anda por tabela, e nao ha ordem global entre tabelas.
    ///   Filha orfa no meio da passada se cura quando a tabela da mae for
    ///   reaplicada; recusar travaria a restauracao inteira por uma ordem que
    ///   se resolve sozinha.
    /// * **O `.tx`.** A marca de commit em curso ja viaja dentro do backup de
    ///   proposito, e quem a completa e a recuperacao do arranque
    ///   (`transacao::recuperar`, chamada em `Servidor::novo`) -- e ela e
    ///   **idempotente pelo rowid**, entao reaplicar o diario por cima nao
    ///   duplica inclusao nenhuma. O PITR nao mexe nela: dois donos para o
    ///   mesmo commit seria um a mais.
    ///
    /// # As tabelas que existem de um lado so
    ///
    /// * **Na copia e nao mais viva** (foi apagada depois do backup): fica
    ///   como estava na copia, e sai na resposta em `sem_diario_vivo`. Nao da
    ///   para saber QUANDO ela foi apagada -- apagar tabela nao deixa evento
    ///   em diario nenhum --, e sumir com ela em silencio seria pior.
    /// * **Viva e nao na copia** (nasceu depois do backup): **nao e criada.**
    ///   Refaze-la exigiria a historia do esquema, que o formato nao guarda.
    ///   Sai em `novas_na_origem`, para quem restaurou saber o que falta.
    ///
    /// # `ate` e INCLUSIVO
    ///
    /// `carimbo <= ate`. Quem pede «ate as 15:00:00» quer o que aconteceu as
    /// 15:00:00,000 -- e o instante que a tela mostra e o instante que se
    /// digita de volta.
    ///
    /// # Por que a recusa por tabela nao aborta a restauracao
    ///
    /// As recusas que dao para conferir ANTES de tocar em disco acontecem
    /// antes (ver `op_restaurar_backup`): `ate` ilegivel, backup sem carimbo,
    /// `ate` anterior a copia, modo por cima, interruptor da imagem desligado,
    /// database vivo que sumiu. Sobra a continuidade, que so se confere com o
    /// diario da copia na mao -- ou seja, com a copia ja extraida. Ela sai
    /// **nomeada, por tabela**, no `parou_em`: aquela tabela ficou no instante
    /// da copia, e a resposta diz qual e por que. Nao e fracasso da
    /// restauracao, e por isso nao aborta.
    ///
    /// # Onde isto escreve, e por que o `fsync` fica FORA da trava
    ///
    /// O destino e o **palco** -- o database ja extraido e conferido, ainda
    /// fora da raiz de dados (ver [`Preparada::palco`]). Ele e uma raiz de
    /// dados particular deste pedido: a ficha exclusiva dele e a variavel
    /// local `raiz_do_palco`, e nao a trava global, porque nao ha segundo dono
    /// possivel de um diretorio cujo nome so quem preparou conhece.
    ///
    /// A trava GLOBAL entra so pelo lado VIVO: abrir o database de origem e
    /// ler o diario dele exige a ficha, porque ali ha escritor concorrente.
    /// Ela sai por `drop` explicito **antes** do `fsync` -- que e o pedaco
    /// caro, uns 10 `sync_all` por tabela. Reaplicar com a trava na mao e
    /// necessario; sincroniza-la com a trava na mao nao era, e era o que fazia
    /// esta secao aparecer na catraca `alcancam-fsync` do mapa da trava
    /// (pedido 252, decisao do dono de 18/09/2026).
    ///
    /// O que fica aberto ate la sao as tabelas que RECEBERAM evento, e so
    /// elas: cada `Table` aberta carrega o cache de paginas do `.ndx` (teto de
    /// `recursos.cache_paginas`), entao segurar todas as restauradas pagaria
    /// RAM por tabela que nem foi tocada.
    fn reaplicar_diario_ate(
        &self,
        de: &str,
        palco: &Path,
        copia_ms: i64,
        ate_ms: i64,
    ) -> Result<Json> {
        // Lotes, e nao o diario inteiro: um `.log` de meio milhao de eventos
        // com imagem nao cabe em RAM de uma vez. Mesmo teto do `replicar`.
        const LOTE: u64 = 500;

        // O palco visto como raiz de dados: o pai dele e a base, e o nome do
        // diretorio e o "database". `Raiz::nova` nao cria nada que ja nao
        // exista -- o pai e o vizinho da raiz, onde o palco ja esta.
        let (pai, nome_no_palco) = match (palco.parent(), palco.file_name()) {
            (Some(p), Some(n)) => (p.to_path_buf(), n.to_string_lossy().into_owned()),
            _ => {
                return Err(PhxError::Esquema(format!(
                    "o palco da restauracao ({}) nao tem pai e nome: sem eles \
                     nao da para abrir a copia para reaplicar o diario",
                    palco.display()
                )))
            }
        };
        let mut raiz_do_palco = Raiz::nova(&pai)?;
        // O restaurado tambem grava imagem no diario dele, pelo mesmo motivo
        // da replica intermediaria: sem isso, um backup TIRADO do restaurado
        // nasce sem o que reaplicar. A mesma politica do servidor vivo.
        raiz_do_palco.definir_politica_do_diario(politica_do_diario(&self.config));
        let db_destino = raiz_do_palco.exclusiva().abrir_database(&nome_no_palco)?;
        // O catalogo de verdade, e nao a vitrine dos nomes de arquivo: depois
        // de extraida quem responde "que tabelas ha aqui" e o diretorio.
        let restauradas = db_destino.todas_as_tabelas()?;

        let trava = self.travar_dados()?;
        let db_vivo = trava.abrir_database(de)?;
        let vivas = db_vivo.todas_as_tabelas()?;
        // O `fsync` de cada tabela reaplicada, guardado para depois da trava.
        let mut a_sincronizar: Vec<Table> = Vec::new();

        let mut por_tabela = Vec::new();
        let mut sem_diario_vivo = Vec::new();
        let mut total = 0u64;

        for nome in &restauradas {
            if !vivas.contains(nome) {
                sem_diario_vivo.push(Json::texto_de(nome));
                continue;
            }
            let mut td = db_destino.abrir_qualificada(nome)?;
            let mut tv = db_vivo.abrir_qualificada(nome)?;

            let posicao = td.eventos()?;
            let vivos = tv.eventos()?;
            let mut reaplicados = 0u64;
            let mut pulados = 0u64;
            let mut ultimo: Option<i64> = None;
            let mut parou: Option<String> = None;

            // Pedido 706: a copia e mais velha que o que o diario vivo ainda
            // guarda -- o expurgo levou o trecho entre ela e o agora (e o
            // evento `posicao - 1`, com que a continuidade se confere). A
            // recusa diz «expurgado», pela fabrica, e nao «a copia diverge».
            let base_viva = tv.base_do_diario()?;
            let continua = if base_viva > 0 && posicao <= base_viva {
                Err(self.msg(
                    "erro.pitr_diario_expurgado",
                    &[
                        ("tabela", nome),
                        ("posicao", &posicao.to_string()),
                        ("base", &base_viva.to_string()),
                    ],
                ))
            } else {
                Self::diario_vivo_continua(&mut td, &mut tv, posicao, vivos)
            };
            match continua {
                Ok(()) => {
                    let mut pos = posicao;
                    'tabela: while pos < vivos {
                        let lote = tv.diario_com_imagem(pos, LOTE)?;
                        if lote.is_empty() {
                            break;
                        }
                        for (e, imagem) in &lote {
                            pos += 1;
                            // O corte de cima, evento a evento. Ver o cabecalho.
                            if e.carimbo > ate_ms {
                                pulados += 1;
                                continue;
                            }
                            // O evento reaplicado guarda o carimbo e a origem
                            // do ORIGINAL, e nao a hora da restauracao: o
                            // diario e trilha de auditoria, e um restaurado
                            // que jurasse que tudo aconteceu agora destruiria
                            // justamente o que se foi buscar nele.
                            td.forcar_proximo_evento(e.carimbo, e.origem);
                            // O irmao do `replicar` (pedido 344): a imagem do
                            // diario vivo leva o externo marcado selado com a
                            // chave do `.reg` VIVO, e quem tem essa chave e o
                            // `tv`. Abrir por ele deixa o restaurado selar com
                            // a dele, em vez de depender de os dois arquivos
                            // ainda dividirem o sal.
                            let aplicado = tv.imagem_para_o_fio(imagem).and_then(|i| {
                                // O diario e DESTE servidor: a recusa da
                                // replica sem cofre (pedido 613) nao cabe.
                                td.reaplicar_evento_do_proprio_diario(e.operacao, e.rowid, &i)
                            });
                            if let Err(erro) = aplicado {
                                parou = Some(format!(
                                    "no evento {pos} do diario ({}): {erro}",
                                    phxsql_core::datahora::instante_iso(e.carimbo)
                                ));
                                break 'tabela;
                            }
                            reaplicados += 1;
                            ultimo = Some(e.carimbo);
                        }
                    }
                }
                Err(motivo) => parou = Some(motivo),
            }

            if reaplicados > 0 {
                a_sincronizar.push(td);
            }
            total += reaplicados;
            let mut campos = vec![
                ("tabela", Json::texto_de(nome)),
                ("reaplicados", Json::de_u64(reaplicados)),
                ("pulados", Json::de_u64(pulados)),
                (
                    "ultimo_carimbo_ms",
                    match ultimo {
                        Some(c) => Json::Numero(c as f64),
                        None => Json::Nulo,
                    },
                ),
            ];
            if let Some(c) = ultimo {
                campos.push((
                    "ultimo",
                    Json::texto_de(phxsql_core::datahora::instante_iso(c)),
                ));
            }
            if let Some(motivo) = parou {
                campos.push(("parou_em", Json::texto_de(motivo)));
            }
            por_tabela.push(Json::objeto(campos));
        }

        // A TRAVA SAI AQUI, e sai ANTES do `fsync`. Dali para baixo so se
        // escreve no palco, que nao esta na raiz de dados e nao tem segundo
        // dono possivel -- entao a ficha nao protege nada ali, e segura-la
        // custaria ao servidor inteiro uma dezena de `sync_all` por tabela.
        //
        // Quem mexer aqui: o `drop` tem de continuar ACIMA do `sincronizar`.
        // A catraca `alcancam-fsync` do `bancada/concorrencia/mapa-da-trava.py`
        // conta esta secao pelo que ela alcanca com a trava na mao, e o teste
        // `o_fsync_da_restauracao_fica_fora_da_trava` reprova a volta.
        drop(trava);
        for mut td in a_sincronizar {
            td.sincronizar()?;
        }

        let novas: Vec<Json> = vivas
            .iter()
            .filter(|n| !restauradas.contains(n))
            .map(Json::texto_de)
            .collect();

        Ok(Json::objeto(vec![
            ("de", Json::texto_de(de)),
            ("copia_ms", Json::Numero(copia_ms as f64)),
            (
                "copia",
                Json::texto_de(phxsql_core::datahora::instante_iso(copia_ms)),
            ),
            ("ate_ms", Json::Numero(ate_ms as f64)),
            (
                "ate",
                Json::texto_de(phxsql_core::datahora::instante_iso(ate_ms)),
            ),
            ("reaplicados", Json::de_u64(total)),
            ("tabelas", Json::Lista(por_tabela)),
            ("sem_diario_vivo", Json::Lista(sem_diario_vivo)),
            ("novas_na_origem", Json::Lista(novas)),
        ]))
    }

    /// O diario vivo CONTINUA o da copia, ou ja e outro diario?
    ///
    /// # O que ela protege, e por que a conta e esta
    ///
    /// A reaplicacao comeca na posicao `posicao` do diario vivo porque essa e
    /// a posicao que a copia tinha. Isso so vale se o diario vivo for o MESMO
    /// arquivo, crescido -- e o `.log` e append-only e nunca gira (o
    /// `rodizio.rs` desta casa e dos logs de TEXTO: `perfil.txt`,
    /// `diretivas.log`, `acessos.log`; o unico lugar que apaga um `.log` de
    /// tabela e o `excluir_tabela`, que leva a tabela junto).
    ///
    /// Sobra um caso, e ele e real: a tabela foi **apagada e recriada** depois
    /// do backup. Ai o diario vivo comeca do zero, a posicao da copia aponta
    /// para o meio de uma historia que nao e a mesma, e reaplicar dali gravaria
    /// linhas de outra vida.
    ///
    /// # E UM guarda, com duas explicacoes -- e isso foi medido
    ///
    /// O guarda e a comparacao: o ultimo evento da copia tem de ser IGUAL ao
    /// evento daquela posicao no diario vivo. A conta de tamanho que vem antes
    /// **nao pega nenhum caso que a comparacao deixaria passar** -- diario mais
    /// curto que a posicao devolve lista vazia, e lista vazia ja cai na recusa.
    /// Provado sabotando: com a conta desligada, os dois testes de
    /// continuidade continuam vermelhos pela comparacao.
    ///
    /// Ela fica porque a MENSAGEM e outra: «tem 1 evento e a copia tinha 2»
    /// diz a um operador o que aconteceu, e «o evento 1 nao e o mesmo» nao diz.
    /// Guarda que nao guarda mas explica melhor e mensagem, e o teste que a
    /// prova e o que confere o TEXTO -- nao o que confere a recusa.
    ///
    /// Com a copia de diario vazio nao ha o que comparar, e a funcao diz que
    /// sim: nao havia historia para continuar. Quem cobre esse caso e o rowid
    /// do `aplicar_evento`, que e a mesma guarda de sempre.
    fn diario_vivo_continua(
        td: &mut Table,
        tv: &mut Table,
        posicao: u64,
        vivos: u64,
    ) -> std::result::Result<(), String> {
        if vivos < posicao {
            return Err(format!(
                "o diario vivo tem {vivos} evento(s) e a copia tinha {posicao}: \
                 ele nao continua o da copia -- a tabela foi apagada e recriada \
                 depois do backup. Esta tabela ficou no instante da copia"
            ));
        }
        if posicao == 0 {
            return Ok(());
        }
        let da_copia = td.diario(posicao - 1, 1).map_err(|e| e.to_string())?;
        let do_vivo = tv.diario(posicao - 1, 1).map_err(|e| e.to_string())?;
        match (da_copia.first(), do_vivo.first()) {
            (Some(a), Some(b))
                if a.carimbo == b.carimbo
                    && a.operacao == b.operacao
                    && a.rowid == b.rowid
                    && a.versao == b.versao => {}
            _ => {
                return Err(format!(
                    "o evento {} do diario vivo nao e o mesmo que a copia tinha ali: \
                     o diario vivo nao continua o da copia. Esta tabela ficou no \
                     instante da copia",
                    posicao - 1
                ))
            }
        }
        Ok(())
    }

    /// A impressao digital de uma tabela, para comparar duas copias.
    ///
    /// # Para que serve
    ///
    /// Responder "estas duas tabelas sao a mesma?" sem transportar as duas.
    /// E o que falta para conferir uma replica contra a origem, e para provar
    /// que um backup restaurado ficou igual ao original -- hoje o
    /// `conferir-backup` compara ARQUIVO, e arquivo igual e mais forte do que
    /// preciso: dois `.reg` podem diferir no enchimento e ter o mesmo dado.
    ///
    /// # Como a conta e feita
    ///
    /// CRC-32 de cada linha viva, dobrado num acumulador que **depende da
    /// ordem**. Depender da ordem e de proposito: no PhxSql a ordem de
    /// digitacao E o dado, e duas tabelas com as mesmas linhas em ordem
    /// diferente nao sao a mesma tabela.
    ///
    /// Slot excluido nao entra. Se entrasse, restaurar um backup daria outro
    /// numero so porque os buracos caem em outro lugar.
    pub(super) fn op_checksum(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let comeco = Instant::now();
        let dados = self.travar_dados()?;
        let mut t = self.abrir_travada(&dados, p, sessao)?;
        let esquema = t.esquema().clone();

        let mut soma: u64 = 0xcbf2_9ce4_8422_2325; // semente do FNV-1a de 64
        let mut linhas = 0u64;
        // PONTO DE CANCELAMENTO. A soma de verificacao le a tabela inteira
        // segurando a trava de dados: e a operacao que mais para o servidor
        // sem gravar nada. Abandonar entre duas linhas nao deixa rastro
        // nenhum -- nada foi escrito --, e por isso ela e cancelavel do
        // comeco ao fim.
        let atividade = crate::telemetria::corrente();
        let _fase = atividade
            .as_ref()
            .map(|a| a.fase_cancelavel("somando a tabela"));
        for (rowid, _) in t.varrer()? {
            if let Some(a) = &atividade {
                a.siga(1)?;
            }
            let Some(linha) = t.ler(rowid)? else { continue };
            // A linha volta a forma canonica antes de entrar na conta: somar o
            // byte cru do slot faria o enchimento de um `Str` de largura fixa
            // pesar, e duas tabelas iguais com larguras diferentes dariam
            // numeros diferentes.
            let mut texto = String::with_capacity(64);
            for (v, c) in linha.iter().zip(esquema.colunas()) {
                texto.push('\u{1}');
                if v.e_null() {
                    texto.push('\u{0}');
                } else {
                    texto.push_str(&crate::valores::valor_para_json(v, &c.ty).escrever());
                }
            }
            let crc = phxsql_core::crc::crc32(texto.as_bytes()) as u64;
            // Multiplicar antes de somar e o que faz a ordem contar: trocar
            // duas linhas de lugar muda o resultado.
            soma = (soma ^ crc).wrapping_mul(0x1000_0000_01b3);
            linhas += 1;
        }

        Ok(Json::objeto(vec![
            ("database", Json::texto_de(p.texto_ou("database", ""))),
            ("tabela", Json::texto_de(p.texto_ou("tabela", ""))),
            ("checksum", Json::texto_de(format!("{soma:016x}"))),
            ("linhas", Json::de_u64(linhas)),
            ("slots", Json::de_u64(t.registros())),
            ("ms", Json::de_u64(comeco.elapsed().as_millis() as u64)),
        ]))
    }
}

/// O nome da lapide do backup agendado, dentro do destino -- pedido 502.
/// Comeca com ponto, e nao tem a cara de zip nenhum: a faxina do `manter` so
/// apaga o que tem.
pub(super) const LAPIDE_DO_BACKUP: &str = ".phxsql-backup-agendado.em-curso";

/// O erro da corrida (de job ou de backup) cuja filha morreu em panico --
/// pedido 502. E um erro como outro qualquer para quem o anota: vai ao
/// historico, ao e-mail e ao `acessos.log`.
pub(super) fn corrida_em_panico(panico: &str) -> PhxError {
    PhxError::Corrompido(format!(
        "a corrida entrou em PANICO: {panico}. Ela foi interrompida e o servidor \
         segue de pe -- se ela segurava a trava de dados, a trava foi reparada \
         (pedido 502)"
    ))
}
