//! Alcance do modo servidor: por onde o membro chega ao OpenVPN da rede
//! quando o caminho de sempre falha.
//!
//! Tres coisas por rede, escolhidas pelo ADMINISTRADOR (mexem em porta do
//! host e no que todo perfil carrega):
//!
//! - **Enderecos alternativos** (`remote` a mais no perfil, `remote-random`
//!   opcional): outro IP ou nome do mesmo servidor, ou um segundo servidor.
//!   Com mais de um, o perfil leva `server-poll-timeout` e `connect-retry`
//!   curtos -- o padrao do OpenVPN espera 120 s por endereco morto
//!   (client-options.rst:541-544).
//! - **Queda para TCP** numa rede UDP: o perfil sai com blocos
//!   `<connection>` (UDP primeiro, TCP depois), e o supervisor abre a ponte
//!   TCP -> UDP (`queda_tcp.rs`, e la o porque de nao serem dois processos).
//! - **`port-share`**: a porta TCP da rede divide espaco com um HTTPS que ja
//!   existia. Rede TCP: e a diretiva do proprio OpenVPN (so fora do
//!   Windows, server-options.rst:438). Queda TCP: e a ponte que decide.
//!
//! E o lado do MEMBRO, escolhido por quem baixa o perfil: proxy HTTP ou
//! SOCKS, com credencial perguntada ao conectar ou num arquivo dele -- nunca
//! a senha dentro do perfil (o `.ovpn` circula por e-mail e pendrive).
//!
//! # Os blocos `<connection>` vao no FIM do perfil
//!
//! Medido no 2.6.19: opcao de conexao escrita DEPOIS de um bloco nao vale
//! para ele (`Option 'tls-crypt' ... is ignored by previous <connection>
//! blocks`, options.c:5617-5626) -- e o `<tls-crypt>` embutido e opcao de
//! conexao. Com os blocos no meio, o perfil subia sem a chave que o servidor
//! exige. Por isso `perfil_membro` os poe por ultimo, e o teste
//! `blocos_de_conexao_sao_a_ultima_coisa_do_perfil` trava isso.

use crate::ovpn::{self, Rede};
use crate::painel::{Painel, Usuario};
use crate::pg::R;
use crate::queda_tcp;
use phxsql_core::json::Json;
use std::path::{Path, PathBuf};

pub const ESQUEMA: &str = "
CREATE TABLE IF NOT EXISTS phx_rede_alcance (
    rede_id    int PRIMARY KEY REFERENCES phx_rede ON DELETE RESTRICT,
    remotos    text NOT NULL DEFAULT '',
    aleatorio  boolean NOT NULL DEFAULT false,
    queda_tcp  int UNIQUE CHECK (queda_tcp BETWEEN 1 AND 65535),
    port_share text NOT NULL DEFAULT ''
);
ALTER TABLE phx_rede_alcance ADD COLUMN IF NOT EXISTS proxy_sem_texto_claro boolean NOT NULL DEFAULT false;
";

/// Enderecos alternativos por rede. Cada um custa ate `POLL` segundos a quem
/// esta com todos mortos; oito ja e um minuto e meio por volta.
pub const MAX_REMOTOS: usize = 8;
/// `server-poll-timeout`: quanto o membro espera um endereco responder antes
/// do proximo (o padrao de 120 s deixaria o failover em dois minutos).
pub const POLL: u32 = 10;
/// `connect-retry n max`: 1 s entre tentativas, dobrando ate 30 s (o padrao
/// e 300 s -- quem volta de uma queda longa esperaria cinco minutos).
pub const RETRY: (u32, u32) = (1, 30);

// ------------------------------------------------------------ endereco ----

/// Um endereco de servidor: `host`, `host:porta`, `[v6]:porta` ou `v6`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endereco {
    pub host: String,
    pub porta: Option<u16>,
}

impl Endereco {
    /// O perfil e um arquivo de diretivas, uma por linha: so letra, digito,
    /// ponto, hifen e dois-pontos no host -- espaco ou quebra de linha
    /// injetaria diretiva no perfil de todo membro.
    pub fn analisar(t: &str) -> R<Endereco> {
        let t = t.trim();
        let (host, porta) = if let Some(r) = t.strip_prefix('[') {
            let (h, resto) = r.split_once(']').ok_or("endereco: falta o ]")?;
            match resto {
                "" => (h, None),
                p => (
                    h,
                    Some(
                        p.strip_prefix(':')
                            .ok_or("endereco: depois do ] vem :porta")?,
                    ),
                ),
            }
        } else if t.matches(':').count() == 1 {
            let (h, p) = t.split_once(':').expect("um dois-pontos");
            (h, Some(p))
        } else {
            (t, None)
        };
        let porta = match porta {
            Some(p) => Some(
                p.parse::<u16>()
                    .ok()
                    .filter(|p| *p > 0)
                    .ok_or_else(|| format!("endereco {t}: porta invalida"))?,
            ),
            None => None,
        };
        let ok = (1..=253).contains(&host.len())
            && !host.starts_with('-')
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b));
        if !ok {
            return Err(format!(
                "endereco {t}: host so com letras, digitos, ponto, hifen e dois-pontos"
            ));
        }
        Ok(Endereco {
            host: host.to_string(),
            porta,
        })
    }

    pub fn texto(&self) -> String {
        match (self.porta, self.host.contains(':')) {
            (None, _) => self.host.clone(),
            (Some(p), true) => format!("[{}]:{p}", self.host),
            (Some(p), false) => format!("{}:{p}", self.host),
        }
    }
}

// ------------------------------------------------------------- alcance ----

/// O que o administrador escolheu para a rede. O padrao (vazio) e o perfil
/// de sempre, byte a byte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Alcance {
    pub remotos: Vec<Endereco>,
    pub aleatorio: bool,
    pub queda_tcp: Option<u16>,
    pub port_share: Option<(String, u16)>,
    /// O admin recusa senha de proxy em texto claro (Basic, SOCKS5 com
    /// usuario e senha): o perfil sai com `auto-nct`.
    pub proxy_sem_texto_claro: bool,
}

