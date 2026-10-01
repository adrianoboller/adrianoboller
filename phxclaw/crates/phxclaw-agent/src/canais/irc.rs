//! IRC (e o chat da Twitch, que e IRC): uma conexao TCP que fica aberta entre as voltas.
//!
//! O IRC entrega a linha uma vez e nao tem historico; o que chega vai primeiro para a
//! `Caixa` em disco e o laco le dela pelo cursor, como os outros. Uma thread le o soquete
//! e responde PING na hora -- se a resposta esperasse a volta do laco, o servidor derrubaria
//! a conexao durante uma tarefa longa.
//!
//! TLS: por padrao a conexao e TLS desde o primeiro byte (porta 6697; Twitch:
//! `irc.chat.twitch.tv:6697`), pelo `canais::tls`. Sem TLS (`PHXCLAW_IRC_TLS=false`) a senha
//! (o token `oauth:` da Twitch, a do NickServ) andaria em texto claro, entao a conexao sem
//! TLS continua aceita so em loopback (um tunel local).
//!
//! Quem fala: num canal (`#sala`) a conversa e o canal, e todo mundo nele fala com o
//! agente, como num grupo do Telegram; em mensagem direta a conversa e o nick. Nick de IRC
//! nao e autenticado sem NickServ -- a lista de nicks so vale em rede que o exige.

use super::caixa::Caixa;
use super::http::Credencial;
use super::tls::{Fio, Tls, conectar, ler_em_fundo};
use super::{Entrada, Mensagem, Provedor, Unidade};
use serde_json::Value;
use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Conexao TCP que so aceita loopback (ver o topo do modulo). Serve ao IRC, ao XMPP e ao
/// IMAP: a decisao de quando o texto claro e aceitavel e uma so.
pub fn conectar_sem_tls(endereco: &str) -> Result<TcpStream, String> {
    let alvos: Vec<_> = endereco
        .to_socket_addrs()
        .map_err(|e| format!("{endereco}: {e}"))?
        .collect();
    if alvos.is_empty() || !alvos.iter().all(|a| a.ip().is_loopback()) {
        return Err(format!(
            "{endereco}: conexao sem TLS so em loopback (use um tunel TLS local)"
        ));
    }
    let s = TcpStream::connect_timeout(&alvos[0], Duration::from_secs(10))
        .map_err(|e| format!("{endereco}: {e}"))?;
    s.set_nodelay(true).ok();
    Ok(s)
}

pub struct Config {
    /// "irc" ou "twitch": nome do canal, limite e se o nick vai em minusculas.
    pub nome: &'static str,
    pub endereco: String,
    pub nick: String,
    pub senha: Option<Credencial>,
    /// Canais (`#sala`) para entrar; nicks da lista nao precisam de JOIN.
    pub salas: Vec<String>,
    /// `None` = texto claro, so em loopback.
    pub tls: Option<Tls>,
}

struct Conexao {
    escrita: Arc<Mutex<Fio>>,
    /// So o `receber` le; o Mutex e para a conexao poder ser compartilhada com o `enviar`.
    linhas: Mutex<Receiver<String>>,
}

pub struct Irc {
    cfg: Config,
    caixa: Arc<Caixa>,
    /// A trava cobre so pegar ou trocar a conexao, nunca a espera: o `enviar` da resposta
    /// nao pode ficar 25 s atras do `receber` que esta esperando linha.
    conexao: Mutex<Option<Arc<Conexao>>>,
}

/// Uma linha `:nick!user@host PRIVMSG alvo :texto` como (nick, alvo, texto).
pub fn privmsg(linha: &str) -> Option<(String, String, String)> {
    let linha = linha.trim_end_matches(['\r', '\n']);
    // Twitch manda tags IRCv3 na frente (`@badge=...;... :nick!...`).
    let linha = if linha.starts_with('@') {
        linha.split_once(' ')?.1
    } else {
        linha
    };
    let resto = linha.strip_prefix(':')?;
    let (origem, resto) = resto.split_once(' ')?;
    let resto = resto.strip_prefix("PRIVMSG ")?;
    let (alvo, texto) = resto.split_once(" :")?;
    let nick = origem.split('!').next()?.to_string();
    Some((nick, alvo.trim().to_string(), texto.to_string()))
}

impl Irc {
    pub fn novo(cfg: Config, caixa: Arc<Caixa>) -> Self {
        Self {
            cfg,
            caixa,
            conexao: Mutex::new(None),
        }
    }

    fn escrever(s: &Mutex<Fio>, linha: &str) -> Result<(), String> {
        // CR ou LF no meio viraria um comando IRC novo: injecao.
        let limpa: String = linha.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        let mut g = s.lock().map_err(|_| "soquete envenenado".to_string())?;
        g.write_all(format!("{limpa}\r\n").as_bytes())
            .map_err(|e| format!("irc: escrita: {e}"))
    }

