//! ZIP minimo para pacotes OPC.
//!
//! Escrita: so o metodo 0 (store). O OPC aceita, e sem compressao a saida nao
//! depende de heuristica de compressor nenhum: mesma entrada, mesmos bytes.
//! Leitura: store e deflate (RFC 1951), porque os anexos que o agente recebe
//! vem do Word/Excel/LibreOffice, que sempre comprimem.

use crate::OfficeError;

/// Teto de descompressao por entrada. Existe para que um anexo hostil
/// (bomba de zip) nao esgote a memoria do agente.
pub const MAX_ENTRY_BYTES: usize = 256 * 1024 * 1024;

// Data DOS fixa (1980-01-01 00:00): e o menor valor representavel e mantem a
// saida deterministica; carimbo de relogio mudaria o hash a cada geracao.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = (1 << 5) | 1;

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (i, slot) in t.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    t
}

/// CRC-32 (IEEE 802.3, polinomio refletido 0xEDB88320), o que o ZIP exige.
pub fn crc32(data: &[u8]) -> u32 {
    let t = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Monta um ZIP com as entradas na ordem dada (a ordem faz parte do
/// determinismo; o `[Content_Types].xml` vai primeiro por convencao).
pub fn write_store(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, OfficeError> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        // Sem ZIP64: um entregavel de escritorio gerado aqui nunca chega perto.
        if data.len() > u32::MAX as usize || out.len() > u32::MAX as usize {
            return Err(OfficeError::Invalid("pacote maior que 4 GiB".into()));
        }
        let crc = crc32(data);
        let offset = out.len() as u32;
        let size = data.len() as u32;
        let nb = name.as_bytes();
        put32(&mut out, 0x0403_4b50);
        put16(&mut out, 20);
        put16(&mut out, 0);
        put16(&mut out, 0);
        put16(&mut out, DOS_TIME);
        put16(&mut out, DOS_DATE);
        put32(&mut out, crc);
        put32(&mut out, size);
        put32(&mut out, size);
        put16(&mut out, nb.len() as u16);
        put16(&mut out, 0);
        out.extend_from_slice(nb);
        out.extend_from_slice(data);

        put32(&mut central, 0x0201_4b50);
        put16(&mut central, 20);
        put16(&mut central, 20);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, DOS_TIME);
        put16(&mut central, DOS_DATE);
        put32(&mut central, crc);
        put32(&mut central, size);
        put32(&mut central, size);
        put16(&mut central, nb.len() as u16);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put32(&mut central, 0);
        put32(&mut central, offset);
        central.extend_from_slice(nb);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    put32(&mut out, 0x0605_4b50);
    put16(&mut out, 0);
    put16(&mut out, 0);
    put16(&mut out, entries.len() as u16);
    put16(&mut out, entries.len() as u16);
    put32(&mut out, cd_size);
    put32(&mut out, cd_offset);
    put16(&mut out, 0);
    Ok(out)
}

fn corrupt(msg: &str) -> OfficeError {
    OfficeError::Corrupt(msg.to_string())
}

fn rd16(b: &[u8], at: usize) -> Result<u16, OfficeError> {
    b.get(at..at + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| corrupt("zip truncado"))
}
fn rd32(b: &[u8], at: usize) -> Result<u32, OfficeError> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| corrupt("zip truncado"))
}

/// Pacote lido: nome da entrada e conteudo ja descomprimido.
pub struct Archive {
    entries: Vec<(String, Vec<u8>)>,
}

impl Archive {
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        // Nomes de parte OPC sao comparados sem caixa (ECMA-376 parte 2).
        self.entries
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, d)| d.as_slice())
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(n, _)| n.as_str())
    }
}

