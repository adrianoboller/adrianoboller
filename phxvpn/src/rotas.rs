//! Redes ALCANCAVEIS pela VPN do modo servidor: a LAN da empresa atras do
//! servidor (lacunas, item 4) e a LAN atras de um membro -- a filial com
//! roteador, o site-to-site (item 9).
//!
//! # O que cada uma escreve
//!
//! - **Atras do servidor**: `push "route <lan>"` no conf da rede, e o HOST do
//!   servidor passa a encaminhar (`ip_forward`) com as regras numa tabela
//!   PROPRIA (`table ip phxvpn` no nftables, ou as cadeias `PHXVPN-*` no
//!   iptables). A volta e NAT (padrao: a LAN ve o IP do servidor e nao precisa
//!   saber da VPN) ou «rota de volta» (sem NAT: o roteador da empresa leva
//!   `10.77.N.0/24` ao servidor, e a LAN ve o IP verdadeiro do membro).
//! - **Atras de um membro**: `iroute` no `ccd/` daquele membro (o OpenVPN so
//!   aceita dele pacote com origem na filial se a filial for dele),
//!   `route` no conf (o kernel do servidor manda a filial para a placa da
//!   rede) e `push "route"` para os outros -- com `push-remove` no `ccd/` do
//!   proprio, senao ele receberia rota para a propria LAN pelo tunel.
//!
//! # Por que a tabela propria tem uma cadeia de FILTRO, e nao so o NAT
//!
//! Ligar o `ip_forward` abre o host inteiro para encaminhar -- inclusive de
//! `tun0` (rede A) para `tun1` (rede B), que e exatamente o isolamento que o
//! `ovpn.rs` promete. A cadeia `encaminhar` aceita so o par (origem da rede N,
//! LAN da rede N) nos dois sentidos, e descarta qualquer outro pacote que
//! venha ou va para o espaco da VPN ou para uma filial. Nao mexe em regra
//! alheia: uma tabela so, apagada inteira quando a ultima rota sai.
//!
//! E a guarda fica SEMPRE que o painel sobe redes, com ou sem rota (decisao
//! do integrador, 24/09/2026): host que ja encaminha por outro motivo
//! (Docker, roteador) abria a rede A para a rede B sem nenhuma rota do
//! phxvpn. Sem rota, a tabela e so o `drop` entre os enderecos da VPN --
//! nada de NAT, e o `ip_forward` nao e tocado. Sem rede nenhuma, some.
//!
//! Limite, e ele e do nftables: um `drop` de OUTRA tabela no mesmo gancho
//! vence o nosso `accept` (Docker e firewalld poem `policy drop` no forward).
//! O painel nao desfaz regra alheia -- ele AVISA, na resposta, que achou uma.
//!
//! # Por que so faixas privadas, e nunca `0.0.0.0/0`
//!
//! Faixa publica pelo tunel faz do servidor a saida de internet daquela faixa
//! para todos os membros -- e tunel total em fatias, que e outro item
//! (`redirect-gateway`), e so entra com a protecao de vazamento de DNS/IPv6
//! junto. `10.77.0.0/16` e o espaco das proprias redes da VPN: rota ali
//! sequestraria o trafego entre membros.

use crate::painel::{Painel, Usuario};
use crate::pg::R;
use phxsql_core::json::Json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const ESQUEMA: &str = "
CREATE TABLE IF NOT EXISTS phx_rota (
    rede_id   int NOT NULL REFERENCES phx_rede ON DELETE RESTRICT,
    cidr      text NOT NULL,
    membro_id int,
    volta     text NOT NULL DEFAULT 'nat' CHECK (volta IN ('nat', 'rota')),
    criada_em timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (rede_id, cidr),
    FOREIGN KEY (rede_id, membro_id) REFERENCES phx_membro (rede_id, usuario_id) ON DELETE RESTRICT
);
";

/// Todo o espaco do modo servidor: `10.77.N.0/24`, N de 1 a 254.
pub const ESPACO_VPN: Cidr = Cidr {
    rede: 0x0a4d_0000,
    prefixo: 16,
};

/// As faixas privadas que podem ir pelo tunel (RFC 1918 e a compartilhada
/// da RFC 6598, que ha empresa usando por dentro).
const PRIVADAS: [Cidr; 4] = [
    Cidr {
        rede: 0x0a00_0000,
        prefixo: 8,
    },
    Cidr {
        rede: 0xac10_0000,
        prefixo: 12,
    },
    Cidr {
        rede: 0xc0a8_0000,
        prefixo: 16,
    },
    Cidr {
        rede: 0x6440_0000,
        prefixo: 10,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cidr {
    pub rede: u32,
    pub prefixo: u8,
}

impl Cidr {
    /// `a.b.c.d/p`, estrito: octeto sem zero a esquerda (`010` e octal em
    /// muito analisador, e o admin leria dez), e sem bit de host ligado --
    /// `192.168.10.5/24` quase sempre e o IP do servidor digitado no lugar da
    /// rede, e aceitar calado esconderia o engano.
    pub fn analisar(t: &str) -> Result<Cidr, String> {
        let t = t.trim();
        let (ip, p) = t
            .split_once('/')
            .ok_or_else(|| format!("«{t}»: escreva a rede com a mascara, ex. 192.168.10.0/24"))?;
        let numero = |s: &str, max: u32| -> Option<u32> {
            let ok = (1..=3).contains(&s.len())
                && s.bytes().all(|b| b.is_ascii_digit())
                && !(s.len() > 1 && s.starts_with('0'));
            ok.then(|| s.parse::<u32>().ok())
                .flatten()
                .filter(|v| *v <= max)
        };
        let octetos: Vec<&str> = ip.split('.').collect();
        if octetos.len() != 4 {
            return Err(format!("«{t}»: endereco invalido"));
        }
        let mut rede = 0u32;
        for o in &octetos {
            rede =
                (rede << 8) | numero(o, 255).ok_or_else(|| format!("«{t}»: endereco invalido"))?;
        }
        let prefixo = numero(p, 32).ok_or_else(|| format!("«{t}»: mascara de 0 a 32"))? as u8;
        let c = Cidr { rede, prefixo };
        if rede & !c.bits() != 0 {
            let certo = Cidr {
                rede: rede & c.bits(),
                prefixo,
            };
            return Err(format!(
                "«{t}» tem bits de host ligados: a rede e {certo} (o IP de uma maquina nao e a rede)"
            ));
        }
        Ok(c)
    }

    fn bits(&self) -> u32 {
        if self.prefixo == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefixo)
        }
    }

    fn quad(v: u32) -> String {
        let b = v.to_be_bytes();
        format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
    }

    pub fn endereco(&self) -> String {
        Cidr::quad(self.rede)
    }

    pub fn mascara(&self) -> String {
        Cidr::quad(self.bits())
    }

    pub fn sobrepoe(&self, o: &Cidr) -> bool {
        let m = if self.prefixo < o.prefixo {
            self.bits()
        } else {
            o.bits()
        };
        self.rede & m == o.rede & m
    }

    pub fn contem_ip(&self, ip: u32) -> bool {
        ip & self.bits() == self.rede
    }

    pub(crate) fn contem(&self, o: &Cidr) -> bool {
        self.prefixo <= o.prefixo && o.rede & self.bits() == self.rede
    }

    /// `rede mascara`, o formato das diretivas do OpenVPN.
    fn ovpn(&self) -> String {
        format!("{} {}", self.endereco(), self.mascara())
    }
}

