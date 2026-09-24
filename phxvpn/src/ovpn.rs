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
    /// Chave `tls-crypt-v2` (uma por membro) em vez da `tls-crypt` unica.
    pub v2: bool,
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
        "# phxvpn -- rede «{nome}» (gerado; nao edite, o painel reescreve)\n\
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
{tls}\
crl-verify {dir}/crl.pem\n\
dh none\n\
tls-version-min 1.3\n\
remote-cert-tls client\n\
data-ciphers {CIFRAS}\n\
status {dir}/status.log 10\n\
status-version 2\n\
{sem_root}\
verb 3\n",
        nome = rede.nome,
        porta = rede.porta,
        sub = subrede(rede.octeto),
        sem_root = sem_root(),
        tls = if rede.v2 {
            // O comando roda a cada conexao, ANTES do TLS: membro removido
            // nao chega nem a gastar um aperto de mao do servidor.
            format!(
                "tls-crypt-v2 {dir}/tls-crypt.key\n\
                 script-security 2\n\
                 tls-crypt-v2-verify \"{} ovpn-v2-verificar {dir}\"\n",
                exe_para_o_openvpn()
            )
        } else {
            format!("tls-crypt {dir}/tls-crypt.key\n")
        },
    )
}

fn exe_para_o_openvpn() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "phxvpn".into())
}

// ------------------------------------------------------ tls-crypt-v2 ----
//
// Com a `tls-crypt` (v1) todo membro carrega a MESMA chave: quem sai da rede
// continua podendo fazer o servidor abrir um aperto TLS (e a CRL so o barra
// depois). Com a v2 cada membro leva a propria chave, EMBRULHADA pela chave
// do servidor e com a serie do certificado dentro; o servidor desembrulha e
// pergunta ao `phxvpn ovpn-v2-verificar` se a serie foi revogada -- antes do
// TLS. O embrulho e AES-256-CTR + HMAC no formato do OpenVPN, e quem o gera
// e o proprio `openvpn`, que o modo servidor ja exige: AES em software foi
// recusado no nucleo (tempo que vaza), e o formato e dele.

pub const INICIO_V2_SERVIDOR: &str = "-----BEGIN OpenVPN tls-crypt-v2 server key-----";
const INICIO_V2_CLIENTE: &str = "-----BEGIN OpenVPN tls-crypt-v2 client key-----";

pub fn e_v2(chave: &str) -> bool {
    chave.starts_with(INICIO_V2_SERVIDOR)
}

/// Pasta 0700 so para esta geracao; some no fim, com as chaves dentro.
fn pasta_temporaria() -> R<std::path::PathBuf> {
    let d = std::env::temp_dir().join(format!(
        "phxvpn-v2-{}",
        phxsql_core::hash::para_hex(&phxsql_core::senha::bytes_aleatorios(8))
    ));
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut b, 0o700);
    #[cfg(not(unix))]
    let b = &mut b;
    b.create(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    Ok(d)
}

type R<T> = Result<T, String>;

fn openvpn(args: &[&std::ffi::OsStr]) -> R<()> {
    let bin = crate::supervisor::achar_no_path("openvpn").ok_or("openvpn nao esta no PATH")?;
    let s = std::process::Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if s.status.success() {
        Ok(())
    } else {
        Err(format!(
            "openvpn --genkey: {}",
            String::from_utf8_lossy(&s.stderr).trim()
        ))
    }
}

/// Chave de servidor v2 nova. `None` sem o `openvpn` no PATH: a rede nasce
/// com a v1, e quem chama diz isso.
pub fn gerar_v2_servidor() -> Option<String> {
    let d = pasta_temporaria().ok()?;
    let k = d.join("s.key");
    let r = openvpn(&[
        "--genkey".as_ref(),
        "tls-crypt-v2-server".as_ref(),
        k.as_os_str(),
    ])
    .and_then(|_| std::fs::read_to_string(&k).map_err(|e| e.to_string()));
    let _ = std::fs::remove_dir_all(&d);
    r.ok().filter(|t| e_v2(t))
}

/// Chave do membro, embrulhada pela do servidor, com `serie:<hex>` dentro.
pub fn gerar_v2_cliente(servidor: &str, serie_hex: &str) -> R<String> {
    let d = pasta_temporaria()?;
    let (ks, kc) = (d.join("s.key"), d.join("c.key"));
    let meta = phxsql_core::base64::codificar(format!("serie:{serie_hex}").as_bytes());
    let r = std::fs::write(&ks, servidor)
        .map_err(|e| e.to_string())
        .and_then(|_| {
            openvpn(&[
                "--tls-crypt-v2".as_ref(),
                ks.as_os_str(),
                "--genkey".as_ref(),
                "tls-crypt-v2-client".as_ref(),
                kc.as_os_str(),
                meta.as_ref(),
            ])
        })
        .and_then(|_| std::fs::read_to_string(&kc).map_err(|e| e.to_string()));
    let _ = std::fs::remove_dir_all(&d);
    let k = r?;
    if !k.starts_with(INICIO_V2_CLIENTE) {
        return Err("openvpn gerou uma chave de cliente torta".into());
    }
    Ok(k)
}

