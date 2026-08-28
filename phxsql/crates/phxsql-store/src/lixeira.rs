//! `.trash` -- a linha inteira, guardada antes de sumir do `.reg`.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
//! ```
//!
//! # A ordem e o recurso
//!
//! A linha e gravada aqui e o arquivo e **sincronizado** antes de o slot do
//! `.reg` ser liberado. Se a maquina cair no meio, o pior caso e a linha
//! aparecer nos dois lugares -- que se resolve olhando --, e nunca em nenhum.
//! A ordem inversa (liberar e depois guardar) tem uma janela em que o registro
//! nao existe em lugar nenhum, e essa janela nao tem conserto depois.
//!
//! # Por que nao e um `.reg` paralelo
//!
//! Um `.reg` guarda payload de largura fixa, e as colunas `Bin`/`Memo` moram
//! nele como PONTEIRO para o `.bin`/`.memo`. Copiar so o payload para um `.reg`
//! paralelo guardaria os ponteiros -- que apontam para blocos que a propria
//! exclusao acabou de liberar. A foto voltaria sem a foto.
//!
//! Entao o registro daqui e de tamanho variavel: o payload byte a byte, mais o
//! CONTEUDO de cada coluna externa logo em seguida. E a linha inteira, e volta
//! inteira.
//!
//! # Registro (56 bytes de cabecalho + payload + externos)
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
//! ```
//!
//! `total_len` esta no cabecalho de proposito: quem percorre o arquivo avanca
//! por ele sem precisar somar os externos um a um, e um registro que se
//! declara maior do que o volume e recusado em vez de arrastar a leitura para
//! dentro do registro seguinte.
//!
//! # Quem le
//!
//! So quem administra. Aqui esta o dado que alguem mandou apagar.

use std::collections::HashMap;
use std::path::Path;

use phxsql_core::crc::{crc32, crc32_with};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::uuid::Uuid;
use phxsql_core::RowId;

use crate::util::{agora, agora_ms, conferir_magic, por_i64, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_LIXEIRA: &[u8; 8] = b"PHXTRH\0\0";
pub const EXT_TRASH: &str = "trash";

const CAB_LEN: usize = 64;
/// Bytes do cabecalho de cada registro, antes do payload.
pub const REGISTRO_CAB: usize = 56;
/// Byte onde comeca o campo do CRC, que e o unico que ele nao cobre.
const OFF_CRC: usize = 52;
const VERSAO: u16 = 1;

/// Uma linha na lixeira.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descartada {
    /// Identidade do descarte. v7: ordenar por ele e ordenar por tempo.
    pub uuid: Uuid,
    /// Milissegundos desde 1970-01-01T00:00:00Z.
    pub carimbo: i64,
    /// O rowid que a linha tinha. Nao e reservado nem devolvido -- e memoria
    /// de onde ela estava, nao promessa de para onde volta.
    pub rowid: RowId,
    /// Quem excluiu. Zero = nao informado.
    pub usuario: u32,
    /// O payload do slot, byte a byte como estava no `.reg`.
    pub payload: Vec<u8>,
    /// Quantas colunas externas a linha TEM, sempre.
    ///
    /// Separado de `externos.len()` de proposito: a listagem pode nao carregar
    /// os anexos, e ai `externos` vem vazio. Se o contador saisse dele, a tela
    /// da lixeira diria "0 anexos" para uma linha que tem tres -- e quem
    /// investiga concluiria que a foto nunca existiu.
    pub n_externos: u8,
    /// Conteudo de cada coluna externa: `(indice da coluna, bytes)`.
    ///
    /// Vazio quando a leitura pediu sem anexos. Compare com `n_externos` para
    /// saber se e "nao tem" ou "nao carregou".
    pub externos: Vec<(u16, Vec<u8>)>,
}

/// CRC do registro inteiro, pulando os quatro bytes do proprio campo.
fn crc32_do(registro: &[u8]) -> u32 {
    let crc = crc32(&registro[..OFF_CRC]);
    crc32_with(crc, &registro[OFF_CRC + 4..])
}

