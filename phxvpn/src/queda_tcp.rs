//! A porta TCP de uma rede UDP: a ponte que deixa o membro cair do UDP para
//! o TCP sem um segundo `openvpn`.
//!
//! # Por que uma ponte, e nao dois processos
//!
//! O servidor 2.6 escuta num protocolo so (o multi-soquete e da 2.7,
//! `Changes.rst` «Multi-socket support for servers»). A receita de sempre
//! para UDP+TCP na 2.6 e dois processos -- e aqui ela custava o que a casa
//! promete: cada processo tem a sua placa e a sua sub-rede, entao o membro que
//! caisse para o TCP ganharia OUTRO IP (o `ccd/` fixa um so), o
//! `client-to-client` nao atravessa de um processo ao outro, e o kernel teria
//! de encaminhar entre as duas placas -- `ip_forward` ligado e a guarda de
//! isolamento (`rotas.rs`) aberta para uma faixa nova, que e exatamente o
//! buraco que ela existe para fechar.
//!
//! A ponte evita as tres coisas: escuta TCP, tira o quadro do OpenVPN (2
//! bytes de tamanho + pacote, o mesmo que ele poe no fio TCP) e entrega o
//! pacote ao `openvpn` UDP da rede por `127.0.0.1`, um soquete UDP por
//! conexao. O pacote do OpenVPN e igual nos dois transportes; so o
//! enquadramento muda. Medido com o 2.6.19: cliente `tcp-client` fala com o
//! servidor `udp` sem aviso de incompatibilidade (`provas/servidor-alcance`).
//!
//! # O endereco de origem
//!
//! Para o OpenVPN, todo cliente da ponte vem do loopback -- e o limitador do
//! autenticador (`verificar.rs`) conta tentativas por `untrusted_ip`. Com
//! `127.0.0.1` para todos, um atacante pela porta TCP travaria o codigo de
//! todo membro que tambem caiu para o TCP. Por isso cada IP de fora ganha o
//! seu endereco em `127.0.0.0/8` (espalhado por SHA-256 com chave do
//! processo, para ninguem escolher colisao), e a linha `queda-tcp:` do
//! `openvpn.log` diz qual IP de fora e qual `127.x.y.z:porta`.
//!
//! # `port-share` na ponte
//!
//! Com o `port-share` da rede, a primeira leitura decide: parece o primeiro
//! pacote de um cliente OpenVPN (a regra do `ps.c:975-1019` da 2.6.19), vai
//! ao OpenVPN; nao parece (um `ClientHello` TLS, um `GET`), vai ao servidor
//! HTTPS de quem ja usava a porta. Sem `port-share`, nada se examina: o
//! OpenVPN decide o que e dele.

use crate::supervisor::Registro;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Arquivo na pasta da rede que liga a ponte. Quem escreve e o painel
/// (`alcance.rs`); quem le e o supervisor, a cada `garantir`.
pub const ARQUIVO: &str = "queda-tcp";

/// Conexoes vivas por ponte. Cada uma custa duas threads: sem teto, quem
/// abrisse conexoes sem mandar nada levaria o painel as threads do sistema.
pub const MAX_CONEXOES: usize = 256;
/// O cliente tem isto para mandar os primeiros bytes.
const PRIMEIROS_BYTES: Duration = Duration::from_secs(10);
/// Sem trafego por isto, a conexao cai. O servidor manda `ping` a cada 10 s
/// (`keepalive 10 60`): 180 s calados e conexao morta, nao membro quieto.
const OCIOSO: Duration = Duration::from_secs(180);
/// A volta (UDP -> TCP) acorda a cada tanto para ver se a ida ja fechou.
const VOLTA_ACORDA: Duration = Duration::from_millis(500);

/// O que o supervisor precisa para abrir a ponte de uma rede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conf {
    /// Porta TCP que a ponte escuta (a da queda).
    pub porta: u16,
    /// Porta UDP do `openvpn` da rede (em `127.0.0.1`).
    pub udp: u16,
    /// `host`, `porta` do servidor HTTPS que divide a porta TCP.
    pub port_share: Option<(String, u16)>,
}

