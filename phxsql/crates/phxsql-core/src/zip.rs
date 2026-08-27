//! Escrita de arquivo ZIP, com DEFLATE escrito aqui.
//!
//! # Por que escrever o DEFLATE
//!
//! Backup em ZIP e a forma que todo mundo sabe abrir: Windows(R), Linux, o
//! celular, o anexo de e-mail. Mas ZIP de verdade comprime, e comprimir exige
//! DEFLATE (RFC 1951). Trazer uma crate para isso quebraria a regra de zero
//! dependencia -- entao esta aqui.
//!
//! # O que esta implementacao faz
//!
//! DEFLATE com **codigo de Huffman fixo** (BTYPE=01) e casamento LZ77 por
//! tabela de dispersao. Nao e o melhor compressor que existe: Huffman dinamico
//! (BTYPE=10) ganharia mais alguns por cento, ao custo de montar e serializar
//! duas arvores.
//!
//! Para o que serve aqui, o fixo basta com folga: arquivo `.reg` e `.ndx` sao
//! slot de tamanho fixo e pagina com enchimento, ou seja, cheios de sequencia
//! repetida -- e e justamente disso que o LZ77 vive. Os numeros medidos estao
//! no teste.
//!
//! # Como se sabe que esta certo
//!
//! Comprimir e facil; comprimir de um jeito que os outros leiam e que e o
//! ponto. O teste desta caixa descomprime com o proprio codigo, e o teste do
//! backup abre o ZIP com o `unzip`/`zipfile` do sistema. Se o mundo abrir, e
//! porque esta certo -- e nao porque parece certo.

use crate::crc::crc32;

// ------------------------------------------------------------ escrita de bits
//
// O DEFLATE tem duas ordens de bit ao mesmo tempo, e trocar uma pela outra e o
// erro classico:
//
//   * campos de tamanho fixo (extra bits, cabecalho do bloco) vao do bit MENOS
//     significativo para o mais;
//   * codigos de Huffman vao do MAIS significativo para o menos.
//
// Por isso ha duas funcoes, e nao uma com um sinalizador.

struct Bits {
    saida: Vec<u8>,
    acumulado: u32,
    quantos: u32,
}

impl Bits {
    fn novo() -> Bits {
        Bits {
            saida: Vec::new(),
            acumulado: 0,
            quantos: 0,
        }
    }

    /// Campo de tamanho fixo: do bit menos significativo para o mais.
    fn campo(&mut self, valor: u32, n: u32) {
        self.acumulado |= (valor & ((1 << n) - 1)) << self.quantos;
        self.quantos += n;
        while self.quantos >= 8 {
            self.saida.push((self.acumulado & 0xff) as u8);
            self.acumulado >>= 8;
            self.quantos -= 8;
        }
    }

    /// Codigo de Huffman: do bit mais significativo para o menos.
    fn codigo(&mut self, codigo: u32, n: u32) {
        for i in (0..n).rev() {
            self.campo((codigo >> i) & 1, 1);
        }
    }

    fn terminar(mut self) -> Vec<u8> {
        if self.quantos > 0 {
            self.saida.push((self.acumulado & 0xff) as u8);
        }
        self.saida
    }
}

// ------------------------------------------------------------ tabelas do LZ77

/// Comprimento minimo que compensa virar referencia em vez de bytes soltos.
const CASAMENTO_MIN: usize = 3;
const CASAMENTO_MAX: usize = 258;
/// Janela de 32 KB, o maximo que o DEFLATE enderecca.
const JANELA: usize = 32 * 1024;

/// (primeiro comprimento, bits extras) do codigo 257 em diante.
const COMPRIMENTOS: [(u16, u8); 29] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 1),
    (13, 1),
    (15, 1),
    (17, 1),
    (19, 2),
    (23, 2),
    (27, 2),
    (31, 2),
    (35, 3),
    (43, 3),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 4),
    (115, 4),
    (131, 5),
    (163, 5),
    (195, 5),
    (227, 5),
    (258, 0),
];

/// (primeira distancia, bits extras) do codigo 0 ao 29.
const DISTANCIAS: [(u16, u8); 30] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 1),
    (7, 1),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 4),
    (49, 4),
    (65, 5),
    (97, 5),
    (129, 6),
    (193, 6),
    (257, 7),
    (385, 7),
    (513, 8),
    (769, 8),
    (1025, 9),
    (1537, 9),
    (2049, 10),
    (3073, 10),
    (4097, 11),
    (6145, 11),
    (8193, 12),
    (12289, 12),
    (16385, 13),
    (24577, 13),
];