impl Descartada {
    pub fn instante_iso(&self) -> String {
        phxsql_core::datahora::instante_iso(self.carimbo)
    }

    /// Bytes que este registro ocupa no arquivo.
    ///
    /// So vale com os anexos carregados: numa `Descartada` que veio de uma
    /// listagem leve, `externos` esta vazio e a conta sai menor.
    pub fn tamanho(&self) -> usize {
        REGISTRO_CAB
            + self.payload.len()
            + self
                .externos
                .iter()
                .map(|(_, b)| 6 + b.len())
                .sum::<usize>()
    }

    fn escrever(&self) -> Result<Vec<u8>> {
        debug_assert_eq!(self.n_externos as usize, self.externos.len());
        if self.externos.len() > u8::MAX as usize {
            return Err(PhxError::LimiteExcedido(format!(
                "{} colunas externas numa linha; a lixeira guarda ate {}",
                self.externos.len(),
                u8::MAX
            )));
        }
        let total = self.tamanho();
        if total > u32::MAX as usize {
            return Err(PhxError::LimiteExcedido(
                "linha grande demais para a lixeira (mais de 4 GiB)".into(),
            ));
        }
        let mut buf = vec![0u8; REGISTRO_CAB];
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
        let crc = crc32_do(&buf);
        por_u32(&mut buf, OFF_CRC, crc);
        Ok(buf)
    }

