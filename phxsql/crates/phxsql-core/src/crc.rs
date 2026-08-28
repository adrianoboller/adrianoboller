//! CRC-32 (IEEE 802.3, refletido, polinomio 0xEDB88320).
//!
//! Implementacao propria para manter o PhxSql sem dependencias externas:
//! o projeto precisa compilar totalmente offline.

const fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

static TABLE: [u32; 256] = build_table();

/// Oito tabelas: a coluna `k` guarda o CRC de um byte deslocado `k` posicoes.
///
/// Existe por medicao, nao por gosto. O laco byte a byte tem uma dependencia
/// serial -- cada volta precisa do `crc` da anterior para indexar a tabela --,
/// e isso o prende a uma leitura de memoria por byte. Com oito tabelas os oito
/// bytes de uma palavra sao consultados em paralelo pelo processador e so os
/// XOR no fim dependem uns dos outros.
///
/// A conta que motivou: cada pagina do `.ndx` passa inteira pelo CRC em toda
/// leitura e em toda gravacao, e a insercao toca ~17 paginas por linha. Byte a
/// byte isso dava 10,0 us por pagina; assim da 2,3 us.
const fn build_tabelas8() -> [[u32; 256]; 8] {
    let mut t = [[0u32; 256]; 8];
    t[0] = build_table();
    let mut k = 1;
    while k < 8 {
        let mut i = 0;
        while i < 256 {
            let anterior = t[k - 1][i];
            t[k][i] = (anterior >> 8) ^ t[0][(anterior & 0xFF) as usize];
            i += 1;
        }
        k += 1;
    }
    t
}

static TABELAS8: [[u32; 256]; 8] = build_tabelas8();

/// CRC-32 de um bloco.
pub fn crc32(data: &[u8]) -> u32 {
    crc32_with(0, data)
}

/// CRC-32 encadeado, para calcular sobre varios blocos.
///
/// Mesmo polinomio e mesmo resultado do laco byte a byte -- ha teste que
/// compara os dois em todo tamanho de 0 a 300 bytes, com e sem semente.
pub fn crc32_with(seed: u32, data: &[u8]) -> u32 {
    let mut crc = !seed;
    let mut resto = data;

    // Oito bytes por volta enquanto der.
    while resto.len() >= 8 {
        let p: [u8; 8] = resto[..8].try_into().unwrap();
        let a = crc ^ u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
        let b = u32::from_le_bytes([p[4], p[5], p[6], p[7]]);
        crc = TABELAS8[7][(a & 0xFF) as usize]
            ^ TABELAS8[6][((a >> 8) & 0xFF) as usize]
            ^ TABELAS8[5][((a >> 16) & 0xFF) as usize]
            ^ TABELAS8[4][((a >> 24) & 0xFF) as usize]
            ^ TABELAS8[3][(b & 0xFF) as usize]
            ^ TABELAS8[2][((b >> 8) & 0xFF) as usize]
            ^ TABELAS8[1][((b >> 16) & 0xFF) as usize]
            ^ TABELAS8[0][((b >> 24) & 0xFF) as usize];
        resto = &resto[8..];
    }

    // A sobra, byte a byte.
    for &byte in resto {
        crc = TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// O laco byte a byte, guardado para o teste poder conferir contra ele.
///
/// E a definicao de referencia: se as duas divergirem, quem esta errada e a
/// rapida. Um CRC que muda de valor invalida todo `.reg`, `.ndx`, `.bin`,
/// `.memo` e `.log` ja gravado.
#[cfg(test)]
fn crc32_byte_a_byte(seed: u32, data: &[u8]) -> u32 {
    let mut crc = !seed;
    for &byte in data {
        crc = TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vetores_conhecidos() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn slice8_concorda_com_o_laco_byte_a_byte() {
        // O teste que autoriza a troca. Um CRC diferente invalidaria todo
        // arquivo ja gravado, entao a prova tem de cobrir os cantos: tamanhos
        // menores que a palavra, do tamanho dela, e com sobra.
        let mut estado = 0x2545_F491_4F6C_DD1Du64;
        let mut dados: Vec<u8> = Vec::new();
        for tam in 0..=300usize {
            dados.clear();
            for _ in 0..tam {
                estado ^= estado << 13;
                estado ^= estado >> 7;
                estado ^= estado << 17;
                dados.push((estado >> 24) as u8);
            }
            for semente in [0u32, 1, 0xFFFF_FFFF, 0x1234_5678] {
                assert_eq!(
                    crc32_with(semente, &dados),
                    crc32_byte_a_byte(semente, &dados),
                    "divergiu com {tam} bytes e semente {semente:#x}"
                );
            }
        }
    }

    #[test]
    fn pagina_inteira_bate() {
        // O caso real: uma pagina do `.ndx`.
        let pagina: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(31)) as u8).collect();
        assert_eq!(crc32(&pagina), crc32_byte_a_byte(0, &pagina));
    }

    #[test]
    fn encadeado_igual_a_bloco_unico() {
        let inteiro = crc32(b"cadastroClientes");
        let parcial = crc32_with(crc32(b"cadastro"), b"Clientes");
        assert_eq!(inteiro, parcial);
    }
}
