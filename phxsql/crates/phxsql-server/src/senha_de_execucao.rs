//! Pedido 767, fatia P14: o cadastro da SENHA DE EXECUCAO.
//!
//! Decisoes do dono (09/10/2026), e o que cada uma pede daqui:
//!
//! * «Nao e a senha do usuario, e uma SEGUNDA senha» -- cadastro a parte, com
//!   PBKDF2 e sal proprios ([`phxsql_core::senha`], o mesmo motor da senha de
//!   login), recusada se igual a de login, troca e bloqueio por tentativas
//!   proprios;
//! * «uma vez informada, fica na sessao» -- quem libera a sessao e o servidor
//!   (`Servidor::op_liberar_execucao`); aqui so se confere e se conta.
//!
//! # Por que um ARQUIVO a parte, e nao um campo no `config.json`
//!
//! Tres motivos medidos contra este servidor:
//!
//! 1. o `usuario_alterar` reescreve o objeto do usuario a partir do pedido,
//!    e a `ficha` do usuario e devolvida pela tela -- ha teste que reprova a
//!    ficha que vaza o hash da senha de LOGIN; um segundo hash no mesmo
//!    objeto seria mais um campo para cada um desses caminhos esquecer;
//! 2. o contador de falhas e o bloqueio MUDAM a cada tentativa errada, e
//!    regravar o `config.json` inteiro (comentarios e tudo) a cada senha
//!    errada seria trocar a configuracao por causa de um erro de digitacao;
//! 3. o arquivo nasce 0600, pela mesma troca atomica do `jobs.json` e da chave
//!    do fio (`config::gravar_privado`).
//!
//! Mora ao lado do `acessos.log`, como o `diretivas.log`: o mesmo lugar que
//! quem administra ja olha, sem um campo novo de configuracao.
//!
//! # O que nunca sai daqui
//!
//! A senha. Nem o hash sai: nao ha `Debug` nos registros, nao ha op que
//! devolva o arquivo, e as respostas dizem so «liberada», «bloqueada ate» e
//! quantas tentativas restam.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

/// Falhas seguidas que bloqueiam. O mesmo numero da politica leve de login
/// da casa (`seguranca.tentativas_ate_bloquear`, 5) -- uma regua so para
/// «quantas vezes alguem pode errar a senha».
pub const TENTATIVAS_ATE_BLOQUEAR: u32 = 5;

/// Quanto dura o bloqueio: 15 minutos. Curto o bastante para o dono da
/// senha que errou digitando nao ficar o dia sem trabalhar; longo o bastante
/// para tornar a adivinhacao inviavel -- 5 tentativas a cada 15 minutos, cada
/// uma pagando um PBKDF2 de 210.000 voltas.
pub const BLOQUEIO_MS: i64 = 15 * 60 * 1000;

/// O menor tamanho da senha de execucao, em bytes. Ela protege o comando que
/// apaga o banco, e e digitada poucas vezes por sessao: oito e o piso que o
/// NIST SP 800-63B pede para segredo escolhido pelo usuario.
pub const TAMANHO_MINIMO: usize = 8;

/// A identidade da sessao sem login -- o token de servico de um servidor sem
/// cadastro. Ela tambem pode ter senha de execucao: sem isso, um servidor sem
/// usuarios nunca mais apagaria tabela nenhuma.
pub const IDENTIDADE_DO_SERVICO: &str = "(token de servico)";

/// Um registro do cadastro. SEM `Debug` de proposito: carrega o hash.
#[derive(Clone)]
struct Registro {
    login: String,
    hash: String,
    falhas: u32,
    bloqueado_ate_ms: i64,
    definida_ms: i64,
    definida_por: String,
}

impl Registro {
    fn de_json(j: &Json) -> Option<Registro> {
        let login = j.texto_ou("login", "").to_string();
        let hash = j.texto_ou("hash", "").to_string();
        if login.is_empty() || !phxsql_core::senha::e_hash(&hash) {
            return None;
        }
        Some(Registro {
            login,
            hash,
            falhas: j.inteiro_ou("falhas", 0).clamp(0, i64::from(u32::MAX)) as u32,
            bloqueado_ate_ms: j.inteiro_ou("bloqueado_ate_ms", 0),
            definida_ms: j.inteiro_ou("definida_ms", 0),
            definida_por: j.texto_ou("definida_por", "").to_string(),
        })
    }

    fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("login", Json::texto_de(&self.login)),
            ("hash", Json::texto_de(&self.hash)),
            ("falhas", Json::de_u64(u64::from(self.falhas))),
            ("bloqueado_ate_ms", Json::de_i64(self.bloqueado_ate_ms)),
            ("definida_ms", Json::de_i64(self.definida_ms)),
            ("definida_por", Json::texto_de(&self.definida_por)),
        ])
    }
}

/// O que a conferencia achou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conferencia {
    /// A senha confere.
    Confere,
    /// Nao ha senha de execucao cadastrada para esta identidade.
    NaoCadastrada,
    /// Bloqueada por tentativas, ate este instante.
    Bloqueada { ate_ms: i64 },
    /// Nao confere. `restantes` ate o bloqueio; zero quando esta falha
    /// acabou de bloquear.
    NaoConfere { restantes: u32 },
}

/// O cadastro das senhas de execucao, num arquivo.
pub struct Cofre {
    caminho: PathBuf,
    /// Uma leitura-modificacao-gravacao por vez. Nunca segurada durante o
    /// PBKDF2: a conta acontece fora, com o hash copiado.
    trava: Mutex<()>,
}

impl Cofre {
    /// O cofre ao lado do `acessos.log` -- o molde do `diretivas.log`.
    pub fn ao_lado_de(log_acessos: &Path) -> Cofre {
        let caminho = match log_acessos.parent().filter(|d| !d.as_os_str().is_empty()) {
            Some(dir) => dir.join("senhas-de-execucao.json"),
            None => PathBuf::from("senhas-de-execucao.json"),
        };
        Cofre {
            caminho,
            trava: Mutex::new(()),
        }
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    fn travar(&self) -> std::sync::MutexGuard<'_, ()> {
        self.trava
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Le o arquivo. Ausente = cadastro vazio; ilegivel = erro, e nunca
    /// «vazio»: tratar arquivo estragado como vazio deixaria qualquer um
    /// cadastrar de novo a senha de quem ja tinha uma.
    fn ler(&self) -> Result<Vec<Registro>> {
        let texto = match std::fs::read_to_string(&self.caminho) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(PhxError::Io(e)),
        };
        let j = Json::analisar(&texto).map_err(|_| {
            PhxError::Corrompido(format!(
                "{} nao e JSON: o cadastro das senhas de execucao nao se le, e \
                 nenhuma senha de execucao confere ate ele ser consertado",
                self.caminho.display()
            ))
        })?;
        Ok(j.campo("senhas")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .filter_map(Registro::de_json)
            .collect())
    }

    fn gravar(&self, registros: &[Registro]) -> Result<()> {
        let j = Json::objeto(vec![(
            "senhas",
            Json::Lista(registros.iter().map(Registro::para_json).collect()),
        )]);
        if let Some(pai) = self.caminho.parent() {
            if !pai.as_os_str().is_empty() {
                phxsql_store::permissao::criar_diretorio_do_banco(pai)?;
            }
        }
        crate::config::gravar_privado(&self.caminho, j.escrever_identado().as_bytes())?;
        Ok(())
    }

    /// Ha senha cadastrada para esta identidade?
    pub fn tem(&self, login: &str) -> Result<bool> {
        let _t = self.travar();
        Ok(self.ler()?.iter().any(|r| r.login == login))
    }

    /// Quem tem senha cadastrada. So os logins -- e o que a regra do
    /// primeiro cadastro precisa saber («algum administrador ja tem a dele?»).
    pub fn logins(&self) -> Result<Vec<String>> {
        let _t = self.travar();
        Ok(self.ler()?.into_iter().map(|r| r.login).collect())
    }