impl Conf {
    pub fn texto(&self) -> String {
        let mut t = format!("porta {}\nudp {}\n", self.porta, self.udp);
        if let Some((h, p)) = &self.port_share {
            t.push_str(&format!("port-share {h} {p}\n"));
        }
        t
    }

    pub fn analisar(t: &str) -> Result<Conf, String> {
        let (mut porta, mut udp, mut ps) = (None, None, None);
        let num = |v: Option<&str>| {
            v.and_then(|v| v.parse::<u16>().ok())
                .filter(|p| *p > 0)
                .ok_or_else(|| format!("{ARQUIVO}: porta invalida"))
        };
        for l in t.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let mut p = l.split_whitespace();
            match p.next() {
                Some("porta") => porta = Some(num(p.next())?),
                Some("udp") => udp = Some(num(p.next())?),
                Some("port-share") => {
                    let h = p.next().ok_or(format!("{ARQUIVO}: port-share sem host"))?;
                    ps = Some((h.to_string(), num(p.next())?));
                }
                _ => return Err(format!("{ARQUIVO}: linha desconhecida: {l}")),
            }
        }
        Ok(Conf {
            porta: porta.ok_or(format!("{ARQUIVO}: falta a porta"))?,
            udp: udp.ok_or(format!("{ARQUIVO}: falta a porta udp"))?,
            port_share: ps,
        })
    }

    /// `None` sem o arquivo: a rede nao tem queda.
    pub fn ler(dir: &Path) -> Result<Option<Conf>, String> {
        match std::fs::read_to_string(dir.join(ARQUIVO)) {
            Ok(t) => Conf::analisar(&t).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{ARQUIVO}: {e}")),
        }
    }
}

/// O primeiro pedaco e de um cliente OpenVPN? A regra do `port-share` do
/// proprio OpenVPN (`ps.c:975`): tamanho (2 bytes) e opcode do
/// `P_CONTROL_HARD_RESET_CLIENT_V2` (7) ou `_V3` (10, tls-crypt-v2, cujo
/// pacote e bem maior por levar a chave embrulhada).
pub fn e_openvpn(cab: &[u8]) -> bool {
    const V2: u8 = 7 << 3;
    const V3: u8 = 10 << 3;
    if cab.len() < 3 {
        return false;
    }
    let tam = u16::from_be_bytes([cab[0], cab[1]]);
    match cab[2] {
        V3 => (336..1024 + 255).contains(&tam),
        V2 => (14..=255).contains(&tam),
        _ => false,
    }
}

/// O endereco de loopback com que a ponte fala pelo membro que vem de `ip`.
/// Chave aleatoria do processo: um atacante nao escolhe um IP que caia no
/// mesmo `127.x.y.z` de um membro para travar o autenticador dele.
pub fn origem_local(ip: IpAddr) -> Ipv4Addr {
    static CHAVE: OnceLock<Vec<u8>> = OnceLock::new();
    let chave = CHAVE.get_or_init(|| phxsql_core::senha::bytes_aleatorios(16));
    let mut m = chave.clone();
    match ip {
        IpAddr::V4(v) => m.extend_from_slice(&v.octets()),
        IpAddr::V6(v) => m.extend_from_slice(&v.octets()),
    }
    let h = phxsql_core::hash::sha256(&m);
    // .0 e .255 no ultimo octeto ficam de fora: tem sistema que os trata
    // como rede e difusao mesmo dentro do /8.
    let ultimo = match h[2] {
        0 | 255 => 1,
        x => x,
    };
    Ipv4Addr::new(127, h[0], h[1], ultimo)
}

type Vivas = Arc<Mutex<HashMap<u64, TcpStream>>>;

/// Por quanto tempo a ponte lembra a origem de quem ja saiu: o
/// `client-disconnect` de quem saiu pela ponte chega quando o `openvpn`
/// percebe (o `ping-restart`, ate 120 s depois do TCP fechar).
const LEMBRAR: Duration = Duration::from_secs(900);
/// Teto do mapa: acima dele, as mais velhas das ja fechadas saem primeiro.
const TETO_ORIGENS: usize = MAX_CONEXOES * 16;

