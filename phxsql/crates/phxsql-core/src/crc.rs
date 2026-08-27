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

/// CRC-32 de um bloco.
pub fn crc32(data: &[u8]) -> u32 {
    crc32_with(0, data)
}

/// CRC-32 encadeado, para calcular sobre varios blocos.
pub fn crc32_with(seed: u32, data: &[u8]) -> u32 {
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
    fn encadeado_igual_a_bloco_unico() {
        let inteiro = crc32(b"cadastroClientes");
        let parcial = crc32_with(crc32(b"cadastro"), b"Clientes");
        assert_eq!(inteiro, parcial);
    }
}