/// O que o `tls-crypt-v2-verify` pergunta. Falha fechado: sem metadados do
/// phxvpn (tipo 0, `serie:`), ou com a serie em `revogados.txt`, recusa.
pub fn verificar_v2(dir: &std::path::Path, tipo: &str, metadados: &[u8]) -> bool {
    let Some(serie) = std::str::from_utf8(metadados)
        .ok()
        .and_then(|t| t.strip_prefix("serie:"))
    else {
        return false;
    };
    if tipo != "0" || serie.is_empty() || !serie.bytes().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    match std::fs::read_to_string(dir.join("revogados.txt")) {
        Ok(t) => !t.lines().any(|l| l.trim().eq_ignore_ascii_case(serie)),
        // Sem a lista nao se sabe quem saiu: recusa.
        Err(_) => false,
    }
}

/// Depois de abrir a placa e ler as chaves, o `openvpn` troca de usuario:
/// uma falha no processo que fala com a internet nao vira root. So o que
/// ele rele a cada conexao (`crl.pem`, `ccd/`) precisa ser legivel por esse
/// usuario -- por isso a pasta da rede e de passagem (0711), e as chaves
/// continuam 0600, lidas antes da troca (`persist-key`).
///
/// O grupo muda de nome entre distribuicoes (`nogroup` no Debian, `nobody`
/// no Fedora); o que existir no `/etc/group`. No Windows nao ha troca.
pub fn sem_root() -> String {
    if cfg!(windows) {
        return String::new();
    }
    let grupos = std::fs::read_to_string("/etc/group").unwrap_or_default();
    let tem = |g: &str| grupos.lines().any(|l| l.starts_with(&format!("{g}:")));
    match ["nogroup", "nobody"].into_iter().find(|g| tem(g)) {
        Some(g) => format!("user nobody\ngroup {g}\n"),
        None => String::new(),
    }
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
        "# phxvpn -- rede «{rede}» em {srv}\n\
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
tls-version-min 1.3\n\
data-ciphers {CIFRAS}\n\
verb 3\n\
<ca>\n{ca}</ca>\n\
<cert>\n{cert}</cert>\n\
<key>\n{key}</key>\n\
{tc}",
        rede = p.rede.nome,
        srv = p.servidor.nome,
        end = p.servidor.endereco,
        porta = p.rede.porta,
        ca = p.ca_pem,
        cert = p.cert_pem,
        key = p.chave_pem,
        tc = if p.tls_crypt.starts_with(INICIO_V2_CLIENTE) {
            format!("<tls-crypt-v2>\n{}</tls-crypt-v2>\n", p.tls_crypt)
        } else {
            format!("<tls-crypt>\n{}</tls-crypt>\n", p.tls_crypt)
        },
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn verificador_v2_barra_revogado_e_falha_fechado() {
        let d = std::env::temp_dir().join(format!("phxvpn-v2ver-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // Sem a lista: ninguem entra.
        assert!(!verificar_v2(&d, "0", b"serie:ab12"));
        std::fs::write(d.join("revogados.txt"), "AB12\nffee\n").unwrap();
        assert!(verificar_v2(&d, "0", b"serie:cd34"));
        assert!(!verificar_v2(&d, "0", b"serie:ab12"), "revogada entrou");
        assert!(
            !verificar_v2(&d, "0", b"serie:FFEE"),
            "caixa diferente entrou"
        );
        // Chave sem metadados do phxvpn (tipo 1 = so carimbo), torta ou vazia.
        assert!(!verificar_v2(&d, "1", b"serie:cd34"));
        assert!(!verificar_v2(&d, "0", b"outra:cd34"));
        assert!(!verificar_v2(&d, "0", b"serie:"));
        assert!(!verificar_v2(&d, "0", b"serie:cd34\nab12"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn perfil_v2_leva_a_etiqueta_v2() {
        let r = Rede {
            nome: "x",
            porta: 1,
            octeto: 1,
            v2: true,
        };
        let s = Servidor {
            nome: "s",
            endereco: "e",
        };
        let p = |tc: &str| {
            perfil_membro(&Perfil {
                rede: &r,
                servidor: &s,
                ca_pem: "",
                cert_pem: "",
                chave_pem: "",
                tls_crypt: tc,
            })
        };
        assert!(p(&format!("{INICIO_V2_CLIENTE}\nxx\n")).contains("<tls-crypt-v2>"));
        assert!(p("-----BEGIN OpenVPN Static key V1-----\n").contains("<tls-crypt>"));
        let c = conf_servidor(&r, "/d");
        assert!(c.contains("tls-crypt-v2 /d/tls-crypt.key") && c.contains("ovpn-v2-verificar /d"));
    }

    #[test]
    fn servidor_isola_por_rede_e_exige_ccd() {
        let r = Rede {
            nome: "Matriz",
            porta: 1195,
            octeto: 1,
            v2: false,
        };
        let c = conf_servidor(&r, "/var/lib/phxvpn/redes/1");
        assert!(c.contains("port 1195\n"));
        assert!(c.contains("server 10.77.1.0 255.255.255.0\n"));
        assert!(c.contains("ccd-exclusive\n"));
        assert!(c.contains("client-to-client\n"));
        assert!(c.contains("remote-cert-tls client\n"));
        assert!(c.contains("crl-verify /var/lib/phxvpn/redes/1/crl.pem\n"));
        assert!(c.contains("tls-version-min 1.3\n"));
    }

    #[test]
    fn perfil_confere_o_servidor_pelo_nome_e_pelo_uso() {
        let r = Rede {
            nome: "Matriz",
            porta: 1195,
            octeto: 1,
            v2: false,
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
