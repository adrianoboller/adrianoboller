//! Utilitarios comuns aos quatro arquivos.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use phxsql_core::error::{PhxError, Result};

/// Instante atual em segundos desde a epoca Unix.
pub fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Instante atual em milissegundos desde a epoca Unix.
pub fn agora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn ler_exato<F: Read + Seek>(f: &mut F, offset: u64, buf: &mut [u8]) -> Result<()> {
    f.seek(SeekFrom::Start(offset))?;
    f.read_exact(buf)?;
    Ok(())
}

pub fn escrever_em<F: Write + Seek>(f: &mut F, offset: u64, buf: &[u8]) -> Result<()> {
    f.seek(SeekFrom::Start(offset))?;
    f.write_all(buf)?;
    Ok(())
}

/// Leitura de campos little-endian de uma fatia.
pub struct Campos<'a>(pub &'a [u8]);

impl Campos<'_> {
    pub fn u16(&self, off: usize) -> u16 {
        u16::from_le_bytes(self.0[off..off + 2].try_into().unwrap())
    }
    pub fn u32(&self, off: usize) -> u32 {
        u32::from_le_bytes(self.0[off..off + 4].try_into().unwrap())
    }
    pub fn u64(&self, off: usize) -> u64 {
        u64::from_le_bytes(self.0[off..off + 8].try_into().unwrap())
    }
}

pub fn por_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}
pub fn por_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
pub fn por_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}
pub fn por_i64(buf: &mut [u8], off: usize, v: i64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

pub fn conferir_magic(arquivo: &str, esperado: &'static [u8; 8], achado: &[u8]) -> Result<()> {
    if achado != esperado {
        let mut e = [0u8; 8];
        e.copy_from_slice(&achado[..8]);
        return Err(PhxError::BadMagic {
            arquivo: arquivo.to_string(),
            esperado,
            encontrado: e,
        });
    }
    Ok(())
}

/// Deixa o arquivo legivel so pelo dono.
///
/// Existe porque um arquivo pode guardar valor de dado pessoal em claro
/// quando a cifra esta desligada -- que e o padrao. A permissao restrita e a
/// unica protecao que existe nesse caso. Motor UNICO para quem a chama: a
/// decisao de "quem pode ler" nao se espalha entre `.lgpd` (`trilha.rs`) e
/// `.fts` (`fts.rs`), e a que alguem esquecer aqui nao vira 600 por acidente.
///
/// **Mas nem todo arquivo com essa classe de dado chama esta funcao hoje** --
/// achado do parecer do DBA de 24/09/2026 (pedido 345, condicao C3b):
/// `.reg`, `.ndx`, `.log` e `.trash` carregam o mesmo dado pessoal em claro
/// (a linha inteira, no `.reg`; a chave do indice, no `.ndx`) e nascem
/// `0644`, sem passar por aqui. Alcance-los e a copia/ZIP/restauracao do
/// backup que perdem o 0600 no destino e na volta (`.lgpd`, `.fts`) e' o
/// pedido 542 (N1+N2 do parecer) -- ver `docs/PENDENCIAS.md`.
///
/// Silencioso de proposito: num sistema de arquivos que nao tem modo Unix (um
/// volume FAT, um compartilhamento de rede), falhar aqui derrubaria a
/// gravacao por causa de uma protecao que aquele disco nao sabe oferecer -- e
/// ficar sem o arquivo e pior que ficar sem a permissao.
pub fn apertar_permissao(caminho: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(caminho, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = caminho;
    }
}
