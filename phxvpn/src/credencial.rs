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

    /// A sessao de quem mudou a PROPRIA credencial passa a valer pela nova
    /// -- mas so se `novo` (o que o `UPDATE ... RETURNING credencial` da
    /// propria mudanca devolveu) e exatamente o da sessao + 1 (ou o mesmo,
    /// quando nada mudou). Qualquer outro valor quer dizer que alguem mudou
    /// a conta no meio (o admin zerou o autenticador entre a conferencia da
    /// sessao e a gravacao): a sessao atual cai tambem. Reler o banco para
    /// renovar -- o que se fazia antes -- adotava a mudanca do outro.
    /// Devolve se a sessao ficou.
    pub fn renovar(&self, chave: &str, usuario_id: i64, novo: i64) -> bool {
        let mut m = self.mapa();
        let Some(s) = m.get_mut(chave) else {
            return false;
        };
        if s.usuario_id != usuario_id {
            return false;
        }
        if novo == s.credencial || novo == s.credencial + 1 {
            s.credencial = novo;
            return true;
        }
        m.remove(chave);
        false
    }
}

/// Onde o gancho de teste roda (ver `Estado::gancho_de_teste`).
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Momento {
    /// Depois de conferida a sessao, antes de gravar a propria mudanca.
    AntesDeGravar,
    /// Depois de gravada, antes de renovar a sessao de quem mudou.
    AntesDeRenovar,
}

/// De quanto em quanto tempo o painel reconcilia o banco com o `ccd/` e as
/// conexoes: e o que alcanca a mudanca feita fora das rotas (o `UPDATE` pelo
/// `psql`) e refaz a que falhou. A pergunta e um `SELECT id, credencial` da
/// tabela de usuarios; medido na prova (`provas/mfa/rodar.sh`).
pub const VIGIA: Duration = Duration::from_secs(2);

/// Prazo TOTAL de uma conversa com a gerencia de uma rede: a rota que muda
/// um usuario espera no maximo isto por rede, nunca o soquete inteiro.
pub const PRAZO_GERENCIA: Duration = Duration::from_secs(2);

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
    /// ultimo admin se trancaria para fora do painel. Devolve (id, credencial).
    pub fn usuario_definir_ativo(
        &mut self,
        ator: &Usuario,
        login: &str,
        ativo: bool,
    ) -> R<(i64, i64)> {
        if !ator.admin {
            return Err("só o administrador desativa usuário".into());
        }
        if login == ator.login {
            return Err("o administrador não desativa a si mesmo".into());
        }
        let r = self.pg()?.executar(
            "UPDATE phx_usuario SET ativo = $2::boolean WHERE login = $1 RETURNING id, credencial",
            &[Some(login), Some(if ativo { "true" } else { "false" })],
        )?;
        let v = |c: &str| r.valor(0, c).and_then(|v| v.parse().ok());
        v("id")
            .zip(v("credencial"))
            .ok_or_else(|| "usuário inexistente".into())
    }

    /// Grava o hash novo (a conta cara e a conferencia da senha atual
    /// acontecem antes, fora da trava). Devolve a credencial que o gatilho
    /// deixou.
    pub fn gravar_senha(&mut self, id: i64, hash: &str) -> R<i64> {
        let r = self.pg()?.executar(
            "UPDATE phx_usuario SET senha_hash = $2 WHERE id = $1::int RETURNING credencial",
            &[Some(&id.to_string()), Some(hash)],
        )?;
        credencial_da_resposta(&r)
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

    /// O CN ainda e vinculo vivo deste usuario nesta rede? O token da VPN
    /// renova pelo motor, e membro removido nao renova (a CRL tambem barra,
    /// mas esta e a pergunta direta).
    pub fn vinculo_vale(&mut self, rede_id: &str, cn: &str, usuario_id: i64) -> R<bool> {
        let r = self.pg()?.executar(
            "SELECT 1 AS ok FROM phx_membro WHERE rede_id = $1::int AND cn = $2 AND usuario_id = $3::int",
            &[Some(rede_id), Some(cn), Some(&usuario_id.to_string())],
        )?;
        Ok(!r.linhas.is_empty())
    }

    /// (id, credencial) de todos: o retrato que a vigia compara.
    pub fn credenciais(&mut self) -> R<Vec<(i64, i64)>> {
        let r = self
            .pg()?
            .executar("SELECT id, credencial FROM phx_usuario", &[])?;
        Ok((0..r.linhas.len())
            .filter_map(|i| {
                let v = |c: &str| r.valor(i, c).and_then(|v| v.parse().ok());
                v("id").zip(v("credencial"))
            })
            .collect())
    }
}