/// Alvo do `port-share`: IP literal, fora do loopback e do link-local. A
/// porta TCP da rede e publica; apontar o `port-share` para `127.0.0.1:8470`
/// punha o painel -- que so escuta em loopback porque fala HTTP sem TLS (A4)
/// -- na internet pela 443. Nome tambem nao: resolve no uso, e amanha pode
/// resolver para o loopback.
pub fn validar_port_share(t: &str) -> R<(String, u16)> {
    let (h, p) = ovpn::validar_http_proxy(t).map_err(|e| e.replace("http-proxy", "port-share"))?;
    let ip: std::net::IpAddr = h.parse().map_err(|_| {
        format!("port-share: use o IP do servidor HTTPS (nao nome): «{h}» pode resolver para o loopback")
    })?;
    let ip = match ip {
        std::net::IpAddr::V6(v) => v.to_ipv4_mapped().map(std::net::IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    };
    let proibido = match ip {
        std::net::IpAddr::V4(v) => {
            v.is_loopback()
                || v.is_unspecified()
                || v.is_link_local()
                || v.is_multicast()
                || v.is_broadcast()
        }
        std::net::IpAddr::V6(v) => {
            v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || (v.segments()[0] & 0xffc0) == 0xfe80
        }
    };
    if proibido {
        return Err(format!(
            "port-share: {ip} e loopback, link-local ou sem destino -- o que so escuta ali nao e para a internet; aponte para o HTTPS em outra maquina ou no IP de LAN"
        ));
    }
    Ok((ip.to_string(), p))
}

/// O IP e deste host? Um `bind` nele so da certo se for: sem lista de
/// placas para envelhecer, e o mesmo vale no Windows.
fn e_deste_host(ip: &str) -> bool {
    ip.parse::<std::net::IpAddr>()
        .map(|ip| std::net::UdpSocket::bind((ip, 0)).is_ok())
        .unwrap_or(false)
}

impl Alcance {
    /// O pedido da API: `remotos` (lista ou texto, um por linha),
    /// `aleatorio`, `queda_tcp` (0 ou ausente desliga) e `port_share`
    /// (`host:porta`, vazio desliga).
    pub fn do_pedido(corpo: &Json) -> R<Alcance> {
        let textos: Vec<String> = match corpo.campo("remotos") {
            Some(Json::Lista(l)) => l
                .iter()
                .filter_map(Json::texto)
                .map(str::to_string)
                .collect(),
            Some(Json::Texto(t)) => t.lines().map(str::to_string).collect(),
            _ => Vec::new(),
        };
        let remotos = textos
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(Endereco::analisar)
            .collect::<R<Vec<_>>>()?;
        let queda_tcp = match corpo.campo("queda_tcp").and_then(Json::inteiro) {
            None | Some(0) => None,
            Some(p) => Some(
                u16::try_from(p)
                    .ok()
                    .ok_or("queda_tcp: porta de 1 a 65535")?,
            ),
        };
        let port_share = match corpo.texto_ou("port_share", "").trim() {
            "" => None,
            t => Some(validar_port_share(t)?),
        };
        Ok(Alcance {
            remotos,
            aleatorio: corpo.booleano_ou("aleatorio", false),
            queda_tcp,
            port_share,
            proxy_sem_texto_claro: corpo.booleano_ou("proxy_sem_texto_claro", false),
        })
    }

    /// As regras que nao dependem de outras redes. `porta_rede`: a porta da
    /// rede; `portas_do_painel`: onde o painel escuta -- o `port-share` nao
    /// aponta para nenhuma delas (laco, ou o painel na internet).
    pub fn validar(&self, tcp: bool, porta_rede: u16, portas_do_painel: &[u16]) -> R<()> {
        if self.remotos.len() > MAX_REMOTOS {
            return Err(format!("no maximo {MAX_REMOTOS} enderecos alternativos"));
        }
        if self.aleatorio && self.remotos.is_empty() {
            return Err("sortear a ordem pede ao menos um endereco alternativo".into());
        }
        if self.queda_tcp.is_some() && tcp {
            return Err("a rede ja e TCP: queda para TCP e so para rede UDP".into());
        }
        // `remote-random` embaralha a lista de blocos inteira
        // (client-options.rst:511-514): o TCP poderia vir antes do UDP, e
        // o membro que alcanca o UDP ficaria no TCP sem precisar.
        if self.aleatorio && self.queda_tcp.is_some() {
            return Err(
                "sortear a ordem com queda para TCP poria o TCP antes do UDP: escolha um dos dois"
                    .into(),
            );
        }
        if self.port_share.is_some() && !tcp && self.queda_tcp.is_none() {
            return Err("port-share divide uma porta TCP: a rede e UDP sem queda para TCP".into());
        }
        // So vale para alvo NESTE host: o HTTPS da empresa em outra maquina
        // na 443 e o caso comum, e nao volta para ca.
        if let Some((h, p)) = self.port_share.as_ref().filter(|(h, _)| e_deste_host(h)) {
            if portas_do_painel.contains(p) {
                return Err(format!(
                    "port-share: a porta {p} e a do painel, que fala HTTP sem TLS e nao vai para a internet"
                ));
            }
            if *p == porta_rede || Some(*p) == self.queda_tcp {
                return Err(format!(
                    "port-share: {h}:{p} e a propria porta da rede -- a conexao voltaria para ela"
                ));
            }
        }
        if self.port_share.is_some() && tcp && cfg!(windows) {
            return Err(
                "o OpenVPN nao tem port-share no Windows (server-options.rst:438); use a queda TCP numa rede UDP"
                    .into(),
            );
        }
        Ok(())
    }

    fn remotos_texto(&self) -> String {
        self.remotos
            .iter()
            .map(Endereco::texto)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn json(&self) -> Json {
        Json::objeto(vec![
            (
                "remotos",
                Json::Lista(
                    self.remotos
                        .iter()
                        .map(|e| Json::texto_de(e.texto()))
                        .collect(),
                ),
            ),
            ("aleatorio", Json::de_bool(self.aleatorio)),
            (
                "queda_tcp",
                self.queda_tcp
                    .map(|p| Json::de_i64(p as i64))
                    .unwrap_or(Json::Nulo),
            ),
            (
                "port_share",
                self.port_share
                    .as_ref()
                    .map(|(h, p)| Json::texto_de(format!("{h}:{p}")))
                    .unwrap_or(Json::Nulo),
            ),
            (
                "proxy_sem_texto_claro",
                Json::de_bool(self.proxy_sem_texto_claro),
            ),
        ])
    }
}

// ------------------------------------------------------ proxy do membro ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoProxy {
    Http,
    Socks,
}

/// Onde o OpenVPN do membro acha usuario e senha do proxy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CredProxy {
    Nenhuma,
    /// Perguntada ao conectar (terminal ou OpenVPN GUI, pela gerencia).
    Perguntar,
    /// Arquivo do membro (usuario e senha em duas linhas, 0600), pelo
    /// caminho na maquina DELE: a senha nunca passa pelo painel.
    Arquivo(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyMembro {
    pub tipo: TipoProxy,
    pub host: String,
    pub porta: u16,
    pub cred: CredProxy,
}

impl ProxyMembro {
    /// O proxy HTTP sem credencial de antes (`http_proxy` no pedido).
    pub fn http(t: &str) -> R<ProxyMembro> {
        let (host, porta) = ovpn::validar_http_proxy(t)?;
        Ok(ProxyMembro {
            tipo: TipoProxy::Http,
            host,
            porta,
            cred: CredProxy::Nenhuma,
        })
    }

    /// Do pedido de entrar: `proxy` (`host:porta`), `proxy_tipo`
    /// (`http`|`socks`), `proxy_credencial` (`nenhuma`|`perguntar`|`arquivo`)
    /// e `proxy_arquivo`. `http_proxy` sozinho continua valendo (HTTP sem
    /// credencial), para cliente antigo.
    pub fn do_pedido(corpo: &Json) -> R<Option<ProxyMembro>> {
        let novo = corpo.texto_ou("proxy", "").trim();
        let velho = corpo.texto_ou("http_proxy", "").trim();
        let (endereco, tipo) = match (novo, velho) {
            ("", "") => return Ok(None),
            ("", v) => (v, "http"),
            (n, _) => (n, corpo.texto_ou("proxy_tipo", "http")),
        };
        let mut p = ProxyMembro::http(endereco)?;
        p.tipo = match tipo {
            "http" | "" => TipoProxy::Http,
            "socks" => TipoProxy::Socks,
            _ => return Err("proxy_tipo: http ou socks".into()),
        };
        p.cred = match corpo.texto_ou("proxy_credencial", "nenhuma") {
            "nenhuma" | "" => CredProxy::Nenhuma,
            "perguntar" => CredProxy::Perguntar,
            "arquivo" => {
                CredProxy::Arquivo(caminho_no_perfil(corpo.texto_ou("proxy_arquivo", ""))?)
            }
            _ => return Err("proxy_credencial: nenhuma, perguntar ou arquivo".into()),
        };
        Ok(Some(p))
    }

    /// As linhas do perfil. HTTP com credencial vai com `auto` (ou
    /// `auto-nct`): o OpenVPN tenta sem, e so manda a senha se o proxy
    /// responder 407, descobrindo o metodo (Basic, Digest, NTLM --
    /// proxy-options.rst:17-20).
    ///
    /// `em_bloco`: a linha vai dentro de um `<connection>` (queda para TCP).
    /// Ali o arquivo so entra com metodo fixo: o `http-proxy-user-pass` nao
    /// e aceito dentro do bloco, e fora dele cria o proxy no bloco UDP
    /// tambem (`--http-proxy MUST be used in TCP Client mode`) -- medido no
    /// 2.6.19. `conferir_proxy` recusa antes o caso sem saida (sem texto
    /// claro + arquivo + queda).
    pub fn linhas(&self, sem_texto_claro: bool, em_bloco: bool) -> String {
        let (h, p) = (&self.host, self.porta);
        let auto = if sem_texto_claro { "auto-nct" } else { "auto" };
        match (self.tipo, &self.cred) {
            (TipoProxy::Http, CredProxy::Nenhuma) => format!("http-proxy {h} {p}\n"),
            (TipoProxy::Http, CredProxy::Perguntar) => format!("http-proxy {h} {p} {auto}\n"),
            (TipoProxy::Http, CredProxy::Arquivo(c)) if em_bloco => {
                format!("http-proxy {h} {p} \"{c}\" basic\n")
            }
            (TipoProxy::Http, CredProxy::Arquivo(c)) => {
                format!("http-proxy {h} {p} {auto}\nhttp-proxy-user-pass \"{c}\"\n")
            }
            (TipoProxy::Socks, CredProxy::Nenhuma) => format!("socks-proxy {h} {p}\n"),
            (TipoProxy::Socks, CredProxy::Perguntar) => format!("socks-proxy {h} {p} stdin\n"),
            (TipoProxy::Socks, CredProxy::Arquivo(c)) => format!("socks-proxy {h} {p} \"{c}\"\n"),
        }
    }
}

/// Caminho do arquivo de credencial, do jeito que pode ir entre aspas numa
/// linha do perfil: absoluto, sem aspas nem controle; `\` vira `/` (o
/// OpenVPN le `\` como escape, e o Windows aceita `/`).
pub fn caminho_no_perfil(c: &str) -> R<String> {
    let c = c.trim().replace('\\', "/");
    let absoluto = c.starts_with('/')
        || (c.len() > 2 && c.as_bytes()[0].is_ascii_alphabetic() && &c[1..3] == ":/");
    if !absoluto {
        return Err("proxy_arquivo: caminho absoluto do arquivo na maquina do membro".into());
    }
    if c.len() > 1024 || c.chars().any(|x| x.is_control() || x == '"' || x == '\'') {
        return Err("proxy_arquivo: caminho sem aspas nem caractere de controle".into());
    }
    Ok(c)
}

// ------------------------------------------------------------- perfil ----

/// O que o perfil de um membro precisa alem da rede: o alcance dela e o
/// proxy dele.
#[derive(Clone, Debug, Default)]
pub struct Conexao {
    pub alcance: Alcance,
    pub proxy: Option<ProxyMembro>,
}

/// O proxy so leva TCP (o SOCKS do OpenVPN leva UDP, mas isso nao foi
/// provado aqui -- entra quando for). Rede UDP com queda: o proxy vai no
/// bloco TCP.
pub fn conferir_proxy(tcp: bool, c: &Conexao) -> R<()> {
    let Some(p) = &c.proxy else { return Ok(()) };
    if !tcp && c.alcance.queda_tcp.is_none() {
        return Err("proxy so leva TCP, e esta rede e UDP sem queda para TCP".into());
    }
    if c.alcance.proxy_sem_texto_claro && p.cred != CredProxy::Nenhuma {
        if p.tipo == TipoProxy::Socks {
            return Err(
                "esta rede recusa senha de proxy em texto claro, e o SOCKS5 manda usuario e senha em claro (RFC 1929)"
                    .into(),
            );
        }
        if !tcp && matches!(p.cred, CredProxy::Arquivo(_)) {
            return Err(
                "esta rede recusa senha em texto claro, e com a queda para TCP o arquivo so vai com Basic: use «pedir ao conectar»"
                    .into(),
            );
        }
    }
    Ok(())
}

/// As tres partes do perfil que dependem do alcance: o `topo` (onde ficavam
/// `proto` e `remote`), o aviso de saida e o `fim` (os blocos, por ultimo --
/// ver o topo do arquivo).
pub struct Partes {
    pub topo: String,
    pub saida: &'static str,
    pub fim: String,
}

pub fn partes(rede: &Rede, endereco: &str, c: Option<&Conexao>) -> Partes {
    let padrao = Conexao::default();
    let c = c.unwrap_or(&padrao);
    let a = &c.alcance;
    let mut lista = vec![Endereco {
        host: endereco.to_string(),
        porta: None,
    }];
    lista.extend(a.remotos.iter().cloned());
    let porta = |e: &Endereco| e.porta.unwrap_or(rede.porta);
    let queda = a.queda_tcp.filter(|_| !rede.tcp);
    let mut topo = String::new();
    if lista.len() > 1 || queda.is_some() {
        topo.push_str(&format!(
            "server-poll-timeout {POLL}\nconnect-retry {} {}\n",
            RETRY.0, RETRY.1
        ));
    }
    if a.aleatorio && lista.len() > 1 {
        topo.push_str("remote-random\n");
    }
    let proxy = c
        .proxy
        .as_ref()
        .map(|p| p.linhas(a.proxy_sem_texto_claro, queda.is_some()))
        .unwrap_or_default();
    let Some(t) = queda else {
        let mut p = format!("proto {}\n", rede.proto("client"));
        for e in &lista {
            p.push_str(&format!("remote {} {}\n", e.host, porta(e)));
        }
        p.push_str(&topo);
        // Em UDP o proxy nao entra: o OpenVPN recusaria o perfil.
        if rede.tcp {
            p.push_str(&proxy);
        }
        return Partes {
            topo: p,
            saida: rede.aviso_de_saida(),
            fim: String::new(),
        };
    };
    let mut fim = format!("# UDP primeiro; sem resposta em {POLL} s, TCP na porta {t}\n");
    for e in &lista {
        fim.push_str(&format!(
            "<connection>\nremote {} {} udp\nexplicit-exit-notify 1\n</connection>\n",
            e.host,
            porta(e)
        ));
    }
    for e in &lista {
        fim.push_str(&format!(
            "<connection>\nremote {} {t} tcp-client\n{proxy}</connection>\n",
            e.host
        ));
    }
    Partes {
        topo,
        saida: "",
        fim,
    }
}

// ------------------------------------------------------------ servidor ----

/// O que o alcance acrescenta ao `servidor.conf`, e o arquivo da ponte (ou
/// a falta dele, que a desliga).
pub fn aplicar_na_pasta(dir: &Path, rede: &Rede, a: &Alcance) -> R<String> {
    let arq = dir.join(queda_tcp::ARQUIVO);
    match a.queda_tcp.filter(|_| !rede.tcp) {
        Some(t) => {
            let conf = queda_tcp::Conf {
                porta: t,
                udp: rede.porta,
                port_share: a.port_share.clone(),
            };
            crate::painel::gravar(&arq, conf.texto().as_bytes(), false)?;
        }
        None => match std::fs::remove_file(&arq) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("apagar {}: {e}", arq.display()))
            }
            _ => {}
        },
    }
    Ok(match (&a.port_share, rede.tcp) {
        (Some((h, p)), true) => format!("port-share {h} {p}\n"),
        _ => String::new(),
    })
}

