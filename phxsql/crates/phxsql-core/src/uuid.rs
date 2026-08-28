//! Identificadores: UUID de 128 bits (v4 e v7) e identificador de 256 bits.
//!
//! # Por que o v7 importa AQUI, e nao so por moda
//!
//! A bancada de dez milhoes mediu o buraco do motor: a insercao cai de 5.089
//! linhas/s no primeiro milhao para 3.626/s no ultimo, com o disco parado e a
//! CPU em 99%. A causa e a B+tree do `.ndx` sendo reescrita a cada linha.
//!
//! Chave ALEATORIA -- um UUID v4, por exemplo -- espalha cada insercao por uma
//! folha diferente da arvore: toda gravacao suja uma pagina nova, e quanto
//! maior a tabela, mais longe uma da outra. Chave CRESCENTE cai sempre na
//! folha mais a direita, que ja esta na memoria. E a diferenca entre semear a
//! arvore inteira e anexar no fim dela.
//!
//! O v7 e crescente por construcao: os primeiros 48 bits sao o relogio em
//! milissegundos, em big-endian. E como a chave do `.ndx` guarda os bytes na
//! ordem natural (ver `keyenc`), comparar bytes = comparar tempo.
//!
//! # Monotonico de verdade, nao "quase"
//!
//! Dentro do mesmo milissegundo o relogio nao separa nada, e dois v7 gerados
//! juntos sairiam fora de ordem. Por isso os 12 bits de `rand_a` viram um
//! CONTADOR (o metodo 1 da secao 6.2 do RFC 9562): no primeiro id de cada
//! milissegundo ele nasce sorteado na metade de baixo da faixa, e cada id
//! seguinte do mesmo milissegundo soma 1. Estourou, o relogio anda 1 ms para
//! frente em vez de repetir.
//!
//! O resultado e que `gerar_v7()` NUNCA devolve um valor menor ou igual ao
//! anterior, nem sob concorrencia. Isso e o que o indice precisa.
//!
//! # O de 256 bits
//!
//! `Uuid256` nao e um UUID: o RFC 9562 so define 128 bits, e chamar de UUID algo
//! que nao esta no padrao seria mentir no nome do tipo. E um identificador
//! opaco de 32 bytes, e o motivo de existir e pratico: um SHA-256 cabe nele
//! exatamente, sem sobra e sem texto. Hash de bloco, hash de transacao,
//! impressao digital de arquivo.

use std::fmt;
use std::sync::Mutex;

use crate::error::{PhxError, Result};

/// Bytes de um UUID.
pub const UUID_LEN: usize = 16;
/// Bytes de um identificador de 256 bits.
pub const UUID256_LEN: usize = 32;

/// UUID de 128 bits, guardado na ordem de rede (big-endian), que e a mesma
/// ordem em que ele se escreve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Uuid(pub [u8; UUID_LEN]);

/// Identificador de 256 bits.
///
/// ATENCAO: **nao e um UUID do RFC 9562** -- o padrao so define 128 bits. O
/// nome carrega o 256 justamente para nao passar por um UUID comum. E um
/// identificador opaco de 32 bytes, e existe por um motivo pratico: um SHA-256
/// cabe nele exatamente, sem sobra e sem virar texto. Hash de bloco, hash de
/// transacao, impressao digital de arquivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid256(pub [u8; UUID256_LEN]);

impl Default for Uuid256 {
    fn default() -> Self {
        Uuid256([0u8; UUID256_LEN])
    }
}

// ------------------------------------------------------------------ sorteio

/// Bytes sorteados. Tenta `/dev/urandom`; onde ele nao existe, mistura relogio
/// e endereco -- o mesmo caminho que `senha.rs` ja usa e explica.
fn sortear(dst: &mut [u8]) {
    // ATENCAO: /dev/urandom e um dispositivo INFINITO. Ler o "arquivo inteiro"
    // trava para sempre; le-se exatamente o que se precisa.
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if f.read_exact(dst).is_ok() {
            return;
        }
    }
    misturar(dst);
}

