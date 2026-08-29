//! Arquivo ZIP: escrita e leitura, com o DEFLATE escrito aqui.
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
//! **Escreve** DEFLATE com codigo de Huffman fixo (BTYPE=01) e casamento LZ77
//! por tabela de dispersao. Nao e o melhor compressor que existe: Huffman
//! dinamico (BTYPE=10) ganharia mais alguns por cento, ao custo de montar e
//! serializar duas arvores.
//!
//! Para o que serve aqui, o fixo basta com folga: arquivo `.reg` e `.ndx` sao
//! slot de tamanho fixo e pagina com enchimento, ou seja, cheios de sequencia
//! repetida -- e e justamente disso que o LZ77 vive. Os numeros medidos estao
//! no teste.
//!
//! **Le** os tres tipos de bloco da RFC 1951 -- sem compressao, Huffman fixo e
//! Huffman dinamico --, e nao so o que escreve. A assimetria e de proposito: o
//! backup que volta pode ter passado pela mao de alguem, e quem abre um ZIP
//! para olhar e compacta de novo devolve Huffman DINAMICO, porque e o que todo
//! compressor do mundo emite. Ler so o proprio dialeto recusaria justamente o
//! arquivo que o operador acabou de conferir.
//!
//! # Como se sabe que esta certo
//!
//! Comprimir e facil; comprimir de um jeito que os outros leiam e que e o
//! ponto. O teste desta caixa descomprime com o proprio codigo, e o teste do
//! backup abre o ZIP com o `unzip`/`zipfile` do sistema. Se o mundo abrir, e
//! porque esta certo -- e nao porque parece certo.
//!
//! Do lado da leitura vale a regra oposta, e o vetor e que responde: o teste
//! traz bytes produzidos pela **zlib** (Huffman dinamico) e exige que saia o
//! texto certo. Ida e volta com o proprio compressor nao provaria nada sobre
//! ler o que os outros escrevem -- os dois lados poderiam estar errados juntos.

use std::io::{Read, Seek, SeekFrom};

use crate::crc::crc32;
use crate::error::{PhxError, Result};

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
// O PhxSql passou a LER o ZIP que escreve: e o que a restauracao precisa. Por
// isso o decodificador aqui nao e mais "so para o teste conferir o proprio
// trabalho" -- ele atende os tres tipos de bloco da RFC 1951, e nao so o
// Huffman fixo que o nosso compressor emite.
//
// Ler os tres custa umas cem linhas a mais e evita a armadilha obvia: quem
// baixa o backup, abre para olhar e compacta de novo produz Huffman DINAMICO,
// porque e o que todo compressor do mundo emite. Um leitor que so entende o
// que nos mesmos escrevemos recusaria justamente o arquivo que o operador
// acabou de conferir na mao.

/// Leitura bit a bit, do bit menos significativo para o mais -- a ordem em que
/// o DEFLATE guarda os campos de tamanho fixo.
struct LeitorDeBits<'a> {
    dados: &'a [u8],
    /// Posicao em BITS, e nao em bytes: o formato nao respeita a fronteira.
    pos: usize,
}

impl<'a> LeitorDeBits<'a> {
    fn novo(dados: &'a [u8]) -> LeitorDeBits<'a> {
        LeitorDeBits { dados, pos: 0 }
    }

    fn bit(&mut self) -> Option<u32> {
        let b = (*self.dados.get(self.pos / 8)? >> (self.pos % 8)) & 1;
        self.pos += 1;
        Some(b as u32)
    }

    fn campo(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.bit()? << i;
        }
        Some(v)
    }

    /// Anda ate a proxima fronteira de byte. O bloco sem compressao comeca ali.
    fn alinhar(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }

    fn bytes(&mut self, quantos: usize) -> Option<&'a [u8]> {
        let inicio = self.pos / 8;
        let fim = inicio.checked_add(quantos)?;
        let fatia = self.dados.get(inicio..fim)?;
        self.pos = fim * 8;
        Some(fatia)
    }
}

/// Uma arvore de Huffman CANONICA, guardada como a zlib guarda no `puff`:
/// quantos codigos existem de cada largura, e os simbolos em ordem.
///
/// Guardar assim, e nao como arvore de ponteiros, e o que deixa a decodificacao
/// ser uma conta -- sem alocar dentro do laco e sem montar uma tabela de 32 mil
/// entradas antes de descomprimir tres bytes.
struct Arvore {
    /// `contagem[n]` = quantos simbolos tem codigo de `n` bits.
    contagem: [u16; 16],
    /// Os simbolos, ordenados por largura e depois por valor.
    simbolos: Vec<u16>,
}