impl std::fmt::Display for Cidr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}", self.endereco(), self.prefixo)
    }
}

/// Endereco numa faixa privada (a lista das rotas).
pub fn privada(ip: u32) -> bool {
    PRIVADAS.iter().any(|p| p.contem_ip(ip))
}

/// O que pode ir pelo tunel, conferido sozinho (sem olhar as outras rotas).
pub fn validar(c: &Cidr) -> Result<(), String> {
    if c.prefixo == 0 {
        return Err(
            "0.0.0.0/0 e tunel total (toda a internet pela VPN): e a chave «tunel total» da rede, que leva junto a protecao de vazamento de DNS e IPv6 -- aqui nao"
                .into(),
        );
    }
    if !PRIVADAS.iter().any(|p| p.contem(c)) {
        return Err(format!(
            "{c} nao e faixa privada (10/8, 172.16/12, 192.168/16, 100.64/10): faixa publica pelo tunel faria do servidor a saida de internet dela"
        ));
    }
    if c.sobrepoe(&ESPACO_VPN) {
        return Err(format!(
            "{c} sobrepoe {ESPACO_VPN}, o espaco das proprias redes da VPN"
        ));
    }
    Ok(())
}

/// Uma rota gravada, com o que o conf, o `ccd/` e o firewall precisam.
#[derive(Clone, Debug, PartialEq)]
pub struct Rota {
    pub rede_id: i64,
    pub octeto: u8,
    pub cidr: Cidr,
    /// `usuario_id` do membro com a LAN atras; `None` = atras do servidor.
    pub membro: Option<i64>,
    pub login: Option<String>,
    /// So atras do servidor: mascara a origem ao sair para a LAN.
    pub nat: bool,
}

impl Rota {
    fn subrede_vpn(&self) -> Cidr {
        Cidr {
            rede: ESPACO_VPN.rede | (u32::from(self.octeto) << 8),
            prefixo: 24,
        }
    }
}

/// A nova contra as que ja existem. Na MESMA rede nada se sobrepoe (o
/// OpenVPN escolheria um dos lados calado). Entre redes, a LAN atras do
/// servidor pode ser a mesma (e a mesma LAN, alcancada por duas redes); mas
/// filial nao sobrepoe nada de outra rede: o `route` dela vira rota do kernel
/// do servidor para uma placa so, e a outra rede perderia o destino.
pub fn conferir_conflitos(nova: &Rota, existentes: &[Rota]) -> Result<(), String> {
    for r in existentes {
        if !nova.cidr.sobrepoe(&r.cidr) {
            continue;
        }
        let onde = match &r.login {
            Some(l) => format!("atras de «{l}»"),
            None => "atras do servidor".into(),
        };
        if r.rede_id == nova.rede_id {
            return Err(format!(
                "{} sobrepoe {} ({onde}), ja nesta rede",
                nova.cidr, r.cidr
            ));
        }
        if nova.membro.is_some() || r.membro.is_some() {
            return Err(format!(
                "{} sobrepoe {} ({onde}) de outra rede: o servidor teria duas rotas para o mesmo destino",
                nova.cidr, r.cidr
            ));
        }
    }
    Ok(())
}

/// O trecho do `servidor.conf` de UMA rede.
pub fn conf_servidor(rotas: &[Rota]) -> String {
    let mut s = String::new();
    for r in rotas {
        if r.membro.is_some() {
            s.push_str(&format!("route {}\n", r.cidr.ovpn()));
        }
        s.push_str(&format!("push \"route {}\"\n", r.cidr.ovpn()));
    }
    if !s.is_empty() {
        s.insert_str(0, "# redes alcancaveis (painel: rotas da rede)\n");
    }
    s
}

/// O trecho do `ccd/` do membro que tem a filial atras.
///
/// E ele fica FORA do tunel total da rede (`saida.rs`): e um roteador, e o
/// `redirect-gateway` mandaria a internet da filial inteira para o servidor,
/// que a descarta (a filial esta na guarda). `push-remove` de opcao que nao
/// foi empurrada nao faz nada, entao as linhas vao sempre -- sem precisar
/// saber aqui se a rede tem tunel total.
pub fn ccd_membro(filiais: &[Cidr]) -> String {
    if filiais.is_empty() {
        return String::new();
    }
    let mut s: String = filiais
        .iter()
        .map(|c| format!("iroute {0}\npush-remove \"route {0}\"\n", c.ovpn()))
        .collect();
    for o in crate::saida::FORA_DO_ROTEADOR_DA_FILIAL {
        s.push_str(&format!("push-remove {o}\n"));
    }
    s
}

// ------------------------------------------------------------ firewall ----

pub const TABELA_NFT: &str = "phxvpn";
const CADEIA_ENC: &str = "PHXVPN-ENC";
const CADEIA_NAT: &str = "PHXVPN-NAT";

/// (origens, destinos com a flag de NAT) de uma rede.
type Par = (Vec<Cidr>, Vec<(Cidr, bool)>);

/// Por rede: (origens = a /24 da VPN + as filiais, destinos = as LANs atras
/// do servidor com a flag de NAT). Rede sem LAN atras do servidor nao entra:
/// o que a filial troca com os membros anda DENTRO do OpenVPN
/// (`client-to-client`), sem passar pelo encaminhamento do kernel.
fn pares(rotas: &[Rota]) -> Vec<Par> {
    let mut redes: Vec<i64> = rotas.iter().map(|r| r.rede_id).collect();
    redes.sort_unstable();
    redes.dedup();
    redes
        .into_iter()
        .filter_map(|id| {
            let da_rede: Vec<&Rota> = rotas.iter().filter(|r| r.rede_id == id).collect();
            let destinos: Vec<(Cidr, bool)> = da_rede
                .iter()
                .filter(|r| r.membro.is_none())
                .map(|r| (r.cidr, r.nat))
                .collect();
            if destinos.is_empty() {
                return None;
            }
            let mut origens = vec![da_rede[0].subrede_vpn()];
            origens.extend(
                da_rede
                    .iter()
                    .filter(|r| r.membro.is_some())
                    .map(|r| r.cidr),
            );
            Some((origens, destinos))
        })
        .collect()
}

fn filiais(rotas: &[Rota]) -> Vec<Cidr> {
    rotas
        .iter()
        .filter(|r| r.membro.is_some())
        .map(|r| r.cidr)
        .collect()
}

/// O que nunca atravessa o kernel sem um `accept` explicito: o espaco das
/// redes da VPN e as filiais.
fn guarda(rotas: &[Rota]) -> Vec<Cidr> {
    let mut g = vec![ESPACO_VPN];
    g.extend(filiais(rotas));
    g
}

