//! Limite de tentativas de senha (achados A2 e A3 da revisao de seguranca).
//!
//! Cada chave (um login, um IP, o par usuario+rede) tem uma conta de falhas.
//! As primeiras `LIVRES` passam sem espera -- quem erra a senha uma ou duas
//! vezes nao sente nada. Dali em diante cada falha dobra o bloqueio, ate
//! `TETO`: adivinhar online deixa de ser questao de horas e vira de anos.
//! Acertar zera a conta; ficar `ESQUECER` sem errar tambem.
//!
//! # Por que por login E por IP
//!
//! So por login, o atacante troca de login a cada tentativa (senha comum
//! contra muitos usuarios). So por IP, ele troca de IP (e a vitima de um
//! bloqueio por login e a pessoa certa, nao o atacante). As duas contas juntas
//! fecham os dois lados. O preco conhecido: o dono do login atacado espera o
//! bloqueio tambem -- e e melhor esperar que ter a senha adivinhada.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Falhas que nao bloqueiam.
pub const LIVRES: u32 = 5;
/// Bloqueio maximo depois de muitas falhas.
pub const TETO: Duration = Duration::from_secs(15 * 60);
/// Sem falha nesse prazo, a conta volta a zero.
pub const ESQUECER: Duration = Duration::from_secs(60 * 60);
/// Chaves guardadas no maximo (memoria nao cresce sem fim sob ataque).
const TETO_CHAVES: usize = 100_000;

struct Conta {
    falhas: u32,
    ultima: Instant,
    bloqueado_ate: Option<Instant>,
}

#[derive(Default)]
pub struct Limitador {
    contas: Mutex<HashMap<String, Conta>>,
}

impl Limitador {
    /// Pode tentar agora? `Err` traz quanto falta.
    pub fn antes(&self, chave: &str) -> Result<(), Duration> {
        let contas = self.contas.lock().unwrap_or_else(|e| e.into_inner());
        match contas.get(chave).and_then(|c| c.bloqueado_ate) {
            Some(ate) if ate > Instant::now() => Err(ate - Instant::now()),
            _ => Ok(()),
        }
    }

    pub fn falhou(&self, chave: &str) {
        let mut contas = self.contas.lock().unwrap_or_else(|e| e.into_inner());
        if contas.len() >= TETO_CHAVES {
            contas.retain(|_, c| c.ultima.elapsed() < ESQUECER);
        }
        let c = contas.entry(chave.to_string()).or_insert(Conta {
            falhas: 0,
            ultima: Instant::now(),
            bloqueado_ate: None,
        });
        if c.ultima.elapsed() >= ESQUECER {
            c.falhas = 0;
        }
        c.falhas += 1;
        c.ultima = Instant::now();
        if c.falhas > LIVRES {
            let expoente = (c.falhas - LIVRES - 1).min(16);
            let espera = Duration::from_secs(1u64 << expoente).min(TETO);
            c.bloqueado_ate = Some(Instant::now() + espera);
        }
    }

    pub fn acertou(&self, chave: &str) {
        self.contas
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(chave);
    }

    /// Confere todas as chaves antes; `Err` com a maior espera.
    pub fn antes_de_todas(&self, chaves: &[&str]) -> Result<(), Duration> {
        chaves
            .iter()
            .filter_map(|c| self.antes(c).err())
            .max()
            .map_or(Ok(()), Err)
    }
}

/// A frase que vai para quem foi bloqueado: o tempo, e nada sobre o motivo.
pub fn frase_de_bloqueio(falta: Duration) -> String {
    format!(
        "muitas tentativas erradas: tente de novo em {} s",
        falta.as_secs().max(1)
    )
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn livres_passam_e_depois_bloqueia() {
        let l = Limitador::default();
        for _ in 0..LIVRES {
            assert!(l.antes("ana").is_ok());
            l.falhou("ana");
        }
        assert!(l.antes("ana").is_ok(), "as primeiras falhas nao bloqueiam");
        l.falhou("ana");
        assert!(l.antes("ana").is_err(), "a falha seguinte bloqueia");
        assert!(l.antes("bia").is_ok(), "conta de outra chave nao e afetada");
    }

    #[test]
    fn bloqueio_dobra_ate_o_teto() {
        let l = Limitador::default();
        for _ in 0..(LIVRES + 30) {
            l.falhou("x");
        }
        let falta = l.antes("x").unwrap_err();
        assert!(falta <= TETO && falta > TETO - Duration::from_secs(5));
    }

    #[test]
    fn acertar_zera() {
        let l = Limitador::default();
        for _ in 0..(LIVRES + 1) {
            l.falhou("x");
        }
        l.acertou("x");
        assert!(l.antes("x").is_ok());
        assert!(l.antes_de_todas(&["x", "y"]).is_ok());
    }
}
