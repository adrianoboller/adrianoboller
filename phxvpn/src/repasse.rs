//! O servidor intermediario (repasse) do modo P2P.
//!
//! Decisao do dono (23/09/2026): quando os dois lados estao atras de CGNAT e
//! nao ha caminho direto, o trafego passa por um servidor NOSSO -- que so
//! carrega pacote que ja vem cifrado de ponta a ponta pelo Noise dos pares.
//! O repasse nao tem a PSK nem as chaves de sessao: ve quem fala com quem
//! (as chaves publicas) e o tamanho, nunca o conteudo.
//!
//! # Pacotes (alem dos do `transporte.rs`)
//!
//! ```text
//! REGISTRO [7,0,0,0] chave_do_no:32 carimbo:12 mac:32
//! PARA     [8,0,0,0] chave_destino:32 | pacote do par (INICIO/RESPOSTA/DADOS)
//! DE       [9,0,0,0] chave_origem:32  | pacote do par
//! ```
//!
//! # Por que o REGISTRO prova a chave sem ida-e-volta
//!
//! O `mac` e HMAC-SHA256 com a chave `DH(no, repasse)`: so quem tem a privada
//! do no (ou a do repasse) calcula. Sem isso, qualquer um se registraria com a
//! chave publica de outro e receberia o trafego dele -- cifrado, mas desviado
//! (negacao de servico). O carimbo crescente impede reenviar um REGISTRO
//! gravado de outro endereco.
//!
//! # Por que o repasse nao confia na chave que o PARA declara
//!
//! A origem sai do ENDERECO que registrou, nao do pacote: um no nao consegue
//! mandar em nome de outro. E o no que recebe ainda confere que a chave de
//! origem bate com a do aperto -- o repasse mentiroso nao engana o Noise.

use phxsql_core::hash::{hmac_sha256, iguais_em_tempo_constante};
use phxsql_core::x25519;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub const TIPO_REGISTRO: u8 = 7;
pub const TIPO_PARA: u8 = 8;
pub const TIPO_DE: u8 = 9;

pub const REGISTRO_LEN: usize = 4 + 32 + 12 + 32;
/// O no renova o registro nesse ritmo -- tambem mantem aberto o furo do NAT.
pub const RENOVAR_REGISTRO: Duration = Duration::from_secs(20);
/// Registro sem renovacao nesse prazo sai da tabela.
pub const VALIDADE_REGISTRO: Duration = Duration::from_secs(60);
/// Teto de nos na tabela: registro valido custa um DH, mas ocupa memoria.
pub const TETO_NOS: usize = 100_000;

fn mac(segredo: &[u8; 32], chave: &[u8; 32], carimbo: &[u8; 12]) -> [u8; 32] {
    let mut m = b"phxvpn-repasse-registro".to_vec();
    m.extend_from_slice(chave);
    m.extend_from_slice(carimbo);
    hmac_sha256(segredo, &m)
}

/// O REGISTRO que o no manda ao repasse.
pub fn registro(
    privada_no: &[u8; 32],
    publica_repasse: &[u8; 32],
    carimbo: [u8; 12],
) -> Result<Vec<u8>, String> {
    let segredo = x25519::segredo(privada_no, publica_repasse).map_err(|e| e.to_string())?;
    let chave = x25519::chave_publica(privada_no);
    let mut p = vec![TIPO_REGISTRO, 0, 0, 0];
    p.extend_from_slice(&chave);
    p.extend_from_slice(&carimbo);
    p.extend_from_slice(&mac(&segredo, &chave, &carimbo));
    Ok(p)
}

pub fn embrulhar_para(destino: &[u8; 32], pacote: &[u8]) -> Vec<u8> {
    let mut p = vec![TIPO_PARA, 0, 0, 0];
    p.extend_from_slice(destino);
    p.extend_from_slice(pacote);
    p
}

/// `DE` -> (chave de origem, pacote do par).
pub fn desembrulhar_de(p: &[u8]) -> Option<([u8; 32], &[u8])> {
    (p.len() > 36 && p[..4] == [TIPO_DE, 0, 0, 0])
        .then(|| (p[4..36].try_into().expect("32"), &p[36..]))
}

struct Registro {
    endereco: SocketAddr,
    carimbo: [u8; 12],
    visto: Instant,
}

pub struct Repasse {
    privada: [u8; 32],
    nos: HashMap<[u8; 32], Registro>,
    por_endereco: HashMap<SocketAddr, [u8; 32]>,
    /// Com lista, so repassa entre chaves dela (repasse fechado da empresa).
    permitidas: Option<HashSet<[u8; 32]>>,
}

impl Repasse {
    pub fn novo(privada: [u8; 32], permitidas: Option<HashSet<[u8; 32]>>) -> Repasse {
        Repasse {
            privada,
            nos: HashMap::new(),
            por_endereco: HashMap::new(),
            permitidas,
        }
    }

    pub fn publica(&self) -> [u8; 32] {
        x25519::chave_publica(&self.privada)
    }

    fn permitida(&self, k: &[u8; 32]) -> bool {
        self.permitidas.as_ref().map_or(true, |l| l.contains(k))
    }

    fn vivo(&self, k: &[u8; 32]) -> Option<&Registro> {
        self.nos
            .get(k)
            .filter(|r| r.visto.elapsed() < VALIDADE_REGISTRO)
    }

