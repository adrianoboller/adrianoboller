//! O servico e os jobs.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    pub(super) fn op_servico(&self) -> Result<Json> {
        let corrente = self.endereco_dos_dados.lock().ok().and_then(|e| *e);
        let configurado = self.config.bind.clone();
        Ok(Json::objeto(vec![
            ("no_ar", Json::Bool(self.porta_no_ar.load(Ordering::SeqCst))),
            (
                "endereco",
                match corrente {
                    Some(e) => Json::texto_de(e.to_string()),
                    None => Json::Nulo,
                },
            ),
            // Os dois lado a lado de proposito. Uma troca pela tela vale ate o
            // proximo arranque -- ela NAO reescreve o config.json, que carrega
            // comentario e o resto da configuracao. Sem mostrar os dois, quem
            // reiniciar a maquina meses depois nao entende por que a porta
            // voltou a ser outra.
            ("bind_configurado", Json::texto_de(&configurado)),
            (
                "difere_do_arquivo",
                Json::Bool(match (&corrente, self.config.endereco()) {
                    (Some(c), Ok(cfg)) => *c != cfg,
                    _ => false,
                }),
            ),
            (
                "conexoes",
                Json::de_u64(self.permissoes_de_dados.em_uso() as u64),
            ),
            ("web", Json::texto_de(&self.config.web.bind)),
            ("web_ligada", Json::Bool(self.config.web.ligado)),
        ]))
    }

    /// Para de aceitar conexao nova na porta de dados.
    ///
    /// # Quem fica sem resposta
    ///
    /// Ninguem que ja esta conectado: as conexoes vivas continuam ate elas
    /// mesmas acabarem. O que para e o `accept`. Cliente NOVO recebe recusa de
    /// conexao do sistema operacional -- o mesmo que receberia com o servico
    /// desligado --, e a resposta diz quantas conexoes ficaram abertas para
    /// quem clicou saber o que esta interrompendo.
    ///
    /// # Como se volta
    ///
    /// Pela mesma tela: o processo continua vivo, a interface web continua no
    /// ar na porta dela, e `servico_subir` religa. E por isso que este botao
    /// **nao** derruba o processo: um botao que se desfaz e um botao; um que
    /// nao se desfaz e um alcapao.
    pub(super) fn op_servico_parar(&self) -> Result<Json> {
        if !self.porta_no_ar.load(Ordering::SeqCst) {
            return Err(PhxError::Esquema("a porta de dados ja esta parada".into()));
        }
        let abertas = self.permissoes_de_dados.em_uso();
        self.parar_de_aceitar.store(true, Ordering::SeqCst);
        self.acordar_o_accept()?;
        Ok(Json::objeto(vec![
            ("parando", Json::Bool(true)),
            ("conexoes_abertas", Json::de_u64(abertas as u64)),
            (
                "aviso",
                Json::texto_de(
                    "as conexoes ja abertas seguem ate acabarem; o que parou foi aceitar \
                     conexao nova. A interface web continua no ar, e e por ela que a porta \
                     volta",
                ),
            ),
        ]))
    }

    /// Sobe a porta de dados, no mesmo endereco ou em outro.
    ///
    /// # A ordem que evita o tiro no pe
    ///
    /// O endereco novo e PRESO primeiro. So depois de o `bind` dar certo e que
    /// o laco antigo e mandado parar e solta o endereco velho. Porta ocupada,
    /// permissao negada (porta abaixo de 1024 sem raiz) ou endereco escrito
    /// errado falham AQUI, com o servico intacto e nada trocado -- em vez de
    /// deixar a maquina sem porta de dados nenhuma e sem jeito de voltar.
    pub(super) fn op_servico_subir(&self, p: &Json) -> Result<Json> {
        let pedido = p.texto_ou("bind", "").trim().to_string();
        let alvo = if pedido.is_empty() {
            // Sem endereco: volta para onde estava, ou para o do arquivo.
            match self.endereco_dos_dados.lock().ok().and_then(|e| *e) {
                Some(e) => e,
                None => self.config.endereco()?,
            }
        } else {
            crate::config::endereco_de(&pedido)?
        };

        let no_ar = self.porta_no_ar.load(Ordering::SeqCst);
        let atual = self.endereco_dos_dados.lock().ok().and_then(|e| *e);
        if no_ar && atual == Some(alvo) {
            return Err(PhxError::Esquema(format!(
                "a porta de dados ja esta no ar em {alvo}"
            )));
        }

        let novo = TcpListener::bind(alvo).map_err(|e| {
            PhxError::Esquema(format!(
                "nao consegui escutar em {alvo}: {e}. Nada mudou -- o servico continua \
                 como estava"
            ))
        })?;
        {
            let mut prox = self.proximo_ouvinte.tomar("proximo_ouvinte")?;
            *prox = Some(novo);
        }
        if no_ar {
            self.parar_de_aceitar.store(true, Ordering::SeqCst);
            self.acordar_o_accept()?;
        }
        Ok(Json::objeto(vec![
            ("subindo_em", Json::texto_de(alvo.to_string())),
            ("trocou_de_porta", Json::Bool(atual != Some(alvo))),
            (
                "aviso",
                Json::texto_de(
                    "vale ate o proximo arranque: o config.json nao foi reescrito. Para \
                     valer sempre, mude o campo bind no arquivo",
                ),
            ),
        ]))
    }

    /// Acorda o `accept` bloqueado conectando no proprio endereco.
    ///
    /// # O endereco para onde conectar nao e sempre o do `bind`
    ///
    /// Um servidor preso em `0.0.0.0:5000` escuta em toda placa, e conectar
    /// literalmente em `0.0.0.0` so funciona por acidente do sistema. Com
    /// endereco nao especificado, o despertador vai pelo `localhost` na mesma
    /// porta, que e o caminho que sempre existe.
    fn acordar_o_accept(&self) -> Result<()> {
        let Some(onde) = self.endereco_dos_dados.lock().ok().and_then(|e| *e) else {
            return Err(PhxError::Esquema(
                "nao sei em que endereco a porta esta escutando".into(),
            ));
        };
        let destino = if onde.ip().is_unspecified() {
            match onde {
                SocketAddr::V4(_) => SocketAddr::from(([127, 0, 0, 1], onde.port())),
                SocketAddr::V6(_) => SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, onde.port())),
            }
        } else {
            onde
        };
        match TcpStream::connect_timeout(&destino, Duration::from_secs(3)) {
            // O soquete morre aqui mesmo: o laco sai antes de atender, e do
            // outro lado isto e so o toque que fez o `accept` devolver.
            Ok(_) => Ok(()),
            Err(e) => {
                // O sinalizador volta: deixa-lo levantado faria a PROXIMA
                // conexao de verdade derrubar a porta, minutos depois, sem
                // ninguem ter pedido.
                self.parar_de_aceitar.store(false, Ordering::SeqCst);
                if let Ok(mut prox) = self.proximo_ouvinte.lock() {
                    *prox = None;
                }
                Err(PhxError::Esquema(format!(
                    "nao consegui acordar o laco de aceitacao em {destino}: {e}. \
                     Nada mudou"
                )))
            }
        }
    }

    // ------------------------------------------------------- jobs de execucao

    /// O cadastro, o estado completo de cada um e o historico das corridas.
    pub(super) fn op_jobs(&self, p: &Json) -> Result<Json> {
        let quantas = p.inteiro_ou("historico", 50).clamp(0, 500) as usize;
        let relogio = self.relogio_de_jobs_no_ar();
        // A foto dos que rodam agora sai ANTES da trava do cadastro -- a
        // ordem das duas travas e sempre esta, para nunca haver abraco.
        let rodando_agora: Vec<String> = self
            .jobs_rodando
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let r = self.jobs.tomar("jobs")?;
        // A lista vazia de um cadastro trancado seria mentira.
        r.exigir_legivel()?;
        let agora = crate::agora_ms();
        let lista: Vec<Json> = r
            .jobs
            .iter()
            .map(|j| {
                let ultimo = r.ultimo_de(&j.nome);
                let rodando = rodando_agora
                    .iter()
                    .any(|n| n.eq_ignore_ascii_case(&j.nome));
                let ultima = r.ultima_corrida_de(&j.nome);
                let mut pares = match j.ficha() {
                    Json::Objeto(x) => x,
                    _ => Vec::new(),
                };
                pares.push((
                    "ultimo_ms".to_string(),
                    if ultimo == 0 {
                        Json::Nulo
                    } else {
                        Json::de_i64(ultimo)
                    },
                ));
                // "Venceria agora?" e o que a tela precisa para dizer se um
                // job esta atrasado -- e sai da MESMA funcao que o relogio
                // usa, para os dois nunca discordarem.
                let vencido = j.ligado && j.agenda.hora_de_rodar(agora, ultimo);
                pares.push(("vencido".to_string(), Json::Bool(vencido)));
                pares.push(("rodando".to_string(), Json::Bool(rodando)));
                pares.push((
                    "estado".to_string(),
                    Json::texto_de(crate::jobs::estado_do_job(
                        j.ligado,
                        rodando,
                        ultima.map(|c| c.ok),
                        relogio,
                    )),
                ));
                // Parado e o vencido que ninguem vai rodar -- mesma funcao
                // que o vigia de e-mail usa.
                pares.push((
                    "parado".to_string(),
                    Json::Bool(crate::jobs::job_parado(j.ligado, rodando, vencido, relogio)),
                ));
                // A ultima corrida com resultado -- a semeada do log conta,
                // para "falhou as 03:00" sobreviver a um reinicio.
                pares.push((
                    "ultima".to_string(),
                    match ultima {
                        Some(c) => c.para_json(),
                        None => Json::Nulo,
                    },
                ));
                // A proxima prevista, pela mesma conta do relogio. Pode estar
                // no passado: vencida e informacao, nao erro.
                if j.ligado {
                    let proximo = j.agenda.proximo_ms(agora, ultimo);
                    pares.push(("proximo_ms".to_string(), Json::de_i64(proximo)));
                    pares.push((
                        "proxima".to_string(),
                        Json::texto_de(phxsql_core::datahora::instante_iso(proximo)),
                    ));
                } else {
                    pares.push(("proximo_ms".to_string(), Json::Nulo));
                }
                Json::Objeto(pares)
            })
            .collect();
        let historico: Vec<Json> = r
            .historico(quantas)
            .iter()
            .map(crate::jobs::Corrida::para_json)
            .collect();
        let email = &self.config.alertas.email;
        Ok(Json::objeto(vec![
            ("arquivo", Json::texto_de(r.caminho.display().to_string())),
            (
                "log",
                Json::texto_de(r.caminho_do_log().display().to_string()),
            ),
            ("relogio_no_ar", Json::Bool(relogio)),
            // O estado do aviso por e-mail, para a tela dizer a verdade sobre
            // quem sera avisado -- os enderecos ja aparecem na op `config`,
            // que exige o mesmo `administrar` que esta aqui.
            (
                "aviso_email",
                Json::objeto(vec![
                    // A MESMA funcao que decide se o e-mail sai -- escrita
                    // duas vezes, a copia esquecida viraria uma tela que
                    // mente sobre o aviso.
                    ("ligado", Json::Bool(self.aviso_de_jobs_ligado())),
                    ("email_ligado", Json::Bool(email.ligado)),
                    ("avisar_jobs", Json::Bool(email.avisar_jobs)),
                    (
                        "para",
                        Json::Lista(email.para.iter().map(Json::texto_de).collect()),
                    ),
                    (
                        "repetir_horas",
                        Json::de_u64(self.config.alertas.repetir_horas),
                    ),
                ]),
            ),
            ("jobs", Json::Lista(lista)),
            ("historico", Json::Lista(historico)),
        ]))
    }

    /// Liga ou desliga um job pelo nome, sem tocar no resto da ficha.
    ///
    /// Existe para a tela nao reenviar o job inteiro so para virar uma chave:
    /// reenviar a ficha lida ha minutos gravaria por cima do que outro
    /// administrador mudou nesse meio tempo -- o mesmo estrago da gravacao
    /// sem versao, por outra porta.
    pub(super) fn op_job_ligar(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        let Some(ligado) = p.campo("ligado").and_then(Json::booleano) else {
            return Err(PhxError::Esquema(
                "informe \"ligado\": true liga, false desliga".into(),
            ));
        };
        let mut r = self.jobs.tomar("jobs")?;
        let mut job = r.achar(&nome)?.clone();
        let nome = job.nome.clone();
        job.ligado = ligado;
        r.salvar(job)?;
        Ok(Json::objeto(vec![
            ("job", Json::texto_de(nome)),
            ("ligado", Json::Bool(ligado)),
            // Mesmo aviso do salvar: ligar so vale sozinho se ha relogio.
            ("relogio_no_ar", Json::Bool(self.relogio_de_jobs_no_ar())),
        ]))
    }

    pub(super) fn op_job_salvar(&self, p: &Json) -> Result<Json> {
        let job = crate::jobs::Job::de_json(p.campo("job").unwrap_or(p))?;
        // Conferido na hora de salvar, e nao so na hora de rodar: descobrir
        // que o login nao existe as tres da manha, no historico, e pior do que
        // descobrir agora, com a tela aberta.
        self.sessao_do_job(&job)?;
        let mut r = self.jobs.tomar("jobs")?;
        let nome = job.nome.clone();
        r.salvar(job)?;
        Ok(Json::objeto(vec![
            ("salvo", Json::texto_de(nome)),
            // O relogio le o cadastro a cada volta, entao ligar um job vale na
            // proxima. Mas se NENHUM estava ligado quando o servidor subiu,
            // nao ha relogio -- e a tela precisa dizer isso, senao o job fica
            // ligado e parado sem ninguem entender por que.
            ("relogio_no_ar", Json::Bool(self.relogio_de_jobs_no_ar())),
        ]))
    }

    pub(super) fn op_job_excluir(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("nome", "").trim().to_string();
        let mut r = self.jobs.tomar("jobs")?;
        r.excluir(&nome)?;
        Ok(Json::objeto(vec![("excluido", Json::texto_de(nome))]))
    }

    /// Roda um job agora, fora da agenda.
    ///
    /// O `rodar_job` faz a conferencia de permissao com o usuario DO JOB, e
    /// nao com quem clicou -- senao rodar agora seria um jeito de emprestar o
    /// proprio poder para o job. Quem clica precisa de `administrar`, que e o
    /// que o portao ja exigiu para chegar ate aqui.
    ///
    /// # Job nao dispara job (pedido 530)
    ///
    /// Recusa quando quem pede e a propria corrida de um job (a filha de
    /// familia `corrida`, pedido 502). Cada `job_rodar` de dentro de uma
    /// corrida sobe OUTRA filha e espera por ela: um job que rode a si mesmo,
    /// ou um ciclo A->B->A, empilha uma thread por nivel sem teto nenhum --
    /// medido pelo papel C, 45 corridas aninhadas vivas ate o teto de
    /// enderecamento de 1,5 GB que a propria prova impos. Nenhum dos maduros
    /// deixa um evento ou job disparar outro de forma sincrona, e o
    /// encadeamento que sobra -- «B depois de A» -- e agenda, nao chamada. A
    /// pergunta e pela familia da thread, e nao pelo pedido: assim vale para
    /// o `job_rodar` que chega aninhado em qualquer outra operacao.
    pub(super) fn op_job_rodar(&self, p: &Json) -> Result<Json> {
        if crate::telemetria::familia_desta_thread() == Some("corrida") {
            return Err(PhxError::LimiteExcedido(
                "job nao dispara job: este job_rodar veio de dentro da corrida de \
                 outro job, e cada nivel subiria uma thread a mais sem teto \
                 (pedido 530). Para rodar um job depois do outro, agende os dois"
                    .into(),
            ));
        }
        let nome = p.texto_ou("nome", "").trim().to_string();
        let inicio = crate::agora_ms();
        let r = self.rodar_job(&nome, "tela");
        if let Ok(mut reg) = self.jobs.lock() {
            reg.anotar_corrida(&nome, inicio);
        }
        Ok(Json::objeto(vec![
            ("job", Json::texto_de(nome)),
            ("ok", Json::Bool(r.is_ok())),
            ("duracao_ms", Json::de_i64(crate::agora_ms() - inicio)),
            (
                "detalhe",
                Json::texto_de(match &r {
                    Ok(j) => resumir_resposta(j),
                    Err(e) => e.to_string(),
                }),
            ),
            ("resposta", r.unwrap_or(Json::Nulo)),
        ]))
    }

    /// Ha relogio de jobs rodando neste processo?
    ///
    /// Ele so sobe se algum job estava ligado no arranque -- entao ligar o
    /// primeiro job pela tela nao acorda ninguem ate o proximo arranque. Dizer
    /// isso e melhor do que subir uma linha de execucao que fica acordando de
    /// trinta em trinta segundos num servidor que nao tem job nenhum.
    pub(super) fn relogio_de_jobs_no_ar(&self) -> bool {
        self.relogio_de_jobs.load(Ordering::SeqCst)
    }

    /// Sobe o relogio dos jobs, se houver algum ligado.
    ///
    /// Um relogio so para todos, e nao um por job: o trabalho de perguntar
    /// "chegou a hora?" e uma comparacao de inteiros, e uma linha de execucao
    /// por job custaria pilha para ficar dormindo.
    pub(super) fn subir_jobs(self: &Arc<Self>) {
        // ANTES do portao do relogio: a corrida que caiu pode ser de um job
        // que ninguem mais liga, e a falha dela avisa do mesmo jeito.
        self.avisar_corridas_interrompidas();
        let ligados: Vec<String> = match self.jobs.lock() {
            Ok(r) => r
                .jobs
                .iter()
                .filter(|j| j.ligado)
                .map(|j| format!("{} ({}, {})", j.nome, j.op(), j.agenda.rotulo()))
                .collect(),
            Err(_) => return,
        };
        if ligados.is_empty() {
            // Sem job ligado nao ha relogio: instrumentacao desligada custa
            // zero, e o portao que decide isso vem ANTES do trabalho.
            return;
        }
        eprintln!("jobs de execucao: {}", ligados.join(" | "));
        self.relogio_de_jobs.store(true, Ordering::SeqCst);
        // O «no ar» sai JUNTO com a thread, pelo `Drop` -- o irmao do pedido
        // 452. Nasce aqui, e nao dentro da thread, para sair tambem quando a
        // thread nem chega a nascer.
        let no_ar = RelogioNoAr(Arc::clone(self));
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "relogio-jobs",
            "acorda de tempos em tempos, ve quais jobs venceram a hora e os \
             executa, um por um, com o poder do usuario de cada um",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                // A guarda viaja capturada pelo `move`, e o `Drop` dela roda
                // quando este corpo sai -- inclusive no desenrolar.
                let _no_ar = &no_ar;
                #[cfg(test)]
                if servidor
                    .panico_no_relogio_de_jobs_de_teste
                    .swap(false, Ordering::SeqCst)
                {
                    panic!("panico de teste no relogio de jobs (irmao do pedido 452)");
                }
                let agora = crate::agora_ms();
                // A trava sai antes de executar: um job de backup segura a
                // trava dos dados por segundos, e prender o cadastro junto
                // travaria a tela de jobs e todos os outros jobs enquanto isso.
                let vencidos = match servidor.jobs.lock() {
                    Ok(mut r) => {
                        let v = r.vencidos(agora);
                        for nome in &v {
                            r.anotar_corrida(nome, agora);
                        }
                        v
                    }
                    Err(_) => Vec::new(),
                };
                if vencidos.is_empty() {
                    fio.fazendo("nenhum job vencido");
                }
                for nome in vencidos {
                    fio.fazendo(&format!("rodando o job {nome}"));
                    let _ = servidor.rodar_job(&nome, "agenda");
                }
                std::thread::sleep(Duration::from_secs(crate::jobs::PERIODO_DO_RELOGIO_S));
            },
        );
    }

    /// A corrida fechada no arranque (pedido 502) avisa como a falha comum --
    /// C2 do parecer do DBA. Ela era FALHOU so no historico e no erro padrao,
    /// e isso vale para qualquer queda no meio de um job (`systemctl
    /// restart`, OOM, `SIGKILL`), nao so para a H5: o job fica sem rodar ate
    /// a proxima hora dele, e o dono tem de saber. O irmao, o backup, ja avisa
    /// pelo carteiro.
    ///
    /// Job que saiu do cadastro nao tem ficha para o aviso, e so fica no
    /// historico.
    fn avisar_corridas_interrompidas(&self) {
        let fechadas: Vec<crate::jobs::Corrida> = match self.interrompidas_a_avisar.lock() {
            Ok(mut v) => v.drain(..).collect(),
            Err(_) => return,
        };
        for c in fechadas {
            let job = match self.jobs.lock() {
                Ok(r) => r.achar(&c.job).ok().cloned(),
                Err(_) => None,
            };
            if let Some(job) = job {
                self.avisar_sobre_a_corrida(&job, &c);
            }
        }
    }

    /// Roda um job agora: monta a sessao dele, passa pelos portoes e executa.
    ///
    /// # Por que ele nao roda "como o servidor"
    ///
    /// Porque um agendador com poder proprio e um jeito de contornar a
    /// permissao: bastaria escrever no cadastro de jobs a operacao que a rede
    /// recusaria. O job carrega o login de um usuario do cadastro e roda com o
    /// poder DAQUELE usuario -- e usuario que sumiu ou foi desativado para o
    /// job, com erro escrito, em vez de cair para uma sessao sem dono, que e
    /// o que o `Default` daria.
    ///
    /// # A politica e conferida aqui, e nao no portao comum
    ///
    /// `portoes_do_pedido` deixou de fora o que so faz sentido com um IP do
    /// outro lado. Comando proibido pela politica vale igual para o job -- o
    /// `config.json` diz que ninguem pede aquilo neste servidor --, mas nao ha
    /// IP para bloquear: a recusa vira linha no historico.
    fn rodar_job(&self, nome: &str, disparado_por: &str) -> Result<Json> {
        let inicio = crate::agora_ms();
        // A copia sai de dentro da trava para o job poder rodar por segundos
        // sem prender o cadastro -- e a tela de jobs continua respondendo.
        let job = self.jobs.tomar("jobs")?.achar(nome)?.clone();
        let op = job.op().to_string();

        // A lapide ANTES de executar (pedido 502): se esta corrida derrubar o
        // processo, o arranque seguinte a acha aberta e nao a roda de novo.
        if let Ok(mut r) = self.jobs.lock() {
            r.registrar_inicio(&job.nome, &op, &job.usuario, inicio);
        }
        // O nome entra na lista dos que rodam agora ANTES de executar e sai
        // logo depois: e o que deixa a tela dizer "rodando" e impede o vigia
        // de tratar um backup de dez minutos como job parado. A corrida roda
        // numa filha, e por isso o `false` chega mesmo quando ela entra em
        // panico -- antes, o panico pulava esta linha e o job ficava
        // «rodando» para sempre.
        self.marcar_rodando(&job.nome, true);
        let resultado = self.executar_job_na_filha(&job, &op, inicio);
        self.marcar_rodando(&job.nome, false);
        let corrida = crate::jobs::Corrida {
            quando_ms: inicio,
            job: job.nome.clone(),
            op: op.clone(),
            usuario: job.usuario.clone(),
            ok: resultado.is_ok(),
            duracao_ms: crate::agora_ms() - inicio,
            detalhe: match &resultado {
                // A resposta inteira nao entra: uma varredura de vinte mil
                // linhas nao cabe no historico e nao interessa a ele. O que
                // interessa e ter rodado, e o que voltou de resumo.
                Ok(j) => resumir_resposta(j),
                Err(e) => e.to_string(),
            },
            em_curso: false,
        };
        if let Ok(mut r) = self.jobs.lock() {
            r.registrar(&corrida);
        }
        // O aviso por e-mail, se foi pedido -- e a limpeza do silencio quando
        // o job volta a rodar. Depois do registrar: o historico nunca pode
        // depender de o rele estar no ar.
        self.avisar_sobre_a_corrida(&job, &corrida);
        // O job tambem entra no `acessos.log`, como qualquer outra operacao:
        // quem audita o servidor nao deveria precisar saber que existe um
        // segundo arquivo para descobrir que uma tabela foi mexida.
        self.anotar(&Acesso {
            quando_ms: inicio,
            ip: format!("(job:{disparado_por})"),
            porta_origem: 0,
            op: format!("job:{op}"),
            usuario: job.usuario.clone(),
            autenticado: !job.usuario.is_empty(),
            ok: resultado.is_ok(),
            duracao_ms: corrida.duracao_ms.max(0) as u64,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            database: job.pedido.texto_ou("database", "").to_string(),
            tabela: job.pedido.texto_ou("tabela", "").to_string(),
            codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        });
        if let Err(e) = &resultado {
            eprintln!("job {} FALHOU: {e}", job.nome);
        }
        resultado
    }

    /// A corrida de um job numa thread FILHA -- pedido 502. O panico dela volta
    /// como corrida que FALHOU, e o relogio (ou a conexao da tela) segue vivo.
    /// Ver `Telemetria::rodar_em_filha`.
    fn executar_job_na_filha(&self, job: &crate::jobs::Job, op: &str, inicio: i64) -> Result<Json> {
        let corrida = self.telemetria.rodar_em_filha(
            format!("job-{}", job.nome),
            "executa UMA corrida de job e morre: o panico da corrida volta ao \
             relogio pelo `join` e vira corrida que falhou, em vez de derrubar \
             o processo e rodar de novo no arranque (pedido 502)",
            "corrida",
            inicio,
            |fio| {
                fio.fazendo(&format!("rodando o job {}", job.nome));
                self.executar_job(job, op)
            },
        );
        corrida.unwrap_or_else(|panico| Err(corrida_em_panico(&panico)))
    }

    fn executar_job(&self, job: &crate::jobs::Job, op: &str) -> Result<Json> {
        // O job que voltou do `jobs.json` com credencial no pedido sobe com o
        // servidor e para AQUI -- a porta por onde passam a agenda e a tela
        // (pedido 497, R1 do parecer SEC). O porque de ser recusa ao rodar, e
        // nao desligar, esta em `Job::do_disco`.
        if let Some(e) = job.recusa_de_credencial() {
            return Err(e);
        }
        // So nos testes: o panico pedido para a operacao DESTE job, na thread
        // que o executa (pedido 502).
        #[cfg(test)]
        self.armar_panico_de_teste(op);
        // A politica antes de saber sob qual usuario o job roda: um comando
        // proibido e proibido para todo mundo, e recusar por ele da a mensagem
        // certa a um job cujo dono tambem esta errado.
        self.politica_do_pedido(op, &job.pedido)?;
        let sessao = self.sessao_do_job(job)?;
        self.portoes_do_pedido(op, &job.pedido, &sessao)?;
        // O TERCEIRO irmao. Um job roda sob o usuario dele, e um job de
        // `exportar` da tabela restrita e exatamente o caminho que ninguem
        // olharia -- ele nao chega nem pelo soquete nem pelo SQL.
        self.executar_e_contar_escrita_local(op, &job.pedido, &sessao)
    }

    /// A sessao sob a qual o job roda.
    ///
    /// Sem cadastro de usuarios, o servidor inteiro entra sem login e o job
    /// acompanha -- e o mesmo comportamento da rede, e nao uma excecao. COM
    /// cadastro, o login e obrigatorio e tem de existir e estar ativo.
    fn sessao_do_job(&self, job: &crate::jobs::Job) -> Result<Sessao> {
        if self.cadastro().vazio() {
            return Ok(Sessao::default());
        }
        if job.usuario.is_empty() {
            return Err(PhxError::Autorizacao(format!(
                "job {:?} nao diz sob qual usuario roda, e este servidor tem cadastro. \
                 Um job sem dono rodaria com poder que ninguem concedeu",
                job.nome
            )));
        }
        let cadastro = self.cadastro();
        let u = cadastro.por_login(&job.usuario).ok_or_else(|| {
            PhxError::Autorizacao(format!(
                "job {:?}: o usuario {:?} nao esta no cadastro",
                job.nome, job.usuario
            ))
        })?;
        if !u.ativo {
            return Err(PhxError::Autorizacao(format!(
                "job {:?}: o usuario {:?} esta desativado",
                job.nome, job.usuario
            )));
        }
        Ok(Sessao {
            usuario: Some(u.clone()),
            ..Sessao::default()
        })
    }

    // ------------------------------------------------- aviso de jobs por e-mail

    /// Entra e sai da lista dos jobs em execucao agora.
    fn marcar_rodando(&self, nome: &str, esta: bool) {
        if let Ok(mut g) = self.jobs_rodando.lock() {
            if esta {
                g.push(nome.to_string());
            } else if let Some(i) = g.iter().position(|n| n == nome) {
                g.remove(i);
            }
        }
    }

    /// O aviso esta ligado? E O portao, um so, e vem antes de qualquer
    /// trabalho: desligado, quem chama paga duas leituras de booleano e nada
    /// mais -- nenhuma trava, nenhuma String, nenhum parse.
    ///
    /// Opt-in de proposito: `avisar_jobs` e um campo proprio no bloco de
    /// e-mail. Sem bloco de e-mail nada muda, e quem configurou e-mail so
    /// para o disco tambem continua exatamente como estava.
    fn aviso_de_jobs_ligado(&self) -> bool {
        let email = &self.config.alertas.email;
        email.ligado && email.avisar_jobs
    }

    /// Depois de cada corrida: avisa a falha por e-mail, e limpa o silencio
    /// de quem voltou a rodar.
    ///
    /// A limpeza espelha o vigia de disco: enquanto o job falha, no maximo um
    /// aviso por janela de `repetir_horas`; quando volta a dar certo, a chave
    /// sai do mapa e a PROXIMA falha avisa na hora, porque e noticia nova.
    fn avisar_sobre_a_corrida(&self, job: &crate::jobs::Job, corrida: &crate::jobs::Corrida) {
        if !self.aviso_de_jobs_ligado() {
            return;
        }
        let agora = crate::agora_ms();
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        let chave = format!("falha:{}", job.nome.to_lowercase());
        let mandar = {
            let Ok(mut avisados) = self.avisos_de_jobs.lock() else {
                return;
            };
            // Rodou -- entao parado nao esta.
            avisados.remove(&format!("parado:{}", job.nome.to_lowercase()));
            if corrida.ok {
                avisados.remove(&chave);
                false
            } else {
                crate::jobs::pode_avisar(&mut avisados, &chave, agora, silencio)
            }
        };
        if !mandar {
            return;
        }
        let email = self.config.alertas.email.clone();
        let assunto = format!("PhxSql: job {} falhou", job.nome);
        let corpo = Self::texto_do_aviso_de_falha(job, corrida);
        // Linha de execucao propria: quem dispara pode ser a tela, e ela nao
        // deve esperar o rele -- nem o timeout de um rele fora do ar.
        self.telemetria.subir(
            "aviso-job",
            "entrega UM e-mail de job que falhou e sai; existe em thread \
             propria porque quem dispara pode ser a tela, e ela nao pode \
             esperar o rele -- nem o timeout de um rele fora do ar",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("falando com o rele de e-mail");
                match crate::email::enviar(&email, &assunto, &corpo) {
                    Ok(r) => eprintln!("aviso de job enviado: {r}"),
                    // Falhar em avisar tambem e noticia, como no disco.
                    Err(e) => eprintln!("aviso de job NAO ENVIADO: {e}"),
                }
            },
        );
    }

    /// O corpo do e-mail de falha. Identifica o job, o motivo e a hora -- e
    /// NUNCA carrega credencial: o usuario aparece pelo login, o pedido pela
    /// operacao, e senha nao ha de onde vir.
    fn texto_do_aviso_de_falha(job: &crate::jobs::Job, c: &crate::jobs::Corrida) -> String {
        let mut t = String::new();
        t.push_str(&format!("O job {} falhou.\n\n", job.nome));
        if !job.descricao.is_empty() {
            t.push_str(&format!("  descrição  {}\n", job.descricao));
        }
        t.push_str(&format!(
            "  operação   {}\n  roda como  {}\n  agenda     {}\n  quando     {}\n  duração    {} ms\n\n  erro: {}\n\n",
            c.op,
            if c.usuario.is_empty() { "(sem cadastro)" } else { &c.usuario },
            job.agenda.rotulo(),
            phxsql_core::datahora::instante_iso(c.quando_ms),
            c.duracao_ms,
            c.detalhe
        ));
        t.push_str(
            "Enquanto o job continuar falhando, este aviso se repete no maximo uma vez \
             por janela de silêncio; quando ele voltar a rodar, a próxima falha avisa \
             na hora. O histórico completo está na tela Jobs e no log de corridas, ao \
             lado do cadastro de jobs.\n\n",
        );
        t.push_str(&format!("Servidor PhxSql {VERSAO}\n"));
        t
    }

    /// Sobe o vigia que avisa por e-mail o job PARADO -- ligado, com a hora
    /// vencida, e sem relogio para roda-lo (ex.: o primeiro job foi ligado
    /// pela tela depois do arranque, e o relogio so sobe no arranque).
    ///
    /// So existe se o aviso foi pedido: desligado nao custa nem a thread.
    /// E dorme ANTES da primeira conferencia, para o arranque terminar de
    /// subir o relogio -- senao todo arranque com job vencido comecaria com
    /// um alarme falso.
    pub(super) fn ligar_vigia_de_jobs(self: &Arc<Self>) {
        if !self.aviso_de_jobs_ligado() {
            return;
        }
        let email = &self.config.alertas.email;
        eprintln!(
            "aviso de jobs por e-mail: falha e parado | avisa {}",
            email.para.join(", ")
        );
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            "vigia-jobs",
            "avisa por e-mail o job PARADO: ligado, com a hora vencida e sem \
             relogio que o rode -- o caso que o proprio relogio nao percebe",
            "servico",
            crate::agora_ms(),
            move |fio| loop {
                fio.fazendo("dormindo ate a proxima conferencia");
                std::thread::sleep(Duration::from_secs(crate::jobs::PERIODO_DO_VIGIA_S));
                fio.fazendo("conferindo os jobs parados");
                servidor.conferir_jobs_parados();
            },
        );
    }

    /// Uma rodada do vigia de jobs parados.
    ///
    /// O predicado e o MESMO da tela (`jobs::job_parado`), para os dois nunca
    /// discordarem. O silencio e a limpeza espelham o vigia de disco: quem
    /// deixou de estar parado sai do mapa e volta a ter direito a aviso
    /// imediato.
    fn conferir_jobs_parados(&self) {
        let agora = crate::agora_ms();
        let relogio = self.relogio_de_jobs_no_ar();
        let rodando_agora: Vec<String> = self
            .jobs_rodando
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        // (nome, descricao, agenda, ultima corrida) de cada parado. A copia
        // sai de dentro da trava; o e-mail vai sem ela.
        let parados: Vec<(String, String, String, Option<crate::jobs::Corrida>)> = {
            let Ok(r) = self.jobs.lock() else { return };
            r.jobs
                .iter()
                .filter(|j| {
                    let rodando = rodando_agora
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&j.nome));
                    let vencido = j.agenda.hora_de_rodar(agora, r.ultimo_de(&j.nome));
                    crate::jobs::job_parado(j.ligado, rodando, vencido, relogio)
                })
                .map(|j| {
                    (
                        j.nome.clone(),
                        j.descricao.clone(),
                        j.agenda.rotulo(),
                        r.ultima_corrida_de(&j.nome).cloned(),
                    )
                })
                .collect()
        };
        let silencio = self.config.alertas.repetir_horas as i64 * 3_600_000;
        let novos: Vec<(String, String, String, Option<crate::jobs::Corrida>)> = {
            let Ok(mut avisados) = self.avisos_de_jobs.lock() else {
                return;
            };
            // Quem deixou de estar parado sai do mapa -- como o disco que
            // aliviou -- para a proxima parada avisar na hora.
            avisados.retain(|chave, _| {
                !chave.starts_with("parado:")
                    || parados
                        .iter()
                        .any(|(n, ..)| chave == &format!("parado:{}", n.to_lowercase()))
            });
            parados
                .into_iter()
                .filter(|(n, ..)| {
                    crate::jobs::pode_avisar(
                        &mut avisados,
                        &format!("parado:{}", n.to_lowercase()),
                        agora,
                        silencio,
                    )
                })
                .collect()
        };
        if novos.is_empty() {
            return;
        }
        for (nome, _, agenda, _) in &novos {
            eprintln!("JOB PARADO: {nome} ({agenda}) -- ligado, vencido e sem relogio");
        }
        let assunto = format!("PhxSql: {} job(s) agendado(s) sem rodar", novos.len());
        let corpo = Self::texto_do_aviso_de_parado(&novos, agora);
        // Este metodo ja roda na thread do vigia: o envio pode ser aqui mesmo.
        match crate::email::enviar(&self.config.alertas.email, &assunto, &corpo) {
            Ok(r) => eprintln!("aviso de job parado enviado: {r}"),
            Err(e) => eprintln!("aviso de job parado NAO ENVIADO: {e}"),
        }
    }

    fn texto_do_aviso_de_parado(
        parados: &[(String, String, String, Option<crate::jobs::Corrida>)],
        agora: i64,
    ) -> String {
        let mut t = String::new();
        t.push_str(
            "Há job agendado que não está rodando: a hora dele venceu e o relógio de \
             jobs não está no ar neste processo.\n\n",
        );
        for (nome, descricao, agenda, ultima) in parados {
            t.push_str(&format!("  {nome}\n"));
            if !descricao.is_empty() {
                t.push_str(&format!("    descrição  {descricao}\n"));
            }
            t.push_str(&format!("    agenda     {agenda}\n"));
            match ultima {
                Some(c) => t.push_str(&format!(
                    "    última     {} -- {}\n",
                    phxsql_core::datahora::instante_iso(c.quando_ms),
                    if c.ok { "ok" } else { "falhou" }
                )),
                None => t.push_str("    última     nunca rodou (que o log saiba)\n"),
            }
            t.push('\n');
        }
        t.push_str(
            "O relógio de jobs só sobe no arranque, e só se já havia job ligado. \
             Reinicie o servidor para a agenda valer -- ou rode o job pela tela \
             Jobs, que funciona sem relógio.\n\n",
        );
        t.push_str(&format!(
            "Servidor PhxSql {VERSAO}\nQuando: {}\n",
            phxsql_core::datahora::instante_iso(agora)
        ));
        t
    }
}