    /// Confere a senha, e conta: a falha soma, a quinta seguida bloqueia, o
    /// acerto zera. O PBKDF2 roda FORA da trava.
    pub fn conferir(&self, login: &str, senha: &str, agora_ms: i64) -> Result<Conferencia> {
        let hash = {
            let _t = self.travar();
            let registros = self.ler()?;
            let Some(r) = registros.iter().find(|r| r.login == login) else {
                return Ok(Conferencia::NaoCadastrada);
            };
            if r.bloqueado_ate_ms > agora_ms {
                return Ok(Conferencia::Bloqueada {
                    ate_ms: r.bloqueado_ate_ms,
                });
            }
            r.hash.clone()
        };
        let confere = phxsql_core::senha::conferir(senha, &hash);
        let _t = self.travar();
        let mut registros = self.ler()?;
        let Some(r) = registros.iter_mut().find(|r| r.login == login) else {
            return Ok(Conferencia::NaoCadastrada);
        };
        let saida = if confere {
            r.falhas = 0;
            r.bloqueado_ate_ms = 0;
            Conferencia::Confere
        } else {
            r.falhas = r.falhas.saturating_add(1);
            if r.falhas >= TENTATIVAS_ATE_BLOQUEAR {
                r.falhas = 0;
                r.bloqueado_ate_ms = agora_ms.saturating_add(BLOQUEIO_MS);
                Conferencia::NaoConfere { restantes: 0 }
            } else {
                Conferencia::NaoConfere {
                    restantes: TENTATIVAS_ATE_BLOQUEAR - r.falhas,
                }
            }
        };
        self.gravar(&registros)?;
        Ok(saida)
    }

    /// Grava o hash de uma senha nova -- o primeiro cadastro, a troca, ou a
    /// redefinicao pelo administrador. Zera falhas e bloqueio.
    pub fn definir(&self, login: &str, hash: String, por: &str, agora_ms: i64) -> Result<()> {
        if !phxsql_core::senha::e_hash(&hash) {
            return Err(PhxError::Esquema(
                "a senha de execucao so se guarda como hash PBKDF2".into(),
            ));
        }
        let _t = self.travar();
        let mut registros = self.ler()?;
        let novo = Registro {
            login: login.to_string(),
            hash,
            falhas: 0,
            bloqueado_ate_ms: 0,
            definida_ms: agora_ms,
            definida_por: por.to_string(),
        };
        match registros.iter_mut().find(|r| r.login == login) {
            Some(r) => *r = novo,
            None => registros.push(novo),
        }
        self.gravar(&registros)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use crate::apoio_teste::DirTemp;

    fn cofre(nome: &str) -> (Cofre, DirTemp) {
        let dir = DirTemp::novo(&format!("senha-exec-{nome}"));
        (Cofre::ao_lado_de(&dir.join("acessos.log")), dir)
    }

    #[test]
    fn confere_conta_e_bloqueia_na_quinta() {
        let (c, _dir) = cofre("conta");
        assert_eq!(
            c.conferir("ana", "x", 0).unwrap(),
            Conferencia::NaoCadastrada
        );
        c.definir(
            "ana",
            phxsql_core::senha::cifrar_com("certa-123", 8),
            "ana",
            1,
        )
        .unwrap();
        assert!(c.tem("ana").unwrap());
        for restam in (1..TENTATIVAS_ATE_BLOQUEAR).rev() {
            assert_eq!(
                c.conferir("ana", "errada", 10).unwrap(),
                Conferencia::NaoConfere { restantes: restam }
            );
        }
        assert_eq!(
            c.conferir("ana", "errada", 10).unwrap(),
            Conferencia::NaoConfere { restantes: 0 }
        );
        // Bloqueada: nem a certa passa ate o prazo.
        assert_eq!(
            c.conferir("ana", "certa-123", 20).unwrap(),
            Conferencia::Bloqueada {
                ate_ms: 10 + BLOQUEIO_MS
            }
        );
        assert_eq!(
            c.conferir("ana", "certa-123", 11 + BLOQUEIO_MS).unwrap(),
            Conferencia::Confere
        );
    }

    /// O arquivo nao tem a senha, so o hash; e nasce 0600.
    #[test]
    fn o_arquivo_guarda_o_hash_e_nasce_fechado() {
        let (c, _dir) = cofre("arquivo");
        c.definir(
            "ana",
            phxsql_core::senha::cifrar_com("a-senha-de-execucao", 8),
            "ana",
            1,
        )
        .unwrap();
        let bytes = std::fs::read(c.caminho()).unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        assert!(!texto.contains("a-senha-de-execucao"));
        assert!(texto.contains("pbkdf2-sha256$"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(c.caminho()).unwrap().permissions().mode();
            assert_eq!(modo & 0o077, 0, "o arquivo nasceu aberto: {modo:o}");
        }
    }

    /// Arquivo estragado nao vira «vazio»: recusa.
    #[test]
    fn arquivo_estragado_recusa_em_vez_de_esvaziar() {
        let (c, _dir) = cofre("estragado");
        std::fs::write(c.caminho(), b"{nao e json").unwrap();
        assert!(c.conferir("ana", "x", 0).is_err());
        assert!(c.tem("ana").is_err());
    }
}
