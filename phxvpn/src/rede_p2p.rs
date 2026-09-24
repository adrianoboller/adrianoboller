//! A rede P2P em disco (`<rede>.p2p`) e o convite.
//!
//! # O arquivo da rede
//!
//! JSON, 0600, um por rede neste computador: o proprio IP, a porta, o modo,
//! o repasse, os pares conhecidos e os convites em aberto. A SENHA NAO fica
//! aqui -- e pedida ao ligar e vira a PSK so em memoria. A chave privada fica
//! no arquivo de identidade (`p2p.chave`), separado, tambem 0600.
//!
//! # O convite
//!
//! ```text
//! phxvpn1.<nome da rede em base64url>.<selo em base64url>
//! ```
//!
//! O selo e XChaCha20-Poly1305 com a chave `HKDF(PSK da rede, "convite")`
//! e o nome da rede como dado associado. Quem nao sabe a senha nao le nem
//! forja; mexer num byte do codigo derruba a etiqueta. Dentro: quem convida
//! (chave, IP virtual, endereco), o repasse, o IP reservado ao convidado, a
//! validade e uma FICHA de 16 bytes de uso unico.
//!
//! # A ficha e a admissao
//!
//! O no de quem convidou admite uma chave DESCONHECIDA so se o INICIO dela
//! trouxer uma ficha em aberto e dentro do prazo -- e a ficha so sai de
//! dentro de um convite que so abre com a senha. Admitida, a ficha morre.
//! Assim «entrar na rede» pede o que o Radmin pede: o convite (o nome) e a
//! senha -- sem servidor para guardar a lista.

use crate::comandos::gravar_secreto;
use crate::rol::Rol;
use phxsql_core::base64;
use phxsql_core::cifra::{xabrir, xselar};
use phxsql_core::hash::{de_hex, para_hex};
use phxsql_core::json::Json;
use phxsql_core::senha::bytes_aleatorios;
use std::net::Ipv4Addr;
use std::time::{SystemTime, UNIX_EPOCH};

pub type R<T> = Result<T, String>;

pub const PREFIXO_CONVITE: &str = "phxvpn1";

#[derive(Clone, Debug, PartialEq)]
pub struct Par {
    pub chave: [u8; 32],
    pub ip: Ipv4Addr,
    pub endereco: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConviteAberto {
    pub ficha: [u8; 16],
    pub ip: Ipv4Addr,
    pub expira: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rede {
    pub nome: String,
    pub ip: Ipv4Addr,
    pub prefixo: u8,
    pub porta: u16,
    pub modo: String,
    /// `CHAVE@HOST:PORTA` do servidor intermediario, se houver.
    pub repasse: Option<String>,
    /// Usuario deste computador no servidor intermediario (a senha nunca
    /// fica aqui: e pedida ao ligar).
    pub repasse_usuario: Option<String>,
    pub pares: Vec<Par>,
    pub convites: Vec<ConviteAberto>,
    /// Ficha que ESTE no apresenta ao entrar (veio do convite); some quando
    /// o primeiro aperto fecha.
    pub ficha_de_entrada: Option<[u8; 16]>,
    /// Dispositivos USB (busid) que ESTE computador oferece nesta rede.
    pub usb: Vec<String>,
    /// Publica Ed25519 de quem criou a rede (ver `rol.rs`). `None` = rede de
    /// antes do rol assinado: a malha continua com confianca transitiva.
    pub dono: Option<[u8; 32]>,
    /// O rol mais novo que este no aceitou (ou, no dono, assinou).
    pub rol: Option<Rol>,
    /// Anuncio na LAN (ver `descoberta.rs`). Arquivo sem o campo: ligada.
    pub descoberta: bool,
    /// Nome deste computador no rol (opcional).
    pub apelido: Option<String>,
}

pub fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn b64url(b: &[u8]) -> String {
    base64::codificar(b)
        .trim_end_matches('=')
        .replace('+', "-")
        .replace('/', "_")
}

fn de_b64url(t: &str) -> R<Vec<u8>> {
    let mut s = t.replace('-', "+").replace('_', "/");
    while s.len() % 4 != 0 {
        s.push('=');
    }
    base64::decodificar(&s).map_err(|_| "convite torto (base64)".to_string())
}

fn chave32(t: &str) -> R<[u8; 32]> {
    de_hex(t)
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| format!("chave invalida: {t}"))
}

fn ficha16(t: &str) -> R<[u8; 16]> {
    de_hex(t)
        .and_then(|b| <[u8; 16]>::try_from(b).ok())
        .ok_or_else(|| "ficha invalida".to_string())
}

fn ip(t: &str) -> R<Ipv4Addr> {
    t.parse().map_err(|_| format!("IP invalido: {t}"))
}

fn par_json(p: &Par) -> Json {
    Json::objeto(vec![
        ("chave", Json::texto_de(para_hex(&p.chave))),
        ("ip", Json::texto_de(p.ip.to_string())),
        (
            "endereco",
            p.endereco.clone().map(Json::texto_de).unwrap_or(Json::Nulo),
        ),
    ])
}

pub fn par_de_json(j: &Json) -> R<Par> {
    Ok(Par {
        chave: chave32(j.texto_ou("chave", ""))?,
        ip: ip(j.texto_ou("ip", ""))?,
        endereco: j
            .campo("endereco")
            .and_then(Json::texto)
            .map(str::to_string),
    })
}

pub fn pares_json(pares: &[Par]) -> Json {
    Json::Lista(pares.iter().map(par_json).collect())
}

impl Rede {
    pub fn nova(nome: &str, ip: Ipv4Addr, prefixo: u8, porta: u16) -> Rede {
        Rede {
            nome: nome.into(),
            ip,
            prefixo,
            porta,
            modo: "direto".into(),
            repasse: None,
            repasse_usuario: None,
            pares: Vec::new(),
            convites: Vec::new(),
            ficha_de_entrada: None,
            usb: Vec::new(),
            dono: None,
            rol: None,
            descoberta: true,
            apelido: None,
        }
    }