    fn abrir(&self) -> Result<Conexao, String> {
        let escrita = Arc::new(Mutex::new(conectar(
            &self.cfg.endereco,
            self.cfg.tls.as_ref(),
        )?));
        if let Some(c) = &self.cfg.senha {
            c.com("receive", |p| {
                Self::escrever(&escrita, &format!("PASS {p}"))
            })?;
        }
        Self::escrever(&escrita, &format!("NICK {}", self.cfg.nick))?;
        Self::escrever(&escrita, &format!("USER {} 0 * :PhxClaw", self.cfg.nick))?;
        let (tx, rx) = channel();
        // O leitor nao segura um clone forte do fio: so o fraco, para o PONG. Quando a
        // conexao cai do lado do canal, o fio fica sem dono e a leitura em fundo para.
        let pong = Arc::downgrade(&escrita);
        let mut pendente: Vec<u8> = Vec::new();
        ler_em_fundo(escrita.clone(), move |bytes| {
            pendente.extend_from_slice(bytes);
            while let Some(i) = pendente.iter().position(|b| *b == b'\n') {
                let linha: Vec<u8> = pendente.drain(..=i).collect();
                let linha = String::from_utf8_lossy(&linha).into_owned();
                if let Some(arg) = linha.strip_prefix("PING") {
                    if let Some(p) = pong.upgrade() {
                        let _ = Self::escrever(&p, &format!("PONG{}", arg.trim_end()));
                    }
                } else if tx.send(linha).is_err() {
                    return false;
                }
            }
            // Linha sem fim acima do teto do protocolo (512 bytes, 8 KiB com tags IRCv3):
            // servidor quebrado ou hostil; descarta em vez de crescer sem limite.
            if pendente.len() > 64 * 1024 {
                pendente.clear();
            }
            true
        });
        // Espera o 001 (boas-vindas) antes de entrar nas salas: JOIN antes do registro e
        // ignorado pelo servidor, calado.
        let fim = Instant::now() + Duration::from_secs(15);
        loop {
            let resta = fim.saturating_duration_since(Instant::now());
            let l = rx
                .recv_timeout(resta)
                .map_err(|_| "irc: o servidor nao confirmou o registro (001)".to_string())?;
            let cmd = l.split_whitespace().nth(1).unwrap_or("");
            match cmd {
                "001" => break,
                "433" => return Err(format!("irc: nick {} em uso", self.cfg.nick)),
                "464" => return Err("irc: senha recusada".into()),
                _ => {}
            }
        }
        for sala in &self.cfg.salas {
            Self::escrever(&escrita, &format!("JOIN {sala}"))?;
        }
        Ok(Conexao {
            escrita,
            linhas: Mutex::new(rx),
        })
    }

    fn com_conexao<T>(&self, f: impl FnOnce(&Conexao) -> Result<T, String>) -> Result<T, String> {
        let c = {
            let mut g = self
                .conexao
                .lock()
                .map_err(|_| "conexao envenenada".to_string())?;
            if g.is_none() {
                *g = Some(Arc::new(self.abrir()?));
            }
            g.as_ref().expect("acabou de abrir").clone()
        };
        let r = f(&c);
        if r.is_err()
            && let Ok(mut g) = self.conexao.lock()
            && g.as_ref().is_some_and(|x| Arc::ptr_eq(x, &c))
        {
            // Conexao que falhou nao se reaproveita: a proxima volta abre outra.
            *g = None;
        }
        r
    }
}

impl Provedor for Irc {
    fn nome(&self) -> &str {
        self.cfg.nome
    }

    /// A linha IRC tem 512 bytes contando o prefixo que o servidor poe ao repassar
    /// (`:nick!user@host`); 350 deixa folga. A Twitch conta 500 caracteres.
    fn limite(&self) -> (usize, Unidade) {
        if self.cfg.nome == "twitch" {
            (500, Unidade::Caractere)
        } else {
            (350, Unidade::Byte)
        }
    }

    fn receber(&self, cursor: Option<&str>, espera_seg: u64) -> Result<Vec<Entrada>, String> {
        let nick = self.cfg.nick.to_ascii_lowercase();
        let novas = self.com_conexao(|c| {
            let mut novas = Vec::new();
            let mut espera = Duration::from_secs(espera_seg);
            let linhas = c.linhas.lock().map_err(|_| "fila envenenada".to_string())?;
            loop {
                match linhas.recv_timeout(espera) {
                    Ok(l) => {
                        if let Some((de, alvo, texto)) = privmsg(&l) {
                            let conversa = if alvo.starts_with('#') {
                                alvo
                            } else {
                                de.clone()
                            };
                            if de.to_ascii_lowercase() != nick {
                                novas.push((
                                    Mensagem {
                                        conversa,
                                        autor: de,
                                        id: String::new(),
                                        texto: Some(texto),
                                    },
                                    Value::Null,
                                ));
                            }
                        }
                        // Chegou algo: so drena o que ja esta na fila.
                        espera = Duration::from_millis(50);
                    }
                    Err(RecvTimeoutError::Timeout) => return Ok(novas),
                    Err(RecvTimeoutError::Disconnected) => {
                        if novas.is_empty() {
                            return Err("irc: o servidor fechou a conexao".into());
                        }
                        return Ok(novas);
                    }
                }
            }
        })?;
        if !novas.is_empty() {
            self.caixa.anexar(novas)?;
        }
        self.caixa.ler_desde(cursor, Duration::ZERO)
    }

    fn enviar(&self, conversa: &str, texto: &str) -> Result<String, String> {
        self.com_conexao(|c| {
            for linha in texto.lines().filter(|l| !l.trim().is_empty()) {
                Self::escrever(&c.escrita, &format!("PRIVMSG {conversa} :{linha}"))?;
            }
            Ok(String::new())
        })
    }
}