fn conjunto(c: &[Cidr]) -> String {
    match c {
        [um] => um.to_string(),
        _ => format!(
            "{{ {} }}",
            c.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// O que o tunel total NUNCA alcanca pelo servidor: as faixas privadas e o
/// link-local. Tunel total e SAIDA DE INTERNET; sem esta lista ele daria de
/// brinde a LAN onde o servidor esta (e a LAN de outra rede) a quem so pediu
/// internet -- e LAN pela VPN e so por rota, que so o admin inclui. A rota
/// da propria rede continua passando: o `accept` dela vem antes.
fn fora_do_tunel_total() -> Vec<Cidr> {
    let mut v = PRIVADAS.to_vec();
    v.push(Cidr {
        rede: 0xa9fe_0000,
        prefixo: 16,
    });
    v
}

/// O script do `nft -f -`. A tabela e SEMPRE recriada inteira numa transacao
/// so (`add` + `delete` + definicao): o kernel troca a velha pela nova de uma
/// vez, sem janela com o isolamento aberto. Sem nada a encaminhar, a tabela
/// some -- e so ela.
///
/// `totais`: a /24 de cada rede com tunel total (`saida.rs`). Elas saem para
/// a internet com NAT pelo servidor, e so para a internet.
pub fn script_nft(rotas: &[Rota], totais: &[Cidr], ha_redes: bool) -> String {
    let mut s = format!("add table ip {TABELA_NFT}\ndelete table ip {TABELA_NFT}\n");
    if !ha_redes {
        return s;
    }
    let pares = pares(rotas);
    let recuo = |t: &str| t.lines().map(|l| format!("\t\t{l}\n")).collect::<String>();
    if pares.is_empty() && totais.is_empty() {
        let g = conjunto(&guarda(rotas));
        s.push_str(&format!(
            "table ip {TABELA_NFT} {{\n\
             \tchain encaminhar {{\n\t\ttype filter hook forward priority 0; policy accept;\n{}\t}}\n}}\n",
            recuo(&format!("ip saddr {g} ip daddr {g} drop\n"))
        ));
        return s;
    }
    let mut enc = String::from("ct state established,related accept\n");
    let mut nat = String::new();
    for (origens, destinos) in &pares {
        let o = conjunto(origens);
        for (d, mascara) in destinos {
            enc.push_str(&format!("ip saddr {o} ip daddr {d} accept\n"));
            enc.push_str(&format!("ip daddr {o} ip saddr {d} accept\n"));
            if *mascara {
                nat.push_str(&format!("ip saddr {o} ip daddr {d} masquerade\n"));
            }
        }
    }
    if !totais.is_empty() {
        // Depois das rotas (a LAN que o admin deu continua passando) e antes
        // da guarda (que descartaria a propria rede, parte de 10.77/16).
        let o = conjunto(totais);
        let p = conjunto(&fora_do_tunel_total());
        enc.push_str(&format!(
            "ip saddr {o} ip daddr {p} drop\nip saddr {o} accept\n"
        ));
        // So o que SAI para a internet e mascarado: a rota de volta da mesma
        // rede (sem NAT) continua mostrando o IP do membro a LAN.
        nat.push_str(&format!("ip saddr {o} ip daddr != {p} masquerade\n"));
    }
    let g = conjunto(&guarda(rotas));
    enc.push_str(&format!("ip saddr {g} drop\nip daddr {g} drop\n"));
    s.push_str(&format!(
        "table ip {TABELA_NFT} {{\n\
         \tchain encaminhar {{\n\t\ttype filter hook forward priority 0; policy accept;\n{}\t}}\n",
        recuo(&enc)
    ));
    if !nat.is_empty() {
        s.push_str(&format!(
            "\tchain saida {{\n\t\ttype nat hook postrouting priority 100; policy accept;\n{}\t}}\n",
            recuo(&nat)
        ));
    }
    s.push_str("}\n");
    s
}

/// O mesmo em `iptables-restore --noflush`: declarar a cadeia (`:NOME`) a
/// esvazia, e o COMMIT troca a tabela de uma vez. `pula_enc`/`pula_nat`: o
/// salto na cadeia do sistema ja existe (senao entraria em dobro a cada
/// aplicacao). Sem nada a encaminhar, sai o salto e somem as cadeias -- e a
/// cadeia some mesmo quando o salto ja nao existe: declarar (`:NOME`) cria a
/// que falta, e o `-X` seguinte a apaga. Condicionar o `-X` ao salto deixou
/// uma cadeia orfa na prova (salto tirado a mao, cadeia ficou).
pub fn script_iptables(
    rotas: &[Rota],
    totais: &[Cidr],
    ha_redes: bool,
    pula_enc: bool,
    pula_nat: bool,
) -> String {
    let pares = pares(rotas);
    let mut f = String::from("*filter\n");
    let mut n = String::from("*nat\n");
    if pares.is_empty() && totais.is_empty() {
        if ha_redes {
            // So a guarda: sem NAT, sem aceitar nada -- entre as redes da
            // VPN (e as filiais delas) nada atravessa o kernel.
            f.push_str(&format!(":{CADEIA_ENC} - [0:0]\n"));
            let g = guarda(rotas);
            for o in &g {
                for d in &g {
                    f.push_str(&format!("-A {CADEIA_ENC} -s {o} -d {d} -j DROP\n"));
                }
            }
            if !pula_enc {
                f.push_str(&format!("-I FORWARD 1 -j {CADEIA_ENC}\n"));
            }
        } else {
            f.push_str(&format!(":{CADEIA_ENC} - [0:0]\n"));
            if pula_enc {
                f.push_str(&format!("-D FORWARD -j {CADEIA_ENC}\n"));
            }
            f.push_str(&format!("-X {CADEIA_ENC}\n"));
        }
        n.push_str(&format!(":{CADEIA_NAT} - [0:0]\n"));
        if pula_nat {
            n.push_str(&format!("-D POSTROUTING -j {CADEIA_NAT}\n"));
        }
        n.push_str(&format!("-X {CADEIA_NAT}\n"));
        return format!("{f}COMMIT\n{n}COMMIT\n");
    }
    f.push_str(&format!(":{CADEIA_ENC} - [0:0]\n"));
    n.push_str(&format!(":{CADEIA_NAT} - [0:0]\n"));
    f.push_str(&format!(
        "-A {CADEIA_ENC} -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT\n"
    ));
    for (origens, destinos) in &pares {
        for (d, mascara) in destinos {
            for o in origens {
                f.push_str(&format!("-A {CADEIA_ENC} -s {o} -d {d} -j ACCEPT\n"));
                f.push_str(&format!("-A {CADEIA_ENC} -s {d} -d {o} -j ACCEPT\n"));
                if *mascara {
                    n.push_str(&format!("-A {CADEIA_NAT} -s {o} -d {d} -j MASQUERADE\n"));
                }
            }
        }
    }
    // Tunel total: o mesmo lugar e a mesma regra do nft. O iptables nao nega
    // uma lista, entao o NAT devolve (RETURN) o privado antes de mascarar.
    for o in totais {
        for p in fora_do_tunel_total() {
            f.push_str(&format!("-A {CADEIA_ENC} -s {o} -d {p} -j DROP\n"));
            n.push_str(&format!("-A {CADEIA_NAT} -s {o} -d {p} -j RETURN\n"));
        }
        f.push_str(&format!("-A {CADEIA_ENC} -s {o} -j ACCEPT\n"));
        n.push_str(&format!("-A {CADEIA_NAT} -s {o} -j MASQUERADE\n"));
    }
    for g in guarda(rotas) {
        f.push_str(&format!("-A {CADEIA_ENC} -s {g} -j DROP\n"));
        f.push_str(&format!("-A {CADEIA_ENC} -d {g} -j DROP\n"));
    }
    if !pula_enc {
        f.push_str(&format!("-I FORWARD 1 -j {CADEIA_ENC}\n"));
    }
    if !pula_nat {
        n.push_str(&format!("-I POSTROUTING 1 -j {CADEIA_NAT}\n"));
    }
    format!("{f}COMMIT\n{n}COMMIT\n")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motor {
    Nft,
    Iptables,
}

impl Motor {
    pub fn nome(&self) -> &'static str {
        match self {
            Motor::Nft => "nft",
            Motor::Iptables => "iptables",
        }
    }
}

/// nftables quando existe e responde; senao iptables. `PHXVPN_FIREWALL`
/// (`nft` | `iptables`) forca um dos dois -- e o que a prova usa para
/// exercitar os dois caminhos na mesma maquina.
pub fn detectar() -> Option<Motor> {
    let tem = |b: &str| crate::supervisor::achar_no_path(b).is_some();
    let nft_vivo = || {
        tem("nft")
            && std::process::Command::new("nft")
                .args(["list", "tables"])
                .output()
                .is_ok_and(|s| s.status.success())
    };
    match std::env::var("PHXVPN_FIREWALL").as_deref() {
        Ok("nft") => return nft_vivo().then_some(Motor::Nft),
        Ok("iptables") => return tem("iptables-restore").then_some(Motor::Iptables),
        _ => {}
    }
    if nft_vivo() {
        Some(Motor::Nft)
    } else if tem("iptables-restore") {
        Some(Motor::Iptables)
    } else {
        None
    }
}

fn rodar(bin: &str, args: &[&str], entrada: Option<&str>) -> R<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut c = Command::new(bin);
    c.args(args)
        .stdin(if entrada.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut filho = c.spawn().map_err(|e| format!("{bin}: {e}"))?;
    if let (Some(t), Some(mut e)) = (entrada, filho.stdin.take()) {
        e.write_all(t.as_bytes())
            .map_err(|e| format!("{bin}: {e}"))?;
    }
    let s = filho
        .wait_with_output()
        .map_err(|e| format!("{bin}: {e}"))?;
    if s.status.success() {
        Ok(String::from_utf8_lossy(&s.stdout).into_owned())
    } else {
        Err(format!(
            "{bin} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&s.stderr).trim()
        ))
    }
}

const IP_FORWARD: &str = "/proc/sys/net/ipv4/ip_forward";
/// O valor do `ip_forward` antes de o painel liga-lo: quando a ultima rota
/// sai, ele volta ao que era -- encaminhamento aceso sem a tabela de guarda
/// deixaria a rede A falar com a rede B pelo kernel.
const ARQ_FORWARD_ANTES: &str = "rotas-ip_forward-antes";

/// O que a aplicacao fez, para a resposta da API e para a prova.
#[derive(Debug, Default)]
pub struct Aplicado {
    pub motor: Option<Motor>,
    pub regras: usize,
    pub aviso: Option<String>,
}

/// Aplica o firewall de TODAS as rotas (e sempre o estado inteiro: e o que
/// deixa remover igual a incluir, e o arranque igual aos dois) e acerta o
/// `ip_forward`. Com rede e sem rota, fica so a guarda (sem NAT, sem mexer
/// no `ip_forward`); sem rede nenhuma, a tabela some.
pub fn aplicar(rotas: &[Rota], totais: &[Cidr], ha_redes: bool, dados: &Path) -> R<Aplicado> {
    let motor = detectar().ok_or(
        "sem nftables nem iptables no servidor: o isolamento entre as redes e o encaminhamento para a LAN nao tem como ser guardado",
    )?;
    let vazio = pares(rotas).is_empty() && totais.is_empty();
    let marca = dados.join(ARQ_FORWARD_ANTES);
    if !vazio {
        let antes =
            std::fs::read_to_string(IP_FORWARD).map_err(|e| format!("{IP_FORWARD}: {e}"))?;
        if antes.trim() == "0" && !marca.exists() {
            std::fs::write(&marca, "0\n").map_err(|e| format!("{}: {e}", marca.display()))?;
        }
    }
    // Regras antes do encaminhamento: o `ip_forward` nunca fica aceso sem a
    // guarda no lugar.
    match motor {
        Motor::Nft => {
            rodar(
                "nft",
                &["-f", "-"],
                Some(&script_nft(rotas, totais, ha_redes)),
            )?;
        }
        Motor::Iptables => {
            let tem = |t: &str, c: &str, alvo: &str| {
                rodar("iptables", &["-t", t, "-C", c, "-j", alvo], None).is_ok()
            };
            let texto = script_iptables(
                rotas,
                totais,
                ha_redes,
                tem("filter", "FORWARD", CADEIA_ENC),
                tem("nat", "POSTROUTING", CADEIA_NAT),
            );
            rodar("iptables-restore", &["--noflush"], Some(&texto))?;
        }
    }
    if !vazio {
        std::fs::write(IP_FORWARD, "1\n").map_err(|e| format!("{IP_FORWARD}: {e}"))?;
    } else if let Ok(v) = std::fs::read_to_string(&marca) {
        std::fs::write(IP_FORWARD, v.trim()).map_err(|e| format!("{IP_FORWARD}: {e}"))?;
        let _ = std::fs::remove_file(&marca);
    }
    Ok(Aplicado {
        motor: Some(motor),
        regras: contar_regras(motor),
        aviso: if vazio { None } else { bloqueio_alheio(motor) },
    })
}

/// Quantas regras NOSSAS estao no kernel agora (a prova conta antes/depois).
pub fn contar_regras(motor: Motor) -> usize {
    match motor {
        Motor::Nft => rodar("nft", &["-a", "list", "table", "ip", TABELA_NFT], None)
            .map(|t| {
                t.lines()
                    .filter(|l| {
                        l.contains("# handle") && !l.contains("chain ") && !l.contains("table ")
                    })
                    .count()
            })
            .unwrap_or(0),
        Motor::Iptables => [("filter", CADEIA_ENC), ("nat", CADEIA_NAT)]
            .iter()
            .map(|(t, c)| {
                rodar("iptables", &["-t", t, "-S", c], None)
                    .map(|s| s.lines().filter(|l| l.starts_with("-A ")).count())
                    .unwrap_or(0)
            })
            .sum(),
    }
}

/// Um `policy drop` no forward de outra tabela vence o nosso `accept`: o
/// painel nao mexe nele, mas diz que achou.
fn bloqueio_alheio(motor: Motor) -> Option<String> {
    let achou = match motor {
        Motor::Nft => {
            let t = rodar("nft", &["list", "ruleset"], None).unwrap_or_default();
            forward_com_drop_alheio(&t)
        }
        Motor::Iptables => rodar("iptables", &["-S", "FORWARD"], None)
            .is_ok_and(|t| t.lines().any(|l| l.trim() == "-P FORWARD DROP")),
    };
    achou.then(|| {
        "ha outra regra de firewall com «policy drop» no encaminhamento (Docker, firewalld...): ela vence a do phxvpn -- libere nela o trafego da VPN para a LAN"
            .into()
    })
}

fn forward_com_drop_alheio(ruleset: &str) -> bool {
    let mut tabela = "";
    for l in ruleset.lines() {
        let l = l.trim();
        if let Some(t) = l.strip_prefix("table ") {
            tabela = t.trim_end_matches('{').trim();
        } else if l.contains("hook forward")
            && l.contains("policy drop")
            && tabela != format!("ip {TABELA_NFT}")
        {
            return true;
        }
    }
    false
}

// ------------------------------------------------------------- painel ----

const COLUNAS: &str = "SELECT t.rede_id, r.octeto, t.cidr, t.membro_id, t.volta, u.login \
     FROM phx_rota t JOIN phx_rede r ON r.id = t.rede_id \
     LEFT JOIN phx_usuario u ON u.id = t.membro_id";

impl Painel {
    /// Todas as rotas, de todas as redes (o firewall e um so no host).
    pub fn rotas_todas(&mut self) -> R<Vec<Rota>> {
        self.rotas_onde(&format!("{COLUNAS} ORDER BY t.rede_id, t.cidr"), &[])
    }

    pub(crate) fn rotas_da_rede(&mut self, rede_id: &str) -> R<Vec<Rota>> {
        self.rotas_onde(
            &format!("{COLUNAS} WHERE t.rede_id = $1::int ORDER BY t.cidr"),
            &[Some(rede_id)],
        )
    }

    fn rotas_onde(&mut self, sql: &str, p: &[Option<&str>]) -> R<Vec<Rota>> {
        let r = self.pg()?.executar(sql, p)?;
        (0..r.linhas.len())
            .map(|i| {
                let v = |c: &str| r.valor(i, c).unwrap_or_default().to_string();
                Ok(Rota {
                    rede_id: v("rede_id").parse().map_err(|_| "rota sem rede")?,
                    octeto: v("octeto").parse().map_err(|_| "octeto invalido")?,
                    cidr: Cidr::analisar(&v("cidr"))?,
                    membro: r.valor(i, "membro_id").and_then(|m| m.parse().ok()),
                    login: r.valor(i, "login").map(str::to_string),
                    nat: v("volta") == "nat",
                })
            })
            .collect()
    }

    /// O trecho de rotas do `servidor.conf` (chamado pelo `materializar_rede`).
    pub(crate) fn conf_das_rotas(&mut self, rede_id: &str) -> R<String> {
        Ok(conf_servidor(&self.rotas_da_rede(rede_id)?))
    }

    /// As filiais de cada (rede, usuario), para o `ccd/`. UM lugar so: o
    /// «entrar» e o `acertar_ccd` escrevem o mesmo arquivo, e o que um
    /// esquecesse do `iroute` o outro apagaria na volta seguinte da vigia.
    pub(crate) fn filiais_por_membro(&mut self) -> R<HashMap<(i64, i64), Vec<Cidr>>> {
        let mut m: HashMap<(i64, i64), Vec<Cidr>> = HashMap::new();
        for r in self.rotas_todas()? {
            if let Some(u) = r.membro {
                m.entry((r.rede_id, u)).or_default().push(r.cidr);
            }
        }
        Ok(m)
    }

    /// Recusa tirar da rede quem tem filial atras: a rota e FILHA do vinculo,
    /// e o pai com filhos nao morre (regra primordial). A frase diz o que
    /// fazer, em vez do erro cru da chave estrangeira.
    pub(crate) fn recusar_se_tem_filial(&mut self, rede_id: i64, usuario_id: i64) -> R<()> {
        let r = self.pg()?.executar(
            "SELECT cidr FROM phx_rota WHERE rede_id = $1::int AND membro_id = $2::int ORDER BY cidr",
            &[Some(&rede_id.to_string()), Some(&usuario_id.to_string())],
        )?;
        if r.linhas.is_empty() {
            return Ok(());
        }
        let lista: Vec<&str> = (0..r.linhas.len())
            .filter_map(|i| r.valor(i, "cidr"))
            .collect();
        Err(format!(
            "este membro tem rede atras dele ({}): o administrador remove a rota antes",
            lista.join(", ")
        ))
    }

    /// Quem ve a configuracao de uma rede (rotas, saida): o administrador,
    /// o dono ou um membro.
    pub(crate) fn pode_ver_rede(&mut self, u: &Usuario, rede_id: i64) -> R<()> {
        let pode = self.pg()?.executar(
            "SELECT 1 FROM phx_rede r WHERE r.id = $1::int AND (r.dono_id = $2::int \
               OR EXISTS (SELECT 1 FROM phx_membro m WHERE m.rede_id = r.id AND m.usuario_id = $2::int))",
            &[Some(&rede_id.to_string()), Some(&u.id.to_string())],
        )?;
        if !u.admin && pode.linhas.is_empty() {
            return Err("so o administrador, o dono ou um membro ve a configuracao da rede".into());
        }
        Ok(())
    }

    /// As rotas de uma rede, para quem a administra ou a integra.
    pub fn rotas(&mut self, u: &Usuario, rede_id: i64) -> R<Json> {
        let id = rede_id.to_string();
        self.pode_ver_rede(u, rede_id)?;
        Ok(Json::Lista(
            self.rotas_da_rede(&id)?
                .into_iter()
                .map(|r| {
                    Json::objeto(vec![
                        ("cidr", Json::texto_de(r.cidr.to_string())),
                        ("membro", r.login.map(Json::texto_de).unwrap_or(Json::Nulo)),
                        (
                            "volta",
                            Json::texto_de(match (r.membro, r.nat) {
                                (Some(_), _) => "filial",
                                (None, true) => "nat",
                                (None, false) => "rota",
                            }),
                        ),
                    ])
                })
                .collect(),
        ))
    }

    /// Inclui uma rede alcancavel. So o ADMINISTRADOR: qualquer usuario pode
    /// ser dono de rede (`REDES_POR_USUARIO`), e rota para a LAN atras do
    /// servidor com NAT e acesso a LAN da empresa inteira -- dar isso ao
    /// dono seria deixar qualquer usuario se dar a LAN.
    ///
    /// `membro` vazio: atras do servidor, com `volta` "nat" (padrao) ou
    /// "rota". `membro` = login: a filial atras daquele membro.
    /// Devolve (nome da rede, pasta) para quem reinicia o OpenVPN.
    pub fn rota_incluir(
        &mut self,
        ator: &Usuario,
        rede_id: i64,
        cidr: &str,
        volta: &str,
        membro: &str,
    ) -> R<(String, PathBuf)> {
        if !ator.admin {
            return Err("so o administrador inclui rede alcancavel pela VPN".into());
        }
        let cidr = Cidr::analisar(cidr)?;
        validar(&cidr)?;
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT nome, octeto FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let nome = r.valor(0, "nome").ok_or("rede inexistente")?.to_string();
        let octeto: u8 = r
            .valor(0, "octeto")
            .and_then(|o| o.parse().ok())
            .ok_or("octeto invalido")?;
        let nat = match volta {
            "" | "nat" => true,
            "rota" => false,
            _ => return Err("volta: nat ou rota".into()),
        };
        let membro_id = if membro.trim().is_empty() {
            None
        } else {
            let m = self.pg()?.executar(
                "SELECT m.usuario_id FROM phx_membro m JOIN phx_usuario u ON u.id = m.usuario_id \
                 WHERE m.rede_id = $1::int AND u.login = $2",
                &[Some(&id), Some(membro.trim())],
            )?;
            Some(
                m.valor(0, "usuario_id")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| format!("«{}» nao e membro desta rede", membro.trim()))?,
            )
        };
        let nova = Rota {
            rede_id,
            octeto,
            cidr,
            membro: membro_id,
            login: None,
            nat: nat && membro_id.is_none(),
        };
        conferir_conflitos(&nova, &self.rotas_todas()?)?;
        self.pg()?.executar(
            "INSERT INTO phx_rota (rede_id, cidr, membro_id, volta) VALUES ($1::int, $2, $3::int, $4)",
            &[
                Some(&id),
                Some(&cidr.to_string()),
                membro_id.map(|m| m.to_string()).as_deref(),
                Some(if nova.nat { "nat" } else { "rota" }),
            ],
        )?;
        match self.rotas_gravar_arquivos(&id, membro_id) {
            Ok(dir) => Ok((nome, dir)),
            Err(e) => {
                let _ = self.rota_apagar(rede_id, &cidr);
                Err(e)
            }
        }
    }

    /// Tira uma rede alcancavel: o administrador ou o dono da rede (tirar so
    /// ESTREITA o acesso; quem nao pode dar pode ao menos fechar).
    pub fn rota_remover(
        &mut self,
        ator: &Usuario,
        rede_id: i64,
        cidr: &str,
    ) -> R<(String, PathBuf)> {
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT nome, dono_id FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let dono: i64 = r
            .valor(0, "dono_id")
            .and_then(|v| v.parse().ok())
            .ok_or("rede inexistente")?;
        if !ator.admin && ator.id != dono {
            return Err("so o administrador ou o dono da rede remove rota".into());
        }
        let nome = r.valor(0, "nome").unwrap_or_default().to_string();
        let cidr = Cidr::analisar(cidr)?;
        let membro = self.rota_apagar(rede_id, &cidr)?;
        Ok((nome, self.rotas_gravar_arquivos(&id, membro)?))
    }

    /// Apaga a linha e devolve o membro dela; sem linha, erro.
    fn rota_apagar(&mut self, rede_id: i64, cidr: &Cidr) -> R<Option<i64>> {
        let r = self.pg()?.executar(
            "DELETE FROM phx_rota WHERE rede_id = $1::int AND cidr = $2 RETURNING membro_id",
            &[Some(&rede_id.to_string()), Some(&cidr.to_string())],
        )?;
        if r.linhas.is_empty() {
            return Err(format!("{cidr} nao e rota desta rede"));
        }
        let membro = r.valor(0, "membro_id").and_then(|m| m.parse().ok());
        // Desfazer uma inclusao que nao se completou reescreve os arquivos
        // de volta; aqui so a linha.
        Ok(membro)
    }

    /// O conf da rede e o `ccd/` do membro da filial, reescritos.
    fn rotas_gravar_arquivos(&mut self, rede_id: &str, membro: Option<i64>) -> R<PathBuf> {
        let dir = self.materializar_rede(rede_id)?;
        if let Some(m) = membro {
            self.acertar_ccd(Some(m))?;
        }
        Ok(dir)
    }

    /// Desfaz uma inclusao cujo efeito (firewall, OpenVPN) falhou: a linha
    /// sai e os arquivos voltam -- a rota nunca fica gravada dizendo que
    /// existe sem o servidor a levar.
    pub fn rota_desfazer(&mut self, rede_id: i64, cidr: &str) -> R<()> {
        let cidr = Cidr::analisar(cidr)?;
        let membro = self.rota_apagar(rede_id, &cidr)?;
        self.rotas_gravar_arquivos(&rede_id.to_string(), membro)
            .map(|_| ())
    }
}

/// O firewall de todas as rotas, no estado do painel: grava o script em
/// `dados/rotas.nft` (sempre -- quem sobe o OpenVPN por fora aplica ele) e,
/// com o supervisor ligado, aplica no kernel.
pub fn aplicar_no_host(e: &crate::http::Estado) -> R<Json> {
    let (rotas, totais, ha_redes, dados) = {
        let mut p = e.painel();
        let n = p
            .pg()?
            .executar("SELECT count(*) AS n FROM phx_rede", &[])?;
        let ha = n
            .valor(0, "n")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
            > 0;
        (
            p.rotas_todas()?,
            p.origens_do_tunel_total()?,
            ha,
            p.dados().to_path_buf(),
        )
    };
    let arquivo = dados.join("rotas.nft");
    crate::painel::gravar(
        &arquivo,
        script_nft(&rotas, &totais, ha_redes).as_bytes(),
        false,
    )?;
    if e.supervisor.is_none() {
        return Ok(Json::objeto(vec![
            ("aplicado", Json::de_bool(false)),
            (
                "aviso",
                Json::texto_de(format!(
                    "painel sem --openvpn: o firewall nao foi aplicado; aplique {} (nft -f) e ligue o ip_forward",
                    arquivo.display()
                )),
            ),
        ]));
    }
    let a = aplicar(&rotas, &totais, ha_redes, &dados)?;
    Ok(Json::objeto(vec![
        ("aplicado", Json::de_bool(true)),
        (
            "motor",
            Json::texto_de(a.motor.map(|m| m.nome()).unwrap_or("")),
        ),
        ("regras", Json::de_i64(a.regras as i64)),
        ("aviso", a.aviso.map(Json::texto_de).unwrap_or(Json::Nulo)),
    ]))
}

/// Depois de gravar uma inclusao ou remocao: firewall e OpenVPN. Inclusao
/// cujo firewall nao se aplica e DESFEITA (a rota nunca fica dizendo que
/// existe com o encaminhamento sem guarda); remocao cujo firewall falha fica
/// removida no banco e o erro diz que o kernel ainda tem a regra velha.
pub fn depois_de_mudar(
    e: &crate::http::Estado,
    incluiu: bool,
    rede_id: i64,
    cidr: &str,
    nome: &str,
    dir: &Path,
) -> R<Json> {
    let desfazer = || {
        let _ = e.painel().rota_desfazer(rede_id, cidr);
    };
    efetivar(e, incluiu.then_some(&desfazer), "a rota", nome, dir)
}

/// O passo comum de quem muda o que o servidor encaminha (rota, tunel
/// total): firewall do estado inteiro, depois o OpenVPN da rede. Mudanca que
/// ALARGA traz `desfazer` -- se o firewall nao se aplica, ela sai do cadastro
/// e o firewall volta, e o encaminhamento nunca fica dito sem guarda. A que
/// estreita fica gravada, e o erro diz que o kernel ainda tem a regra velha.
pub fn efetivar(
    e: &crate::http::Estado,
    desfazer: Option<&dyn Fn()>,
    o_que: &str,
    nome: &str,
    dir: &Path,
) -> R<Json> {
    let fw = match (aplicar_no_host(e), desfazer) {
        (Ok(j), _) => j,
        (Err(m), Some(desfazer)) => {
            desfazer();
            let _ = aplicar_no_host(e);
            return Err(format!("{o_que} nao entrou: {m}"));
        }
        (Err(m), None) => {
            return Err(format!(
                "{o_que} mudou no cadastro, mas o firewall nao se reaplicou ({m}); o arranque seguinte do painel o reaplica"
            ))
        }
    };
    // Os resolvedores seguem o cadastro (liga, desliga, zona nova).
    crate::saida::acertar_dns(e);
    // O OpenVPN so le o conf ao subir: `push`/`route` novos pedem reinicio,
    // e os membros recebem as rotas ao reconectar.
    let reiniciado = match &e.supervisor {
        Some(s) => {
            s.reiniciar(nome, dir)?;
            true
        }
        None => false,
    };
    Ok(Json::objeto(vec![
        ("ok", Json::de_bool(true)),
        ("firewall", fw),
        ("openvpn_reiniciado", Json::de_bool(reiniciado)),
    ]))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c(t: &str) -> Cidr {
        Cidr::analisar(t).unwrap()
    }

    fn rota(rede_id: i64, octeto: u8, cidr: &str, membro: Option<i64>, nat: bool) -> Rota {
        Rota {
            rede_id,
            octeto,
            cidr: c(cidr),
            membro,
            login: membro.map(|m| format!("m{m}")),
            nat,
        }
    }

    #[test]
    fn cidr_torto_e_recusado() {
        for ruim in [
            "192.168.10.0",
            "192.168.10.0/33",
            "192.168.10/24",
            "192.168.10.0.0/24",
            "192.168.010.0/24",
            "192.168.256.0/24",
            "192.168.10.0/024",
            "192.168.10.0/",
            "/24",
            "192.168.10.0/24\npush \"route 0.0.0.0 0.0.0.0\"",
            "192.168.10.0 /24",
            "-1.0.0.0/8",
            "192.168.10.0/2a",
        ] {
            assert!(Cidr::analisar(ruim).is_err(), "{ruim:?} passou");
        }
        assert_eq!(c("192.168.10.0/24").to_string(), "192.168.10.0/24");
        assert_eq!(c("192.168.10.0/24").mascara(), "255.255.255.0");
        assert_eq!(c("10.1.2.3/32").mascara(), "255.255.255.255");
        assert_eq!(c("0.0.0.0/0").mascara(), "0.0.0.0");
    }

    #[test]
    fn bit_de_host_ligado_e_recusado_e_diz_a_rede() {
        let e = Cidr::analisar("192.168.10.5/24").unwrap_err();
        assert!(e.contains("192.168.10.0/24"), "{e}");
    }

    #[test]
    fn tunel_total_e_recusado_com_o_motivo() {
        let e = validar(&c("0.0.0.0/0")).unwrap_err();
        assert!(e.contains("tunel total"), "{e}");
    }

    #[test]
    fn faixa_publica_e_recusada() {
        for ruim in [
            "8.8.8.0/24",
            "0.0.0.0/1",
            "128.0.0.0/1",
            "172.32.0.0/16",
            "192.0.0.0/8",
        ] {
            let e = validar(&c(ruim)).unwrap_err();
            assert!(e.contains("privada"), "{ruim}: {e}");
        }
        for bom in [
            "192.168.10.0/24",
            "10.0.0.0/16",
            "172.16.0.0/12",
            "100.64.1.0/24",
            "192.168.10.5/32",
        ] {
            validar(&c(bom)).unwrap_or_else(|e| panic!("{bom}: {e}"));
        }
    }

    #[test]
    fn espaco_da_vpn_e_recusado() {
        for ruim in [
            "10.77.1.0/24",
            "10.77.0.0/16",
            "10.0.0.0/8",
            "10.76.0.0/15",
            "10.77.200.9/32",
        ] {
            let e = validar(&c(ruim)).unwrap_err();
            assert!(e.contains("espaco das proprias redes"), "{ruim}: {e}");
        }
        validar(&c("10.78.0.0/16")).unwrap();
    }

    #[test]
    fn sobreposicao_na_mesma_rede_e_recusada() {
        let ja = [rota(1, 1, "192.168.0.0/16", None, true)];
        for ruim in ["192.168.10.0/24", "192.168.0.0/16", "192.0.0.0/8"] {
            let n = Rota {
                cidr: Cidr::analisar(ruim).unwrap(),
                ..ja[0].clone()
            };
            let e = conferir_conflitos(&n, &ja).unwrap_err();
            assert!(e.contains("ja nesta rede"), "{ruim}: {e}");
        }
        let fora = rota(1, 1, "172.16.0.0/24", None, true);
        conferir_conflitos(&fora, &ja).unwrap();
    }

    #[test]
    fn filial_nao_sobrepoe_nada_de_outra_rede_mas_a_lan_do_servidor_pode_repetir() {
        let ja = [
            rota(1, 1, "192.168.10.0/24", None, true),
            rota(1, 1, "192.168.20.0/24", Some(7), false),
        ];
        // A mesma LAN atras do servidor, alcancada por outra rede: pode.
        conferir_conflitos(&rota(2, 2, "192.168.10.0/24", None, true), &ja).unwrap();
        // Filial de outra rede na LAN do servidor, ou na filial alheia: nao.
        let e =
            conferir_conflitos(&rota(2, 2, "192.168.10.0/25", Some(9), false), &ja).unwrap_err();
        assert!(e.contains("duas rotas"), "{e}");
        let e = conferir_conflitos(&rota(2, 2, "192.168.20.0/24", None, true), &ja).unwrap_err();
        assert!(e.contains("duas rotas"), "{e}");
    }

    #[test]
    fn conf_e_ccd_levam_as_diretivas() {
        let rs = [
            rota(1, 3, "192.168.10.0/24", None, true),
            rota(1, 3, "192.168.20.0/24", Some(7), false),
        ];
        let conf = conf_servidor(&rs);
        assert!(conf.contains("push \"route 192.168.10.0 255.255.255.0\"\n"));
        assert!(!conf.contains("route 192.168.10.0 255.255.255.0\nroute"));
        assert!(
            conf.contains(
                "\nroute 192.168.20.0 255.255.255.0\npush \"route 192.168.20.0 255.255.255.0\"\n"
            ),
            "{conf}"
        );
        assert!(conf_servidor(&[]).is_empty());
        let ccd = ccd_membro(&[c("192.168.20.0/24")]);
        assert!(
            ccd.starts_with(
                "iroute 192.168.20.0 255.255.255.0\npush-remove \"route 192.168.20.0 255.255.255.0\"\n"
            ),
            "{ccd}"
        );
        // O roteador da filial nao recebe o tunel total da rede.
        assert!(ccd.contains("push-remove redirect-gateway\n"), "{ccd}");
        assert!(ccd.contains("push-remove ifconfig-ipv6\n"), "{ccd}");
        assert!(ccd_membro(&[]).is_empty());
    }

    #[test]
    fn sem_rede_nenhuma_a_tabela_some_e_so_ela() {
        let s = script_nft(&[], &[], false);
        assert_eq!(s, "add table ip phxvpn\ndelete table ip phxvpn\n");
        let t = script_iptables(&[], &[], false, true, true);
        assert!(t.contains("-D FORWARD -j PHXVPN-ENC\n-X PHXVPN-ENC"), "{t}");
        assert!(
            t.contains("-D POSTROUTING -j PHXVPN-NAT\n-X PHXVPN-NAT"),
            "{t}"
        );
        // Sem o salto (tirado por fora), a cadeia orfa some do mesmo jeito.
        assert_eq!(
            script_iptables(&[], &[], false, false, false),
            "*filter\n:PHXVPN-ENC - [0:0]\n-X PHXVPN-ENC\nCOMMIT\n*nat\n:PHXVPN-NAT - [0:0]\n-X PHXVPN-NAT\nCOMMIT\n"
        );
    }

    /// Com rede e sem rota: so a guarda entre os enderecos da VPN, sem NAT
    /// e sem aceitar nada -- o host que ja encaminha nao abre A para B.
    #[test]
    fn com_rede_e_sem_rota_fica_so_a_guarda() {
        let s = script_nft(&[], &[], true);
        assert!(
            s.contains("ip saddr 10.77.0.0/16 ip daddr 10.77.0.0/16 drop"),
            "{s}"
        );
        assert!(
            !s.contains("accept\n") && !s.contains("masquerade") && !s.contains("nat"),
            "{s}"
        );
        // So filial: ela entra na guarda (o trafego dela com os membros anda
        // dentro do OpenVPN, nada precisa do kernel).
        let s = script_nft(&[rota(1, 1, "192.168.20.0/24", Some(7), false)], &[], true);
        assert!(
            s.contains("ip saddr { 10.77.0.0/16, 192.168.20.0/24 } ip daddr { 10.77.0.0/16, 192.168.20.0/24 } drop"),
            "{s}"
        );
        let t = script_iptables(&[], &[], true, false, true);
        assert!(
            t.contains("-A PHXVPN-ENC -s 10.77.0.0/16 -d 10.77.0.0/16 -j DROP"),
            "{t}"
        );
        assert!(t.contains("-I FORWARD 1 -j PHXVPN-ENC"), "{t}");
        // O NAT de uma rota que saiu vai embora junto.
        assert!(
            t.contains("-D POSTROUTING -j PHXVPN-NAT\n-X PHXVPN-NAT"),
            "{t}"
        );
        assert!(!script_iptables(&[], &[], true, true, false).contains("-I FORWARD"));
    }

    #[test]
    fn nft_guarda_o_isolamento_entre_redes_e_so_mascara_o_nat() {
        let rs = [
            rota(1, 1, "192.168.10.0/24", None, true),
            rota(1, 1, "192.168.20.0/24", Some(7), false),
            rota(2, 2, "172.16.5.0/24", None, false),
        ];
        let s = script_nft(&rs, &[], true);
        assert!(
            s.contains(
                "ip saddr { 10.77.1.0/24, 192.168.20.0/24 } ip daddr 192.168.10.0/24 accept"
            ),
            "{s}"
        );
        assert!(
            s.contains(
                "ip saddr { 10.77.1.0/24, 192.168.20.0/24 } ip daddr 192.168.10.0/24 masquerade"
            ),
            "{s}"
        );
        assert!(
            s.contains("ip saddr 10.77.2.0/24 ip daddr 172.16.5.0/24 accept"),
            "{s}"
        );
        assert!(
            !s.contains("172.16.5.0/24 masquerade"),
            "rota de volta nao mascara: {s}"
        );
        // A guarda vem DEPOIS dos accept e fecha o resto do espaco da VPN.
        let guarda = s
            .find("ip saddr { 10.77.0.0/16, 192.168.20.0/24 } drop")
            .expect(&s);
        assert!(guarda > s.find("accept").unwrap());
        assert!(s.contains("ip daddr { 10.77.0.0/16, 192.168.20.0/24 } drop"));
        let t = script_iptables(&rs, &[], true, false, true);
        assert!(t.contains("-A PHXVPN-ENC -s 10.77.1.0/24 -d 192.168.10.0/24 -j ACCEPT"));
        assert!(t.contains("-A PHXVPN-NAT -s 192.168.20.0/24 -d 192.168.10.0/24 -j MASQUERADE"));
        assert!(t.contains("-A PHXVPN-ENC -d 10.77.0.0/16 -j DROP"));
        assert!(t.contains("-I FORWARD 1 -j PHXVPN-ENC"));
        assert!(!t.contains("-I POSTROUTING"), "salto em dobro: {t}");
    }

    /// Tunel total: a rede sai para a internet com NAT, mas nao para faixa
    /// privada (a LAN do servidor, a de outra rede) -- LAN so por rota. E a
    /// rota da propria rede, e a rota de volta sem NAT, continuam iguais.
    #[test]
    fn tunel_total_sai_com_nat_so_para_a_internet() {
        let rs = [
            rota(1, 1, "192.168.10.0/24", None, false),
            rota(2, 2, "172.16.5.0/24", None, true),
        ];
        let tt = [c("10.77.1.0/24")];
        let s = script_nft(&rs, &tt, true);
        let privadas =
            "{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 100.64.0.0/10, 169.254.0.0/16 }";
        let barra = format!("ip saddr 10.77.1.0/24 ip daddr {privadas} drop");
        let sai = "ip saddr 10.77.1.0/24 accept";
        let (i_rota, i_barra, i_sai, i_guarda) = (
            s.find("ip saddr 10.77.1.0/24 ip daddr 192.168.10.0/24 accept")
                .expect(&s),
            s.find(&barra).expect(&s),
            s.find(sai).expect(&s),
            s.find("ip saddr 10.77.0.0/16 drop").expect(&s),
        );
        assert!(
            i_rota < i_barra && i_barra < i_sai && i_sai < i_guarda,
            "{s}"
        );
        assert!(
            s.contains(&format!(
                "ip saddr 10.77.1.0/24 ip daddr != {privadas} masquerade"
            )),
            "{s}"
        );
        // A rota de volta da mesma rede nao ganha NAT pelo tunel total.
        assert!(!s.contains("ip saddr 10.77.1.0/24 masquerade"), "{s}");
        // So o tunel total, sem rota nenhuma: sai do modo «so a guarda».
        let s = script_nft(&[], &tt, true);
        assert!(s.contains(sai) && s.contains("masquerade"), "{s}");
        let t = script_iptables(&[], &tt, true, false, false);
        let i_barra = t
            .find("-A PHXVPN-ENC -s 10.77.1.0/24 -d 192.168.0.0/16 -j DROP")
            .expect(&t);
        let i_sai = t.find("-A PHXVPN-ENC -s 10.77.1.0/24 -j ACCEPT").expect(&t);
        let i_guarda = t.find("-A PHXVPN-ENC -s 10.77.0.0/16 -j DROP").expect(&t);
        assert!(i_barra < i_sai && i_sai < i_guarda, "{t}");
        let i_volta = t
            .find("-A PHXVPN-NAT -s 10.77.1.0/24 -d 10.0.0.0/8 -j RETURN")
            .expect(&t);
        let i_nat = t
            .find("-A PHXVPN-NAT -s 10.77.1.0/24 -j MASQUERADE")
            .expect(&t);
        assert!(i_volta < i_nat, "{t}");
        assert!(t.contains("-I POSTROUTING 1 -j PHXVPN-NAT"), "{t}");
    }

    #[test]
    fn drop_alheio_no_forward_e_achado_e_o_nosso_nao() {
        let docker = "table ip filter {\n\tchain FORWARD {\n\t\ttype filter hook forward priority filter; policy drop;\n\t}\n}\n";
        assert!(forward_com_drop_alheio(docker));
        let nosso = "table ip phxvpn {\n\tchain encaminhar {\n\t\ttype filter hook forward priority filter; policy drop;\n\t}\n}\n";
        assert!(!forward_com_drop_alheio(nosso));
    }
}