/// Reserva para onde nao ha `/dev/urandom` (Windows). Nao e criptografico, e
/// nao precisa ser: o que o v7 exige dos bits sorteados e que dois geradores
/// nao colidam, nao que ninguem os adivinhe.
fn misturar(dst: &mut [u8]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let endereco = dst.as_ptr() as u64;
    let mut estado = nanos ^ endereco.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
    for b in dst.iter_mut() {
        // splitmix64: barato, boa dispersao, e cabe em cinco linhas.
        estado = estado.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = estado;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        *b = ((z ^ (z >> 31)) >> 24) as u8;
    }
}

fn agora_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Ultimo milissegundo emitido e o contador dentro dele.
///
/// Guardado num mutex porque a garantia de "nunca repete e nunca anda para
/// tras" tem de valer entre threads: duas conexoes gravando ao mesmo tempo
/// pedem id ao mesmo gerador.
static RELOGIO: Mutex<(u64, u16)> = Mutex::new((0, 0));

/// Faixa util do contador de 12 bits. Comeca sorteado na metade de baixo para
/// sobrar espaco de contagem sem estourar dentro do mesmo milissegundo.
const CONTADOR_MASCARA: u16 = 0x0FFF;
const CONTADOR_SEMENTE: u16 = 0x07FF;

impl Uuid {
    /// O UUID todo-zeros, `00000000-0000-0000-0000-000000000000`.
    pub const NULO: Uuid = Uuid([0u8; UUID_LEN]);

    pub fn bytes(&self) -> &[u8; UUID_LEN] {
        &self.0
    }

    pub fn de_bytes(b: [u8; UUID_LEN]) -> Uuid {
        Uuid(b)
    }

    /// Versao declarada nos 4 bits altos do byte 6. Vale 4 ou 7 aqui.
    pub fn versao(&self) -> u8 {
        self.0[6] >> 4
    }

    /// Milissegundos desde a epoca, para um v7. `None` em qualquer outra
    /// versao: ler relogio de um v4 seria ler bits sorteados.
    pub fn instante_ms(&self) -> Option<i64> {
        if self.versao() != 7 {
            return None;
        }
        let mut ms: u64 = 0;
        for b in &self.0[..6] {
            ms = (ms << 8) | *b as u64;
        }
        Some(ms as i64)
    }

    /// UUID v4: 122 bits sorteados. Sem ordem nenhuma -- use quando o id NAO
    /// deve revelar quando foi criado.
    pub fn v4() -> Uuid {
        let mut b = [0u8; UUID_LEN];
        sortear(&mut b);
        b[6] = (b[6] & 0x0F) | 0x40;
        b[8] = (b[8] & 0x3F) | 0x80;
        Uuid(b)
    }

    /// UUID v7: relogio em milissegundos nos 48 bits altos, contador de 12
    /// bits e 62 bits sorteados. Estritamente crescente.
    pub fn v7() -> Uuid {
        let (ms, contador) = proximo_passo(agora_ms());
        Uuid::montar_v7(ms, contador)
    }

    /// v7 com o instante escolhido a dedo. Existe para o teste poder conferir
    /// o layout contra o vetor do RFC sem depender do relogio da maquina.
    pub fn v7_em(ms: u64, contador: u16, aleatorios: [u8; 8]) -> Uuid {
        let mut u = Uuid::montar_v7(ms, contador);
        u.0[8..16].copy_from_slice(&aleatorios);
        u.0[8] = (u.0[8] & 0x3F) | 0x80;
        u
    }

    fn montar_v7(ms: u64, contador: u16) -> Uuid {
        let mut b = [0u8; UUID_LEN];
        let t = ms.to_be_bytes();
        b[..6].copy_from_slice(&t[2..]);
        let c = contador & CONTADOR_MASCARA;
        b[6] = 0x70 | ((c >> 8) as u8 & 0x0F);
        b[7] = (c & 0xFF) as u8;
        sortear(&mut b[8..]);
        b[8] = (b[8] & 0x3F) | 0x80;
        Uuid(b)
    }

