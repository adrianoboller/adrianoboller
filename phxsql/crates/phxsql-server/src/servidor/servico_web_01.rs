//! A interface web, o HTTP, o REST e o swagger.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

impl Servidor {
    // ----------------------------------------------------------- interface web

    /// Sobe a interface web numa linha de execucao propria, se ligada.
    ///
    /// Falhar aqui NAO derruba o servidor: a interface e conforto, os dados
    /// sao o servico. Se a porta da web estiver ocupada, o aviso sai no
    /// terminal e a porta 5000 continua atendendo.
    pub(super) fn subir_web(self: &Arc<Self>) {
        if !self.config.web.ligado {
            return;
        }
        let endereco = match self.config.web.endereco() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("interface web NAO subiu: {e}");
                return;
            }
        };
        let tls = match self.identidade_http("web", &self.config.web.tls, &self.config.web.bind) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("interface web NAO subiu: {e}");
                return;
            }
        };
        let ouvinte = match TcpListener::bind(endereco) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("interface web NAO subiu em {endereco}: {e}");
                return;
            }
        };
        // O REAL, lido do ouvinte -- nao o `endereco` de cima, que e o do
        // config e continua "…:0" quando o `bind` pede porta 0. E' isto que
        // `porta_web()` devolve para quem subiu com porta 0 (pedido 401).
        if let Ok(e) = ouvinte.local_addr() {
            if let Ok(mut atual) = self.endereco_web.lock() {
                *atual = Some(e);
            }
        }
        anunciar(&format!(
            "interface web em {}://{endereco} | sessao de {} min sem uso, teto {}",
            if tls.is_some() { "https" } else { "http" },
            self.config.web.sessao_minutos,
            match self.config.web.sessao_teto_horas {
                0 => "nenhum".to_string(),
                h => format!("{h} h"),
            }
        ));
        // O estado do tunel de cada destino, dito ALTO no arranque -- e nao so
        // no dia em que alguem abre a conexao. `cifra` sem pino protege da
        // escuta passiva e nada mais, e essa e a linha que a §8 manda dizer com
        // estas palavras: vender protecao passiva como se fosse mais e o unico
        // jeito de o tunel enganar. O endereco entra, o pino NUNCA -- ele nem
        // e segredo (e chave publica), mas log nao e lugar de material de
        // chave, e a regra da casa se aplica sem excecao para nao ter de
        // decidir caso a caso.
        for sv in &self.config.web.servidores {
            if !sv.cifra {
                continue;
            }
            if sv.chave_do_fio.is_empty() {
                eprintln!(
                    "AVISO: web.servidores {} com cifra e SEM pino (chave_do_fio \
                     vazia): o tunel protege so da escuta PASSIVA, nao de quem \
                     esta no meio. Ponha o pino do destino para fechar isso.",
                    sv.endereco
                );
            } else {
                eprintln!("interface web -> {} | tunel cifrado com pino", sv.endereco);
            }
        }
        // O laco e o MESMO das outras duas portas HTTP, e isso e conserto:
        // ate o pedido 248 a interface tinha uma copia propria dele -- a que
        // nascia «SEM TETO», declarado num comentario --, e o teto entrou no
        // `aceitar_http`. Conserto entra no caminho que o motivou e o irmao
        // fica: a copia era o irmao, e some para nao divergir de novo.
        self.aceitar_http(ouvinte, "web", tls, |s, fluxo, par| {
            s.atender_http(fluxo, par)
        });
    }

    /// Atende um pedido HTTP. Uma resposta por conexao -- `Connection: close`.
    ///
    /// O portao de rede e o MESMO das outras duas portas HTTP, e isso e
    /// conserto: ate o pedido 370 a interface tinha uma copia propria dele --
    /// lista negra e whitelist repetidas linha a linha --, e a exigencia de
    /// cifra entrou no `portao_de_rede_http`. A copia era o irmao, e some para
    /// nao divergir de novo: e a mesma historia do laco de aceitacao no
    /// `subir_web`, que ja tinha sido paga aqui uma vez.
    pub(super) fn atender_http(&self, mut fluxo: http::FioWeb, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        if !self.portao_de_rede_http(&mut fluxo, &ip, porta, "web") {
            return;
        }

        let Some(pedido) = self.ler_pedido_http(&mut fluxo, &ip, porta, "web") else {
            return;
        };

        match (pedido.metodo.as_str(), pedido.caminho.as_str()) {
            ("GET", "/") | ("GET", "/index.html") => {
                // Pedido 339(a): a folga da Anthropic no CSP sai da config.
                let _ = http::responder_interface(
                    &mut fluxo,
                    http::montar_pagina(),
                    self.config.web.integracao_claude,
                );
            }
            // Sem token de proposito: e so o sinal de vida que a pagina usa
            // para saber se ha servidor desta origem. Nao conta tentativa e
            // nao diz nada sobre os dados.
            ("GET", "/saude") => {
                // Diz o que a pagina precisa para montar o formulario: a porta
                // que este servidor REALMENTE escuta (nao a de fabrica), os
                // servidores que ela pode alcancar e se ha chave a informar.
                // Nada aqui e segredo, e nada aqui depende de token.
                let _ = http::responder_json(
                    &mut fluxo,
                    200,
                    &Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("phxsql", Json::texto_de(VERSAO)),
                        // A porta que ele REALMENTE escuta agora, e nao a do
                        // arquivo: depois de uma troca pela tela, o formulario
                        // de entrada mandaria todo mundo para a porta velha.
                        ("porta_dados", Json::de_u64(self.porta_dados_agora() as u64)),
                        (
                            "porta_dados_no_ar",
                            Json::Bool(self.porta_no_ar.load(Ordering::SeqCst)),
                        ),
                        (
                            // So o endereco, como sempre foi: a lista antiga da
                            // pagina faz `d.split(":")` e monta o datalist.
                            "servidores",
                            Json::Lista(
                                self.config
                                    .web
                                    .servidores
                                    .iter()
                                    .map(|s| Json::texto_de(&s.endereco))
                                    .collect(),
                            ),
                        ),
                        (
                            // O estado do tunel por servidor, em SEPARADO da
                            // lista de cima para nao quebrar quem so quer o
                            // endereco. A tela usa isto para dizer, ao escolher
                            // o destino, se a conexao vai cifrada e com pino. O
                            // pino em si NAO sai -- so o fato de haver um, pela
                            // mesma regra da senha: resposta de protocolo nao
                            // carrega material de chave em texto puro.
                            "servidores_seguranca",
                            self.servidores_seguranca(),
                        ),
                        (
                            "exige_chave",
                            Json::Bool(self.cadastro().alguem_exige_chave()),
                        ),
                        (
                            // Pedido 667: a tela sem `crypto.subtle` so manda
                            // a senha pela reserva Base64 se o servidor a
                            // aceitaria. Sem isto ela mandaria e so depois
                            // ouviria a recusa -- com a senha ja no fio.
                            "senha_em_claro_pela_rede",
                            Json::Bool(self.config.cifra_fio.senha_em_claro_pela_rede),
                        ),
                        (
                            // Pedido 339(a): o desligamento administrativo da
                            // Claude. Quem barra e o CSP da pagina; isto so
                            // deixa a tela dizer o porque.
                            "integracao_claude",
                            Json::Bool(self.config.web.integracao_claude),
                        ),
                        (
                            // Pedido 772: a dissuasao de F12 e do botao
                            // direito. Vem ANTES do login porque a tela de
                            // entrada tambem e tela; e nao e segredo nenhum
                            // -- e dissuasao, nao seguranca.
                            "dissuadir_inspecao",
                            Json::Bool(self.config.web.dissuadir_inspecao),
                        ),
                    ]),
                );
            }
            // Sem token pelo mesmo motivo do /saude: a tela de ENTRADA precisa
            // dos rotulos antes de existir sessao, e um rotulo de campo nao e
            // dado. So serve os `TextName` de tela; as mensagens do protocolo
            // ficam na mesma tabela e nao saem por aqui.
            //
            // Sem a tabela isto nao toca em disco alem de tentar abrir a base:
            // devolve a fabrica, que e a tela de sempre.
            ("GET", "/idiomas") => {
                let idioma =
                    idiomas::indice_do_idioma(&http::parametro(&pedido.consulta, "idioma"));
                let corpo = match self.travar_dados() {
                    Ok(dados) => idiomas::textos_para_a_pagina(&dados, idioma),
                    Err(_) => idiomas::textos_para_a_pagina_sem_tabela(idioma),
                };
                let _ = http::responder_json(&mut fluxo, 200, &corpo);
            }
            ("POST", "/api") => self.api_http(&mut fluxo, &pedido, &ip, porta),
            ("GET", _) | ("HEAD", _) => {
                let _ = http::erro_json(
                    &mut fluxo,
                    404,
                    "esta interface tem quatro rotas: /, /saude, /idiomas e /api",
                );
            }
            _ => {
                let _ = http::erro_json(&mut fluxo, 405, "use GET / ou POST /api");
            }
        }
    }

    // -------------------------------------------------- webservice REST

    /// Sobe o webservice REST, se ligado. Falhar aqui NAO derruba o servidor.
    ///
    /// Mesmo desenho do `subir_web`, e pelo mesmo motivo: os dados sao o
    /// servico, e uma porta a mais que nao consegue subir vira aviso no
    /// terminal, nunca um servidor que nao arranca.
    pub(super) fn subir_rest(self: &Arc<Self>) {
        if !self.config.rest.ligado {
            return;
        }
        let endereco = match self.config.rest.endereco() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("webservice REST NAO subiu: {e}");
                return;
            }
        };
        let tls = match self.identidade_http("rest", &self.config.rest.tls, &self.config.rest.bind)
        {
            Ok(t) => t,
            Err(e) => {
                eprintln!("webservice REST NAO subiu: {e}");
                return;
            }
        };
        let esquema = if tls.is_some() { "https" } else { "http" };
        let ouvinte = match TcpListener::bind(endereco) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("webservice REST NAO subiu em {endereco}: {e}");
                return;
            }
        };
        // Mesmo motivo do `subir_web`: o REAL, lido do ouvinte, para
        // `porta_rest()` responder a quem subiu com porta 0.
        if let Ok(e) = ouvinte.local_addr() {
            if let Ok(mut atual) = self.endereco_rest.lock() {
                *atual = Some(e);
            }
        }
        anunciar(&format!(
            "webservice REST em {esquema}://{endereco}{} | especificacao em \
             {esquema}://{endereco}/openapi.json",
            crate::rest::PREFIXO
        ));
        self.aceitar_http(ouvinte, "rest", tls, |s, fluxo, par| {
            s.atender_rest(fluxo, par)
        });
    }

    /// Sobe o explorador da especificacao, se ligado -- a OUTRA porta.
    ///
    /// Ele e um segundo ouvinte de proposito: quem sobe numa placa quer o
    /// webservice sem o visualizador, e com uma porta so desligar o segundo
    /// exigiria desligar o primeiro.
    pub(super) fn subir_swagger(self: &Arc<Self>) {
        if !self.config.rest.swagger_ligado {
            return;
        }
        let endereco = match self.config.rest.endereco_do_swagger() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("explorador da API NAO subiu: {e}");
                return;
            }
        };
        // A secao `rest` cobre as DUAS portas dela, como o `atras_de_proxy`.
        let tls = match self.identidade_http(
            "rest",
            &self.config.rest.tls,
            &self.config.rest.swagger_bind,
        ) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("explorador da API NAO subiu: {e}");
                return;
            }
        };
        let ouvinte = match TcpListener::bind(endereco) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("explorador da API NAO subiu em {endereco}: {e}");
                return;
            }
        };
        // Mesmo motivo do `subir_web`: o REAL, lido do ouvinte, para
        // `porta_swagger()` responder a quem subiu com porta 0.
        if let Ok(e) = ouvinte.local_addr() {
            if let Ok(mut atual) = self.endereco_swagger.lock() {
                *atual = Some(e);
            }
        }
        anunciar(&format!(
            "explorador da API REST em {}://{endereco}",
            if tls.is_some() { "https" } else { "http" }
        ));
        self.aceitar_http(ouvinte, "swagger", tls, |s, fluxo, par| {
            s.atender_swagger(fluxo, par)
        });
    }

    /// O laco de aceitacao das portas HTTP, um pedido por conexao.
    ///
    /// Existe porque sao TRES ouvintes agora (web, REST e explorador) e o laco
    /// e o mesmo: aceitar, tirar o Nagle do caminho, registrar a linha de
    /// execucao na telemetria e entregar o pedido. Tres copias divergiriam na
    /// primeira correcao feita numa so.
    pub(super) fn aceitar_http(
        self: &Arc<Self>,
        ouvinte: TcpListener,
        familia: &'static str,
        tls: Option<Arc<phxsql_core::tls::Identidade>>,
        atender: fn(&Arc<Self>, http::FioWeb, SocketAddr),
    ) {
        let servidor = Arc::clone(self);
        self.telemetria.subir(
            format!("ouvinte-{familia}"),
            "aceita as conexoes desta porta HTTP e entrega cada pedido a uma \
             thread propria; ela so aceita, nunca atende",
            "servico",
            crate::agora_ms(),
            move |fio| {
                fio.fazendo("esperando conexao");
                for conexao in ouvinte.incoming() {
                    let mut fluxo = match conexao {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    // Mesma razao da porta de dados: resposta curta, e o Nagle
                    // segurando cada clique da tela por 40 ms.
                    let _ = fluxo.set_nodelay(true);
                    let par = fluxo
                        .peer_addr()
                        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
                    // A vaga vem ANTES da thread, e nao depois: o portao que
                    // decide se o trabalho acontece tem de vir antes do
                    // trabalho. Sem vaga em `fila_web_ms`, 503 daqui mesmo,
                    // sem subir nada.
                    let Some(vaga) = servidor.vaga_http() else {
                        servidor.recusar_http_cheio(&mut fluxo, par, familia, tls.is_none());
                        continue;
                    };
                    let tls = tls.clone();
                    let s = Arc::clone(&servidor);
                    s.telemetria.clone().subir(
                        format!("{familia}-{}", par.port()),
                        "atende UM pedido HTTP e sai: o protocolo aqui e uma \
                         resposta por conexao (`Connection: close`)",
                        familia,
                        crate::agora_ms(),
                        move |f| {
                            // A vaga morre com a thread -- inclusive num
                            // panico dentro do atendimento.
                            let _vaga = vaga;
                            f.fazendo(&format!("pedido de {par}"));
                            // O aperto TLS entra AQUI, uma vez, e nao em cada
                            // rota: daqui para baixo ninguem sabe se o fio e
                            // claro ou cifrado (pedido 572). E ele roda na
                            // thread da conexao, com o prazo de leitura de
                            // sempre -- o laco de aceitacao nao espera o
                            // aperto de ninguem.
                            let fio = match &tls {
                                None => http::FioWeb::Claro(fluxo),
                                Some(id) => {
                                    let _ = fluxo.set_read_timeout(Some(Duration::from_secs(
                                        s.config.timeout_s,
                                    )));
                                    match phxsql_core::tls::aceitar(fluxo, id, &[b"http/1.1"]) {
                                        Ok(t) => http::FioWeb::Tls(Box::new(t)),
                                        Err(e) => {
                                            s.anotar_aperto_tls_recusado(par, familia, &e);
                                            return;
                                        }
                                    }
                                }
                            };
                            atender(&s, fio, par);
                        },
                    );
                }
            },
        );
    }

    /// Uma vaga de thread HTTP, esperando ate `recursos.fila_web_ms`.
    ///
    /// `None` e «a fila estourou», e quem chamou responde 503. A espera e a
    /// diferenca entre esta porta e a de dados: do outro lado ha um navegador
    /// que refaz o pedido sozinho, e dois segundos de fila sao um clique que
    /// demorou -- nao um erro que a pessoa precisa entender.
    ///
    /// # O atalho da fila declarada cheia
    ///
    /// O aceitador e UMA thread. Se cada pedido de uma saturacao longa
    /// esperasse `fila_web_ms` inteiro antes do 503, o aceitador entregaria
    /// uma recusa a cada dois segundos e a fila do sistema operacional
    /// estouraria por tras dele -- e ai o cliente nem 503 receberia, so um
    /// `connect` que nao responde. Por isso, depois de UMA espera que
    /// estourou, os pedidos seguintes recebem o 503 na hora ate `fila_web_ms`
    /// depois: eles esperariam o mesmo tempo pela mesma resposta. A primeira
    /// vaga que aparece desfaz a declaracao.
    fn vaga_http(&self) -> Option<Permissao> {
        if let Some(p) = self.permissoes_http.tentar() {
            self.http_cheia_ate_ms.store(0, Ordering::Relaxed);
            return Some(p);
        }
        let fila_ms = self.config.recursos.fila_web_ms;
        if self.http_cheia_ate_ms.load(Ordering::Relaxed) > crate::agora_ms() as u64 {
            return None;
        }
        match self
            .permissoes_http
            .adquirir_ate(Duration::from_millis(fila_ms))
        {
            Some(p) => {
                self.http_cheia_ate_ms.store(0, Ordering::Relaxed);
                Some(p)
            }
            None => {
                self.http_cheia_ate_ms
                    .store(crate::agora_ms() as u64 + fila_ms, Ordering::Relaxed);
                None
            }
        }
    }

    /// O 503 de porta cheia, com `Retry-After` e linha no `acessos.log`.
    ///
    /// Roda no aceitador, entao nao le o pedido nem toma trava nenhuma: so
    /// escreve a recusa e escoa o que ja chegou (ver `http::responder_cheio`).
    fn recusar_http_cheio(
        &self,
        fluxo: &mut TcpStream,
        par: SocketAddr,
        familia: &'static str,
        em_claro: bool,
    ) {
        let fila_ms = self.config.recursos.fila_web_ms;
        let teto = self.permissoes_http.teto().to_string();
        let ms = fila_ms.to_string();
        let segundos = fila_ms.div_ceil(1_000).max(1);
        self.anotar(&Acesso {
            quando_ms: crate::agora_ms(),
            ip: par.ip().to_string(),
            porta_origem: par.port(),
            op: familia.into(),
            usuario: String::new(),
            autenticado: false,
            ok: false,
            // A duracao e a espera na fila: e o numero que diz se o teto
            // esta apertado ou se o pedido chegou numa saturacao declarada.
            duracao_ms: if self.http_cheia_ate_ms.load(Ordering::Relaxed) > 0 {
                0
            } else {
                fila_ms
            },
            erro: Some(format!(
                "porta HTTP cheia ({teto} threads, fila de {ms} ms)"
            )),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
            ..Acesso::default()
        });
        let corpo = Json::objeto(vec![
            ("ok", Json::Bool(false)),
            (
                "erro",
                Json::texto_de(self.msg("erro.porta_cheia", &[("teto", &teto), ("ms", &ms)])),
            ),
            ("retry_after_s", Json::de_u64(segundos)),
        ]);
        if !em_claro {
            // Numa porta TLS o 503 em claro chegaria como lixo no lugar de um
            // aperto, e fazer o aperto so para recusar gastaria na porta cheia
            // exatamente o que esta faltando. Fecha; o navegador tenta de novo.
            let _ = fluxo.shutdown(std::net::Shutdown::Both);
            return;
        }
        let _ = http::responder_cheio(fluxo, segundos, &corpo.escrever());
    }

    /// Aperto TLS que falhou numa porta HTTP: o cliente ja recebeu o alerta
    /// da §6, e o operador precisa ver o motivo no registro de acessos -- um
    /// navegador que desistiu do certificado e um robo mandando lixo sao
    /// coisas diferentes, e sem a linha os dois somem.
    pub(super) fn anotar_aperto_tls_recusado(&self, par: SocketAddr, familia: &str, e: &PhxError) {
        self.anotar(&Acesso {
            quando_ms: crate::agora_ms(),
            ip: par.ip().to_string(),
            porta_origem: par.port(),
            op: familia.into(),
            usuario: String::new(),
            autenticado: false,
            ok: false,
            duracao_ms: 0,
            erro: Some(format!("aperto TLS recusado: {e}")),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
            ..Acesso::default()
        });
    }

    /// A identidade TLS de uma porta HTTP, se a secao dela pedir (pedido 572).
    ///
    /// `Err` = pediu e nao deu, e a porta NAO sobe: cair calado para o claro
    /// seria o rebaixamento silencioso que o fio de dados ja recusa
    /// (`docs/CIFRA-DO-FIO.md` §2).
    /// O estado do fio de cada `web.servidores`, para o `/saude`: o
    /// interruptor e o FATO de haver pino (Noise e, desde o pedido 740, TLS)
    /// -- nunca o pino. Num lugar so para o teste alcancar o que a tela le.
    pub(super) fn servidores_seguranca(&self) -> Json {
        Json::Lista(
            self.config
                .web
                .servidores
                .iter()
                .map(|s| {
                    Json::objeto(vec![
                        ("endereco", Json::texto_de(&s.endereco)),
                        ("cifra", Json::Bool(s.cifra)),
                        ("tem_pino", Json::Bool(!s.chave_do_fio.is_empty())),
                        // Pedido 740: com pino TLS o destino fala TLS
                        // conferido, ate com a `cifra` desligada.
                        ("tem_pino_tls", Json::Bool(!s.pino_tls.is_empty())),
                    ])
                })
                .collect(),
        )
    }

    pub(super) fn identidade_http(
        &self,
        secao: &str,
        tls: &crate::config::TlsPorta,
        bind: &str,
    ) -> Result<Option<Arc<phxsql_core::tls::Identidade>>> {
        if !tls.ligado {
            return Ok(None);
        }
        let host = bind
            .rsplit_once(':')
            .map(|(h, _)| h.trim_matches(['[', ']']))
            .unwrap_or("");
        let mut nomes = vec!["localhost", "127.0.0.1"];
        if !host.is_empty() && !["0.0.0.0", "::", "localhost", "127.0.0.1"].contains(&host) {
            nomes.push(host);
        }
        let (id, avisos) = tls.identidade(secao, &nomes, self.config.caminho.as_deref())?;
        for a in avisos {
            eprintln!("aviso: {a}");
        }
        // O pino e o que o outro PhxSql escreve no `pino_tls` dele (pedido
        // 572, T6b-2). E publico -- o SHA-256 da chave PUBLICA --, e sem esta
        // linha a unica receita seria o encadeamento de quatro `openssl`.
        eprintln!(
            "tls da porta {secao}: pino {}",
            phxsql_core::tls::pino_em_texto(&id.pino()?)
        );
        Ok(Some(Arc::new(id)))
    }

    /// Os portoes de rede que valem antes de qualquer rota HTTP.
    ///
    /// A exigencia de cifra, a lista negra e a lista de IPs permitidos -- os
    /// mesmos da porta de dados. Devolve `false` quando ja respondeu a recusa
    /// e nao ha mais nada a fazer com esta conexao.
    ///
    /// # Por que a cifra e conferida AQUI, e num lugar so (pedido 370)
    ///
    /// `cifra_fio.exigir` decidia num lugar so -- o laco da porta de DADOS --
    /// e era anunciado por todas. Medido em 18/09/2026 no mesmo servidor e no
    /// mesmo instante: a porta nativa recusava e `POST /api {"op":"login"}`
    /// devolvia 200 com a sessao aberta e a senha em claro, `/v1/login` idem,
    /// `POST /mcp` devolvia o catalogo inteiro e o explorador servia 14.009
    /// bytes. Interruptor de seguranca se mede pelo que RECUSA.
    ///
    /// Esta funcao ja era o portao de rede das tres portas HTTP, e o endpoint
    /// `/mcp` entra por ela porque viaja na porta do REST -- entao a recusa
    /// entra aqui, e em nenhum outro lugar. Espalha-la por rota seria a porta
    /// dos fundos que a lei da casa manda procurar: *portao e UM so*, e a rota
    /// que alguem esquecesse continuaria atendendo em claro.
    ///
    /// E ela entra ANTES da lista negra de proposito: quando a exigencia
    /// morde, ela recusa todo pedido desta porta, entao decidir aqui poupa ate
    /// a trava da lista -- o portao que decide se o trabalho acontece vem
    /// antes do trabalho.
    fn portao_de_rede_http(
        &self,
        fluxo: &mut http::FioWeb,
        ip: &str,
        porta: u16,
        op: &str,
    ) -> bool {
        let agora = crate::agora_ms();
        // O fio TLS nativo (pedido 572) e cifrado de fato; o proxy e a
        // DECLARACAO de que ha um na frente. Qualquer dos dois atende.
        if self.config.cifra_fio.exigir && !fluxo.cifrado() && !self.proxy_desta_porta_http(op).0 {
            self.recusar_http_em_claro(fluxo, ip, porta, op, agora);
            return false;
        }
        if let Some(b) = self.barrado(ip, agora) {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.to_string(),
                porta_origem: porta,
                op: op.into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(Self::motivo_de_bloqueio(&b)),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
                ..Acesso::default()
            });
            // Escoa antes de fechar, senao o RST engole a recusa -- ver
            // `http::escoar`. Sem isto, quem esta na lista negra recebe
            // `Connection reset` e nunca sabe por que nem ate quando.
            let _ = http::erro_json(fluxo, 403, &self.recado_de_bloqueio(&b));
            http::escoar(fluxo);
            return false;
        }
        if !self.config.ip_permitido(ip) {
            self.violacao_leve(ip, op, "ip fora da lista de permitidos");
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.to_string(),
                porta_origem: porta,
                op: op.into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
                database: String::new(),
                tabela: String::new(),
                codigo: 0,
                ..Acesso::default()
            });
            let _ = http::erro_json(fluxo, 403, &self.msg("erro.ip_nao_autorizado", &[]));
            http::escoar(fluxo);
            return false;
        }
        true
    }

    /// Le o pedido de uma das tres portas HTTP -- e, quando ele passa de um
    /// teto, deixa RASTRO: linha no `acessos.log`, a recusa que chega (escoada
    /// antes de fechar, senao o RST a engole) e a violacao leve se o operador
    /// a pediu. E o irmao do pedido 216 na porta web (revisao SEC, B6, pedido
    /// 445): as tres respondiam 400 e saiam sem anotar nada.
    ///
    /// A conexao que fecha sem pedido, ou a linha torta, continua sem linha
    /// no log -- ver [`http::PedidoLido`] sobre por que anotar essas seria a
    /// amplificacao do pedido 444.
    fn ler_pedido_http(
        &self,
        fluxo: &mut http::FioWeb,
        ip: &str,
        porta: u16,
        op: &str,
    ) -> Option<http::Pedido> {
        let motivo = match http::ler_pedido_medindo(&mut *fluxo) {
            http::PedidoLido::Pedido(p) => return Some(p),
            http::PedidoLido::Nada => None,
            http::PedidoLido::GrandeDemais(m) => Some(m),
        };
        let _ = http::erro_json(fluxo, 400, "pedido HTTP invalido ou grande demais");
        if let Some(motivo) = motivo {
            let e = PhxError::LimiteExcedido(motivo);
            self.anotar(&Acesso {
                quando_ms: crate::agora_ms(),
                ip: ip.to_string(),
                porta_origem: porta,
                op: op.into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(e.to_string()),
                database: String::new(),
                tabela: String::new(),
                codigo: e.codigo(),
                ..Acesso::default()
            });
            // A MESMA politica da porta 5000, pelo mesmo interruptor: pedida,
            // nao imposta (pedido 203).
            if self.config.politica.contar_linha_acima_do_teto {
                self.violacao_leve(ip, op, "pedido HTTP acima do teto");
            }
            http::escoar(fluxo);
        }
        None
    }

    /// Esta porta HTTP tem o proxy TLS declarado? E em que secao ele se
    /// declara?
    ///
    /// O TLS do navegador e terminado por proxy reverso -- decisao do dono,
    /// `docs/SEGURANCA.md` §7.1 --, e `"atras_de_proxy": true` e a DECLARACAO
    /// de quem implanta. Ela nao e conferivel (o servidor nao tem como saber
    /// se o proxy existe), e e por isso que ela e o escape ESCRITO da
    /// exigencia nesta porta: a mesma forma do `"exigir": false` da porta de
    /// dados e do `"verificar": false` da chave conferida -- escolha escrita
    /// em vez de omissao.
    ///
    /// `rest` cobre as DUAS portas da secao (`bind` e `swagger_bind`), porque
    /// sobem do mesmo bloco e do mesmo operador: e a mesma regra do aviso de
    /// arranque, e uma segunda ideia de qual campo vale para o explorador
    /// divergiria na primeira correcao feita numa so.
    ///
    /// Familia que ninguem declarou cai no lado SEGURO -- sem proxy, recusada.
    /// No dia em que nascer uma quarta porta HTTP, ela e recusada ate alguem
    /// escrever a secao dela aqui, em vez de nascer sendo a porta dos fundos.
    fn proxy_desta_porta_http<'a>(&self, familia: &'a str) -> (bool, &'a str) {
        match familia {
            "web" => (self.config.web.atras_de_proxy, "web"),
            "rest" | "swagger" => (self.config.rest.atras_de_proxy, "rest"),
            _ => (false, familia),
        }
    }

    /// A recusa de uma porta HTTP quando a comunicacao cifrada e exigida.
    ///
    /// Erro NOMEADO e com as duas saidas escritas, pelo mesmo motivo da recusa
    /// da porta de dados: e a UNICA coisa que quem esta do outro lado recebe
    /// deste servidor, e um "acesso negado" seco mandaria procurar a permissao
    /// errada. Aqui o recado nao pode ser «peca o aperto de mao»: o navegador
    /// fala TLS ou fala claro, e o aperto do fio nao e opcao para ele.
    fn recusar_http_em_claro(
        &self,
        fluxo: &mut http::FioWeb,
        ip: &str,
        porta: u16,
        familia: &str,
        agora: i64,
    ) {
        let (_, secao) = self.proxy_desta_porta_http(familia);
        let recado = self.msg("erro.cifra_exigida_nesta_porta_http", &[("secao", secao)]);
        self.anotar(&Acesso {
            quando_ms: agora,
            ip: ip.to_string(),
            porta_origem: porta,
            op: familia.into(),
            usuario: String::new(),
            autenticado: false,
            ok: false,
            duracao_ms: 0,
            erro: Some(recado.clone()),
            database: String::new(),
            tabela: String::new(),
            codigo: 0,
            ..Acesso::default()
        });
        // Escoa antes de fechar pelo mesmo motivo da recusa por lista negra:
        // sem isto o RST engole a resposta, e quem foi recusado ve
        // «Connection reset» em vez do que precisa fazer.
        let _ = http::erro_json(fluxo, 403, &recado);
        http::escoar(fluxo);
    }

    /// Atende um pedido da porta REST.
    ///
    /// Recebe `self` como `Arc` porque o endpoint `/mcp` monta um
    /// [`ExecutorLocal`], que precisa de posse compartilhada do servidor -- a
    /// mesma que o `--mcp` por stdio usa. As rotas `/v1/<op>`, `/openapi.json`
    /// e `/saude` nao mudaram em nada com isso.
    fn atender_rest(self: &Arc<Self>, mut fluxo: http::FioWeb, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));
        if !self.portao_de_rede_http(&mut fluxo, &ip, porta, "rest") {
            return;
        }
        let Some(pedido) = self.ler_pedido_http(&mut fluxo, &ip, porta, "rest") else {
            return;
        };
        match (pedido.metodo.as_str(), pedido.caminho.as_str()) {
            // A especificacao viaja pela MESMA porta do servico, e nao so pela
            // do explorador: quem gera cliente a partir dela costuma nem abrir
            // o visualizador, e um `openapi.json` que exigisse uma segunda
            // porta ligada seria documentacao presa atras de uma opcao.
            ("GET", "/openapi.json") => {
                let _ = http::responder(
                    &mut fluxo,
                    200,
                    "application/json; charset=utf-8",
                    &crate::rest::openapi(&self.config.rest, VERSAO).escrever_identado(),
                );
            }
            ("GET", "/saude") => {
                let _ = http::responder_json(
                    &mut fluxo,
                    200,
                    &Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("phxsql", Json::texto_de(VERSAO)),
                        ("servico", Json::texto_de(self.config.rest.titulo())),
                        ("openapi", Json::texto_de("/openapi.json")),
                        // O endpoint MCP viaja pela MESMA porta: quem descobre a
                        // porta pelo /saude descobre a conversa com IA junto.
                        ("mcp", Json::texto_de("/mcp")),
                    ]),
                );
            }
            // O endpoint MCP (Model Context Protocol) sobre HTTP: o MESMO
            // JSON-RPC 2.0 do `--mcp`, agora pela porta REST -- para o N8n e o
            // Claude Code falarem por HTTP, e nao so pelo cano de um processo.
            // Somente leitura, sem valvula de escrita aqui -- ver `mcp_http`.
            ("POST", "/mcp") => self.mcp_http(&mut fluxo, &pedido, &ip, porta),
            // Streamable HTTP: um GET no endpoint abriria um fluxo SSE, e esta
            // ponte nao serve SSE -- responde uma resposta por POST e nada mais.
            // O 405 e o que a spec manda dizer, e evita um cliente ficar preso
            // esperando um stream que nunca vem.
            ("GET", "/mcp") => {
                let _ = http::erro_json(
                    &mut fluxo,
                    405,
                    "o endpoint MCP nao abre fluxo SSE; use POST /mcp",
                );
            }
            ("POST", caminho) => match crate::rest::operacao_do_caminho(caminho) {
                Some(o) => self.api_rest(&mut fluxo, &pedido, &ip, porta, o.nome),
                None => {
                    let _ = http::erro_json(
                        &mut fluxo,
                        404,
                        "rota desconhecida: as operacoes estao em /openapi.json, \
                         e o endpoint MCP e POST /mcp",
                    );
                }
            },
            ("GET", _) | ("HEAD", _) => {
                let _ = http::erro_json(
                    &mut fluxo,
                    404,
                    "esta porta atende POST /v1/<operacao>, POST /mcp, \
                     GET /openapi.json e GET /saude",
                );
            }
            _ => {
                let _ = http::erro_json(&mut fluxo, 405, "use POST /v1/<operacao> ou POST /mcp");
            }
        }
    }

    /// Atende o explorador da especificacao -- a porta do visualizador.
    ///
    /// Ela NAO despacha operacao nenhuma, e isso e desenho: a porta que
    /// documenta e a porta que executa sao coisas diferentes, e quem abre a
    /// primeira para a equipe nao quer abrir a segunda junto.
    fn atender_swagger(&self, mut fluxo: http::FioWeb, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));
        if !self.portao_de_rede_http(&mut fluxo, &ip, porta, "swagger") {
            return;
        }
        let Some(pedido) = self.ler_pedido_http(&mut fluxo, &ip, porta, "swagger") else {
            return;
        };
        match (pedido.metodo.as_str(), pedido.caminho.as_str()) {
            ("GET", "/") | ("GET", "/index.html") => {
                let _ = http::responder_pagina(
                    &mut fluxo,
                    200,
                    &crate::rest::explorador::pagina(&self.config.rest),
                );
            }
            ("GET", "/openapi.json") => {
                let _ = http::responder(
                    &mut fluxo,
                    200,
                    "application/json; charset=utf-8",
                    &crate::rest::openapi(&self.config.rest, VERSAO).escrever_identado(),
                );
            }
            // Os mesmos textos de tela que a interface web serve, pela mesma
            // funcao: o explorador nao tem uma segunda tabela de rotulos, e e
            // por isso que trocar o idioma nele funciona sem nada a mais.
            ("GET", "/idiomas") => {
                let idioma =
                    idiomas::indice_do_idioma(&http::parametro(&pedido.consulta, "idioma"));
                let corpo = match self.travar_dados() {
                    Ok(dados) => idiomas::textos_para_a_pagina(&dados, idioma),
                    Err(_) => idiomas::textos_para_a_pagina_sem_tabela(idioma),
                };
                let _ = http::responder_json(&mut fluxo, 200, &corpo);
            }
            ("GET", _) | ("HEAD", _) => {
                let _ = http::erro_json(
                    &mut fluxo,
                    404,
                    "o explorador tem tres rotas: /, /openapi.json e /idiomas",
                );
            }
            _ => {
                let _ = http::erro_json(
                    &mut fluxo,
                    405,
                    "esta porta so mostra a especificacao; os pedidos vao para a porta do REST",
                );
            }
        }
    }

    /// O segredo da porta REST -- o `Bearer` -- traduzido para o token do
    /// protocolo, ou `None` quando a recusa 401 ja foi escrita e nao ha mais o
    /// que fazer com esta conexao.
    ///
    /// # Por que num lugar so
    ///
    /// A porta REST (`/v1/<op>`) e o endpoint MCP (`/mcp`) compartilham esta
    /// porta da rede, e a regra dela e sutil: quando `rest.token` existe, ele
    /// SUBSTITUI o token do protocolo -- e o do protocolo deixa de abrir a
    /// porta, senao ligar o segredo do REST nao fecharia nada. Duas copias
    /// divergiriam na primeira correcao feita numa so, e a que ficasse para
    /// tras viraria a porta dos fundos. E o mesmo motivo do `portao_de_rede_http`
    /// morar num lugar so.
    ///
    /// Isto NAO e o portao de permissao: quem decide o que a sessao pode e o
    /// `despachar`, e ele confere o token de novo la dentro. Aqui e so a chave
    /// da porta da rede.
    fn token_do_rest(
        &self,
        fluxo: &mut http::FioWeb,
        pedido: &http::Pedido,
        ip: &str,
        porta: u16,
        op: &str,
        agora: i64,
    ) -> Option<String> {
        let apresentado = pedido
            .cabecalho("authorization")
            .unwrap_or("")
            .trim()
            .strip_prefix("Bearer ")
            .unwrap_or("")
            .trim()
            .to_string();
        if self.config.rest.token.is_empty() {
            Some(apresentado)
        } else if self.config.rest.token_confere(&apresentado) {
            // Passou pelo segredo da porta; o portao 1 continua conferindo o
            // token do protocolo, que e o que ele sempre conferiu. Em tempo
            // constante, como o portao 1 -- aqui era `==`.
            Some(self.config.token.clone())
        } else {
            self.violacao_de_credencial(ip, op, "token do REST invalido");
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.to_string(),
                porta_origem: porta,
                op: op.into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("token do REST invalido".into()),
                database: String::new(),
                tabela: String::new(),
                codigo: PhxError::Autorizacao(String::new()).codigo(),
                ..Acesso::default()
            });
            let _ = http::responder_json(
                fluxo,
                401,
                &Json::objeto(vec![
                    ("ok", Json::Bool(false)),
                    ("op", Json::texto_de(op)),
                    ("erro", Json::texto_de(self.msg("erro.token_invalido", &[]))),
                    ("codigo", Json::de_u64(4001)),
                    ("nome", Json::texto_de("ACESSO_NEGADO")),
                    ("classe", Json::texto_de("acesso")),
                    ("repetir", Json::Bool(false)),
                ]),
            );
            None
        }
    }

    /// O endpoint MCP sobre HTTP: o MESMO JSON-RPC 2.0 do `--mcp`, pela porta
    /// REST.
    ///
    /// # Por que reusa a Ponte, e nao reescreve a traducao
    ///
    /// A [`crate::mcp::Ponte`] traduz o vocabulario MCP e ja despacha por um
    /// [`ExecutorLocal`], que passa pelos quatro portoes do `despachar`. O
    /// transporte stdio (`mcp::servir`) e este HTTP sao duas portas para a
    /// MESMA sala: reescrever a traducao aqui seria a segunda verdade que o
    /// proprio `mcp.rs` recusou ter, e a que ficasse para tras esqueceria uma
    /// conferencia.
    ///
    /// # Somente leitura -- e SEM valvula de escrita nesta porta (fase 1)
    ///
    /// O `--mcp` por stdio tem `--escrita` porque quem o liga esta no terminal
    /// do servidor, com a mao na maquina. Aqui do outro lado ha a rede e um
    /// modelo de linguagem, e a escrita guardada pela IA e seguranca-sensivel:
    /// fica para a fase 2, depois da revisao do dono. A ponte nasce recusando
    /// escrita e esta porta NAO oferece como liga-la -- uma tentativa de
    /// `phx_inserir` volta com erro JSON-RPC dizendo por que. A fronteira e a
    /// prova: o teste `escrita_pelo_mcp_http_e_recusada` falha se alguem a abrir.
    /// DIVIDA: a porta MCP por HTTP nasce sem escrita e sem login por usuario -- as duas metades ficaram para a fase 2, com o dono
    ///
    /// # Uma resposta por pedido, sem sessao entre eles
    ///
    /// Cada POST /mcp e uma conexao (`Connection: close`) e a ponte nasce nova
    /// a cada uma: nao ha login que sobreviva de um pedido ao outro nesta fase
    /// -- login por usuario sobre HTTP e, como a escrita, materia da revisao do
    /// dono. O token e o da porta (o `Bearer`, ou o do protocolo), carimbado
    /// em todo pedido pela ponte, e o modelo nunca escolhe a credencial.
    fn mcp_http(
        self: &Arc<Self>,
        fluxo: &mut http::FioWeb,
        pedido: &http::Pedido,
        ip: &str,
        porta: u16,
    ) {
        let agora = crate::agora_ms();
        let Some(token) = self.token_do_rest(fluxo, pedido, ip, porta, "mcp", agora) else {
            return;
        };

        // O `ip` no lugar da origem: leitura pela IA que nao deixa rastro seria
        // um buraco na auditoria justamente na origem mais nova. O log sai de
        // dentro do `ExecutorLocal`, a cada `tools/call`.
        let executor = ExecutorLocal::novo(Arc::clone(self), ip);
        let ponte =
            crate::mcp::Ponte::nova(executor).com_campo_fixo("token", Json::texto_de(&token));

        match ponte.atender(&pedido.corpo) {
            Some(resposta) => {
                let _ = http::responder(fluxo, 200, "application/json; charset=utf-8", &resposta);
            }
            // Notificacao (mensagem sem `id`): o MCP manda 202 sem corpo. Um
            // corpo aqui seria uma resposta que o cliente nao espera -- o mesmo
            // engano que quebra o cliente no `notifications/initialized`.
            None => {
                let _ = http::responder(fluxo, 202, "application/json; charset=utf-8", "");
            }
        }
    }

    /// Uma operacao pedida por REST: monta o pedido, estreita e despacha.
    ///
    /// Tudo o que decide ACESSO acontece no `despachar`, como em toda outra
    /// porta. O que mora aqui e o que e proprio do HTTP: de onde vem o token,
    /// de onde vem a sessao, e que codigo devolver.
    fn api_rest(
        &self,
        fluxo: &mut http::FioWeb,
        pedido: &http::Pedido,
        ip: &str,
        porta: u16,
        op: &'static str,
    ) {
        let agora = crate::agora_ms();
        let inicio = Instant::now();

        // O portao da porta: o `Bearer`. Mora num lugar so porque o endpoint
        // MCP sobre HTTP entra pela mesma porta e pela mesma regra -- ver
        // `token_do_rest`. `None` quer dizer que a recusa 401 ja foi escrita.
        let Some(token_do_pedido) = self.token_do_rest(fluxo, pedido, ip, porta, op, agora) else {
            return;
        };

        let id_pedido = pedido
            .cabecalho("x-sessao")
            .unwrap_or("")
            .trim()
            .to_string();
        let (mut sessao, mut id_sessao) =
            self.sessao_do_cabecalho(&id_pedido, ip, "rest", fluxo.cifrado(), agora);

        // O pedido: o caminho manda a operacao, o corpo traz o resto, o
        // `config.json` estreita, e so entao o `despachar` decide.
        let montado = crate::rest::pedido_do_corpo(op, &pedido.corpo).and_then(|mut p| {
            p.definir("token", Json::texto_de(&token_do_pedido));
            crate::rest::estreitar(&self.config.rest, &mut p)?;
            Ok(p)
        });

        let (op_atendida, resultado) = match montado {
            Err(e) => (op.to_string(), Err(e)),
            Ok(p) => {
                let linha = p.escrever();
                let (o, _, r) = self.despachar(&linha, &mut sessao, ip);
                (o, r)
            }
        };
        let decorrido = inicio.elapsed();
        let ms = decorrido.as_millis() as u64;
        self.telemetria.contar_pedido(
            OPS_ESCRITA.contains(&op_atendida.as_str()),
            resultado.is_ok(),
        );

        // Um desafio em aberto so e consumido por um login -- a mesma regra do
        // `/api`, e pelo mesmo motivo: um pedido no meio do caminho devolveria
        // o nonce para a sessao em vez de derrubar a prova.
        if op != "login" && op != "desafio" {
            if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone()) {
                vivas.guardar_desafio(&id_sessao, d);
            }
        }
        if resultado.is_ok() {
            self.acertar_sessao(op, &sessao, &mut id_sessao, agora);
        }
        self.fim_da_sessao_web_pela_protecao(&sessao, &mut id_sessao);

        let (codigo_http, mut campos) = match &resultado {
            Ok(valor) => (
                200,
                vec![
                    ("ok", Json::Bool(true)),
                    ("op", Json::texto_de(&op_atendida)),
                    ("resultado", valor.clone()),
                    ("ms", Json::de_u64(ms)),
                ],
            ),
            Err(e) => (
                // 401 e 403 querem dizer coisas diferentes, e o cliente HTTP
                // trata cada uma de um jeito: 401 e «a PORTA nao abriu, mande
                // credencial»; 403 e «voce entrou e nao pode isso». Quando o
                // token do protocolo nao confere, a recusa e da porta -- e sem
                // esta linha ela saia 403, porque o `despachar` usa o mesmo
                // tipo de erro para as duas coisas.
                //
                // Quem DECIDE continua sendo o `despachar`: o pedido foi
                // despachado e recusado por ele. Isto so escolhe o numero, e
                // nao dispensa portao nenhum.
                if !self.config.token_confere(&token_do_pedido)
                    && matches!(e, PhxError::Autorizacao(_))
                {
                    401
                } else {
                    crate::rest::status_do_erro(e)
                },
                self.campos_do_erro(&op_atendida, e, ms),
            ),
        };
        if !id_sessao.is_empty() {
            campos.push(("sessao", Json::texto_de(&id_sessao)));
        }

        let alvo = objeto_do_pedido(&pedido.corpo, &resultado);
        self.anotar(&Acesso {
            quando_ms: agora,
            ip: ip.to_string(),
            porta_origem: porta,
            // Antes do `op`, que move o nome para dentro do registro.
            desfecho: self.desfecho_para_contar(&op_atendida, resultado.as_ref().ok()),
            op: op_atendida,
            usuario: sessao.login().to_string(),
            autenticado: sessao.usuario.is_some(),
            ok: resultado.is_ok(),
            duracao_ms: ms,
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            database: alvo.database,
            tabela: alvo.tabela,
            codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
            us: decorrido.as_micros().max(1) as u64,
            espera_us: crate::aquario::base::tomar_espera(),
            digital: crate::aquario::base::tomar_digital(),
        });
        let _ = http::responder_json(fluxo, codigo_http, &Json::objeto(campos));
    }

    /// Abre uma conexao para outro PhxSql e manda o login por ela.
    ///
    /// A politica DESTE servidor vale antes de qualquer coisa sair daqui:
    /// comando proibido aqui nao vira pedido la. A interface nao e uma porta
    /// dos fundos para o que a porta da frente recusa.
    #[allow(clippy::type_complexity)]
    pub(super) fn abrir_remoto(
        &self,
        destino: &str,
        linha: &str,
        ip: &str,
    ) -> std::result::Result<(String, Json, Arc<Mutex<Remoto>>), (String, PhxError)> {
        let op = Json::analisar(linha)
            .map(|j| j.texto_ou("op", "login").to_string())
            .unwrap_or_else(|_| "login".into());

        if !self.config.web.alcanca_outro_servidor() {
            return Err((
                op,
                PhxError::Autorizacao(
                    "esta interface nao fala com outro servidor: preencha web.servidores no config.json".into(),
                ),
            ));
        }
        if !self.config.web.servidor_permitido(destino) {
            // Endereco fora da lista e sondagem de rede, nao engano: alguem
            // esta procurando o que mais existe do outro lado.
            self.violacao_grave(ip, &op, "servidor fora de web.servidores");
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino} nao esta em web.servidores; o IP foi bloqueado"
                )),
            ));
        }
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            let erro = PhxError::Autorizacao(format!("operacao {op} esta proibida neste servidor"));
            return Err((op, erro));
        }

        let mut remoto =
            Remoto::abrir(destino, self.config.timeout_s).map_err(|e| (op.clone(), e))?;
        // O tunel ANTES do login, de proposito: e a prova do desafio-resposta e
        // o token que ele existe para esconder, e o `linha` abaixo e justamente
        // o login. So liga quando a configuracao DESTE destino pede `cifra` --
        // texto solto continua em claro, como sempre foi. Sem pino, protege so
        // da escuta passiva, e o arranque ja avisou disso. O `servidor(destino)`
        // devolve Some porque `servidor_permitido` acima ja deixou passar; o
        // `if let` e cinto de seguranca, nao um caminho novo.
        if let Some(sv) = self.config.web.servidor(destino) {
            // O `pino_tls` decide antes da `cifra` (pedido 572, T6b-2): a mesma
            // ordem do `replica::Cliente::proteger`.
            if let Some(p) = sv.pino_tls().map_err(|e| (op.clone(), e))? {
                remoto.cifrar_tls(p).map_err(|e| (op.clone(), e))?;
            } else if sv.cifra {
                let pino = sv.pino_do_fio().map_err(|e| (op.clone(), e))?;
                remoto.cifrar(pino).map_err(|e| (op.clone(), e))?;
            }
        }
        let resposta = remoto.conversar(linha).map_err(|e| (op.clone(), e))?;
        if !resposta.booleano_ou("ok", false) {
            return Err((
                op,
                PhxError::Autorizacao(format!(
                    "{destino}: {}",
                    resposta.texto_ou("erro", "recusou o login")
                )),
            ));
        }
        let valor = resposta.campo("resultado").cloned().unwrap_or(Json::Nulo);
        Ok((op, valor, Arc::new(Mutex::new(remoto))))
    }

    /// Manda o pedido para o servidor remoto desta sessao.
    fn encaminhar(
        &self,
        conexao: &Arc<Mutex<Remoto>>,
        linha: &str,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let op = match Json::analisar(linha) {
            Ok(j) => {
                let o = j.texto_ou("op", "ping").trim().to_string();
                if o.is_empty() {
                    "ping".to_string()
                } else {
                    o
                }
            }
            Err(e) => return ("?".into(), false, Err(e)),
        };
        // A politica local vale para o que passa por aqui, mesmo indo embora.
        if self.config.politica.comando_proibido(&op) {
            self.violacao_grave(ip, &op, "comando proibido pela politica");
            return (
                op.clone(),
                false,
                Err(PhxError::Autorizacao(format!(
                    "operacao {op} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }
        let mut r = match conexao.tomar("da conexao remota") {
            Ok(r) => r,
            Err(e) => return (op, false, Err(e)),
        };
        match r.conversar(linha) {
            Ok(resposta) => {
                if resposta.booleano_ou("ok", false) {
                    (
                        op,
                        true,
                        Ok(resposta.campo("resultado").cloned().unwrap_or(Json::Nulo)),
                    )
                } else {
                    let erro = resposta
                        .texto_ou("erro", "o servidor remoto recusou")
                        .to_string();
                    (op, true, Err(PhxError::Autorizacao(erro)))
                }
            }
            Err(e) => (op, true, Err(e)),
        }
    }

    /// A sessao que um cabecalho `X-Sessao` reconstroi.
    ///
    /// # Por que ela nao mora dentro do `/api`
    ///
    /// Porque ha DOIS caminhos HTTP agora -- a interface web e o webservice
    /// REST -- e os dois precisam da mesma memoria de quem entrou. Uma copia
    /// em cada lugar seria duas ideias do que e estar logado, e a que alguem
    /// esquecesse de atualizar viraria a porta dos fundos: e a mesma razao
    /// pela qual o portao de permissao e um so.
    ///
    /// Devolve a sessao reconstruida e o identificador que continua valendo
    /// (vazio quando o cabecalho veio vazio, errado ou vencido).
    fn sessao_do_cabecalho(
        &self,
        id_pedido: &str,
        ip: &str,
        familia: &str,
        fio_tls: bool,
        agora: i64,
    ) -> (Sessao, String) {
        let duracao = self.config.web.sessao_ms();
        let mut sessao = Sessao {
            ip: ip.to_string(),
            // A mesma pergunta do portao de rede, pela mesma funcao: uma
            // segunda ideia de qual secao declara o proxy divergiria na
            // primeira correcao feita numa so.
            ip_do_proxy: self.proxy_desta_porta_http(familia).0,
            // Os DOIS caminhos HTTP passam por aqui -- a interface web e o
            // webservice REST --, e e por isso que a marca da entrada tambem
            // mora num lugar so: uma copia em cada lado seria duas ideias do
            // que e estar em claro, e a que alguem esquecesse voltaria a
            // anunciar cifra onde nao ha.
            entrada: Entrada::Http,
            fio_tls,
            ..Sessao::default()
        };
        let mut id_sessao = String::new();
        if !id_pedido.is_empty() {
            if let Ok(mut vivas) = self.sessoes.lock() {
                if let Some(login) =
                    vivas.usar(id_pedido, duracao, self.config.web.sessao_teto_ms(), agora)
                {
                    id_sessao = id_pedido.to_string();
                    sessao.desafio = vivas.tomar_desafio(id_pedido);
                    if !login.is_empty() {
                        // O cadastro VIVO, e nao o do arranque: excluir ou
                        // desativar alguem tem de valer no proximo clique
                        // dele, sem esperar reinicio nenhum.
                        sessao.usuario = self
                            .cadastro()
                            .por_login(&login)
                            .filter(|u| u.ativo)
                            .cloned();
                        // A liberacao da senha de execucao vem junto, para o
                        // mesmo login e o MESMO IP -- o veredito confere os
                        // dois (`Sessao::execucao_liberada`).
                        if let Some(ip_liberado) = vivas.execucao_liberada_em(id_pedido) {
                            sessao.execucao_liberada = Some(LiberacaoDeExecucao {
                                login: login.clone(),
                                ip: ip_liberado,
                            });
                        }
                    }
                }
            }
        }
        (sessao, id_sessao)
    }

    /// Acerta a sessao web depois de um despacho que deu certo.
    ///
    /// Quatro operacoes mexem nela e nenhuma outra: o `desafio` cria a sessao
    /// anonima que carrega o nonce, o `login` a troca por uma NOVA com nome
    /// (pedido 719 -- ver `girar_sessao`), o `sair` a encerra e o
    /// `trancar_execucao` tira a liberacao da senha de execucao (767). Esta funcao e chamada pelos DOIS caminhos HTTP -- ver
    /// `sessao_do_cabecalho` para o motivo de nao haver duas copias.
    /// A sessao que a camada de protecao encerrou (pedido 766, P8) morre
    /// tambem no `http::Sessoes`: a copia do `despachar` ja perdeu a
    /// identidade, mas o id do navegador continuaria valendo no clique
    /// seguinte. UM lugar para as duas portas HTTP que despacham local -- a
    /// `/api` e o REST --, pelo mesmo motivo do `acertar_sessao`.
    pub(super) fn fim_da_sessao_web_pela_protecao(&self, sessao: &Sessao, id_sessao: &mut String) {
        if !sessao.encerrada_pela_protecao || id_sessao.is_empty() {
            return;
        }
        if let Ok(mut vivas) = self.sessoes.lock() {
            vivas.encerrar(id_sessao);
        }
        if let Ok(mut r) = self.remotos.lock() {
            r.remove(id_sessao.as_str());
        }
        id_sessao.clear();
    }

    fn acertar_sessao(&self, op: &str, sessao: &Sessao, id_sessao: &mut String, agora: i64) {
        let duracao = self.config.web.sessao_ms();
        match op {
            "desafio" => {
                if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone()) {
                    // O desafio vem antes da identidade: a sessao nasce
                    // anonima so para carregar o nonce ate o login.
                    if id_sessao.is_empty() {
                        *id_sessao = vivas.nova("", duracao, agora);
                    }
                    vivas.guardar_desafio(id_sessao, d);
                }
            }
            "login" => self.girar_sessao(id_sessao, sessao.login(), agora),
            // A senha de execucao liberou a sessao (pedido 767): o id GIRA,
            // como no login (719). O id velho cruzou o fio antes da prova; se
            // alguem o levou, leva uma sessao que nunca foi liberada.
            "liberar_execucao" => {
                self.girar_sessao(id_sessao, sessao.login(), agora);
                if let Ok(mut vivas) = self.sessoes.lock() {
                    vivas.liberar_execucao(id_sessao, &sessao.ip);
                }
            }
            "trancar_execucao" => {
                if let Ok(mut vivas) = self.sessoes.lock() {
                    vivas.trancar_execucao(id_sessao);
                }
            }
            "sair" => {
                if let Ok(mut vivas) = self.sessoes.lock() {
                    vivas.encerrar(id_sessao);
                }
                if let Ok(mut r) = self.remotos.lock() {
                    r.remove(id_sessao.as_str());
                }
                id_sessao.clear();
            }
            _ => {}
        }
    }

    /// O login troca o id de sessao: nasce um NOVO, e o anterior morre
    /// (pedido 719, fixacao de sessao).
    ///
    /// O id do `desafio` nasce anonimo e cruza o fio antes da credencial;
    /// promove-lo no login entregaria a identidade a quem o viu passar --
    /// ou a quem o plantou no navegador da vitima. Vale tambem com TLS: a
    /// fixacao nao depende de escutar o fio. O remoto, se havia, muda de
    /// chave junto, porque o encaminhamento e procurado pelo id.
    ///
    /// UM lugar para os dois caminhos que fazem login pela web: o local (por
    /// `acertar_sessao`) e o que vai para outro servidor, que nao passa por
    /// ele.
    fn girar_sessao(&self, id_sessao: &mut String, login: &str, agora: i64) {
        let duracao = self.config.web.sessao_ms();
        let velho = std::mem::take(id_sessao);
        if let Ok(mut vivas) = self.sessoes.lock() {
            *id_sessao = vivas.nova(login, duracao, agora);
            if !velho.is_empty() {
                vivas.encerrar(&velho);
            }
        }
        if !velho.is_empty() {
            if let Ok(mut r) = self.remotos.lock() {
                if let Some(conexao) = r.remove(velho.as_str()) {
                    if !id_sessao.is_empty() {
                        r.insert(id_sessao.clone(), conexao);
                    }
                }
            }
        }
    }

    /// O `/api`: o mesmo protocolo da porta 5000, um pedido por vez.
    ///
    /// A diferenca esta na identidade. Em TCP a conexao lembra quem entrou; em
    /// HTTP nao ha conexao que dure, entao a memoria e a sessao: o `login`
    /// devolve um identificador, o navegador o repete no cabecalho `X-Sessao`,
    /// e o PBKDF2 de 210.000 iteracoes roda uma vez por login em vez de uma
    /// vez por clique.
    fn api_http(&self, fluxo: &mut http::FioWeb, pedido: &http::Pedido, ip: &str, porta: u16) {
        let duracao = self.config.web.sessao_ms();
        let agora = crate::agora_ms();
        let id_pedido = pedido
            .cabecalho("x-sessao")
            .unwrap_or("")
            .trim()
            .to_string();

        let (mut sessao, mut id_sessao) =
            self.sessao_do_cabecalho(&id_pedido, ip, "web", fluxo.cifrado(), agora);

        // Abrir conexao para outro PhxSql, se o login pediu um servidor.
        //
        // O campo se chama "servidor" e nao "destino" porque "destino" ja e o
        // diretorio do backup -- e a colisao de nome mandava todo pedido de
        // backup para o relay. Achado ligando a peca, nao lendo o codigo.
        let servidor_remoto = Json::analisar(&pedido.corpo)
            .ok()
            .map(|j| j.texto_ou("servidor", "").trim().to_string())
            .unwrap_or_default();

        let inicio = Instant::now();
        let quando_ms = crate::agora_ms();

        // A atividade da web e da SESSAO, e nao do pedido: HTTP abre uma
        // conexao por clique, e uma bolha por conexao viraria um enxame que
        // nasce e morre a cada volta da tela. Sem sessao -- o login e o
        // `saude` --, a chave e o IP, que e o dono que existe naquele
        // instante.
        //
        // A chave e o RESUMO da sessao, e nunca o id dela: o id e a
        // credencial do `X-Sessao`, e a chave da atividade sai no `telemetria`,
        // no `aquario_retrato` (que quem so MONITORA le), no `aquario.log` e
        // no `ocorrencias.log`. Com o id cru, a TV levava a sessao do
        // administrador para casa. Achado exercitando a F9 do 495: a
        // ocorrencia gravava `"tarefa":"web:<id da sessao>"`.
        let chave_da_atividade = if id_sessao.is_empty() {
            format!("web:{ip}")
        } else {
            format!("web:{}", resumo_da_sessao(&id_sessao))
        };
        let atividade = self
            .telemetria
            .entrar(&chave_da_atividade, "web", ip, 0, quando_ms);
        let _amarrada = crate::telemetria::amarrar(atividade.clone());
        if let Some(a) = &atividade {
            let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
            let nome_op = Json::analisar(&pedido.corpo)
                .ok()
                .map(|j| j.texto_ou("op", "?").to_string())
                .unwrap_or_else(|| "?".into());
            a.comecou_pedido(
                &nome_op,
                sessao.login(),
                &alvo.database,
                &alvo.tabela,
                quando_ms,
            );
        }

        let ja_remota = self
            .remotos
            .lock()
            .ok()
            .and_then(|r| r.get(&id_sessao).cloned());

        // O login que vai para OUTRO servidor nao passa pelo `op_login`
        // daqui, mas a senha atravessou o fio ate aqui do mesmo jeito: o
        // mesmo portao, antes de abrir ou de encaminhar (pedido 667).
        // E o id de sessao que o desafio e o login remotos fazem nascer aqui
        // tambem nao sai por fio em claro (pedido 674): o remoto nao passa
        // pelo `op_desafio` nem pelo `op_login` deste servidor, entao o
        // portao e chamado no caminho dele.
        let fio_da_senha = if ja_remota.is_some() || !servidor_remoto.is_empty() {
            Json::analisar(&pedido.corpo).ok().and_then(|j| {
                let op_pedida = j.texto_ou("op", "").trim();
                let emite = op_pedida == "login" || op_pedida == "desafio";
                self.conferir_o_fio_da_senha(&j, &sessao)
                    .and_then(|()| {
                        if emite {
                            self.conferir_a_emissao_da_sessao(&sessao)
                        } else {
                            Ok(())
                        }
                    })
                    .err()
            })
        } else {
            None
        };
        let (op, autenticado, resultado) =
            match (fio_da_senha, &ja_remota, servidor_remoto.is_empty()) {
                (Some(e), _, _) => ("login".to_string(), false, Err(e)),
                // Sessao ja amarrada a um servidor remoto: tudo vai para la.
                (None, Some(conexao), _) => {
                    let saida = self.encaminhar(conexao, &pedido.corpo, ip);
                    // O login que o remoto aceitou troca o id, como o local.
                    if saida.0 == "login" && saida.2.is_ok() {
                        self.girar_sessao(&mut id_sessao, "", agora);
                    }
                    saida
                }
                // Login novo pedindo servidor: abre, encaminha, e guarda se entrou.
                (None, None, false) => {
                    let r = self.abrir_remoto(&servidor_remoto, &pedido.corpo, ip);
                    match r {
                        Ok((op, valor, conexao)) => {
                            if op == "login" {
                                // Login aceito: id novo, o anterior morre.
                                self.girar_sessao(&mut id_sessao, "", agora);
                            } else if id_sessao.is_empty() {
                                if let Ok(mut vivas) = self.sessoes.lock() {
                                    id_sessao = vivas.nova("", duracao, agora);
                                }
                            }
                            if let Ok(mut r) = self.remotos.lock() {
                                r.insert(id_sessao.clone(), conexao);
                            }
                            (op, true, Ok(valor))
                        }
                        Err((op, e)) => (op, false, Err(e)),
                    }
                }
                (None, None, true) => {
                    // O PROFILER olha aqui tambem. A porta da interface e HTTP e
                    // nao JSON por linha, mas o pedido e o mesmo objeto e chega
                    // pelo mesmo TCP -- deixar a web de fora faria o profiler
                    // mentir por omissao justamente para quem esta olhando por
                    // ela.
                    let marca = if self.profiler_ligado.load(Ordering::Relaxed) {
                        let alvo = objeto_do_pedido(&pedido.corpo, &Ok(Json::Nulo));
                        let nome_op = Json::analisar(&pedido.corpo)
                            .ok()
                            .map(|j| j.texto_ou("op", "?").to_string())
                            .unwrap_or_else(|| "?".into());
                        self.profiler.lock().ok().and_then(|mut pr| {
                            pr.chegou(
                                &pedido.corpo,
                                &nome_op,
                                sessao.login(),
                                &alvo.database,
                                &alvo.tabela,
                                ip,
                                agora,
                            )
                        })
                    } else {
                        None
                    };
                    let saida = self.despachar(&pedido.corpo, &mut sessao, ip);
                    if let Some(serial) = marca {
                        if let Ok(mut pr) = self.profiler.lock() {
                            pr.terminou(
                                serial,
                                inicio.elapsed().as_millis() as u64,
                                saida.2.is_ok(),
                                &saida
                                    .2
                                    .as_ref()
                                    .err()
                                    .map(|e| e.to_string())
                                    .unwrap_or_default(),
                            );
                        }
                    }
                    saida
                }
            };
        let remota = ja_remota.is_some() || !servidor_remoto.is_empty();
        let decorrido = inicio.elapsed();
        let ms = decorrido.as_millis() as u64;
        if let Some(a) = &atividade {
            a.terminou_pedido(sessao.login());
        }
        self.telemetria
            .contar_pedido(OPS_ESCRITA.contains(&op.as_str()), resultado.is_ok());

        // Um desafio em aberto so e consumido por um login. Qualquer outra
        // operacao no meio do caminho devolve o nonce para a sessao, senao um
        // "ping" entre o desafio e o login derrubaria a prova.
        if op != "login" && op != "desafio" {
            if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone()) {
                vivas.guardar_desafio(&id_sessao, d);
            }
        }

        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() && !remota {
            self.acertar_sessao(&op, &sessao, &mut id_sessao, agora);
        }
        self.fim_da_sessao_web_pela_protecao(&sessao, &mut id_sessao);

        let mut campos = match &resultado {
            Ok(valor) => vec![
                ("ok", Json::Bool(true)),
                ("op", Json::texto_de(&op)),
                ("resultado", valor.clone()),
                ("ms", Json::de_u64(ms)),
            ],
            // O codigo vem JUNTO com o texto, e nao no lugar dele: o texto e
            // para quem le, o codigo e para quem programa. Trocar um pelo
            // outro obrigaria alguem a perder. O texto passa pela tabela de
            // mensagens; o codigo nunca muda com o idioma.
            Err(e) => self.campos_do_erro(&op, e, ms),
        };
        if !id_sessao.is_empty() {
            campos.push(("sessao", Json::texto_de(&id_sessao)));
        }

        self.anotar(&Acesso {
            quando_ms,
            ip: ip.to_string(),
            porta_origem: porta,
            op: op.clone(),
            usuario: sessao.login().to_string(),
            autenticado,
            ok: resultado.is_ok(),
            duracao_ms: ms,
            us: decorrido.as_micros().max(1) as u64,
            espera_us: crate::aquario::base::tomar_espera(),
            digital: crate::aquario::base::tomar_digital(),
            erro: resultado.as_ref().err().map(|e| e.to_string()),
            desfecho: self.desfecho_para_contar(&op, resultado.as_ref().ok()),
            // O objeto sai do proprio pedido: e o unico ponto que ve os dois
            // -- a operacao e sobre o que ela foi.
            ..objeto_do_pedido(&pedido.corpo, &resultado)
        });

        if remota && op == "sair" {
            if let Ok(mut r) = self.remotos.lock() {
                r.remove(&id_sessao);
            }
            if let Ok(mut vivas) = self.sessoes.lock() {
                vivas.encerrar(&id_sessao);
            }
            id_sessao.clear();
        }

        let codigo = match &resultado {
            Ok(_) => 200,
            Err(PhxError::Autorizacao(_)) => 403,
            Err(PhxError::NaoEncontrado(_)) => 404,
            Err(_) => 400,
        };
        let _ = http::responder_json(fluxo, codigo, &Json::objeto(campos));
    }
}

/// Os campos do log que saem do PEDIDO, e nao do resultado.
///
/// Devolve um `Acesso` so para preencher com `..`: os outros campos do
/// registro vem de quem chama, e repetir a leitura do corpo em dois lugares e
/// como os dois caminhos (porta de dados e web) divergiriam com o tempo.
pub(super) fn objeto_do_pedido(corpo: &str, resultado: &Result<Json>) -> Acesso {
    let j = Json::analisar(corpo).unwrap_or(Json::Nulo);
    Acesso {
        database: j.texto_ou("database", "").to_string(),
        tabela: j.texto_ou("tabela", "").to_string(),
        codigo: resultado.as_ref().err().map(|e| e.codigo()).unwrap_or(0),
        ..Acesso::default()
    }
}

/// O nome PUBLICO de uma sessao web: 8 bytes do SHA-256 do id, em hex.
///
/// O id da sessao e credencial (o `X-Sessao` que autentica cada clique), e
/// a chave da atividade e mostrada a quem administra e a quem so monitora.
/// O resumo continua um por sessao -- a bolha da mesma sessao e a mesma, e o
/// `telemetria_encerrar` acha a atividade por ele --, mas nao abre porta
/// nenhuma: do resumo nao se volta ao id.
pub(super) fn resumo_da_sessao(id_sessao: &str) -> String {
    let h = phxsql_core::hash::sha256(id_sessao.as_bytes());
    phxsql_core::hash::para_hex(&h[..8])
}
