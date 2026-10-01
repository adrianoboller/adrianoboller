//! TLS de soquete dos canais que falam TCP direto: IRC e Twitch (TLS desde o primeiro
//! byte, porta 6697), XMPP (STARTTLS no meio do fluxo, RFC 6120 §5) e IMAP (porta 993).
//!
//! Uma peca so para os quatro, como a decisao de quando o texto claro e aceitavel ja era
//! uma so (`irc::conectar_sem_tls`, que continua valendo para o caso sem TLS: so loopback).
//!
//! Decisoes:
//!
//! - **rustls com aws-lc**, o mesmo do `phxclaw-device-transport`: nada novo no Cargo.lock.
//! - **Raizes publicas (webpki-roots) por padrao**; com `<CANAL>_CA`, SO a autoridade dada
//!   -- servidor interno com CA propria nao deve aceitar, de quebra, qualquer certificado
//!   valido da internet (a mesma regra do `connect_with_ca` dos dispositivos).
//! - **O aperto de mao acontece ja na conexao**: certificado errado e erro na hora de ligar
//!   o canal, nao na primeira mensagem minutos depois.
//! - **Quem le em fundo nao segura a trava enquanto espera** (`ler_em_fundo`): o fluxo TLS
//!   nao se parte em leitor e escritor como o `TcpStream::try_clone`, entao os dois dividem
//!   uma trava; o leitor espia o soquete sem ela e so a toma quando ha bytes. Sem isso, a
//!   resposta do agente esperaria o leitor desistir de esperar.

use rustls::{ClientConfig, ClientConnection, StreamOwned};
use rustls_pki_types::ServerName;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Configuracao do cliente TLS de um canal.
#[derive(Clone)]
pub struct Tls {
    config: Arc<ClientConfig>,
}

fn provedor() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::aws_lc_rs::default_provider())
}

impl Tls {
    /// Raizes publicas da web (as do Mozilla, embutidas).
    pub fn publico() -> Result<Self, String> {
        let raizes = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        Self::com_raizes(raizes)
    }

    /// So a(s) autoridade(s) do PEM dado.
    pub fn com_ca_pem(pem: &[u8]) -> Result<Self, String> {
        use rustls_pki_types::CertificateDer;
        use rustls_pki_types::pem::PemObject;
        let mut raizes = rustls::RootCertStore::empty();
        for c in CertificateDer::pem_slice_iter(pem) {
            raizes
                .add(c.map_err(|e| format!("CA: {e}"))?)
                .map_err(|e| format!("CA: {e}"))?;
        }
        if raizes.is_empty() {
            return Err("CA: nenhum certificado no PEM".into());
        }
        Self::com_raizes(raizes)
    }

    /// A configuracao de um canal: `ca` e o caminho de um PEM, ou nada para as publicas.
    pub fn da_config(ca: Option<&str>) -> Result<Self, String> {
        match ca {
            None => Self::publico(),
            Some(p) => Self::com_ca_pem(&std::fs::read(p).map_err(|e| format!("CA {p}: {e}"))?),
        }
    }

    fn com_raizes(raizes: rustls::RootCertStore) -> Result<Self, String> {
        let config = ClientConfig::builder_with_provider(provedor())
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(raizes)
            .with_no_client_auth();
        Ok(Self {
            config: Arc::new(config),
        })
    }
}

/// O fio de um canal, com ou sem TLS; quem le e escreve nao precisa saber qual.
pub enum Fio {
    Claro(TcpStream),
    Tls(Box<StreamOwned<ClientConnection, TcpStream>>),
}

impl Fio {
    pub fn tcp(&self) -> &TcpStream {
        match self {
            Fio::Claro(s) => s,
            Fio::Tls(s) => &s.sock,
        }
    }

    pub fn set_read_timeout(&self, d: Option<Duration>) -> std::io::Result<()> {
        self.tcp().set_read_timeout(d)
    }

    /// STARTTLS: o mesmo TCP passa a falar TLS, com o certificado conferido contra `nome`
    /// (no XMPP, o dominio do JID, RFC 6120 §13.7.2 -- nao o host a que se conectou).
    pub fn subir(self, nome: &str, tls: &Tls) -> Result<Fio, String> {
        match self {
            Fio::Tls(_) => Err("o fio ja e TLS".into()),
            Fio::Claro(tcp) => apertar_mao(tcp, nome, tls),
        }
    }
}

impl Read for Fio {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Fio::Claro(s) => s.read(buf),
            Fio::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Fio {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Fio::Claro(s) => s.write(buf),
            Fio::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Fio::Claro(s) => s.flush(),
            Fio::Tls(s) => s.flush(),
        }
    }
}

