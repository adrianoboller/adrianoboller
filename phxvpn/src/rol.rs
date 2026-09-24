//! O rol ASSINADO da rede P2P: quem e membro, dito por quem criou a rede.
//!
//! # Por que existe
//!
//! Sem ele, a lista de pares viaja cifrada entre membros com confianca
//! transitiva: quem tem a senha da rede apresenta quem quiser, e um membro
//! removido nunca sai de verdade -- basta outro membro (ou ele mesmo, por um
//! terceiro) apresenta-lo de novo. Com o rol, a senha continua sendo a
//! condicao de fechar aperto (a PSK), mas deixa de ser a condicao de SER
//! membro: membro e quem esta no rol de versao mais nova que a chave do dono
//! assinou.
//!
//! # O dono
//!
//! A chave Ed25519 do dono NAO e um segredo novo em disco: sai por HKDF da
//! identidade X25519 de quem criou (`p2p.chave`) e do nome da rede. Perder a
//! identidade ja era perder o lugar na rede; agora e perder tambem o poder de
//! mudar o rol -- o mesmo arquivo, a mesma protecao (0600 / DACL / DPAPI), e
//! nenhum caminho novo onde um segredo possa vazar. O HKDF separa as duas
//! chaves: a X25519 nunca assina e a Ed25519 nunca faz DH.
//!
//! # Um assinante so
//!
//! Delegacao para administradores foi avaliada e RECUSADA nesta rodada: com
//! dois assinantes, dois rois de versao N+1 podem nascer ao mesmo tempo, e a
//! malha precisaria de regra de desempate e de fusao -- o problema de
//! replicacao inteiro, para uma rede de dezenas de membros. Com um, a versao
//! e uma sequencia e «mais novo» nao tem ambiguidade. O preco esta no
//! `PHXVPN.md`: com o criador fora do ar ninguem entra nem sai.
//!
//! # Formato (o que se assina)
//!
//! ```text
//! "phxvpn-rol-v1" | u16 len(rede) | rede | u64 versao | u16 n |
//!   n x ( chave X25519 32 | IPv4 4 | u8 len(nome) | nome ) | assinatura 64
//! ```
//!
//! Tudo big-endian. Binario e nao JSON de proposito: o que se assina tem de
//! ter UMA forma so, e JSON tem muitas (espacos, ordem das chaves, escape).
//! A mesma sequencia vai no tunel (controle `R`) e no arquivo (em hex).

use phxsql_core::ed25519;
use std::net::Ipv4Addr;