    /// Le a forma canonica `8-4-4-4-12`. Aceita sem hifens e entre chaves,
    /// porque e assim que os ids chegam colados de fora.
    pub fn de_texto(s: &str) -> Result<Uuid> {
        let limpo: String = s
            .trim()
            .trim_start_matches('{')
            .trim_end_matches('}')
            .chars()
            .filter(|c| *c != '-')
            .collect();
        if limpo.len() != 32 {
            return Err(PhxError::Tipo(format!(
                "UUID precisa de 32 digitos hexadecimais, veio {}: {s:?}",
                limpo.len()
            )));
        }
        let mut b = [0u8; UUID_LEN];
        hex_para(&limpo, &mut b)
            .map_err(|e| PhxError::Tipo(format!("UUID invalido {s:?}: {e}")))?;
        Ok(Uuid(b))
    }
}

impl Uuid256 {
    pub const NULO: Uuid256 = Uuid256([0u8; UUID256_LEN]);

    pub fn bytes(&self) -> &[u8; UUID256_LEN] {
        &self.0
    }

    pub fn de_bytes(b: [u8; UUID256_LEN]) -> Uuid256 {
        Uuid256(b)
    }

    /// 256 bits sorteados.
    pub fn aleatorio() -> Uuid256 {
        let mut b = [0u8; UUID256_LEN];
        sortear(&mut b);
        Uuid256(b)
    }

    /// Le 64 digitos hexadecimais. Aceita o prefixo `0x`, que e como hash de
    /// bloco costuma vir escrito.
    pub fn de_texto(s: &str) -> Result<Uuid256> {
        let limpo = s.trim().trim_start_matches("0x").trim_start_matches("0X");
        if limpo.len() != 64 {
            return Err(PhxError::Tipo(format!(
                "identificador de 256 bits precisa de 64 digitos hexadecimais, veio {}",
                limpo.len()
            )));
        }
        let mut b = [0u8; UUID256_LEN];
        hex_para(limpo, &mut b)
            .map_err(|e| PhxError::Tipo(format!("identificador invalido {s:?}: {e}")))?;
        Ok(Uuid256(b))
    }
}

/// Avanca o relogio logico e devolve (milissegundos, contador).
///
/// Tres casos, e o terceiro e o que garante a monotonia: se o contador estourou
/// dentro do mesmo milissegundo, empresta-se um milissegundo do futuro em vez
/// de repetir ou esperar. O id continua crescente e a geracao nunca bloqueia.
fn proximo_passo(agora: u64) -> (u64, u16) {
    let mut guarda = match RELOGIO.lock() {
        Ok(g) => g,
        // Mutex envenenado nao pode derrubar a geracao de id: o estado dele e
        // so um par de numeros, e seguir com o valor de dentro e seguro.
        Err(e) => e.into_inner(),
    };
    let (ultimo_ms, contador) = *guarda;

    let passo = if agora > ultimo_ms {
        let mut semente = [0u8; 2];
        sortear(&mut semente);
        (agora, u16::from_be_bytes(semente) & CONTADOR_SEMENTE)
    } else if contador < CONTADOR_MASCARA {
        (ultimo_ms, contador + 1)
    } else {
        (ultimo_ms + 1, 0)
    };

    *guarda = passo;
    passo
}

fn hex_para(s: &str, dst: &mut [u8]) -> std::result::Result<(), String> {
    let b = s.as_bytes();
    for (i, alvo) in dst.iter_mut().enumerate() {
        let hi = digito(b[i * 2])?;
        let lo = digito(b[i * 2 + 1])?;
        *alvo = (hi << 4) | lo;
    }
    Ok(())
}

fn digito(c: u8) -> std::result::Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        outro => Err(format!("caractere {:?} nao e hexadecimal", outro as char)),
    }
}

fn escrever_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for b in bytes {
        write!(f, "{b:02x}")?;
    }
    Ok(())
}

impl fmt::Display for Uuid {
    /// Forma canonica em minusculas, com hifens: e a que o RFC manda escrever.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        escrever_hex(&self.0[0..4], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[4..6], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[6..8], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[8..10], f)?;
        f.write_str("-")?;
        escrever_hex(&self.0[10..16], f)
    }
}

