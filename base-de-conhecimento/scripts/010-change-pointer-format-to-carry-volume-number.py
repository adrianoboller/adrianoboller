# Change pointer format to carry volume number
# 27/08 18:18

p='crates/phxsql-core/src/value.rs'
s=open(p).read()

s=s.replace('''/// Ponteiro de 16 bytes gravado no `.reg` apontando para um bloco do
/// `.bin` ou do `.memo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ponteiro {
    /// Offset absoluto do cabecalho do bloco dentro do arquivo externo.
    pub offset: u64,
    /// Tamanho do conteudo em bytes.
    pub tamanho: u32,
    /// CRC-32 do conteudo, conferido a cada leitura.
    pub crc: u32,
}

impl Ponteiro {
    pub const VAZIO: Ponteiro = Ponteiro {
        offset: 0,
        tamanho: 0,
        crc: 0,
    };

    pub fn e_vazio(&self) -> bool {
        self.offset == 0 && self.tamanho == 0
    }

    pub fn escrever(&self, dst: &mut [u8]) -> Result<()> {
        if dst.len() < PONTEIRO_LEN {
            return Err(PhxError::Corrompido(
                "espaco insuficiente para ponteiro".into(),
            ));
        }
        dst[0..8].copy_from_slice(&self.offset.to_le_bytes());
        dst[8..12].copy_from_slice(&self.tamanho.to_le_bytes());
        dst[12..16].copy_from_slice(&self.crc.to_le_bytes());
        Ok(())
    }

    pub fn ler(src: &[u8]) -> Result<Ponteiro> {
        if src.len() < PONTEIRO_LEN {
            return Err(PhxError::Corrompido("ponteiro truncado".into()));
        }
        Ok(Ponteiro {
            offset: u64::from_le_bytes(src[0..8].try_into().unwrap()),
            tamanho: u32::from_le_bytes(src[8..12].try_into().unwrap()),
            crc: u32::from_le_bytes(src[12..16].try_into().unwrap()),
        })
    }
}''','''/// Maior offset representavel dentro de um volume externo (2^48 - 1).
pub const OFFSET_MAXIMO: u64 = (1 << 48) - 1;

/// Ponteiro de 16 bytes gravado no `.reg`, apontando para um bloco do
/// `.bin` ou do `.memo`.
///
/// ```text
/// [offset u48 | volume u16 | tamanho u32 | crc32 u32]
/// ```
///
/// O offset ocupa 48 bits (256 TB por volume) e os 16 bits liberados passam a
/// guardar o numero do volume, para que o conteudo externo tambem possa ser
/// paginado em `Tabela_001.bin`, `Tabela_002.bin` e assim por diante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ponteiro {
    /// Volume do arquivo externo. 1 quando nao ha paginacao.
    pub volume: u16,
    /// Offset do cabecalho do bloco dentro do volume. Cabe em 48 bits.
    pub offset: u64,
    /// Tamanho do conteudo em bytes.
    pub tamanho: u32,
    /// CRC-32 do conteudo, conferido a cada leitura.
    pub crc: u32,
}

impl Ponteiro {
    pub const VAZIO: Ponteiro = Ponteiro {
        volume: 0,
        offset: 0,
        tamanho: 0,
        crc: 0,
    };

    pub fn e_vazio(&self) -> bool {
        self.offset == 0 && self.tamanho == 0
    }

    pub fn escrever(&self, dst: &mut [u8]) -> Result<()> {
        if dst.len() < PONTEIRO_LEN {
            return Err(PhxError::Corrompido(
                "espaco insuficiente para ponteiro".into(),
            ));
        }
        if self.offset > OFFSET_MAXIMO {
            return Err(PhxError::LimiteExcedido(format!(
                "offset {} excede o maximo de 48 bits de um volume",
                self.offset
            )));
        }
        dst[0..6].copy_from_slice(&self.offset.to_le_bytes()[..6]);
        dst[6..8].copy_from_slice(&self.volume.to_le_bytes());
        dst[8..12].copy_from_slice(&self.tamanho.to_le_bytes());
        dst[12..16].copy_from_slice(&self.crc.to_le_bytes());
        Ok(())
    }

    pub fn ler(src: &[u8]) -> Result<Ponteiro> {
        if src.len() < PONTEIRO_LEN {
            return Err(PhxError::Corrompido("ponteiro truncado".into()));
        }
        let mut off = [0u8; 8];
        off[..6].copy_from_slice(&src[0..6]);
        Ok(Ponteiro {
            offset: u64::from_le_bytes(off),
            volume: u16::from_le_bytes(src[6..8].try_into().unwrap()),
            tamanho: u32::from_le_bytes(src[8..12].try_into().unwrap()),
            crc: u32::from_le_bytes(src[12..16].try_into().unwrap()),
        })
    }
}''')

s=s.replace('''    #[test]
    fn ponteiro_roundtrip() {
        let p = Ponteiro {
            offset: 4096,
            tamanho: 1234,
            crc: 0xDEAD_BEEF,
        };
        let mut buf = [0u8; PONTEIRO_LEN];
        p.escrever(&mut buf).unwrap();
        assert_eq!(Ponteiro::ler(&buf).unwrap(), p);
    }''','''    #[test]
    fn ponteiro_roundtrip() {
        let p = Ponteiro {
            volume: 7,
            offset: 4096,
            tamanho: 1234,
            crc: 0xDEAD_BEEF,
        };
        let mut buf = [0u8; PONTEIRO_LEN];
        p.escrever(&mut buf).unwrap();
        assert_eq!(Ponteiro::ler(&buf).unwrap(), p);
    }

    #[test]
    fn ponteiro_no_limite_dos_48_bits() {
        let p = Ponteiro {
            volume: u16::MAX,
            offset: OFFSET_MAXIMO,
            tamanho: u32::MAX,
            crc: 0xFFFF_FFFF,
        };
        let mut buf = [0u8; PONTEIRO_LEN];
        p.escrever(&mut buf).unwrap();
        assert_eq!(Ponteiro::ler(&buf).unwrap(), p);
    }

    #[test]
    fn offset_acima_de_48_bits_e_recusado() {
        let p = Ponteiro {
            volume: 1,
            offset: OFFSET_MAXIMO + 1,
            tamanho: 1,
            crc: 0,
        };
        let mut buf = [0u8; PONTEIRO_LEN];
        assert!(p.escrever(&mut buf).is_err());
    }''')
open(p,'w').write(s)
print("value.rs: ponteiro com volume")
