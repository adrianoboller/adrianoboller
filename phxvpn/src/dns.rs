//! Resolvedor minimo da rede (modo servidor): responde `<membro>.<rede>.phx`
//! com o IP do membro na VPN e REPASSA o resto ao DNS do sistema (ou ao da
//! empresa). O OpenVPN nao tem DNS proprio -- so empurra o endereco de um
//! (`dhcp-option DNS`) --, e o nome dos membros era o que faltava para a
//! rede parecer uma LAN (`\\ana.matriz.phx\pasta`).
//!
//! # Hipoteses (papel J, 24/09/2026)
//!
//! - **So empurrar o DNS da empresa: morreu como resposta inteira.** Resolve
//!   o nome externo e o interno da empresa, mas nao os membros: o IP deles
//!   vive no painel, e o DNS da empresa nao sabe dele (teria de ser
//!   atualizado por fora a cada entrada). Fica como opcao (`dns_empresa`),
//!   e vira o repasse quando os nomes estao ligados.
//! - **Resolvedor embutido so da zona, com repasse: venceu.** E o desenho do
//!   MagicDNS do Tailscale e do ZeroNSd do ZeroTier (os dois tiveram de
//!   escrever um): uma zona pequena, autoritativa, e o resto adiante. Aqui
//!   cabe em `std` -- so registro A, UDP, sem cache.
//!
//! # O que impede de virar resolvedor aberto
//!
//! Um soquete por rede, preso a `10.77.N.1:53` (o IP do servidor dentro do
//! tunel daquela rede), e so responde a origem `10.77.N.0/24`. O OpenVPN
//! descarta pacote de membro com origem que nao e a dele (`ccd`), entao a
//! origem e confiavel: um membro da rede A que pergunte ao `10.77.B.1` (o
//! kernel entrega, e endereco local) fica sem resposta -- nem os nomes da
//! rede B, nem repasse.
//!
//! # De onde vem os nomes
//!
//! Do `ccd/` da rede, que e exatamente quem pode conectar (`ccd-exclusive`):
//! o arquivo tem o CN (`login.rede.serie`) no nome e o `ifconfig-push` com o
//! IP dentro. Um motor so -- quem sai, e desativado ou removido some do DNS
//! no mesmo passo em que perde o acesso, sem segunda lista para esquecer.

use crate::rotas::Cidr;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Validade da resposta local: curta, porque membro entra e sai.
const TTL: u32 = 30;
/// Repasses simultaneos por resolvedor: cada um e uma thread esperando o
/// DNS de cima por ate `PRAZO_REPASSE`.
const TETO_REPASSES: usize = 64;
const PRAZO_REPASSE: Duration = Duration::from_secs(3);
/// Idade maxima da tabela de nomes antes de reler o `ccd/` (o `mtime` do
/// diretorio ja avisa; isto cobre o sistema de arquivos de segundo inteiro).
const IDADE_NOMES: Duration = Duration::from_secs(5);

/// Um resolvedor: onde escuta, de quem aceita, a zona e de onde le os nomes.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub escuta: SocketAddr,
    pub aceita: Cidr,
    /// `matriz.phx`, sem ponto final, em minusculas.
    pub zona: String,
    pub ccd: PathBuf,
    /// O DNS de cima; sem ele, nome de fora da zona recebe SERVFAIL.
    pub repasse: Option<SocketAddr>,
}

/// Rotulo DNS de um texto do painel (nome de rede, login): minusculas,
/// acento tirado, o que nao for letra ou digito vira hifen. `None` se nao
/// sobra nada.
pub fn rotulo(t: &str) -> Option<String> {
    let mut s = String::new();
    for c in t.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        };
        if c == '-' && (s.is_empty() || s.ends_with('-')) {
            continue;
        }
        s.push(c);
    }
    let s = s.trim_end_matches('-');
    let s = &s[..s.len().min(63)];
    let s = s.trim_end_matches('-');
    (!s.is_empty()).then(|| s.to_string())
}

/// A zona de uma rede: `<rotulo do nome>.phx`.
pub fn zona(nome_rede: &str, octeto: u8) -> String {
    format!(
        "{}.phx",
        rotulo(nome_rede).unwrap_or_else(|| format!("rede{octeto}"))
    )
}