pub(crate) fn credencial_da_resposta(r: &crate::pg::Resposta) -> R<i64> {
    r.valor(0, "credencial")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| "usuário inexistente".into())
}

impl Estado {
    /// A credencial de `usuario_id` mudou para `novo` (o valor que o
    /// `RETURNING` da mudanca devolveu). A sessao `manter` -- a de quem mudou
    /// a propria -- fica se `novo` for o esperado; as outras caem na proxima
    /// conferencia; o `ccd/` acompanha o `ativo`; e a conexao VPN do usuario
    /// cai agora, em cada rede. `Err` diz o que NAO se completou (a vigia
    /// tenta de novo): desativar que nao tirou o `ccd/` nao responde «ok».
    pub fn credencial_mudou(
        &self,
        usuario_id: i64,
        novo: i64,
        manter: Option<&str>,
    ) -> Result<(), String> {
        let (ccd, vinculos, dados) = {
            let mut p = self.painel();
            (
                p.acertar_ccd(Some(usuario_id)),
                p.vinculos_do_usuario(usuario_id),
                p.dados().to_path_buf(),
            )
        };
        if let Some(tk) = manter {
            self.gancho(Momento::AntesDeRenovar);
            self.sessoes.renovar(tk, usuario_id, novo);
        }
        let mut falhas = Vec::new();
        if let Err(m) = ccd {
            falhas.push(format!("ccd/: {m}"));
        }
        match vinculos {
            Ok(v) => {
                for (rede, cn) in v {
                    if let Err(m) = derrubar_na_rede(&dados, &rede, &cn, "credencial mudou") {
                        falhas.push(m);
                    }
                }
            }
            Err(m) => falhas.push(m),
        }
        if !falhas.is_empty() {
            let m = falhas.join("; ");
            eprintln!(
                "phxvpn: usuario {usuario_id}: mudanca incompleta ({m}); a vigia tenta de novo"
            );
            return Err(m);
        }
        if let Some(v) = self
            .vistos
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            v.insert(usuario_id, novo);
        }
        Ok(())
    }

    /// So para os testes: roda `f` uma vez, no `momento` -- as duas janelas
    /// da corrida do M3 (outra mudanca antes da gravacao da propria, ou
    /// entre ela e a renovacao da sessao).
    #[doc(hidden)]
    pub fn gancho_de_teste(&self, momento: Momento, f: impl FnOnce(&Estado) + Send + 'static) {
        *self
            .gancho_de_teste
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some((momento, Box::new(f)));
    }

    /// Roda o gancho de teste, se houver um para este momento. Fora dos
    /// testes e uma trava sem disputa e um `None`.
    pub(crate) fn gancho(&self, momento: Momento) {
        let g = {
            let mut g = self
                .gancho_de_teste
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match g.as_ref() {
                Some((m, _)) if *m == momento => g.take(),
                _ => None,
            }
        };
        if let Some((_, f)) = g {
            f(self);
        }
    }

    /// Derruba as conexoes dos certificados revogados desde a ultima vez
    /// (membro removido, saiu, reentrou com certificado novo). Quem revoga e
    /// o `Painel::revogar`, um so ponto para todos os caminhos; quem derruba
    /// e isto, chamado no fim de cada pedido e a cada volta da vigia. O que
    /// falhar volta para a fila.
    pub fn derrubar_revogados(&self) {
        let (lista, dados) = {
            let mut p = self.painel();
            if p.revogados.is_empty() {
                return;
            }
            (std::mem::take(&mut p.revogados), p.dados().to_path_buf())
        };
        let mut ficou = Vec::new();
        for (rede, cn) in lista {
            if derrubar_na_rede(&dados, &rede, &cn, "certificado revogado").is_err() {
                ficou.push((rede, cn));
            }
        }
        if !ficou.is_empty() {
            self.painel().revogados.extend(ficou);
        }
    }

    /// Uma volta da vigia: compara `(id, credencial)` do banco com o ultimo
    /// retrato e aplica `credencial_mudou` a quem mudou -- inclusive por
    /// `UPDATE` feito a mao, e inclusive o que falhou antes. A primeira volta
    /// so tira o retrato (o arranque ja reconciliou o `ccd/`).
    pub fn reconciliar(&self) {
        self.derrubar_revogados();
        let atuais = match self.painel().credenciais() {
            Ok(v) => v,
            Err(m) => {
                eprintln!("phxvpn: vigia: {m}");
                return;
            }
        };
        let mudados: Vec<(i64, i64)> = {
            let mut g = self.vistos.lock().unwrap_or_else(|e| e.into_inner());
            let Some(vistos) = g.as_mut() else {
                *g = Some(atuais.into_iter().collect());
                return;
            };
            let mut mudados = Vec::new();
            for (id, c) in atuais {
                match vistos.get(&id) {
                    Some(v) if *v != c => mudados.push((id, c)),
                    Some(_) => {}
                    // Usuario novo: nasceu agora, nada a derrubar.
                    None => {
                        vistos.insert(id, c);
                    }
                }
            }
            mudados
        };
        for (id, c) in mudados {
            let _ = self.credencial_mudou(id, c, None);
        }
    }
}

