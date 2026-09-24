//! Sessao aberta so vale enquanto a credencial de quem a abriu nao mudou.
//!
//! # O motor, e por que ele mora no banco
//!
//! `phx_usuario.credencial` e um contador que um GATILHO do PostgreSQL sobe
//! a cada mudanca do que autentica ou autoriza alguem: senha, `ativo`,
//! autenticador (`totp_selado`), `admin` e `login`. Toda sessao guarda o
//! valor com que nasceu, e a conferencia de TODA sessao -- a do painel (o
//! token do navegador) e a da VPN (o `session_id` do `auth-gen-token`) --
//! passa por [`Sessoes::conferir`], que pergunta ao banco se o usuario ainda
//! esta ativo e se o contador ainda e o mesmo.
//!
//! No gatilho, e nao em cada rota, porque rota que muda usuario e lista que
//! cresce: a que alguem esquecer de avisar deixaria a sessao antiga viva, e
//! um `UPDATE` feito a mao no banco (desativar alguem pelo `psql`) nao passa
//! por rota nenhuma. O contador nao sobe com `totp_ultimo` (todo codigo
//! aceito o grava) nem com `totp_pendente` (cadastro que nao terminou nao
//! muda o que autentica).
//!
//! # A sessao de quem fez a mudanca fica
//!
//! Quem troca a PROPRIA senha ou liga/desliga o PROPRIO autenticador
//! continua na sessao em que fez isso; as outras dele caem. E o que a OWASP
//! ASVS 4.0.3 pede em 3.3.3 («terminate all OTHER active sessions after a
//! successful password change»), o que o Django faz no
//! `update_session_auth_hash` e o que o GitLab faz ao ligar o 2FA
//! (`destroy_all_but_current_user_session!`). Mudanca feita pelo admin em
//! OUTRO usuario derruba todas as sessoes do alvo. Registro em
//! `docs/PHXVPN.md` (limite 12).
//!
//! # E a conexao VPN cai na hora
//!
//! Esperar a proxima renegociacao (ate 1 h) nao basta: [`Estado::credencial_mudou`]
//! pede ao `openvpn` de cada rede, pela interface de gerencia (soquete Unix
//! privado), o `client-kill` de cada conexao do CN do membro. O cliente
//! reconecta com o token -- e o token cai no mesmo motor: `external-auth`
//! faz o `openvpn` chamar o verificador mesmo com token valido, e a sessao
//! de credencial velha e recusada.

use crate::http::Estado;
use crate::painel::{Painel, Usuario};
use crate::pg::R;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Coluna e gatilho; idempotente, roda a cada abertura como o resto do
/// esquema. `CREATE TRIGGER` so quando ainda nao existe: o PostgreSQL
/// anterior ao 14 nao tem `CREATE OR REPLACE TRIGGER`.
pub const ESQUEMA: &str = "
ALTER TABLE phx_usuario ADD COLUMN IF NOT EXISTS credencial bigint NOT NULL DEFAULT 0;
CREATE OR REPLACE FUNCTION phx_credencial_mudou() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.senha_hash IS DISTINCT FROM OLD.senha_hash
       OR NEW.ativo IS DISTINCT FROM OLD.ativo
       OR NEW.totp_selado IS DISTINCT FROM OLD.totp_selado
       OR NEW.admin IS DISTINCT FROM OLD.admin
       OR NEW.login IS DISTINCT FROM OLD.login THEN
        NEW.credencial := OLD.credencial + 1;
    END IF;
    RETURN NEW;
END
$$;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger
                   WHERE tgname = 'phx_credencial' AND tgrelid = 'phx_usuario'::regclass) THEN
        CREATE TRIGGER phx_credencial BEFORE UPDATE ON phx_usuario
            FOR EACH ROW EXECUTE FUNCTION phx_credencial_mudou();
    END IF;
END
$$;
";

/// Por que uma sessao foi recusada. `Banco` nao derruba a sessao: o
/// PostgreSQL fora do ar nao e motivo para deslogar todo mundo.
#[derive(Debug, PartialEq)]
pub enum Recusa {
    Desconhecida,
    Expirou,
    CredencialMudou,
    Banco(String),
}

struct Sessao {
    usuario_id: i64,
    credencial: i64,
    desde: Instant,
}

/// Sessoes abertas (do painel ou da VPN), cada uma presa a credencial com
/// que nasceu.
pub struct Sessoes {
    vida: Duration,
    mapa: Mutex<HashMap<String, Sessao>>,
}

