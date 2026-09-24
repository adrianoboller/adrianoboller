//! Usuario, senha e codigo do autenticador NA CONEXAO OpenVPN.
//!
//! # O caminho
//!
//! ```text
//! cliente (auth-user-pass + static-challenge)
//!   -> openvpn servidor (ja como phxvpn-ovpn) grava usuario e senha num arquivo
//!   -> `phxvpn ovpn-mfa-verificar SOQUETE REDE ARQUIVO`  (auth-user-pass-verify via-file)
//!   -> soquete local do painel  -> PBKDF2 + TOTP + anti-reuso, no PostgreSQL
//! ```
//!
//! # Por que um soquete do painel, e nao o verificador lendo o banco
//!
//! O `openvpn` troca para o usuario proprio `phxvpn-ovpn` depois de abrir a
//! placa, e o verificador roda como ele. Ler o banco pediria a senha do
//! PostgreSQL legivel por esse usuario; conferir o TOTP pediria a chave do
//! selo tambem. Pelo soquete ele so consegue PERGUNTAR «este usuario, esta
//! senha, este codigo?», e a resposta passa pelo limitador do painel, com a
//! tentativa contada ANTES do PBKDF2. Quem pergunta passa por duas portas: o
//! arquivo do soquete (0660, grupo `phxvpn-ovpn`) e o `SO_PEERCRED` (so o uid
//! dele, root e o painel). Nao o `nobody`: ele e de todo daemon sem dono.
//!
//! # O usuario tem de ser o do certificado
//!
//! O CN do membro e `login.rede.serie`. O verificador recusa usuario que nao
//! e o login do CN, ou rede que nao e a desta instancia: sem isso, o
//! certificado da ana com a senha e o codigo do bruno entrava como ana.
//!
//! # Adiado, para nao parar a VPN
//!
//! O script do `auth-user-pass-verify` roda DENTRO do laco do `openvpn`: os
//! ~430 ms do PBKDF2 parariam o trafego de todos os membros a cada conexao.
//! Com `auth_control_file` no ambiente (OpenVPN 2.6), o verificador devolve 2
//! (adiado) na hora e um filho responde `1`/`0` no arquivo de controle.

use crate::http::Estado;
use crate::painel::conferir_login;
use crate::totp;
use phxsql_core::json::Json;
#[cfg(unix)]
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::time::Duration;

/// O que o perfil do membro ganha quando a rede exige o autenticador.
pub const PERFIL_MFA: &str = "# usuario, senha e codigo do autenticador (a rede exige)\n\
auth-user-pass\n\
static-challenge \"Código do autenticador\" 1\n";

/// O perfil pede o codigo a quem conecta? (linha `static-challenge`, com
/// ou sem espaco antes, fora de comentario).
pub fn perfil_pede_codigo(perfil: &str) -> bool {
    perfil
        .lines()
        .any(|l| l.trim_start().starts_with("static-challenge"))
}

/// Prazo de validade do token que o servidor entrega depois da conferencia:
/// a renegociacao de hora em hora usa o token em vez de pedir outro codigo.
/// Passado o prazo, codigo novo.
pub const VIDA_TOKEN: u32 = 12 * 3600;

/// Maior pedido aceito no soquete (usuario e senha sao curtos).
const TETO_PEDIDO: u64 = 4096;

pub fn socket(dados: &Path) -> PathBuf {
    dados.join("verificar.sock")
}

/// As linhas do `servidor.conf` de uma rede que exige o autenticador.
pub fn conf_servidor_mfa(dados: &Path, rede_id: &str) -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "phxvpn".into());
    format!(
        "# usuario, senha e codigo do autenticador (phxvpn ovpn-mfa-verificar)\n\
script-security 2\n\
auth-user-pass-verify \"{exe} ovpn-mfa-verificar {sock} {rede_id}\" via-file\n\
auth-gen-token {VIDA_TOKEN}\n",
        sock = socket(dados).display(),
    )
}

