//! O transporte do modo P2P: formato dos pacotes UDP, a sessao cifrada e a
//! janela contra repeticao. Logica pura, sem soquete -- quem manda e recebe e
//! o `p2p.rs`.
//!
//! # Pacotes (inteiros em little-endian)
//!
//! ```text
//! INICIO   [1,0,0,0] remetente:u32 | mensagem 1 do Noise (carga = carimbo 12 B)
//! RESPOSTA [2,0,0,0] remetente:u32 receptor:u32 | mensagem 2 do Noise
//! DADOS    [4,0,0,0] receptor:u32 contador:u64 | pacote IP cifrado + etiqueta
//! ```
//!
//! E o molde do WireGuard (whitepaper secao 5.4). Onde diverge, e por que:
//! o cabecalho de DADOS entra como dado associado da cifra -- aqui nao custa
//! nada e amarra o indice e o contador a etiqueta; e ainda nao ha `mac1`/
//! cookie contra inundacao de INICIO (fica anotado no PHXVPN.md).
//!
//! # Os prazos (whitepaper secao 6.1)
//!
//! Quem iniciou refaz o aperto aos 120 s; sessao com 180 s nao cifra nem
//! decifra mais; 2^60 mensagens idem. Chave velha morre -- e isso que da o
//! sigilo adiante: roubar a estatica amanha nao abre o trafego de hoje.

use crate::noise::Sessao as Chaves;
use phxsql_core::cifra::{abrir, selar, TAG_LEN};
use phxsql_core::fio::nonce_do_contador;
use std::time::{Duration, Instant};

pub const TIPO_INICIO: u8 = 1;
pub const TIPO_RESPOSTA: u8 = 2;
pub const TIPO_DADOS: u8 = 4;

pub const REFAZER_APOS: Duration = Duration::from_secs(120);
pub const REJEITAR_APOS: Duration = Duration::from_secs(180);
pub const REJEITAR_APOS_MENSAGENS: u64 = 1 << 60;

/// Cabecalho de DADOS: tipo(4) + receptor(4) + contador(8).
pub const CAB_DADOS: usize = 16;

pub type R<T> = Result<T, String>;

/// Janela contra repeticao de 2.048 contadores (RFC 6479, em bitmap).
///
/// UDP entrega fora de ordem, entao nao basta «maior que o ultimo»; e um
/// pacote gravado e reenviado nao pode entrar duas vezes. A janela so e
/// ATUALIZADA depois que a etiqueta confere: marcar antes deixaria qualquer
/// um queimar contadores legitimos mandando lixo.
pub struct Janela {
    maior: u64,
    bits: [u64; JANELA_BLOCOS],
    vazia: bool,
}

const JANELA_BLOCOS: usize = 32;
pub const JANELA_BITS: u64 = (JANELA_BLOCOS * 64) as u64;

impl Default for Janela {
    fn default() -> Self {
        Janela {
            maior: 0,
            bits: [0; JANELA_BLOCOS],
            vazia: true,
        }
    }
}

impl Janela {
    fn posicao(n: u64) -> (usize, u64) {
        (((n / 64) as usize) % JANELA_BLOCOS, 1u64 << (n % 64))
    }

    /// O contador ainda nao foi visto e esta dentro da janela?
    pub fn pode(&self, n: u64) -> bool {
        if self.vazia || n > self.maior {
            return true;
        }
        if self.maior - n >= JANELA_BITS - 64 {
            return false;
        }
        let (b, m) = Janela::posicao(n);
        self.bits[b] & m == 0
    }

    /// Marca o contador (so depois da etiqueta conferida).
    pub fn marcar(&mut self, n: u64) {
        if self.vazia || n > self.maior {
            let bloco_novo = n / 64;
            let bloco_velho = if self.vazia {
                bloco_novo
            } else {
                self.maior / 64
            };
            // Zera os blocos que a janela atravessou -- no maximo todos.
            let pular = (bloco_novo - bloco_velho).min(JANELA_BLOCOS as u64);
            for i in 1..=pular {
                self.bits[((bloco_velho + i) as usize) % JANELA_BLOCOS] = 0;
            }
            if self.vazia {
                self.bits = [0; JANELA_BLOCOS];
            }
            self.maior = n;
            self.vazia = false;
        }
        let (b, m) = Janela::posicao(n);
        self.bits[b] |= m;
    }
}

