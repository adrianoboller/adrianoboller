use crate::error::{BrowserError, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
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
            return Ok(url);
        }
        if !self.allow_any_public {
            return Err(negar("origem fora da lista permitida"));
        }
        if !self.block_private_networks {
            return Ok(url);
        }
        let port = url.port_or_known_default().unwrap_or(80);
        let ips: Vec<IpAddr> = match host {
            Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
            Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
            Host::Domain(nome) => {
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
        Ok(url)
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
    // 64:ff9b::/96 (NAT64) carrega um IPv4 que pode ser interno.
    if s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        let v4 = Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8);
        return is_blocked_v4(v4);
    }
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00 // fc00::/7
        || (s[0] & 0xffc0) == 0xfe80 // fe80::/10
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
        ] {
            assert!(is_blocked_ip(ip(s)), "{s} devia ser interno");
        }
        for s in ["8.8.8.8", "172.32.0.1", "93.184.216.34", "2606:4700::1111"] {
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