    fn ler(src: &[u8]) -> Result<Descartada> {
        if src.len() < REGISTRO_CAB {
            return Err(PhxError::Corrompido("registro de .trash truncado".into()));
        }
        let c = Campos(src);
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

        let payload = src[REGISTRO_CAB..REGISTRO_CAB + payload_len].to_vec();
        let mut externos = Vec::with_capacity(src[9] as usize);
        let mut pos = REGISTRO_CAB + payload_len;
        for _ in 0..src[9] {
            if pos + 6 > total {
                return Err(PhxError::Corrompido(
                    "a lixeira declara mais colunas externas do que cabem no registro".into(),
                ));
            }
            let coluna = u16::from_le_bytes([src[pos], src[pos + 1]]);
            let n = u32::from_le_bytes(src[pos + 2..pos + 6].try_into().unwrap()) as usize;
            pos += 6;
            if pos + n > total {
                return Err(PhxError::Corrompido(
                    "conteudo externo da lixeira passa do fim do registro".into(),
                ));
            }
            externos.push((coluna, src[pos..pos + n].to_vec()));
            pos += n;
        }
        Ok(Descartada {
            uuid: Uuid::de_bytes(src[28..44].try_into().unwrap()),
            carimbo: c.u64(0) as i64,
            rowid: c.u64(12),
            usuario: c.u32(20),
            n_externos: src[9],
            payload,
            externos,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Cabecalho {
    volume: u32,
    fim: u64,
    quantos: u64,
}

pub struct LixeiraFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// Usuario aplicado aos descartes gravados daqui em diante.
    pub usuario: u32,
}

impl LixeiraFile {
    pub fn criar(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<LixeiraFile> {
        let mut l = LixeiraFile {
            volumes: Volumes::novo(diretorio, nome, EXT_TRASH, paginacao),
            cabs: HashMap::new(),
            volume_atual: 1,
            usuario: 0,
        };
        l.volumes.criar(1)?;
        l.gravar_cab(Cabecalho {
            volume: 1,
            fim: CAB_LEN as u64,
            quantos: 0,
        })?;
        Ok(l)
    }

    /// Abre; cria se nao existir, pela mesma razao do `.reason`.
    pub fn abrir(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<LixeiraFile> {
        let volumes = Volumes::novo(&diretorio, nome, EXT_TRASH, paginacao);
        if volumes.existentes().is_empty() {
            return LixeiraFile::criar(diretorio, nome, paginacao);
        }
        let volume_atual = *volumes.existentes().last().unwrap();
        let mut l = LixeiraFile {
            volumes,
            cabs: HashMap::new(),
            volume_atual,
            usuario: 0,
        };
        l.cab(1)?;
        l.cab(volume_atual)?;
        Ok(l)
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let mut buf = [0u8; CAB_LEN];
        self.volumes.ler(volume, 0, &mut buf)?;
        let nome = self.volumes.caminho(volume).display().to_string();
        conferir_magic(&nome, MAGIC_LIXEIRA, &buf[0..8])?;
        let c = Campos(&buf);
        let versao = c.u16(8);
        if versao != VERSAO {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: nome,
                encontrada: versao,
                suportada: VERSAO,
            });
        }
        if crc32(&buf[..56]) != c.u32(56) {
            return Err(PhxError::Corrompido(format!(
                "cabecalho de {nome} com CRC invalido"
            )));
        }
        let cab = Cabecalho {
            volume: c.u32(12),
            fim: c.u64(24),
            quantos: c.u64(16),
        };
        self.cabs.insert(volume, cab);
        Ok(cab)
    }

    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(MAGIC_LIXEIRA);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, cab.volume);
        por_u64(&mut buf, 16, cab.quantos);
        por_u64(&mut buf, 24, cab.fim);
        por_i64(&mut buf, 32, agora());
        let crc = crc32(&buf[..56]);
        por_u32(&mut buf, 56, crc);
        self.volumes.escrever(cab.volume, 0, &buf)?;
        self.cabs.insert(cab.volume, cab);
        Ok(())
    }

    /// Guarda a linha e **espera o disco confirmar**.
    ///
    /// O `sincronizar` esta aqui dentro, e nao a cargo de quem chama, porque a
    /// garantia que este arquivo existe para dar depende dele: sem o disco
    /// confirmar, "ja esta na lixeira" e so uma pagina suja na memoria, e a
    /// exclusao que vem em seguida e definitiva.
    pub fn guardar(
        &mut self,
        rowid: RowId,
        payload: &[u8],
        externos: Vec<(u16, Vec<u8>)>,
    ) -> Result<Descartada> {
        let d = Descartada {
            uuid: Uuid::v7(),
            carimbo: agora_ms(),
            rowid,
            usuario: self.usuario,
            n_externos: externos.len().min(u8::MAX as usize) as u8,
            payload: payload.to_vec(),
            externos,
        };
        self.anexar(&d)?;
        self.volumes.sincronizar()?;
        Ok(d)
    }

    fn anexar(&mut self, d: &Descartada) -> Result<()> {
        let bytes = d.escrever()?;
        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, bytes.len() as u64, vazio);

        let cab = if virou {
            if paginacao.ligada() && volume > paginacao.max_arquivos {
                return Err(PhxError::LimiteExcedido(format!(
                    "a lixeira de {} chegou ao teto de {} volumes",
                    self.volumes.nome(),
                    paginacao.max_arquivos
                )));
            }
            self.volumes.garantir(volume)?;
            let novo = Cabecalho {
                volume,
                fim: CAB_LEN as u64,
                quantos: 0,
            };
            self.gravar_cab(novo)?;
            self.volume_atual = volume;
            novo
        } else {
            atual
        };

        self.volumes.escrever(volume, cab.fim, &bytes)?;
        self.gravar_cab(Cabecalho {
            volume,
            fim: cab.fim + bytes.len() as u64,
            quantos: cab.quantos + 1,
        })
    }

    pub fn total(&mut self) -> Result<u64> {
        let mut t = 0;
        for v in self.volumes.existentes() {
            t += self.cab(v)?.quantos;
        }
        Ok(t)
    }