impl Sessoes {
    pub fn nova(vida: Duration) -> Sessoes {
        Sessoes {
            vida,
            mapa: Mutex::new(HashMap::new()),
        }
    }

    fn mapa(&self) -> std::sync::MutexGuard<'_, HashMap<String, Sessao>> {
        self.mapa.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn abrir(&self, chave: String, u: &Usuario) {
        let mut m = self.mapa();
        m.retain(|_, s| s.desde.elapsed() < self.vida);
        m.insert(
            chave,
            Sessao {
                usuario_id: u.id,
                credencial: u.credencial,
                desde: Instant::now(),
            },
        );
    }

    pub fn fechar(&self, chave: &str) {
        self.mapa().remove(chave);
    }

    /// A conferencia de toda sessao. `vigente` le do banco o usuario ATIVO
    /// com a credencial de agora (`None`: inativo ou apagado). A trava das
    /// sessoes nao fica presa durante a ida ao banco.
    pub fn conferir(
        &self,
        chave: &str,
        vigente: impl FnOnce(i64) -> R<Option<Usuario>>,
    ) -> Result<Usuario, Recusa> {
        let (id, credencial) = {
            let mut m = self.mapa();
            match m.get(chave) {
                None => return Err(Recusa::Desconhecida),
                Some(s) if s.desde.elapsed() >= self.vida => {
                    m.remove(chave);
                    return Err(Recusa::Expirou);
                }
                Some(s) => (s.usuario_id, s.credencial),
            }
        };
        match vigente(id) {
            Ok(Some(u)) if u.credencial == credencial => Ok(u),
            Ok(_) => {
                self.fechar(chave);
                Err(Recusa::CredencialMudou)
            }
            Err(m) => Err(Recusa::Banco(m)),
        }
    }

    /// A sessao de quem mudou a PROPRIA credencial passa a valer pela nova.
    /// So se ela ainda existe e e do mesmo usuario: nao ressuscita sessao
    /// fechada nem adota a de outro.
    pub fn renovar(&self, chave: &str, u: &Usuario) {
        if let Some(s) = self.mapa().get_mut(chave) {
            if s.usuario_id == u.id {
                s.credencial = u.credencial;
            }
        }
    }
}

impl Painel {
    /// O usuario como ele e AGORA, se ativo -- a pergunta do motor.
    pub fn usuario_vigente(&mut self, id: i64) -> R<Option<Usuario>> {
        let r = self.pg()?.executar(
            "SELECT id, login, admin, credencial FROM phx_usuario WHERE id = $1::int AND ativo",
            &[Some(&id.to_string())],
        )?;
        if r.linhas.is_empty() {
            return Ok(None);
        }
        crate::painel::usuario_da_linha(&r, 0).map(Some)
    }

    /// O admin desativa (ou reativa) OUTRO usuario. A si mesmo nao: o
    /// ultimo admin se trancaria para fora do painel.
    pub fn usuario_definir_ativo(&mut self, ator: &Usuario, login: &str, ativo: bool) -> R<i64> {
        if !ator.admin {
            return Err("só o administrador desativa usuário".into());
        }
        if login == ator.login {
            return Err("o administrador não desativa a si mesmo".into());
        }
        let r = self.pg()?.executar(
            "UPDATE phx_usuario SET ativo = $2::boolean WHERE login = $1 RETURNING id",
            &[Some(login), Some(if ativo { "true" } else { "false" })],
        )?;
        r.valor(0, "id")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| "usuário inexistente".into())
    }

    /// Grava o hash novo (a conta cara e a conferencia da senha atual
    /// acontecem antes, fora da trava). O gatilho sobe a credencial.
    pub fn gravar_senha(&mut self, id: i64, hash: &str) -> R<()> {
        self.pg()?.executar(
            "UPDATE phx_usuario SET senha_hash = $2 WHERE id = $1::int",
            &[Some(&id.to_string()), Some(hash)],
        )?;
        Ok(())
    }

    /// (rede, CN) de cada vinculo do usuario: o que se derruba na gerencia.
    pub fn vinculos_do_usuario(&mut self, id: i64) -> R<Vec<(String, String)>> {
        let r = self.pg()?.executar(
            "SELECT rede_id, cn FROM phx_membro WHERE usuario_id = $1::int ORDER BY rede_id",
            &[Some(&id.to_string())],
        )?;
        Ok((0..r.linhas.len())
            .map(|i| {
                (
                    r.valor(i, "rede_id").unwrap_or_default().to_string(),
                    r.valor(i, "cn").unwrap_or_default().to_string(),
                )
            })
            .collect())
    }
}