impl Arvore {
    /// Monta a arvore a partir da LARGURA do codigo de cada simbolo, que e a
    /// unica coisa que o DEFLATE transmite -- os codigos em si saem da regra
    /// canonica, igual dos dois lados.
    fn nova(larguras: &[u8]) -> Option<Arvore> {
        let mut contagem = [0u16; 16];
        for &l in larguras {
            if l as usize >= 16 {
                return None;
            }
            contagem[l as usize] += 1;
        }
        // Largura zero quer dizer "simbolo nao usado", e nao "codigo de zero
        // bits": sem esta linha ele entraria na conta e desalinharia tudo.
        contagem[0] = 0;

        let mut deslocamento = [0u16; 16];
        for n in 1..15 {
            deslocamento[n + 1] = deslocamento[n] + contagem[n];
        }
        let mut simbolos = vec![0u16; larguras.len()];
        for (s, &l) in larguras.iter().enumerate() {
            if l != 0 {
                let d = &mut deslocamento[l as usize];
                *simbolos.get_mut(*d as usize)? = s as u16;
                *d += 1;
            }
        }
        Some(Arvore { contagem, simbolos })
    }

    /// Le um codigo do fluxo e devolve o simbolo.
    ///
    /// Um bit por volta, da largura menor para a maior: o codigo canonico
    /// garante que, quando o valor lido cabe na faixa daquela largura, ele E o
    /// codigo -- e por isso nao ha ambiguidade sem tabela nenhuma.
    fn decodificar(&self, b: &mut LeitorDeBits) -> Option<u16> {
        let mut codigo = 0i32;
        let mut primeiro = 0i32;
        let mut indice = 0i32;
        for largura in 1..16 {
            codigo |= b.bit()? as i32;
            let quantos = self.contagem[largura] as i32;
            if codigo - quantos < primeiro {
                return self
                    .simbolos
                    .get((indice + (codigo - primeiro)) as usize)
                    .copied();
            }
            indice += quantos;
            primeiro = (primeiro + quantos) << 1;
            codigo <<= 1;
        }
        None
    }
}

