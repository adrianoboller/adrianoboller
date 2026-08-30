# Add web layer and build
# 27/08 19:45

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# 1. extrai a checagem de bloqueio do atender() para um metodo reaproveitavel
velho = '''        // Antes de qualquer coisa: quem esta na lista de bloqueio nao entra.
        let agora = crate::agora_ms();
        let bloqueado = {
            let mut lista = match self.lista_negra.lock() {
                Ok(l) => l,
                Err(_) => return,
            };
            // Outro processo pode ter mexido no arquivo (phxsqld --desbloquear).
            let _ = lista.recarregar_se_mudou();
            let _ = lista.limpar_vencidos(agora, &self.config.politica);
            lista.bloqueado(&ip, agora).map(|b| {
                format!(
                    "bloqueado desde {} ate {} por {} ({})",
                    b.desde(),
                    b.ate(),
                    b.motivo,
                    b.comando
                )
            })
        };
        if let Some(motivo) = bloqueado {'''
novo = '''        // Antes de qualquer coisa: quem esta na lista de bloqueio nao entra.
        let agora = crate::agora_ms();
        if let Some(motivo) = self.barrado(&ip, agora) {'''
assert s.count(velho)==1
s = s.replace(velho, novo)

# 2. metodo barrado + toda a camada web, antes de fn atender
ancora = '''    fn atender(&self, fluxo: TcpStream, par: SocketAddr) {'''
assert s.count(ancora)==1
bloco = '''    /// Este IP esta barrado agora? Devolve o motivo, ja formatado para o log.
    ///
    /// Reaproveitado pelas duas portas: a de dados e a da interface web. Um IP
    /// bloqueado e bloqueado no servidor inteiro, nao numa porta so.
    fn barrado(&self, ip: &str, agora: i64) -> Option<String> {
        let mut lista = self.lista_negra.lock().ok()?;
        // Outro processo pode ter mexido no arquivo (phxsqld --desbloquear).
        let _ = lista.recarregar_se_mudou();
        let _ = lista.limpar_vencidos(agora, &self.config.politica);
        lista.bloqueado(ip, agora).map(|b| {
            format!(
                "bloqueado desde {} ate {} por {} ({})",
                b.desde(),
                b.ate(),
                b.motivo,
                b.comando
            )
        })
    }

    // ----------------------------------------------------------- interface web

    /// Sobe a interface web numa linha de execucao propria, se ligada.
    ///
    /// Falhar aqui NAO derruba o servidor: a interface e conforto, os dados
    /// sao o servico. Se a porta da web estiver ocupada, o aviso sai no
    /// terminal e a porta 5000 continua atendendo.
    fn subir_web(self: &Arc<Self>) {
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
        let ouvinte = match TcpListener::bind(endereco) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("interface web NAO subiu em {endereco}: {e}");
                return;
            }
        };
        eprintln!(
            "interface web em http://{endereco} | sessao de {} min",
            self.config.web.sessao_minutos
        );
        let servidor = Arc::clone(self);
        std::thread::spawn(move || {
            for conexao in ouvinte.incoming() {
                let fluxo = match conexao {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let par = fluxo
                    .peer_addr()
                    .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
                let s = Arc::clone(&servidor);
                std::thread::spawn(move || s.atender_http(fluxo, par));
            }
        });
    }

    /// Atende um pedido HTTP. Uma resposta por conexao -- `Connection: close`.
    fn atender_http(&self, mut fluxo: TcpStream, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        let agora = crate::agora_ms();
        if let Some(motivo) = self.barrado(&ip, agora) {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip,
                porta_origem: porta,
                op: "web".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(motivo.clone()),
            });
            let _ = http::erro_json(&mut fluxo, 403, &motivo);
            return;
        }
        if !self.config.ip_permitido(&ip) {
            self.violacao_leve(&ip, "web", "ip fora da lista de permitidos");
            self.anotar(&Acesso {
                quando_ms: agora,
                ip,
                porta_origem: porta,
                op: "web".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
            });
            let _ = http::erro_json(&mut fluxo, 403, "ip nao autorizado");
            return;
        }

        let pedido = match http::ler_pedido(&fluxo) {
            Some(p) => p,
            None => {
                let _ = http::erro_json(&mut fluxo, 400, "pedido HTTP invalido ou grande demais");
                return;
            }
        };

        match (pedido.metodo.as_str(), pedido.caminho.as_str()) {
            ("GET", "/") | ("GET", "/index.html") => {
                let _ = http::responder(&mut fluxo, 200, "text/html; charset=utf-8", http::PAGINA);
            }
            // Sem token de proposito: e so o sinal de vida que a pagina usa
            // para saber se ha servidor desta origem. Nao conta tentativa e
            // nao diz nada sobre os dados.
            ("GET", "/saude") => {
                let _ = http::responder_json(
                    &mut fluxo,
                    200,
                    &Json::objeto(vec![
                        ("ok", Json::Bool(true)),
                        ("phxsql", Json::texto_de(VERSAO)),
                    ]),
                );
            }
            ("POST", "/api") => self.api_http(&mut fluxo, &pedido, &ip, porta),
            ("GET", _) | ("HEAD", _) => {
                let _ = http::erro_json(&mut fluxo, 404, "esta interface tem tres rotas: /, /saude e /api");
            }
            _ => {
                let _ = http::erro_json(&mut fluxo, 405, "use GET / ou POST /api");
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
    fn api_http(&self, fluxo: &mut TcpStream, pedido: &http::Pedido, ip: &str, porta: u16) {
        let duracao = self.config.web.sessao_ms();
        let agora = crate::agora_ms();
        let id_pedido = pedido.cabecalho("x-sessao").unwrap_or("").trim().to_string();

        // Reconstroi, a partir da sessao, o mesmo estado que a conexao TCP
        // teria: quem esta logado e que desafio esta em aberto.
        let mut sessao = Sessao::default();
        let mut id_sessao = String::new();
        if !id_pedido.is_empty() {
            if let Ok(mut vivas) = self.sessoes.lock() {
                if let Some(login) = vivas.usar(&id_pedido, duracao, agora) {
                    id_sessao = id_pedido.clone();
                    sessao.desafio = vivas.tomar_desafio(&id_pedido);
                    if !login.is_empty() {
                        sessao.usuario = self
                            .config
                            .cadastro
                            .por_login(&login)
                            .filter(|u| u.ativo)
                            .cloned();
                    }
                }
            }
        }

        let inicio = Instant::now();
        let quando_ms = crate::agora_ms();
        let (op, autenticado, resultado) = self.despachar(&pedido.corpo, &mut sessao, ip);
        let ms = inicio.elapsed().as_millis() as u64;

        // Depois do despacho, acerta a sessao conforme o que aconteceu.
        if resultado.is_ok() {
            match op.as_str() {
                "desafio" => {
                    if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone())
                    {
                        // O desafio vem antes da identidade: a sessao nasce
                        // anonima so para carregar o nonce ate o login.
                        if id_sessao.is_empty() {
                            id_sessao = vivas.nova("", duracao, agora);
                        }
                        vivas.guardar_desafio(&id_sessao, d);
                    }
                }
                "login" => {
                    if let Ok(mut vivas) = self.sessoes.lock() {
                        let login = sessao.login().to_string();
                        if id_sessao.is_empty() || !vivas.definir_login(&id_sessao, &login) {
                            id_sessao = vivas.nova(&login, duracao, agora);
                        }
                    }
                }
                "sair" => {
                    if let Ok(mut vivas) = self.sessoes.lock() {
                        vivas.encerrar(&id_sessao);
                    }
                    id_sessao.clear();
                }
                _ => {}
            }
        }

        let mut campos = match &resultado {
            Ok(valor) => vec![
                ("ok", Json::Bool(true)),
                ("op", Json::texto_de(&op)),
                ("resultado", valor.clone()),
                ("ms", Json::de_u64(ms)),
            ],
            Err(e) => vec![
                ("ok", Json::Bool(false)),
                ("op", Json::texto_de(&op)),
                ("erro", Json::texto_de(e.to_string())),
                ("ms", Json::de_u64(ms)),
            ],
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
            erro: resultado.as_ref().err().map(|e| e.to_string()),
        });

        let codigo = match &resultado {
            Ok(_) => 200,
            Err(PhxError::Autorizacao(_)) => 403,
            Err(PhxError::NaoEncontrado(_)) => 404,
            Err(_) => 400,
        };
        let _ = http::responder_json(fluxo, codigo, &Json::objeto(campos));
    }

'''
s = s.replace(ancora, bloco + ancora)
open(p,'w').write(s)
print("ok")
