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
//! Limite medido (`provas/rotas`): isso so vale com o host SEM encaminhar.
//! Com `ip_forward=1` o kernel roteia de `tun0` para `tun1` (3/3 pings da
//! rede B na rede A). Quem acende o encaminhamento para a LAN da empresa
//! (`rotas.rs`) acende junto a tabela de guarda -- e a guarda fica sempre
//! que ha rede, para o host que ja encaminhava por outro motivo (Docker,
//! roteador) nao abrir A para B.
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
    /// `proto tcp-server` / `tcp-client` em vez de UDP: a rede que so deixa
    /// sair TCP/443, ou so por proxy HTTP (que so leva TCP).
    pub tcp: bool,
}

impl Rede<'_> {
    /// `explicit-exit-notify 1`, so em UDP. Em TCP o fim da conexao ja e o
    /// aviso, e o OpenVPN descarta a linha com um NOTICE (options.c:3265-3268
    /// da 2.6.19) -- escreve-la so poria ruido no log.
    ///
    /// No perfil: quem sai avisa, e some da lista de membros em ~9 s em vez
    /// de ~129 s (o `keepalive 10 60` do servidor dobra para 120 s). No
    /// servidor: ao receber SIGTERM ele manda `RESTART` a todos e espera 2 s
    /// (multi.c, `multi_push_restart_schedule_exit`), e o membro volta em
    /// segundos em vez de esperar o `ping-restart 60`.
    pub(crate) fn aviso_de_saida(&self) -> &'static str {
        if self.tcp {
            ""
        } else {
            "explicit-exit-notify 1\n"
        }
    }

    /// ` force-cookie` na `tls-crypt-v2`, so em UDP: o servidor so guarda
    /// estado de quem devolve o cookie (aperto de tres vias sem estado), e
    /// um datagrama forjado nao faz o servidor desembrulhar chave nem gastar
    /// memoria. O padrao do OpenVPN ainda e `allow-noncookie`
    /// (tls-options.rst:511-516 da 2.6.19). Em TCP o aperto do proprio TCP
    /// ja prova o endereco e so o `mudp.c:122` le a opcao.
    ///
    /// Quem fica de fora e so cliente antigo: o 2.5 (medido em netns, o
    /// 2.5.11 nao entra em 25 s; o 2.6.19 entra, e sem a opcao o 2.5.11
    /// tambem) e o OpenVPN 3 anterior ao core 3.8 -- o core manda o
    /// `EARLY_NEG_START` e reenvia a WKc desde 2ff291e7 (16/11/2022). Ver
    /// PHXVPN.md, «force-cookie».
    fn cookie(&self) -> &'static str {
        if self.tcp {
            ""
        } else {
            " force-cookie"
        }
    }

    pub(crate) fn proto(&self, lado: &str) -> String {
        if self.tcp {
            format!("tcp-{lado}")
        } else {
            "udp".into()
        }
    }
}