/// Uma sessao estabelecida com um par.
pub struct Sessao {
    chaves: Chaves,
    pub meu_indice: u32,
    pub indice_dele: u32,
    enviados: u64,
    janela: Janela,
    nasceu: Instant,
    pub iniciador: bool,
    /// Quem respondeu so envia depois de receber o primeiro DADOS valido: e o
    /// que prova que o iniciador tem mesmo as chaves (e o que o WireGuard faz
    /// contra personificacao por chave comprometida, secao 5.1).
    pub confirmada: bool,
}

impl Sessao {
    pub fn nova(chaves: Chaves, meu_indice: u32, indice_dele: u32, iniciador: bool) -> Sessao {
        Sessao {
            chaves,
            meu_indice,
            indice_dele,
            enviados: 0,
            janela: Janela::default(),
            nasceu: Instant::now(),
            iniciador,
            confirmada: iniciador,
        }
    }

    pub fn idade(&self) -> Duration {
        self.nasceu.elapsed()
    }

    pub fn expirada(&self) -> bool {
        self.idade() >= REJEITAR_APOS || self.enviados >= REJEITAR_APOS_MENSAGENS
    }

    /// Quem iniciou refaz o aperto aos 120 s (so quem iniciou, para os dois
    /// lados nao refazerem juntos e se atropelarem).
    pub fn pede_novo_aperto(&self) -> bool {
        self.iniciador && self.idade() >= REFAZER_APOS
    }

    /// Cifra um pacote IP (vazio = manter vivo) num DADOS.
    pub fn selar(&mut self, claro: &[u8]) -> R<Vec<u8>> {
        if self.expirada() {
            return Err("sessao expirada".into());
        }
        if !self.confirmada {
            return Err("sessao ainda nao confirmada pelo iniciador".into());
        }
        let n = self.enviados;
        self.enviados += 1;
        let mut pacote = Vec::with_capacity(CAB_DADOS + claro.len() + TAG_LEN);
        pacote.extend_from_slice(&[TIPO_DADOS, 0, 0, 0]);
        pacote.extend_from_slice(&self.indice_dele.to_le_bytes());
        pacote.extend_from_slice(&n.to_le_bytes());
        let (c, tag) = selar(
            &self.chaves.envio,
            &nonce_do_contador(n),
            &pacote[..CAB_DADOS],
            claro,
        );
        pacote.extend_from_slice(&c);
        pacote.extend_from_slice(&tag);
        Ok(pacote)
    }

    /// Abre um DADOS ja roteado para esta sessao (pelo indice do receptor).
    pub fn abrir(&mut self, pacote: &[u8]) -> R<Vec<u8>> {
        if pacote.len() < CAB_DADOS + TAG_LEN || pacote[0] != TIPO_DADOS {
            return Err("pacote de dados torto".into());
        }
        if self.expirada() {
            return Err("sessao expirada".into());
        }
        let n = u64::from_le_bytes(pacote[8..16].try_into().expect("8 bytes"));
        if !self.janela.pode(n) {
            return Err("pacote repetido ou velho demais".into());
        }
        let corte = pacote.len() - TAG_LEN;
        let tag: [u8; TAG_LEN] = pacote[corte..].try_into().expect("16 bytes");
        let claro = abrir(
            &self.chaves.recepcao,
            &nonce_do_contador(n),
            &pacote[..CAB_DADOS],
            &pacote[CAB_DADOS..corte],
            &tag,
        )
        .map_err(|_| "etiqueta do pacote nao confere".to_string())?;
        self.janela.marcar(n);
        self.confirmada = true;
        Ok(claro)
    }
}

/// O indice do receptor de um DADOS (para achar a sessao antes de abrir).
pub fn receptor_de_dados(pacote: &[u8]) -> Option<u32> {
    (pacote.len() >= CAB_DADOS && pacote[0] == TIPO_DADOS)
        .then(|| u32::from_le_bytes(pacote[4..8].try_into().expect("4 bytes")))
}

pub fn embrulhar_inicio(remetente: u32, m1: &[u8]) -> Vec<u8> {
    let mut p = vec![TIPO_INICIO, 0, 0, 0];
    p.extend_from_slice(&remetente.to_le_bytes());
    p.extend_from_slice(m1);
    p
}

pub fn desembrulhar_inicio(p: &[u8]) -> Option<(u32, &[u8])> {
    (p.len() > 8 && p[..4] == [TIPO_INICIO, 0, 0, 0])
        .then(|| (u32::from_le_bytes(p[4..8].try_into().expect("4")), &p[8..]))
}