impl Estado {
    /// A credencial de `usuario_id` mudou (o gatilho ja subiu o contador):
    /// a sessao `manter` -- a de quem mudou a propria -- passa a valer pela
    /// nova; as outras caem na proxima conferencia; o `ccd/` acompanha o
    /// `ativo`; e a conexao VPN do usuario cai agora, em cada rede.
    pub fn credencial_mudou(&self, usuario_id: i64, manter: Option<&str>) {
        let (vinculos, fresco, dados) = {
            let mut p = self.painel();
            if let Err(m) = p.acertar_ccd(Some(usuario_id)) {
                eprintln!("phxvpn: ccd do usuario {usuario_id}: {m}");
            }
            let fresco = match manter {
                Some(_) => p.usuario_vigente(usuario_id).ok().flatten(),
                None => None,
            };
            (
                p.vinculos_do_usuario(usuario_id).unwrap_or_default(),
                fresco,
                p.dados().to_path_buf(),
            )
        };
        if let (Some(tk), Some(u)) = (manter, fresco) {
            self.sessoes.renovar(tk, &u);
        }
        // Fora da trava do painel: a gerencia de um openvpn parado espera
        // o prazo, e o painel nao para junto.
        for (rede, cn) in vinculos {
            match derrubar(&socket(&dados, &rede), &cn) {
                Ok(true) => eprintln!(
                    "phxvpn: rede {rede}: conexao de «{}» derrubada (credencial mudou)",
                    crate::verificar::para_log(&cn)
                ),
                Ok(false) => {}
                Err(m) => eprintln!("phxvpn: rede {rede}: gerencia do openvpn: {m}"),
            }
        }
    }
}

// ------------------------------------------- gerencia do openvpn ----

/// Soquete de gerencia de uma rede. Fica em `dados/gerencia/` (0700, do
/// dono do painel) e nao na pasta da rede, que e de passagem (0711) para o
/// `openvpn` sem root: o OpenVPN cria o soquete com `umask(0)`, e por padrao
/// «qualquer processo pode conectar» (manual 2.6, `--management`).
pub fn socket(dados: &Path, rede_id: &str) -> PathBuf {
    dados.join("gerencia").join(format!("{rede_id}.sock"))
}

/// As linhas do `servidor.conf` que ligam a gerencia. Duas portas: a pasta
/// 0700 e o `management-client-user` (so o usuario do painel conversa).
/// No Windows nao ha soquete Unix para a gerencia: sem linha, e a queda
/// fica para a renegociacao.
pub fn conf(dados: &Path, rede_id: &str) -> String {
    if cfg!(windows) {
        return String::new();
    }
    let mut c = format!(
        "# gerencia: o painel derruba a conexao de quem mudou de credencial\n\
         management {} unix\n",
        socket(dados, rede_id).display()
    );
    if let Some(u) = usuario_do_painel() {
        c.push_str(&format!("management-client-user {u}\n"));
    }
    c
}

#[cfg(target_os = "linux")]
fn usuario_do_painel() -> Option<String> {
    extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: sem argumentos, nao falha.
    let eu = unsafe { geteuid() };
    nome_do_uid(&std::fs::read_to_string("/etc/passwd").ok()?, eu)
}

#[cfg(not(target_os = "linux"))]
fn usuario_do_painel() -> Option<String> {
    None
}

pub fn nome_do_uid(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|l| {
        let p: Vec<&str> = l.split(':').collect();
        (p.len() > 2 && p[2].parse() == Ok(uid)).then(|| p[0].to_string())
    })
}

