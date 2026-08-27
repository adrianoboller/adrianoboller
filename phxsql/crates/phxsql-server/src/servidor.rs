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

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_store::catalogo::Instancia;
use phxsql_store::memoria::{Consulta, Filtro, Operador, Ordem, TabelaMemoria};
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

/// Uma conexao viva para outro PhxSql, do lado de ca da interface.
pub struct Remoto {
    pub destino: String,
    leitor: BufReader<TcpStream>,
    escrita: TcpStream,
}

impl Remoto {
    /// Abre a conexao. Nao autentica -- quem autentica e o pedido de login,
    /// que segue por aqui igual a qualquer outro.
    pub fn abrir(destino: &str, timeout_s: u64) -> Result<Remoto> {
        use std::net::ToSocketAddrs;
        let endereco = destino
            .to_socket_addrs()
            .map_err(|e| PhxError::Esquema(format!("destino {destino:?} nao resolve: {e}")))?
            .next()
            .ok_or_else(|| PhxError::Esquema(format!("destino {destino:?} sem endereco")))?;
        let fluxo =
            TcpStream::connect_timeout(&endereco, Duration::from_secs(timeout_s.min(10)))
                .map_err(|e| PhxError::Esquema(format!("nao consegui falar com {destino}: {e}")))?;
        fluxo.set_read_timeout(Some(Duration::from_secs(timeout_s)))?;
        let escrita = fluxo.try_clone()?;
        Ok(Remoto {
            destino: destino.to_string(),
            leitor: BufReader::new(fluxo),
            escrita,
        })
    }