pub fn embrulhar_resposta(remetente: u32, receptor: u32, m2: &[u8]) -> Vec<u8> {
    let mut p = vec![TIPO_RESPOSTA, 0, 0, 0];
    p.extend_from_slice(&remetente.to_le_bytes());
    p.extend_from_slice(&receptor.to_le_bytes());
    p.extend_from_slice(m2);
    p
}

pub fn desembrulhar_resposta(p: &[u8]) -> Option<(u32, u32, &[u8])> {
    (p.len() > 12 && p[..4] == [TIPO_RESPOSTA, 0, 0, 0]).then(|| {
        (
            u32::from_le_bytes(p[4..8].try_into().expect("4")),
            u32::from_le_bytes(p[8..12].try_into().expect("4")),
            &p[12..],
        )
    })
}

/// Carimbo de 12 bytes (segundos + nanossegundos, big-endian, como o TAI64N)
/// que vai na carga do INICIO. Quem responde guarda o ultimo por par e recusa
/// o que nao for maior: um INICIO gravado e reenviado nao abre sessao nova.
pub fn carimbo_agora() -> [u8; 12] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut c = [0u8; 12];
    c[..8].copy_from_slice(&(d.as_secs() + (1u64 << 62)).to_be_bytes());
    c[8..].copy_from_slice(&d.subsec_nanos().to_be_bytes());
    c
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::noise;
    use phxsql_core::x25519;

    fn par_de_sessoes() -> (Sessao, Sessao) {
        let (a, b) = (x25519::gerar_privada(), x25519::gerar_privada());
        let (ini, m1) =
            noise::Iniciador::comecar(noise::PROLOGO, a, &x25519::chave_publica(&b), [7; 32], b"")
                .unwrap();
        let (sb, m2) = noise::ler_chamada(noise::PROLOGO, &b, &m1)
            .unwrap()
            .responder([7; 32], b"")
            .unwrap();
        let (sa, _) = ini.terminar(&m2).unwrap();
        (Sessao::nova(sa, 1, 2, true), Sessao::nova(sb, 2, 1, false))
    }

    #[test]
    fn ida_e_volta_e_repeticao_recusada() {
        let (mut a, mut b) = par_de_sessoes();
        // Quem respondeu nao fala antes de ouvir o iniciador.
        assert!(b.selar(b"cedo").is_err());
        let p = a.selar(b"ping").unwrap();
        assert_eq!(b.abrir(&p).unwrap(), b"ping");
        assert!(b.abrir(&p).is_err(), "o mesmo pacote nao entra duas vezes");
        let q = b.selar(b"pong").unwrap();
        assert_eq!(a.abrir(&q).unwrap(), b"pong");
    }

    #[test]
    fn bit_virado_nao_abre_e_nao_queima_o_contador() {
        let (mut a, mut b) = par_de_sessoes();
        let p = a.selar(b"dado").unwrap();
        let mut torto = p.clone();
        let fim = torto.len() - 1;
        torto[fim] ^= 1;
        assert!(b.abrir(&torto).is_err());
        // O legitimo com o mesmo contador ainda entra: o lixo nao marcou.
        assert_eq!(b.abrir(&p).unwrap(), b"dado");
        let mut cab = p.clone();
        cab[8] ^= 1; // mexer no contador (dado associado) derruba a etiqueta
        assert!(b.abrir(&cab).is_err());
    }

    #[test]
    fn janela_aceita_fora_de_ordem_e_recusa_velho() {
        let mut j = Janela::default();
        for n in [5u64, 3, 9, 4] {
            assert!(j.pode(n));
            j.marcar(n);
        }
        assert!(!j.pode(3) && !j.pode(9));
        j.marcar(10_000);
        assert!(!j.pode(9), "fora da janela e velho demais");
        assert!(j.pode(9_990));
        j.marcar(9_990);
        assert!(!j.pode(9_990));
    }

    #[test]
    fn embrulhos_ida_e_volta() {
        let i = embrulhar_inicio(77, &[9; 40]);
        assert_eq!(desembrulhar_inicio(&i), Some((77, &[9u8; 40][..])));
        let r = embrulhar_resposta(1, 2, &[8; 48]);
        assert_eq!(desembrulhar_resposta(&r), Some((1, 2, &[8u8; 48][..])));
        assert!(desembrulhar_inicio(&r).is_none());
        assert!(carimbo_agora() > [0; 12]);
    }
}