/// O resumo de uma resposta, para caber numa linha do historico de jobs.
///
/// Ele **analisa e reserializa**, nunca recorta: o corpo de um `varrer` de
/// vinte mil linhas nao entra cortado no meio, porque um pedaco de JSON nao e
/// JSON e a tela nao teria como distinguir "cortado" de "gravado assim". O que
/// nao se resume vira o tamanho em bytes, que e verdade sobre o que voltou.
fn resumir_resposta(j: &Json) -> String {
    let Json::Objeto(pares) = j else {
        return format!("{} bytes de resposta", j.escrever().len());
    };
    let curto: Vec<(String, Json)> = pares
        .iter()
        .filter(|(_, v)| !matches!(v, Json::Lista(_) | Json::Objeto(_)))
        .cloned()
        .collect();
    let grandes: Vec<String> = pares
        .iter()
        .filter_map(|(k, v)| match v {
            Json::Lista(l) => Some(format!("{k}: {} itens", l.len())),
            Json::Objeto(o) => Some(format!("{k}: {} campos", o.len())),
            _ => None,
        })
        .collect();
    let mut texto = Json::Objeto(curto).escrever();
    if !grandes.is_empty() {
        texto.push_str(&format!(" ({})", grandes.join(", ")));
    }
    texto
}
