//! USB pela rede virtual: um pendrive, um token ou uma impressora espetada
//! num membro aparece no outro como se estivesse espetada ali.
//!
//! # O mecanismo
//!
//! O protocolo e o USB/IP do kernel Linux (`Documentation/usb/usbip_protocol.rst`),
//! porta TCP 3240. Duas mensagens de combinado e, depois, o proprio kernel
//! assume o soquete e carrega as URBs:
//!
//! * `OP_REQ_DEVLIST` -- «o que voce compartilha?»;
//! * `OP_REQ_IMPORT` -- «me da o `1-1.2`»: quem compartilha entrega o
//!   descritor do soquete ao driver `usbip-host` pelo sysfs, quem usa entrega
//!   o seu ao `vhci_hcd`, e dali em diante e kernel com kernel.
//!
//! A parte de usuario (esta) e escrita aqui, so com `std`, falando com o
//! kernel pelo sysfs -- do mesmo jeito que o `usbip`/`usbipd` de referencia,
//! com quem este modulo interopera (prova no `docs/PHXVPN.md`). No Windows so
//! ha o lado de quem USA, pelo `usbip-win2` (driver de fora, autorizado pelo
//! dono em 24/09/2026 como excecao a pétrea de zero dependencias).
//!
//! # Por que so dentro da rede
//!
//! O USB/IP nao tem autenticacao nem cifra: quem alcanca a porta 3240 le o
//! pendrive. Por isso o servidor daqui:
//!
//! 1. escuta SO no IP virtual e, no Linux, preso a placa da rede
//!    (`SO_BINDTODEVICE`) -- pacote que chega pela placa fisica com destino
//!    ao IP virtual (modelo de host fraco do Linux) nao entra;
//! 2. so atende IP que e de um par da rede -- e no P2P a origem ja foi
//!    conferida contra a chave de quem mandou (`p2p.rs`);
//! 3. so oferece o que foi compartilhado NESTA rede (`usb` no arquivo da
//!    rede), mesmo que o dispositivo esteja preso ao `usbip-host`.
//!
//! O trafego vai cifrado porque vai pelo tunel.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub type R<T> = Result<T, String>;

pub const PORTA: u16 = 3240;
const VERSAO: u16 = 0x0111;
const OP_REQ_DEVLIST: u16 = 0x8005;
const OP_REP_DEVLIST: u16 = 0x0005;
const OP_REQ_IMPORT: u16 = 0x8003;
const OP_REP_IMPORT: u16 = 0x0003;
const ST_OK: u32 = 0;
/// «Dispositivo nao disponivel» -- a unica recusa que se manda: nao se diz
/// se o busid existe e nao foi compartilhado, ou se esta em uso.
const ST_NA: u32 = 1;
/// `usb_device` no fio: caminho 256 + busid 32 + 3x u32 + 3x u16 + 6x u8.
pub const TAM_DISPOSITIVO: usize = 312;
/// Resposta de lista com mais que isso e torta: nao se aloca pelo que o
/// outro lado diz.
const TETO_DISPOSITIVOS: u32 = 256;
const TETO_INTERFACES: u8 = 32;
/// Prazo do combinado. Tem de SAIR antes de o soquete ir ao kernel: com
/// `SO_RCVTIMEO` ligado, a thread de recepcao do kernel desiste na primeira
/// pausa do dispositivo e derruba a conexao.
const PRAZO: Duration = Duration::from_secs(10);

/// `usbip_status` do `usbip-host`: 1 = livre para exportar.
const SDEV_ST_AVAILABLE: u32 = 1;
/// `sta` do `vhci_hcd`: 4 = porta vazia.
const VDEV_ST_NULL: u32 = 4;
const VDEV_ST_USED: u32 = 6;

#[derive(Clone, Debug, PartialEq)]
pub struct Interface {
    pub classe: u8,
    pub subclasse: u8,
    pub protocolo: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dispositivo {
    pub caminho: String,
    pub busid: String,
    pub busnum: u32,
    pub devnum: u32,
    /// `enum usb_device_speed`: 1 baixa, 2 plena, 3 alta, 5 super, 6 super+.
    pub velocidade: u32,
    pub fabricante: u16,
    pub produto: u16,
    pub bcd: u16,
    pub classe: u8,
    pub subclasse: u8,
    pub protocolo: u8,
    pub configuracao: u8,
    pub n_configuracoes: u8,
    pub n_interfaces: u8,
    pub interfaces: Vec<Interface>,
    /// Nome legivel (`product` do sysfs). So local: nao viaja no protocolo.
    pub nome: String,
}

impl Dispositivo {
    /// O `devid` que o `vhci_hcd` pede no `attach`.
    pub fn devid(&self) -> u32 {
        (self.busnum << 16) | (self.devnum & 0xFFFF)
    }

    pub fn para_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(TAM_DISPOSITIVO);
        b.extend_from_slice(&texto_fixo(&self.caminho, 256));
        b.extend_from_slice(&texto_fixo(&self.busid, 32));
        for v in [self.busnum, self.devnum, self.velocidade] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        for v in [self.fabricante, self.produto, self.bcd] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&[
            self.classe,
            self.subclasse,
            self.protocolo,
            self.configuracao,
            self.n_configuracoes,
            self.n_interfaces,
        ]);
        b
    }

    pub fn de_bytes(b: &[u8]) -> R<Dispositivo> {
        if b.len() != TAM_DISPOSITIVO {
            return Err("dispositivo torto".into());
        }
        let u32_em = |i: usize| u32::from_be_bytes(b[i..i + 4].try_into().expect("4"));
        let u16_em = |i: usize| u16::from_be_bytes(b[i..i + 2].try_into().expect("2"));
        Ok(Dispositivo {
            caminho: de_texto_fixo(&b[..256]),
            busid: de_texto_fixo(&b[256..288]),
            busnum: u32_em(288),
            devnum: u32_em(292),
            velocidade: u32_em(296),
            fabricante: u16_em(300),
            produto: u16_em(302),
            bcd: u16_em(304),
            classe: b[306],
            subclasse: b[307],
            protocolo: b[308],
            configuracao: b[309],
            n_configuracoes: b[310],
            n_interfaces: b[311],
            interfaces: Vec::new(),
            nome: String::new(),
        })
    }

    /// Uma linha para gente: `1-1.2  0781:5583  alta  SanDisk Ultra`.
    pub fn resumo(&self) -> String {
        let nome = if self.nome.is_empty() {
            classe_legivel(self)
        } else {
            self.nome.clone()
        };
        format!(
            "{:<10} {:04x}:{:04x}  {:<6} {nome}",
            self.busid,
            self.fabricante,
            self.produto,
            velocidade_legivel(self.velocidade)
        )
    }
}

