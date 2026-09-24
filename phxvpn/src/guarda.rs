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
        contar(&mut contas, chave);
    }

    /// Reserva a tentativa ANTES do trabalho caro: confere e CONTA como
    /// falha, tudo sob a mesma trava. Conferir antes e contar so depois do
    /// PBKDF2 (~430 ms) deixava a janela aberta: 256 pedidos simultaneos
    /// passavam todos pelo `antes`, porque nenhum tinha falhado AINDA. Quem
    /// acerta chama [`Reserva::acertou`]; quem erra so deixa a reserva cair
    /// (a falha ja esta contada).
    pub fn reservar(&self, chaves: &[&str]) -> Result<Reserva<'_>, Duration> {
        let mut contas = self.contas.lock().unwrap_or_else(|e| e.into_inner());
        let agora = Instant::now();
        let espera = chaves
            .iter()
            .filter_map(|c| contas.get(*c).and_then(|x| x.bloqueado_ate))
            .filter(|ate| *ate > agora)
            .map(|ate| ate - agora)
            .max();
        if let Some(falta) = espera {
            return Err(falta);
        }
        for c in chaves {
            contar(&mut contas, c);
        }
        Ok(Reserva {
            limitador: self,
            chaves: chaves.iter().map(|c| c.to_string()).collect(),
        })
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

fn contar(contas: &mut HashMap<String, Conta>, chave: &str) {
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

/// Uma tentativa ja contada como falha, esperando o veredito.
pub struct Reserva<'a> {
    limitador: &'a Limitador,
    chaves: Vec<String>,
}

impl Reserva<'_> {
    /// Acertou: as chaves em `zerar` (a da conta) voltam a zero; as outras
    /// (o IP) so devolvem esta tentativa -- quem acerta nao limpa o que
    /// outros erraram do mesmo IP.
    pub fn acertou(self, zerar: &[&str]) {
        let mut contas = self
            .limitador
            .contas
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for c in &self.chaves {
            if zerar.contains(&c.as_str()) {
                contas.remove(c);
            } else {
                devolver(&mut contas, c);
            }
        }
    }

    /// Nao foi tentativa de senha (o banco caiu, por exemplo): desfaz.
    pub fn devolver(self) {
        let mut contas = self
            .limitador
            .contas
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for c in &self.chaves {
            devolver(&mut contas, c);
        }
    }
}

fn devolver(contas: &mut HashMap<String, Conta>, chave: &str) {
    if let Some(c) = contas.get_mut(chave) {
        c.falhas = c.falhas.saturating_sub(1);
        if c.falhas <= LIVRES {
            c.bloqueado_ate = None;
        }
        if c.falhas == 0 {
            contas.remove(chave);
        }
    }
}

/// A chave de IP no limitador, com o prefixo do canal. IPv6 conta por /64:
/// quem tem um /64 (o normal de qualquer assinante) troca de endereco a cada
/// tentativa sem custo, e cada endereco seria uma conta nova.
pub fn chave_de_ip(canal: &str, ip: &str) -> String {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V6(v6)) if v6.to_ipv4_mapped().is_none() => {
            let s = v6.segments();
            format!("{canal}:{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
        Ok(std::net::IpAddr::V6(v6)) => {
            format!("{canal}:{}", v6.to_ipv4_mapped().expect("mapeado"))
        }
        Ok(v4) => format!("{canal}:{v4}"),
        Err(_) => format!("{canal}:{ip}"),
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

    /// ALTO 1 da revisao: 64 tentativas SIMULTANEAS, cada uma com 50 ms de
    /// «PBKDF2» no meio. So LIVRES+1 podem chegar a conferir; o resto e 429.
    /// RED: com `antes` + `falhou` depois do trabalho, passavam as 64.
    #[test]
    fn reserva_segura_tentativas_simultaneas() {
        let l = std::sync::Arc::new(Limitador::default());
        let passaram = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let barreira = std::sync::Arc::new(std::sync::Barrier::new(64));
        let fios: Vec<_> = (0..64)
            .map(|_| {
                let (l, passaram, barreira) = (l.clone(), passaram.clone(), barreira.clone());
                std::thread::spawn(move || {
                    barreira.wait();
                    if let Ok(r) = l.reservar(&["conta:ana", "ip-painel:x"]) {
                        passaram.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(50));
                        drop(r); // errou
                    }
                })
            })
            .collect();
        for f in fios {
            f.join().unwrap();
        }
        assert_eq!(
            passaram.load(std::sync::atomic::Ordering::SeqCst),
            LIVRES + 1
        );
    }

    /// B5: o mesmo /64 e UMA chave; /64 diferentes, chaves diferentes.
    /// RED: com o endereco inteiro, cada troca de sufixo zerava a conta.
    #[test]
    fn ipv6_conta_por_64() {
        let a = chave_de_ip("ip-painel", "2001:db8:1:2::1");
        assert_eq!(a, chave_de_ip("ip-painel", "2001:db8:1:2:ffff:1:2:3"));
        assert_eq!(a, "ip-painel:2001:db8:1:2::/64");
        assert_ne!(a, chave_de_ip("ip-painel", "2001:db8:1:3::1"));
        assert_eq!(chave_de_ip("ip-vpn", "192.0.2.7"), "ip-vpn:192.0.2.7");
        assert_eq!(
            chave_de_ip("ip-vpn", "::ffff:192.0.2.7"),
            "ip-vpn:192.0.2.7"
        );
    }

    #[test]
    fn reserva_que_acerta_nao_bloqueia_e_devolve_o_ip() {
        let l = Limitador::default();
        for _ in 0..(LIVRES * 3) {
            l.reservar(&["conta:ana", "ip:x"])
                .unwrap()
                .acertou(&["conta:ana"]);
        }
        assert!(l.antes("ip:x").is_ok() && l.antes("conta:ana").is_ok());
        // Erros de outra conta no mesmo IP continuam contados.
        for _ in 0..LIVRES {
            drop(l.reservar(&["conta:bia", "ip:x"]).unwrap());
        }
        l.reservar(&["conta:ana", "ip:x"])
            .unwrap()
            .acertou(&["conta:ana"]);
        drop(l.reservar(&["conta:bia", "ip:x"]).unwrap());
        assert!(l.antes("ip:x").is_err(), "acerto da ana nao limpa o IP");
    }
}
