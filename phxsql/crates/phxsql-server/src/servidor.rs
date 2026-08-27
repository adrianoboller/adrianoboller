//! Servidor TCP do PhxSql.
//!
//! Protocolo JSON Lines: uma linha JSON por pedido, uma linha JSON por
//! resposta, UTF-8, terminadas em `\n`. A conexao aceita varios pedidos
//! seguidos e cada um vira uma entrada no log de acessos.
//!
//! ```text
//! -> {"token":"...","op":"ping"}
//! <- {"ok":true,"op":"ping","resultado":{"phxsql":"0.1.0"},"ms":0}
//! ```
//!
//! # Concorrencia
//!
//! O motor de armazenamento ainda nao tem travas de arquivo nem de registro,
//! entao TODO acesso a dados passa por um mutex unico: as conexoes sao
//! aceitas em paralelo, mas as operacoes se enfileiram. E lento sob carga e e
//! correto -- o contrario seria rapido e corrompido. Travas finas entram junto
//! com as transacoes.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Table;

use crate::acesso::{Acesso, LogAcessos};
use crate::blacklist::Blacklist;
use crate::config::Config;
use crate::http;
use crate::usuarios::{Atividade, Usuario};
use crate::valores::{json_para_chave, json_para_linha, linha_para_json};

pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Operacoes que alteram dados. Recusadas quando `somente_leitura` esta ligado.
const OPS_ESCRITA: &[&str] = &[
    "inserir",
    "atualizar",
    "excluir",
    "reindexar",
    "criar_database",
    "criar_schema",
];

/// Estado de uma conexao.
///
/// A senha e conferida com PBKDF2, que custa da ordem de 100 ms de proposito.
/// Fazer isso a cada pedido inviabilizaria o servidor, entao a autenticacao
/// acontece UMA VEZ por conexao e o resultado fica aqui.
#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
    /// Desafio em aberto: (usuario, nonce do servidor, quando expira).
    /// Vale uma vez so -- e consumido no login, dando certo ou errado.
    desafio: Option<(String, String, i64)>,
}

impl Sessao {
    fn login(&self) -> &str {
        self.usuario
            .as_ref()
            .map(|u| u.login.as_str())
            .unwrap_or("")
    }

    /// Id gravado no `.log` da tabela como autor da operacao.
    /// Zero quando a conexao veio pelo token de servico, sem login.
    fn id(&self) -> u32 {
        self.usuario.as_ref().map(|u| u.id).unwrap_or(0)
    }
}

pub struct Servidor {
    config: Config,
    /// Trava unica de dados. Ver a nota de concorrencia no topo do modulo.
    dados: Mutex<Instancia>,
    log: Mutex<LogAcessos>,
    lista_negra: Mutex<Blacklist>,
    /// Sessoes do navegador. Vazio enquanto a interface web estiver desligada.
    sessoes: Mutex<http::Sessoes>,
    conexoes: AtomicUsize,
}

