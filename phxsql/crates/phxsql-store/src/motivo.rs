//! `.reason` -- por que cada linha foi excluida, e por quem.
//!
//! O `.log` ja diz que houve uma exclusao no rowid tal, no instante tal. O que
//! ele nao diz -- e nao tem onde dizer, porque o evento dele tem 36 bytes
//! fixos -- e **por que**. Este arquivo guarda a frase, a identidade do
//! registro e o usuario, e sobrevive ao registro: a linha pode sumir do `.reg`
//! e do `.trash`, e o motivo continua aqui.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason
//! ```
//!
//! # Registro (48 bytes de cabecalho + dois textos)
//!
//! ```text
//! [carimbo i64 ms][tipo u8][flags u8][motivo_len u16]
//! [rowid u64][usuario u32]
//! [uuid do evento 16 bytes]
//! [identidade_len u16][reservado u16][crc32 u32]
//! [motivo utf-8][identidade utf-8]
//! ```
//!
//! O `uuid` e um v7 do proprio evento: e ele que identifica *esta* exclusao,
//! e como o v7 leva o relogio nos primeiros 48 bits, ordenar por ele e
//! ordenar por quando aconteceu. A `identidade` e o valor que identifica a
//! linha na tabela -- a chave primaria, ou a coluna `Uuid`, ou a sequencia --
//! ja em texto, porque quem le o motivo seis meses depois nao tem mais o
//! esquema daquela linha na cabeca.
//!
//! # Quem le
//!
//! So quem administra. E a razao esta no proprio conteudo: um motivo de
//! exclusao costuma ser mais revelador que o registro que foi excluido
//! ("fraude", "pedido de remocao do titular", "duplicidade com o contrato X").
//!
//! # A cifra dos dois textos (versao 3)
//!
//! Com a cifra ligada, um volume novo nasce na versao 3 e os DOIS TEXTOS vao
//! cifrados juntos, com 16 bytes de etiqueta atras. O cabecalho de 48 bytes
//! continua em claro -- e ele que diz onde o proximo registro comeca --, e
//! entra como dado associado da etiqueta.
//!
//! O nonce sai do UUID do proprio registro mais o offset dele no volume. O
//! UUID ja e unico por definicao, entao nao ha byte novo a gravar. Ver
//! `crate::cofre`.

use std::collections::HashMap;
use std::path::Path;

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::uuid::Uuid;
use phxsql_core::RowId;

use crate::cofre::{self, Cabecalho};
use crate::util::{agora_ms, por_i64, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_MOTIVO: &[u8; 8] = b"PHXRSN\0\0";
pub const EXT_REASON: &str = "reason";

/// Bytes do cabecalho de cada registro, antes dos dois textos.
pub const REGISTRO_CAB: usize = 48;

/// Teto do texto do motivo. Frase, nao dissertacao -- e o `.reason` e lido
/// inteiro para ser mostrado.
pub const MOTIVO_MAX: usize = 2000;
/// Teto da identidade. Chave composta grande cabe; despejo de linha, nao.
pub const IDENTIDADE_MAX: usize = 512;

/// O que aconteceu com a linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Marcada como excluida; continua no `.reg`.
    Suave,
    /// Saiu do `.reg`. A linha inteira foi para o `.trash` antes.
    Fisica,
    /// Voltou de uma exclusao suave.
    Restauracao,
    /// Saiu do `.trash` tambem. Daqui nao volta.
    Expurgo,
}

impl Tipo {
    fn tag(self) -> u8 {
        match self {
            Tipo::Suave => 1,
            Tipo::Fisica => 2,
            Tipo::Restauracao => 3,
            Tipo::Expurgo => 4,
        }
    }