// -------------------------------------------------------------- painel ----

fn de_linha(r: &crate::pg::Resposta, i: usize) -> R<Alcance> {
    let v = |c: &str| r.valor(i, c).unwrap_or_default().to_string();
    let remotos = v("remotos")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(Endereco::analisar)
        .collect::<R<Vec<_>>>()?;
    // Gravado antes da guarda de hoje (loopback): cai o port-share, fica a
    // rede -- falhar fechado no alvo, nao derrubar o arranque do painel.
    let port_share = match v("port_share").as_str() {
        "" => None,
        t => match validar_port_share(t) {
            Ok(x) => Some(x),
            Err(e) => {
                eprintln!("phxvpn: AVISO port-share gravado ignorado: {e}");
                None
            }
        },
    };
    Ok(Alcance {
        remotos,
        aleatorio: v("aleatorio") == "t",
        queda_tcp: r.valor(i, "queda_tcp").and_then(|p| p.parse().ok()),
        port_share,
        proxy_sem_texto_claro: v("proxy_sem_texto_claro") == "t",
    })
}

impl Painel {
    /// O alcance gravado (o padrao, se a rede nunca teve um).
    pub(crate) fn alcance_da_rede(&mut self, rede_id: &str) -> R<Alcance> {
        let r = self.pg()?.executar(
            "SELECT remotos, aleatorio, queda_tcp, port_share, proxy_sem_texto_claro \
             FROM phx_rede_alcance WHERE rede_id = $1::int",
            &[Some(rede_id)],
        )?;
        if r.linhas.is_empty() {
            return Ok(Alcance::default());
        }
        de_linha(&r, 0)
    }