impl Servidor {
    pub fn novo(config: Config) -> Result<Arc<Servidor>> {
        let instancia = Instancia::nova(&config.base)?;
        let log = LogAcessos::abrir(&config.log_acessos)?;
        let lista_negra = Blacklist::abrir(&config.blacklist)?;
        Ok(Arc::new(Servidor {
            config,
            dados: Mutex::new(instancia),
            log: Mutex::new(log),
            lista_negra: Mutex::new(lista_negra),
            sessoes: Mutex::new(http::Sessoes::default()),
            conexoes: AtomicUsize::new(0),
        }))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Sobe o servidor e atende ate o processo ser encerrado.
    pub fn escutar(self: &Arc<Self>) -> Result<()> {
        let endereco = self.config.endereco()?;
        let ouvinte = TcpListener::bind(endereco)
            .map_err(|e| PhxError::Esquema(format!("nao consegui escutar em {endereco}: {e}")))?;
        eprintln!(
            "PhxSql {VERSAO} escutando em {endereco} | base {} | papel {}",
            self.config.base.display(),
            self.config.replicacao.papel.nome()
        );
        eprintln!("log de acessos: {}", self.config.log_acessos.display());
        eprintln!("lista de bloqueio: {}", self.config.blacklist.display());
        if !self.config.politica.comandos_proibidos.is_empty() {
            eprintln!(
                "comandos proibidos: {}",
                self.config.politica.comandos_proibidos.join(", ")
            );
        }
        if let Some(fw) = &self.config.politica.firewall {
            eprintln!(
                "firewall: {}",
                if fw.ligado {
                    "ligado -- IP bloqueado vira regra no sistema"
                } else {
                    "desligado -- o bloqueio vale so dentro do servidor"
                }
            );
        }

        self.subir_web();

        for conexao in ouvinte.incoming() {
            match conexao {
                Ok(fluxo) => {
                    let par = fluxo.peer_addr().ok();
                    if self.conexoes.load(Ordering::SeqCst) >= self.config.conexoes_max {
                        // Recusa sem derrubar o servico, e deixa registro.
                        if let Some(p) = par {
                            self.anotar(&Acesso {
                                quando_ms: crate::agora_ms(),
                                ip: p.ip().to_string(),
                                porta_origem: p.port(),
                                op: "conexao".into(),
                                usuario: String::new(),
                                autenticado: false,
                                ok: false,
                                duracao_ms: 0,
                                erro: Some("limite de conexoes atingido".into()),
                            });
                        }
                        continue;
                    }
                    let servidor = Arc::clone(self);
                    self.conexoes.fetch_add(1, Ordering::SeqCst);
                    std::thread::spawn(move || {
                        let endereco = par.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
                        servidor.atender(fluxo, endereco);
                        servidor.conexoes.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) => eprintln!("conexao recusada pelo sistema: {e}"),
            }
        }
        Ok(())
    }

    /// Violacao grave: bloqueia na hora e avisa no log.
    fn violacao_grave(&self, ip: &str, comando: &str, motivo: &str) {
        if let Ok(mut lista) = self.lista_negra.lock() {
            let (b, aviso) = lista.violacao_grave(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            );
            eprintln!(
                "BLOQUEADO {ip} ate {} -- {} ({})",
                b.ate(),
                b.motivo,
                b.comando
            );
            if let Some(a) = aviso {
                eprintln!("AVISO: {a}");
            }
        }
    }

    /// Tentativa leve: conta, e bloqueia se passar do limite na janela.
    fn violacao_leve(&self, ip: &str, comando: &str, motivo: &str) {
        if let Ok(mut lista) = self.lista_negra.lock() {
            if let Some((b, aviso)) = lista.tentativa_leve(
                ip,
                comando,
                motivo,
                &self.config.politica,
                crate::agora_ms(),
            ) {
                eprintln!(
                    "BLOQUEADO {ip} ate {} -- {} apos {} tentativas",
                    b.ate(),
                    b.motivo,
                    b.tentativas
                );
                if let Some(a) = aviso {
                    eprintln!("AVISO: {a}");
                }
            }
        }
    }

    fn anotar(&self, acesso: &Acesso) {
        if let Ok(mut log) = self.log.lock() {
            if let Err(e) = log.registrar(acesso) {
                eprintln!("falha ao gravar o log de acessos: {e}");
            }
        }
    }

    /// Este IP esta barrado agora? Devolve o motivo, ja formatado para o log.
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
                let _ = http::responder(
                    &mut fluxo,
                    200,
                    "text/html; charset=utf-8",
                    &http::montar_pagina(),
                );
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
                let _ = http::erro_json(
                    &mut fluxo,
                    404,
                    "esta interface tem tres rotas: /, /saude e /api",
                );
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
        let id_pedido = pedido
            .cabecalho("x-sessao")
            .unwrap_or("")
            .trim()
            .to_string();

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

        // Um desafio em aberto so e consumido por um login. Qualquer outra
        // operacao no meio do caminho devolve o nonce para a sessao, senao um
        // "ping" entre o desafio e o login derrubaria a prova.
        if op != "login" && op != "desafio" {
            if let (Ok(mut vivas), Some(d)) = (self.sessoes.lock(), sessao.desafio.clone()) {
                vivas.guardar_desafio(&id_sessao, d);
            }
        }

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

    fn atender(&self, fluxo: TcpStream, par: SocketAddr) {
        let ip = par.ip().to_string();
        let porta = par.port();
        let _ = fluxo.set_read_timeout(Some(Duration::from_secs(self.config.timeout_s)));

        // Antes de qualquer coisa: quem esta na lista de bloqueio nao entra.
        let agora = crate::agora_ms();
        if let Some(motivo) = self.barrado(&ip, agora) {
            self.anotar(&Acesso {
                quando_ms: agora,
                ip: ip.clone(),
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some(motivo.clone()),
            });
            let escrita = fluxo.try_clone();
            if let Ok(mut saida) = escrita {
                let _ = writeln!(saida, "{}", resposta_erro("conexao", &motivo, 0).escrever());
            }
            return;
        }

        let permitido = self.config.ip_permitido(&ip);
        let escrita = fluxo.try_clone();
        let mut leitor = BufReader::new(fluxo);
        let mut saida = match escrita {
            Ok(f) => f,
            Err(_) => return,
        };

        if !permitido {
            self.violacao_leve(&ip, "conexao", "ip fora da lista de permitidos");
            self.anotar(&Acesso {
                quando_ms: crate::agora_ms(),
                ip,
                porta_origem: porta,
                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),
            });
            let _ = writeln!(
                saida,
                "{}",
                resposta_erro("conexao", "ip nao autorizado", 0).escrever()
            );
            return;
        }

        let mut sessao = Sessao::default();
        let mut linha = String::new();
        loop {
            linha.clear();
            match leitor.read_line(&mut linha) {
                Ok(0) => return, // conexao fechada
                Ok(_) => {}
                Err(_) => return,
            }
            if linha.trim().is_empty() {
                continue;
            }

            let inicio = Instant::now();
            let quando_ms = crate::agora_ms();
            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao, &ip);
            let duracao = inicio.elapsed().as_millis() as u64;

            let resposta = match &resultado {
                Ok(valor) => Json::objeto(vec![
                    ("ok", Json::Bool(true)),
                    ("op", Json::texto_de(&op)),
                    ("resultado", valor.clone()),
                    ("ms", Json::de_u64(duracao)),
                ]),
                Err(e) => resposta_erro(&op, &e.to_string(), duracao),
            };

            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                usuario: sessao.login().to_string(),
                autenticado,
                ok: resultado.is_ok(),
                duracao_ms: duracao,
                erro: resultado.as_ref().err().map(|e| e.to_string()),
            });

            if writeln!(saida, "{}", resposta.escrever()).is_err() {
                return;
            }
            let _ = saida.flush();
        }
    }

    /// Le o pedido e o leva pelos portoes, nesta ordem: politica (o que ninguem
    /// pode), token (a rede), login (a identidade) e permissao (o poder).
    fn despachar(
        &self,
        linha: &str,
        sessao: &mut Sessao,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() {
            "ping".to_string()
        } else {
            op
        };
        let base = pedido.texto_ou("database", "").to_string();

        // Portao 0 -- a politica. Vale para todo mundo, root inclusive: e o
        // que o config.json diz que ninguem pede por esta porta. Pedir vira
        // bloqueio na hora, sem contar tentativa.
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
        if self.config.politica.base_proibida(&base) {
            self.violacao_grave(ip, &op, "base proibida pela politica");
            return (
                op,
                false,
                Err(PhxError::Autorizacao(format!(
                    "a base {base} esta proibida neste servidor; o IP foi bloqueado"
                ))),
            );
        }

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            self.violacao_leve(ip, &op, "token invalido");
            return (
                op,
                false,
                Err(PhxError::Autorizacao("token invalido".into())),
            );
        }

        // Portao 2 -- o login.
        if op == "desafio" {
            let r = self.op_desafio(&pedido, sessao);
            return (op, true, r);
        }
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            if r.is_err() {
                self.violacao_leve(ip, "login", "credencial invalida");
            }
            return (op, r.is_ok(), r);
        }
        // Sair nao precisa de poder nenhum: e devolver o que se tem.
        if op == "sair" {
            sessao.usuario = None;
            sessao.desafio = None;
            return (op, true, Ok(Json::objeto(vec![("saiu", Json::Bool(true))])));
        }
        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()
            && Atividade::da_operacao(&op).is_some()
        {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}".into(),
                )),
            );
        }

        if self.config.somente_leitura && OPS_ESCRITA.contains(&op.as_str()) {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(
                    "servidor em modo somente leitura".into(),
                )),
            );
        }

        // Portao 3 -- o poder deste usuario sobre a base deste pedido.
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(&op), sessao.usuario.as_ref())
        {
            if !usuario.pode(&base, atividade) {
                return (
                    op,
                    true,
                    Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de {} em {}",
                        usuario.login,
                        atividade.nome(),
                        if base.is_empty() { "(sem base)" } else { &base }
                    ))),
                );
            }
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }

    /// Abre um desafio: devolve sal, iteracoes e um nonce de uso unico.
    ///
    /// Usuario que nao existe recebe um desafio de aparencia normal, com sal
    /// derivado do proprio login -- assim quem sonda nao descobre quem existe
    /// pela resposta.
    fn op_desafio(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\"".into()));
        }
        let (sal_hex, iteracoes) = match self.config.cadastro.por_login(&login) {
            Some(u) => {
                let (sal, it) = phxsql_core::senha::sal_e_iteracoes(&u.senha_hash)?;
                (phxsql_core::hash::para_hex(&sal), it)
            }
            None => {
                // Sal falso, estavel por login e indistinguivel de um real.
                let falso =
                    phxsql_core::hash::hmac_sha256(self.config.token.as_bytes(), login.as_bytes());
                (
                    phxsql_core::hash::para_hex(&falso[..16]),
                    phxsql_core::senha::ITERACOES_PADRAO,
                )
            }
        };

        let nonce = phxsql_core::desafio::nonce();
        sessao.desafio = Some((
            login,
            nonce.clone(),
            crate::agora_ms() + phxsql_core::desafio::VALIDADE_MS,
        ));
        Ok(Json::objeto(vec![
            ("sal", Json::texto_de(sal_hex)),
            ("iteracoes", Json::de_u64(iteracoes as u64)),
            ("nonce", Json::texto_de(nonce)),
            (
                "validade_ms",
                Json::de_i64(phxsql_core::desafio::VALIDADE_MS),
            ),
        ]))
    }

    /// Confere a credencial e guarda a identidade na conexao.
    ///
    /// Aceita tres formas, da mais segura para a menos:
    ///
    /// 1. `prova` + `nonce_cliente` -- desafio-resposta. A senha nao sai da
    ///    maquina do cliente.
    /// 2. `senha_b64` -- Base64. Some do grep e do olho, mas quem captura o
    ///    pacote decodifica: NAO e cifra.
    /// 3. `senha` -- texto puro.
    fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = match p.campo("usuario_b64").and_then(Json::texto) {
            Some(b) => phxsql_core::base64::decodificar_texto(b)?,
            None => p.texto_ou("usuario", p.texto_ou("login", "")).to_string(),
        };
        let login = login.trim().to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \"usuario\" e \"senha\"".into()));
        }

        // Todo caminho de erro devolve a MESMA mensagem, para nao dizer se o
        // que falhou foi o login, a senha ou o desafio.
        let recusa = || PhxError::Autorizacao("usuario ou senha invalidos".into());

        let autenticado = if let Some(prova) = p.campo("prova").and_then(Json::texto) {
            // (1) desafio-resposta
            let (usuario_desafio, nonce, expira) = sessao.desafio.take().ok_or_else(|| {
                PhxError::Autorizacao("peca um desafio antes de mandar a prova".into())
            })?;
            if crate::agora_ms() > expira {
                return Err(PhxError::Autorizacao(
                    "o desafio expirou; peca outro".into(),
                ));
            }
            if usuario_desafio != login {
                return Err(recusa());
            }
            let nonce_cliente = p.texto_ou("nonce_cliente", "");
            match self.config.cadastro.por_login(&login) {
                Some(u) if u.ativo => {
                    let dk = phxsql_core::senha::derivado_do_hash(&u.senha_hash)?;
                    phxsql_core::desafio::conferir_prova(&dk, &nonce, nonce_cliente, &login, prova)
                        .then_some(u)
                }
                _ => None,
            }
        } else {
            // (2) Base64 ou (3) texto puro
            let clara = match p.campo("senha_b64").and_then(Json::texto) {
                Some(b) => phxsql_core::base64::decodificar_texto(b)?,
                None => p.texto_ou("senha", "").to_string(),
            };
            self.config.cadastro.autenticar(&login, &clara)
        };

        match autenticado {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                Err(recusa())
            }
        }
    }

    fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {
        match op {
            "ping" => Ok(Json::objeto(vec![
                ("phxsql", Json::texto_de(VERSAO)),
                ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
                (
                    "conexoes",
                    Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                ),
            ])),
            "config" => Ok(self.config.para_json()),
            "quem_sou" => Ok(match &sessao.usuario {
                Some(u) => u.ficha(),
                None => Json::objeto(vec![
                    ("usuario", Json::Nulo),
                    ("via", Json::texto_de("token de servico")),
                ]),
            }),
            "usuarios" => Ok(self.config.cadastro.fichas()),
            "acessos" => self.op_acessos(p),
            "ips" => self.op_ips(),
            "bloqueios" => self.op_bloqueios(),
            "desbloquear" => self.op_desbloquear(p),
            "bancos" => self.op_bancos(),
            "tabelas" => self.op_tabelas(p),
            "esquema" => self.op_esquema(p, sessao),
            "criar_database" => self.op_criar_database(p),
            "ler" => self.op_ler(p, sessao),
            "varrer" => self.op_varrer(p, sessao),
            "buscar" => self.op_buscar(p, sessao),
            "inserir" => self.op_inserir(p, sessao),
            "atualizar" => self.op_atualizar(p, sessao),
            "excluir" => self.op_excluir(p, sessao),
            "diario" => self.op_diario(p, sessao),
            "verificar" => self.op_verificar(p, sessao),
            "reindexar" => self.op_reindexar(p, sessao),
            outro => Err(PhxError::NaoEncontrado(format!(
                "operacao desconhecida: {outro}"
            ))),
        }
    }

    // ------------------------------------------------------------ ajudantes

    fn abrir(&self, p: &Json, sessao: &Sessao) -> Result<Table> {
        let database = p.texto_ou("database", "");
        let tabela = p.texto_ou("tabela", "");
        if database.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"database\" e \"tabela\"".into(),
            ));
        }
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = dados.abrir_database(database)?.abrir_qualificada(tabela)?;
        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        Ok(t)
    }

    fn rowid(&self, p: &Json) -> Result<u64> {
        p.campo("rowid")
            .and_then(Json::inteiro)
            .filter(|n| *n > 0)
            .map(|n| n as u64)
            .ok_or_else(|| PhxError::Esquema("informe \"rowid\" maior que zero".into()))
    }

    fn limite(&self, p: &Json) -> u64 {
        let pedido = p.inteiro_ou("max", self.config.max_linhas as i64).max(0) as u64;
        if pedido == 0 {
            self.config.max_linhas
        } else {
            pedido.min(self.config.max_linhas)
        }
    }

    // ------------------------------------------------------------ operacoes

    fn op_acessos(&self, p: &Json) -> Result<Json> {
        let max = self.limite(p) as usize;
        let todos = LogAcessos::ler(&self.config.log_acessos)?;
        let total = todos.len();
        let recentes: Vec<Json> = todos
            .iter()
            .rev()
            .take(max)
            .map(|a| a.para_json())
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("acessos", Json::Lista(recentes)),
        ]))
    }

    fn op_ips(&self) -> Result<Json> {
        let resumo = LogAcessos::resumo_por_ip(&self.config.log_acessos)?;
        Ok(Json::Lista(
            resumo
                .iter()
                .map(|r| {
                    Json::objeto(vec![
                        ("ip", Json::texto_de(&r.ip)),
                        ("acessos", Json::de_u64(r.acessos)),
                        ("recusados", Json::de_u64(r.recusados)),
                        ("primeiro", Json::texto_de(r.primeiro())),
                        ("ultimo", Json::texto_de(r.ultimo())),
                    ])
                })
                .collect(),
        ))
    }

    fn op_bloqueios(&self) -> Result<Json> {
        let lista = self.lista_negra.lock().map_err(|_| trava_envenenada())?;
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            (
                "arquivo",
                Json::texto_de(lista.caminho().display().to_string()),
            ),
            (
                "ativos",
                Json::Lista(
                    lista
                        .ativos(agora)
                        .into_iter()
                        .map(|b| b.para_json())
                        .collect(),
                ),
            ),
        ]))
    }

    fn op_desbloquear(&self, p: &Json) -> Result<Json> {
        let ip = p.texto_ou("ip", "").trim().to_string();
        if ip.is_empty() {
            return Err(PhxError::Esquema("informe \"ip\"".into()));
        }
        let mut lista = self.lista_negra.lock().map_err(|_| trava_envenenada())?;
        let tinha = lista.desbloquear(&ip, &self.config.politica)?;
        Ok(Json::objeto(vec![
            ("ip", Json::texto_de(&ip)),
            ("estava_bloqueado", Json::Bool(tinha)),
        ]))
    }

    fn op_bancos(&self) -> Result<Json> {
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        Ok(Json::Lista(
            dados.databases()?.into_iter().map(Json::texto_de).collect(),
        ))
    }

    fn op_tabelas(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.abrir_database(nome)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(nome)),
            (
                "schemas",
                Json::Lista(db.schemas()?.into_iter().map(Json::texto_de).collect()),
            ),
            (
                "tabelas",
                Json::Lista(
                    db.todas_as_tabelas()?
                        .into_iter()
                        .map(Json::texto_de)
                        .collect(),
                ),
            ),
        ]))
    }

    fn op_criar_database(&self, p: &Json) -> Result<Json> {
        let nome = p.texto_ou("database", "");
        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let db = dados.criar_database(nome)?;
        Ok(Json::objeto(vec![
            ("database", Json::texto_de(db.nome())),
            (
                "caminho",
                Json::texto_de(db.caminho().display().to_string()),
            ),
        ]))
    }

    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let t = self.abrir(p, sessao)?;
        let e = t.esquema();
        let colunas: Vec<Json> = e
            .colunas()
            .iter()
            .map(|c| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                ])
            })
            .collect();
        let indices: Vec<Json> = e
            .indices()
            .iter()
            .map(|i| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&i.nome)),
                    ("unico", Json::Bool(i.unico)),
                    (
                        "colunas",
                        Json::Lista(
                            i.colunas
                                .iter()
                                .map(|ic| {
                                    Json::objeto(vec![
                                        ("coluna", Json::texto_de(&e.colunas()[ic.coluna].nome)),
                                        ("desc", Json::Bool(ic.desc)),
                                        ("nocase", Json::Bool(ic.nocase)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let fks: Vec<Json> = e
            .chaves_estrangeiras()
            .iter()
            .map(|fk| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&fk.nome)),
                    (
                        "colunas",
                        Json::Lista(
                            fk.colunas
                                .iter()
                                .map(|c| Json::texto_de(&e.colunas()[*c].nome))
                                .collect(),
                        ),
                    ),
                    ("tabela_ref", Json::texto_de(&fk.tabela_ref)),
                    (
                        "colunas_ref",
                        Json::Lista(fk.colunas_ref.iter().map(Json::texto_de).collect()),
                    ),
                    ("ao_excluir", Json::texto_de(format!("{:?}", fk.ao_excluir))),
                    ("ao_alterar", Json::texto_de(format!("{:?}", fk.ao_alterar))),
                ])
            })
            .collect();
        let pag = e.paginacao();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(e.nome())),
            ("registros", Json::de_u64(t.registros())),
            ("slots", Json::de_u64(t.slots())),
            ("colunas", Json::Lista(colunas)),
            ("indices", Json::Lista(indices)),
            ("chaves_estrangeiras", Json::Lista(fks)),
            (
                "paginacao",
                if pag.ligada() {
                    Json::objeto(vec![
                        (
                            "registros_por_arquivo",
                            Json::de_u64(pag.registros_por_arquivo),
                        ),
                        ("max_arquivos", Json::de_u64(pag.max_arquivos as u64)),
                        ("capacidade", Json::de_u64(pag.capacidade())),
                    ])
                } else {
                    Json::Nulo
                },
            ),
        ]))
    }

    fn op_ler(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        match t.ler(rowid)? {
            None => Ok(Json::Nulo),
            Some(linha) => Ok(linha_para_json(&linha, t.esquema())),
        }
    }

    fn op_varrer(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p);
        let indice = p.texto_ou("indice", "").to_string();
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;

        let rowids: Vec<u64> = if indice.is_empty() {
            t.varrer()?.into_iter().map(|(r, _)| r).collect()
        } else {
            t.varrer_indice(&indice)?
        };
        let total = rowids.len();
        let mut linhas = Vec::new();
        for rowid in rowids.into_iter().take(max as usize) {
            if let Some(l) = t.ler(rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("devolvidas", Json::de_u64(linhas.len() as u64)),
            (
                "ordem",
                Json::texto_de(if indice.is_empty() {
                    "digitacao".to_string()
                } else {
                    format!("indice {indice}")
                }),
            ),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    fn op_buscar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let indice = p.texto_ou("indice", "").to_string();
        if indice.is_empty() {
            return Err(PhxError::Esquema("informe \"indice\"".into()));
        }
        let chave_json = p
            .campo("chave")
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"chave\"".into()))?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let pos = t
            .esquema()
            .indice_por_nome(&indice)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {indice} nao existe")))?;
        let chave = json_para_chave(&chave_json, t.esquema(), pos)?;
        let rowids = t.buscar(&indice, &chave)?;

        let mut linhas = Vec::new();
        for rowid in rowids.iter().take(self.limite(p) as usize) {
            if let Some(l) = t.ler(*rowid)? {
                let mut obj = vec![("rowid".to_string(), Json::de_u64(*rowid))];
                if let Json::Objeto(pares) = linha_para_json(&l, t.esquema()) {
                    obj.extend(pares);
                }
                linhas.push(Json::Objeto(obj));
            }
        }
        Ok(Json::objeto(vec![
            ("encontrados", Json::de_u64(rowids.len() as u64)),
            ("linhas", Json::Lista(linhas)),
        ]))
    }

    fn op_inserir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let linha = json_para_linha(&valores_json, t.esquema())?;
        let rowid = t.inserir(&linha)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("registros", Json::de_u64(t.registros())),
        ]))
    }

    fn op_atualizar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let valores_json = p
            .campo("valores")
            .or_else(|| p.campo("linha"))
            .cloned()
            .ok_or_else(|| PhxError::Esquema("informe \"valores\"".into()))?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let linha = json_para_linha(&valores_json, t.esquema())?;
        t.atualizar(rowid, &linha)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))
    }

    fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let removeu = t.excluir(rowid)?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(removeu)),
        ]))
    }

    fn op_diario(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let max = self.limite(p) as usize;
        let rowid = p.campo("rowid").and_then(Json::inteiro).map(|n| n as u64);
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let eventos = match rowid {
            Some(r) => t.historico(r)?,
            None => t.diario(0, 0)?,
        };
        let total = eventos.len();
        let recentes: Vec<Json> = eventos
            .iter()
            .rev()
            .take(max)
            .rev()
            .map(|e| {
                Json::objeto(vec![
                    ("quando", Json::texto_de(e.instante_iso())),
                    ("carimbo_ms", Json::Numero(e.carimbo as f64)),
                    ("operacao", Json::texto_de(e.operacao.nome())),
                    ("rowid", Json::de_u64(e.rowid)),
                    ("versao", Json::de_u64(e.versao)),
                    ("usuario", Json::de_u64(e.usuario as u64)),
                ])
            })
            .collect();
        Ok(Json::objeto(vec![
            ("total", Json::de_u64(total as u64)),
            ("eventos", Json::Lista(recentes)),
        ]))
    }

    fn op_verificar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let r = t.verificar()?;
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&r.tabela)),
            ("registros", Json::de_u64(r.registros)),
            ("slots", Json::de_u64(r.slots)),
            ("eventos", Json::de_u64(r.eventos)),
            (
                "indices",
                Json::Objeto(
                    r.indices
                        .iter()
                        .map(|(n, q)| (n.clone(), Json::de_u64(*q)))
                        .collect(),
                ),
            ),
            (
                "volumes",
                Json::objeto(vec![
                    ("reg", Json::de_u64(r.volumes.0 as u64)),
                    ("bin", Json::de_u64(r.volumes.1 as u64)),
                    ("memo", Json::de_u64(r.volumes.2 as u64)),
                    ("log", Json::de_u64(r.volumes.3 as u64)),
                ]),
            ),
        ]))
    }

    fn op_reindexar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let indices = t.reindexar()?;
        t.sincronizar()?;
        Ok(Json::Objeto(
            indices
                .into_iter()
                .map(|(n, q)| (n, Json::de_u64(q)))
                .collect(),
        ))
    }
}

fn trava_envenenada() -> PhxError {
    PhxError::Corrompido("uma operacao anterior entrou em panico e deixou a trava suja".into())
}

fn resposta_erro(op: &str, mensagem: &str, ms: u64) -> Json {
    Json::objeto(vec![
        ("ok", Json::Bool(false)),
        ("op", Json::texto_de(op)),
        ("erro", Json::texto_de(mensagem)),
        ("ms", Json::de_u64(ms)),
    ])
}