pub fn velocidade_legivel(v: u32) -> &'static str {
    match v {
        1 => "baixa",
        2 => "plena",
        3 => "alta",
        4 => "sem fio",
        5 => "super",
        6 => "super+",
        _ => "?",
    }
}

fn classe_legivel(d: &Dispositivo) -> String {
    // Classe 0: cada interface diz a sua; a primeira basta para gente.
    let c = if d.classe == 0 {
        d.interfaces.first().map(|i| i.classe).unwrap_or(0)
    } else {
        d.classe
    };
    match c {
        0x01 => "audio",
        0x02 | 0x0a => "comunicacao",
        0x03 => "teclado/mouse (HID)",
        0x06 => "imagem",
        0x07 => "impressora",
        0x08 => "armazenamento",
        0x0b => "cartao inteligente",
        0x0e => "video",
        0xe0 => "sem fio",
        0xff => "do fabricante",
        _ => "dispositivo",
    }
    .to_string()
}

fn texto_fixo(t: &str, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    let t = t.as_bytes();
    let k = t.len().min(n - 1);
    b[..k].copy_from_slice(&t[..k]);
    b
}

fn de_texto_fixo(b: &[u8]) -> String {
    let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..fim]).into_owned()
}

fn cabecalho(codigo: u16, status: u32) -> [u8; 8] {
    let mut h = [0u8; 8];
    h[..2].copy_from_slice(&VERSAO.to_be_bytes());
    h[2..4].copy_from_slice(&codigo.to_be_bytes());
    h[4..].copy_from_slice(&status.to_be_bytes());
    h
}

fn ler_cabecalho(r: &mut impl Read) -> R<(u16, u16, u32)> {
    let mut h = [0u8; 8];
    r.read_exact(&mut h).map_err(|e| format!("usb/ip: {e}"))?;
    Ok((
        u16::from_be_bytes([h[0], h[1]]),
        u16::from_be_bytes([h[2], h[3]]),
        u32::from_be_bytes(h[4..].try_into().expect("4")),
    ))
}

/// `1-1`, `3-2.4.1`: barramento, traco, portas com ponto. E o que vai virar
/// caminho no sysfs, entao nada de `/` nem `..` -- nem vindo da rede.
pub fn validar_busid(b: &str) -> R<()> {
    let ok = b.len() < 32
        && b.split_once('-').is_some_and(|(bus, portas)| {
            !bus.is_empty()
                && bus.bytes().all(|c| c.is_ascii_digit())
                && !portas.is_empty()
                && portas
                    .split('.')
                    .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
        });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "busid invalido: {b:?} (esperado algo como 1-1 ou 1-1.2)"
        ))
    }
}

// ------------------------------------------------------------- sysfs ----

/// A porta do `vhci_hcd` (o controlador virtual de quem USA).
#[derive(Clone, Debug, PartialEq)]
pub struct Porta {
    /// `hs` (ate USB 2) ou `ss` (USB 3).
    pub hub: String,
    pub numero: u32,
    pub estado: u32,
    pub busid_local: String,
}

impl Porta {
    pub fn em_uso(&self) -> bool {
        self.estado == VDEV_ST_USED
    }
}

/// O kernel visto pelo sysfs. A raiz e parametro para os testes montarem um
/// sysfs de mentira numa pasta e conferirem o que se escreveu nele.
#[derive(Clone, Debug)]
pub struct Sysfs {
    raiz: PathBuf,
    /// Onde o `usbip` de referencia anota quem esta em cada porta; escrever
    /// no mesmo formato deixa o `usbip port` mostrar o que o phxvpn anexou.
    registro: PathBuf,
}

impl Sysfs {
    pub fn sistema() -> Sysfs {
        Sysfs {
            raiz: PathBuf::from("/sys"),
            registro: PathBuf::from("/var/run/vhci_hcd"),
        }
    }

    /// O do sistema, ou o de `PHXVPN_SYSFS` (um sysfs de mentira: e como a
    /// janela, o console e o servidor da rede se exercitam sem USB). Um
    /// lugar so decide, para as tres portas nunca olharem kernels diferentes.
    pub fn do_ambiente() -> Sysfs {
        match std::env::var("PHXVPN_SYSFS") {
            Ok(r) if !r.is_empty() => Sysfs::em(Path::new(&r)),
            _ => Sysfs::sistema(),
        }
    }

    pub fn em(raiz: &Path) -> Sysfs {
        Sysfs {
            raiz: raiz.to_path_buf(),
            registro: raiz.join("run-vhci_hcd"),
        }
    }

    fn dispositivos(&self) -> PathBuf {
        self.raiz.join("bus/usb/devices")
    }

    fn host(&self) -> PathBuf {
        self.raiz.join("bus/usb/drivers/usbip-host")
    }

    fn vhci(&self) -> PathBuf {
        self.raiz.join("devices/platform/vhci_hcd.0")
    }

    fn escrever(&self, arquivo: &Path, texto: &str) -> R<()> {
        std::fs::write(arquivo, texto).map_err(|e| {
            let dica = if e.kind() == std::io::ErrorKind::PermissionDenied {
                " (precisa de administrador: root)"
            } else if e.kind() == std::io::ErrorKind::NotFound {
                " (módulo do kernel carregado? modprobe usbip-host / vhci-hcd)"
            } else {
                ""
            };
            format!("{}: {e}{dica}", arquivo.display())
        })
    }

    /// O driver ao qual o dispositivo esta preso, se algum.
    pub fn driver(&self, busid: &str) -> Option<String> {
        let alvo = std::fs::read_link(self.dispositivos().join(busid).join("driver")).ok()?;
        alvo.file_name().map(|n| n.to_string_lossy().into_owned())
    }