/// `login.rede.serie` -> (login, rede). O login nao tem ponto no inicio e
/// pode ter ponto no meio (`joao.silva`): corta pelos DOIS ultimos.
pub fn cn_login_rede(cn: &str) -> Option<(&str, &str)> {
    let (resto, serie) = cn.rsplit_once('.')?;
    let (login, rede) = resto.rsplit_once('.')?;
    (!login.is_empty() && !serie.is_empty() && rede.bytes().all(|b| b.is_ascii_digit()))
        .then_some((login, rede))
}

/// O pedido que o verificador leva ao painel.
pub fn pedido(rede: &str, cn: &str, usuario: &str, senha_crua: &str, ip: &str) -> Json {
    let (senha, codigo) = totp::scrv1(senha_crua);
    Json::objeto(vec![
        ("rede", Json::texto_de(rede)),
        ("cn", Json::texto_de(cn)),
        ("usuario", Json::texto_de(usuario)),
        ("senha", Json::texto_de(senha)),
        ("codigo", Json::texto_de(codigo)),
        ("ip", Json::texto_de(ip)),
    ])
}

// ------------------------------------ lado do OpenVPN (usuario proprio) ----

/// `phxvpn ovpn-mfa-verificar SOQUETE REDE ARQUIVO` -- chamado pelo
/// `openvpn`. Codigo de saida: 0 aceita, 1 recusa, 2 adiado. Tudo o que der
/// errado recusa.
pub fn principal(args: &[String]) -> i32 {
    let (Some(sock), Some(rede), Some(arquivo)) = (args.first(), args.get(1), args.last()) else {
        return 1;
    };
    let Ok(texto) = std::fs::read_to_string(arquivo) else {
        return 1;
    };
    let mut linhas = texto.lines();
    let usuario = linhas.next().unwrap_or_default();
    let senha = linhas.next().unwrap_or_default();
    let var = |n: &str| std::env::var(n).unwrap_or_default();
    // Cliente por IPv6: o openvpn so preenche `untrusted_ip6`, e sem ele o
    // limitador contaria todos esses clientes numa chave vazia so.
    let ip = Some(var("untrusted_ip"))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| var("untrusted_ip6"));
    let p = pedido(rede, &var("common_name"), usuario, senha, &ip).escrever();
    let controle = var("auth_control_file");
    if !controle.is_empty() {
        if let Ok(eu) = std::env::current_exe() {
            // A senha vai pelo stdin do filho, nunca por argumento (`ps`).
            let filho = std::process::Command::new(eu)
                .args(["ovpn-mfa-adiado", sock, &controle])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if let Ok(mut f) = filho {
                let escreveu = f
                    .stdin
                    .take()
                    .map(|mut e| e.write_all(p.as_bytes()).is_ok())
                    .unwrap_or(false);
                if escreveu {
                    return 2;
                }
                let _ = f.kill();
            }
        }
        // Sem conseguir adiar, confere aqui mesmo (mais lento, mas certo).
    }
    if perguntar(Path::new(sock), &p) {
        0
    } else {
        1
    }
}

