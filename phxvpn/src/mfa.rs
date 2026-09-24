//! O segundo fator (TOTP) no cadastro: segredo por usuario, exigencia por
//! rede. As contas do codigo moram em `totp.rs`; aqui fica o que toca o banco.
//!
//! # Onde o segredo mora, e por que NAO no cofre da senha mestre
//!
//! O segredo TOTP e da familia da senha: quem o tem gera o codigo. Vai ao
//! banco SELADO (XChaCha20-Poly1305, o mesmo selo do cofre), com o id do
//! usuario no dado associado -- selo copiado para outra linha nao abre.
//!
//! A chave do selo NAO e a da senha mestre, e isso e decisao: o login do
//! painel acontece ANTES de destrancar o cofre (o admin entra para digitar a
//! senha mestre). Selado pela mestre, o admin com MFA nunca mais entraria num
//! painel recem-ligado. A chave e um arquivo do painel (`mfa.chave`, 0600, na
//! pasta de dados, ao lado das chaves de servidor que o OpenVPN ja le em
//! claro): quem leva so o dump do banco nao gera codigo.
//!
//! O segredo so sai numa resposta, a do `iniciar` do cadastro (e o QR que o
//! telefone le). Depois de confirmado, nenhuma rota o devolve.
//!
//! # Reuso
//!
//! `totp_ultimo` guarda o ultimo passo aceito, no banco: o mesmo codigo nao
//! vale duas vezes nem entre o painel e a VPN, nem depois de reiniciar. A
//! gravacao e condicional (`totp_ultimo < passo`), entao duas conferencias
//! simultaneas do mesmo codigo nao passam as duas.

use crate::cofre::Cofre;
use crate::painel::{Painel, Usuario};
use crate::pg::R;
use crate::totp;
use phxsql_core::json::Json;
use std::path::{Path, PathBuf};

/// Colunas novas em bancos ja instalados: `ADD COLUMN IF NOT EXISTS` e
/// idempotente e roda a cada abertura, como o resto do esquema.
pub const ESQUEMA: &str = "
ALTER TABLE phx_usuario ADD COLUMN IF NOT EXISTS totp_selado text;
ALTER TABLE phx_usuario ADD COLUMN IF NOT EXISTS totp_pendente text;
ALTER TABLE phx_usuario ADD COLUMN IF NOT EXISTS totp_ultimo bigint NOT NULL DEFAULT 0;
ALTER TABLE phx_rede ADD COLUMN IF NOT EXISTS exige_mfa boolean NOT NULL DEFAULT false;
";

const ARQUIVO_CHAVE: &str = "mfa.chave";

/// A chave do selo do segredo TOTP; nasce na primeira vez, 0600.
fn chave(dados: &Path) -> R<Cofre> {
    let caminho = dados.join(ARQUIVO_CHAVE);
    let bytes = match std::fs::read(&caminho) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            use std::io::Write;
            let nova = phxsql_core::senha::bytes_aleatorios(32);
            let mut o = std::fs::OpenOptions::new();
            o.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
            let mut f = o
                .open(&caminho)
                .map_err(|e| format!("criar {}: {e}", caminho.display()))?;
            crate::acl::so_do_dono(&caminho)?;
            f.write_all(&nova)
                .and_then(|_| f.sync_all())
                .map_err(|e| format!("gravar {}: {e}", caminho.display()))?;
            nova
        }
        Err(e) => return Err(format!("ler {}: {e}", caminho.display())),
    };
    let k: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{} torta (tem de ter 32 bytes)", caminho.display()))?;
    Ok(Cofre::de_chave(k))
}

fn aad(usuario_id: i64, pendente: bool) -> String {
    if pendente {
        format!("totp-pendente:{usuario_id}")
    } else {
        format!("totp:{usuario_id}")
    }
}