    pub fn ler(&self, busid: &str) -> R<Dispositivo> {
        validar_busid(busid)?;
        let dir = self.dispositivos().join(busid);
        if !dir.is_dir() {
            return Err(format!("não há dispositivo USB {busid} neste computador"));
        }
        let at = |nome: &str| -> String {
            std::fs::read_to_string(dir.join(nome))
                .map(|t| t.trim().to_string())
                .unwrap_or_default()
        };
        let dec = |nome: &str| at(nome).parse::<u32>().unwrap_or(0);
        let hex = |nome: &str| u32::from_str_radix(&at(nome), 16).unwrap_or(0);
        let mut interfaces: Vec<(u32, Interface)> = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                let (_, resto) = n.strip_prefix(busid)?.split_once(':')?;
                let num: u32 = resto.split_once('.')?.1.parse().ok()?;
                let lh = |a: &str| {
                    std::fs::read_to_string(e.path().join(a))
                        .ok()
                        .and_then(|t| u8::from_str_radix(t.trim(), 16).ok())
                        .unwrap_or(0)
                };
                Some((
                    num,
                    Interface {
                        classe: lh("bInterfaceClass"),
                        subclasse: lh("bInterfaceSubClass"),
                        protocolo: lh("bInterfaceProtocol"),
                    },
                ))
            })
            .collect();
        interfaces.sort_by_key(|(n, _)| *n);
        let velocidade = match at("speed").as_str() {
            "1.5" => 1,
            "12" => 2,
            "480" => 3,
            "53.3-480" => 4,
            "5000" => 5,
            "10000" | "20000" => 6,
            _ => 0,
        };
        Ok(Dispositivo {
            caminho: std::fs::canonicalize(&dir)
                .unwrap_or(dir.clone())
                .to_string_lossy()
                .into_owned(),
            busid: busid.to_string(),
            busnum: dec("busnum"),
            devnum: dec("devnum"),
            velocidade,
            fabricante: hex("idVendor") as u16,
            produto: hex("idProduct") as u16,
            bcd: hex("bcdDevice") as u16,
            classe: hex("bDeviceClass") as u8,
            subclasse: hex("bDeviceSubClass") as u8,
            protocolo: hex("bDeviceProtocol") as u8,
            configuracao: dec("bConfigurationValue") as u8,
            n_configuracoes: dec("bNumConfigurations") as u8,
            n_interfaces: dec("bNumInterfaces") as u8,
            interfaces: interfaces.into_iter().map(|(_, i)| i).collect(),
            nome: {
                let f = at("manufacturer");
                let p = at("product");
                [f, p]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            },
        })
    }

    /// Os dispositivos espetados aqui, sem os hubs (compartilhar hub nao
    /// faz sentido e o `usbip` de referencia tambem os esconde).
    pub fn locais(&self) -> R<Vec<Dispositivo>> {
        let mut v = Vec::new();
        let dir = match std::fs::read_dir(self.dispositivos()) {
            Ok(d) => d,
            Err(_) => return Ok(v),
        };
        for e in dir.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.contains(':') || n.starts_with("usb") || validar_busid(&n).is_err() {
                continue;
            }
            if let Ok(d) = self.ler(&n) {
                if d.classe != 0x09 {
                    v.push(d);
                }
            }
        }
        v.sort_by(|a, b| a.busid.cmp(&b.busid));
        Ok(v)
    }

    /// Solta o dispositivo do driver dele e o prende ao `usbip-host` --
    /// daqui em diante ESTE computador nao o ve mais, ate `parar`.
    pub fn compartilhar(&self, busid: &str) -> R<()> {
        validar_busid(busid)?;
        self.ler(busid)?;
        if !self.host().is_dir() {
            return Err("o módulo usbip-host não está carregado (sudo modprobe usbip-host)".into());
        }
        match self.driver(busid).as_deref() {
            Some("usbip-host") => return Ok(()),
            Some(_) => self.escrever(
                &self.dispositivos().join(busid).join("driver/unbind"),
                busid,
            )?,
            None => {}
        }
        self.escrever(&self.host().join("match_busid"), &format!("add {busid}"))?;
        self.escrever(&self.host().join("bind"), busid)
    }

    /// Devolve o dispositivo ao driver normal dele.
    pub fn parar(&self, busid: &str) -> R<()> {
        validar_busid(busid)?;
        if self.driver(busid).as_deref() == Some("usbip-host") {
            self.escrever(&self.host().join("unbind"), busid)?;
        }
        self.escrever(&self.host().join("match_busid"), &format!("del {busid}"))?;
        // `rebind` pede ao kernel que procure o driver de sempre.
        self.escrever(&self.host().join("rebind"), busid)
    }

    pub fn estado_host(&self, busid: &str) -> Option<u32> {
        std::fs::read_to_string(self.dispositivos().join(busid).join("usbip_status"))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    /// Entrega o soquete ao `usbip-host`: o kernel pega uma referencia e o
    /// descritor daqui pode fechar.
    pub fn entregar(&self, busid: &str, fd: i64) -> R<()> {
        validar_busid(busid)?;
        self.escrever(
            &self.dispositivos().join(busid).join("usbip_sockfd"),
            &fd.to_string(),
        )
    }

    pub fn portas(&self) -> R<Vec<Porta>> {
        let dir = self.vhci();
        let mut arquivos: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|_| {
                "o módulo vhci-hcd não está carregado (sudo modprobe vhci-hcd)".to_string()
            })?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("status"))
            })
            .collect();
        arquivos.sort();
        let mut v = Vec::new();
        for a in arquivos {
            let t = std::fs::read_to_string(&a).map_err(|e| e.to_string())?;
            for l in t.lines().skip(1) {
                let c: Vec<&str> = l.split_whitespace().collect();
                if c.len() < 3 {
                    continue;
                }
                let (Ok(numero), Ok(estado)) = (c[1].parse(), c[2].parse()) else {
                    continue;
                };
                v.push(Porta {
                    hub: c[0].to_string(),
                    numero,
                    estado,
                    busid_local: c.get(6).unwrap_or(&"").to_string(),
                });
            }
        }
        Ok(v)
    }

    /// Anexa o soquete ao controlador virtual numa porta vazia do hub certo
    /// (USB 3 no `ss`, o resto no `hs`) e devolve o numero da porta.
    pub fn anexar(&self, fd: i64, d: &Dispositivo, remoto: &str) -> R<u32> {
        let hub = if d.velocidade >= 5 { "ss" } else { "hs" };
        let porta = self
            .portas()?
            .into_iter()
            .find(|p| p.hub == hub && p.estado == VDEV_ST_NULL)
            .ok_or("nenhuma porta USB virtual livre")?
            .numero;
        self.escrever(
            &self.vhci().join("attach"),
            &format!("{porta} {fd} {} {}", d.devid(), d.velocidade),
        )?;
        let _ = std::fs::create_dir_all(&self.registro);
        let _ = std::fs::write(
            self.registro.join(format!("port{porta}")),
            format!("{remoto} {PORTA} {}\n", d.busid),
        );
        Ok(porta)
    }

    /// O controlador virtual existe? Sem ele nao ha o que listar em «em uso»
    /// -- e isso e o normal de quem so compartilha, nao um erro.
    pub fn tem_vhci(&self) -> bool {
        self.vhci().is_dir()
    }

    pub fn soltar(&self, porta: u32) -> R<()> {
        self.escrever(&self.vhci().join("detach"), &porta.to_string())?;
        let _ = std::fs::remove_file(self.registro.join(format!("port{porta}")));
        Ok(())
    }

    /// `ip busid` de quem esta na porta, pelo registro.
    pub fn origem(&self, porta: u32) -> Option<String> {
        let t = std::fs::read_to_string(self.registro.join(format!("port{porta}"))).ok()?;
        let c: Vec<&str> = t.split_whitespace().collect();
        (c.len() >= 3).then(|| format!("{} {}", c[0], c[2]))
    }
}