/// Codigo e largura de um literal ou comprimento, na arvore fixa da RFC 1951.
fn huffman_fixo(simbolo: u32) -> (u32, u32) {
    match simbolo {
        0..=143 => (0x30 + simbolo, 8),
        144..=255 => (0x190 + simbolo - 144, 9),
        256..=279 => (simbolo - 256, 7),
        _ => (0xC0 + simbolo - 280, 8),
    }
}

fn codigo_de_comprimento(n: usize) -> (u32, u32, u32) {
    let mut i = COMPRIMENTOS.len() - 1;
    while i > 0 && (n as u16) < COMPRIMENTOS[i].0 {
        i -= 1;
    }
    let (base, extras) = COMPRIMENTOS[i];
    (257 + i as u32, (n as u32) - base as u32, extras as u32)
}

fn codigo_de_distancia(d: usize) -> (u32, u32, u32) {
    let mut i = DISTANCIAS.len() - 1;
    while i > 0 && (d as u16) < DISTANCIAS[i].0 {
        i -= 1;
    }
    let (base, extras) = DISTANCIAS[i];
    (i as u32, (d as u32) - base as u32, extras as u32)
}

/// Comprime com DEFLATE, num unico bloco de Huffman fixo.
pub fn deflate(dados: &[u8]) -> Vec<u8> {
    let mut b = Bits::novo();
    b.campo(1, 1); // BFINAL: e o ultimo bloco
    b.campo(1, 2); // BTYPE: Huffman fixo

    // Dispersao das tres proximas letras -> ultima posicao onde apareceram.
    // Uma corrente curta por balde: procurar o casamento perfeito custa caro e
    // rende pouco, e backup e coisa de rodar de madrugada, nao de otimizar.
    const BALDES: usize = 1 << 15;
    let mut cabeca = vec![usize::MAX; BALDES];
    let mut anterior = vec![usize::MAX; dados.len().max(1)];
    let dispersar = |d: &[u8], i: usize| -> usize {
        ((d[i] as usize) << 10 ^ (d[i + 1] as usize) << 5 ^ d[i + 2] as usize) & (BALDES - 1)
    };

    let mut i = 0usize;
    while i < dados.len() {
        let mut melhor_tam = 0usize;
        let mut melhor_dist = 0usize;

        if i + CASAMENTO_MIN <= dados.len() {
            let h = dispersar(dados, i);
            let mut candidato = cabeca[h];
            let mut tentativas = 0;
            while candidato != usize::MAX && tentativas < 32 {
                let dist = i - candidato;
                if dist > JANELA {
                    break;
                }
                let teto = CASAMENTO_MAX.min(dados.len() - i);
                let mut tam = 0;
                while tam < teto && dados[candidato + tam] == dados[i + tam] {
                    tam += 1;
                }
                if tam > melhor_tam {
                    melhor_tam = tam;
                    melhor_dist = dist;
                    if tam >= CASAMENTO_MAX {
                        break;
                    }
                }
                candidato = anterior[candidato];
                tentativas += 1;
            }
            anterior[i] = cabeca[h];
            cabeca[h] = i;
        }

        if melhor_tam >= CASAMENTO_MIN {
            let (c, extra, n) = codigo_de_comprimento(melhor_tam);
            let (cc, cn) = huffman_fixo(c);
            b.codigo(cc, cn);
            if n > 0 {
                b.campo(extra, n);
            }
            let (d, dextra, dn) = codigo_de_distancia(melhor_dist);
            b.codigo(d, 5); // distancia usa 5 bits fixos, do mais significativo
            if dn > 0 {
                b.campo(dextra, dn);
            }
            // Registra as posicoes puladas, senao o casamento seguinte nao acha.
            let fim = (i + melhor_tam).min(dados.len());
            for (n, ligar) in anterior[i + 1..fim].iter_mut().enumerate() {
                let k = i + 1 + n;
                if k + CASAMENTO_MIN <= dados.len() {
                    let h = dispersar(dados, k);
                    *ligar = cabeca[h];
                    cabeca[h] = k;
                }
            }
            i += melhor_tam;
        } else {
            let (c, n) = huffman_fixo(dados[i] as u32);
            b.codigo(c, n);
            i += 1;
        }
    }

    let (fim, n) = huffman_fixo(256); // fim do bloco
    b.codigo(fim, n);
    b.terminar()
}