    /// Para o cartao da rede: o administrador e o dono veem.
    pub fn alcance(&mut self, u: &Usuario, rede_id: i64) -> R<Json> {
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT dono_id, protocolo, porta FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let dono: i64 = r
            .valor(0, "dono_id")
            .and_then(|v| v.parse().ok())
            .ok_or("rede inexistente")?;
        if !u.admin && u.id != dono {
            return Err("so o administrador ou o dono da rede ve o alcance dela".into());
        }
        let tcp = r.valor(0, "protocolo") == Some("tcp");
        let porta = r.valor(0, "porta").unwrap_or_default().to_string();
        let mut j = self.alcance_da_rede(&id)?.json();
        if let Json::Objeto(c) = &mut j {
            c.push((
                "protocolo".into(),
                Json::texto_de(if tcp { "tcp" } else { "udp" }),
            ));
            c.push(("porta".into(), Json::de_i64(porta.parse().unwrap_or(0))));
        }
        Ok(j)
    }

    /// Porta TCP nova no host: nao pode ser a de uma rede TCP nem a queda de
    /// outra rede (o `UNIQUE` pega esta; a mensagem diz qual das duas).
    pub(crate) fn porta_tcp_livre(&mut self, porta: u16, fora: Option<&str>) -> R<()> {
        let p = porta.to_string();
        let r = self.pg()?.executar(
            "SELECT nome FROM phx_rede WHERE protocolo = 'tcp' AND porta = $1::int \
             UNION ALL SELECT r.nome FROM phx_rede_alcance a JOIN phx_rede r ON r.id = a.rede_id \
             WHERE a.queda_tcp = $1::int AND a.rede_id <> COALESCE($2::int, 0)",
            &[Some(&p), fora],
        )?;
        match r.valor(0, "nome") {
            Some(n) => Err(format!("a porta TCP {porta} ja e da rede «{n}»")),
            None => Ok(()),
        }
    }