struct Origem {
    real: SocketAddr,
    fechou: Option<std::time::Instant>,
}

/// `127.x.y.z:porta` (como o `openvpn` ve quem veio pela ponte) -> o
/// endereco de fora. De todas as pontes do processo: cada conexao tem o seu
/// soquete UDP, entao o par visto nao se repete entre redes.
fn origens() -> &'static Mutex<HashMap<SocketAddr, Origem>> {
    static O: OnceLock<Mutex<HashMap<SocketAddr, Origem>>> = OnceLock::new();
    O.get_or_init(Mutex::default)
}

fn lembrar_origem(vista: SocketAddr, real: SocketAddr) {
    let mut m = origens().lock().unwrap_or_else(|e| e.into_inner());
    m.retain(|_, o| o.fechou.map_or(true, |t| t.elapsed() < LEMBRAR));
    while m.len() >= TETO_ORIGENS {
        let Some(k) = m
            .iter()
            .filter_map(|(k, o)| o.fechou.map(|t| (t, *k)))
            .min()
            .map(|(_, k)| k)
        else {
            break;
        };
        m.remove(&k);
    }
    m.insert(vista, Origem { real, fechou: None });
}

fn esquecer_depois(vista: SocketAddr) {
    let mut m = origens().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(o) = m.get_mut(&vista) {
        o.fechou = Some(std::time::Instant::now());
    }
}

/// O endereco de fora de quem o `openvpn` ve como `vista` -- `None` quando
/// nao veio pela ponte. E por aqui, e nao relendo o `openvpn.log`, que o
/// historico de conexoes (`historico.rs`) grava o IP REAL: o mapa e o da
/// propria ponte, no mesmo processo.
pub fn origem_real(vista: SocketAddr) -> Option<SocketAddr> {
    let m = origens().lock().unwrap_or_else(|e| e.into_inner());
    m.get(&vista)
        .filter(|o| o.fechou.map_or(true, |t| t.elapsed() < LEMBRAR))
        .map(|o| o.real)
}

/// Uma ponte no ar. Sai do ar no `drop`: para de aceitar e derruba as
/// conexoes vivas (o membro reconecta, como num reinicio do OpenVPN).
pub struct Ponte {
    conf: Conf,
    parar: Arc<AtomicBool>,
    porta_local: u16,
    vivas: Vivas,
    fio: Option<std::thread::JoinHandle<()>>,
}

impl Ponte {
    /// Abre a escuta (dupla pilha quando o sistema tem IPv6, como o
    /// OpenVPN; so IPv4 quando nao tem) e comeca a aceitar.
    pub fn abrir(conf: Conf, registro: Arc<Mutex<Registro>>) -> Result<Ponte, String> {
        let escuta = TcpListener::bind(("::", conf.porta))
            .or_else(|_| TcpListener::bind(("0.0.0.0", conf.porta)))
            .map_err(|e| format!("escutar TCP {}: {e}", conf.porta))?;
        let porta_local = escuta.local_addr().map_err(|e| e.to_string())?.port();
        let parar = Arc::new(AtomicBool::new(false));
        let vivas: Vivas = Arc::default();
        let fio = {
            let (conf, parar, vivas) = (conf.clone(), Arc::clone(&parar), Arc::clone(&vivas));
            std::thread::spawn(move || aceitar(escuta, conf, parar, vivas, registro))
        };
        Ok(Ponte {
            conf,
            parar,
            porta_local,
            vivas,
            fio: Some(fio),
        })
    }

    pub fn conf(&self) -> &Conf {
        &self.conf
    }

    /// A porta que a ponte escuta de fato (a da conf; os testes pedem 0).
    pub fn porta(&self) -> u16 {
        self.porta_local
    }
}