/// Le o diretorio central (e nao os cabecalhos locais em sequencia), porque
/// e ele que vale quando o produtor usou descritor de dados no fim da entrada.
pub fn read(bytes: &[u8]) -> Result<Archive, OfficeError> {
    if bytes.len() < 22 {
        return Err(corrupt("arquivo pequeno demais para ser zip"));
    }
    let min = bytes.len().saturating_sub(22 + 65535);
    let mut eocd = None;
    let mut i = bytes.len() - 22;
    loop {
        if rd32(bytes, i)? == 0x0605_4b50 {
            eocd = Some(i);
            break;
        }
        if i == min {
            break;
        }
        i -= 1;
    }
    let eocd = eocd.ok_or_else(|| corrupt("fim de diretorio central ausente"))?;
    let count = rd16(bytes, eocd + 10)? as usize;
    let cd_offset = rd32(bytes, eocd + 16)? as usize;
    if count == 0xFFFF || cd_offset == 0xFFFF_FFFF {
        return Err(corrupt("zip64 nao suportado"));
    }
    let mut entries = Vec::with_capacity(count);
    let mut p = cd_offset;
    for _ in 0..count {
        if rd32(bytes, p)? != 0x0201_4b50 {
            return Err(corrupt("entrada do diretorio central invalida"));
        }
        let flags = rd16(bytes, p + 8)?;
        let method = rd16(bytes, p + 10)?;
        let crc = rd32(bytes, p + 16)?;
        let csize = rd32(bytes, p + 20)? as usize;
        let usize_ = rd32(bytes, p + 24)? as usize;
        let nlen = rd16(bytes, p + 28)? as usize;
        let elen = rd16(bytes, p + 30)? as usize;
        let clen = rd16(bytes, p + 32)? as usize;
        let local = rd32(bytes, p + 42)? as usize;
        let name_bytes = bytes
            .get(p + 46..p + 46 + nlen)
            .ok_or_else(|| corrupt("nome truncado"))?;
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        p += 46 + nlen + elen + clen;
        if flags & 1 != 0 {
            return Err(corrupt("entrada cifrada nao suportada"));
        }
        if name.ends_with('/') {
            continue;
        }
        if usize_ > MAX_ENTRY_BYTES {
            return Err(corrupt("entrada acima do teto de descompressao"));
        }
        if rd32(bytes, local)? != 0x0403_4b50 {
            return Err(corrupt("cabecalho local invalido"));
        }
        let lnlen = rd16(bytes, local + 26)? as usize;
        let lelen = rd16(bytes, local + 28)? as usize;
        let start = local + 30 + lnlen + lelen;
        let raw = bytes
            .get(start..start + csize)
            .ok_or_else(|| corrupt("dados da entrada truncados"))?;
        let data = match method {
            0 => raw.to_vec(),
            8 => inflate(raw, usize_)?,
            m => {
                return Err(OfficeError::Corrupt(format!(
                    "metodo de compressao {m} nao suportado"
                )));
            }
        };
        if data.len() != usize_ || crc32(&data) != crc {
            return Err(OfficeError::Corrupt(format!(
                "crc ou tamanho divergente em {name}"
            )));
        }
        entries.push((name, data));
    }
    Ok(Archive { entries })
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    nbits: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> Result<u32, OfficeError> {
        while self.nbits < n {
            let b = *self
                .data
                .get(self.pos)
                .ok_or_else(|| corrupt("fluxo deflate truncado"))?;
            self.pos += 1;
            self.bit |= (b as u32) << self.nbits;
            self.nbits += 8;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.nbits -= n;
        Ok(v)
    }
}

/// Codigo de Huffman canonico: contagem por comprimento e simbolos em ordem.
/// Decodifica bit a bit (como o puff do zlib): mais lento que tabela, mas
/// curto e facil de conferir, e leitura de anexo nao e laco quente.
struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Result<Huff, OfficeError> {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut left: i32 = 1;
        for &c in &count[1..] {
            left = (left << 1) - c as i32;
            if left < 0 {
                return Err(corrupt("codigo de huffman superpovoado"));
            }
        }
        let mut offs = [0u16; 16];
        for l in 1..15 {
            offs[l + 1] = offs[l] + count[l];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Ok(Huff { count, symbol })
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, OfficeError> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= bits.need(1)? as i32;
            let c = self.count[len] as i32;
            if code - c < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += c;
            first += c;
            first <<= 1;
            code <<= 1;
        }
        Err(corrupt("codigo de huffman invalido"))
    }
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Descomprime deflate cru (RFC 1951). `limit` e o tamanho declarado no
/// diretorio central: passar dele ja e prova de arquivo mentiroso, e parar ali
/// e o que impede a bomba de zip.
pub fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, OfficeError> {
    let mut out: Vec<u8> = Vec::with_capacity(limit.min(MAX_ENTRY_BYTES));
    let mut bits = Bits {
        data,
        pos: 0,
        bit: 0,
        nbits: 0,
    };
    loop {
        let last = bits.need(1)?;
        match bits.need(2)? {
            0 => {
                bits.bit = 0;
                bits.nbits = 0;
                let len = rd16(data, bits.pos)? as usize;
                let nlen = rd16(data, bits.pos + 2)? as usize;
                if len != !nlen & 0xFFFF {
                    return Err(corrupt("bloco stored com tamanho inconsistente"));
                }
                let start = bits.pos + 4;
                let chunk = data
                    .get(start..start + len)
                    .ok_or_else(|| corrupt("bloco stored truncado"))?;
                if out.len() + len > limit {
                    return Err(corrupt("descompressao passou do tamanho declarado"));
                }
                out.extend_from_slice(chunk);
                bits.pos = start + len;
            }
            1 => {
                let mut l = [0u8; 288];
                l[..144].fill(8);
                l[144..256].fill(9);
                l[256..280].fill(7);
                l[280..].fill(8);
                let lit = Huff::new(&l)?;
                let dist = Huff::new(&[5u8; 30])?;
                codes(&mut bits, &mut out, &lit, &dist, limit)?;
            }
            2 => {
                let hlit = bits.need(5)? as usize + 257;
                let hdist = bits.need(5)? as usize + 1;
                let hclen = bits.need(4)? as usize + 4;
                const ORD: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut cl = [0u8; 19];
                for &o in ORD.iter().take(hclen) {
                    cl[o] = bits.need(3)? as u8;
                }
                let clh = Huff::new(&cl)?;
                let mut lens = vec![0u8; hlit + hdist];
                let mut i = 0;
                while i < hlit + hdist {
                    let sym = clh.decode(&mut bits)?;
                    let (val, rep) = match sym {
                        0..=15 => (sym as u8, 1),
                        16 => {
                            if i == 0 {
                                return Err(corrupt("repeticao sem comprimento anterior"));
                            }
                            (lens[i - 1], 3 + bits.need(2)? as usize)
                        }
                        17 => (0, 3 + bits.need(3)? as usize),
                        _ => (0, 11 + bits.need(7)? as usize),
                    };
                    if i + rep > hlit + hdist {
                        return Err(corrupt("comprimentos demais no bloco dinamico"));
                    }
                    lens[i..i + rep].fill(val);
                    i += rep;
                }
                let lit = Huff::new(&lens[..hlit])?;
                let dist = Huff::new(&lens[hlit..])?;
                codes(&mut bits, &mut out, &lit, &dist, limit)?;
            }
            _ => return Err(corrupt("tipo de bloco deflate invalido")),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

fn codes(
    bits: &mut Bits,
    out: &mut Vec<u8>,
    lit: &Huff,
    dist: &Huff,
    limit: usize,
) -> Result<(), OfficeError> {
    loop {
        let sym = lit.decode(bits)? as usize;
        if sym < 256 {
            if out.len() >= limit {
                return Err(corrupt("descompressao passou do tamanho declarado"));
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let s = sym - 257;
            if s >= 29 {
                return Err(corrupt("simbolo de comprimento invalido"));
            }
            let len = LBASE[s] as usize + bits.need(LEXT[s] as u32)? as usize;
            let ds = dist.decode(bits)? as usize;
            if ds >= 30 {
                return Err(corrupt("simbolo de distancia invalido"));
            }
            let d = DBASE[ds] as usize + bits.need(DEXT[ds] as u32)? as usize;
            if d > out.len() {
                return Err(corrupt("distancia aponta antes do inicio"));
            }
            if out.len() + len > limit {
                return Err(corrupt("descompressao passou do tamanho declarado"));
            }
            let from = out.len() - d;
            // Copia byte a byte de proposito: a janela pode se sobrepor ao
            // que esta sendo escrito (d < len), e o deflate conta com isso.
            for k in 0..len {
                let b = out[from + k];
                out.push(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_vetor_padrao() {
        // Valor de verificacao do CRC-32/ISO-HDLC para "123456789".
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn store_ida_e_volta() {
        let e = vec![
            ("a.xml".to_string(), b"<a/>".to_vec()),
            ("pasta/b.txt".to_string(), "acao".as_bytes().to_vec()),
        ];
        let z = write_store(&e).unwrap();
        let a = read(&z).unwrap();
        assert_eq!(a.get("a.xml").unwrap(), b"<a/>");
        assert_eq!(a.get("PASTA/B.TXT").unwrap(), b"acao");
        assert_eq!(a.names().count(), 2);
    }

    // Fluxos gerados pelo zlib do Python (zlib.compressobj(9, 8, -15)): um
    // com bloco de Huffman fixo e outro dinamico com repeticoes sobrepostas.
    #[test]
    fn inflate_bloco_fixo() {
        let z = [
            0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00,
        ];
        assert_eq!(inflate(&z, 11).unwrap(), b"hello world");
    }

    #[test]
    fn inflate_fixo_com_referencias_sobrepostas() {
        let z = [
            0x0b, 0x4a, 0xcd, 0x49, 0x2c, 0xc9, 0x2f, 0xca, 0xcc, 0x57, 0x48, 0x49, 0x55, 0x48,
            0x4c, 0x4e, 0xcc, 0xd7, 0x51, 0x08, 0x4e, 0xcc, 0x57, 0x08, 0x48, 0x2c, 0xcd, 0xc9,
            0xb7, 0x56, 0x08, 0x1a, 0x95, 0x1d, 0x95, 0x25, 0x4b, 0x36, 0x31, 0x29, 0x99, 0x6c,
            0x04, 0x00,
        ];
        let esperado = "Relatorio de acao, Sao Paulo; ".repeat(20) + &"abcabcabcabc".repeat(5);
        assert_eq!(inflate(&z, 660).unwrap(), esperado.as_bytes());
    }

    // Z_HUFFMAN_ONLY sobre texto enviesado: forca o bloco dinamico (tipo 2).
    #[test]
    fn inflate_bloco_dinamico() {
        let z = [
            0x05, 0xc1, 0x01, 0x01, 0x00, 0x00, 0x08, 0xc3, 0xa0, 0xac, 0xcc, 0xf7, 0xcf, 0x20,
            0x00, 0x10, 0x00, 0x07, 0x40, 0x00, 0x0c, 0x00, 0x00, 0x02, 0xe0, 0x00, 0x08, 0x80,
            0x01, 0x00, 0x40, 0x00, 0x1c, 0x00, 0x01, 0x30, 0x00, 0x00, 0x08, 0x80, 0x03, 0x20,
            0x00, 0x06, 0xc0, 0x03,
        ];
        let esperado = "aaaaaaaaaabaaaaaaaaaacaaaaaaaaaabaaaaaaaaaadaaaaaaaaaa".repeat(4);
        assert_eq!(inflate(&z, 216).unwrap(), esperado.as_bytes());
    }

    #[test]
    fn inflate_respeita_teto() {
        let z = [
            0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x28, 0xcf, 0x2f, 0xca, 0x49, 0x01, 0x00,
        ];
        assert!(inflate(&z, 5).is_err());
    }

    #[test]
    fn inflate_bloco_stored() {
        let z = [0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(inflate(&z, 3).unwrap(), b"abc");
    }
}