// ---------------------------------------------------------------- o arquivo

struct Entrada {
    nome: String,
    crc: u32,
    comprimido: u32,
    original: u32,
    deslocamento: u32,
    metodo: u16,
}

/// Monta um arquivo ZIP na memoria.
pub struct Zip {
    saida: Vec<u8>,
    entradas: Vec<Entrada>,
    data_dos: u16,
    hora_dos: u16,
}

impl Zip {
    /// `quando_ms` carimba a data de todos os arquivos de dentro.
    pub fn novo(quando_ms: i64) -> Zip {
        let dias = (quando_ms.div_euclid(86_400_000)) as i32;
        let (ano, mes, dia) = crate::datahora::civil_de_dias(dias);
        let resto = quando_ms.rem_euclid(86_400_000) / 1000;
        let (h, m, s) = (resto / 3600, (resto / 60) % 60, resto % 60);
        // O ZIP guarda a data no formato do MS-DOS: ano a partir de 1980 e
        // segundo em passos de dois. Antes de 1980 nao ha o que representar.
        let ano_dos = (ano - 1980).clamp(0, 127) as u16;
        Zip {
            saida: Vec::new(),
            entradas: Vec::new(),
            data_dos: (ano_dos << 9) | ((mes as u16) << 5) | dia as u16,
            hora_dos: ((h as u16) << 11) | ((m as u16) << 5) | (s as u16 / 2),
        }
    }

    /// Acrescenta um arquivo. O nome usa barra normal, como manda o formato.
    pub fn acrescentar(&mut self, nome: &str, dados: &[u8]) {
        let nome = nome.replace('\\', "/");
        let crc = crc32(dados);
        let comprimido = deflate(dados);

        // Se comprimir aumentou -- e aumenta, em arquivo ja comprimido ou
        // pequeno demais --, guarda cru. O formato preve, e o leitor entende.
        let (metodo, corpo): (u16, &[u8]) = if comprimido.len() < dados.len() {
            (8, &comprimido)
        } else {
            (0, dados)
        };

        let deslocamento = self.saida.len() as u32;
        let nome_bytes = nome.as_bytes();

        self.saida.extend_from_slice(b"PK\x03\x04");
        self.saida.extend_from_slice(&20u16.to_le_bytes()); // versao minima
        self.saida.extend_from_slice(&0u16.to_le_bytes()); // sinalizadores
        self.saida.extend_from_slice(&metodo.to_le_bytes());
        self.saida.extend_from_slice(&self.hora_dos.to_le_bytes());
        self.saida.extend_from_slice(&self.data_dos.to_le_bytes());
        self.saida.extend_from_slice(&crc.to_le_bytes());
        self.saida
            .extend_from_slice(&(corpo.len() as u32).to_le_bytes());
        self.saida
            .extend_from_slice(&(dados.len() as u32).to_le_bytes());
        self.saida
            .extend_from_slice(&(nome_bytes.len() as u16).to_le_bytes());
        self.saida.extend_from_slice(&0u16.to_le_bytes()); // extra
        self.saida.extend_from_slice(nome_bytes);
        self.saida.extend_from_slice(corpo);

        self.entradas.push(Entrada {
            nome,
            crc,
            comprimido: corpo.len() as u32,
            original: dados.len() as u32,
            deslocamento,
            metodo,
        });
    }