/// O primeiro `nameserver` IPv4 de um `resolv.conf`.
pub fn dns_do_sistema(resolv_conf: &str) -> Option<SocketAddr> {
    resolv_conf.lines().find_map(|l| {
        let mut p = l.split_whitespace();
        (p.next() == Some("nameserver"))
            .then(|| p.next()?.parse::<Ipv4Addr>().ok())
            .flatten()
            .map(|ip| SocketAddr::new(IpAddr::V4(ip), 53))
    })
}

/// Os nomes do `ccd/`: rotulo do login -> IPs. Arquivo pela metade da
/// gravacao (`.gravando`) nao conta.
pub fn nomes_do_ccd(dir: &Path) -> HashMap<String, Vec<Ipv4Addr>> {
    let mut m: HashMap<String, Vec<Ipv4Addr>> = HashMap::new();
    let Ok(ls) = std::fs::read_dir(dir) else {
        return m;
    };
    for e in ls.flatten() {
        let nome = e.file_name().to_string_lossy().into_owned();
        if nome.ends_with(".gravando") {
            continue;
        }
        // CN = login.rede.serie; o login pode ter ponto.
        let mut partes = nome.rsplitn(3, '.');
        let (Some(_serie), Some(_rede), Some(login)) =
            (partes.next(), partes.next(), partes.next())
        else {
            continue;
        };
        let Some(r) = rotulo(login) else { continue };
        let Ok(texto) = std::fs::read_to_string(e.path()) else {
            continue;
        };
        let ip = texto.lines().find_map(|l| {
            let mut p = l.split_whitespace();
            (p.next() == Some("ifconfig-push"))
                .then(|| p.next()?.parse::<Ipv4Addr>().ok())
                .flatten()
        });
        if let Some(ip) = ip {
            let v = m.entry(r).or_default();
            if !v.contains(&ip) {
                v.push(ip);
            }
        }
    }
    m
}

/// O que fazer com um datagrama recebido.
#[derive(Debug, PartialEq)]
pub enum Decisao {
    Responder(Vec<u8>),
    /// Fora da zona: vai ao DNS de cima como veio.
    Repassar,
    /// Nao e pergunta (resposta, lixo curto): silencio.
    Ignorar,
}

const RC_FORMERR: u16 = 1;
const RC_SERVFAIL: u16 = 2;
const RC_NXDOMAIN: u16 = 3;
const RC_NOTIMP: u16 = 4;
const RC_REFUSED: u16 = 5;