/// `phxvpn ovpn-mfa-adiado SOQUETE CONTROLE`, o filho do adiado: le o
/// pedido do stdin, pergunta, e escreve `1` ou `0` no arquivo de controle.
pub fn adiado(args: &[String]) -> i32 {
    let (Some(sock), Some(controle)) = (args.first(), args.get(1)) else {
        return 1;
    };
    let mut p = String::new();
    let _ = std::io::stdin().take(TETO_PEDIDO).read_to_string(&mut p);
    let ok = perguntar(Path::new(sock), &p);
    // O arquivo ja existe (o openvpn o criou, como este mesmo usuario).
    match std::fs::write(controle, if ok { "1" } else { "0" }) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

#[cfg(unix)]
fn perguntar(sock: &Path, pedido: &str) -> bool {
    use std::os::unix::net::UnixStream;
    let Ok(mut s) = UnixStream::connect(sock) else {
        return false;
    };
    let prazo = Some(Duration::from_secs(30));
    let _ = s.set_read_timeout(prazo);
    let _ = s.set_write_timeout(prazo);
    if s.write_all(format!("{pedido}\n").as_bytes()).is_err() {
        return false;
    }
    let mut r = String::new();
    let _ = BufReader::new(s.take(TETO_PEDIDO)).read_line(&mut r);
    Json::analisar(r.trim())
        .map(|j| j.booleano_ou("ok", false))
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn perguntar(_sock: &Path, _pedido: &str) -> bool {
    // Sem soquete local no Windows (ainda): recusa, nunca aceita.
    false
}

// ---------------------------------------------------- lado do painel ----

/// A conferencia inteira de um pedido do verificador. `Err` traz o motivo,
/// que vai para o log do painel -- nunca a senha nem o codigo, e o usuario
/// digitado so escapado e cortado ([`para_log`]).
///
/// Limite que fica: `cn` e `ip` chegam no pedido. Quem os preenche e o
/// `openvpn` (pelo ambiente do verificador), e so o usuario dele passa pelo
/// `SO_PEERCRED`; um `openvpn` tomado pode mentir -- mas ele ja e quem decide
/// quem entra no tunel.
pub fn conferir(e: &Estado, j: &Json) -> Result<String, String> {
    let t = |c: &str| j.texto_ou(c, "").to_string();
    let (usuario, cn, rede) = (t("usuario"), t("cn"), t("rede"));
    let (login, rede_cn) = cn_login_rede(&cn).ok_or("CN fora do formato do phxvpn")?;
    // A conta e a MESMA do painel (orcamento unico); o IP e so deste canal,
    // para erro na VPN nao trancar o login do painel pelo mesmo IP.
    let conta = crate::http::chave_conta(login);
    let chave_ip = format!("ip-vpn:{}", t("ip"));
    let reserva = e
        .tentativas
        .reservar(&[&conta, &chave_ip])
        .map_err(crate::guarda::frase_de_bloqueio)?;
    if login != usuario {
        return Err(format!(
            "usuário «{}» não é o do certificado",
            para_log(&usuario)
        ));
    }
    if rede_cn != rede {
        return Err("certificado de outra rede".into());
    }
    let (u, hash) = match e.painel().hash_do_login(login) {
        Ok(x) => x,
        Err(m) => {
            reserva.devolver();
            return Err(m);
        }
    };
    let u = {
        let _vez = e.conferencias.adquirir();
        conferir_login(u, &hash, &t("senha"))
    }?;
    e.painel().mfa_conferir(u.id, &t("codigo"))?;
    reserva.acertou(&[&conta]);
    Ok(u.login)
}

/// Texto que veio de fora, para o log: so `[a-z0-9._-]`, ate 32.
pub fn para_log(t: &str) -> String {
    let limpo: String = t
        .chars()
        .take(32)
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '?'
            }
        })
        .collect();
    if t.chars().count() > 32 {
        limpo + "…"
    } else {
        limpo
    }
}

/// Quem pode perguntar: root, o proprio painel e o usuario do OpenVPN. NAO
/// o `nobody` quando existe o usuario proprio (ALTO 2 da revisao: com
/// `nobody` aceito, todo daemon sem dono da maquina era oraculo).
pub fn uid_aceito(uid: u32, eu: u32, do_openvpn: Option<u32>) -> bool {
    uid == 0 || uid == eu || Some(uid) == do_openvpn
}