impl Drop for Ponte {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::SeqCst);
        // O `accept` so volta com uma conexao: esta, que ele descarta.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], self.porta_local)),
            Duration::from_secs(1),
        );
        if let Some(f) = self.fio.take() {
            let _ = f.join();
        }
        if let Ok(v) = self.vivas.lock() {
            for s in v.values() {
                let _ = s.shutdown(Shutdown::Both);
            }
        }
    }
}

fn anotar(registro: &Arc<Mutex<Registro>>, linha: &str) {
    if let Ok(mut r) = registro.lock() {
        let _ = r.escrever(format!("phxvpn queda-tcp: {linha}\n").as_bytes());
    }
}

fn aceitar(
    escuta: TcpListener,
    conf: Conf,
    parar: Arc<AtomicBool>,
    vivas: Vivas,
    registro: Arc<Mutex<Registro>>,
) {
    static PROXIMO: AtomicU64 = AtomicU64::new(1);
    let mut avisou_teto = false;
    for c in escuta.incoming() {
        if parar.load(Ordering::SeqCst) {
            break;
        }
        let Ok(c) = c else { continue };
        let id = PROXIMO.fetch_add(1, Ordering::Relaxed);
        {
            let mut v = vivas.lock().expect("vivas");
            if v.len() >= MAX_CONEXOES {
                if !avisou_teto {
                    anotar(
                        &registro,
                        &format!("{MAX_CONEXOES} conexoes vivas: recusando"),
                    );
                    avisou_teto = true;
                }
                continue;
            }
            avisou_teto = false;
            match c.try_clone() {
                Ok(k) => v.insert(id, k),
                Err(_) => continue,
            };
        }
        let (conf, vivas, registro) = (conf.clone(), Arc::clone(&vivas), Arc::clone(&registro));
        std::thread::spawn(move || {
            atender(c, &conf, &registro);
            vivas.lock().expect("vivas").remove(&id);
        });
    }
}

