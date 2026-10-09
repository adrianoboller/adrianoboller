use crate::error::{BrowserError, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use url::{Host, Url};

/// Politica de rede do navegador. Nega por padrao: sem origem listada e sem
/// `allow_any_public`, nada navega. O agente le paginas escolhidas por texto
/// que ele mesmo nao controla, entao cada destino tem de ser uma decisao.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserPolicy {
    /// Origens liberadas explicitamente (`esquema://host[:porta]`). Liberacao
    /// explicita vence `block_private_networks`: e o unico jeito de alcancar
    /// um servico interno, e ele tem de ser escrito, nunca implicito.
    pub allowed_origins: Vec<String>,
    /// Libera qualquer host cujo IP resolvido seja publico.
    pub allow_any_public: bool,
    /// Recusa loopback, rede privada, link-local e afins (SSRF).
    pub block_private_networks: bool,
}

impl Default for BrowserPolicy {
    fn default() -> Self {
        Self {
            allowed_origins: Vec::new(),
            allow_any_public: false,
            block_private_networks: true,
        }
    }
}

/// Prazo da resolucao de nome dentro da checagem; sem ele um DNS lento
/// seguraria a requisicao pausada no Chromium indefinidamente.
const DNS_TIMEOUT: Duration = Duration::from_secs(5);

impl BrowserPolicy {
    /// Politica que so libera as origens dadas.
    pub fn only(origins: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed_origins: origins.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    pub fn allow_origin(mut self, origin: impl Into<String>) -> Self {
        self.allowed_origins.push(origin.into());
        self
    }

    fn origin_listed(&self, url: &Url) -> bool {
        let alvo = url.origin().ascii_serialization();
        self.allowed_origins.iter().any(|o| {
            // Normaliza pela mesma via do alvo: "http://h:80/" e "http://h"
            // sao a mesma origem, e comparar texto cru deixaria passar ou
            // barrar conforme a grafia de quem configurou.
            Url::parse(o)
                .map(|u| u.origin().ascii_serialization() == alvo)
                .unwrap_or(false)
        })
    }

    /// Decide se `raw` pode ser carregado. Resolve o host por DNS e confere
    /// os IPs resolvidos: conferir so o texto deixaria passar um nome publico
    /// que aponta para 127.0.0.1.
    pub async fn check_url(&self, raw: &str) -> Result<Url> {
        self.check_url_resolved(raw).await.map(|(url, _)| url)
    }

    /// Como `check_url`, devolvendo tambem os enderecos que a conferencia resolveu para um
    /// NOME (vazio quando o host e IP literal, ou quando a origem passou sem conferir IP).
    /// Quem conecta por fora do Chromium fixa a conexao neles: resolver de novo na hora de
    /// conectar abriria a janela do DNS rebinding entre a conferencia e o uso.
    pub async fn check_url_resolved(&self, raw: &str) -> Result<(Url, Vec<SocketAddr>)> {
        let negar = |reason: &str| BrowserError::PolicyDenied {
            url: raw.to_string(),
            reason: reason.to_string(),
        };
        let url = Url::parse(raw).map_err(|e| negar(&format!("url invalida: {e}")))?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(negar(&format!("esquema '{}' nao permitido", url.scheme())));
        }
        let host = url.host().ok_or_else(|| negar("url sem host"))?;
        if self.origin_listed(&url) {
            return Ok((url, vec![]));
        }
        if !self.allow_any_public {
            return Err(negar("origem fora da lista permitida"));
        }
        if !self.block_private_networks {
            return Ok((url, vec![]));
        }
        let port = url.port_or_known_default().unwrap_or(80);
        let mut por_nome = false;
        let ips: Vec<IpAddr> = match host {
            Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
            Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
            Host::Domain(nome) => {
                por_nome = true;
                let consulta = tokio::net::lookup_host((nome, port));
                match tokio::time::timeout(DNS_TIMEOUT, consulta).await {
                    Ok(Ok(it)) => it.map(|s| s.ip()).collect(),
                    Ok(Err(e)) => return Err(negar(&format!("dns falhou: {e}"))),
                    Err(_) => return Err(negar("dns sem resposta no prazo")),
                }
            }
        };
        if ips.is_empty() {
            return Err(negar("dns nao devolveu endereco"));
        }
        // Basta um IP interno para recusar: o Chromium escolhe sozinho qual
        // endereco usar, entao todos tem de ser publicos.
        if let Some(ip) = ips.iter().find(|ip| is_blocked_ip(**ip)) {
            return Err(negar(&format!("endereco interno {ip}")));
        }
        let enderecos = if por_nome {
            ips.iter().map(|ip| SocketAddr::new(*ip, port)).collect()
        } else {
            vec![]
        };
        Ok((url, enderecos))
    }
}

/// Enderecos que nao sao internet publica. Vai alem da lista minima
/// (loopback/privado/link-local) porque CGNAT, "este host" e o IPv4 embutido
/// em IPv6 sao os desvios classicos de filtro de SSRF.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_unspecified()
        || o[0] == 0
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64/10 CGNAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18/15
        || o[0] >= 240 // 240/4 reservado
}

fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }
    let s = ip.segments();
    let v4_em = |alto: u16, baixo: u16| {
        Ipv4Addr::new(
            (alto >> 8) as u8,
            alto as u8,
            (baixo >> 8) as u8,
            baixo as u8,
        )
    };
    // As formas que carregam um IPv4 e que o kernel (ou o tradutor da rede) entrega ao
    // IPv4 de dentro: conferir so o prefixo IPv6 deixaria 169.254.169.254 passar disfarcado.
    // 64:ff9b::/96 (NAT64 bem conhecido, RFC 6052): o IPv4 nos ultimos 32 bits.
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return is_blocked_v4(v4_em(s[6], s[7]));
    }
    // 64:ff9b:1::/48 (NAT64 de uso local, RFC 8215): o comprimento do prefixo e escolha do
    // operador (/48, /56, /64 ou /96), e cada um poe o IPv4 num lugar diferente (RFC 6052
    // §2.2). Conferir um so lugar deixaria os outros passarem; o prefixo e de uso local e
    // nao e internet publica, entao fecha inteiro.
    if s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1 {
        return true;
    }
    // ::ffff:0:a.b.c.d (IPv4 traduzido, SIIT, RFC 6145): o IPv4 nos ultimos 32 bits.
    if s[..4] == [0, 0, 0, 0] && s[4] == 0xffff && s[5] == 0 {
        return is_blocked_v4(v4_em(s[6], s[7]));
    }
    // 2002::/16 (6to4, RFC 3056): o IPv4 do roteador de borda nos bits 16..48.
    if s[0] == 0x2002 {
        return is_blocked_v4(v4_em(s[1], s[2]));
    }
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // fc00::/7
        || (s[0] & 0xffc0) == 0xfe80 // fe80::/10
        || (s[0] & 0xffc0) == 0xfec0 // fec0::/10 site-local (obsoleto, ainda roteado dentro)
        || (s[0] == 0x2001 && s[1] == 0x0db8) // documentacao
        || s[..6] == [0, 0, 0, 0, 0, 0] // ::/96 compativel com IPv4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifica_enderecos_internos() {
        for s in [
            "127.0.0.1",
            "127.9.9.9",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.10",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "64:ff9b::a9fe:a9fe",
            // RED medido: cada forma abaixo passava antes do conserto (`is_blocked_v6` sem
            // o 6to4, o NAT64 local, o site-local e o IPv4 traduzido).
            "64:ff9b:1::a9fe:a9fe",
            "64:ff9b:1:abcd::7f00:1",
            "2002:a9fe:a9fe::1",
            "2002:7f00:1::",
            "2002:0a00:0001::1",
            "fec0::1",
            "feff::1",
            "::ffff:0:a9fe:a9fe",
            "::ffff:0:7f00:1",
        ] {
            assert!(is_blocked_ip(ip(s)), "{s} devia ser interno");
        }
        // O 6to4 e o IPv4 traduzido de um IPv4 PUBLICO continuam publicos: a conferencia
        // decodifica, nao fecha o prefixo inteiro.
        for s in [
            "8.8.8.8",
            "172.32.0.1",
            "93.184.216.34",
            "2606:4700::1111",
            "2002:5db8:d822::1",
            "::ffff:0:808:808",
        ] {
            assert!(!is_blocked_ip(ip(s)), "{s} devia ser publico");
        }
    }

    #[tokio::test]
    async fn padrao_nega_tudo() {
        let p = BrowserPolicy::default();
        let e = p.check_url("http://93.184.216.34/").await.unwrap_err();
        assert!(matches!(e, BrowserError::PolicyDenied { .. }));
    }

    #[tokio::test]
    async fn so_http_e_https() {
        let p = BrowserPolicy {
            allow_any_public: true,
            ..Default::default()
        };
        for u in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,oi",
            "chrome://settings",
            "ftp://93.184.216.34/",
        ] {
            assert!(p.check_url(u).await.is_err(), "{u} devia ser recusada");
        }
        assert!(p.check_url("https://93.184.216.34/x").await.is_ok());
    }

    #[tokio::test]
    async fn publico_recusa_interno_inclusive_disfarcado() {
        let p = BrowserPolicy {
            allow_any_public: true,
            ..Default::default()
        };
        for u in [
            "http://127.0.0.1:8080/",
            "http://2130706433/",
            "http://0x7f.1/",
            "http://[::1]/",
            "http://[::ffff:169.254.169.254]/",
            "http://169.254.169.254/latest/meta-data",
            "http://localhost/",
        ] {
            let r = p.check_url(u).await;
            assert!(
                matches!(r, Err(BrowserError::PolicyDenied { .. })),
                "{u} devia ser recusada: {r:?}"
            );
        }
    }

    #[tokio::test]
    async fn origem_listada_e_normalizada_e_so_ela_passa() {
        let p = BrowserPolicy::only(["http://127.0.0.1:80/"]);
        assert!(p.check_url("http://127.0.0.1/a?b=1").await.is_ok());
        assert!(p.check_url("http://127.0.0.1:81/").await.is_err());
        assert!(p.check_url("https://127.0.0.1/").await.is_err());
    }

    #[tokio::test]
    async fn sem_bloqueio_de_rede_privada_publico_libera_tudo() {
        let p = BrowserPolicy {
            allow_any_public: true,
            block_private_networks: false,
            ..Default::default()
        };
        assert!(p.check_url("http://127.0.0.1/").await.is_ok());
    }
}
