# Fix trash record header layout and CRC
# 28/08 17:29

import io
p='crates/phxsql-store/src/lixeira.rs'
s=io.open(p,encoding='utf-8').read()

velho_doc = '''//! # Registro (48 bytes de cabecalho + payload + externos)
//!
//! ```text
//! [carimbo i64 ms][flags u8][n_externos u8][reservado u16]
//! [rowid u64][usuario u32][payload_len u32]
//! [uuid do evento 16 bytes]
//! [total_len u32][crc32 u32]
//! [payload]
//! [ (coluna u16)(tamanho u32)(bytes) ]  x n_externos
//! ```'''
novo_doc = '''//! # Registro (56 bytes de cabecalho + payload + externos)
//!
//! ```text
//! 0  [carimbo i64 ms]
//! 8  [flags u8][n_externos u8][reservado u16]
//! 12 [rowid u64]
//! 20 [usuario u32][payload_len u32]
//! 28 [uuid do descarte, 16 bytes]
//! 44 [total_len u32][reservado u32][crc32 u32]
//! 56 [payload]
//!    [ (coluna u16)(tamanho u32)(bytes) ]  x n_externos
//! ```'''
assert velho_doc in s
s = s.replace(velho_doc, novo_doc, 1)

s = s.replace('''/// Bytes do cabecalho de cada registro, antes do payload.
pub const REGISTRO_CAB: usize = 48;''',
'''/// Bytes do cabecalho de cada registro, antes do payload.
pub const REGISTRO_CAB: usize = 56;
/// Byte onde comeca o campo do CRC, que e o unico que ele nao cobre.
const OFF_CRC: usize = 52;''', 1)

velho_esc = '''        let mut buf = vec![0u8; REGISTRO_CAB];
        por_i64(&mut buf, 0, self.carimbo);
        buf[9] = self.externos.len() as u8;
        por_u64(&mut buf, 12, self.rowid);
        por_u32(&mut buf, 20, self.usuario);
        por_u32(&mut buf, 24, self.payload.len() as u32);
        buf[28..44].copy_from_slice(self.uuid.bytes());
        por_u32(&mut buf, 44, total as u32);

        buf.extend_from_slice(&self.payload);
        for (coluna, bytes) in &self.externos {
            buf.extend_from_slice(&coluna.to_le_bytes());
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        debug_assert_eq!(buf.len(), total);

        // O CRC cobre o cabecalho sem o proprio campo, e todo o corpo.
        let mut crc = crc32(&buf[..48]);
        crc = crc32_with(crc, &buf[REGISTRO_CAB..]);
        // O campo do CRC fica nos ultimos 4 bytes do cabecalho, e por isso o
        // trecho coberto acima para em 48 -- que e o cabecalho inteiro menos
        // nada. Grava depois de calcular, sobre zeros.
        let mut saida = buf;
        por_u32(&mut saida, 40, 0);
        let _ = crc;
        let mut crc = crc32(&saida[..40]);
        crc = crc32_with(crc, &saida[REGISTRO_CAB..]);
        por_u32(&mut saida, 40, crc);
        Ok(saida)
    }'''
novo_esc = '''        let mut buf = vec![0u8; REGISTRO_CAB];
        por_i64(&mut buf, 0, self.carimbo);
        buf[9] = self.externos.len() as u8;
        por_u64(&mut buf, 12, self.rowid);
        por_u32(&mut buf, 20, self.usuario);
        por_u32(&mut buf, 24, self.payload.len() as u32);
        buf[28..44].copy_from_slice(self.uuid.bytes());
        por_u32(&mut buf, 44, total as u32);

        buf.extend_from_slice(&self.payload);
        for (coluna, bytes) in &self.externos {
            buf.extend_from_slice(&coluna.to_le_bytes());
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        debug_assert_eq!(buf.len(), total);

        // Cobre o cabecalho ate o campo do CRC e depois o corpo inteiro: o
        // payload e os anexos entram na conta. Um `.trash` so vale como prova
        // do que a linha era se adulterar o conteudo for detectado.
        por_u32(&mut buf, OFF_CRC, crc32_do(&buf));
        Ok(buf)
    }'''
assert velho_esc in s
s = s.replace(novo_esc if False else velho_esc, novo_esc, 1)

velho_ler = '''        let c = Campos(src);
        let total = c.u32(44) as usize;
        let payload_len = c.u32(24) as usize;
        if total < REGISTRO_CAB + payload_len || src.len() < total {
            return Err(PhxError::Corrompido(
                "registro de .trash menor que o tamanho que declara".into(),
            ));
        }
        let mut zerado = src[..total].to_vec();
        let gravado = c.u32(40);
        por_u32(&mut zerado, 40, 0);
        let mut crc = crc32(&zerado[..40]);
        crc = crc32_with(crc, &zerado[REGISTRO_CAB..]);
        if crc != gravado {
            return Err(PhxError::Corrompido(
                "registro de .trash com CRC invalido".into(),
            ));
        }
'''
novo_ler = '''        let c = Campos(src);
        let total = c.u32(44) as usize;
        let payload_len = c.u32(24) as usize;
        if total < REGISTRO_CAB + payload_len || src.len() < total {
            return Err(PhxError::Corrompido(
                "registro de .trash menor que o tamanho que declara".into(),
            ));
        }
        if crc32_do(&src[..total]) != c.u32(OFF_CRC) {
            return Err(PhxError::Corrompido(
                "registro de .trash com CRC invalido".into(),
            ));
        }
'''
assert velho_ler in s
s = s.replace(velho_ler, novo_ler, 1)

# a funcao auxiliar do CRC, logo antes do impl Descartada
velho_impl = '''impl Descartada {
    pub fn instante_iso(&self) -> String {'''
novo_impl = '''/// CRC do registro inteiro, pulando os quatro bytes do proprio campo.
fn crc32_do(registro: &[u8]) -> u32 {
    let crc = crc32(&registro[..OFF_CRC]);
    crc32_with(crc, &registro[OFF_CRC + 4..])
}

impl Descartada {
    pub fn instante_iso(&self) -> String {'''
assert velho_impl in s
s = s.replace(velho_impl, novo_impl, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