/// O host de `host:porta` (com `[::1]:porta` para IPv6).
pub fn host_de(endereco: &str) -> String {
    let h = endereco
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(endereco);
    h.trim_start_matches('[').trim_end_matches(']').to_string()
}

fn apertar_mao(mut tcp: TcpStream, nome: &str, tls: &Tls) -> Result<Fio, String> {
    let servidor = ServerName::try_from(nome.to_string()).map_err(|e| format!("{nome}: {e}"))?;
    let mut conn = ClientConnection::new(tls.config.clone(), servidor)
        .map_err(|e| format!("{nome}: tls: {e}"))?;
    // O prazo do aperto de mao; depois quem usa o fio poe o dele.
    tcp.set_read_timeout(Some(Duration::from_secs(15))).ok();
    while conn.is_handshaking() {
        conn.complete_io(&mut tcp)
            .map_err(|e| format!("{nome}: tls: {e}"))?;
    }
    tcp.set_read_timeout(None).ok();
    Ok(Fio::Tls(Box::new(StreamOwned::new(conn, tcp))))
}

/// TCP para o primeiro endereco que atender: `localhost` costuma resolver `::1` antes de
/// `127.0.0.1`, e o servidor pode escutar so num deles.
fn tcp(endereco: &str) -> Result<TcpStream, String> {
    let alvos: Vec<_> = endereco
        .to_socket_addrs()
        .map_err(|e| format!("{endereco}: {e}"))?
        .collect();
    let mut ultimo = format!("{endereco}: sem endereco");
    for a in alvos {
        match TcpStream::connect_timeout(&a, Duration::from_secs(10)) {
            Ok(s) => {
                s.set_nodelay(true).ok();
                return Ok(s);
            }
            Err(e) => ultimo = format!("{endereco}: {e}"),
        }
    }
    Err(ultimo)
}

/// Conexao de canal: com `tls`, TLS desde o primeiro byte; sem, so loopback.
pub fn conectar(endereco: &str, tls: Option<&Tls>) -> Result<Fio, String> {
    match tls {
        None => super::irc::conectar_sem_tls(endereco).map(Fio::Claro),
        Some(t) => apertar_mao(tcp(endereco)?, &host_de(endereco), t),
    }
}

/// TCP sem TLS AINDA, para quem sobe por STARTTLS: qualquer host, porque o texto claro so
/// dura ate o `<starttls/>` -- e quem chama recusa seguir se o servidor nao oferecer.
pub fn conectar_para_starttls(endereco: &str) -> Result<Fio, String> {
    tcp(endereco).map(Fio::Claro)
}

/// Le o fio em fundo, numa thread, entregando cada pedaco a `ao_ler` (que devolve `false`
/// para parar). Termina quando o servidor fecha, quando o fio da erro ou quando ninguem
/// mais segura o fio (a conexao caiu do lado do canal).
pub fn ler_em_fundo(fio: Arc<Mutex<Fio>>, mut ao_ler: impl FnMut(&[u8]) -> bool + Send + 'static) {
    let espia = fio.lock().ok().and_then(|f| f.tcp().try_clone().ok());
    let Some(espia) = espia else {
        return;
    };
    // O prazo vale para o soquete (os clones dividem): a espera da espiada e a leitura sob a
    // trava acordam a cada 200 ms para ver se o canal ainda quer o fio.
    espia
        .set_read_timeout(Some(Duration::from_millis(200)))
        .ok();
    std::thread::spawn(move || {
        let mut bloco = vec![0u8; 64 * 1024];
        loop {
            if Arc::strong_count(&fio) == 1 {
                return;
            }
            match espia.peek(&mut [0u8; 1]) {
                Ok(_) => {}
                Err(e) if esgotou(&e) => continue,
                Err(_) => return,
            }
            loop {
                let lido = match fio.lock() {
                    Ok(mut f) => f.read(&mut bloco),
                    Err(_) => return,
                };
                match lido {
                    Ok(0) => return,
                    Ok(n) => {
                        if !ao_ler(&bloco[..n]) {
                            return;
                        }
                        // Bloco cheio: o TLS pode ter mais texto decifrado guardado, que a
                        // espiada do soquete nao ve.
                        if n < bloco.len() {
                            break;
                        }
                    }
                    Err(e) if esgotou(&e) => break,
                    Err(_) => return,
                }
            }
        }
    });
}

fn esgotou(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_do_endereco() {
        assert_eq!(host_de("irc.libera.chat:6697"), "irc.libera.chat");
        assert_eq!(host_de("[::1]:993"), "::1");
        assert!(Tls::publico().is_ok());
        assert!(Tls::com_ca_pem(b"nada").is_err());
    }
}
