# CRC slice-by-16 e os testes de referencia
# 29/08 05:37

import io
p='crates/phxsql-core/src/crc.rs'
s=io.open(p,encoding='utf-8').read()

velho = '''/// Oito tabelas: a coluna `k` guarda o CRC de um byte deslocado `k` posicoes.
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

static TABELAS8: [[u32; 256]; 8] = build_tabelas8();'''

novo = '''/// Numero de tabelas, e portanto de bytes consumidos por volta.
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

static TABELAS: [[u32; 256]; FATIAS] = build_tabelas();'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2 = '''    // Oito bytes por volta enquanto der.
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
    }'''
novo2 = '''    // Dezesseis bytes por volta enquanto der. So a primeira palavra leva o
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
    }'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