impl fmt::Display for Uuid256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        escrever_hex(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v7_tem_o_layout_do_rfc_9562() {
        // Vetor do apendice A.6 do RFC 9562: 2022-02-22T19:22:22.000Z.
        let ms = 0x017F_22E2_79B0u64;
        let u = Uuid::v7_em(ms, 0x0CC3, [0x18, 0xC4, 0xDC, 0x0C, 0x0C, 0x07, 0x39, 0x8F]);
        assert_eq!(u.to_string(), "017f22e2-79b0-7cc3-98c4-dc0c0c07398f");
        assert_eq!(u.versao(), 7);
        assert_eq!(u.instante_ms(), Some(ms as i64));
        // Variante: os dois bits altos do byte 8 sao 0b10.
        assert_eq!(u.0[8] & 0xC0, 0x80);
    }

    #[test]
    fn v4_declara_versao_e_variante() {
        let u = Uuid::v4();
        assert_eq!(u.versao(), 4);
        assert_eq!(u.0[8] & 0xC0, 0x80);
        assert_eq!(u.instante_ms(), None, "v4 nao tem relogio para ler");
    }

    #[test]
    fn v7_nunca_repete_nem_anda_para_tras() {
        // O caso que interessa: milhares de ids no mesmo milissegundo.
        let mut anterior = Uuid::v7();
        for i in 0..20_000 {
            let u = Uuid::v7();
            assert!(u > anterior, "id {i} nao cresceu: {anterior} depois {u}",);
            anterior = u;
        }
    }

    #[test]
    fn comparar_bytes_e_comparar_tempo() {
        // E disto que o .ndx depende: memcmp na ordem certa.
        let a = Uuid::v7_em(1_000, 0, [0xFF; 8]);
        let b = Uuid::v7_em(2_000, 0, [0x00; 8]);
        assert!(a.0 < b.0, "o mais antigo tem de ordenar primeiro");
        assert!(a < b);
    }

    #[test]
    fn contador_estourado_empresta_do_futuro() {
        // Com o contador no teto, o proximo passo anda um milissegundo em vez
        // de repetir -- senao dois ids sairiam iguais.
        *RELOGIO.lock().unwrap() = (5_000, CONTADOR_MASCARA);
        let (ms, c) = proximo_passo(5_000);
        assert_eq!((ms, c), (5_001, 0));
    }

    #[test]
    fn texto_vai_e_volta() {
        let u = Uuid::v7();
        assert_eq!(Uuid::de_texto(&u.to_string()).unwrap(), u);
        // Sem hifens e entre chaves tambem entram.
        let s = u.to_string();
        let sem = s.replace('-', "");
        assert_eq!(Uuid::de_texto(&sem).unwrap(), u);
        assert_eq!(Uuid::de_texto(&format!("{{{s}}}")).unwrap(), u);
        assert_eq!(Uuid::de_texto(&s.to_uppercase()).unwrap(), u);
    }

    #[test]
    fn texto_torto_e_recusado() {
        for ruim in [
            "",
            "nao-e-uuid",
            "017f22e2-79b0-7cc3-98c4-dc0c0c07398",
            "zz",
        ] {
            assert!(Uuid::de_texto(ruim).is_err(), "aceitou {ruim:?}");
        }
    }

    #[test]
    fn nulo_se_escreve_todo_zero() {
        assert_eq!(
            Uuid::NULO.to_string(),
            "00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn id256_cabe_um_sha256() {
        // O hash do vetor classico "abc" do FIPS 180-4, que a suite do
        // hash.rs ja confere. Aqui o que se testa e o transporte.
        let hex = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let id = Uuid256::de_texto(hex).unwrap();
        assert_eq!(id.to_string(), hex);
        assert_eq!(Uuid256::de_texto(&format!("0x{hex}")).unwrap(), id);
    }

    #[test]
    fn id256_torto_e_recusado() {
        assert!(Uuid256::de_texto("ba7816bf").is_err());
        assert!(Uuid256::de_texto(&"z".repeat(64)).is_err());
    }

    #[test]
    fn id256_aleatorio_nao_repete() {
        let a = Uuid256::aleatorio();
        let b = Uuid256::aleatorio();
        assert_ne!(a, b);
        assert_ne!(a, Uuid256::NULO);
    }

    #[test]
    fn sorteio_de_reserva_nao_devolve_zeros() {
        // O caminho de Windows tem de produzir bytes de verdade.
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];
        misturar(&mut a);
        misturar(&mut b);
        assert_ne!(a, [0u8; 16]);
        assert_ne!(a, b);
    }
}