/// `host:porta` de um proxy HTTP que pode ir para dentro do perfil. O perfil
/// e um arquivo de diretivas, uma por linha: espaco ou quebra de linha aqui
/// injetaria diretiva (`up /bin/sh ...`) no perfil de outra pessoa.
pub fn validar_http_proxy(t: &str) -> Result<(String, u16), String> {
    let (h, p) = t
        .rsplit_once(':')
        .ok_or("http-proxy no formato host:porta")?;
    let porta = p
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or("http-proxy: porta invalida")?;
    let h = h.trim_start_matches('[').trim_end_matches(']');
    let ok = (1..=253).contains(&h.len())
        && h.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b));
    if !ok {
        return Err("http-proxy: host so com letras, digitos, ponto, hifen e dois-pontos".into());
    }
    Ok((h.to_string(), porta))
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
proto {proto}\n\
dev tun\n\
topology subnet\n\
server {sub} 255.255.255.0\n\
client-to-client\n\
client-config-dir {dir}/ccd\n\
ccd-exclusive\n\
keepalive 10 60\n\
{saida}\
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
        proto = rede.proto("server"),
        sub = subrede(rede.octeto),
        saida = rede.aviso_de_saida(),
        sem_root = sem_root(),
        tls = if rede.v2 {
            // O comando roda a cada conexao, ANTES do TLS: membro removido
            // nao chega nem a gastar um aperto de mao do servidor.
            format!(
                "tls-crypt-v2 {dir}/tls-crypt.key{}\n\
                 script-security 2\n\
                 tls-crypt-v2-verify \"{} ovpn-v2-verificar {dir}\"\n",
                rede.cookie(),
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

/// O usuario PROPRIO com que o `openvpn` do servidor roda depois de abrir
/// a placa. Nao o `nobody`: o soquete do verificador (`verificar.rs`) aceita
/// perguntas do usuario do OpenVPN, e `nobody` e de todo daemon sem dono da
/// maquina -- qualquer um deles viraria oraculo de senha e codigo.
pub const USUARIO_OVPN: &str = "phxvpn-ovpn";

/// Cria o usuario de sistema (sem casa, sem shell) se faltar. Pede root, e
/// roda fora da caixa do systemd (`ProtectSystem=strict` deixa o `/etc` so
/// de leitura): no `servico instalar painel` e no arranque em primeiro plano.
pub fn garantir_usuario_dedicado() -> Result<(), String> {
    if cfg!(windows) || uid_gid(USUARIO_OVPN).is_some() {
        return Ok(());
    }
    let r = std::process::Command::new("useradd")
        .args([
            "--system",
            "--user-group",
            "--no-create-home",
            "--home-dir",
            "/nonexistent",
            "--shell",
            "/usr/sbin/nologin",
            USUARIO_OVPN,
        ])
        .output()
        .map_err(|e| format!("useradd: {e}"))?;
    if r.status.success() || uid_gid(USUARIO_OVPN).is_some() {
        Ok(())
    } else {
        Err(format!(
            "useradd {USUARIO_OVPN}: {}",
            String::from_utf8_lossy(&r.stderr).trim()
        ))
    }
}

/// (uid, gid primario) de um usuario, lidos do `/etc/passwd`.
pub fn uid_gid(nome: &str) -> Option<(u32, u32)> {
    uid_gid_em(&std::fs::read_to_string("/etc/passwd").ok()?, nome)
}

/// O mesmo, num texto no formato do `/etc/passwd` (os testes passam o seu).
pub fn uid_gid_em(passwd: &str, nome: &str) -> Option<(u32, u32)> {
    passwd.lines().find_map(|l| {
        let p: Vec<&str> = l.split(':').collect();
        (p.len() > 3 && p[0] == nome).then(|| Some((p[2].parse().ok()?, p[3].parse().ok()?)))?
    })
}

/// (usuario, grupo) para o `user`/`group` do conf: o proprio, se existe; se
/// nao, `nobody` -- so para rede SEM autenticador: o verificador nunca aceita
/// `nobody`, e rede que exige o autenticador nao sobe sem o usuario proprio
/// (`mfa::exigir_usuario_proprio`). O grupo do `nobody` muda entre distribuicoes (`nogroup` no
/// Debian, `nobody` no Fedora).
pub fn usuario_do_openvpn() -> Option<(String, String)> {
    if cfg!(windows) {
        return None;
    }
    if uid_gid(USUARIO_OVPN).is_some() {
        return Some((USUARIO_OVPN.into(), USUARIO_OVPN.into()));
    }
    let grupos = std::fs::read_to_string("/etc/group").unwrap_or_default();
    let tem = |g: &str| grupos.lines().any(|l| l.starts_with(&format!("{g}:")));
    ["nogroup", "nobody"]
        .into_iter()
        .find(|g| tem(g))
        .map(|g| ("nobody".into(), g.into()))
}

/// Depois de abrir a placa e ler as chaves, o `openvpn` troca de usuario:
/// uma falha no processo que fala com a internet nao vira root. So o que
/// ele rele a cada conexao (`crl.pem`, `ccd/`) precisa ser legivel por esse
/// usuario -- por isso a pasta da rede e de passagem (0711), e as chaves
/// continuam 0600, lidas antes da troca (`persist-key`). No Windows nao ha
/// troca.
pub fn sem_root() -> String {
    match usuario_do_openvpn() {
        Some((u, g)) => format!("user {u}\ngroup {g}\n"),
        None => String::new(),
    }
}

/// O arquivo `ccd/<cn>` do membro: o IP fixo dele na rede.
pub fn ccd_membro(octeto: u8, host: u8) -> String {
    format!("ifconfig-push {} 255.255.255.0\n", ip_membro(octeto, host))
}

/// O nome do arquivo do perfil de uma rede: o painel o sugere na resposta e
/// a linha de comando o usa quando nao ha `--saida` (e para por a
/// credencial do proxy ao lado dele) -- uma regra so.
pub fn arquivo_do_perfil(rede: &str) -> String {
    let limpo: String = rede
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("phxvpn-{limpo}.ovpn")
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
    /// Enderecos alternativos, queda para TCP e o proxy do membro
    /// (`alcance.rs`). `None`: um `remote` so, sem proxy -- o perfil de antes.
    pub conexao: Option<&'a crate::alcance::Conexao>,
}

pub fn perfil_membro(p: &Perfil) -> String {
    let c = crate::alcance::partes(p.rede, p.servidor.endereco, p.conexao);
    format!(
        "# phxvpn -- rede «{rede}» em {srv}\n\
client\n\
dev tun\n\
{topo}\
resolv-retry infinite\n\
nobind\n\
{saida}\
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
{tc}\
{fim}",
        rede = p.rede.nome,
        srv = p.servidor.nome,
        topo = c.topo,
        saida = c.saida,
        fim = c.fim,
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
            tcp: false,
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
                conexao: None,
            })
        };
        assert!(p(&format!("{INICIO_V2_CLIENTE}\nxx\n")).contains("<tls-crypt-v2>"));
        assert!(p("-----BEGIN OpenVPN Static key V1-----\n").contains("<tls-crypt>"));
        let c = conf_servidor(&r, "/d");
        assert!(c.contains("tls-crypt-v2 /d/tls-crypt.key") && c.contains("ovpn-v2-verificar /d"));
    }

    /// Item 8: rede v2 em UDP exige o cookie (sem estado para datagrama
    /// forjado); em TCP a linha fica sem o parametro, que o `mtcp` nao le.
    /// RED: sem o `cookie()`, a linha sai `tls-crypt-v2 ARQ` e o servidor
    /// aceita o cliente 2.5 que nao devolve cookie (provado em netns).
    #[test]
    fn v2_em_udp_exige_o_cookie() {
        for tcp in [false, true] {
            let r = Rede {
                nome: "x",
                porta: 1,
                octeto: 1,
                v2: true,
                tcp,
            };
            let c = conf_servidor(&r, "/d");
            let com = "tls-crypt-v2 /d/tls-crypt.key force-cookie\n";
            assert_eq!(c.contains(com), !tcp, "tcp={tcp}: {c}");
            assert!(c.contains("tls-crypt-v2 /d/tls-crypt.key"));
        }
        // A v1 nao tem cookie a pedir: a linha nao muda.
        let v1 = Rede {
            nome: "x",
            porta: 1,
            octeto: 1,
            v2: false,
            tcp: false,
        };
        assert!(conf_servidor(&v1, "/d").contains("tls-crypt /d/tls-crypt.key\n"));
    }

    #[test]
    fn servidor_isola_por_rede_e_exige_ccd() {
        let r = Rede {
            nome: "Matriz",
            porta: 1195,
            octeto: 1,
            v2: false,
            tcp: false,
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
            tcp: false,
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
            conexao: None,
        });
        assert!(p.contains("remote vpn.empresa.com.br 1195\n"));
        assert!(p.contains("remote-cert-tls server\n"));
        assert!(p.contains("verify-x509-name vpn1 name\n"));
        assert!(p.contains("<key>\nK\n</key>"));
    }

    #[test]
    fn rede_tcp_vira_tcp_server_e_perfil_tcp_client_com_proxy() {
        let r = Rede {
            nome: "Hotel",
            porta: 443,
            octeto: 2,
            v2: false,
            tcp: true,
        };
        let c = conf_servidor(&r, "/d");
        assert!(c.contains("port 443\nproto tcp-server\n"), "{c}");
        let s = Servidor {
            nome: "vpn1",
            endereco: "vpn.empresa.com.br",
        };
        let conexao = crate::alcance::Conexao {
            proxy: Some(crate::alcance::ProxyMembro::http("proxy.hotel.local:3128").unwrap()),
            ..Default::default()
        };
        let perfil = |rede: &Rede| {
            perfil_membro(&Perfil {
                rede,
                servidor: &s,
                ca_pem: "",
                cert_pem: "",
                chave_pem: "",
                tls_crypt: "",
                conexao: Some(&conexao),
            })
        };
        let t = perfil(&r);
        assert!(t.contains("proto tcp-client\nremote vpn.empresa.com.br 443\nhttp-proxy proxy.hotel.local 3128\n"), "{t}");
        // Em UDP o proxy nao entra (o OpenVPN recusaria o perfil).
        let u = Rede { tcp: false, ..r };
        let t = perfil(&u);
        assert!(t.contains("proto udp\n") && !t.contains("http-proxy"));
        assert!(conf_servidor(&u, "/d").contains("proto udp\n"));
    }

    /// Em UDP, os dois lados avisam a saida: o membro que sai some da lista,
    /// e o servidor que reinicia chama todos de volta. Em TCP a linha nao
    /// vai -- o OpenVPN a ignoraria com um NOTICE.
    #[test]
    fn aviso_de_saida_so_em_udp_nos_dois_lados() {
        let s = Servidor {
            nome: "vpn1",
            endereco: "vpn.empresa.com.br",
        };
        for tcp in [false, true] {
            let r = Rede {
                nome: "R",
                porta: 1195,
                octeto: 1,
                v2: false,
                tcp,
            };
            let perfil = perfil_membro(&Perfil {
                rede: &r,
                servidor: &s,
                ca_pem: "",
                cert_pem: "",
                chave_pem: "",
                tls_crypt: "",
                conexao: None,
            });
            let conf = conf_servidor(&r, "/d");
            let linha = "\nexplicit-exit-notify 1\n";
            assert_eq!(perfil.contains(linha), !tcp, "perfil tcp={tcp}: {perfil}");
            assert_eq!(conf.contains(linha), !tcp, "servidor tcp={tcp}: {conf}");
        }
    }

    #[test]
    fn http_proxy_nao_injeta_diretiva_no_perfil() {
        assert!(validar_http_proxy("10.0.0.4:3128").is_ok());
        assert!(validar_http_proxy("[2001:db8::1]:3128").is_ok());
        for ruim in ["p:3128\nup /bin/sh", "p 3128", "p:0", "p", "p;x:1", ":3128"] {
            assert!(validar_http_proxy(ruim).is_err(), "{ruim:?} passou");
        }
    }
}