/// Derruba as conexoes do CN pela gerencia. `Ok(true)`: derrubou;
/// `Ok(false)`: nao estava conectado (ou a rede nao tem gerencia no ar).
///
/// `client-kill CID`, e nao `kill CN`: no 2.6 o `kill` fecha a instancia no
/// servidor sem avisar o cliente (`multi_signal_instance`), e o cliente so
/// percebe no `ping-restart` (60 s aqui); o `client-kill` manda o `RESTART`
/// pelo canal de controle (`send_restart`), e o cliente reconecta na hora --
/// com o token, que cai no motor e e recusado. O CID sai do `status 2`.
#[cfg(unix)]
pub fn derrubar(sock: &Path, cn: &str) -> Result<bool, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    let mut s = match UnixStream::connect(sock) {
        Ok(s) => s,
        // Rede sem openvpn no ar (ou sem gerencia ainda): nada a derrubar.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("{}: {e}", sock.display())),
    };
    let prazo = Some(Duration::from_secs(3));
    let _ = s.set_read_timeout(prazo);
    let _ = s.set_write_timeout(prazo);
    let mut leitor = BufReader::new(s.try_clone().map_err(|e| e.to_string())?);
    // Uma linha que satisfaz `fim`, pulando as notificacoes (`>INFO:`...),
    // que chegam a qualquer hora.
    let mut ler_ate = |fim: &dyn Fn(&str) -> bool| -> Result<Vec<String>, String> {
        let mut linhas = Vec::new();
        loop {
            let mut l = String::new();
            match leitor.read_line(&mut l) {
                Ok(0) => return Err("gerencia fechou sem responder".into()),
                Ok(_) => {}
                Err(e) => return Err(format!("gerencia: {e}")),
            }
            let l = l.trim_end().to_string();
            if l.starts_with('>') {
                continue;
            }
            let acabou = fim(&l);
            linhas.push(l);
            if acabou {
                return Ok(linhas);
            }
        }
    };
    s.write_all(b"status 2\n")
        .map_err(|e| format!("gerencia: {e}"))?;
    let status = ler_ate(&|l| l == "END" || l.starts_with("ERROR:"))?;
    let cids = cids_do_cn(&status, cn);
    for cid in &cids {
        s.write_all(format!("client-kill {cid}\n").as_bytes())
            .map_err(|e| format!("gerencia: {e}"))?;
        let r = ler_ate(&|l| l.starts_with("SUCCESS:") || l.starts_with("ERROR:"))?;
        if let Some(erro) = r.last().filter(|l| l.starts_with("ERROR:")) {
            return Err(erro.clone());
        }
    }
    let _ = s.write_all(b"quit\n");
    Ok(!cids.is_empty())
}

/// Os `Client ID` do CN no `status 2` (`CLIENT_LIST,cn,real,virtual,
/// virtual6,recebidos,enviados,desde,desde_t,usuario,CID,...`). So numero:
/// o CID vai numa linha de comando.
pub fn cids_do_cn(status: &[String], cn: &str) -> Vec<String> {
    status
        .iter()
        .filter_map(|l| l.strip_prefix("CLIENT_LIST,"))
        .map(|l| l.split(',').collect::<Vec<_>>())
        .filter(|c| c.first() == Some(&cn) && c.len() > 9)
        .map(|c| c[9].to_string())
        .filter(|cid| !cid.is_empty() && cid.bytes().all(|b| b.is_ascii_digit()))
        .collect()
}