    /// Trata um datagrama. Devolve o que mandar e para onde.
    pub fn tratar(&mut self, dado: &[u8], de: SocketAddr) -> Option<(SocketAddr, Vec<u8>)> {
        match *dado.first()? {
            TIPO_REGISTRO => {
                self.registrar(dado, de);
                None
            }
            TIPO_PARA => {
                if dado.len() <= 36 {
                    return None;
                }
                let origem = *self.por_endereco.get(&de)?;
                self.vivo(&origem).filter(|r| r.endereco == de)?;
                let destino: [u8; 32] = dado[4..36].try_into().ok()?;
                if !self.permitida(&destino) {
                    return None;
                }
                let alvo = self.vivo(&destino)?.endereco;
                let mut saida = vec![TIPO_DE, 0, 0, 0];
                saida.extend_from_slice(&origem);
                saida.extend_from_slice(&dado[36..]);
                Some((alvo, saida))
            }
            _ => None,
        }
    }

    fn registrar(&mut self, dado: &[u8], de: SocketAddr) {
        if dado.len() != REGISTRO_LEN || dado[..4] != [TIPO_REGISTRO, 0, 0, 0] {
            return;
        }
        let chave: [u8; 32] = dado[4..36].try_into().expect("32");
        let carimbo: [u8; 12] = dado[36..48].try_into().expect("12");
        if !self.permitida(&chave) {
            return;
        }
        let Ok(segredo) = x25519::segredo(&self.privada, &chave) else {
            return;
        };
        if !iguais_em_tempo_constante(&mac(&segredo, &chave, &carimbo), &dado[48..80]) {
            return;
        }
        if let Some(r) = self.nos.get(&chave) {
            if carimbo <= r.carimbo {
                return;
            }
        } else if self.nos.len() >= TETO_NOS {
            self.nos
                .retain(|_, r| r.visto.elapsed() < VALIDADE_REGISTRO);
            if self.nos.len() >= TETO_NOS {
                return;
            }
        }
        if let Some(velho) = self.nos.get(&chave) {
            self.por_endereco.remove(&velho.endereco);
        }
        self.por_endereco.insert(de, chave);
        self.nos.insert(
            chave,
            Registro {
                endereco: de,
                carimbo,
                visto: Instant::now(),
            },
        );
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn end(p: u16) -> SocketAddr {
        SocketAddr::from(([198, 51, 100, 1], p))
    }

    fn carimbo(n: u8) -> [u8; 12] {
        let mut c = [0u8; 12];
        c[11] = n;
        c
    }

    #[test]
    fn repassa_entre_registrados_com_a_origem_do_endereco() {
        let r_priv = x25519::gerar_privada();
        let mut r = Repasse::novo(r_priv, None);
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (pa, pb) = (x25519::chave_publica(&a), x25519::chave_publica(&b));
        r.tratar(&registro(&a, &r.publica(), carimbo(1)).unwrap(), end(1));
        r.tratar(&registro(&b, &r.publica(), carimbo(1)).unwrap(), end(2));
        let (alvo, saida) = r.tratar(&embrulhar_para(&pb, b"oi"), end(1)).unwrap();
        assert_eq!(alvo, end(2));
        assert_eq!(desembrulhar_de(&saida), Some((pa, &b"oi"[..])));
        // Endereco nao registrado nao manda nada.
        assert!(r.tratar(&embrulhar_para(&pb, b"oi"), end(9)).is_none());
    }

    #[test]
    fn registro_com_chave_alheia_ou_repetido_e_recusado() {
        let mut r = Repasse::novo(x25519::gerar_privada(), None);
        let (a, b, m) = (
            x25519::gerar_privada(),
            x25519::gerar_privada(),
            x25519::gerar_privada(),
        );
        let pb = x25519::chave_publica(&b);
        r.tratar(&registro(&b, &r.publica(), carimbo(5)).unwrap(), end(2));
        // O atacante M tenta se registrar como B: poe a chave de B com o mac dele.
        let mut falso = registro(&m, &r.publica(), carimbo(9)).unwrap();
        falso[4..36].copy_from_slice(&pb);
        r.tratar(&falso, end(66));
        // Reenvio do registro legitimo (carimbo velho) de outro endereco.
        r.tratar(&registro(&b, &r.publica(), carimbo(5)).unwrap(), end(67));
        r.tratar(&registro(&a, &r.publica(), carimbo(1)).unwrap(), end(1));
        let (alvo, _) = r.tratar(&embrulhar_para(&pb, b"x"), end(1)).unwrap();
        assert_eq!(alvo, end(2), "o trafego de B continua indo para B");
    }

    #[test]
    fn lista_de_permitidas_fecha_o_repasse() {
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let pa = x25519::chave_publica(&a);
        let mut r = Repasse::novo(x25519::gerar_privada(), Some([pa].into_iter().collect()));
        r.tratar(&registro(&a, &r.publica(), carimbo(1)).unwrap(), end(1));
        r.tratar(&registro(&b, &r.publica(), carimbo(1)).unwrap(), end(2));
        assert!(r
            .tratar(&embrulhar_para(&x25519::chave_publica(&b), b"x"), end(1))
            .is_none());
    }
}