    /// Grava o alcance, SO o administrador: abre porta no host (queda,
    /// `port-share`) e muda o perfil de todos. Devolve (nome, pasta, o
    /// alcance anterior) para quem reinicia o OpenVPN poder desfazer.
    pub fn alcance_gravar(
        &mut self,
        u: &Usuario,
        rede_id: i64,
        novo: &Alcance,
        portas_do_painel: &[u16],
    ) -> R<(String, PathBuf, Alcance)> {
        if !u.admin {
            return Err("so o administrador muda o alcance da rede".into());
        }
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT nome, protocolo, porta FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let nome = r.valor(0, "nome").ok_or("rede inexistente")?.to_string();
        let porta_rede: u16 = r
            .valor(0, "porta")
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        novo.validar(
            r.valor(0, "protocolo") == Some("tcp"),
            porta_rede,
            portas_do_painel,
        )?;
        if let Some(t) = novo.queda_tcp {
            self.porta_tcp_livre(t, Some(&id))?;
        }
        let anterior = self.alcance_da_rede(&id)?;
        self.alcance_escrever(&id, novo)?;
        match self.materializar_rede(&id) {
            Ok(dir) => Ok((nome, dir, anterior)),
            Err(e) => {
                let _ = self.alcance_escrever(&id, &anterior);
                let _ = self.materializar_rede(&id);
                Err(e)
            }
        }
    }

    /// Volta ao alcance anterior (o OpenVPN ou a ponte nao subiram com o
    /// novo): banco e arquivos.
    pub fn alcance_restaurar(&mut self, rede_id: i64, anterior: &Alcance) -> R<()> {
        let id = rede_id.to_string();
        self.alcance_escrever(&id, anterior)?;
        self.materializar_rede(&id).map(|_| ())
    }

    fn alcance_escrever(&mut self, id: &str, a: &Alcance) -> R<()> {
        let queda = a.queda_tcp.map(|p| p.to_string());
        let ps = a
            .port_share
            .as_ref()
            .map(|(h, p)| format!("{h}:{p}"))
            .unwrap_or_default();
        self.pg()?
            .executar(
                "INSERT INTO phx_rede_alcance (rede_id, remotos, aleatorio, queda_tcp, port_share, proxy_sem_texto_claro) \
                 VALUES ($1::int, $2, $3::boolean, $4::int, $5, $6::boolean) \
                 ON CONFLICT (rede_id) DO UPDATE SET remotos = $2, aleatorio = $3::boolean, \
                 queda_tcp = $4::int, port_share = $5, proxy_sem_texto_claro = $6::boolean",
                &[
                    Some(id),
                    Some(&a.remotos_texto()),
                    Some(if a.aleatorio { "true" } else { "false" }),
                    queda.as_deref(),
                    Some(&ps),
                    Some(if a.proxy_sem_texto_claro { "true" } else { "false" }),
                ],
            )
            .map(|_| ())
            .map_err(|e| {
                if e.contains("queda_tcp") {
                    "essa porta TCP ja e a queda de outra rede".into()
                } else {
                    e
                }
            })
    }

    /// O trecho do `servidor.conf` e o arquivo da ponte (chamado pelo
    /// `materializar_rede`).
    pub(crate) fn conf_do_alcance(&mut self, rede_id: &str, dir: &Path, rede: &Rede) -> R<String> {
        let a = self.alcance_da_rede(rede_id)?;
        aplicar_na_pasta(dir, rede, &a)
    }
}

