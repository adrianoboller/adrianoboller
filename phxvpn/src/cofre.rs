//! O cofre: segredo que vai ao banco cifrado pela SENHA MESTRE.
//!
//! A chave privada da AC e a chave `tls-crypt` de cada rede vivem no
//! PostgreSQL, e quem le o banco nao pode sair emitindo certificado. Entao elas
//! vao seladas com XChaCha20-Poly1305 (do `phxsql-core`, conferido contra o
//! draft-irtf-cfrg-xchacha), com a chave derivada da senha mestre por
//! PBKDF2-HMAC-SHA256. A senha mestre NUNCA e gravada -- nem hash dela: o
//! proprio selo e a prova (etiqueta que nao confere = senha errada).
//!
//! Formato do texto guardado: `v1$iteracoes$sal$nonce$cifrado$etiqueta`, tudo
//! em hex. O sal viaja junto porque sal nao e segredo; nonce de 24 bytes
//! sorteado a cada selo, e com XChaCha sorteio nao colide na pratica.

use phxsql_core::cifra::{chave_de_senha, xabrir, xselar, CHAVE_LEN, TAG_LEN};
use phxsql_core::hash::{de_hex, para_hex};
use phxsql_core::senha::bytes_aleatorios;

/// Iteracoes do PBKDF2 da senha mestre. Alto de proposito: destrancar acontece
/// uma vez por arranque do painel, e cada iteracao a mais e custo para quem
/// tenta adivinhar a senha com o banco na mao.
pub const ITERACOES: u32 = 310_000;

/// A chave ja derivada, guardada so em memoria enquanto o painel roda.
#[derive(Clone)]
pub struct Cofre {
    chave: [u8; CHAVE_LEN],
    sal: Vec<u8>,
    iteracoes: u32,
}

impl Cofre {
    /// Cofre novo, com sal novo (instalacao).
    pub fn novo(senha_mestre: &str, iteracoes: u32) -> Cofre {
        let sal = bytes_aleatorios(16);
        Cofre {
            chave: chave_de_senha(senha_mestre, &sal, iteracoes),
            sal,
            iteracoes,
        }
    }

    /// Reabre o cofre a partir de um selo ja gravado: deriva com o sal e as
    /// iteracoes dele e prova a senha abrindo o selo.
    pub fn destrancar(senha_mestre: &str, selo_de_prova: &str) -> Result<Cofre, String> {
        let (iteracoes, sal, ..) = partes(selo_de_prova)?;
        // As iteracoes vem do banco: quem altera a linha nao pode por o painel
        // a derivar por horas no arranque.
        if !(1_000..=10_000_000).contains(&iteracoes) {
            return Err("selo com iteracoes fora da faixa (1.000 a 10.000.000)".into());
        }
        let cofre = Cofre {
            chave: chave_de_senha(senha_mestre, &sal, iteracoes),
            sal,
            iteracoes,
        };
        cofre
            .abrir(selo_de_prova)
            .map_err(|_| "senha mestre nao confere".to_string())?;
        Ok(cofre)
    }

    pub fn selar(&self, claro: &[u8], aad: &str) -> String {
        let nonce: [u8; 24] = bytes_aleatorios(24).try_into().expect("24 bytes");
        let (cifrado, tag) = xselar(&self.chave, &nonce, aad.as_bytes(), claro);
        format!(
            "v1${}${}${}${}${}",
            self.iteracoes,
            para_hex(&self.sal),
            para_hex(&nonce),
            para_hex(&cifrado),
            para_hex(&tag)
        )
    }

    /// Abre um selo. O `aad` e o proprio uso (ex.: «ac», «rede:7»), que entra na
    /// etiqueta: um selo copiado de uma coluna para outra nao abre.
    pub fn abrir_com(&self, selo: &str, aad: &str) -> Result<Vec<u8>, String> {
        let (_, _, nonce, cifrado, tag) = partes(selo)?;
        let nonce: [u8; 24] = nonce.try_into().map_err(|_| "nonce do selo torto")?;
        let tag: [u8; TAG_LEN] = tag.try_into().map_err(|_| "etiqueta do selo torta")?;
        xabrir(&self.chave, &nonce, aad.as_bytes(), &cifrado, &tag).map_err(|e| e.to_string())
    }

    fn abrir(&self, selo: &str) -> Result<Vec<u8>, String> {
        self.abrir_com(selo, AAD_PROVA)
    }
}

/// O `aad` do selo de prova da senha mestre.
pub const AAD_PROVA: &str = "phxvpn:prova";

type Partes = (u32, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

fn partes(selo: &str) -> Result<Partes, String> {
    let p: Vec<&str> = selo.split('$').collect();
    if p.len() != 6 || p[0] != "v1" {
        return Err("selo em formato desconhecido".into());
    }
    let hex = |s: &str| de_hex(s).ok_or_else(|| "selo com hex invalido".to_string());
    let it = p[1].parse().map_err(|_| "iteracoes do selo invalidas")?;
    Ok((it, hex(p[2])?, hex(p[3])?, hex(p[4])?, hex(p[5])?))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn senha_certa_abre_e_errada_nao() {
        let c = Cofre::novo("mestre-forte", 1_000);
        let prova = c.selar(b"ok", AAD_PROVA);
        let segredo = c.selar(b"chave da AC", "ac");
        let c2 = Cofre::destrancar("mestre-forte", &prova).unwrap();
        assert_eq!(c2.abrir_com(&segredo, "ac").unwrap(), b"chave da AC");
        assert!(Cofre::destrancar("mestre-fraca", &prova).is_err());
    }

    #[test]
    fn selo_trocado_de_uso_nao_abre() {
        let c = Cofre::novo("m", 1_000);
        let s = c.selar(b"x", "rede:1");
        assert!(c.abrir_com(&s, "rede:2").is_err());
    }

    #[test]
    fn senha_mestre_nao_aparece_no_selo() {
        let c = Cofre::novo("senha-que-nao-pode-vazar", 1_000);
        let s = c.selar(b"x", "ac");
        assert!(!s.contains("senha-que-nao-pode-vazar"));
        assert!(!s.contains(&para_hex(b"senha-que-nao-pode-vazar")));
    }
}