/// Liga a vigia numa thread propria, a cada [`VIGIA`].
pub fn vigiar(e: std::sync::Arc<Estado>) {
    std::thread::spawn(move || loop {
        e.reconciliar();
        std::thread::sleep(VIGIA);
    });
}

fn derrubar_na_rede(dados: &Path, rede: &str, cn: &str, motivo: &str) -> Result<(), String> {
    match derrubar(&socket(dados, rede), cn, PRAZO_GERENCIA) {
        Ok(true) => {
            eprintln!(
                "phxvpn: rede {rede}: conexao de «{}» derrubada ({motivo})",
                crate::verificar::para_log(cn)
            );
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(m) => {
            let m = format!("rede {rede}: gerencia do openvpn: {m}");
            eprintln!("phxvpn: {m}");
            Err(m)
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

/// As linhas do `servidor.conf` que ligam a gerencia -- ou nenhuma. Duas
/// portas, e sem qualquer uma delas a gerencia NAO abre (a queda fica para a
/// renegociacao, e o log diz): a pasta `gerencia/` 0700 e do dono do painel
/// (apertada sempre, mesmo se ja existia frouxa) e o `management-client-user`
/// (so o usuario do painel conversa). No Windows nao ha soquete Unix.
pub fn conf(dados: &Path, rede_id: &str) -> String {
    match preparar_gerencia(dados) {
        Ok(usuario) => format!(
            "# gerencia: o painel derruba a conexao de quem mudou de credencial\n\
             management {} unix\n\
             management-client-user {usuario}\n",
            socket(dados, rede_id).display()
        ),
        Err(m) => {
            eprintln!("phxvpn: rede {rede_id}: sem gerencia do openvpn ({m}): a conexao de quem muda cai so na renegociacao");
            String::new()
        }
    }
}

#[cfg(target_os = "linux")]
fn preparar_gerencia(dados: &Path) -> Result<String, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: sem argumentos, nao falha.
    let eu = unsafe { geteuid() };
    let usuario = nome_do_uid(
        &std::fs::read_to_string("/etc/passwd").unwrap_or_default(),
        eu,
    )
    .ok_or("o usuario do painel nao esta no /etc/passwd (sem management-client-user)")?;
    let dir = dados.join("gerencia");
    std::fs::create_dir_all(&dir).map_err(|e| format!("criar {}: {e}", dir.display()))?;
    let meta = std::fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != eu {
        return Err(format!("{} nao e pasta do dono do painel", dir.display()));
    }
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("permissao de {}: {e}", dir.display()))?;
    Ok(usuario)
}

#[cfg(not(target_os = "linux"))]
fn preparar_gerencia(_dados: &Path) -> Result<String, String> {
    Err("gerencia por soquete Unix so no Linux".into())
}

pub fn nome_do_uid(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|l| {
        let p: Vec<&str> = l.split(':').collect();
        (p.len() > 2 && p[2].parse() == Ok(uid)).then(|| p[0].to_string())
    })
}

/// Derruba as conexoes do CN pela gerencia, em no maximo `prazo` no total.
/// `Ok(true)`: derrubou; `Ok(false)`: nao estava conectado (ou a rede nao
/// tem gerencia no ar).
///
/// `client-kill CID`, e nao `kill CN`: no 2.6 o `kill` fecha a instancia no
/// servidor sem avisar o cliente (`multi_signal_instance`), e o cliente so
/// percebe no `ping-restart` (60 s aqui); o `client-kill` manda o `RESTART`
/// pelo canal de controle (`send_restart`), e o cliente reconecta na hora --
/// com o token, que cai no motor e e recusado. O CID sai do `status 2`.
#[cfg(unix)]
pub fn derrubar(sock: &Path, cn: &str, prazo: Duration) -> Result<bool, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    let fim = Instant::now() + prazo;
    let mut s = match UnixStream::connect(sock) {
        Ok(s) => s,
        // Rede sem openvpn no ar (ou sem gerencia ainda): nada a derrubar.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("{}: {e}", sock.display())),
    };
    let _ = s.set_write_timeout(Some(prazo));
    let mut leitor = BufReader::new(s.try_clone().map_err(|e| e.to_string())?);
    // Uma linha que satisfaz `fim_de`, pulando as notificacoes (`>INFO:`...),
    // que chegam a qualquer hora -- e cada leitura so espera o que sobra do
    // prazo total, para uma gerencia que pinga notificacao nao segurar a rota.
    let mut ler_ate = |fim_de: &dyn Fn(&str) -> bool| -> Result<Vec<String>, String> {
        let mut linhas = Vec::new();
        loop {
            let resta = fim.saturating_duration_since(Instant::now());
            if resta.is_zero() {
                return Err("gerencia nao respondeu no prazo".into());
            }
            let _ = leitor.get_ref().set_read_timeout(Some(resta));
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
            let acabou = fim_de(&l);
            linhas.push(l);
            if acabou {
                return Ok(linhas);
            }
        }
    };
    s.write_all(b"status 2\n")
        .map_err(|e| format!("gerencia: {e}"))?;
    let status = ler_ate(&|l| l == "END" || l.starts_with("ERROR:"))?;
    let cids = cids_do_cn(&status, cn)?;
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

/// Os `Client ID` do CN no `status 2`, pelas colunas do cabecalho
/// `HEADER,CLIENT_LIST,...` (e nao por posicao fixa: versao que acrescente
/// coluna no meio faria a posicao apontar outro campo). Sem cabecalho, ou
/// sem as colunas, erro -- nunca um palpite. So numero: o CID vai numa linha
/// de comando.
pub fn cids_do_cn(status: &[String], cn: &str) -> Result<Vec<String>, String> {
    let cab: Vec<&str> = status
        .iter()
        .find_map(|l| l.strip_prefix("HEADER,CLIENT_LIST,"))
        .ok_or("status 2 sem o cabecalho HEADER,CLIENT_LIST")?
        .split(',')
        .collect();
    let col = |nome: &str| {
        cab.iter()
            .position(|c| *c == nome)
            .ok_or(format!("status 2 sem a coluna «{nome}»"))
    };
    let (i_cn, i_cid) = (col("Common Name")?, col("Client ID")?);
    Ok(status
        .iter()
        .filter_map(|l| l.strip_prefix("CLIENT_LIST,"))
        .map(|l| l.split(',').collect::<Vec<_>>())
        .filter(|c| c.get(i_cn) == Some(&cn))
        .filter_map(|c| c.get(i_cid).map(|x| x.to_string()))
        .filter(|cid| !cid.is_empty() && cid.bytes().all(|b| b.is_ascii_digit()))
        .collect())
}

/// No Windows a gerencia nao abre (`conf` nao a liga): nada a derrubar
/// por ela, e a conexao cai na renegociacao.
#[cfg(not(unix))]
pub fn derrubar(_sock: &Path, _cn: &str, _prazo: Duration) -> Result<bool, String> {
    Ok(false)
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
        assert!(s.renovar("a", 7, 4));
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

    /// M3: renovar so com o valor esperado (o da sessao + 1, ou o mesmo).
    /// Outro valor e mudanca de outro no meio -- a sessao atual cai.
    /// RED: renovar adotando qualquer valor deixa «a» viva com 5.
    #[test]
    fn renovar_so_com_o_esperado() {
        let s = Sessoes::nova(Duration::from_secs(60));
        s.abrir("a".into(), &u(7, 3));
        assert!(s.renovar("a", 7, 3), "nada mudou: fica");
        assert!(!s.renovar("a", 7, 5), "pulou um: alguem mudou no meio");
        assert_eq!(
            s.conferir("a", |_| Ok(Some(u(7, 5)))).unwrap_err(),
            Recusa::Desconhecida
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
        assert!(!s.renovar("a", 8, 2));
        assert!(s.conferir("a", |_| Ok(Some(u(7, 1)))).is_ok());
        assert!(!s.renovar("sumida", 7, 2));
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

    /// B3: a pasta da gerencia que ja existia frouxa (0755) sai 0700, e o
    /// conf leva o `management-client-user`. RED: so criar (sem apertar)
    /// deixa 0755.
    #[cfg(target_os = "linux")]
    #[test]
    fn gerencia_aperta_a_pasta_e_exige_o_usuario() {
        use std::os::unix::fs::PermissionsExt;
        let dados = std::env::temp_dir().join(format!("phxvpn-conf-ger-{}", std::process::id()));
        let dir = dados.join("gerencia");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let c = conf(&dados, "3");
        assert!(c.contains(&format!(
            "management {}/gerencia/3.sock unix\n",
            dados.display()
        )));
        assert!(c.contains("management-client-user "), "{c}");
        let modo = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o700);
        let _ = std::fs::remove_dir_all(&dados);
    }

    fn linhas(v: &[&str]) -> Vec<String> {
        v.iter().map(|l| l.to_string()).collect()
    }

    /// B2: o CID sai pela coluna do cabecalho, nao pela posicao.
    #[test]
    fn cid_sai_do_status_2_pelo_cabecalho() {
        let st = linhas(&[
            "TITLE,OpenVPN 2.6.19",
            "HEADER,CLIENT_LIST,Common Name,Real Address,Virtual Address,Virtual IPv6 Address,Bytes Received,Bytes Sent,Connected Since,Connected Since (time_t),Username,Client ID,Peer ID,Data Channel Cipher",
            "CLIENT_LIST,ana.1.ab,192.0.2.1:5000,10.77.1.3,,10,20,2026-09-24 10:00:00,1790000000,ana,7,0,AES-256-GCM",
            "CLIENT_LIST,bia.1.cd,192.0.2.2:5000,10.77.1.4,,10,20,2026-09-24 10:00:00,1790000000,bia,8,1,AES-256-GCM",
            "CLIENT_LIST,ana.1.ab,192.0.2.1:5001,10.77.1.3,,10,20,2026-09-24 10:00:00,1790000000,ana,9;x,0,AES-256-GCM",
            "END",
        ]);
        assert_eq!(cids_do_cn(&st, "ana.1.ab").unwrap(), vec!["7"]);
        assert_eq!(cids_do_cn(&st, "bia.1.cd").unwrap(), vec!["8"]);
        assert!(cids_do_cn(&st, "ana.1").unwrap().is_empty());
        // Versao com uma coluna a mais no meio: a posicao fixa pegaria o
        // «Username»; o cabecalho acha o CID.
        let outra = linhas(&[
            "HEADER,CLIENT_LIST,Common Name,Nova,Real Address,Virtual Address,Virtual IPv6 Address,Bytes Received,Bytes Sent,Connected Since,Connected Since (time_t),Username,Client ID",
            "CLIENT_LIST,ana.1.ab,x,192.0.2.1:5000,10.77.1.3,,10,20,d,1,ana,42",
        ]);
        assert_eq!(cids_do_cn(&outra, "ana.1.ab").unwrap(), vec!["42"]);
        assert!(cids_do_cn(&linhas(&["CLIENT_LIST,ana.1.ab,a,b"]), "ana.1.ab").is_err());
    }

    /// Gerencia de mentira: responde `status 2` com o cabecalho e a ana
    /// (CID 7), `client-kill 7` com SUCCESS; devolve o que ouviu.
    #[cfg(unix)]
    fn gerencia_de_mentira(sock: &Path, conexoes: usize) -> std::thread::JoinHandle<Vec<String>> {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;
        let _ = std::fs::remove_file(sock);
        let ouvinte = UnixListener::bind(sock).unwrap();
        std::thread::spawn(move || {
            let mut pedidos = Vec::new();
            for _ in 0..conexoes {
                let (mut c, _) = ouvinte.accept().unwrap();
                c.write_all(b">INFO:OpenVPN Management Interface Version 5\r\n")
                    .unwrap();
                let mut leitor = BufReader::new(c.try_clone().unwrap());
                loop {
                    let mut l = String::new();
                    if leitor.read_line(&mut l).unwrap_or(0) == 0 {
                        break;
                    }
                    pedidos.push(l.trim().to_string());
                    let r = match l.trim() {
                        "status 2" => "HEADER,CLIENT_LIST,Common Name,Real Address,Virtual Address,Virtual IPv6 Address,Bytes Received,Bytes Sent,Connected Since,Connected Since (time_t),Username,Client ID,Peer ID,Data Channel Cipher\r\nCLIENT_LIST,ana.1.ab,192.0.2.1:5000,10.77.1.3,,1,2,d,1,ana,7,0,c\r\n>BYTECOUNT:1,2\r\nEND\r\n",
                        "client-kill 7" => "SUCCESS: client-kill command succeeded\r\n",
                        _ => break,
                    };
                    c.write_all(r.as_bytes()).unwrap();
                }
            }
            pedidos
        })
    }

    /// Pede o `status 2`, manda o `client-kill` do CID do CN (e so dele).
    #[cfg(unix)]
    #[test]
    fn derrubar_pede_o_status_e_mata_pelo_cid() {
        let dir = std::env::temp_dir().join(format!("phxvpn-gerencia-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("1.sock");
        let t = gerencia_de_mentira(&sock, 2);
        let prazo = Duration::from_secs(2);
        assert_eq!(derrubar(&sock, "ana.1.ab", prazo), Ok(true));
        assert_eq!(derrubar(&sock, "bia.1.cd", prazo), Ok(false));
        assert_eq!(
            t.join().unwrap(),
            ["status 2", "client-kill 7", "quit", "status 2", "quit"]
        );
        assert_eq!(derrubar(&dir.join("nenhum.sock"), "x", prazo), Ok(false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// B4: gerencia que so manda notificacao e nunca responde nao segura a
    /// rota alem do prazo TOTAL. RED: com prazo so por leitura, cada `>INFO`
    /// renova a espera e a chamada nao volta.
    #[cfg(unix)]
    #[test]
    fn derrubar_respeita_o_prazo_total() {
        use std::io::Write;
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("phxvpn-gerencia-muda-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let sock = dir.join("1.sock");
        let _ = std::fs::remove_file(&sock);
        let ouvinte = UnixListener::bind(&sock).unwrap();
        std::thread::spawn(move || {
            let (mut c, _) = ouvinte.accept().unwrap();
            for _ in 0..100 {
                if c.write_all(b">INFO:ainda aqui\r\n").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let t0 = Instant::now();
        let r = derrubar(&sock, "ana.1.ab", Duration::from_millis(400));
        let gasto = t0.elapsed();
        assert!(r.is_err(), "{r:?}");
        assert!(gasto < Duration::from_millis(1500), "{gasto:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