    /// Le em ordem cronologica. `limite` zero devolve tudo.
    ///
    /// `com_externos` falso deixa os anexos de fora: a tela que LISTA a
    /// lixeira nao precisa carregar as fotos de mil linhas para mostrar quem
    /// excluiu o que e quando.
    pub fn ler(&mut self, pular: u64, limite: u64, com_externos: bool) -> Result<Vec<Descartada>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            let mut offset = CAB_LEN as u64;
            while offset + REGISTRO_CAB as u64 <= cab.fim {
                let mut cabecalho = [0u8; REGISTRO_CAB];
                self.volumes.ler(volume, offset, &mut cabecalho)?;
                let total = Campos(&cabecalho).u32(44) as usize;
                if total < REGISTRO_CAB || offset + total as u64 > cab.fim {
                    return Err(PhxError::Corrompido(format!(
                        "registro de .trash em {} passa do fim do volume",
                        self.volumes.caminho(volume).display()
                    )));
                }
                if vistos >= pular {
                    let mut buf = vec![0u8; total];
                    self.volumes.ler(volume, offset, &mut buf)?;
                    let mut d = Descartada::ler(&buf)?;
                    if !com_externos {
                        // So o CONTEUDO sai; `n_externos` fica.
                        d.externos.clear();
                    }
                    saida.push(d);
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += total as u64;
            }
        }
        Ok(saida)
    }

    /// A linha descartada com este uuid, com os anexos.
    pub fn por_uuid(&mut self, uuid: &Uuid) -> Result<Option<Descartada>> {
        Ok(self
            .ler(0, 0, true)?
            .into_iter()
            .find(|d| d.uuid.bytes() == uuid.bytes()))
    }

    /// Confere o CRC de tudo e a contagem dos cabecalhos.
    pub fn verificar(&mut self) -> Result<u64> {
        let quantos = self.ler(0, 0, true)?.len() as u64;
        let declarado = self.total()?;
        if quantos != declarado {
            return Err(PhxError::Corrompido(format!(
                "{}: os cabecalhos do .trash declaram {declarado} linhas, \
                 e o arquivo tem {quantos}",
                self.volumes.nome()
            )));
        }
        Ok(quantos)
    }

    /// Esvazia a lixeira: apaga os volumes e comeca um do zero.
    ///
    /// Daqui nao volta, e por isso quem chama tem de ter registrado o expurgo
    /// no `.reason` antes -- o motivo sobrevive ao dado.
    pub fn esvaziar(&mut self) -> Result<u64> {
        let quantos = self.total()?;
        self.volumes.apagar_tudo()?;
        self.cabs.clear();
        self.volume_atual = 1;
        self.volumes.criar(1)?;
        self.gravar_cab(Cabecalho {
            volume: 1,
            fim: CAB_LEN as u64,
            quantos: 0,
        })?;
        self.volumes.sincronizar()?;
        Ok(quantos)
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.volumes.sincronizar()
    }

    pub fn fechar_todos(&mut self) {
        self.volumes.fechar_todos();
    }

    pub fn apagar_tudo(&mut self) -> Result<()> {
        self.volumes.apagar_tudo()
    }

    pub fn volumes_existentes(&self) -> Vec<u32> {
        self.volumes.existentes()
    }

    /// Bytes ocupados por todos os volumes da lixeira.
    pub fn bytes(&mut self) -> Result<u64> {
        let mut t = 0;
        for v in self.volumes.existentes() {
            t += self.volumes.tamanho(v)?;
        }
        Ok(t)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn temp(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("phx-lixeira-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn guarda_e_devolve_a_linha_inteira() {
        let d = temp("ida-e-volta");
        let mut l = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.usuario = 3;
        let payload = vec![9u8; 40];
        let externos = vec![
            (2u16, b"foto".to_vec()),
            (5u16, b"observacao longa".to_vec()),
        ];
        let guardada = l.guardar(7, &payload, externos.clone()).unwrap();

        let lidas = l.ler(0, 0, true).unwrap();
        assert_eq!(lidas.len(), 1);
        assert_eq!(lidas[0].rowid, 7);
        assert_eq!(lidas[0].usuario, 3);
        assert_eq!(lidas[0].payload, payload);
        assert_eq!(lidas[0].externos, externos);
        assert_eq!(lidas[0].uuid.bytes(), guardada.uuid.bytes());
        assert_eq!(l.total().unwrap(), 1);
        assert_eq!(l.verificar().unwrap(), 1);
    }

    /// O motivo de o registro ser de tamanho variavel: linhas com anexos de
    /// tamanhos diferentes, uma atras da outra, tem de ser lidas em ordem.
    #[test]
    fn registros_de_tamanhos_diferentes_seguidos() {
        let d = temp("tamanhos");
        let mut l = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 0..6u64 {
            let anexo = vec![i as u8; (i as usize) * 37];
            l.guardar(i + 1, &[i as u8; 16], vec![(0, anexo)]).unwrap();
        }
        let lidas = l.ler(0, 0, true).unwrap();
        assert_eq!(lidas.len(), 6);
        for (i, d) in lidas.iter().enumerate() {
            assert_eq!(d.rowid, i as u64 + 1);
            assert_eq!(d.externos[0].1.len(), i * 37);
        }
    }

    #[test]
    fn sem_externos_nao_carrega_anexo() {
        let d = temp("sem-anexo");
        let mut l = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.guardar(1, &[1, 2, 3], vec![(0, vec![7u8; 5000])])
            .unwrap();
        let leves = l.ler(0, 0, false).unwrap();
        assert!(leves[0].externos.is_empty());
        assert_eq!(leves[0].payload, vec![1, 2, 3]);
        // ... mas continua no arquivo.
        let cheias = l.ler(0, 0, true).unwrap();
        assert_eq!(cheias[0].externos[0].1.len(), 5000);
    }

    /// Adulterar o payload ou o anexo tem de ser pego: a lixeira e prova de
    /// que a linha era assim quando foi excluida.
    #[test]
    fn conteudo_adulterado_nao_passa() {
        let d = Descartada {
            uuid: Uuid::v7(),
            carimbo: 1,
            rowid: 1,
            usuario: 1,
            n_externos: 1,
            payload: vec![1, 2, 3, 4],
            externos: vec![(0, b"anexo".to_vec())],
        };
        let bom = d.escrever().unwrap();
        assert!(Descartada::ler(&bom).is_ok());

        let mut torto = bom.clone();
        torto[REGISTRO_CAB] ^= 0xFF;
        assert!(
            Descartada::ler(&torto).is_err(),
            "payload adulterado passou"
        );

        let mut torto = bom.clone();
        let pos = REGISTRO_CAB + 4 + 6;
        torto[pos] ^= 0xFF;
        assert!(Descartada::ler(&torto).is_err(), "anexo adulterado passou");
    }

    #[test]
    fn linha_sem_externos() {
        let d = temp("puro");
        let mut l = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.guardar(1, &[0xAB; 24], vec![]).unwrap();
        let lidas = l.ler(0, 0, true).unwrap();
        assert!(lidas[0].externos.is_empty());
        assert_eq!(lidas[0].payload, vec![0xAB; 24]);
    }

    #[test]
    fn esvaziar_zera_e_continua_usavel() {
        let d = temp("esvaziar");
        let mut l = LixeiraFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 0..4 {
            l.guardar(i + 1, &[1, 2], vec![]).unwrap();
        }
        assert_eq!(l.esvaziar().unwrap(), 4);
        assert_eq!(l.total().unwrap(), 0);
        assert!(l.ler(0, 0, true).unwrap().is_empty());
        l.guardar(9, &[3, 4], vec![]).unwrap();
        assert_eq!(l.total().unwrap(), 1);
    }

    #[test]
    fn abrir_cria_quando_nao_existe() {
        let d = temp("abre-e-cria");
        let mut l = LixeiraFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 0);
        l.guardar(1, &[1], vec![]).unwrap();
        drop(l);
        let mut de_novo = LixeiraFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(de_novo.total().unwrap(), 1);
    }
}
