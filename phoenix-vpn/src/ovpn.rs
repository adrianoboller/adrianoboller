//! Geracao das configuracoes do OpenVPN: a do servidor de cada rede e o perfil
//! `.ovpn` de cada membro.
//!
//! # Uma instancia de OpenVPN por rede
//!
//! E o que da o isolamento do Radmin sem regra de firewall: cada rede tem a
//! sua porta, a sua sub-rede /24, a sua chave `tls-crypt` e o seu `ccd/`.
//! Quem esta na rede A nao tem como falar com a rede B, porque sao processos e
//! interfaces diferentes. `client-to-client` dentro da rede e o que faz ela
//! parecer uma LAN: os membros se enxergam direto.
//!
//! # Por que `ccd-exclusive`
//!
//! Com ele, so conecta quem tem arquivo em `ccd/` -- e o painel escreve um por
//! membro, com o IP fixo dele. Tirar alguem da rede e apagar o arquivo: o
//! certificado continua valido pela AC, mas a porta desta rede nao o aceita
//! mais. Sem precisar de CRL para o caso comum.

/// Dados de uma rede, do jeito que as duas configuracoes precisam.
pub struct Rede<'a> {
    pub nome: &'a str,
    pub porta: u16,
    /// Terceiro octeto da sub-rede `10.77.N.0/24`.
    pub octeto: u8,
}

pub struct Servidor<'a> {
    pub nome: &'a str,
    pub endereco: &'a str,
}

pub fn subrede(octeto: u8) -> String {
    format!("10.77.{octeto}.0")
}

pub fn ip_membro(octeto: u8, host: u8) -> String {
    format!("10.77.{octeto}.{host}")
}

const CIFRAS: &str = "AES-256-GCM:CHACHA20-POLY1305";

pub fn conf_servidor(rede: &Rede, dir: &str) -> String {
    format!(
        "# Phoenix VPN -- rede «{nome}» (gerado; nao edite, o painel reescreve)\n\
port {porta}\n\
proto udp\n\
dev tun\n\
topology subnet\n\
server {sub} 255.255.255.0\n\
client-to-client\n\
client-config-dir {dir}/ccd\n\
ccd-exclusive\n\
keepalive 10 60\n\
persist-key\n\
persist-tun\n\
ca {dir}/ca.crt\n\
cert {dir}/servidor.crt\n\
key {dir}/servidor.key\n\
tls-crypt {dir}/tls-crypt.key\n\
dh none\n\
tls-version-min 1.2\n\
remote-cert-tls client\n\
data-ciphers {CIFRAS}\n\
status {dir}/status.log 10\n\
status-version 2\n\
verb 3\n",
        nome = rede.nome,
        porta = rede.porta,
        sub = subrede(rede.octeto),
    )
}

/// O arquivo `ccd/<cn>` do membro: o IP fixo dele na rede.
pub fn ccd_membro(octeto: u8, host: u8) -> String {
    format!("ifconfig-push {} 255.255.255.0\n", ip_membro(octeto, host))
}

/// O perfil `.ovpn` do membro, com tudo embutido: um arquivo so, que o
/// OpenVPN Connect, o OpenVPN GUI e o `openvpn --config` abrem igual.
pub struct Perfil<'a> {
    pub rede: &'a Rede<'a>,
    pub servidor: &'a Servidor<'a>,
    pub ca_pem: &'a str,
    pub cert_pem: &'a str,
    pub chave_pem: &'a str,
    pub tls_crypt: &'a str,
}

pub fn perfil_membro(p: &Perfil) -> String {
    format!(
        "# Phoenix VPN -- rede «{rede}» em {srv}\n\
client\n\
dev tun\n\
proto udp\n\
remote {end} {porta}\n\
resolv-retry infinite\n\
nobind\n\
persist-key\n\
persist-tun\n\
remote-cert-tls server\n\
verify-x509-name {srv} name\n\
tls-version-min 1.2\n\
data-ciphers {CIFRAS}\n\
verb 3\n\
<ca>\n{ca}</ca>\n\
<cert>\n{cert}</cert>\n\
<key>\n{key}</key>\n\
<tls-crypt>\n{tc}</tls-crypt>\n",
        rede = p.rede.nome,
        srv = p.servidor.nome,
        end = p.servidor.endereco,
        porta = p.rede.porta,
        ca = p.ca_pem,
        cert = p.cert_pem,
        key = p.chave_pem,
        tc = p.tls_crypt,
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn servidor_isola_por_rede_e_exige_ccd() {
        let r = Rede {
            nome: "Matriz",
            porta: 1195,
            octeto: 1,
        };
        let c = conf_servidor(&r, "/var/lib/phxvpn/redes/1");
        assert!(c.contains("port 1195\n"));
        assert!(c.contains("server 10.77.1.0 255.255.255.0\n"));
        assert!(c.contains("ccd-exclusive\n"));
        assert!(c.contains("client-to-client\n"));
        assert!(c.contains("remote-cert-tls client\n"));
    }

    #[test]
    fn perfil_confere_o_servidor_pelo_nome_e_pelo_uso() {
        let r = Rede {
            nome: "Matriz",
            porta: 1195,
            octeto: 1,
        };
        let s = Servidor {
            nome: "vpn1",
            endereco: "vpn.empresa.com.br",
        };
        let p = perfil_membro(&Perfil {
            rede: &r,
            servidor: &s,
            ca_pem: "CA\n",
            cert_pem: "C\n",
            chave_pem: "K\n",
            tls_crypt: "T\n",
        });
        assert!(p.contains("remote vpn.empresa.com.br 1195\n"));
        assert!(p.contains("remote-cert-tls server\n"));
        assert!(p.contains("verify-x509-name vpn1 name\n"));
        assert!(p.contains("<key>\nK\n</key>"));
    }
}