fn u16_em(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// So o cabecalho, com o codigo de erro (sem a pergunta).
fn so_cabecalho(q: &[u8], rcode: u16) -> Vec<u8> {
    let rd = u16_em(q, 2) & 0x0100;
    let mut r = Vec::with_capacity(12);
    r.extend_from_slice(&q[..2]);
    r.extend_from_slice(&(0x8000 | 0x0080 | rd | rcode).to_be_bytes());
    r.extend_from_slice(&[0; 8]);
    r
}

/// Resposta a uma pergunta que nao se consegue repassar (sem DNS de cima,
/// ou teto de repasses cheio).
pub fn servfail(q: &[u8]) -> Option<Vec<u8>> {
    (q.len() >= 12).then(|| so_cabecalho(q, RC_SERVFAIL))
}

/// Decide a resposta. `nomes` so e chamado para pergunta DENTRO da zona.
pub fn responder(q: &[u8], zona: &str, nomes: &mut dyn FnMut(&str) -> Vec<Ipv4Addr>) -> Decisao {
    if q.len() < 12 {
        return Decisao::Ignorar;
    }
    let flags = u16_em(q, 2);
    if flags & 0x8000 != 0 {
        return Decisao::Ignorar;
    }
    if (flags >> 11) & 0xF != 0 {
        return Decisao::Responder(so_cabecalho(q, RC_NOTIMP));
    }
    if u16_em(q, 4) != 1 {
        return Decisao::Responder(so_cabecalho(q, RC_FORMERR));
    }
    // O nome da pergunta, rotulo a rotulo (sem ponteiro: pergunta nao
    // comprime).
    let mut i = 12;
    let mut rotulos: Vec<String> = Vec::new();
    loop {
        let Some(&n) = q.get(i) else {
            return Decisao::Responder(so_cabecalho(q, RC_FORMERR));
        };
        i += 1;
        if n == 0 {
            break;
        }
        if n > 63 || i + n as usize > q.len() || i > 255 {
            return Decisao::Responder(so_cabecalho(q, RC_FORMERR));
        }
        rotulos.push(String::from_utf8_lossy(&q[i..i + n as usize]).to_ascii_lowercase());
        i += n as usize;
    }
    if i + 4 > q.len() {
        return Decisao::Responder(so_cabecalho(q, RC_FORMERR));
    }
    let (tipo, classe) = (u16_em(q, i), u16_em(q, i + 2));
    let fim_pergunta = i + 4;
    let nome = rotulos.join(".");
    let dentro = nome == zona || nome.ends_with(&format!(".{zona}"));
    if !dentro {
        return Decisao::Repassar;
    }
    let pergunta = &q[12..fim_pergunta];
    let monta = |rcode: u16, ips: &[Ipv4Addr]| {
        let rd = u16_em(q, 2) & 0x0100;
        let mut r = Vec::with_capacity(fim_pergunta + ips.len() * 16);
        r.extend_from_slice(&q[..2]);
        // QR, AA (a zona e nossa), RD copiado, RA.
        r.extend_from_slice(&(0x8000 | 0x0400 | rd | 0x0080 | rcode).to_be_bytes());
        r.extend_from_slice(&1u16.to_be_bytes());
        r.extend_from_slice(&(ips.len() as u16).to_be_bytes());
        r.extend_from_slice(&[0; 4]);
        r.extend_from_slice(pergunta);
        for ip in ips {
            // Ponteiro para o nome da pergunta (deslocamento 12).
            r.extend_from_slice(&[0xC0, 0x0C, 0, 1, 0, 1]);
            r.extend_from_slice(&TTL.to_be_bytes());
            r.extend_from_slice(&4u16.to_be_bytes());
            r.extend_from_slice(&ip.octets());
        }
        r
    };
    if classe != 1 && classe != 255 {
        return Decisao::Responder(monta(RC_REFUSED, &[]));
    }
    if nome == zona {
        return Decisao::Responder(monta(0, &[]));
    }
    let membro = &nome[..nome.len() - zona.len() - 1];
    if membro.contains('.') {
        return Decisao::Responder(monta(RC_NXDOMAIN, &[]));
    }
    let ips = nomes(membro);
    if ips.is_empty() {
        return Decisao::Responder(monta(RC_NXDOMAIN, &[]));
    }
    // AAAA e outros tipos: o nome existe, sem dado daquele tipo (NODATA) --
    // NXDOMAIN aqui faria o cliente desistir do A tambem.
    if tipo == 1 || tipo == 255 {
        Decisao::Responder(monta(0, &ips))
    } else {
        Decisao::Responder(monta(0, &[]))
    }
}

/// So a rede do proprio resolvedor pergunta a ele.
pub fn origem_aceita(de: &SocketAddr, aceita: &Cidr) -> bool {
    match de.ip() {
        IpAddr::V4(ip) => aceita.contem_ip(u32::from(ip)),
        IpAddr::V6(_) => false,
    }
}

/// A tabela de nomes, relida quando o `ccd/` muda.
struct Nomes {
    dir: PathBuf,
    visto: Option<(SystemTime, Instant)>,
    tabela: HashMap<String, Vec<Ipv4Addr>>,
}

impl Nomes {
    fn buscar(&mut self, membro: &str) -> Vec<Ipv4Addr> {
        let mtime = std::fs::metadata(&self.dir).and_then(|m| m.modified()).ok();
        let velho = match (self.visto, mtime) {
            (Some((m, quando)), Some(agora)) => m != agora || quando.elapsed() > IDADE_NOMES,
            _ => true,
        };
        if velho {
            self.tabela = nomes_do_ccd(&self.dir);
            self.visto = mtime.map(|m| (m, Instant::now()));
        }
        self.tabela.get(membro).cloned().unwrap_or_default()
    }
}

/// O laco de um resolvedor, no soquete ja aberto. Volta quando `parar`.
pub fn servir(sock: UdpSocket, cfg: &Config, parar: &AtomicBool) {
    let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
    let em_voo = Arc::new(AtomicUsize::new(0));
    let mut nomes = Nomes {
        dir: cfg.ccd.clone(),
        visto: None,
        tabela: HashMap::new(),
    };
    let mut buf = [0u8; 4096];
    while !parar.load(Ordering::Relaxed) {
        let Ok((n, de)) = sock.recv_from(&mut buf) else {
            continue;
        };
        if !origem_aceita(&de, &cfg.aceita) {
            continue;
        }
        let q = &buf[..n];
        match responder(q, &cfg.zona, &mut |m| nomes.buscar(m)) {
            Decisao::Ignorar => {}
            Decisao::Responder(r) => {
                let _ = sock.send_to(&r, de);
            }
            Decisao::Repassar => {
                let (Some(cima), Ok(volta)) = (cfg.repasse, sock.try_clone()) else {
                    if let Some(r) = servfail(q) {
                        let _ = sock.send_to(&r, de);
                    }
                    continue;
                };
                if em_voo.fetch_add(1, Ordering::SeqCst) >= TETO_REPASSES {
                    em_voo.fetch_sub(1, Ordering::SeqCst);
                    if let Some(r) = servfail(q) {
                        let _ = sock.send_to(&r, de);
                    }
                    continue;
                }
                let q = q.to_vec();
                let em_voo = em_voo.clone();
                std::thread::spawn(move || {
                    if let Some(r) = repassar(&q, cima) {
                        let _ = volta.send_to(&r, de);
                    } else if let Some(r) = servfail(&q) {
                        let _ = volta.send_to(&r, de);
                    }
                    em_voo.fetch_sub(1, Ordering::SeqCst);
                });
            }
        }
    }
}

/// Uma pergunta ao DNS de cima, por um soquete proprio: a resposta so vale
/// com o MESMO id (o resto e lixo ou tentativa de envenenar).
fn repassar(q: &[u8], cima: SocketAddr) -> Option<Vec<u8>> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(cima).ok()?;
    s.set_read_timeout(Some(PRAZO_REPASSE)).ok()?;
    s.send(q).ok()?;
    let fim = Instant::now() + PRAZO_REPASSE;
    let mut buf = [0u8; 4096];
    while Instant::now() < fim {
        let n = s.recv(&mut buf).ok()?;
        if n >= 12 && buf[..2] == q[..2] {
            return Some(buf[..n].to_vec());
        }
    }
    None
}

