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

/// Numero de tabelas, e portanto de bytes consumidos por volta.
///
/// Comecou em 8 e foi para 16 depois da leitura do InnoDB: eles usam a
/// instrucao de maquina (`ut/crc32.cc:794`), que nao cabe aqui -- ela usa outro
/// polinomio, e mudar de polinomio invalidaria todo arquivo ja gravado. Mas a
/// mesma tabela com o dobro de colunas cabe, e da a mesma resposta.
const FATIAS: usize = 16;

/// Uma tabela por posicao: a coluna `k` guarda o CRC de um byte deslocado `k`.
///
/// Existe por medicao, nao por gosto. O laco byte a byte tem uma dependencia
/// serial -- cada volta precisa do `crc` da anterior para indexar a tabela --,
/// e isso o prende a uma leitura de memoria por byte. Com varias tabelas os
/// bytes de uma volta sao consultados em paralelo pelo processador e so os XOR
/// do fim dependem uns dos outros. Quanto mais colunas, mais trabalho
/// independente para a maquina despachar de uma vez.
///
/// A conta que motivou: cada pagina do `.ndx` passa inteira pelo CRC em toda
/// leitura e em toda gravacao. Byte a byte dava 10,0 us por pagina; com oito
/// tabelas, 2,3; com dezesseis, 1,8. O preco e 16 KiB de tabela estatica em vez
/// de 8 KiB -- e ela e const, entao nasce pronta no binario.
const fn build_tabelas() -> [[u32; 256]; FATIAS] {
    let mut t = [[0u32; 256]; FATIAS];
    t[0] = build_table();
    let mut k = 1;
    while k < FATIAS {
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

static TABELAS: [[u32; 256]; FATIAS] = build_tabelas();

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

    // Dezesseis bytes por volta enquanto der. So a primeira palavra leva o
    // `crc` de entrada; as outras tres sao independentes dele, e e dai que vem
    // o paralelismo.
    while resto.len() >= FATIAS {
        let p: [u8; FATIAS] = resto[..FATIAS].try_into().unwrap();
        let a = crc ^ u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
        let b = u32::from_le_bytes([p[4], p[5], p[6], p[7]]);
        let c = u32::from_le_bytes([p[8], p[9], p[10], p[11]]);
        let d = u32::from_le_bytes([p[12], p[13], p[14], p[15]]);
        crc = TABELAS[15][(a & 0xFF) as usize]
            ^ TABELAS[14][((a >> 8) & 0xFF) as usize]
            ^ TABELAS[13][((a >> 16) & 0xFF) as usize]
            ^ TABELAS[12][((a >> 24) & 0xFF) as usize]
            ^ TABELAS[11][(b & 0xFF) as usize]
            ^ TABELAS[10][((b >> 8) & 0xFF) as usize]
            ^ TABELAS[9][((b >> 16) & 0xFF) as usize]
            ^ TABELAS[8][((b >> 24) & 0xFF) as usize]
            ^ TABELAS[7][(c & 0xFF) as usize]
            ^ TABELAS[6][((c >> 8) & 0xFF) as usize]
            ^ TABELAS[5][((c >> 16) & 0xFF) as usize]
            ^ TABELAS[4][((c >> 24) & 0xFF) as usize]
            ^ TABELAS[3][(d & 0xFF) as usize]
            ^ TABELAS[2][((d >> 8) & 0xFF) as usize]
            ^ TABELAS[1][((d >> 16) & 0xFF) as usize]
            ^ TABELAS[0][((d >> 24) & 0xFF) as usize];
        resto = &resto[FATIAS..];
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
    fn a_versao_rapida_concorda_com_o_laco_byte_a_byte() {
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