    fn de_tag(t: u8) -> Result<Tipo> {
        Ok(match t {
            1 => Tipo::Suave,
            2 => Tipo::Fisica,
            3 => Tipo::Restauracao,
            4 => Tipo::Expurgo,
            outro => {
                return Err(PhxError::Corrompido(format!(
                    "tipo de exclusao desconhecido no .reason: {outro}"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Suave => "suave",
            Tipo::Fisica => "fisica",
            Tipo::Restauracao => "restauracao",
            Tipo::Expurgo => "expurgo",
        }
    }
}

/// Uma exclusao registrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Motivo {
    /// Identidade do evento. v7: ordenar por ele e ordenar por tempo.
    pub uuid: Uuid,
    /// Milissegundos desde 1970-01-01T00:00:00Z.
    pub carimbo: i64,
    pub tipo: Tipo,
    pub rowid: RowId,
    /// Quem fez. Zero = nao informado.
    pub usuario: u32,
    /// A frase. Pode ser vazia quando a tabela nao exige motivo.
    pub motivo: String,
    /// Como a linha se identificava. Vazio quando a tabela nao tem chave.
    pub identidade: String,
}

impl Motivo {
    /// Data e hora em ISO (`AAAA-MM-DD HH:MM:SS,mmm`).
    pub fn instante_iso(&self) -> String {
        phxsql_core::datahora::instante_iso(self.carimbo)
    }

    /// Bytes que este registro ocupa no arquivo.
    pub fn tamanho(&self) -> usize {
        REGISTRO_CAB + self.motivo.len() + self.identidade.len()
    }

    fn escrever(&self, cab: &Cabecalho, offset: u64) -> Vec<u8> {
        let mut buf = vec![0u8; REGISTRO_CAB];
        por_i64(&mut buf, 0, self.carimbo);
        buf[8] = self.tipo.tag();
        // Os dois tamanhos sao os do TEXTO CLARO, e nao os do que vai ao
        // disco: e por eles que os dois textos se separam depois de decifrar.
        buf[10..12].copy_from_slice(&(self.motivo.len() as u16).to_le_bytes());
        por_u64(&mut buf, 12, self.rowid);
        por_u32(&mut buf, 20, self.usuario);
        buf[24..40].copy_from_slice(self.uuid.bytes());
        buf[40..42].copy_from_slice(&(self.identidade.len() as u16).to_le_bytes());

        let mut claro = Vec::with_capacity(self.motivo.len() + self.identidade.len());
        claro.extend_from_slice(self.motivo.as_bytes());
        claro.extend_from_slice(self.identidade.as_bytes());
        let corpo = cab.selar(tempero(self.uuid.bytes()), offset, &associado(&buf), &claro);
        buf.extend_from_slice(&corpo);

        // O CRC cobre o cabecalho SEM o proprio campo, e o corpo COMO ELE VAI
        // AO DISCO: um motivo adulterado tem de ser detectado como qualquer
        // outro dado, e a varredura confere o arquivo sem precisar da chave.
        let mut crc = crc32(&buf[..44]);
        crc = phxsql_core::crc::crc32_with(crc, &buf[REGISTRO_CAB..]);
        por_u32(&mut buf, 44, crc);
        buf
    }

    /// Le a partir de `src`, que precisa ter o registro inteiro.
    fn ler(src: &[u8], cab: &Cabecalho, offset: u64, nome: &str) -> Result<Motivo> {
        if src.len() < REGISTRO_CAB {
            return Err(PhxError::Corrompido("registro de .reason truncado".into()));
        }
        let c = Campos(src);
        let n_motivo = u16::from_le_bytes([src[10], src[11]]) as usize;
        let n_ident = u16::from_le_bytes([src[40], src[41]]) as usize;
        let total = cab.ocupa(n_motivo + n_ident) + REGISTRO_CAB;
        if src.len() < total {
            return Err(PhxError::Corrompido(
                "registro de .reason menor que os tamanhos que declara".into(),
            ));
        }
        let mut crc = crc32(&src[..44]);
        crc = phxsql_core::crc::crc32_with(crc, &src[REGISTRO_CAB..total]);
        if crc != c.u32(44) {
            return Err(PhxError::Corrompido(
                "registro de .reason com CRC invalido".into(),
            ));
        }
        let uuid = Uuid::de_bytes(src[24..40].try_into().unwrap());
        let claro = cab.abrir(
            tempero(uuid.bytes()),
            offset,
            &associado(&src[..REGISTRO_CAB]),
            &src[REGISTRO_CAB..total],
            nome,
        )?;
        let texto = |a: usize, b: usize| -> Result<String> {
            String::from_utf8(claro[a..b].to_vec())
                .map_err(|e| PhxError::Corrompido(format!(".reason nao e UTF-8 valido: {e}")))
        };
        if claro.len() < n_motivo + n_ident {
            return Err(PhxError::Corrompido(
                "registro de .reason com menos texto do que declara".into(),
            ));
        }
        Ok(Motivo {
            uuid,
            carimbo: c.u64(0) as i64,
            tipo: Tipo::de_tag(src[8])?,
            rowid: c.u64(12),
            usuario: c.u32(20),
            motivo: texto(0, n_motivo)?,
            identidade: texto(n_motivo, n_motivo + n_ident)?,
        })
    }
}

/// O dado associado da etiqueta: o cabecalho do registro, menos o CRC.
///
/// O CRC fica de fora porque depende do corpo, que depende da etiqueta, que
/// depende do dado associado.
fn associado(cab: &[u8]) -> [u8; REGISTRO_CAB] {
    let mut aad = [0u8; REGISTRO_CAB];
    aad.copy_from_slice(&cab[..REGISTRO_CAB]);
    aad[44..48].fill(0);
    aad
}

/// Os quatro bytes de tempero do nonce saem do UUID do proprio registro.
///
/// Nao ha byte novo a gravar: o UUID v7 ja esta no cabecalho e ja e unico por
/// definicao. Ele cobre o unico caso em que o offset se repetiria -- o
/// registro que entra por cima de um rabo estragado por uma queda.
fn tempero(uuid: &[u8; 16]) -> [u8; 4] {
    [uuid[12], uuid[13], uuid[14], uuid[15]]
}

pub struct MotivoFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// Usuario aplicado aos registros gravados daqui em diante.
    pub usuario: u32,
}

impl MotivoFile {
    pub fn criar(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<MotivoFile> {
        // Ver `crate::diario`: o corte do diario e dele, e sem configuracao
        // manda o esquema.
        let paginacao = crate::diario::paginacao(paginacao);
        let mut m = MotivoFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REASON, paginacao),
            cabs: HashMap::new(),
            volume_atual: 1,
            usuario: 0,
        };
        m.volumes.criar(1)?;
        m.gravar_cab(Cabecalho::novo(1)?)?;
        Ok(m)
    }

    /// Abre; **cria se nao existir**.
    ///
    /// Tabela feita antes deste arquivo existir nao tem `.reason`, e recusar
    /// abrir por causa disso deixaria o banco inteiro inacessivel por um
    /// arquivo que ainda esta vazio de qualquer jeito.
    /// Abre SEM escrever nada, e devolve `None` quando abrir exigiria escrever.
    ///
    /// Pelo mesmo motivo da lixeira: o `abrir` CRIA o `.reason` quando falta, e
    /// criar arquivo sob a ficha compartilhada e dois leitores criando o mesmo
    /// arquivo ao mesmo tempo.
    pub fn abrir_sem_escrever(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<Option<MotivoFile>> {
        let pag = crate::diario::paginacao(paginacao);
        let volumes = Volumes::novo(&diretorio, nome, EXT_REASON, pag);
        if volumes.existentes().is_empty() {
            return Ok(None);
        }
        MotivoFile::abrir(diretorio, nome, paginacao).map(Some)
    }

    pub fn abrir(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<MotivoFile> {
        let paginacao = crate::diario::paginacao(paginacao);
        let volumes = Volumes::novo(&diretorio, nome, EXT_REASON, paginacao);
        if volumes.existentes().is_empty() {
            return MotivoFile::criar(diretorio, nome, paginacao);
        }
        let volume_atual = *volumes.existentes().last().unwrap();
        let mut m = MotivoFile {
            volumes,
            cabs: HashMap::new(),
            volume_atual,
            usuario: 0,
        };
        m.cab(1)?;
        m.cab(volume_atual)?;
        Ok(m)
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let cab = cofre::ler_cabecalho_do_volume(&mut self.volumes, volume, MAGIC_MOTIVO)?;
        self.cabs.insert(volume, cab);
        Ok(cab)
    }
    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        cofre::gravar_cabecalho_no_volume(&mut self.volumes, &cab, MAGIC_MOTIVO)?;
        self.cabs.insert(cab.volume, cab);
        Ok(())
    }

    /// Registra uma exclusao e devolve o que foi gravado.
    pub fn registrar(
        &mut self,
        tipo: Tipo,
        rowid: RowId,
        motivo: &str,
        identidade: &str,
    ) -> Result<Motivo> {
        let m = Motivo {
            uuid: Uuid::v7(),
            carimbo: agora_ms(),
            tipo,
            rowid,
            usuario: self.usuario,
            motivo: cortar(motivo, MOTIVO_MAX),
            identidade: cortar(identidade, IDENTIDADE_MAX),
        };
        self.anexar(&m)?;
        Ok(m)
    }

    fn anexar(&mut self, m: &Motivo) -> Result<()> {
        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let vazio = atual.fim <= atual.cab_len as u64;
        let ocupa = (REGISTRO_CAB + atual.ocupa(m.motivo.len() + m.identidade.len())) as u64;
        let (volume, virou) = paginacao.volume_externo(self.volume_atual, atual.fim, ocupa, vazio);

        let cab = if virou {
            if paginacao.ligada() && volume > paginacao.max_arquivos {
                return Err(PhxError::LimiteExcedido(format!(
                    "os motivos de {} chegaram ao teto de {} volumes",
                    self.volumes.nome(),
                    paginacao.max_arquivos
                )));
            }
            self.volumes.garantir(volume)?;
            let novo = Cabecalho::novo(volume)?;
            self.gravar_cab(novo)?;
            self.volume_atual = volume;
            novo
        } else {
            atual
        };

        // O offset entra no nonce: e ele o numero de ordem que um arquivo
        // append-only nunca reaproveita.
        let bytes = m.escrever(&cab, cab.fim);
        self.volumes.escrever(volume, cab.fim, &bytes)?;
        self.gravar_cab(cab.com(cab.fim + bytes.len() as u64, cab.quantos + 1))
    }

    /// Total de registros em todos os volumes.
    pub fn total(&mut self) -> Result<u64> {
        let mut t = 0;
        for v in self.volumes.existentes() {
            t += self.cab(v)?.quantos;
        }
        Ok(t)
    }

    /// Le em ordem cronologica. `limite` zero devolve tudo.
    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Motivo>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            let nome = self.volumes.caminho(volume).display().to_string();
            let mut offset = cab.cab_len as u64;
            while offset + REGISTRO_CAB as u64 <= cab.fim {
                let mut cabecalho = [0u8; REGISTRO_CAB];
                self.volumes.ler(volume, offset, &mut cabecalho)?;
                // O tamanho sai dos dois campos de texto MAIS a etiqueta,
                // quando o volume e cifrado: e o que deixa caminhar pelo
                // arquivo sem a chave.
                let n = REGISTRO_CAB
                    + cab.ocupa(
                        u16::from_le_bytes([cabecalho[10], cabecalho[11]]) as usize
                            + u16::from_le_bytes([cabecalho[40], cabecalho[41]]) as usize,
                    );
                if offset + n as u64 > cab.fim {
                    return Err(PhxError::Corrompido(format!(
                        "registro de .reason em {} passa do fim do volume",
                        self.volumes.caminho(volume).display()
                    )));
                }
                if vistos >= pular {
                    let mut buf = vec![0u8; n];
                    self.volumes.ler(volume, offset, &mut buf)?;
                    saida.push(Motivo::ler(&buf, &cab, offset, &nome)?);
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += n as u64;
            }
        }
        Ok(saida)
    }

    /// Os motivos de um registro, em ordem cronologica.
    pub fn de(&mut self, rowid: RowId) -> Result<Vec<Motivo>> {
        Ok(self
            .ler(0, 0)?
            .into_iter()
            .filter(|m| m.rowid == rowid)
            .collect())
    }

    /// Confere o CRC de todos os registros e a contagem dos cabecalhos.
    pub fn verificar(&mut self) -> Result<u64> {
        let quantos = self.ler(0, 0)?.len() as u64;
        let declarado = self.total()?;
        if quantos != declarado {
            return Err(PhxError::Corrompido(format!(
                "{}: os cabecalhos do .reason declaram {declarado} registros, \
                 e o arquivo tem {quantos}",
                self.volumes.nome()
            )));
        }
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
}

/// Corta em `max` BYTES sem partir caractere no meio.
///
/// Cortar por `char` custaria percorrer a string inteira; cortar por byte cru
/// produziria UTF-8 invalido que nem volta da leitura. Este anda para tras ate
/// o inicio de um caractere, o que sao no maximo tres passos.
fn cortar(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut fim = max;
    while fim > 0 && !s.is_char_boundary(fim) {
        fim -= 1;
    }
    s[..fim].to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn temp(nome: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("motivo-{nome}"))
    }

    #[test]
    fn grava_e_le_de_volta() {
        let d = temp("ida-e-volta");
        let mut m = MotivoFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        m.usuario = 7;
        m.registrar(Tipo::Suave, 1, "pedido do titular", "id=42")
            .unwrap();
        m.registrar(Tipo::Fisica, 2, "duplicidade", "id=43")
            .unwrap();

        let lidos = m.ler(0, 0).unwrap();
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[0].tipo, Tipo::Suave);
        assert_eq!(lidos[0].motivo, "pedido do titular");
        assert_eq!(lidos[0].identidade, "id=42");
        assert_eq!(lidos[0].usuario, 7);
        assert_eq!(lidos[1].rowid, 2);
        assert_eq!(m.total().unwrap(), 2);
        assert_eq!(m.verificar().unwrap(), 2);
    }

    /// Registros de tamanhos diferentes um atras do outro: se o avanco do
    /// offset usasse tamanho fixo, o segundo sairia deslocado.
    #[test]
    fn tamanhos_diferentes_seguidos() {
        let d = temp("tamanhos");
        let mut m = MotivoFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        let frases = ["", "a", "motivo bem mais longo que os outros", "xy"];
        for (i, f) in frases.iter().enumerate() {
            m.registrar(Tipo::Suave, i as u64 + 1, f, "").unwrap();
        }
        let lidos = m.ler(0, 0).unwrap();
        assert_eq!(lidos.len(), frases.len());
        for (i, f) in frases.iter().enumerate() {
            assert_eq!(lidos[i].motivo, *f);
            assert_eq!(lidos[i].rowid, i as u64 + 1);
        }
    }

    #[test]
    fn uuid_do_evento_e_crescente() {
        let d = temp("uuid");
        let mut m = MotivoFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 0..50 {
            m.registrar(Tipo::Suave, i, "", "").unwrap();
        }
        let lidos = m.ler(0, 0).unwrap();
        for par in lidos.windows(2) {
            assert!(
                par[0].uuid.bytes() < par[1].uuid.bytes(),
                "o v7 do evento saiu fora de ordem"
            );
        }
    }

    /// Adulterar o texto do motivo tem de ser pego. O CRC cobre os textos --
    /// se cobrisse so o cabecalho, trocar "fraude" por "engano" passaria.
    #[test]
    fn motivo_adulterado_nao_passa() {
        let mut m = Motivo {
            uuid: Uuid::v7(),
            carimbo: 1,
            tipo: Tipo::Suave,
            rowid: 1,
            usuario: 1,
            motivo: "fraude".into(),
            identidade: "id=1".into(),
        };
        // Cabecalho em claro: o que este teste prova e o CRC, e ele vale nos
        // dois modos -- a cifra so muda o que esta dentro do corpo.
        let cab = Cabecalho::novo(1).unwrap();
        let mut bytes = m.escrever(&cab, 64);
        assert!(Motivo::ler(&bytes, &cab, 64, "t").is_ok());
        let pos = REGISTRO_CAB;
        bytes[pos] = b'F';
        assert!(Motivo::ler(&bytes, &cab, 64, "t").is_err());

        // E a identidade tambem.
        m.motivo = "fraude".into();
        let mut bytes = m.escrever(&cab, 64);
        let pos = REGISTRO_CAB + m.motivo.len();
        bytes[pos] = b'X';
        assert!(Motivo::ler(&bytes, &cab, 64, "t").is_err());
    }

    #[test]
    fn motivo_gigante_e_cortado_sem_quebrar_utf8() {
        let d = temp("corte");
        let mut m = MotivoFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        // "ç" tem 2 bytes: o corte cai no meio dele se for cru.
        let longo = "ç".repeat(MOTIVO_MAX);
        m.registrar(Tipo::Suave, 1, &longo, "").unwrap();
        let lidos = m.ler(0, 0).unwrap();
        assert!(lidos[0].motivo.len() <= MOTIVO_MAX);
        assert!(longo.starts_with(&lidos[0].motivo));
    }

    #[test]
    fn abrir_cria_quando_nao_existe() {
        let d = temp("abre-e-cria");
        let mut m = MotivoFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(m.total().unwrap(), 0);
        m.registrar(Tipo::Fisica, 1, "", "").unwrap();
        drop(m);
        let mut de_novo = MotivoFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(de_novo.total().unwrap(), 1);
    }
}