    pub fn caminho(nome: &str) -> String {
        let limpo: String = nome
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{limpo}.p2p")
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("rede", Json::texto_de(&self.nome)),
            (
                "ip",
                Json::texto_de(format!("{}/{}", self.ip, self.prefixo)),
            ),
            ("porta", Json::de_i64(self.porta as i64)),
            ("modo", Json::texto_de(&self.modo)),
            (
                "repasse",
                self.repasse
                    .clone()
                    .map(Json::texto_de)
                    .unwrap_or(Json::Nulo),
            ),
            (
                "repasse_usuario",
                self.repasse_usuario
                    .clone()
                    .map(Json::texto_de)
                    .unwrap_or(Json::Nulo),
            ),
            ("pares", pares_json(&self.pares)),
            (
                "convites",
                Json::Lista(
                    self.convites
                        .iter()
                        .map(|c| {
                            Json::objeto(vec![
                                ("ficha", Json::texto_de(para_hex(&c.ficha))),
                                ("ip", Json::texto_de(c.ip.to_string())),
                                ("expira", Json::de_i64(c.expira as i64)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "ficha_de_entrada",
                self.ficha_de_entrada
                    .map(|f| Json::texto_de(para_hex(&f)))
                    .unwrap_or(Json::Nulo),
            ),
            (
                "usb",
                Json::Lista(self.usb.iter().map(Json::texto_de).collect()),
            ),
            (
                "dono",
                self.dono
                    .map(|d| Json::texto_de(para_hex(&d)))
                    .unwrap_or(Json::Nulo),
            ),
            (
                "rol",
                self.rol
                    .as_ref()
                    .map(|r| Json::texto_de(para_hex(&r.para_bytes())))
                    .unwrap_or(Json::Nulo),
            ),
            ("descoberta", Json::de_bool(self.descoberta)),
            (
                "apelido",
                self.apelido
                    .clone()
                    .map(Json::texto_de)
                    .unwrap_or(Json::Nulo),
            ),
        ])
    }

    pub fn de_json(j: &Json) -> R<Rede> {
        let (ipt, pre) = j
            .texto_ou("ip", "")
            .split_once('/')
            .ok_or("arquivo da rede sem ip/prefixo")?;
        let convites = j
            .campo("convites")
            .and_then(Json::lista)
            .unwrap_or_default()
            .iter()
            .map(|c| {
                Ok(ConviteAberto {
                    ficha: ficha16(c.texto_ou("ficha", ""))?,
                    ip: ip(c.texto_ou("ip", ""))?,
                    expira: c.inteiro_ou("expira", 0) as u64,
                })
            })
            .collect::<R<Vec<_>>>()?;
        Ok(Rede {
            nome: j.texto_ou("rede", "").to_string(),
            ip: ip(ipt)?,
            prefixo: pre.parse().map_err(|_| "prefixo invalido")?,
            porta: j.inteiro_ou("porta", 51820) as u16,
            modo: j.texto_ou("modo", "direto").to_string(),
            repasse: j.campo("repasse").and_then(Json::texto).map(str::to_string),
            repasse_usuario: j
                .campo("repasse_usuario")
                .and_then(Json::texto)
                .map(str::to_string),
            pares: j
                .campo("pares")
                .and_then(Json::lista)
                .unwrap_or_default()
                .iter()
                .map(par_de_json)
                .collect::<R<Vec<_>>>()?,
            convites,
            ficha_de_entrada: match j.campo("ficha_de_entrada").and_then(Json::texto) {
                Some(f) => Some(ficha16(f)?),
                None => None,
            },
            usb: j
                .campo("usb")
                .and_then(Json::lista)
                .unwrap_or_default()
                .iter()
                .filter_map(|u| u.texto().map(str::to_string))
                .filter(|u| crate::usb::validar_busid(u).is_ok())
                .collect(),
            dono: match j.campo("dono").and_then(Json::texto) {
                Some(d) => Some(chave32(d)?),
                None => None,
            },
            // O rol do disco se relê sem conferir a assinatura aqui: quem o
            // usa (`p2p.rs`, `sincronizar_rol`) confere antes de adotar.
            rol: match j.campo("rol").and_then(Json::texto) {
                Some(h) => Some(
                    de_hex(h)
                        .and_then(|b| Rol::de_bytes(&b))
                        .ok_or("rol torto no arquivo da rede")?,
                ),
                None => None,
            },
            descoberta: j.booleano_ou("descoberta", true),
            apelido: j.campo("apelido").and_then(Json::texto).map(str::to_string),
        })
    }

    pub fn ler(caminho: &str) -> R<Rede> {
        let t = std::fs::read_to_string(caminho).map_err(|e| format!("{caminho}: {e}"))?;
        Rede::de_json(&Json::analisar(&t).map_err(|e| format!("{caminho}: {e}"))?)
    }

    /// Grava por arquivo temporario + renomear: queda no meio nao deixa o
    /// arquivo da rede pela metade.
    pub fn gravar(&self, caminho: &str) -> R<()> {
        let tmp = format!("{caminho}.tmp");
        let _ = std::fs::remove_file(&tmp);
        gravar_secreto(&tmp, self.para_json().escrever_identado().as_bytes(), true)?;
        std::fs::rename(&tmp, caminho).map_err(|e| format!("gravar {caminho}: {e}"))
    }

    /// O menor IP da sub-rede que ninguem usa (nem par, nem convite aberto).
    pub fn proximo_ip(&self) -> R<Ipv4Addr> {
        let mascara = if self.prefixo == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefixo as u32)
        };
        let base = u32::from(self.ip) & mascara;
        let tamanho = !mascara;
        let usado = |c: Ipv4Addr| {
            c == self.ip
                || self.pares.iter().any(|p| p.ip == c)
                || self
                    .convites
                    .iter()
                    .any(|v| v.ip == c && v.expira > agora())
        };
        (1..tamanho)
            .map(|n| Ipv4Addr::from(base | n))
            .find(|c| !usado(*c))
            .ok_or_else(|| "a sub-rede da rede esta cheia".into())
    }

    /// Consome a ficha, se estiver aberta e no prazo; devolve o IP reservado.
    pub fn usar_ficha(&mut self, ficha: &[u8; 16]) -> Option<Ipv4Addr> {
        let agora = agora();
        let i = self.convites.iter().position(|c| {
            c.expira > agora && phxsql_core::hash::iguais_em_tempo_constante(&c.ficha, ficha)
        })?;
        Some(self.convites.remove(i).ip)
    }
}

fn chave_do_convite(psk: &[u8; 32]) -> [u8; 32] {
    let mut k = [0u8; 32];
    phxsql_core::hkdf::derivar(b"phxvpn-convite-v1", psk, b"convite", &mut k).expect("32 bytes");
    k
}

/// O que vai dentro do convite.
#[derive(Debug, PartialEq)]
pub struct Convite {
    pub rede: String,
    pub prefixo: u8,
    pub anfitriao: Par,
    pub repasse: Option<String>,
    pub modo: String,
    pub ip_convidado: Ipv4Addr,
    pub expira: u64,
    pub ficha: [u8; 16],
    /// Publica Ed25519 do dono; `None` em rede sem rol assinado.
    pub dono: Option<[u8; 32]>,
}

/// Monta o codigo do convite e ja registra a ficha como aberta na rede.
pub fn convidar(
    rede: &mut Rede,
    psk: &[u8; 32],
    minha_chave: [u8; 32],
    meu_endereco: Option<String>,
    validade_s: u64,
) -> R<String> {
    let ip_convidado = rede.proximo_ip()?;
    let ficha: [u8; 16] = bytes_aleatorios(16).try_into().expect("16");
    let expira = agora() + validade_s;
    rede.convites.retain(|c| c.expira > agora());
    rede.convites.push(ConviteAberto {
        ficha,
        ip: ip_convidado,
        expira,
    });
    let dentro = Json::objeto(vec![
        ("prefixo", Json::de_i64(rede.prefixo as i64)),
        (
            "anfitriao",
            par_json(&Par {
                chave: minha_chave,
                ip: rede.ip,
                endereco: meu_endereco,
            }),
        ),
        (
            "repasse",
            rede.repasse
                .clone()
                .map(Json::texto_de)
                .unwrap_or(Json::Nulo),
        ),
        ("modo", Json::texto_de(&rede.modo)),
        ("ip", Json::texto_de(ip_convidado.to_string())),
        ("expira", Json::de_i64(expira as i64)),
        ("ficha", Json::texto_de(para_hex(&ficha))),
        (
            "dono",
            rede.dono
                .map(|d| Json::texto_de(para_hex(&d)))
                .unwrap_or(Json::Nulo),
        ),
    ])
    .escrever();
    let nonce: [u8; 24] = bytes_aleatorios(24).try_into().expect("24");
    let (mut c, tag) = xselar(
        &chave_do_convite(psk),
        &nonce,
        rede.nome.as_bytes(),
        dentro.as_bytes(),
    );
    c.extend_from_slice(&tag);
    let mut selo = nonce.to_vec();
    selo.extend_from_slice(&c);
    Ok(format!(
        "{PREFIXO_CONVITE}.{}.{}",
        b64url(rede.nome.as_bytes()),
        b64url(&selo)
    ))
}

/// So o nome da rede (para derivar a PSK antes de abrir o resto).
pub fn rede_do_convite(codigo: &str) -> R<String> {
    let partes: Vec<&str> = codigo.trim().split('.').collect();
    if partes.len() != 3 || partes[0] != PREFIXO_CONVITE {
        return Err("isto nao e um convite do phxvpn".into());
    }
    String::from_utf8(de_b64url(partes[1])?).map_err(|_| "nome de rede torto".into())
}

pub fn abrir_convite(codigo: &str, psk: &[u8; 32]) -> R<Convite> {
    let rede = rede_do_convite(codigo)?;
    let selo = de_b64url(codigo.trim().split('.').nth(2).unwrap_or_default())?;
    if selo.len() < 24 + 16 {
        return Err("convite curto demais".into());
    }
    let nonce: [u8; 24] = selo[..24].try_into().expect("24");
    let corte = selo.len() - 16;
    let tag: [u8; 16] = selo[corte..].try_into().expect("16");
    let claro = xabrir(
        &chave_do_convite(psk),
        &nonce,
        rede.as_bytes(),
        &selo[24..corte],
        &tag,
    )
    .map_err(|_| "senha da rede errada, ou convite adulterado".to_string())?;
    let j = Json::analisar(std::str::from_utf8(&claro).map_err(|_| "convite torto")?)
        .map_err(|e| e.to_string())?;
    let c = Convite {
        rede,
        prefixo: j.inteiro_ou("prefixo", 24) as u8,
        anfitriao: par_de_json(j.campo("anfitriao").ok_or("convite sem anfitriao")?)?,
        repasse: j.campo("repasse").and_then(Json::texto).map(str::to_string),
        modo: j.texto_ou("modo", "direto").to_string(),
        ip_convidado: ip(j.texto_ou("ip", ""))?,
        expira: j.inteiro_ou("expira", 0) as u64,
        ficha: ficha16(j.texto_ou("ficha", ""))?,
        dono: match j.campo("dono").and_then(Json::texto) {
            Some(d) => Some(chave32(d)?),
            None => None,
        },
    };
    if c.expira <= agora() {
        return Err("convite vencido: peca outro".into());
    }
    Ok(c)
}

/// A rede deste computador, montada a partir do convite aberto.
pub fn rede_do_convidado(c: &Convite, porta: u16) -> Rede {
    Rede {
        nome: c.rede.clone(),
        ip: c.ip_convidado,
        prefixo: c.prefixo,
        porta,
        modo: c.modo.clone(),
        repasse: c.repasse.clone(),
        repasse_usuario: None,
        pares: vec![c.anfitriao.clone()],
        convites: Vec::new(),
        ficha_de_entrada: Some(c.ficha),
        usb: Vec::new(),
        // O convidado passa a so aceitar rol assinado por esta chave; ate o
        // primeiro chegar, so conhece o anfitriao do convite.
        dono: c.dono,
        rol: None,
        descoberta: true,
        apelido: None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rede() -> Rede {
        let mut r = Rede::nova("Filial Sul", "10.78.0.1".parse().unwrap(), 24, 51820);
        r.pares.push(Par {
            chave: [3; 32],
            ip: "10.78.0.2".parse().unwrap(),
            endereco: None,
        });
        r
    }

    #[test]
    fn convite_abre_com_a_senha_e_nao_abre_sem() {
        let mut r = rede();
        let psk = crate::p2p::psk_da_rede("Filial Sul", "senha-1", 1_000);
        let codigo = convidar(
            &mut r,
            &psk,
            [9; 32],
            Some("198.51.100.7:51820".into()),
            3600,
        )
        .unwrap();
        assert_eq!(rede_do_convite(&codigo).unwrap(), "Filial Sul");
        let c = abrir_convite(&codigo, &psk).unwrap();
        assert_eq!(
            c.ip_convidado,
            "10.78.0.3".parse::<Ipv4Addr>().unwrap(),
            "pula o .1 e o .2 usados"
        );
        assert_eq!(c.anfitriao.chave, [9; 32]);
        let errada = crate::p2p::psk_da_rede("Filial Sul", "outra", 1_000);
        assert!(abrir_convite(&codigo, &errada).is_err());
        // Um byte trocado no fim do codigo derruba a etiqueta.
        let mut torto = codigo.clone();
        let ultimo = torto.pop().unwrap();
        torto.push(if ultimo == 'A' { 'B' } else { 'A' });
        assert!(abrir_convite(&torto, &psk).is_err());
    }

    #[test]
    fn ficha_vale_uma_vez() {
        let mut r = rede();
        let psk = [5; 32];
        let codigo = convidar(&mut r, &psk, [9; 32], None, 3600).unwrap();
        let c = abrir_convite(&codigo, &psk).unwrap();
        assert_eq!(r.usar_ficha(&c.ficha), Some(c.ip_convidado));
        assert_eq!(r.usar_ficha(&c.ficha), None);
        assert_eq!(r.usar_ficha(&[0; 16]), None);
    }

    #[test]
    fn arquivo_ida_e_volta() {
        let mut r = rede();
        r.repasse = Some("ab@1.2.3.4:51821".into());
        r.ficha_de_entrada = Some([7; 16]);
        convidar(&mut r, &[1; 32], [9; 32], None, 60).unwrap();
        let j = Json::analisar(&r.para_json().escrever()).unwrap();
        assert_eq!(Rede::de_json(&j).unwrap(), r);
    }
}