const ROTULO: &[u8] = b"phxvpn-rol-v1";
/// O mesmo teto de pares aprendidos da malha.
pub const TETO_MEMBROS: usize = 1024;
/// Nome de membro e apelido, nao ficha cadastral.
pub const TETO_NOME: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct Membro {
    /// Publica X25519 -- a mesma que o aperto Noise autentica.
    pub chave: [u8; 32],
    /// O IP virtual fica AMARRADO a chave no rol: sem isso, um membro podia
    /// se apresentar com o IP de outro e o roteamento pela chave cairia.
    pub ip: Ipv4Addr,
    pub nome: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rol {
    pub rede: String,
    pub versao: u64,
    pub membros: Vec<Membro>,
    pub assinatura: [u8; ed25519::ASSINATURA_LEN],
}

/// A chave privada Ed25519 do dono desta rede, derivada da identidade.
pub fn privada_do_dono(identidade: &[u8; 32], rede: &str) -> [u8; 32] {
    let mut k = [0u8; 32];
    phxsql_core::hkdf::derivar(b"phxvpn-rol-dono-v1", identidade, rede.as_bytes(), &mut k)
        .expect("32 bytes");
    k
}

pub fn publica_do_dono(identidade: &[u8; 32], rede: &str) -> [u8; 32] {
    ed25519::chave_publica(&privada_do_dono(identidade, rede))
}

fn corpo(rede: &str, versao: u64, membros: &[Membro]) -> Vec<u8> {
    let mut b = ROTULO.to_vec();
    b.extend_from_slice(&(rede.len() as u16).to_be_bytes());
    b.extend_from_slice(rede.as_bytes());
    b.extend_from_slice(&versao.to_be_bytes());
    b.extend_from_slice(&(membros.len() as u16).to_be_bytes());
    for m in membros {
        b.extend_from_slice(&m.chave);
        b.extend_from_slice(&m.ip.octets());
        let nome = m.nome.as_deref().unwrap_or("");
        b.push(nome.len() as u8);
        b.extend_from_slice(nome.as_bytes());
    }
    b
}

fn validar(membros: &[Membro]) -> Result<(), String> {
    if membros.len() > TETO_MEMBROS {
        return Err(format!("rol acima de {TETO_MEMBROS} membros"));
    }
    for (i, m) in membros.iter().enumerate() {
        if m.nome.as_ref().is_some_and(|n| n.len() > TETO_NOME) {
            return Err(format!("nome de membro acima de {TETO_NOME} bytes"));
        }
        if membros[..i]
            .iter()
            .any(|o| o.chave == m.chave || o.ip == m.ip)
        {
            return Err(format!("rol com chave ou IP repetido ({})", m.ip));
        }
    }
    Ok(())
}

/// Segundos desde a epoca: a versao nova e `max(anterior + 1, agora)`. O
/// relogio so ajuda (uma rede recriada com o mesmo nome e a mesma identidade
/// nasce com versao acima da antiga); quem garante que ela sobe e o `+ 1`.
fn proxima_versao(anterior: u64) -> u64 {
    (anterior + 1).max(crate::rede_p2p::agora())
}

impl Rol {
    pub fn assinar(
        rede: &str,
        versao: u64,
        membros: Vec<Membro>,
        privada_dono: &[u8; 32],
    ) -> Result<Rol, String> {
        validar(&membros)?;
        let assinatura = ed25519::assinar(privada_dono, &corpo(rede, versao, &membros));
        Ok(Rol {
            rede: rede.to_string(),
            versao,
            membros,
            assinatura,
        })
    }

    /// O primeiro rol, so com quem cria.
    pub fn primeiro(rede: &str, criador: Membro, identidade: &[u8; 32]) -> Result<Rol, String> {
        Rol::assinar(
            rede,
            proxima_versao(0),
            vec![criador],
            &privada_do_dono(identidade, rede),
        )
    }

    /// Rol novo com um membro a mais (so o dono consegue).
    pub fn com(&self, m: Membro, identidade: &[u8; 32]) -> Result<Rol, String> {
        let mut membros = self.membros.clone();
        membros.push(m);
        Rol::assinar(
            &self.rede,
            proxima_versao(self.versao),
            membros,
            &privada_do_dono(identidade, &self.rede),
        )
    }

    /// Rol novo sem o membro da chave dada.
    pub fn sem(&self, chave: &[u8; 32], identidade: &[u8; 32]) -> Result<Rol, String> {
        let membros: Vec<Membro> = self
            .membros
            .iter()
            .filter(|m| m.chave != *chave)
            .cloned()
            .collect();
        if membros.len() == self.membros.len() {
            return Err("essa chave nao esta no rol".into());
        }
        Rol::assinar(
            &self.rede,
            proxima_versao(self.versao),
            membros,
            &privada_do_dono(identidade, &self.rede),
        )
    }

    pub fn membro(&self, chave: &[u8; 32]) -> Option<&Membro> {
        self.membros.iter().find(|m| m.chave == *chave)
    }

    pub fn para_bytes(&self) -> Vec<u8> {
        let mut b = corpo(&self.rede, self.versao, &self.membros);
        b.extend_from_slice(&self.assinatura);
        b
    }

    /// So desmonta; quem decide se vale e `conferir` / `avaliar`. Nunca entra
    /// em panico: o dado vem da rede.
    pub fn de_bytes(b: &[u8]) -> Option<Rol> {
        let mut p = b.strip_prefix(ROTULO)?;
        let mut tomar = |n: usize| -> Option<&[u8]> {
            let (a, r) = (p.get(..n)?, p.get(n..)?);
            p = r;
            Some(a)
        };
        let n_rede = u16::from_be_bytes(tomar(2)?.try_into().ok()?) as usize;
        let rede = std::str::from_utf8(tomar(n_rede)?).ok()?.to_string();
        let versao = u64::from_be_bytes(tomar(8)?.try_into().ok()?);
        let n = u16::from_be_bytes(tomar(2)?.try_into().ok()?) as usize;
        if n > TETO_MEMBROS {
            return None;
        }
        let mut membros = Vec::with_capacity(n);
        for _ in 0..n {
            let chave: [u8; 32] = tomar(32)?.try_into().ok()?;
            let ip: [u8; 4] = tomar(4)?.try_into().ok()?;
            let n_nome = *tomar(1)?.first()? as usize;
            let nome = std::str::from_utf8(tomar(n_nome)?).ok()?;
            membros.push(Membro {
                chave,
                ip: Ipv4Addr::from(ip),
                nome: (!nome.is_empty()).then(|| nome.to_string()),
            });
        }
        let assinatura: [u8; 64] = tomar(64)?.try_into().ok()?;
        if !p.is_empty() || validar(&membros).is_err() {
            return None;
        }
        Some(Rol {
            rede,
            versao,
            membros,
            assinatura,
        })
    }

    /// Assinado pelo dono DESTA rede. O nome entra na conta: o mesmo dono
    /// com duas redes nao consegue ter o rol de uma aceito na outra.
    pub fn conferir(&self, dono: &[u8; 32], rede: &str) -> bool {
        self.rede == rede
            && ed25519::conferir(
                dono,
                &corpo(&self.rede, self.versao, &self.membros),
                &self.assinatura,
            )
    }
}

/// O que fazer com um rol que chegou pela malha.
#[derive(Debug, PartialEq)]
pub enum Veredito {
    /// Assinado pelo dono e mais novo que o que se tem: adota.
    Novo(Rol),
    /// Assinado, mas de versao igual ou anterior: ignora (e quem mandou
    /// merece receber o nosso, que e mais novo ou igual).
    Velho(u64),
    /// Torto, adulterado, de outra rede ou de outra chave.
    Invalido,
}

/// A conferencia inteira de um rol que chegou. A ordem importa: a assinatura
/// vem ANTES da versao -- senao um rol forjado de versao alta seria contado
/// como «mais novo» por quem ainda nao olhou quem o assinou.
pub fn avaliar(bytes: &[u8], dono: &[u8; 32], rede: &str, versao_atual: u64) -> Veredito {
    let Some(r) = Rol::de_bytes(bytes) else {
        return Veredito::Invalido;
    };
    if !r.conferir(dono, rede) {
        return Veredito::Invalido;
    }
    if r.versao <= versao_atual {
        return Veredito::Velho(r.versao);
    }
    Veredito::Novo(r)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn membro(n: u8) -> Membro {
        Membro {
            chave: [n; 32],
            ip: Ipv4Addr::new(10, 78, 0, n),
            nome: (n == 1).then(|| "matriz".to_string()),
        }
    }

    fn base() -> ([u8; 32], [u8; 32], Rol) {
        let identidade = [7u8; 32];
        let dono = publica_do_dono(&identidade, "R");
        let r = Rol::primeiro("R", membro(1), &identidade)
            .unwrap()
            .com(membro(2), &identidade)
            .unwrap();
        (identidade, dono, r)
    }

    #[test]
    fn rol_assinado_confere_e_faz_ida_e_volta() {
        let (_, dono, r) = base();
        let b = r.para_bytes();
        assert_eq!(Rol::de_bytes(&b).unwrap(), r);
        assert_eq!(avaliar(&b, &dono, "R", 0), Veredito::Novo(r.clone()));
        assert_eq!(r.membro(&[1; 32]).unwrap().nome.as_deref(), Some("matriz"));
    }

    /// Cada byte do corpo esta coberto: trocar a chave de um membro (o
    /// ataque de quem quer se enfiar no rol) derruba a assinatura.
    #[test]
    fn rol_adulterado_e_recusado() {
        let (_, dono, r) = base();
        let b = r.para_bytes();
        let mut torto = b.clone();
        let pos = b.len() - 64 - 10; // dentro do ultimo membro
        torto[pos] ^= 1;
        assert_eq!(avaliar(&torto, &dono, "R", 0), Veredito::Invalido);
        // Versao inflada a mao, para passar na frente: tambem cai.
        let mut inflado = b.clone();
        let v = ROTULO.len() + 2 + 1;
        inflado[v] = 0xff;
        assert_eq!(avaliar(&inflado, &dono, "R", 0), Veredito::Invalido);
    }

    #[test]
    fn assinatura_de_outra_chave_e_recusada() {
        let (_, dono, r) = base();
        let intruso = Rol::assinar("R", r.versao + 1, r.membros.clone(), &[9; 32]).unwrap();
        assert_eq!(
            avaliar(&intruso.para_bytes(), &dono, "R", 0),
            Veredito::Invalido
        );
        // E o rol certo, apresentado como sendo de outra rede, tambem.
        assert_eq!(avaliar(&r.para_bytes(), &dono, "S", 0), Veredito::Invalido);
    }

    /// Anti-rollback: o rol que ainda tinha o membro removido, reapresentado
    /// depois, nao volta a valer.
    #[test]
    fn rol_velho_e_recusado() {
        let (identidade, dono, r) = base();
        let novo = r.sem(&[2; 32], &identidade).unwrap();
        assert!(novo.versao > r.versao);
        assert_eq!(
            avaliar(&r.para_bytes(), &dono, "R", novo.versao),
            Veredito::Velho(r.versao)
        );
        assert_eq!(
            avaliar(&novo.para_bytes(), &dono, "R", novo.versao),
            Veredito::Velho(novo.versao),
            "a mesma versao tambem nao e novidade"
        );
    }

    #[test]
    fn rol_com_ip_ou_chave_repetidos_nao_nasce() {
        let (identidade, _, r) = base();
        let mut m = membro(3);
        m.ip = Ipv4Addr::new(10, 78, 0, 2);
        assert!(r.com(m, &identidade).is_err());
        assert!(r.com(membro(2), &identidade).is_err());
        assert!(r.sem(&[5; 32], &identidade).is_err());
    }

    #[test]
    fn dono_muda_com_a_rede_e_com_a_identidade() {
        assert_ne!(
            publica_do_dono(&[1; 32], "R"),
            publica_do_dono(&[1; 32], "S")
        );
        assert_ne!(
            publica_do_dono(&[1; 32], "R"),
            publica_do_dono(&[2; 32], "R")
        );
    }

    #[test]
    fn lixo_nao_derruba() {
        for n in 0..200 {
            let lixo: Vec<u8> = (0..n).map(|i| (i * 31 + 7) as u8).collect();
            assert!(Rol::de_bytes(&lixo).is_none());
        }
        let (_, _, r) = base();
        let b = r.para_bytes();
        for corte in 0..b.len() {
            assert!(Rol::de_bytes(&b[..corte]).is_none());
        }
    }
}