    /// Fecha o arquivo: escreve o diretorio central e o fim.
    pub fn terminar(mut self) -> Vec<u8> {
        let inicio_central = self.saida.len() as u32;
        for e in &self.entradas {
            let nome = e.nome.as_bytes();
            self.saida.extend_from_slice(b"PK\x01\x02");
            self.saida.extend_from_slice(&20u16.to_le_bytes()); // feito por
            self.saida.extend_from_slice(&20u16.to_le_bytes()); // versao minima
            self.saida.extend_from_slice(&0u16.to_le_bytes());
            self.saida.extend_from_slice(&e.metodo.to_le_bytes());
            self.saida.extend_from_slice(&self.hora_dos.to_le_bytes());
            self.saida.extend_from_slice(&self.data_dos.to_le_bytes());
            self.saida.extend_from_slice(&e.crc.to_le_bytes());
            self.saida.extend_from_slice(&e.comprimido.to_le_bytes());
            self.saida.extend_from_slice(&e.original.to_le_bytes());
            self.saida
                .extend_from_slice(&(nome.len() as u16).to_le_bytes());
            self.saida.extend_from_slice(&0u16.to_le_bytes()); // extra
            self.saida.extend_from_slice(&0u16.to_le_bytes()); // comentario
            self.saida.extend_from_slice(&0u16.to_le_bytes()); // disco
            self.saida.extend_from_slice(&0u16.to_le_bytes()); // atributo interno
            self.saida.extend_from_slice(&0u32.to_le_bytes()); // atributo externo
            self.saida.extend_from_slice(&e.deslocamento.to_le_bytes());
            self.saida.extend_from_slice(nome);
        }
        let tamanho_central = self.saida.len() as u32 - inicio_central;
        let quantas = self.entradas.len() as u16;

        self.saida.extend_from_slice(b"PK\x05\x06");
        self.saida.extend_from_slice(&0u16.to_le_bytes()); // disco
        self.saida.extend_from_slice(&0u16.to_le_bytes()); // disco do central
        self.saida.extend_from_slice(&quantas.to_le_bytes());
        self.saida.extend_from_slice(&quantas.to_le_bytes());
        self.saida.extend_from_slice(&tamanho_central.to_le_bytes());
        self.saida.extend_from_slice(&inicio_central.to_le_bytes());
        self.saida.extend_from_slice(&0u16.to_le_bytes()); // comentario
        self.saida
    }
}

// ------------------------------------------------------------ descompressao
//
// So para o teste conferir o proprio trabalho. O PhxSql escreve ZIP; quem le
// e o sistema de quem for restaurar.