impl Painel {
    /// O usuario ja confirmou um autenticador?
    pub fn mfa_ativo(&mut self, usuario_id: i64) -> R<bool> {
        let r = self.pg()?.executar(
            "SELECT totp_selado IS NOT NULL AS ativo FROM phx_usuario WHERE id = $1::int",
            &[Some(&usuario_id.to_string())],
        )?;
        Ok(r.valor(0, "ativo") == Some("t"))
    }

    /// Comeca o cadastro: sorteia o segredo, guarda selado como PENDENTE e
    /// devolve segredo, URI e QR -- a unica resposta que os leva. Com um
    /// autenticador ja ativo, recusa: trocar exige desativar (com codigo).
    pub fn mfa_iniciar(&mut self, u: &Usuario) -> R<Json> {
        if self.mfa_ativo(u.id)? {
            return Err(
                "o autenticador já está ativo; desative-o (com um código) antes de cadastrar outro"
                    .into(),
            );
        }
        let segredo = phxsql_core::senha::bytes_aleatorios(totp::SEGREDO_LEN);
        let selado = chave(self.dados())?.selar(&segredo, &aad(u.id, true));
        self.pg()?.executar(
            "UPDATE phx_usuario SET totp_pendente = $2 WHERE id = $1::int",
            &[Some(&u.id.to_string()), Some(&selado)],
        )?;
        let r = self
            .pg()?
            .executar("SELECT nome FROM phx_empresa LIMIT 1", &[])?;
        let emissor = format!("phxvpn {}", r.valor(0, "nome").unwrap_or_default());
        let uri = totp::uri(&emissor, &u.login, &segredo);
        Ok(Json::objeto(vec![
            (
                "segredo",
                Json::texto_de(totp::base32_sem_preenchimento(&segredo)),
            ),
            ("qr_svg", Json::texto_de(totp::qr_svg(&uri)?)),
            ("uri", Json::texto_de(uri)),
        ]))
    }