/// Depois de gravar: o OpenVPN da rede (e a ponte) sobem com o novo. Se nao
/// subirem -- a porta 443 ja e de alguem --, o alcance volta ao anterior e o
/// erro diz o porque: o painel nunca diz «queda na 443» com nada escutando.
pub fn gravar_e_aplicar(
    e: &crate::http::Estado,
    u: &Usuario,
    rede_id: i64,
    novo: &Alcance,
) -> R<Json> {
    let portas: Vec<u16> = e.porta_do_painel().into_iter().collect();
    let (nome, dir, anterior) = e.painel().alcance_gravar(u, rede_id, novo, &portas)?;
    let reiniciado = match &e.supervisor {
        Some(s) => {
            if let Err(m) = s.reiniciar(&nome, &dir) {
                let _ = e.painel().alcance_restaurar(rede_id, &anterior);
                let _ = s.reiniciar(&nome, &dir);
                return Err(format!("o alcance nao entrou: {m}"));
            }
            true
        }
        None => false,
    };
    Ok(Json::objeto(vec![
        ("ok", Json::de_bool(true)),
        ("openvpn_reiniciado", Json::de_bool(reiniciado)),
        (
            "aviso",
            Json::texto_de(
                "os membros precisam baixar o perfil de novo (entrar na rede outra vez)",
            ),
        ),
    ]))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn rede(tcp: bool) -> Rede<'static> {
        Rede {
            nome: "Matriz",
            porta: 1195,
            octeto: 1,
            v2: false,
            tcp,
            cookie: false,
        }
    }

    fn perfil(r: &Rede, c: Option<&Conexao>) -> String {
        ovpn::perfil_membro(&ovpn::Perfil {
            rede: r,
            servidor: &ovpn::Servidor {
                nome: "vpn1",
                endereco: "vpn.empresa.com.br",
            },
            ca_pem: "CA\n",
            cert_pem: "C\n",
            chave_pem: "K\n",
            tls_crypt: "T\n",
            conexao: c,
        })
    }

    fn alc(remotos: &[&str], queda: Option<u16>) -> Conexao {
        Conexao {
            alcance: Alcance {
                remotos: remotos
                    .iter()
                    .map(|r| Endereco::analisar(r).unwrap())
                    .collect(),
                queda_tcp: queda,
                ..Alcance::default()
            },
            proxy: None,
        }
    }

    #[test]
    fn endereco_aceita_as_formas_e_barra_injecao() {
        let e = |t| Endereco::analisar(t).unwrap();
        assert_eq!(e("vpn2.empresa.com.br").porta, None);
        assert_eq!(e("200.1.2.3:1196").porta, Some(1196));
        assert_eq!(e("[2001:db8::1]:443").host, "2001:db8::1");
        assert_eq!(e("2001:db8::1").porta, None);
        assert_eq!(e("[2001:db8::1]:443").texto(), "[2001:db8::1]:443");
        for ruim in [
            "vpn2\nup /bin/sh",
            "vpn2 1194",
            "vpn2:0",
            "vpn2:x",
            "",
            "-v",
            "[::1",
            "a;b",
        ] {
            assert!(Endereco::analisar(ruim).is_err(), "{ruim:?} passou");
        }
    }

    /// Sem alcance, o perfil e o de antes, byte a byte: rede criada antes
    /// desta frente nao muda ao baixar de novo.
    #[test]
    fn sem_alcance_o_perfil_nao_muda() {
        for tcp in [false, true] {
            let r = rede(tcp);
            let p = perfil(&r, None);
            assert_eq!(p, perfil(&r, Some(&Conexao::default())));
            let proto = if tcp { "tcp-client" } else { "udp" };
            assert!(
                p.contains(&format!("dev tun\nproto {proto}\nremote vpn.empresa.com.br 1195\nresolv-retry infinite\n")),
                "{p}"
            );
            assert!(!p.contains("<connection>") && !p.contains("server-poll-timeout"));
        }
    }

    #[test]
    fn remotos_alternativos_com_espera_curta_e_sorteio() {
        let r = rede(false);
        let mut c = alc(&["vpn2.empresa.com.br", "200.1.2.3:1300"], None);
        let p = perfil(&r, Some(&c));
        assert!(
            p.contains(
                "proto udp\nremote vpn.empresa.com.br 1195\nremote vpn2.empresa.com.br 1195\n\
                 remote 200.1.2.3 1300\nserver-poll-timeout 10\nconnect-retry 1 30\n"
            ),
            "{p}"
        );
        assert!(!p.contains("remote-random"));
        c.alcance.aleatorio = true;
        assert!(perfil(&r, Some(&c)).contains("remote-random\n"));
    }

    #[test]
    fn queda_tcp_poe_udp_primeiro_e_tcp_depois() {
        let r = rede(false);
        let p = perfil(&r, Some(&alc(&["vpn2"], Some(443))));
        let blocos: Vec<&str> = p.split("<connection>\n").skip(1).collect();
        assert_eq!(blocos.len(), 4, "{p}");
        assert!(
            blocos[0].starts_with("remote vpn.empresa.com.br 1195 udp\nexplicit-exit-notify 1\n")
        );
        assert!(blocos[1].starts_with("remote vpn2 1195 udp\n"));
        assert!(blocos[2].starts_with("remote vpn.empresa.com.br 443 tcp-client\n</connection>"));
        assert!(blocos[3].starts_with("remote vpn2 443 tcp-client\n"));
        // Nada de proto/remote/aviso fora dos blocos: valeria para o TCP.
        let antes = p.split("<connection>").next().unwrap();
        assert!(!antes.contains("\nproto ") && !antes.contains("\nremote "));
        assert!(!antes.contains("explicit-exit-notify"), "{antes}");
        assert!(antes.contains("server-poll-timeout 10\nconnect-retry 1 30\n"));
    }

    /// Medido no 2.6.19: opcao de conexao DEPOIS de um bloco nao vale para
    /// ele -- e o `<tls-crypt>` e uma. Os blocos sao a ultima coisa.
    #[test]
    fn blocos_de_conexao_sao_a_ultima_coisa_do_perfil() {
        let p = perfil(&rede(false), Some(&alc(&[], Some(443))));
        let depois = p.rsplit("</connection>\n").next().unwrap();
        assert_eq!(depois, "", "sobrou depois do ultimo bloco: {depois}");
        assert!(p.find("<tls-crypt>").unwrap() < p.find("<connection>").unwrap());
    }

    /// As opcoes que o 2.6.19 trata como «de conexao» (options.c, as que
    /// pedem `OPT_P_CONNECTION`, mais as que o laboratorio viu ignoradas).
    const OPCOES_DE_CONEXAO: &[&str] = &[
        "bind",
        "connect-retry",
        "connect-retry-max",
        "connect-timeout",
        "float",
        "fragment",
        "http-proxy",
        "http-proxy-option",
        "key-direction",
        "link-mtu",
        "local",
        "lport",
        "mssfix",
        "mtu-disc",
        "nobind",
        "port",
        "proto",
        "remote",
        "rport",
        "server-poll-timeout",
        "socks-proxy",
        "tun-mtu",
        "tun-mtu-extra",
        "explicit-exit-notify",
        "tls-auth",
        "tls-crypt",
        "tls-crypt-v2",
        "<tls-auth>",
        "<tls-crypt>",
        "<tls-crypt-v2>",
    ];

    /// O perfil INTEIRO que sai do painel -- o de `perfil_membro` mais o que
    /// outros modulos penduram depois (DNS no Linux, sem IPv6, autenticador):
    /// nada disso pode ser opcao de conexao, senao os blocos nao a veriam.
    #[test]
    fn nada_pendurado_depois_dos_blocos_e_opcao_de_conexao() {
        let c = alc(&["vpn2"], Some(443));
        let base = perfil(&rede(false), Some(&c));
        let saida = crate::saida::Saida {
            tunel_total: true,
            bloquear_local: false,
            dns_nomes: true,
            dns_empresa: vec![],
        };
        let cliente = crate::saida::cliente("systemd-resolved", true).unwrap();
        let inteiro =
            base + &crate::saida::perfil_cliente(&cliente, &saida) + crate::verificar::PERFIL_MFA;
        let depois = inteiro.rsplit("</connection>\n").next().unwrap();
        assert!(!depois.is_empty(), "o teste precisa de algo pendurado");
        for l in depois.lines() {
            let op = l.split_whitespace().next().unwrap_or("");
            assert!(
                !OPCOES_DE_CONEXAO.contains(&op),
                "«{l}» depois do ultimo </connection> nao valeria para os blocos"
            );
        }
    }

    #[test]
    fn proxy_vai_so_no_bloco_tcp_e_nunca_com_senha() {
        let mut c = alc(&[], Some(443));
        c.proxy = Some(ProxyMembro {
            tipo: TipoProxy::Http,
            host: "proxy.hotel".into(),
            porta: 3128,
            cred: CredProxy::Perguntar,
        });
        let p = perfil(&rede(false), Some(&c));
        assert!(
            p.contains("remote vpn.empresa.com.br 443 tcp-client\nhttp-proxy proxy.hotel 3128 auto\n</connection>"),
            "{p}"
        );
        assert_eq!(p.matches("http-proxy").count(), 1);
        assert!(!p.contains("http-proxy-user-pass"));
        // Rede UDP sem queda: o proxy e recusado antes de virar perfil.
        let mut sem = c.clone();
        sem.alcance.queda_tcp = None;
        assert!(conferir_proxy(false, &sem).is_err());
        assert!(conferir_proxy(false, &c).is_ok());
        assert!(conferir_proxy(true, &sem).is_ok());
    }

    #[test]
    fn linhas_de_proxy_por_tipo_e_credencial() {
        let p = |tipo, cred, nct, bloco| {
            ProxyMembro {
                tipo,
                host: "px".into(),
                porta: 1080,
                cred,
            }
            .linhas(nct, bloco)
        };
        let arq = CredProxy::Arquivo("/home/ana/px.cred".into());
        use TipoProxy::{Http, Socks};
        assert_eq!(
            p(Http, CredProxy::Nenhuma, false, false),
            "http-proxy px 1080\n"
        );
        assert_eq!(
            p(Http, CredProxy::Perguntar, false, true),
            "http-proxy px 1080 auto\n"
        );
        assert_eq!(
            p(Http, CredProxy::Perguntar, true, true),
            "http-proxy px 1080 auto-nct\n"
        );
        // Fora de bloco: o metodo sai do 407 (auto), a senha do arquivo.
        assert_eq!(
            p(Http, arq.clone(), false, false),
            "http-proxy px 1080 auto\nhttp-proxy-user-pass \"/home/ana/px.cred\"\n"
        );
        assert_eq!(
            p(Http, arq.clone(), true, false),
            "http-proxy px 1080 auto-nct\nhttp-proxy-user-pass \"/home/ana/px.cred\"\n"
        );
        // Dentro do bloco da queda o 2.6 so aceita metodo fixo.
        assert_eq!(
            p(Http, arq.clone(), false, true),
            "http-proxy px 1080 \"/home/ana/px.cred\" basic\n"
        );
        assert_eq!(
            p(Socks, CredProxy::Nenhuma, false, false),
            "socks-proxy px 1080\n"
        );
        assert_eq!(
            p(Socks, CredProxy::Perguntar, false, false),
            "socks-proxy px 1080 stdin\n"
        );
        assert_eq!(
            p(Socks, arq, false, false),
            "socks-proxy px 1080 \"/home/ana/px.cred\"\n"
        );
    }

    /// Sem texto claro pedido pelo admin: SOCKS com senha e arquivo na
    /// queda (que so vai com Basic) sao recusados; o resto passa.
    #[test]
    fn sem_texto_claro_recusa_o_que_mandaria_a_senha_em_claro() {
        let px = |tipo, cred| {
            Some(ProxyMembro {
                tipo,
                host: "px".into(),
                porta: 3128,
                cred,
            })
        };
        let arq = CredProxy::Arquivo("/c".into());
        let mut c = alc(&[], Some(443));
        c.alcance.proxy_sem_texto_claro = true;
        c.proxy = px(TipoProxy::Http, arq.clone());
        assert!(
            conferir_proxy(false, &c).is_err(),
            "arquivo na queda iria como Basic"
        );
        assert!(
            conferir_proxy(true, &c).is_ok(),
            "rede TCP: auto-nct com arquivo"
        );
        c.proxy = px(TipoProxy::Http, CredProxy::Perguntar);
        assert!(conferir_proxy(false, &c).is_ok());
        c.proxy = px(TipoProxy::Socks, CredProxy::Perguntar);
        assert!(
            conferir_proxy(true, &c).is_err(),
            "SOCKS5 manda a senha em claro"
        );
        c.proxy = px(TipoProxy::Socks, CredProxy::Nenhuma);
        assert!(conferir_proxy(true, &c).is_ok());
        c.alcance.proxy_sem_texto_claro = false;
        c.proxy = px(TipoProxy::Http, arq);
        assert!(conferir_proxy(false, &c).is_ok());
        assert!(perfil(&rede(false), Some(&c)).contains(" basic\n"));
        c.alcance.proxy_sem_texto_claro = true;
        c.proxy = px(TipoProxy::Http, CredProxy::Perguntar);
        assert!(perfil(&rede(false), Some(&c)).contains("http-proxy px 3128 auto-nct\n"));
    }

    /// O port-share nao aponta para o que so escuta no loopback (o painel,
    /// HTTP sem TLS) nem para a porta do painel ou a da propria rede.
    #[test]
    fn port_share_nao_expoe_o_loopback_nem_o_painel() {
        let j = |t: &str| Json::analisar(t).unwrap();
        for ruim in [
            "127.0.0.1:8470",
            "[::1]:8470",
            "127.9.9.9:443",
            "0.0.0.0:443",
            "169.254.1.1:443",
            "[fe80::1]:443",
            "[::ffff:127.0.0.1]:8470",
            "localhost:8443",
            "web.empresa:443",
        ] {
            let pedido = j(&format!(r#"{{"queda_tcp":443,"port_share":"{ruim}"}}"#));
            assert!(Alcance::do_pedido(&pedido).is_err(), "{ruim} passou");
        }
        let ok =
            Alcance::do_pedido(&j(r#"{"queda_tcp":443,"port_share":"192.168.10.5:443"}"#)).unwrap();
        assert!(ok.validar(false, 1195, &[8470]).is_ok());
        // Outra maquina na porta do painel nao e o painel.
        let fora = Alcance {
            port_share: Some(("192.168.10.5".into(), 8470)),
            ..ok.clone()
        };
        assert!(fora.validar(false, 1195, &[8470]).is_ok());
        // Um IP DESTE host (o de saida), para a porta do painel e o laco.
        let Some(local) = std::net::UdpSocket::bind("0.0.0.0:0")
            .and_then(|u| u.connect("192.0.2.99:9").and_then(|_| u.local_addr()))
            .ok()
            .map(|a| a.ip().to_string())
            .filter(|ip| ip != "0.0.0.0")
        else {
            eprintln!("sem IP de saida: parte local pulada");
            return;
        };
        let painel = Alcance {
            port_share: Some((local.clone(), 8470)),
            ..ok.clone()
        };
        assert!(
            painel.validar(false, 1195, &[8470]).is_err(),
            "porta do painel"
        );
        let laco = Alcance {
            port_share: Some((local.clone(), 443)),
            ..ok.clone()
        };
        assert!(
            laco.validar(false, 1195, &[8470]).is_err(),
            "a propria queda"
        );
        let laco_tcp = Alcance {
            queda_tcp: None,
            port_share: Some((local.clone(), 8443)),
            ..ok
        };
        assert!(
            laco_tcp.validar(true, 8443, &[8470]).is_err(),
            "a propria rede TCP"
        );
    }

    #[test]
    fn caminho_da_credencial_nao_injeta_nem_e_relativo() {
        assert_eq!(
            caminho_no_perfil("/home/ana/p.cred").unwrap(),
            "/home/ana/p.cred"
        );
        assert_eq!(
            caminho_no_perfil("C:\\Users\\Ana Paula\\p.cred").unwrap(),
            "C:/Users/Ana Paula/p.cred"
        );
        for ruim in [
            "p.cred",
            "",
            "/a\"\nup /bin/sh",
            "/a'b",
            "C:p.cred",
            "/a\tb",
        ] {
            assert!(caminho_no_perfil(ruim).is_err(), "{ruim:?} passou");
        }
    }

    #[test]
    fn pedido_de_proxy_novo_e_o_antigo() {
        let j = |t: &str| Json::analisar(t).unwrap();
        assert_eq!(ProxyMembro::do_pedido(&j("{}")).unwrap(), None);
        let velho = ProxyMembro::do_pedido(&j(r#"{"http_proxy":"px:3128"}"#))
            .unwrap()
            .unwrap();
        assert_eq!(
            (velho.tipo, velho.cred),
            (TipoProxy::Http, CredProxy::Nenhuma)
        );
        let s = ProxyMembro::do_pedido(&j(
            r#"{"proxy":"px:1080","proxy_tipo":"socks","proxy_credencial":"arquivo","proxy_arquivo":"/c/p"}"#,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(s.tipo, TipoProxy::Socks);
        assert_eq!(s.cred, CredProxy::Arquivo("/c/p".into()));
        assert!(ProxyMembro::do_pedido(&j(r#"{"proxy":"px:1080","proxy_tipo":"ftp"}"#)).is_err());
        assert!(ProxyMembro::do_pedido(&j(
            r#"{"proxy":"px:1080","proxy_credencial":"arquivo","proxy_arquivo":"rel"}"#
        ))
        .is_err());
    }

    #[test]
    fn validar_recusa_as_combinacoes_sem_sentido() {
        let e = |t| Endereco::analisar(t).unwrap();
        let ok = Alcance {
            remotos: vec![e("vpn2")],
            aleatorio: true,
            ..Alcance::default()
        };
        assert!(ok.validar(false, 1195, &[8470]).is_ok());
        let muitos = Alcance {
            remotos: vec![e("v"); MAX_REMOTOS + 1],
            ..Alcance::default()
        };
        assert!(muitos.validar(false, 1195, &[8470]).is_err());
        let sorteio_sozinho = Alcance {
            aleatorio: true,
            ..Alcance::default()
        };
        assert!(sorteio_sozinho.validar(false, 1195, &[8470]).is_err());
        let queda = Alcance {
            queda_tcp: Some(443),
            ..Alcance::default()
        };
        assert!(queda.validar(false, 1195, &[8470]).is_ok());
        assert!(
            queda.validar(true, 1195, &[8470]).is_err(),
            "queda em rede ja TCP"
        );
        let sorteio_e_queda = Alcance {
            aleatorio: true,
            ..ok.clone()
        };
        assert!(Alcance {
            queda_tcp: Some(443),
            ..sorteio_e_queda
        }
        .validar(false, 1195, &[8470])
        .is_err());
        let ps = Alcance {
            port_share: Some(("192.168.10.5".into(), 8443)),
            ..Alcance::default()
        };
        assert!(
            ps.validar(false, 1195, &[8470]).is_err(),
            "port-share sem porta TCP"
        );
        assert!(Alcance {
            queda_tcp: Some(443),
            ..ps.clone()
        }
        .validar(false, 1195, &[8470])
        .is_ok());
        assert_eq!(ps.validar(true, 1195, &[8470]).is_ok(), !cfg!(windows));
    }

    #[test]
    fn servidor_tcp_ganha_port_share_e_udp_ganha_a_ponte() {
        let d = std::env::temp_dir().join(format!("phxvpn-alcance-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let ps = Some(("192.168.10.5".to_string(), 8443));
        let tcp = Alcance {
            port_share: ps.clone(),
            ..Alcance::default()
        };
        assert_eq!(
            aplicar_na_pasta(&d, &rede(true), &tcp).unwrap(),
            "port-share 192.168.10.5 8443\n"
        );
        assert!(!d.join(queda_tcp::ARQUIVO).exists());
        let udp = Alcance {
            queda_tcp: Some(443),
            port_share: ps,
            ..Alcance::default()
        };
        assert_eq!(aplicar_na_pasta(&d, &rede(false), &udp).unwrap(), "");
        let c = queda_tcp::Conf::ler(&d).unwrap().unwrap();
        assert_eq!((c.porta, c.udp), (443, 1195));
        assert_eq!(c.port_share, Some(("192.168.10.5".into(), 8443)));
        // Tirar a queda apaga o arquivo: o supervisor derruba a ponte.
        aplicar_na_pasta(&d, &rede(false), &Alcance::default()).unwrap();
        assert!(queda_tcp::Conf::ler(&d).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