/// Le ate `n` bytes (menos so se o outro lado fechou ou calou no prazo).
fn ler_ate(c: &mut TcpStream, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    let mut lidos = 0;
    while lidos < n {
        match c.read(&mut b[lidos..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => lidos += k,
        }
    }
    b.truncate(lidos);
    b
}

fn atender(mut c: TcpStream, conf: &Conf, registro: &Arc<Mutex<Registro>>) {
    let Ok(origem) = c.peer_addr() else { return };
    let _ = c.set_nodelay(true);
    let _ = c.set_read_timeout(Some(PRIMEIROS_BYTES));
    let cab = ler_ate(&mut c, 3);
    if cab.len() < 3 {
        return;
    }
    if let Some((h, p)) = &conf.port_share {
        if !e_openvpn(&cab) {
            if let Err(e) = repartir(c, &cab, h, *p) {
                anotar(registro, &format!("{origem} -> port-share {h}:{p}: {e}"));
            }
            return;
        }
    }
    let _ = c.set_read_timeout(Some(OCIOSO));
    if let Err(e) = transportar(c, cab, origem, conf.udp, registro) {
        anotar(registro, &format!("{origem}: {e}"));
    }
}

/// Os quadros TCP do OpenVPN viram datagramas para o `openvpn` UDP, e os
/// datagramas dele voltam enquadrados.
fn transportar(
    mut c: TcpStream,
    cab: Vec<u8>,
    origem: SocketAddr,
    udp: u16,
    registro: &Arc<Mutex<Registro>>,
) -> Result<(), String> {
    let local = origem_local(origem.ip());
    let u = UdpSocket::bind((local, 0))
        .or_else(|_| UdpSocket::bind(("127.0.0.1", 0)))
        .map_err(|e| format!("soquete UDP: {e}"))?;
    u.connect(("127.0.0.1", udp))
        .map_err(|e| format!("ligar ao openvpn UDP {udp}: {e}"))?;
    let _ = u.set_read_timeout(Some(VOLTA_ACORDA));
    let vista = u.local_addr().map_err(|e| e.to_string())?;
    anotar(registro, &format!("{origem} entra como {vista}"));
    lembrar_origem(vista, origem);
    let fechou = Arc::new(AtomicBool::new(false));
    let volta = {
        let (u, mut t, fechou) = (
            u.try_clone().map_err(|e| e.to_string())?,
            c.try_clone().map_err(|e| e.to_string())?,
            Arc::clone(&fechou),
        );
        std::thread::spawn(move || {
            let mut b = vec![0u8; 65535];
            let mut calado = Duration::ZERO;
            while !fechou.load(Ordering::SeqCst) {
                match u.recv(&mut b) {
                    Ok(n) => {
                        calado = Duration::ZERO;
                        let mut q = Vec::with_capacity(n + 2);
                        q.extend_from_slice(&(n as u16).to_be_bytes());
                        q.extend_from_slice(&b[..n]);
                        if t.write_all(&q).is_err() {
                            break;
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        calado += VOLTA_ACORDA;
                        if calado >= OCIOSO {
                            break;
                        }
                    }
                    // ECONNREFUSED: o openvpn caiu ou reinicia. A conexao
                    // fica; o membro percebe pelo ping e reconecta.
                    Err(_) => std::thread::sleep(VOLTA_ACORDA),
                }
            }
            let _ = t.shutdown(Shutdown::Both);
        })
    };
    // Os 3 bytes ja lidos: o tamanho do primeiro quadro e o 1o byte dele.
    let mut tam = u16::from_be_bytes([cab[0], cab[1]]) as usize;
    let mut quadro = vec![cab[2]];
    let r = loop {
        // Quadro vazio o OpenVPN nao manda; e com ele o 3o byte ja lido
        // seria do quadro seguinte.
        if tam == 0 {
            break Err("quadro vazio: nao e OpenVPN".into());
        }
        let resto = ler_ate(&mut c, tam - quadro.len());
        quadro.extend_from_slice(&resto);
        if quadro.len() < tam {
            break Ok(());
        }
        if let Err(e) = u.send(&quadro) {
            if e.kind() != std::io::ErrorKind::ConnectionRefused {
                break Err(format!("enviar ao openvpn: {e}"));
            }
        }
        let t = ler_ate(&mut c, 2);
        if t.len() < 2 {
            break Ok(());
        }
        tam = u16::from_be_bytes([t[0], t[1]]) as usize;
        quadro.clear();
    };
    fechou.store(true, Ordering::SeqCst);
    let _ = c.shutdown(Shutdown::Both);
    let _ = volta.join();
    anotar(registro, &format!("{origem} saiu"));
    esquecer_depois(vista);
    r
}

/// O que nao e OpenVPN vai ao servidor que ja usava a porta, com os bytes ja
/// lidos na frente.
fn repartir(c: TcpStream, cab: &[u8], host: &str, porta: u16) -> Result<(), String> {
    use std::net::ToSocketAddrs;
    let alvo = (host, porta)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("nao resolveu")?;
    let mut s =
        TcpStream::connect_timeout(&alvo, Duration::from_secs(5)).map_err(|e| e.to_string())?;
    s.write_all(cab).map_err(|e| e.to_string())?;
    let _ = c.set_read_timeout(Some(OCIOSO));
    let _ = s.set_read_timeout(Some(OCIOSO));
    let (mut c2, mut s2) = (
        c.try_clone().map_err(|e| e.to_string())?,
        s.try_clone().map_err(|e| e.to_string())?,
    );
    let ida = std::thread::spawn(move || {
        let mut c = c;
        let _ = std::io::copy(&mut c, &mut s);
        let _ = s.shutdown(Shutdown::Write);
    });
    let _ = std::io::copy(&mut s2, &mut c2);
    let _ = c2.shutdown(Shutdown::Both);
    let _ = ida.join();
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn registro(nome: &str) -> (Arc<Mutex<Registro>>, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("phxvpn-queda-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let r = Registro::abrir(&d.join("openvpn.log"), 1 << 20, 1).unwrap();
        (Arc::new(Mutex::new(r)), d)
    }

    #[test]
    fn conf_vai_e_volta_pelo_arquivo() {
        let c = Conf {
            porta: 443,
            udp: 1195,
            port_share: Some(("127.0.0.1".into(), 8443)),
        };
        assert_eq!(Conf::analisar(&c.texto()).unwrap(), c);
        let sem = Conf {
            port_share: None,
            ..c
        };
        assert_eq!(Conf::analisar(&sem.texto()).unwrap(), sem);
        assert!(Conf::analisar("porta 443\n").is_err());
        assert!(Conf::analisar("porta 0\nudp 1\n").is_err());
        assert!(Conf::analisar("porta 1\nudp 1\nup /bin/sh\n").is_err());
    }

    /// A regra do `ps.c`: o primeiro pacote do OpenVPN passa, o TLS e o
    /// HTTP nao.
    #[test]
    fn distingue_openvpn_de_https() {
        assert!(e_openvpn(&[0, 60, 7 << 3]));
        assert!(e_openvpn(&[0x01, 0x80, 10 << 3]));
        assert!(!e_openvpn(&[0x16, 0x03, 0x01]), "ClientHello TLS");
        assert!(!e_openvpn(b"GET"));
        assert!(!e_openvpn(&[0, 13, 7 << 3]), "curto demais");
        assert!(!e_openvpn(&[0, 60, 10 << 3]), "v3 sem a chave embrulhada");
        assert!(!e_openvpn(&[0, 60]));
    }

    /// IP de fora diferente, loopback diferente; o mesmo IP, o mesmo.
    #[test]
    fn origem_local_separa_quem_vem_de_fora() {
        let a = origem_local("203.0.113.9".parse().unwrap());
        let b = origem_local("203.0.113.10".parse().unwrap());
        assert_eq!(a, origem_local("203.0.113.9".parse().unwrap()));
        assert_ne!(a, b);
        assert!(a.is_loopback() && b.is_loopback());
        assert_ne!(a.octets()[3], 0);
        assert_ne!(a.octets()[3], 255);
    }

    /// Ponta a ponta sem OpenVPN: um «servidor UDP» de eco responde; o
    /// cliente TCP manda quadros e recebe os mesmos de volta, enquadrados.
    /// E o datagrama chega de um endereco de loopback que NAO e o 127.0.0.1
    /// (a guarda do limitador do autenticador).
    #[test]
    fn quadro_tcp_vira_datagrama_e_volta() {
        let eco = UdpSocket::bind("127.0.0.1:0").unwrap();
        let udp = eco.local_addr().unwrap().port();
        let (reg, d) = registro("eco");
        let ponte = Ponte::abrir(
            Conf {
                porta: 0,
                udp,
                port_share: None,
            },
            reg,
        )
        .unwrap();
        let visto = std::thread::spawn(move || {
            let mut b = [0u8; 2048];
            let mut de = Vec::new();
            for _ in 0..2 {
                let (n, o) = eco.recv_from(&mut b).unwrap();
                de.push(o);
                eco.send_to(&b[..n], o).unwrap();
            }
            de
        });
        let mut c = TcpStream::connect(("127.0.0.1", ponte.porta())).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        for msg in [&b"\x38primeiro"[..], &[0x38; 1400][..]] {
            let mut q = (msg.len() as u16).to_be_bytes().to_vec();
            q.extend_from_slice(msg);
            // Em dois pedacos: o quadro nao depende de chegar inteiro.
            c.write_all(&q[..3]).unwrap();
            std::thread::sleep(Duration::from_millis(20));
            c.write_all(&q[3..]).unwrap();
            let mut t = [0u8; 2];
            c.read_exact(&mut t).unwrap();
            let mut volta = vec![0u8; u16::from_be_bytes(t) as usize];
            c.read_exact(&mut volta).unwrap();
            assert_eq!(volta, msg);
        }
        let de = visto.join().unwrap();
        match de[0].ip() {
            IpAddr::V4(v) => {
                assert!(v.is_loopback());
                assert_ne!(v, Ipv4Addr::LOCALHOST, "todos pareceriam o mesmo cliente");
            }
            IpAddr::V6(_) => panic!("{de:?}"),
        }
        drop(ponte);
        let log = std::fs::read_to_string(d.join("openvpn.log")).unwrap();
        assert!(log.contains("phxvpn queda-tcp: 127.0.0.1:"), "{log}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O historico pergunta a origem real de quem o `openvpn` ve como
    /// `127.x.y.z:porta` -- e ela continua sabida depois que a conexao
    /// fecha (o `client-disconnect` chega depois). RED: sem o
    /// `lembrar_origem` no `transportar`, `origem_real` devolve `None` e o
    /// historico grava o loopback.
    #[test]
    fn ponte_diz_a_origem_real_de_quem_o_openvpn_ve() {
        let eco = UdpSocket::bind("127.0.0.1:0").unwrap();
        let udp = eco.local_addr().unwrap().port();
        let (reg, d) = registro("origem");
        let ponte = Ponte::abrir(
            Conf {
                porta: 0,
                udp,
                port_share: None,
            },
            reg,
        )
        .unwrap();
        let mut c = TcpStream::connect(("127.0.0.1", ponte.porta())).unwrap();
        let de_fora = c.local_addr().unwrap();
        c.write_all(&[0, 3, 0x38, 1, 2]).unwrap();
        eco.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut b = [0u8; 16];
        let (_, vista) = eco.recv_from(&mut b).unwrap();
        assert_eq!(origem_real(vista), Some(de_fora));
        drop(c);
        drop(ponte);
        assert_eq!(origem_real(vista), Some(de_fora), "esqueceu ao fechar");
        assert_eq!(origem_real("127.0.0.1:9".parse().unwrap()), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Com `port-share`, o que nao e OpenVPN vai ao outro servidor com os
    /// bytes ja lidos na frente; sem `port-share` nada vai la.
    #[test]
    fn port_share_manda_o_https_ao_dono_da_porta() {
        let web = TcpListener::bind("127.0.0.1:0").unwrap();
        let wp = web.local_addr().unwrap().port();
        let atendeu = std::thread::spawn(move || {
            let (mut s, _) = web.accept().unwrap();
            let mut b = [0u8; 18];
            s.read_exact(&mut b).unwrap();
            s.write_all(b"HTTP/1.0 200 OK\r\n\r\nola").unwrap();
            b.to_vec()
        });
        let (reg, d) = registro("ps");
        let ponte = Ponte::abrir(
            Conf {
                porta: 0,
                udp: 9,
                port_share: Some(("127.0.0.1".into(), wp)),
            },
            reg,
        )
        .unwrap();
        let mut c = TcpStream::connect(("127.0.0.1", ponte.porta())).unwrap();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        c.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
        let mut r = String::new();
        let _ = c.read_to_string(&mut r);
        assert_eq!(r, "HTTP/1.0 200 OK\r\n\r\nola");
        assert_eq!(atendeu.join().unwrap(), b"GET / HTTP/1.0\r\n\r\n");
        drop(ponte);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Passado o teto, a conexao seguinte e fechada sem atendimento.
    #[test]
    fn teto_de_conexoes_vivas() {
        let (reg, d) = registro("teto");
        let ponte = Ponte::abrir(
            Conf {
                porta: 0,
                udp: 9,
                port_share: None,
            },
            reg,
        )
        .unwrap();
        let mut abertas = Vec::new();
        for _ in 0..MAX_CONEXOES {
            abertas.push(TcpStream::connect(("127.0.0.1", ponte.porta())).unwrap());
        }
        let fim = std::time::Instant::now() + Duration::from_secs(10);
        while ponte.vivas.lock().unwrap().len() < MAX_CONEXOES && std::time::Instant::now() < fim {
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut sobra = TcpStream::connect(("127.0.0.1", ponte.porta())).unwrap();
        sobra
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut b = [0u8; 1];
        assert!(
            matches!(sobra.read(&mut b), Ok(0)),
            "a conexao alem do teto foi atendida"
        );
        drop(abertas);
        drop(ponte);
        let _ = std::fs::remove_dir_all(&d);
    }
}