#[cfg(target_os = "linux")]
fn quem_pergunta_pode(s: &std::os::unix::net::UnixStream) -> bool {
    use std::os::unix::io::AsRawFd;
    #[repr(C)]
    struct Ucred {
        pid: i32,
        uid: u32,
        gid: u32,
    }
    extern "C" {
        fn getsockopt(fd: i32, nivel: i32, nome: i32, valor: *mut Ucred, tam: *mut u32) -> i32;
        fn geteuid() -> u32;
    }
    const SOL_SOCKET: i32 = 1;
    const SO_PEERCRED: i32 = 17;
    let mut c = Ucred {
        pid: 0,
        uid: u32::MAX,
        gid: 0,
    };
    let mut tam = std::mem::size_of::<Ucred>() as u32;
    // SAFETY: `c` e `tam` vivem ate o fim da chamada e `tam` e o tamanho de `c`.
    let r = unsafe { getsockopt(s.as_raw_fd(), SOL_SOCKET, SO_PEERCRED, &mut c, &mut tam) };
    if r != 0 {
        return false;
    }
    // SAFETY: sem argumentos, nao falha.
    let eu = unsafe { geteuid() };
    let do_openvpn = crate::ovpn::usuario_do_openvpn()
        .and_then(|(u, _)| crate::ovpn::uid_gid(&u))
        .map(|(uid, _)| uid);
    uid_aceito(c.uid, eu, do_openvpn)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn quem_pergunta_pode(_s: &std::os::unix::net::UnixStream) -> bool {
    true
}

#[cfg(unix)]
fn gid_do_grupo(nome: &str) -> Option<u32> {
    std::fs::read_to_string("/etc/group")
        .ok()?
        .lines()
        .find_map(|l| {
            let p: Vec<&str> = l.split(':').collect();
            (p.len() > 2 && p[0] == nome).then(|| p[2].parse().ok())?
        })
}

/// Perguntas atendidas ao mesmo tempo; a mais recusa na porta.
#[cfg(unix)]
const TETO_PERGUNTAS: usize = 32;
/// Prazo para o pedido chegar: o verificador escreve na hora; quem conecta
/// e fica mudo so segura uma vaga por isto.
#[cfg(unix)]
const PRAZO_PEDIDO: Duration = Duration::from_secs(2);

/// Liga o soquete do verificador (uma thread por pergunta, com teto).
#[cfg(unix)]
pub fn servir(e: std::sync::Arc<Estado>) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let caminho = socket(e.painel().dados());
    let _ = std::fs::remove_file(&caminho);
    let ouvinte =
        UnixListener::bind(&caminho).map_err(|x| format!("soquete {}: {x}", caminho.display()))?;
    // Duas portas: o arquivo (0660, grupo do usuario do OpenVPN -- quem nao
    // e do grupo nem conecta) e o `SO_PEERCRED` (so o uid dele, root e o
    // painel). Sem o usuario proprio, o grupo e o do `nobody`, e o aviso diz.
    let grupo = crate::ovpn::usuario_do_openvpn().map(|(_, g)| g);
    let gid = grupo.as_deref().and_then(gid_do_grupo);
    if let Some(gid) = gid {
        std::os::unix::fs::chown(&caminho, None, Some(gid))
            .map_err(|x| format!("dono de {}: {x}", caminho.display()))?;
    }
    std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o660))
        .map_err(|x| format!("permissao de {}: {x}", caminho.display()))?;
    if crate::ovpn::usuario_do_openvpn().is_some_and(|(u, _)| u != crate::ovpn::USUARIO_OVPN) {
        eprintln!(
            "phxvpn: AVISO sem o usuario {}: o openvpn roda como nobody, e o soquete do \
             verificador aceita qualquer processo nobody (crie-o: phxvpn servico instalar painel)",
            crate::ovpn::USUARIO_OVPN
        );
    }
    let em_curso = std::sync::Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for s in ouvinte.incoming().flatten() {
            if em_curso.load(Ordering::SeqCst) >= TETO_PERGUNTAS {
                continue;
            }
            em_curso.fetch_add(1, Ordering::SeqCst);
            let (e, em_curso) = (e.clone(), em_curso.clone());
            std::thread::spawn(move || {
                atender(&e, s);
                em_curso.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    Ok(caminho)
}

#[cfg(unix)]
fn atender(e: &Estado, mut s: std::os::unix::net::UnixStream) {
    let _ = s.set_read_timeout(Some(PRAZO_PEDIDO));
    let _ = s.set_write_timeout(Some(PRAZO_PEDIDO));
    let ok = if quem_pergunta_pode(&s) {
        let mut linha = String::new();
        let lido = s
            .try_clone()
            .map(|c| BufReader::new(c.take(TETO_PEDIDO)).read_line(&mut linha));
        match (lido, Json::analisar(linha.trim())) {
            (Ok(Ok(_)), Ok(j)) => {
                let rede = para_log(j.texto_ou("rede", ""));
                match conferir(e, &j) {
                    Ok(login) => {
                        eprintln!("phxvpn: VPN rede {rede}: «{login}» conferido (senha e código)");
                        true
                    }
                    Err(m) => {
                        eprintln!("phxvpn: VPN rede {rede}: recusado -- {m}");
                        false
                    }
                }
            }
            _ => false,
        }
    } else {
        eprintln!("phxvpn: soquete do verificador: pergunta de usuário não autorizado, recusada");
        false
    };
    let _ = s.write_all(
        format!(
            "{}\n",
            Json::objeto(vec![("ok", Json::de_bool(ok))]).escrever()
        )
        .as_bytes(),
    );
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cn_da_o_login_e_a_rede() {
        assert_eq!(cn_login_rede("ana.3.ab12cd34"), Some(("ana", "3")));
        assert_eq!(
            cn_login_rede("joao.silva.12.ab12cd34"),
            Some(("joao.silva", "12"))
        );
        assert_eq!(cn_login_rede("ana.x.ab12"), None);
        assert_eq!(cn_login_rede("ana"), None);
        assert_eq!(cn_login_rede(".3.ab"), None);
    }

    #[test]
    fn conf_e_perfil_levam_o_verificador() {
        let c = conf_servidor_mfa(Path::new("/var/lib/phxvpn"), "7");
        assert!(c.contains("script-security 2\n"));
        assert!(c.contains("ovpn-mfa-verificar /var/lib/phxvpn/verificar.sock 7\" via-file\n"));
        assert!(c.contains("auth-gen-token 43200\n"));
        assert!(PERFIL_MFA.contains("auth-user-pass\n"));
        assert!(PERFIL_MFA.contains("static-challenge \"Código do autenticador\" 1\n"));
    }

    #[test]
    fn pedido_separa_o_codigo_da_senha() {
        let b = phxsql_core::base64::codificar;
        let p = pedido(
            "1",
            "ana.1.aa",
            "ana",
            &format!("SCRV1:{}:{}", b(b"s3nha"), b(b"123456")),
            "192.0.2.1",
        );
        assert_eq!(p.texto_ou("senha", ""), "s3nha");
        assert_eq!(p.texto_ou("codigo", ""), "123456");
    }

    #[cfg(unix)]
    #[test]
    fn uid_do_root_sai_do_passwd() {
        assert_eq!(crate::ovpn::uid_gid("root"), Some((0, 0)));
        assert_eq!(crate::ovpn::uid_gid("ninguem-com-este-nome"), None);
    }

    /// ALTO 2: com o usuario proprio existindo, `nobody` NAO pergunta.
    /// RED: a regra antiga aceitava `nobody` sempre.
    #[test]
    fn nobody_nao_pergunta_quando_ha_usuario_proprio() {
        let (painel, ovpn, nobody) = (0, 998, 65534);
        assert!(uid_aceito(ovpn, painel, Some(ovpn)));
        assert!(uid_aceito(0, 1000, Some(ovpn)));
        assert!(!uid_aceito(nobody, painel, Some(ovpn)), "nobody passou");
        assert!(!uid_aceito(1000, painel, Some(ovpn)));
        assert!(!uid_aceito(1000, painel, None));
    }

    #[test]
    fn usuario_digitado_vai_ao_log_escapado_e_cortado() {
        assert_eq!(para_log("ana"), "ana");
        assert_eq!(para_log("a\n phxvpn: aceito"), "a??phxvpn??aceito");
        assert_eq!(para_log(&"x".repeat(40)), format!("{}…", "x".repeat(32)));
    }
}