struct Ativo {
    cfg: Config,
    parar: Arc<AtomicBool>,
}

static ATIVOS: Mutex<Vec<Ativo>> = Mutex::new(Vec::new());

/// Deixa no ar exatamente os resolvedores de `cfgs`: para os que sairam ou
/// mudaram, sobe os novos. O endereco `10.77.N.1` so existe com o OpenVPN
/// da rede no ar, entao a thread tenta abrir o soquete ate conseguir (ou
/// ate mandarem parar).
pub fn acertar(cfgs: Vec<Config>) {
    let mut ativos = ATIVOS.lock().unwrap_or_else(|e| e.into_inner());
    ativos.retain(|a| {
        let fica = cfgs.contains(&a.cfg);
        if !fica {
            a.parar.store(true, Ordering::SeqCst);
        }
        fica
    });
    for cfg in cfgs {
        if ativos.iter().any(|a| a.cfg == cfg) {
            continue;
        }
        let parar = Arc::new(AtomicBool::new(false));
        let (c, p) = (cfg.clone(), parar.clone());
        std::thread::spawn(move || {
            let mut avisou = false;
            let sock = loop {
                if p.load(Ordering::SeqCst) {
                    return;
                }
                match UdpSocket::bind(c.escuta) {
                    Ok(s) => break s,
                    Err(e) => {
                        if !avisou {
                            eprintln!("phxvpn: DNS da zona {} espera {} ({e})", c.zona, c.escuta);
                            avisou = true;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            };
            eprintln!("phxvpn: DNS da zona {} em {}", c.zona, c.escuta);
            servir(sock, &c, &p);
        });
        ativos.push(Ativo { cfg, parar });
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Pergunta DNS crua: id 0x1234, RD, um nome, tipo e classe IN.
    fn pergunta(nome: &str, tipo: u16) -> Vec<u8> {
        let mut q = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        for r in nome.split('.') {
            q.push(r.len() as u8);
            q.extend_from_slice(r.as_bytes());
        }
        q.push(0);
        q.extend_from_slice(&tipo.to_be_bytes());
        q.extend_from_slice(&1u16.to_be_bytes());
        q
    }

    fn rcode(r: &[u8]) -> u16 {
        u16_em(r, 2) & 0xF
    }

    fn ips_da_resposta(r: &[u8], q_len: usize) -> Vec<Ipv4Addr> {
        let n = u16_em(r, 6) as usize;
        (0..n)
            .map(|k| {
                let i = q_len + k * 16 + 12;
                Ipv4Addr::new(r[i], r[i + 1], r[i + 2], r[i + 3])
            })
            .collect()
    }

    fn tabela(m: &str) -> Vec<Ipv4Addr> {
        match m {
            "ana" => vec![Ipv4Addr::new(10, 77, 1, 2)],
            _ => vec![],
        }
    }

    #[test]
    fn rotulo_tira_acento_e_o_que_nao_cabe_em_dns() {
        assert_eq!(rotulo("Matriz").as_deref(), Some("matriz"));
        assert_eq!(
            rotulo("Escritório São Paulo").as_deref(),
            Some("escritorio-sao-paulo")
        );
        assert_eq!(rotulo("joao.silva").as_deref(), Some("joao-silva"));
        assert_eq!(rotulo("--x__y--").as_deref(), Some("x-y"));
        assert_eq!(rotulo("!!!"), None);
        assert_eq!(rotulo(&"a".repeat(80)).unwrap().len(), 63);
        assert_eq!(zona("!!!", 7), "rede7.phx");
        assert_eq!(zona("Matriz", 1), "matriz.phx");
    }

    #[test]
    fn membro_da_zona_responde_o_ip_e_o_resto_vai_adiante() {
        let q = pergunta("ANA.matriz.phx", 1);
        let Decisao::Responder(r) = responder(&q, "matriz.phx", &mut tabela) else {
            panic!("nao respondeu")
        };
        assert_eq!(&r[..2], &[0x12, 0x34]);
        assert_eq!(u16_em(&r, 2) & 0x8400, 0x8400, "QR e AA");
        assert_eq!(rcode(&r), 0);
        assert_eq!(
            ips_da_resposta(&r, q.len()),
            vec![Ipv4Addr::new(10, 77, 1, 2)]
        );
        // Quem nao existe: NXDOMAIN; AAAA de quem existe: NODATA.
        let Decisao::Responder(r) =
            responder(&pergunta("bia.matriz.phx", 1), "matriz.phx", &mut tabela)
        else {
            panic!()
        };
        assert_eq!(rcode(&r), RC_NXDOMAIN);
        let Decisao::Responder(r) =
            responder(&pergunta("ana.matriz.phx", 28), "matriz.phx", &mut tabela)
        else {
            panic!()
        };
        assert_eq!((rcode(&r), u16_em(&r, 6)), (0, 0));
        // Fora da zona -- inclusive o sufixo parecido -- vai ao DNS de cima.
        for fora in ["exemplo.com.br", "ana.outramatriz.phx", "phx"] {
            assert_eq!(
                responder(&pergunta(fora, 1), "matriz.phx", &mut tabela),
                Decisao::Repassar,
                "{fora}"
            );
        }
    }

    #[test]
    fn pergunta_torta_nao_derruba_nem_repassa() {
        let mut nunca = |_: &str| -> Vec<Ipv4Addr> { panic!("consultou a tabela") };
        assert_eq!(
            responder(&[1, 2, 3], "matriz.phx", &mut nunca),
            Decisao::Ignorar
        );
        let mut resposta = pergunta("ana.matriz.phx", 1);
        resposta[2] |= 0x80;
        assert_eq!(
            responder(&resposta, "matriz.phx", &mut nunca),
            Decisao::Ignorar
        );
        let mut cortada = pergunta("ana.matriz.phx", 1);
        cortada.truncate(20);
        let Decisao::Responder(r) = responder(&cortada, "matriz.phx", &mut nunca) else {
            panic!()
        };
        assert_eq!(rcode(&r), RC_FORMERR);
        let mut duas = pergunta("ana.matriz.phx", 1);
        duas[5] = 2;
        let Decisao::Responder(r) = responder(&duas, "matriz.phx", &mut nunca) else {
            panic!()
        };
        assert_eq!(rcode(&r), RC_FORMERR);
    }

    #[test]
    fn nomes_saem_do_ccd_e_so_dos_arquivos_inteiros() {
        let d = std::env::temp_dir().join(format!("phxvpn-dns-ccd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("ana.1.0a1b2c3d"),
            "ifconfig-push 10.77.1.2 255.255.255.0\n",
        )
        .unwrap();
        std::fs::write(
            d.join("joao.silva.1.ffee0011"),
            "ifconfig-push 10.77.1.3 255.255.255.0\niroute 192.168.20.0 255.255.255.0\n",
        )
        .unwrap();
        std::fs::write(
            d.join("bia.1.gravando"),
            "ifconfig-push 10.77.1.9 255.255.255.0\n",
        )
        .unwrap();
        let m = nomes_do_ccd(&d);
        assert_eq!(m.get("ana"), Some(&vec![Ipv4Addr::new(10, 77, 1, 2)]));
        assert_eq!(
            m.get("joao-silva"),
            Some(&vec![Ipv4Addr::new(10, 77, 1, 3)])
        );
        assert!(!m.contains_key("bia"), "arquivo pela metade entrou");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dns_do_sistema_pega_o_primeiro_ipv4() {
        let t = "# gerado\nsearch x\nnameserver ::1\nnameserver 127.0.0.53\nnameserver 8.8.8.8\n";
        assert_eq!(dns_do_sistema(t), Some("127.0.0.53:53".parse().unwrap()));
        assert_eq!(dns_do_sistema("search x\n"), None);
    }

    /// De ponta a ponta no soquete: a zona responde, o resto vai ao DNS de
    /// cima (e volta com o mesmo id), e quem esta FORA da rede do resolvedor
    /// nao recebe nada -- nem a zona, nem o repasse.
    #[test]
    fn resolvedor_responde_repassa_e_ignora_quem_e_de_fora() {
        let d = std::env::temp_dir().join(format!("phxvpn-dns-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("ana.1.0a1b2c3d"),
            "ifconfig-push 10.77.1.2 255.255.255.0\n",
        )
        .unwrap();
        // DNS de cima de mentira: devolve a pergunta com QR e um byte a mais.
        let cima = UdpSocket::bind("127.0.0.1:0").unwrap();
        let end_cima = cima.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut b = [0u8; 512];
            while let Ok((n, de)) = cima.recv_from(&mut b) {
                let mut r = b[..n].to_vec();
                r[2] |= 0x80;
                r.push(0xEE);
                let _ = cima.send_to(&r, de);
            }
        });
        let sobe = |aceita: &str| {
            let s = UdpSocket::bind("127.0.0.1:0").unwrap();
            let cfg = Config {
                escuta: s.local_addr().unwrap(),
                aceita: Cidr::analisar(aceita).unwrap(),
                zona: "matriz.phx".into(),
                ccd: d.clone(),
                repasse: Some(end_cima),
            };
            let end = cfg.escuta;
            let parar = Arc::new(AtomicBool::new(false));
            let p = parar.clone();
            std::thread::spawn(move || servir(s, &cfg, &p));
            (end, parar)
        };
        let perguntar = |end: SocketAddr, q: &[u8]| -> Option<Vec<u8>> {
            let c = UdpSocket::bind("127.0.0.1:0").unwrap();
            c.set_read_timeout(Some(Duration::from_millis(800)))
                .unwrap();
            c.send_to(q, end).unwrap();
            let mut b = [0u8; 512];
            c.recv(&mut b).ok().map(|n| b[..n].to_vec())
        };
        let (dentro, parar) = sobe("127.0.0.0/8");
        let q = pergunta("ana.matriz.phx", 1);
        let r = perguntar(dentro, &q).expect("a zona nao respondeu");
        assert_eq!(
            ips_da_resposta(&r, q.len()),
            vec![Ipv4Addr::new(10, 77, 1, 2)]
        );
        let q = pergunta("exemplo.com.br", 1);
        let r = perguntar(dentro, &q).expect("o repasse nao voltou");
        assert_eq!(r.last(), Some(&0xEE), "nao veio do DNS de cima");
        parar.store(true, Ordering::SeqCst);
        // O mesmo resolvedor, aceitando so a rede 10.77.1.0/24: a pergunta
        // vinda de 127.0.0.1 (outra rede) fica sem resposta.
        let (so_rede, parar) = sobe("10.77.1.0/24");
        assert_eq!(perguntar(so_rede, &pergunta("ana.matriz.phx", 1)), None);
        assert_eq!(perguntar(so_rede, &pergunta("exemplo.com.br", 1)), None);
        parar.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_dir_all(&d);
    }
}