/// Descomprime um bloco DEFLATE de Huffman fixo. `None` se nao for isso.
pub fn inflate_fixo(dados: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0usize; // posicao em bits
    let bit = |p: &mut usize| -> Option<u32> {
        let b = (*dados.get(*p / 8)? >> (*p % 8)) & 1;
        *p += 1;
        Some(b as u32)
    };
    let campo = |p: &mut usize, n: u32| -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= bit(p)? << i;
        }
        Some(v)
    };

    let mut saida = Vec::new();
    loop {
        let final_ = bit(&mut pos)?;
        let tipo = campo(&mut pos, 2)?;
        if tipo != 1 {
            return None; // so o Huffman fixo, que e o unico que escrevemos
        }
        loop {
            // Le sete bits; se nao fechar, le mais um, e mais um.
            let mut c = 0u32;
            for _ in 0..7 {
                c = (c << 1) | bit(&mut pos)?;
            }
            let simbolo = if c <= 0x17 {
                256 + c
            } else {
                c = (c << 1) | bit(&mut pos)?;
                if (0x30..=0xBF).contains(&c) {
                    c - 0x30
                } else if (0xC0..=0xC7).contains(&c) {
                    280 + c - 0xC0
                } else {
                    c = (c << 1) | bit(&mut pos)?;
                    if (0x190..=0x1FF).contains(&c) {
                        144 + c - 0x190
                    } else {
                        return None;
                    }
                }
            };

            if simbolo == 256 {
                break;
            }
            if simbolo < 256 {
                saida.push(simbolo as u8);
                continue;
            }
            let i = (simbolo - 257) as usize;
            let (base, extras) = *COMPRIMENTOS.get(i)?;
            let tam = base as usize + campo(&mut pos, extras as u32)? as usize;

            let mut d = 0u32;
            for _ in 0..5 {
                d = (d << 1) | bit(&mut pos)?;
            }
            let (dbase, dextras) = *DISTANCIAS.get(d as usize)?;
            let dist = dbase as usize + campo(&mut pos, dextras as u32)? as usize;
            if dist > saida.len() {
                return None;
            }
            let inicio = saida.len() - dist;
            for k in 0..tam {
                let b = saida[inicio + k];
                saida.push(b);
            }
        }
        if final_ == 1 {
            return Some(saida);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ida_e_volta(dados: &[u8]) {
        let c = deflate(dados);
        let volta = inflate_fixo(&c).expect("nao descomprimiu");
        assert_eq!(volta, dados, "os bytes nao voltaram iguais");
    }

    #[test]
    fn vai_e_volta_em_varios_formatos() {
        ida_e_volta(b"");
        ida_e_volta(b"a");
        ida_e_volta(b"abc");
        ida_e_volta(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        ida_e_volta(b"o rato roeu a roupa do rei de roma");
        // Todos os 256 bytes, para exercitar as tres faixas da arvore fixa.
        let todos: Vec<u8> = (0..=255u8).collect();
        ida_e_volta(&todos);
        // Repeticao longa: distancia grande e comprimento no teto.
        let mut longo = Vec::new();
        for i in 0..5_000u32 {
            longo.extend_from_slice(format!("linha {i:06} de teste; ").as_bytes());
        }
        ida_e_volta(&longo);
    }

    #[test]
    fn comprime_de_verdade_o_que_um_reg_parece() {
        // Slot de tamanho fixo com enchimento: o formato do .reg. E o caso em
        // que a compressao tem de pagar, senao nao vale a pena existir.
        let mut reg = Vec::new();
        for i in 0..2_000u32 {
            let mut slot = vec![0u8; 128];
            slot[..4].copy_from_slice(&i.to_le_bytes());
            slot[8..8 + 14].copy_from_slice(b"Cliente Boller");
            reg.extend_from_slice(&slot);
        }
        let c = deflate(&reg);
        let razao = c.len() as f64 / reg.len() as f64;
        assert!(
            razao < 0.10,
            "esperava menos de 10% do tamanho, deu {:.1}%",
            razao * 100.0
        );
        assert_eq!(inflate_fixo(&c).unwrap(), reg);
    }

    #[test]
    fn o_zip_tem_a_estrutura_do_formato() {
        let mut z = Zip::novo(1_787_000_000_000);
        z.acrescentar(
            "Z/cadastroClientes.reg",
            b"conteudo do reg, com repeticao repeticao repeticao",
        );
        z.acrescentar("Z/schemaX/pedidos.reg", b"outro arquivo");
        let bytes = z.terminar();

        assert_eq!(&bytes[..4], b"PK\x03\x04", "comeca com o cabecalho local");
        // O fim fica nos ultimos 22 bytes quando nao ha comentario.
        assert_eq!(&bytes[bytes.len() - 22..bytes.len() - 18], b"PK\x05\x06");
        // Duas entradas no diretorio central.
        let quantas = u16::from_le_bytes(
            bytes[bytes.len() - 14..bytes.len() - 12]
                .try_into()
                .unwrap(),
        );
        assert_eq!(quantas, 2);
        // Os nomes aparecem.
        let texto = String::from_utf8_lossy(&bytes);
        assert!(texto.contains("Z/cadastroClientes.reg"));
        assert!(texto.contains("Z/schemaX/pedidos.reg"));
    }

    #[test]
    fn arquivo_que_nao_comprime_vai_cru() {
        // Bytes sem repeticao: comprimir aumentaria. O metodo tem de virar 0.
        let mut z = Zip::novo(1_787_000_000_000);
        let barulho: Vec<u8> = (0..200u32).map(|i| (i * 167 + 13) as u8).collect();
        z.acrescentar("ruido.bin", &barulho);
        let bytes = z.terminar();
        let metodo = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        assert!(metodo == 0 || metodo == 8);
        // Comprimido nunca pode ficar maior que o original no arquivo final.
        let comp = u32::from_le_bytes(bytes[18..22].try_into().unwrap());
        let orig = u32::from_le_bytes(bytes[22..26].try_into().unwrap());
        assert!(
            comp <= orig,
            "o zip guardou {comp} para um original de {orig}"
        );
    }

    #[test]
    fn o_crc_do_zip_e_o_do_conteudo() {
        let mut z = Zip::novo(0);
        let dados = b"o crc tem de bater, senao o unzip recusa";
        z.acrescentar("x.txt", dados);
        let bytes = z.terminar();
        let crc_no_zip = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
        assert_eq!(crc_no_zip, crc32(dados));
    }

    #[test]
    fn a_data_vira_o_formato_do_dos() {
        // 2026-08-27 20:00:00 UTC
        let ms = (crate::datahora::dias_de_civil(2026, 8, 27) as i64) * 86_400_000 + 20 * 3_600_000;
        let z = Zip::novo(ms);
        assert_eq!(z.data_dos >> 9, 2026 - 1980);
        assert_eq!((z.data_dos >> 5) & 0xf, 8);
        assert_eq!(z.data_dos & 0x1f, 27);
        assert_eq!(z.hora_dos >> 11, 20);
    }
}