/// As larguras da arvore fixa da RFC 1951, secao 3.2.6.
fn larguras_fixas() -> ([u8; 288], [u8; 30]) {
    let mut literais = [8u8; 288];
    for (s, l) in literais.iter_mut().enumerate() {
        *l = match s {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    (literais, [5u8; 30])
}

/// A ordem em que o bloco dinamico transmite as larguras do alfabeto de
/// larguras. Nao e caprichosa: poe primeiro as que quase sempre aparecem, para
/// as ultimas poderem ser omitidas.
const ORDEM_DAS_LARGURAS: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Descomprime um fluxo DEFLATE inteiro (RFC 1951): sem compressao, Huffman
/// fixo e Huffman dinamico. `None` quando o fluxo nao fecha.
pub fn inflate(dados: &[u8]) -> Option<Vec<u8>> {
    let mut b = LeitorDeBits::novo(dados);
    let mut saida: Vec<u8> = Vec::new();
    loop {
        let ultimo = b.bit()?;
        match b.campo(2)? {
            0 => {
                b.alinhar();
                let cabecalho = b.bytes(4)?;
                let tamanho = u16::from_le_bytes([cabecalho[0], cabecalho[1]]);
                let complemento = u16::from_le_bytes([cabecalho[2], cabecalho[3]]);
                // O tamanho vem duas vezes, a segunda invertida. E a unica
                // conferencia que o bloco cru tem, e ignora-la seria copiar
                // lixo achando que e dado.
                if tamanho != !complemento {
                    return None;
                }
                saida.extend_from_slice(b.bytes(tamanho as usize)?);
            }
            1 => {
                let (lit, dist) = larguras_fixas();
                simbolos(
                    &mut b,
                    &Arvore::nova(&lit)?,
                    &Arvore::nova(&dist)?,
                    &mut saida,
                )?;
            }
            2 => {
                let hlit = b.campo(5)? as usize + 257;
                let hdist = b.campo(5)? as usize + 1;
                let hclen = b.campo(4)? as usize + 4;
                if hlit > 288 || hdist > 32 {
                    return None;
                }
                let mut larguras_do_alfabeto = [0u8; 19];
                for &onde in ORDEM_DAS_LARGURAS.iter().take(hclen) {
                    larguras_do_alfabeto[onde] = b.campo(3)? as u8;
                }
                let alfabeto = Arvore::nova(&larguras_do_alfabeto)?;

                // As larguras das duas arvores vem numa lista so, comprimida
                // por repeticao -- e por isso sao lidas juntas e separadas
                // depois.
                let mut larguras = Vec::with_capacity(hlit + hdist);
                while larguras.len() < hlit + hdist {
                    match alfabeto.decodificar(&mut b)? {
                        s @ 0..=15 => larguras.push(s as u8),
                        16 => {
                            let anterior = *larguras.last()?;
                            for _ in 0..3 + b.campo(2)? {
                                larguras.push(anterior);
                            }
                        }
                        // 17 e 18 sao a MESMA coisa com faixas diferentes:
                        // "os proximos n simbolos nao tem codigo". O 17 conta
                        // ate 10 e o 18 ate 138.
                        17 => {
                            let quantos = larguras.len() + 3 + b.campo(3)? as usize;
                            larguras.resize(quantos, 0);
                        }
                        18 => {
                            let quantos = larguras.len() + 11 + b.campo(7)? as usize;
                            larguras.resize(quantos, 0);
                        }
                        _ => return None,
                    }
                }
                if larguras.len() != hlit + hdist {
                    return None; // a repeticao passou do fim: fluxo quebrado
                }
                let lit = Arvore::nova(&larguras[..hlit])?;
                let dist = Arvore::nova(&larguras[hlit..])?;
                simbolos(&mut b, &lit, &dist, &mut saida)?;
            }
            _ => return None, // BTYPE 3 nao existe
        }
        if ultimo == 1 {
            return Some(saida);
        }
    }
}

/// O laco de simbolos de um bloco comprimido: literal, fim de bloco ou
/// referencia para tras. E o mesmo para o Huffman fixo e para o dinamico --
/// so as arvores mudam.
fn simbolos(b: &mut LeitorDeBits, lit: &Arvore, dist: &Arvore, saida: &mut Vec<u8>) -> Option<()> {
    loop {
        let simbolo = lit.decodificar(b)?;
        match simbolo {
            0..=255 => saida.push(simbolo as u8),
            256 => return Some(()),
            _ => {
                let i = simbolo as usize - 257;
                let (base, extras) = *COMPRIMENTOS.get(i)?;
                let tam = base as usize + b.campo(extras as u32)? as usize;
                let d = dist.decodificar(b)? as usize;
                let (dbase, dextras) = *DISTANCIAS.get(d)?;
                let distancia = dbase as usize + b.campo(dextras as u32)? as usize;
                if distancia > saida.len() || distancia == 0 {
                    return None;
                }
                // Byte a byte de proposito: o DEFLATE permite a copia se
                // sobrepor a si mesma (distancia 1 e comprimento 100 = repetir
                // o mesmo byte cem vezes), e copiar a fatia de uma vez daria
                // outro resultado.
                let inicio = saida.len() - distancia;
                for k in 0..tam {
                    let byte = saida[inicio + k];
                    saida.push(byte);
                }
            }
        }
    }
}

/// Descomprime um bloco DEFLATE de Huffman fixo. `None` se nao for isso.
///
/// Continua com o contrato antigo -- recusa o que nao comeca com bloco fixo --,
/// mas o trabalho e do [`inflate`]. Dois decodificadores lado a lado seriam
/// duas verdades, e a segunda envelhece calada.
pub fn inflate_fixo(dados: &[u8]) -> Option<Vec<u8>> {
    let mut espia = LeitorDeBits::novo(dados);
    espia.bit()?;
    if espia.campo(2)? != 1 {
        return None;
    }
    inflate(dados)
}

// ---------------------------------------------------------- leitura do ZIP
//
// A contraparte do `Zip`: abrir o arquivo que ele escreveu e tirar de volta o
// que entrou. Le pelo DIRETORIO CENTRAL, no fim do arquivo, e nao varrendo os
// cabecalhos locais desde o comeco -- e por isso da para tirar UM arquivo de
// dentro de um backup de gigabytes sem ler o resto. E o que faz a tela
// conseguir mostrar o que tem dentro de dez backups sem ler dez backups.

/// Uma entrada do diretorio central.
#[derive(Debug, Clone)]
pub struct EntradaZip {
    pub nome: String,
    /// 0 = guardado cru, 8 = DEFLATE. Os dois que o nosso escritor emite.
    pub metodo: u16,
    pub comprimido: u64,
    pub original: u64,
    pub crc: u32,
    /// Onde comeca o cabecalho local desta entrada.
    deslocamento: u64,
}

/// Le um arquivo ZIP de qualquer fonte que saiba procurar posicao.
///
/// Generico em `F` para o teste poder abrir um `Cursor` de memoria e a
/// restauracao poder abrir um `File` de dez gigabytes sem carrega-lo.
pub struct LeitorZip<F> {
    fonte: F,
    entradas: Vec<EntradaZip>,
}

/// O comentario final do ZIP cabe em 65.535 bytes, e o registro de fim tem 22:
/// e o quanto se precisa reler do fim do arquivo para achar o `PK\x05\x06`.
const RABO_MAXIMO: u64 = 65_535 + 22;

impl<F: Read + Seek> LeitorZip<F> {
    /// Abre o arquivo e le so o diretorio central.
    pub fn abrir(mut fonte: F) -> Result<LeitorZip<F>> {
        let tamanho = fonte.seek(SeekFrom::End(0))?;
        if tamanho < 22 {
            return Err(PhxError::Corrompido(
                "arquivo pequeno demais para ser um ZIP".into(),
            ));
        }
        let quanto = RABO_MAXIMO.min(tamanho);
        fonte.seek(SeekFrom::Start(tamanho - quanto))?;
        let mut rabo = vec![0u8; quanto as usize];
        fonte.read_exact(&mut rabo)?;

        // De tras para a frente: a assinatura de fim pode aparecer DENTRO do
        // comentario, e a valida e sempre a ultima.
        let fim = (0..=rabo.len() - 22)
            .rev()
            .find(|&i| rabo[i..i + 4] == *b"PK\x05\x06")
            .ok_or_else(|| {
                PhxError::Corrompido("nao achei o fim do diretorio central: nao e um ZIP".into())
            })?;
        let eocd = &rabo[fim..];
        let quantas = u16::from_le_bytes([eocd[10], eocd[11]]) as usize;
        let tamanho_central = u32::from_le_bytes([eocd[12], eocd[13], eocd[14], eocd[15]]) as usize;
        let inicio_central = u32::from_le_bytes([eocd[16], eocd[17], eocd[18], eocd[19]]) as u64;
        if quantas == 0xffff || inicio_central == 0xffff_ffff {
            return Err(PhxError::Corrompido(
                "este ZIP e ZIP64, e o PhxSql nao escreve nem le esse formato".into(),
            ));
        }

        fonte.seek(SeekFrom::Start(inicio_central))?;
        let mut central = vec![0u8; tamanho_central];
        fonte.read_exact(&mut central)?;

        let mut entradas = Vec::with_capacity(quantas);
        let mut i = 0usize;
        while i + 46 <= central.len() && entradas.len() < quantas {
            if central[i..i + 4] != *b"PK\x01\x02" {
                return Err(PhxError::Corrompido(
                    "o diretorio central do ZIP nao tem a assinatura esperada".into(),
                ));
            }
            let u16em = |o: usize| u16::from_le_bytes([central[i + o], central[i + o + 1]]);
            let u32em = |o: usize| {
                u32::from_le_bytes([
                    central[i + o],
                    central[i + o + 1],
                    central[i + o + 2],
                    central[i + o + 3],
                ])
            };
            let n = u16em(28) as usize;
            let extra = u16em(30) as usize;
            let comentario = u16em(32) as usize;
            let nome = String::from_utf8_lossy(
                central
                    .get(i + 46..i + 46 + n)
                    .ok_or_else(|| PhxError::Corrompido("nome cortado no ZIP".into()))?,
            )
            .into_owned();
            entradas.push(EntradaZip {
                nome,
                metodo: u16em(10),
                crc: u32em(16),
                comprimido: u32em(20) as u64,
                original: u32em(24) as u64,
                deslocamento: u32em(42) as u64,
            });
            i += 46 + n + extra + comentario;
        }
        Ok(LeitorZip { fonte, entradas })
    }

    pub fn entradas(&self) -> &[EntradaZip] {
        &self.entradas
    }

    /// Tira UM arquivo de dentro, conferindo o CRC-32 gravado no proprio ZIP.
    pub fn ler(&mut self, nome: &str) -> Result<Vec<u8>> {
        let e = self
            .entradas
            .iter()
            .find(|e| e.nome == nome)
            .cloned()
            .ok_or_else(|| PhxError::NaoEncontrado(format!("{nome} nao esta no ZIP")))?;
        self.ler_entrada(&e)
    }

    pub fn ler_entrada(&mut self, e: &EntradaZip) -> Result<Vec<u8>> {
        self.fonte.seek(SeekFrom::Start(e.deslocamento))?;
        let mut cabecalho = [0u8; 30];
        self.fonte.read_exact(&mut cabecalho)?;
        // A assinatura local e a prova de que o deslocamento do diretorio
        // central aponta para onde diz. Sem ela, um ZIP escrito acima de 4 GiB
        // -- em que o deslocamento nao coube nos 32 bits do formato -- seria
        // lido a partir de um lugar qualquer, e restauraria lixo em silencio.
        if cabecalho[..4] != *b"PK\x03\x04" {
            return Err(PhxError::Corrompido(format!(
                "{}: o cabecalho local nao esta onde o diretorio central disse",
                e.nome
            )));
        }
        let n = u16::from_le_bytes([cabecalho[26], cabecalho[27]]) as usize;
        let extra = u16::from_le_bytes([cabecalho[28], cabecalho[29]]) as i64;
        // O nome vem gravado duas vezes -- aqui e no diretorio central --, e
        // conferir os dois custa nada. Se diferirem, o deslocamento caiu no
        // cabecalho de OUTRA entrada, e o que sairia dali seria o arquivo
        // errado com o nome certo: o pior jeito de restaurar.
        let mut nome_local = vec![0u8; n];
        self.fonte.read_exact(&mut nome_local)?;
        if String::from_utf8_lossy(&nome_local) != e.nome {
            return Err(PhxError::Corrompido(format!(
                "{}: o cabecalho local diz chamar-se {:?}",
                e.nome,
                String::from_utf8_lossy(&nome_local)
            )));
        }
        self.fonte.seek(SeekFrom::Current(extra))?;

        let mut corpo = vec![0u8; e.comprimido as usize];
        self.fonte.read_exact(&mut corpo)?;
        let dados = match e.metodo {
            0 => corpo,
            8 => inflate(&corpo).ok_or_else(|| {
                PhxError::Corrompido(format!("{}: o DEFLATE de dentro do ZIP nao fecha", e.nome))
            })?,
            outro => {
                return Err(PhxError::Corrompido(format!(
                    "{}: metodo de compressao {outro} nao suportado (so 0 e 8)",
                    e.nome
                )))
            }
        };
        if dados.len() as u64 != e.original {
            return Err(PhxError::Corrompido(format!(
                "{}: sairam {} bytes e o ZIP prometia {}",
                e.nome,
                dados.len(),
                e.original
            )));
        }
        if crc32(&dados) != e.crc {
            return Err(PhxError::Corrompido(format!(
                "{}: o CRC-32 gravado no ZIP nao bate com o conteudo",
                e.nome
            )));
        }
        Ok(dados)
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

    // ---------------------------------------------------------- leitura
    //
    // Ida e volta com o proprio compressor NAO prova que sabemos ler o ZIP do
    // mundo: os dois lados podem estar errados juntos, e foi assim que a casa
    // ja se enganou antes. Por isso a prova da leitura sao VETORES -- bytes
    // produzidos pela zlib, com o texto que tem de sair deles.

    fn bytes(hex: &str) -> Vec<u8> {
        crate::hash::de_hex(hex).expect("vetor com hexadecimal invalido")
    }

    /// Huffman DINAMICO (BTYPE=10), que o nosso compressor nunca emite e todo
    /// compressor do mundo emite. Sem este vetor, um backup reempacotado por
    /// quem o abriu para olhar voltaria como "o DEFLATE nao fecha".
    #[test]
    fn le_o_huffman_dinamico_da_zlib() {
        let comprimido = bytes(
            "05c15b0280101000c02bb11ecba754249194e4fe0769c6104f333466791045765c54d495\
             0cea20b1874f629327be6ad63bb9e8072b3bf82d8cf49855d396045aa0b3854751e540a7\
             92fe01",
        );
        let esperado =
            "A0H1O2V3C4J5Q6X7E8L9S0Z1G2N3U4B5I6P7W8D9K0R1Y2F3M4T5A6H7O8V9C0J1Q2X3E4L5S6Z7G8N9";
        assert_eq!(
            inflate(&comprimido).expect("nao leu o Huffman dinamico"),
            esperado.as_bytes()
        );
        // E o contrato antigo do `inflate_fixo` continua o mesmo: ele recusa.
        assert!(inflate_fixo(&comprimido).is_none());
    }

    /// Bloco SEM compressao (BTYPE=00), que a zlib emite no nivel 0 e que
    /// aparece em arquivo ja comprimido dentro de outro ZIP.
    #[test]
    fn le_o_bloco_sem_compressao_da_zlib() {
        let comprimido = bytes(
            "012400dbff677561726461646f206372752c2073656d20636f6d7072657373616f206e65\
             6e68756d61",
        );
        assert_eq!(
            inflate(&comprimido).unwrap(),
            b"guardado cru, sem compressao nenhuma"
        );
    }

    /// O que o `Zip` escreveu, o `LeitorZip` tira de volta -- inclusive a
    /// hierarquia de diretorios, que e o que a restauracao recria.
    #[test]
    fn o_leitor_tira_de_volta_o_que_o_escritor_guardou() {
        let mut z = Zip::novo(1_787_000_000_000);
        let grande: Vec<u8> = (0..40_000u32).flat_map(|i| (i % 251) as u8..=250).collect();
        z.acrescentar("Z/cadastroClientes.reg", &grande);
        z.acrescentar("Z/schemaX/pedidos.reg", b"pedidos do schema");
        // Curto demais para comprimir: entra cru, e o leitor tem de aceitar o
        // metodo 0 tambem.
        z.acrescentar("backup.json", b"{}");
        let arquivo = z.terminar();

        let mut leitor = LeitorZip::abrir(std::io::Cursor::new(&arquivo)).unwrap();
        let nomes: Vec<String> = leitor.entradas().iter().map(|e| e.nome.clone()).collect();
        assert_eq!(
            nomes,
            vec![
                "Z/cadastroClientes.reg".to_string(),
                "Z/schemaX/pedidos.reg".to_string(),
                "backup.json".to_string()
            ]
        );
        assert!(
            leitor.entradas().iter().any(|e| e.metodo == 8),
            "nada comprimiu"
        );
        assert!(
            leitor.entradas().iter().any(|e| e.metodo == 0),
            "nada entrou cru"
        );
        assert_eq!(leitor.ler("Z/cadastroClientes.reg").unwrap(), grande);
        assert_eq!(
            leitor.ler("Z/schemaX/pedidos.reg").unwrap(),
            b"pedidos do schema"
        );
        assert_eq!(leitor.ler("backup.json").unwrap(), b"{}");
        assert!(leitor.ler("nao/existe.reg").is_err());
    }

    /// Byte trocado no meio do corpo comprimido: o CRC-32 que o proprio ZIP
    /// carrega pega, antes mesmo de o manifesto ser conferido.
    #[test]
    fn o_leitor_recusa_o_zip_adulterado() {
        let mut z = Zip::novo(1_787_000_000_000);
        let dados: Vec<u8> = (0..8_000u32).map(|i| (i % 97) as u8).collect();
        z.acrescentar("Z/tabela.reg", &dados);
        let mut arquivo = z.terminar();
        // 30 bytes de cabecalho local mais 12 do nome: dai para a frente e
        // corpo comprimido, que e onde a podridao de disco aparece.
        arquivo[30 + 12 + 20] ^= 0x40;

        let mut leitor = LeitorZip::abrir(std::io::Cursor::new(&arquivo)).unwrap();
        let e = leitor.ler("Z/tabela.reg").unwrap_err();
        assert_eq!(e.nome(), "CORROMPIDO", "veio {e}");
    }

    /// Arquivo que nao e ZIP nao vira ZIP por insistencia.
    #[test]
    fn o_que_nao_e_zip_e_recusado_dizendo_isso() {
        let lixo = vec![7u8; 500];
        let Err(e) = LeitorZip::abrir(std::io::Cursor::new(&lixo)) else {
            panic!("500 bytes de lixo passaram por ZIP");
        };
        assert!(e.to_string().contains("nao e um ZIP"), "veio {e}");
    }
}