    /// Termina o cadastro com um codigo do aplicativo: prova que o telefone
    /// leu o segredo certo ANTES de a conta passar a exigi-lo.
    pub fn mfa_confirmar(&mut self, u: &Usuario, codigo: &str) -> R<()> {
        let id = u.id.to_string();
        let r = self.pg()?.executar(
            "SELECT totp_pendente FROM phx_usuario WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let pendente = r
            .valor(0, "totp_pendente")
            .ok_or("comece o cadastro do autenticador antes de confirmar")?
            .to_string();
        let k = chave(self.dados())?;
        let segredo = k.abrir_com(&pendente, &aad(u.id, true))?;
        let passo = totp::conferir(&segredo, codigo, totp::agora(), 0)
            .ok_or("código do autenticador não confere")?;
        let selado = k.selar(&segredo, &aad(u.id, false));
        self.pg()?.executar(
            "UPDATE phx_usuario SET totp_selado = $2, totp_pendente = NULL, totp_ultimo = $3::bigint \
             WHERE id = $1::int",
            &[Some(&id), Some(&selado), Some(&passo.to_string())],
        )?;
        Ok(())
    }

    /// Confere o codigo de quem ja tem autenticador e grava o passo: o mesmo
    /// codigo nao passa de novo. Sem autenticador cadastrado, recusa.
    pub fn mfa_conferir(&mut self, usuario_id: i64, codigo: &str) -> R<()> {
        let id = usuario_id.to_string();
        let r = self.pg()?.executar(
            "SELECT totp_selado, totp_ultimo FROM phx_usuario WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let selado = r
            .valor(0, "totp_selado")
            .ok_or("o autenticador não está cadastrado")?
            .to_string();
        let ultimo: u64 = r
            .valor(0, "totp_ultimo")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let segredo = chave(self.dados())?.abrir_com(&selado, &aad(usuario_id, false))?;
        let passo = totp::conferir(&segredo, codigo, totp::agora(), ultimo)
            .ok_or("código do autenticador não confere")?;
        // Condicional: quem chegou junto com o mesmo codigo perde aqui.
        let gravou = self.pg()?.executar(
            "UPDATE phx_usuario SET totp_ultimo = $2::bigint \
             WHERE id = $1::int AND totp_ultimo < $2::bigint RETURNING id",
            &[Some(&id), Some(&passo.to_string())],
        )?;
        if gravou.linhas.is_empty() {
            return Err("código do autenticador não confere".into());
        }
        Ok(())
    }

    /// O proprio usuario desliga o autenticador -- com um codigo, para uma
    /// sessao roubada nao conseguir tirar o segundo fator.
    pub fn mfa_desativar(&mut self, u: &Usuario, codigo: &str) -> R<()> {
        self.mfa_conferir(u.id, codigo)?;
        self.mfa_zerar_id(u.id)
    }

    /// O administrador zera o autenticador de quem perdeu o telefone.
    pub fn mfa_zerar(&mut self, ator: &Usuario, login: &str) -> R<()> {
        if !ator.admin {
            return Err("só o administrador zera o autenticador de outro usuário".into());
        }
        let r = self.pg()?.executar(
            "SELECT id FROM phx_usuario WHERE login = $1",
            &[Some(login)],
        )?;
        let id: i64 = r
            .valor(0, "id")
            .and_then(|v| v.parse().ok())
            .ok_or("usuário inexistente")?;
        self.mfa_zerar_id(id)
    }

    fn mfa_zerar_id(&mut self, id: i64) -> R<()> {
        self.pg()?.executar(
            "UPDATE phx_usuario SET totp_selado = NULL, totp_pendente = NULL WHERE id = $1::int",
            &[Some(&id.to_string())],
        )?;
        Ok(())
    }

    pub fn rede_exige_mfa(&mut self, rede_id: &str) -> R<bool> {
        let r = self.pg()?.executar(
            "SELECT exige_mfa FROM phx_rede WHERE id = $1::int",
            &[Some(rede_id)],
        )?;
        Ok(r.valor(0, "exige_mfa") == Some("t"))
    }

    /// No «entrar»: a rede exige o autenticador? Exigindo, quem ainda nao
    /// cadastrou e recusado AQUI, com o motivo -- e nao depois, com um
    /// AUTH_FAILED mudo no cliente OpenVPN.
    pub(crate) fn mfa_da_rede(&mut self, rede_id: &str, u: &Usuario) -> R<bool> {
        if !self.rede_exige_mfa(rede_id)? {
            return Ok(false);
        }
        if !self.mfa_ativo(u.id)? {
            return Err(
                "esta rede exige o autenticador: cadastre-o em «Autenticador» antes de entrar"
                    .into(),
            );
        }
        Ok(true)
    }

    /// O dono da rede ou o administrador liga/desliga a exigencia. Os
    /// perfis baixados antes nao pedem codigo: quem liga manda os membros
    /// baixarem de novo (a resposta da API diz isso).
    pub fn rede_definir_mfa(
        &mut self,
        ator: &Usuario,
        rede_id: i64,
        exige: bool,
    ) -> R<(String, PathBuf)> {
        let id = rede_id.to_string();
        let r = self.pg()?.executar(
            "SELECT dono_id, nome FROM phx_rede WHERE id = $1::int",
            &[Some(&id)],
        )?;
        let dono: i64 = r
            .valor(0, "dono_id")
            .and_then(|v| v.parse().ok())
            .ok_or("rede inexistente")?;
        if !ator.admin && ator.id != dono {
            return Err(
                "só o administrador ou o dono da rede muda a exigência do autenticador".into(),
            );
        }
        let nome = r.valor(0, "nome").unwrap_or_default().to_string();
        self.pg()?.executar(
            "UPDATE phx_rede SET exige_mfa = $2::boolean WHERE id = $1::int",
            &[Some(&id), Some(if exige { "true" } else { "false" })],
        )?;
        let dir = self.materializar_rede(&id)?;
        Ok((nome, dir))
    }
}