#[cfg(not(unix))]
pub fn derrubar(_sock: &Path, _cn: &str) -> Result<bool, String> {
    Err("sem gerencia do openvpn no Windows: a conexao cai na renegociacao".into())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn u(id: i64, credencial: i64) -> Usuario {
        Usuario {
            id,
            login: "ana".into(),
            admin: false,
            credencial,
        }
    }

    /// RED: com a conferencia ignorando o contador, a sessao de credencial
    /// velha passa.
    #[test]
    fn sessao_cai_quando_a_credencial_muda() {
        let s = Sessoes::nova(Duration::from_secs(60));
        s.abrir("a".into(), &u(7, 3));
        s.abrir("b".into(), &u(7, 3));
        assert_eq!(s.conferir("a", |_| Ok(Some(u(7, 3)))).unwrap().id, 7);
        // Mudou: quem fez a mudanca renova a propria; a outra cai.
        s.renovar("a", &u(7, 4));
        assert!(s.conferir("a", |_| Ok(Some(u(7, 4)))).is_ok());
        assert_eq!(
            s.conferir("b", |_| Ok(Some(u(7, 4)))).unwrap_err(),
            Recusa::CredencialMudou
        );
        // E nao volta nem com o contador antigo: foi fechada.
        assert_eq!(
            s.conferir("b", |_| Ok(Some(u(7, 3)))).unwrap_err(),
            Recusa::Desconhecida
        );
        // Desativado: `vigente` devolve None.
        assert_eq!(
            s.conferir("a", |_| Ok(None)).unwrap_err(),
            Recusa::CredencialMudou
        );
    }

    #[test]
    fn banco_fora_nao_derruba_e_renovar_nao_adota_a_de_outro() {
        let s = Sessoes::nova(Duration::from_secs(60));
        s.abrir("a".into(), &u(7, 1));
        assert!(matches!(
            s.conferir("a", |_| Err("PostgreSQL fora".into())),
            Err(Recusa::Banco(_))
        ));
        assert!(s.conferir("a", |_| Ok(Some(u(7, 1)))).is_ok());
        s.renovar("a", &u(8, 2));
        assert!(s.conferir("a", |_| Ok(Some(u(7, 1)))).is_ok());
        s.renovar("sumida", &u(7, 2));
        assert_eq!(
            s.conferir("sumida", |_| Ok(Some(u(7, 2)))).unwrap_err(),
            Recusa::Desconhecida
        );
    }

    #[test]
    fn sessao_expira() {
        let s = Sessoes::nova(Duration::ZERO);
        s.abrir("a".into(), &u(1, 0));
        assert_eq!(
            s.conferir("a", |_| Ok(Some(u(1, 0)))).unwrap_err(),
            Recusa::Expirou
        );
    }

    #[test]
    fn nome_do_uid_sai_do_passwd() {
        let p = "root:x:0:0::/root:/bin/sh\nphxvpn:x:998:998::/:/sbin/nologin\n";
        assert_eq!(nome_do_uid(p, 998).as_deref(), Some("phxvpn"));
        assert_eq!(nome_do_uid(p, 5), None);
    }

    #[cfg(unix)]
    #[test]
    fn conf_liga_a_gerencia_em_pasta_privada() {
        let c = conf(Path::new("/var/lib/phxvpn"), "3");
        assert!(c.contains("management /var/lib/phxvpn/gerencia/3.sock unix\n"));
    }

    #[test]
    fn cid_sai_do_status_2() {
        let st: Vec<String> = [
            "TITLE,OpenVPN 2.6.19",
            "HEADER,CLIENT_LIST,Common Name,Real Address,Virtual Address,Virtual IPv6 Address,Bytes Received,Bytes Sent,Connected Since,Connected Since (time_t),Username,Client ID,Peer ID,Data Channel Cipher",
            "CLIENT_LIST,ana.1.ab,192.0.2.1:5000,10.77.1.3,,10,20,2026-09-24 10:00:00,1790000000,ana,7,0,AES-256-GCM",
            "CLIENT_LIST,bia.1.cd,192.0.2.2:5000,10.77.1.4,,10,20,2026-09-24 10:00:00,1790000000,bia,8,1,AES-256-GCM",
            "CLIENT_LIST,ana.1.ab,192.0.2.1:5001,10.77.1.3,,10,20,2026-09-24 10:00:00,1790000000,ana,9;x,0,AES-256-GCM",
            "END",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(cids_do_cn(&st, "ana.1.ab"), vec!["7"]);
        assert_eq!(cids_do_cn(&st, "bia.1.cd"), vec!["8"]);
        assert!(cids_do_cn(&st, "ana.1").is_empty());
    }

    /// Contra uma gerencia de mentira: pede o `status 2`, manda o
    /// `client-kill` do CID do CN (e so dele) e le a resposta.
    #[cfg(unix)]
    #[test]
    fn derrubar_pede_o_status_e_mata_pelo_cid() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("phxvpn-gerencia-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("1.sock");
        let _ = std::fs::remove_file(&sock);
        let ouvinte = UnixListener::bind(&sock).unwrap();
        let t = std::thread::spawn(move || {
            let mut pedidos = Vec::new();
            for _ in 0..2 {
                let (mut c, _) = ouvinte.accept().unwrap();
                c.write_all(b">INFO:OpenVPN Management Interface Version 5\r\n")
                    .unwrap();
                let mut leitor = BufReader::new(c.try_clone().unwrap());
                loop {
                    let mut l = String::new();
                    if leitor.read_line(&mut l).unwrap() == 0 {
                        break;
                    }
                    pedidos.push(l.trim().to_string());
                    let r = match l.trim() {
                        "status 2" => "CLIENT_LIST,ana.1.ab,192.0.2.1:5000,10.77.1.3,,1,2,d,1,ana,7,0,c\r\n>BYTECOUNT:1,2\r\nEND\r\n",
                        "client-kill 7" => "SUCCESS: client-kill command succeeded\r\n",
                        _ => break,
                    };
                    c.write_all(r.as_bytes()).unwrap();
                }
            }
            pedidos
        });
        assert_eq!(derrubar(&sock, "ana.1.ab"), Ok(true));
        assert_eq!(derrubar(&sock, "bia.1.cd"), Ok(false));
        assert_eq!(
            t.join().unwrap(),
            ["status 2", "client-kill 7", "quit", "status 2", "quit"]
        );
        assert_eq!(derrubar(&dir.join("nenhum.sock"), "x"), Ok(false));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