    /// Manda uma linha e devolve a resposta, crua.
    ///
    /// Crua de proposito: o que o servidor remoto respondeu e o que o
    /// navegador recebe. Reescrever no meio do caminho seria mentir sobre
    /// quem respondeu o que.
    pub fn conversar(&mut self, linha: &str) -> Result<Json> {
        let limpa = linha.replace(['\n', '\r'], " ");
        writeln!(self.escrita, "{limpa}")?;
        self.escrita.flush()?;
        let mut resposta = String::new();
        if self.leitor.read_line(&mut resposta)? == 0 {
            return Err(PhxError::Esquema(format!(
                "{} fechou a conexao",
                self.destino
            )));
        }
        Json::analisar(&resposta)
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
    /// Tabelas residentes em RAM, por "database/tabela". Nada entra aqui
    /// sozinho: so o que alguem pediu para carregar.
    residentes: Mutex<HashMap<String, TabelaMemoria>>,
    /// Conexoes abertas para outros PhxSql, uma por sessao do navegador.
    ///
    /// Ficam abertas de proposito: o protocolo da porta 5000 autentica uma vez
    /// por CONEXAO, entao manter o soquete e o que faz o PBKDF2 do servidor
    /// remoto rodar uma vez por login e nao a cada clique.
    remotos: Mutex<HashMap<String, Arc<Mutex<Remoto>>>>,
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
            residentes: Mutex::new(HashMap::new()),
            remotos: Mutex::new(HashMap::new()),
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
        if self.config.replicacao.papel != crate::config::Papel::Isolado {
            let portas = self.config.replicacao.portas();
            eprintln!(
                "replicacao: papel {} | {}",
                self.config.replicacao.papel.nome(),
                if portas.is_empty() {
                    "envio e retorno pela porta de dados".to_string()
                } else {
                    portas
                        .iter()
                        .map(|(k, v)| format!("{k} {v}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                }
            );
            eprintln!(
                "ATENCAO: o transporte de eventos ainda nao esta implementado \
                 (ver docs/REPLICACAO.md). As portas sao configuracao, nao servico."
            );
        }

        self.subir_web();
        self.subir_backup_agendado();

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

    /// Sobe o relogio do backup agendado, se ligado.
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
                let pasta = b.destino.join(
                    phxsql_core::datahora::instante_iso(quando).replace([' ', ':', ','], "-"),
                );
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
                        (
                            "porta_dados",
                            Json::de_u64(
                                self.config.endereco().map(|e| e.port()).unwrap_or(0) as u64
                            ),
                        ),
                        (
                            "servidores",
                            Json::Lista(
                                self.config
                                    .web
                                    .servidores
                                    .iter()
                                    .map(Json::texto_de)
                                    .collect(),
                            ),
                        ),
                        (
                            "exige_chave",
                            Json::Bool(self.config.cadastro.alguem_exige_chave()),
                        ),
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

    /// Abre uma conexao para outro PhxSql e manda o login por ela.
    ///
    /// A politica DESTE servidor vale antes de qualquer coisa sair daqui:
    /// comando proibido aqui nao vira pedido la. A interface nao e uma porta
    /// dos fundos para o que a porta da frente recusa.
    #[allow(clippy::type_complexity)]
    fn abrir_remoto(
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
        let mut r = match conexao.lock() {
            Ok(r) => r,
            Err(_) => return (op, false, Err(trava_envenenada())),
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

        let ja_remota = self
            .remotos
            .lock()
            .ok()
            .and_then(|r| r.get(&id_sessao).cloned());

        let (op, autenticado, resultado) = match (&ja_remota, servidor_remoto.is_empty()) {
            // Sessao ja amarrada a um servidor remoto: tudo vai para la.
            (Some(conexao), _) => self.encaminhar(conexao, &pedido.corpo, ip),
            // Login novo pedindo servidor: abre, encaminha, e guarda se entrou.
            (None, false) => {
                let r = self.abrir_remoto(&servidor_remoto, &pedido.corpo, ip);
                match r {
                    Ok((op, valor, conexao)) => {
                        if id_sessao.is_empty() {
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
            (None, true) => self.despachar(&pedido.corpo, &mut sessao, ip),
        };
        let remota = ja_remota.is_some() || !servidor_remoto.is_empty();
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
        if resultado.is_ok() && !remota {
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
                    if let Ok(mut r) = self.remotos.lock() {
                        r.remove(&id_sessao);
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
        // Nome com ".." ou barra nao e engano de digitacao: e sondagem de
        // travessia de diretorio. O motor ja recusava -- mas recusava calado, e
        // quem sonda podia tentar a noite inteira sem nunca ser barrado. Agora
        // e violacao grave, igual a comando proibido: bloqueia na primeira.
        for (rotulo, valor) in [
            ("database", &base),
            ("tabela", &pedido.texto_ou("tabela", "").to_string()),
            ("schema", &pedido.texto_ou("schema", "").to_string()),
        ] {
            if !valor.is_empty() && phxsql_store::catalogo::nome_hostil(valor) {
                self.violacao_grave(ip, &op, "tentativa de travessia de diretorio");
                return (
                    op,
                    false,
                    Err(PhxError::Autorizacao(format!(
                        "{rotulo} {valor:?} nao e um nome; o IP foi bloqueado"
                    ))),
                );
            }
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

        let mut nonces: Option<(String, String)> = None;
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
            nonces = Some((nonce.clone(), nonce_cliente.to_string()));
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

        // Segundo fator: quem tem chave publica no config.json tambem assina.
        //
        // A mensagem assinada e a MESMA do desafio-resposta -- os dois nonces
        // e o login --, entao a assinatura tambem vale uma vez so. Nao ha
        // atalho: sem desafio aberto nao ha o que assinar.
        if let Some(u) = &autenticado {
            if let Some(publica) = &u.chave_publica {
                let (nonce, nonce_cliente) =
                    match &nonces {
                        Some(par) => par.clone(),
                        None => return Err(PhxError::Autorizacao(
                            "este usuario exige chave: peca um desafio e mande a prova assinada"
                                .into(),
                        )),
                    };
                let hex = p.texto_ou("assinatura", "");
                let assinatura = phxsql_core::ed25519::assinatura_de_hex(hex).ok_or_else(|| {
                    PhxError::Autorizacao(
                        "este usuario exige \"assinatura\" com 128 hexadecimais".into(),
                    )
                })?;
                let mensagem =
                    phxsql_core::desafio::mensagem_assinada(&nonce, &nonce_cliente, &login);
                if !phxsql_core::ed25519::conferir(publica, &mensagem, &assinatura) {
                    return Err(recusa());
                }
            }
        }

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
            "memoria_carregar" => self.op_memoria_carregar(p, sessao),
            "memoria_liberar" => self.op_memoria_liberar(p),
            "memoria" => self.op_memoria(),
            "painel" => self.op_painel(sessao),
            "backup" => self.op_backup(p, sessao),
            "reparar" => self.op_reparar(p, sessao),
            "conferir_backup" => self.op_conferir_backup(p),
            // O nome que o Adriano pediu, e o nome em portugues do projeto.
            // Sao a mesma operacao: a interface usa um, o script usa o outro.
            "SelectMemory" | "selectmemory" | "selecionar_memoria" => {
                self.op_selecionar_memoria(p, sessao)
            }
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
        // O espelho e decisao do servidor, nao da tabela: ligar no config.json
        // vale para tudo que este servidor abrir daqui para a frente.
        if self.config.espelho && !t.tem_espelho() {
            t.espelhar()?;
        }
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
        // A copia em RAM acompanha DENTRO da mesma trava: nao existe instante
        // em que o disco e a memoria discordem.
        self.residente_mut(p, |m| m.anotar_insercao(rowid, &linha));
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
        self.residente_mut(p, |m| m.anotar_alteracao(rowid, &linha));
        Ok(Json::objeto(vec![("rowid", Json::de_u64(rowid))]))
    }

    fn op_excluir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let rowid = self.rowid(p)?;
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let removeu = t.excluir(rowid)?;
        t.sincronizar()?;
        if removeu {
            self.residente_mut(p, |m| m.anotar_exclusao(rowid));
        }
        Ok(Json::objeto(vec![
            ("rowid", Json::de_u64(rowid)),
            ("excluido", Json::Bool(removeu)),
        ]))
    }

    /// Copia de seguranca, com a trava de dados segurada do inicio ao fim.
    ///
    /// `"zip": true` faz um arquivo unico chamado
    /// `Banco_Admin_Data_HoraMin.zip`, com o manifesto dentro. Sem isso,
    /// copia a arvore de diretorios como antes.
    fn op_backup(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \"destino\"".into()));
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
    }

    /// Confere `.reg` contra `.bkp` e conserta o que der.
    fn op_reparar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
        let (conferidos, reparados, perdidos) = t.reparar()?;
        t.sincronizar()?;
        Ok(Json::objeto(vec![
            ("conferidos", Json::de_u64(conferidos)),
            ("reparados", Json::de_u64(reparados)),
            ("perdidos", Json::de_u64(perdidos)),
            ("integro", Json::Bool(perdidos == 0)),
        ]))
    }

    fn op_conferir_backup(&self, p: &Json) -> Result<Json> {
        let destino = p.texto_ou("destino", "").trim().to_string();
        if destino.is_empty() {
            return Err(PhxError::Esquema("informe \"destino\"".into()));
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

    // -------------------------------------------------------------- o painel

    /// Tudo que o painel mostra, numa chamada so.
    ///
    /// Poderia ser dez chamadas do navegador, e o painel ficaria dez vezes
    /// mais lento por causa da ida e volta. Agregar aqui tambem deixa a conta
    /// do que o usuario PODE VER acontecer de um lado so: o painel nunca
    /// mostra numero de base que quem esta olhando nao poderia abrir.
    fn op_painel(&self, sessao: &Sessao) -> Result<Json> {
        let agora = crate::agora_ms();

        // ---------------------------------------------------------- bancos
        let (mut bancos, mut tabelas_total, mut registros_total, mut bytes_total) =
            (Vec::new(), 0u64, 0u64, 0u64);
        let mut maiores: Vec<(String, u64, u64)> = Vec::new();
        {
            let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
            for nome in dados.databases()? {
                // O painel so conta o que quem esta olhando poderia abrir.
                if let Some(u) = &sessao.usuario {
                    if !u.pode(&nome, Atividade::Ler) {
                        continue;
                    }
                }
                let db = dados.abrir_database(&nome)?;
                let lista = db.todas_as_tabelas()?;
                let schemas = db.schemas()?.len() as u64;
                let mut registros_db = 0u64;
                for t in &lista {
                    if let Ok(tab) = db.abrir_qualificada(t) {
                        let regs = tab.registros();
                        registros_db += regs;
                        let bytes: u64 = tab
                            .volumes_por_arquivo()
                            .0
                            .iter()
                            .map(|v| {
                                std::fs::metadata(tab.diretorio().join(format!(
                                    "{}{}.reg",
                                    t.rsplit('.').next().unwrap_or(t),
                                    if *v == 1 {
                                        String::new()
                                    } else {
                                        format!("_{v:03}")
                                    }
                                )))
                                .map(|m| m.len())
                                .unwrap_or(0)
                            })
                            .sum();
                        bytes_total += bytes;
                        maiores.push((format!("{nome}/{t}"), regs, bytes));
                    }
                }
                tabelas_total += lista.len() as u64;
                registros_total += registros_db;
                bancos.push(Json::objeto(vec![
                    ("nome", Json::texto_de(&nome)),
                    ("tabelas", Json::de_u64(lista.len() as u64)),
                    ("schemas", Json::de_u64(schemas)),
                    ("registros", Json::de_u64(registros_db)),
                ]));
            }
        }
        // As dez maiores, por registro. Mais que isso vira lista, nao grafico.
        maiores.sort_by(|a, b| b.1.cmp(&a.1));
        maiores.truncate(10);

        // --------------------------------------------------------- acessos
        //
        // Uma passada so sobre o log, alimentando todas as contagens de uma
        // vez. Ler o arquivo cinco vezes para responder cinco perguntas seria
        // o painel ficando lento com o log crescendo.
        let acessos = LogAcessos::ler(&self.config.log_acessos).unwrap_or_default();
        let dia_ms = 86_400_000i64;
        let desde = agora - dia_ms;
        let mut por_hora = [0u64; 24];
        let mut recusadas_por_hora = [0u64; 24];
        let mut por_op: HashMap<String, (u64, u64)> = HashMap::new();
        let mut por_usuario: HashMap<String, u64> = HashMap::new();
        let (mut ok, mut falhas, mut soma_ms) = (0u64, 0u64, 0u64);
        for a in &acessos {
            if a.ok {
                ok += 1;
            } else {
                falhas += 1;
            }
            soma_ms += a.duracao_ms;
            let e = por_op.entry(a.op.clone()).or_insert((0, 0));
            if a.ok {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
            if !a.usuario.is_empty() {
                *por_usuario.entry(a.usuario.clone()).or_insert(0) += 1;
            }
            if a.quando_ms >= desde {
                // Balde por hora, contando de tras para frente a partir de
                // agora: o balde 23 e a hora corrente.
                let atras = ((agora - a.quando_ms) / 3_600_000) as usize;
                if atras < 24 {
                    let i = 23 - atras;
                    por_hora[i] += 1;
                    if !a.ok {
                        recusadas_por_hora[i] += 1;
                    }
                }
            }
        }
        let mut ops: Vec<(String, u64, u64)> =
            por_op.into_iter().map(|(k, (a, b))| (k, a, b)).collect();
        ops.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
        ops.truncate(12);
        let mut usuarios_ativos: Vec<(String, u64)> = por_usuario.into_iter().collect();
        usuarios_ativos.sort_by(|a, b| b.1.cmp(&a.1));
        usuarios_ativos.truncate(8);

        let ips = LogAcessos::resumo_por_ip(&self.config.log_acessos).unwrap_or_default();
        let mut top_ips: Vec<&crate::acesso::ResumoIp> = ips.iter().collect();
        top_ips.sort_by(|a, b| b.acessos.cmp(&a.acessos));
        top_ips.truncate(8);

        // -------------------------------------------------------- usuarios
        let cadastro = &self.config.cadastro;
        let mut por_nivel: HashMap<&'static str, u64> = HashMap::new();
        for u in cadastro.root.iter().chain(cadastro.usuarios.iter()) {
            *por_nivel.entry(u.nivel.nome()).or_insert(0) += 1;
        }
        let ordem_nivel = ["admin", "dono", "operador", "leitor", "nenhum"];

        // --------------------------------------------------------- estado
        let bloqueios = self
            .lista_negra
            .lock()
            .map(|l| l.ativos(agora).len() as u64)
            .unwrap_or(0);
        let (residentes, bytes_ram) = self
            .residentes
            .lock()
            .map(|r| {
                (
                    r.len() as u64,
                    r.values().map(|m| m.bytes() as u64).sum::<u64>(),
                )
            })
            .unwrap_or((0, 0));
        let sessoes_web = self.sessoes.lock().map(|s| s.quantas() as u64).unwrap_or(0);

        Ok(Json::objeto(vec![
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(agora)),
            ),
            ("versao", Json::texto_de(VERSAO)),
            ("papel", Json::texto_de(self.config.replicacao.papel.nome())),
            (
                "resumo",
                Json::objeto(vec![
                    ("bancos", Json::de_u64(bancos.len() as u64)),
                    ("tabelas", Json::de_u64(tabelas_total)),
                    ("registros", Json::de_u64(registros_total)),
                    ("bytes_reg", Json::de_u64(bytes_total)),
                    (
                        "usuarios",
                        Json::de_u64(
                            (cadastro.usuarios.len() + usize::from(cadastro.root.is_some())) as u64,
                        ),
                    ),
                    (
                        "conexoes",
                        Json::de_u64(self.conexoes.load(Ordering::SeqCst) as u64),
                    ),
                    ("sessoes_web", Json::de_u64(sessoes_web)),
                    ("bloqueios", Json::de_u64(bloqueios)),
                    ("tabelas_em_ram", Json::de_u64(residentes)),
                    ("bytes_em_ram", Json::de_u64(bytes_ram)),
                    ("acessos", Json::de_u64(ok + falhas)),
                    ("acessos_ok", Json::de_u64(ok)),
                    ("acessos_recusados", Json::de_u64(falhas)),
                    (
                        "ms_medio",
                        Json::de_u64(if ok + falhas > 0 {
                            soma_ms / (ok + falhas)
                        } else {
                            0
                        }),
                    ),
                    ("espelho", Json::Bool(self.config.espelho)),
                    ("somente_leitura", Json::Bool(self.config.somente_leitura)),
                ]),
            ),
            ("bancos", Json::Lista(bancos)),
            (
                "maiores_tabelas",
                Json::Lista(
                    maiores
                        .iter()
                        .map(|(n, r, b)| {
                            Json::objeto(vec![
                                ("tabela", Json::texto_de(n)),
                                ("registros", Json::de_u64(*r)),
                                ("bytes", Json::de_u64(*b)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_hora",
                Json::Lista(por_hora.iter().map(|n| Json::de_u64(*n)).collect()),
            ),
            (
                "recusadas_por_hora",
                Json::Lista(
                    recusadas_por_hora
                        .iter()
                        .map(|n| Json::de_u64(*n))
                        .collect(),
                ),
            ),
            (
                "por_operacao",
                Json::Lista(
                    ops.iter()
                        .map(|(o, a, r)| {
                            Json::objeto(vec![
                                ("op", Json::texto_de(o)),
                                ("ok", Json::de_u64(*a)),
                                ("recusados", Json::de_u64(*r)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "por_nivel",
                Json::Lista(
                    ordem_nivel
                        .iter()
                        .filter_map(|n| {
                            por_nivel.get(n).map(|q| {
                                Json::objeto(vec![
                                    ("nivel", Json::texto_de(*n)),
                                    ("quantos", Json::de_u64(*q)),
                                ])
                            })
                        })
                        .collect(),
                ),
            ),
            (
                "usuarios_ativos",
                Json::Lista(
                    usuarios_ativos
                        .iter()
                        .map(|(u, q)| {
                            Json::objeto(vec![
                                ("usuario", Json::texto_de(u)),
                                ("acessos", Json::de_u64(*q)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "top_ips",
                Json::Lista(
                    top_ips
                        .iter()
                        .map(|r| {
                            Json::objeto(vec![
                                ("ip", Json::texto_de(&r.ip)),
                                ("acessos", Json::de_u64(r.acessos)),
                                ("recusados", Json::de_u64(r.recusados)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    // ------------------------------------------------------ tabela em memoria

    /// Chave de residencia. Inclui o database porque duas bases podem ter
    /// tabela de mesmo nome -- e teriam, se ninguem cuidasse disso.
    fn chave_residente(p: &Json) -> String {
        format!(
            "{}/{}",
            p.texto_ou("database", ""),
            p.texto_ou("tabela", "")
        )
    }

    /// Mexe na copia residente, se a tabela deste pedido estiver carregada.
    fn residente_mut(&self, p: &Json, f: impl FnOnce(&mut TabelaMemoria)) {
        if let Ok(mut r) = self.residentes.lock() {
            if let Some(m) = r.get_mut(&Self::chave_residente(p)) {
                f(m);
            }
        }
    }

    /// Le a tabela inteira para a RAM.
    fn op_memoria_carregar(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let mut t = self.abrir(p, sessao)?;
        let esquema = t.esquema().clone();

        // As colunas com mapa de igualdade. Sem pedido, mapeia as que ja sao
        // primeira coluna de algum indice: quem indexou no disco costuma
        // filtrar pelo mesmo campo na memoria.
        let mapear: Vec<usize> = match p.campo("mapear").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, &esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => {
                let mut v: Vec<usize> = esquema
                    .indices()
                    .iter()
                    .filter_map(|i| i.colunas.first().map(|c| c.coluna))
                    .collect();
                v.sort_unstable();
                v.dedup();
                v
            }
        };

        let inicio = Instant::now();
        let m = {
            let _trava = self.dados.lock().map_err(|_| trava_envenenada())?;
            TabelaMemoria::carregar(&mut t, &mapear, crate::agora_ms())?
        };
        let ficha = ficha_residente(&Self::chave_residente(p), &m);
        let ms = inicio.elapsed().as_millis() as u64;
        self.residentes
            .lock()
            .map_err(|_| trava_envenenada())?
            .insert(Self::chave_residente(p), m);

        let mut campos = ficha;
        campos.push(("carregou_em_ms", Json::de_u64(ms)));
        Ok(Json::objeto(campos))
    }

    fn op_memoria_liberar(&self, p: &Json) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let saiu = self
            .residentes
            .lock()
            .map_err(|_| trava_envenenada())?
            .remove(&chave)
            .is_some();
        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            ("estava_carregada", Json::Bool(saiu)),
        ]))
    }

    /// O que esta residente agora.
    fn op_memoria(&self) -> Result<Json> {
        let r = self.residentes.lock().map_err(|_| trava_envenenada())?;
        let mut chaves: Vec<&String> = r.keys().collect();
        chaves.sort();
        let agora = crate::agora_ms();
        Ok(Json::objeto(vec![
            ("tabelas", Json::de_u64(r.len() as u64)),
            (
                "bytes",
                Json::de_u64(r.values().map(|m| m.bytes() as u64).sum()),
            ),
            (
                "residentes",
                Json::Lista(
                    chaves
                        .into_iter()
                        .map(|c| {
                            let m = &r[c];
                            let mut f = ficha_residente(c, m);
                            f.push((
                                "carregada_ha_s",
                                Json::de_u64(((agora - m.carregada_ms()) / 1000).max(0) as u64),
                            ));
                            Json::objeto(f)
                        })
                        .collect(),
                ),
            ),
        ]))
    }

    /// `SelectMemory`: a consulta que nao toca em disco.
    ///
    /// Recusa em vez de adivinhar quando a tabela nao esta carregada. Carregar
    /// uma tabela grande sem ninguem ter pedido seria a operacao rapida virando
    /// a operacao lenta, calada, na hora errada.
    fn op_selecionar_memoria(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        let chave = Self::chave_residente(p);
        let r = self.residentes.lock().map_err(|_| trava_envenenada())?;
        let m = r.get(&chave).ok_or_else(|| {
            PhxError::NaoEncontrado(format!(
                "{chave} nao esta em memoria; carregue antes com {{\"op\":\"memoria_carregar\",\"database\":...,\"tabela\":...}}"
            ))
        })?;
        let esquema = m.esquema();

        // O poder vale igual na memoria e no disco. O portao ja passou pelo
        // despachar; isto e o cinto: quem chegar aqui por outro caminho para.
        if let Some(u) = &sessao.usuario {
            if !u.pode(p.texto_ou("database", ""), Atividade::Ler) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de ler em {}",
                    u.login,
                    p.texto_ou("database", "")
                )));
            }
        }

        let mut onde = Vec::new();
        if let Some(l) = p.campo("onde").and_then(Json::lista) {
            for f in l {
                let coluna = coluna_de(
                    f.campo("coluna")
                        .ok_or_else(|| PhxError::Esquema("filtro sem \"coluna\"".into()))?,
                    esquema,
                )?;
                let op = Operador::de_texto(f.texto_ou("op", "="))?;
                let valor = match f.campo("valor") {
                    Some(v) => crate::valores::json_para_valor(v, &esquema.colunas()[coluna].ty)?,
                    None => phxsql_core::value::Value::Null,
                };
                onde.push(Filtro { coluna, op, valor });
            }
        }

        let mut ordenar = Vec::new();
        if let Some(l) = p.campo("ordenar").and_then(Json::lista) {
            for o in l {
                ordenar.push(Ordem {
                    coluna: coluna_de(
                        o.campo("coluna")
                            .ok_or_else(|| PhxError::Esquema("ordem sem \"coluna\"".into()))?,
                        esquema,
                    )?,
                    desc: o.booleano_ou("desc", false),
                });
            }
        }

        let colunas = match p.campo("colunas").and_then(Json::lista) {
            Some(l) => l
                .iter()
                .map(|j| coluna_de(j, esquema))
                .collect::<Result<Vec<usize>>>()?,
            None => Vec::new(),
        };

        let consulta = Consulta {
            onde,
            ordenar,
            colunas,
            pular: p.inteiro_ou("pular", 0).max(0) as u64,
            max: self.limite(p),
        };

        let inicio = Instant::now();
        let saida = m.selecionar(&consulta)?;
        let us = inicio.elapsed().as_micros() as u64;

        // A projecao muda as colunas, entao os nomes vem com o resultado --
        // senao quem le nao sabe qual campo e qual.
        let indices: Vec<usize> = if consulta.colunas.is_empty() {
            (0..esquema.colunas().len()).collect()
        } else {
            consulta.colunas.clone()
        };
        let nomes: Vec<String> = indices
            .iter()
            .map(|i| esquema.colunas()[*i].nome.clone())
            .collect();
        let tipos: Vec<phxsql_core::types::ColumnType> =
            indices.iter().map(|i| esquema.colunas()[*i].ty).collect();

        Ok(Json::objeto(vec![
            ("tabela", Json::texto_de(&chave)),
            (
                "colunas",
                Json::Lista(nomes.iter().map(Json::texto_de).collect()),
            ),
            ("achadas", Json::de_u64(saida.achadas)),
            ("devolvidas", Json::de_u64(saida.linhas.len() as u64)),
            ("examinadas", Json::de_u64(saida.examinadas)),
            (
                "por_mapa",
                match &saida.por_mapa {
                    Some(c) => Json::texto_de(c),
                    None => Json::Nulo,
                },
            ),
            ("us", Json::de_u64(us)),
            (
                "linhas",
                Json::Lista(
                    saida
                        .linhas
                        .iter()
                        .map(|(rowid, l)| {
                            let mut campos = vec![("rowid", Json::de_u64(*rowid))];
                            for ((n, v), ty) in nomes.iter().zip(l.iter()).zip(tipos.iter()) {
                                campos.push((n.as_str(), crate::valores::valor_para_json(v, ty)));
                            }
                            Json::objeto(campos)
                        })
                        .collect(),
                ),
            ),
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

/// Uma coluna, pelo nome ou pelo numero. Aceitar os dois e o que deixa a
/// consulta legivel a mao e barata pela interface.
fn coluna_de(j: &Json, esquema: &phxsql_core::schema::Schema) -> Result<usize> {
    if let Some(n) = j.inteiro() {
        let i = n as usize;
        if n < 0 || i >= esquema.colunas().len() {
            return Err(PhxError::Esquema(format!("coluna {n} nao existe")));
        }
        return Ok(i);
    }
    let nome = j.texto().unwrap_or("");
    esquema
        .colunas()
        .iter()
        .position(|c| c.nome == nome)
        .ok_or_else(|| PhxError::Esquema(format!("coluna {nome:?} nao existe")))
}

fn ficha_residente(chave: &str, m: &TabelaMemoria) -> Vec<(&'static str, Json)> {
    let nomes: Vec<Json> = m
        .colunas_mapeadas()
        .iter()
        .map(|i| Json::texto_de(&m.esquema().colunas()[*i].nome))
        .collect();
    vec![
        ("tabela", Json::texto_de(chave)),
        ("linhas", Json::de_u64(m.vivos())),
        ("slots", Json::de_u64(m.slots())),
        ("bytes", Json::de_u64(m.bytes() as u64)),
        ("mapas", Json::Lista(nomes)),
    ]
}