// ---------------------------------------------------------- servidor ----

type Filtro<T> = Arc<dyn Fn(T) -> bool + Send + Sync>;

/// Quem compartilha. As duas perguntas sao funcoes, e nao listas, para
/// valerem no instante da conexao: par que entrou agora ja usa, dispositivo
/// que deixou de ser compartilhado some sem reiniciar nada.
#[derive(Clone)]
pub struct Servidor {
    pub sysfs: Sysfs,
    pub permitido: Filtro<Ipv4Addr>,
    pub compartilhado: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl Servidor {
    fn exportados(&self) -> Vec<Dispositivo> {
        self.sysfs
            .locais()
            .unwrap_or_default()
            .into_iter()
            .filter(|d| self.oferece(&d.busid))
            .collect()
    }

    fn oferece(&self, busid: &str) -> bool {
        (self.compartilhado)(busid) && self.sysfs.driver(busid).as_deref() == Some("usbip-host")
    }

    /// Atende UMA conexao. Devolve o que houve, para o registro.
    pub fn atender(&self, mut c: TcpStream) -> R<String> {
        let de = match c.peer_addr() {
            Ok(SocketAddr::V4(a)) => *a.ip(),
            _ => return Err("conexao sem IPv4".into()),
        };
        if !(self.permitido)(de) {
            // Nem um byte: quem nao e da rede nao descobre nem a versao.
            return Err(format!("{de} nao e membro da rede; recusado"));
        }
        let _ = c.set_read_timeout(Some(PRAZO));
        let _ = c.set_write_timeout(Some(PRAZO));
        let (versao, codigo, _) = ler_cabecalho(&mut c)?;
        if versao != VERSAO {
            return Err(format!("{de}: versao USB/IP {versao:#06x} nao suportada"));
        }
        match codigo {
            OP_REQ_DEVLIST => {
                let lista = self.exportados();
                let mut b = cabecalho(OP_REP_DEVLIST, ST_OK).to_vec();
                b.extend_from_slice(&(lista.len() as u32).to_be_bytes());
                for d in &lista {
                    b.extend_from_slice(&d.para_bytes());
                    for i in d.interfaces.iter().take(d.n_interfaces as usize) {
                        b.extend_from_slice(&[i.classe, i.subclasse, i.protocolo, 0]);
                    }
                }
                c.write_all(&b).map_err(|e| e.to_string())?;
                Ok(format!("{de} listou {} dispositivo(s)", lista.len()))
            }
            OP_REQ_IMPORT => {
                let mut bid = [0u8; 32];
                c.read_exact(&mut bid).map_err(|e| e.to_string())?;
                let busid = de_texto_fixo(&bid);
                let livre = validar_busid(&busid).is_ok()
                    && self.oferece(&busid)
                    && self.sysfs.estado_host(&busid) == Some(SDEV_ST_AVAILABLE);
                let d = match livre.then(|| self.sysfs.ler(&busid)) {
                    Some(Ok(d)) => d,
                    _ => {
                        let _ = c.write_all(&cabecalho(OP_REP_IMPORT, ST_NA));
                        return Err(format!("{de} pediu {busid:?}: indisponivel"));
                    }
                };
                let mut b = cabecalho(OP_REP_IMPORT, ST_OK).to_vec();
                b.extend_from_slice(&d.para_bytes());
                c.write_all(&b).map_err(|e| e.to_string())?;
                preparar_para_o_kernel(&c)?;
                self.sysfs.entregar(&busid, descritor(&c))?;
                Ok(format!("{de} esta usando {busid}"))
            }
            outro => Err(format!("{de}: pedido USB/IP {outro:#06x} desconhecido")),
        }
    }

    /// Laco de aceitar ate `parar`. Uma conexao por vez basta: o combinado
    /// dura milissegundos e o trafego depois e do kernel.
    pub fn servir(&self, ouvinte: TcpListener, parar: impl Fn() -> bool, log: impl Fn(&str)) {
        let _ = ouvinte.set_nonblocking(true);
        while !parar() {
            match ouvinte.accept() {
                Ok((c, _)) => {
                    let _ = c.set_nonblocking(false);
                    match self.atender(c) {
                        Ok(m) => log(&m),
                        Err(e) => log(&format!("recusado: {e}")),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                Err(e) => {
                    log(&format!("aceitar: {e}"));
                    std::thread::sleep(Duration::from_millis(500))
                }
            }
        }
    }
}

/// Abre a porta 3240 no IP virtual e, no Linux, prende o soquete a placa da
/// rede. Sem conseguir prender, recusa: escutar no IP virtual sem a trava da
/// placa deixaria a LAN alcancar o USB (modelo de host fraco).
pub fn escutar(ip: Ipv4Addr, interface: Option<&str>) -> R<TcpListener> {
    let o = TcpListener::bind(SocketAddrV4::new(ip, PORTA))
        .map_err(|e| format!("escutar {ip}:{PORTA}: {e}"))?;
    #[cfg(target_os = "linux")]
    if let Some(i) = interface {
        prender_na_interface(descritor(&o) as i32, i)?;
    }
    #[cfg(not(target_os = "linux"))]
    let _ = interface;
    Ok(o)
}

// ------------------------------------------------------------ cliente ----

fn conectar(ip: Ipv4Addr) -> R<TcpStream> {
    let c = TcpStream::connect_timeout(&SocketAddr::V4(SocketAddrV4::new(ip, PORTA)), PRAZO)
        .map_err(|e| format!("{ip}:{PORTA}: {e}"))?;
    let _ = c.set_read_timeout(Some(PRAZO));
    let _ = c.set_write_timeout(Some(PRAZO));
    Ok(c)
}

/// O que um membro compartilha.
pub fn remotos(ip: Ipv4Addr) -> R<Vec<Dispositivo>> {
    let mut c = conectar(ip)?;
    c.write_all(&cabecalho(OP_REQ_DEVLIST, ST_OK))
        .map_err(|e| e.to_string())?;
    let (_, codigo, status) =
        ler_cabecalho(&mut c).map_err(|_| format!("{ip} recusou (não é membro desta rede?)"))?;
    if codigo != OP_REP_DEVLIST || status != ST_OK {
        return Err(format!("{ip}: resposta USB/IP inesperada"));
    }
    let mut n = [0u8; 4];
    c.read_exact(&mut n).map_err(|e| e.to_string())?;
    let n = u32::from_be_bytes(n);
    if n > TETO_DISPOSITIVOS {
        return Err(format!("{ip}: lista com {n} dispositivos, recusada"));
    }
    let mut v = Vec::new();
    for _ in 0..n {
        let mut b = [0u8; TAM_DISPOSITIVO];
        c.read_exact(&mut b).map_err(|e| e.to_string())?;
        let mut d = Dispositivo::de_bytes(&b)?;
        if d.n_interfaces > TETO_INTERFACES {
            return Err(format!(
                "{ip}: dispositivo com {} interfaces",
                d.n_interfaces
            ));
        }
        for _ in 0..d.n_interfaces {
            let mut i = [0u8; 4];
            c.read_exact(&mut i).map_err(|e| e.to_string())?;
            d.interfaces.push(Interface {
                classe: i[0],
                subclasse: i[1],
                protocolo: i[2],
            });
        }
        v.push(d);
    }
    Ok(v)
}

/// Faz o combinado do `IMPORT` e devolve o soquete pronto para o kernel.
pub fn importar(ip: Ipv4Addr, busid: &str) -> R<(TcpStream, Dispositivo)> {
    validar_busid(busid)?;
    let mut c = conectar(ip)?;
    let mut b = cabecalho(OP_REQ_IMPORT, ST_OK).to_vec();
    b.extend_from_slice(&texto_fixo(busid, 32));
    c.write_all(&b).map_err(|e| e.to_string())?;
    let (_, codigo, status) = ler_cabecalho(&mut c)?;
    if codigo != OP_REP_IMPORT {
        return Err(format!("{ip}: resposta USB/IP inesperada"));
    }
    if status != ST_OK {
        return Err(format!(
            "{ip} não entregou {busid}: não está compartilhado nesta rede, ou já está em uso"
        ));
    }
    let mut d = [0u8; TAM_DISPOSITIVO];
    c.read_exact(&mut d).map_err(|e| e.to_string())?;
    let d = Dispositivo::de_bytes(&d)?;
    if d.busid != busid {
        return Err(format!(
            "{ip} respondeu com outro dispositivo ({})",
            d.busid
        ));
    }
    preparar_para_o_kernel(&c)?;
    Ok((c, d))
}

/// Usa o dispositivo de um membro como se estivesse espetado aqui. Devolve
/// a mensagem para quem pediu.
#[cfg(unix)]
pub fn usar(sysfs: &Sysfs, ip: Ipv4Addr, busid: &str) -> R<String> {
    // Conferir o controlador ANTES do combinado: depois dele o outro lado ja
    // entregou o dispositivo ao kernel, e desistir aqui o deixaria preso.
    sysfs.portas()?;
    let (c, d) = importar(ip, busid)?;
    let porta = sysfs.anexar(descritor(&c), &d, &ip.to_string())?;
    // O kernel ficou com a sua referencia; o descritor daqui pode fechar.
    drop(c);
    Ok(format!("{busid} de {ip} anexado na porta {porta}"))
}

#[cfg(windows)]
pub fn usar(_sysfs: &Sysfs, ip: Ipv4Addr, busid: &str) -> R<String> {
    validar_busid(busid)?;
    usbip_win2(&["attach", "-r", &ip.to_string(), "-b", busid])
}

#[cfg(unix)]
pub fn soltar(sysfs: &Sysfs, porta: u32) -> R<String> {
    sysfs.soltar(porta)?;
    Ok(format!("porta {porta} solta"))
}

#[cfg(windows)]
pub fn soltar(_sysfs: &Sysfs, porta: u32) -> R<String> {
    usbip_win2(&["detach", "-p", &porta.to_string()])
}

/// Uma porta em uso aqui: o numero (para soltar) e a descricao.
#[derive(Clone, Debug, PartialEq)]
pub struct EmUso {
    pub porta: u32,
    pub texto: String,
}

/// O que esta anexado aqui.
#[cfg(unix)]
pub fn em_uso(sysfs: &Sysfs) -> R<Vec<EmUso>> {
    Ok(sysfs
        .portas()?
        .into_iter()
        .filter(Porta::em_uso)
        .map(|p| EmUso {
            porta: p.numero,
            texto: format!(
                "{}  (aqui: {})",
                sysfs.origem(p.numero).unwrap_or_else(|| "?".into()),
                p.busid_local
            ),
        })
        .collect())
}

#[cfg(windows)]
pub fn em_uso(_sysfs: &Sysfs) -> R<Vec<EmUso>> {
    Ok(portas_do_usbip(&usbip_win2(&["port"])?))
}

/// Le a saida do `usbip port` (a de referencia e a do usbip-win2 seguem o
/// mesmo molde): `Port 01: <Port in Use> at High Speed(480Mbps)` abre um
/// bloco, as linhas seguintes o descrevem.
pub fn portas_do_usbip(saida: &str) -> Vec<EmUso> {
    let mut v: Vec<EmUso> = Vec::new();
    for l in saida.lines() {
        let t = l.trim();
        if let Some(resto) = t.strip_prefix("Port ") {
            if let Some((n, desc)) = resto.split_once(':') {
                if let Ok(porta) = n.trim().parse() {
                    v.push(EmUso {
                        porta,
                        texto: desc.trim().to_string(),
                    });
                    continue;
                }
            }
        }
        if let (Some(u), false) = (v.last_mut(), t.is_empty()) {
            u.texto.push_str(" | ");
            u.texto.push_str(t);
        }
    }
    v
}

// ------------------------------------------------ operacoes por rede ----
//
// O MESMO motor para o `phxvpn usb`, o `USB` do console e os botoes da
// janela: quem formata e cada porta; a decisao (o que se oferece em qual
// rede) mora aqui.

/// Um dispositivo daqui, com o estado dele nesta rede.
pub struct Local {
    pub dispositivo: Dispositivo,
    /// Preso ao `usbip-host` (saiu deste computador).
    pub preso: bool,
    /// Na lista `usb` desta rede.
    pub nesta_rede: bool,
}

pub fn locais_da_rede(sysfs: &Sysfs, rede: Option<&crate::rede_p2p::Rede>) -> R<Vec<Local>> {
    Ok(sysfs
        .locais()?
        .into_iter()
        .map(|d| Local {
            preso: sysfs.driver(&d.busid).as_deref() == Some("usbip-host"),
            nesta_rede: rede.is_some_and(|r| r.usb.contains(&d.busid)),
            dispositivo: d,
        })
        .collect())
}

/// Prende ao `usbip-host` e poe na lista da rede (o servidor da rede ja
/// ligada rele a lista a cada conexao: vale na hora).
pub fn compartilhar_na_rede(sysfs: &Sysfs, caminho_rede: &str, busid: &str) -> R<String> {
    let mut rede = crate::rede_p2p::Rede::ler(caminho_rede)?;
    sysfs.compartilhar(busid)?;
    if !rede.usb.iter().any(|u| u == busid) {
        rede.usb.push(busid.to_string());
        rede.gravar(caminho_rede)?;
    }
    Ok(format!(
        "{busid} compartilhado na rede {} — este computador deixa de vê-lo até «parar»",
        rede.nome
    ))
}

/// Tira da lista da rede e devolve o dispositivo a este computador.
pub fn parar_na_rede(sysfs: &Sysfs, caminho_rede: &str, busid: &str) -> R<String> {
    validar_busid(busid)?;
    let mut rede = crate::rede_p2p::Rede::ler(caminho_rede)?;
    rede.usb.retain(|u| u != busid);
    rede.gravar(caminho_rede)?;
    sysfs.parar(busid)?;
    Ok(format!("{busid} voltou para este computador"))
}

/// O `usbip.exe` do usbip-win2: na pasta de instalacao dele ou no PATH.
#[cfg(windows)]
fn usbip_win2(args: &[&str]) -> R<String> {
    let instalado = std::env::var("ProgramFiles")
        .map(|p| PathBuf::from(p).join("USBip").join("usbip.exe"))
        .ok()
        .filter(|p| p.is_file());
    let exe = instalado.unwrap_or_else(|| PathBuf::from("usbip.exe"));
    let s = std::process::Command::new(&exe)
        .args(args)
        .output()
        .map_err(|e| {
            format!(
                "usbip-win2 nao encontrado ({e}). Instale de \
                 https://github.com/vadimgrn/usbip-win2/releases (como administrador)"
            )
        })?;
    let texto = format!(
        "{}{}",
        String::from_utf8_lossy(&s.stdout),
        String::from_utf8_lossy(&s.stderr)
    );
    if s.status.success() {
        Ok(texto.trim().to_string())
    } else {
        Err(format!("usbip-win2: {}", texto.trim()))
    }
}

// ------------------------------------------------------ soquete bruto ----

/// Tira os prazos e liga o que o `usbip` de referencia liga antes de
/// entregar o soquete ao kernel.
fn preparar_para_o_kernel(c: &TcpStream) -> R<()> {
    c.set_read_timeout(None).map_err(|e| e.to_string())?;
    c.set_write_timeout(None).map_err(|e| e.to_string())?;
    let _ = c.set_nodelay(true);
    #[cfg(target_os = "linux")]
    manter_viva(descritor(c) as i32);
    Ok(())
}

#[cfg(unix)]
fn descritor<T: std::os::fd::AsRawFd>(s: &T) -> i64 {
    s.as_raw_fd() as i64
}

#[cfg(windows)]
fn descritor<T: std::os::windows::io::AsRawSocket>(s: &T) -> i64 {
    s.as_raw_socket() as i64
}

#[cfg(target_os = "linux")]
extern "C" {
    fn setsockopt(fd: i32, nivel: i32, opcao: i32, valor: *const u8, tam: u32) -> i32;
}

#[cfg(target_os = "linux")]
const SOL_SOCKET: i32 = 1;

#[cfg(target_os = "linux")]
fn manter_viva(fd: i32) {
    const SO_KEEPALIVE: i32 = 9;
    let um: i32 = 1;
    // SAFETY: fd vivo (emprestado de um TcpStream), valor de 4 bytes.
    unsafe {
        setsockopt(
            fd,
            SOL_SOCKET,
            SO_KEEPALIVE,
            &um as *const i32 as *const u8,
            4,
        )
    };
}

#[cfg(target_os = "linux")]
fn prender_na_interface(fd: i32, nome: &str) -> R<()> {
    const SO_BINDTODEVICE: i32 = 25;
    if nome.is_empty() || nome.len() >= 16 || nome.contains('\0') {
        return Err(format!("interface invalida: {nome:?}"));
    }
    let mut b = nome.as_bytes().to_vec();
    b.push(0);
    // SAFETY: fd vivo, `b` terminado em zero com o tamanho informado.
    let r = unsafe { setsockopt(fd, SOL_SOCKET, SO_BINDTODEVICE, b.as_ptr(), b.len() as u32) };
    if r != 0 {
        return Err(format!(
            "prender a porta USB na placa {nome}: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

// --------------------------------------------------------- rede P2P ----

/// Sobe o servidor USB de uma rede P2P numa thread: escuta no IP virtual,
/// presa a placa, e confere membros e dispositivos RELENDO o arquivo da
/// rede a cada conexao.
pub fn servir_rede(
    caminho_rede: String,
    interface: Option<String>,
    parar: impl Fn() -> bool + Send + 'static,
    log: impl Fn(&str) + Send + 'static,
) -> R<std::thread::JoinHandle<()>> {
    use crate::rede_p2p::Rede;
    let rede = Rede::ler(&caminho_rede)?;
    let ouvinte = escutar(rede.ip, interface.as_deref())?;
    let c1 = caminho_rede.clone();
    let proprio = rede.ip;
    let s = Servidor {
        sysfs: Sysfs::do_ambiente(),
        permitido: Arc::new(move |ip| {
            ip != proprio
                && Rede::ler(&c1)
                    .map(|r| r.pares.iter().any(|p| p.ip == ip))
                    .unwrap_or(false)
        }),
        compartilhado: Arc::new(move |b| {
            Rede::ler(&caminho_rede)
                .map(|r| r.usb.iter().any(|u| u == b))
                .unwrap_or(false)
        }),
    };
    Ok(std::thread::spawn(move || s.servir(ouvinte, parar, log)))
}

#[cfg(all(test, unix))]
mod testes {
    use super::*;

    /// Um sysfs de mentira: um pendrive `1-1` (livre, preso ao usbip-host),
    /// um teclado `1-2` preso ao usbhid, um hub `1-3`, e o vhci com 2
    /// portas hs e 2 ss.
    fn falso(nome: &str) -> (PathBuf, Sysfs) {
        let r = std::env::temp_dir().join(format!("phxvpn-usb-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        let dev = r.join("bus/usb/devices");
        let drv = r.join("bus/usb/drivers");
        for d in ["usbip-host", "usb-storage", "usbhid", "hub"] {
            std::fs::create_dir_all(drv.join(d)).unwrap();
        }
        let disp = |busid: &str, driver: &str, attrs: &[(&str, &str)], ifs: &[&str]| {
            let d = dev.join(busid);
            std::fs::create_dir_all(&d).unwrap();
            for (k, v) in attrs {
                std::fs::write(d.join(k), format!("{v}\n")).unwrap();
            }
            for (n, cls) in ifs.iter().enumerate() {
                let i = d.join(format!("{busid}:1.{n}"));
                std::fs::create_dir_all(&i).unwrap();
                std::fs::write(i.join("bInterfaceClass"), cls).unwrap();
                std::fs::write(i.join("bInterfaceSubClass"), "06").unwrap();
                std::fs::write(i.join("bInterfaceProtocol"), "50").unwrap();
            }
            std::os::unix::fs::symlink(drv.join(driver), d.join("driver")).unwrap();
        };
        let base = [
            ("busnum", "1"),
            ("bDeviceSubClass", "00"),
            ("bDeviceProtocol", "00"),
            ("bConfigurationValue", "1"),
            ("bNumConfigurations", "1"),
            ("bNumInterfaces", " 1"),
            ("bcdDevice", "0100"),
        ];
        let mut a = base.to_vec();
        a.extend([
            ("devnum", "5"),
            ("speed", "480"),
            ("idVendor", "0781"),
            ("idProduct", "5583"),
            ("bDeviceClass", "00"),
            ("product", "Ultra Fit"),
            ("manufacturer", "SanDisk"),
            ("usbip_status", "1"),
        ]);
        disp("1-1", "usbip-host", &a, &["08"]);
        let mut a = base.to_vec();
        a.extend([
            ("devnum", "6"),
            ("speed", "12"),
            ("idVendor", "046d"),
            ("idProduct", "c31c"),
            ("bDeviceClass", "00"),
        ]);
        disp("1-2", "usbhid", &a, &["03"]);
        let mut a = base.to_vec();
        a.extend([("devnum", "2"), ("speed", "480"), ("bDeviceClass", "09")]);
        disp("1-3", "hub", &a, &[]);
        let v = r.join("devices/platform/vhci_hcd.0");
        std::fs::create_dir_all(&v).unwrap();
        std::fs::write(
            v.join("status"),
            "hub port sta spd dev      sockfd local_busid\n\
             hs  0000 006 003 00010003 000011 3-1\n\
             hs  0001 004 000 00000000 000000 0-0\n\
             ss  0002 004 000 00000000 000000 0-0\n\
             ss  0003 004 000 00000000 000000 0-0\n",
        )
        .unwrap();
        (r.clone(), Sysfs::em(&r))
    }

    fn servidor(
        s: &Sysfs,
        membros: Vec<Ipv4Addr>,
        compartilhados: Vec<&'static str>,
    ) -> SocketAddr {
        let o = TcpListener::bind("127.0.0.1:0").unwrap();
        let end = o.local_addr().unwrap();
        let srv = Servidor {
            sysfs: s.clone(),
            permitido: Arc::new(move |ip| membros.contains(&ip)),
            compartilhado: Arc::new(move |b| compartilhados.contains(&b)),
        };
        std::thread::spawn(move || {
            for c in o.incoming().take(4).flatten() {
                let _ = srv.atender(c);
            }
        });
        end
    }

    /// O cliente fala com a porta 3240; nos testes a porta e sorteada.
    fn remotos_em(end: SocketAddr) -> R<Vec<Dispositivo>> {
        let mut c = TcpStream::connect(end).unwrap();
        c.set_read_timeout(Some(PRAZO)).unwrap();
        c.write_all(&cabecalho(OP_REQ_DEVLIST, 0)).unwrap();
        let (_, cod, st) = ler_cabecalho(&mut c)?;
        assert_eq!((cod, st), (OP_REP_DEVLIST, ST_OK));
        let mut n = [0u8; 4];
        c.read_exact(&mut n).unwrap();
        let mut v = Vec::new();
        for _ in 0..u32::from_be_bytes(n) {
            let mut b = [0u8; TAM_DISPOSITIVO];
            c.read_exact(&mut b).unwrap();
            let d = Dispositivo::de_bytes(&b)?;
            let mut i = vec![0u8; 4 * d.n_interfaces as usize];
            c.read_exact(&mut i).unwrap();
            v.push(d);
        }
        Ok(v)
    }

    fn importar_em(end: SocketAddr, busid: &str) -> (u32, Option<Dispositivo>) {
        let mut c = TcpStream::connect(end).unwrap();
        c.set_read_timeout(Some(PRAZO)).unwrap();
        let mut b = cabecalho(OP_REQ_IMPORT, 0).to_vec();
        b.extend_from_slice(&texto_fixo(busid, 32));
        c.write_all(&b).unwrap();
        let (_, cod, st) = ler_cabecalho(&mut c).unwrap();
        assert_eq!(cod, OP_REP_IMPORT);
        if st != ST_OK {
            return (st, None);
        }
        let mut d = [0u8; TAM_DISPOSITIVO];
        c.read_exact(&mut d).unwrap();
        (st, Some(Dispositivo::de_bytes(&d).unwrap()))
    }

    /// Interoperacao com o `usbip` de referencia (usbip-utils 2.0): ele
    /// lista o nosso servidor. So roda com `PHXVPN_USBIP=/caminho/usbip` e a
    /// porta 3240 livre -- o cliente de referencia nao deixa escolher porta.
    #[test]
    fn o_usbip_de_referencia_le_o_nosso_servidor() {
        let Ok(usbip) = std::env::var("PHXVPN_USBIP") else {
            eprintln!("PHXVPN_USBIP ausente: prova de interoperacao nao rodou");
            return;
        };
        let (r, s) = falso("interop");
        let o = TcpListener::bind(("127.0.0.1", PORTA)).expect("porta 3240 ocupada");
        let srv = Servidor {
            sysfs: s,
            permitido: Arc::new(|ip| ip == Ipv4Addr::LOCALHOST),
            compartilhado: Arc::new(|b| b == "1-1"),
        };
        std::thread::spawn(move || {
            for c in o.incoming().take(1).flatten() {
                srv.atender(c).unwrap();
            }
        });
        let saida = std::process::Command::new(usbip)
            .args(["list", "-r", "127.0.0.1"])
            .output()
            .unwrap();
        let t = String::from_utf8_lossy(&saida.stdout).to_string()
            + &String::from_utf8_lossy(&saida.stderr);
        eprintln!("{t}");
        assert!(saida.status.success(), "{t}");
        assert!(t.contains("1-1:"), "{t}");
        assert!(t.contains("0781:5583"), "{t}");
        assert!(
            t.contains("1-1:1.0 -> ") || t.contains("Mass Storage"),
            "{t}"
        );
        let _ = std::fs::remove_dir_all(r);
    }

    #[test]
    fn le_a_saida_do_usbip_port() {
        let saida = "Imported USB devices\n====================\n\
Port 00: <Port in Use> at High Speed(480Mbps)\n\
       SanDisk Corp. : Ultra Fit (0781:5583)\n\
       1-1 -> usbip://10.78.0.1:3240/1-1\n\
Port 08: <Port in Use> at Super Speed(5000Mbps)\n";
        let v = portas_do_usbip(saida);
        assert_eq!(v.iter().map(|u| u.porta).collect::<Vec<_>>(), [0, 8]);
        assert!(
            v[0].texto.contains("usbip://10.78.0.1:3240/1-1"),
            "{}",
            v[0].texto
        );
    }

    #[test]
    fn dispositivo_no_fio_tem_312_bytes_nos_deslocamentos_da_norma() {
        let (r, s) = falso("fio");
        let d = s.ler("1-1").unwrap();
        let b = d.para_bytes();
        assert_eq!(b.len(), TAM_DISPOSITIVO);
        assert_eq!(&b[256..259], b"1-1\0"[..3].as_ref());
        assert_eq!(&b[288..292], &1u32.to_be_bytes()); // busnum
        assert_eq!(&b[292..296], &5u32.to_be_bytes()); // devnum
        assert_eq!(&b[296..300], &3u32.to_be_bytes()); // alta velocidade
        assert_eq!(&b[300..304], &[0x07, 0x81, 0x55, 0x83]);
        assert_eq!(b[311], 1); // bNumInterfaces
        assert_eq!(Dispositivo::de_bytes(&b).unwrap().busid, "1-1");
        assert_eq!(d.devid(), (1 << 16) | 5);
        assert_eq!(d.interfaces[0].classe, 0x08);
        assert!(d.resumo().contains("SanDisk Ultra Fit"), "{}", d.resumo());
        let _ = std::fs::remove_dir_all(r);
    }

    #[test]
    fn busid_de_fora_nao_vira_caminho() {
        for ok in ["1-1", "3-2.4.1", "10-12"] {
            validar_busid(ok).unwrap();
        }
        for torto in [
            "",
            "1",
            "1-",
            "-1",
            "1-1/../..",
            "../1-1",
            "1-1.",
            "1-a",
            "1-1 2",
        ] {
            assert!(validar_busid(torto).is_err(), "{torto:?} passou");
        }
    }

    #[test]
    fn lista_so_o_compartilhado_nesta_rede_e_sem_hub() {
        let (r, s) = falso("lista");
        let locais: Vec<String> = s.locais().unwrap().into_iter().map(|d| d.busid).collect();
        assert_eq!(locais, ["1-1", "1-2"], "o hub 1-3 nao aparece");
        let membro = Ipv4Addr::LOCALHOST;
        // 1-2 esta "compartilhado" na rede mas preso ao usbhid: nao sai.
        let end = servidor(&s, vec![membro], vec!["1-1", "1-2"]);
        let v = remotos_em(end).unwrap();
        assert_eq!(
            v.iter().map(|d| d.busid.as_str()).collect::<Vec<_>>(),
            ["1-1"]
        );
        // Preso ao usbip-host mas nao compartilhado NESTA rede: nao sai.
        let end = servidor(&s, vec![membro], vec![]);
        assert!(remotos_em(end).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(r);
    }

    #[test]
    fn quem_nao_e_membro_nao_recebe_nem_um_byte() {
        let (r, s) = falso("membro");
        let end = servidor(&s, vec![Ipv4Addr::new(10, 78, 0, 2)], vec!["1-1"]);
        assert!(remotos_em(end).is_err(), "estranho leu a lista");
        let _ = std::fs::remove_dir_all(r);
    }

    #[test]
    fn importar_entrega_o_soquete_ao_usbip_host() {
        let (r, s) = falso("importar");
        let end = servidor(&s, vec![Ipv4Addr::LOCALHOST], vec!["1-1", "1-2"]);
        let sockfd = r.join("bus/usb/devices/1-1/usbip_sockfd");
        let (st, d) = importar_em(end, "1-1");
        assert_eq!(st, ST_OK);
        assert_eq!(d.unwrap().produto, 0x5583);
        // Espera a thread do servidor escrever -- o CONTEUDO, nao o arquivo:
        // `fs::write` cria e so depois escreve, e sob carga a leitura caia
        // no meio (falhou uma vez na suite inteira, 24/09).
        let mut fd: i64 = -1;
        for _ in 0..100 {
            if let Some(n) = std::fs::read_to_string(&sockfd)
                .ok()
                .and_then(|t| t.trim().parse().ok())
            {
                fd = n;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(fd > 2, "descritor {fd}");
        // Nao compartilhado de verdade (usbhid) e busid torto: recusa igual.
        assert_eq!(importar_em(end, "1-2").0, ST_NA);
        assert_eq!(importar_em(end, "../1-1").0, ST_NA);
        // Em uso (usbip_status 2): recusa.
        std::fs::write(r.join("bus/usb/devices/1-1/usbip_status"), "2\n").unwrap();
        assert_eq!(importar_em(end, "1-1").0, ST_NA);
        let _ = std::fs::remove_dir_all(r);
    }

    #[test]
    fn compartilhar_e_parar_escrevem_o_que_o_usbip_escreve() {
        let (r, s) = falso("bind");
        s.compartilhar("1-2").unwrap();
        let drv = r.join("bus/usb/drivers");
        let ler = |p: &str| std::fs::read_to_string(drv.join(p)).unwrap();
        assert_eq!(ler("usbhid/unbind"), "1-2");
        assert_eq!(ler("usbip-host/match_busid"), "add 1-2");
        assert_eq!(ler("usbip-host/bind"), "1-2");
        // Ja preso ao usbip-host: nao mexe em nada.
        std::fs::remove_file(drv.join("usbip-host/bind")).unwrap();
        s.compartilhar("1-1").unwrap();
        assert!(!drv.join("usbip-host/bind").exists());
        s.parar("1-1").unwrap();
        assert_eq!(ler("usbip-host/unbind"), "1-1");
        assert_eq!(ler("usbip-host/match_busid"), "del 1-1");
        assert_eq!(ler("usbip-host/rebind"), "1-1");
        assert!(s.compartilhar("9-9").is_err(), "dispositivo que nao existe");
        let _ = std::fs::remove_dir_all(r);
    }

    #[test]
    fn anexar_escolhe_porta_livre_do_hub_certo() {
        let (r, s) = falso("vhci");
        let mut d = s.ler("1-1").unwrap();
        // USB 2: a porta 0 (hs) esta usada, vai na 1.
        assert_eq!(s.anexar(7, &d, "10.78.0.1").unwrap(), 1);
        let attach = r.join("devices/platform/vhci_hcd.0/attach");
        assert_eq!(
            std::fs::read_to_string(&attach).unwrap(),
            format!("1 7 {} 3", (1 << 16) | 5)
        );
        assert_eq!(s.origem(1).as_deref(), Some("10.78.0.1 1-1"));
        // USB 3 vai no hub ss.
        d.velocidade = 5;
        assert_eq!(s.anexar(8, &d, "10.78.0.1").unwrap(), 2);
        let usadas: Vec<u32> = s
            .portas()
            .unwrap()
            .into_iter()
            .filter(Porta::em_uso)
            .map(|p| p.numero)
            .collect();
        assert_eq!(usadas, [0]);
        s.soltar(1).unwrap();
        assert_eq!(s.origem(1), None);
        let _ = std::fs::remove_dir_all(r);
    }
}
