//! `.reg` -- a tabela fisica, na ordem de digitacao.
//!
//! O `.reg` e um heap de slots de largura fixa. O rowid e o numero do slot
//! dentro da TABELA (nao dentro do volume), comecando em 1, e o endereco sai
//! de uma conta, nao de uma busca:
//!
//! ```text
//! volume = (rowid - 1) / registros_por_arquivo + 1
//! slot   = (rowid - 1) % registros_por_arquivo + 1
//! offset = data_offset + (slot - 1) * slot_size
//! ```
//!
//! Sem paginacao o volume e sempre 1 e o slot e o proprio rowid.
//!
//! # Ordem de digitacao
//!
//! Registros sao SEMPRE anexados no fim. Excluir marca o slot como livre, mas
//! o slot nao e reaproveitado: isso manteria o arquivo compacto ao custo de
//! quebrar a garantia de que percorrer o `.reg` do inicio ao fim devolve os
//! registros na ordem em que foram digitados. O espaco de slots excluidos so
//! volta com uma compactacao explicita.
//!
//! Com paginacao a garantia continua valendo: o volume N+1 vem sempre depois
//! do N, e dentro de cada volume os slots seguem em ordem de insercao.
//!
//! # Layout de cada volume
//!
//! ```text
//! cabecalho     128 bytes  (bytes 36..44: proximo valor da sequencia)
//! esquema       schema_len bytes (serializado, auto-descritivo)
//! [alinhamento ate multiplo de 64]
//! slot 1, slot 2, ...
//!
//! slot: [status u8][flags u8][res u16][crc32 payload u32]
//!       [versao u64][res u64][payload ...]
//! ```
//!
//! Todo volume carrega o cabecalho completo com o esquema, entao qualquer um
//! deles se descreve sozinho. Apenas o volume 1 tem contadores autoritativos
//! da tabela inteira.

use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::Schema;
use phxsql_core::{RowId, EXT_REG};

use crate::util::{agora, conferir_magic, por_i64, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_REG: &[u8; 8] = b"PHXREG\0\0";
const CAB_LEN: usize = 128;
/// Bytes de cabecalho de cada slot, antes do payload.
pub const SLOT_CAB: usize = 24;
const VERSAO: u16 = 2;
const ALINHAMENTO: u64 = 64;

const STATUS_LIVRE: u8 = 0;
const STATUS_ATIVO: u8 = 1;

/// O byte de status so pode ser LIVRE ou ATIVO. Qualquer outro valor e
/// corrupcao, nao um estado.
///
/// A distincao importa mais do que parece. Enquanto qualquer coisa diferente
/// de ATIVO era tratada como "excluido", um unico bit trocado no cabecalho do
/// slot APAGAVA o registro em silencio: a leitura devolvia "nao existe" sem
/// erro, e o reparo considerava o slot bom e nunca ia buscar a copia no
/// espelho -- que estava la, inteira.
fn status_valido(b: u8) -> bool {
    b == STATUS_LIVRE || b == STATUS_ATIVO
}

/// Um slot esta integro quando o status e valido e, se ativo, o CRC bate.
fn slot_integro(slot: &[u8]) -> bool {
    match slot[0] {
        STATUS_LIVRE => true,
        STATUS_ATIVO => crc32(&slot[SLOT_CAB..]) == Campos(slot).u32(4),
        _ => false,
    }
}

pub struct RegFile {
    volumes: Volumes,
    esquema: Schema,
    slot_size: usize,
    data_offset: u64,
    slot_count: u64,
    live_count: u64,
    criado_em: i64,
    /// Proximo valor a sair da coluna `Sequence`, se a tabela tiver uma.
    ///
    /// Mora no cabecalho do volume 1, como os outros contadores da tabela
    /// inteira. Zero quer dizer "ainda nao usada", e o primeiro valor sai 1.
    /// Nunca anda para tras: excluir uma linha nao devolve o numero dela,
    /// pela mesma razao que o `.reg` nao reaproveita slot.
    proxima_sequencia: u64,
    /// Leituras salvas pelo espelho nesta sessao.
    recuperados: u64,
    /// Onde cada volume comeca, quando a particao e por periodo.
    ///
    /// Indice do vetor = volume - 1. Vazio quando a particao e por quantidade,
    /// porque ali o volume sai de uma divisao e nao ha o que guardar.
    fronteiras: Vec<Fronteira>,
}

/// O comeco de um volume: o primeiro rowid que ele recebeu e o periodo em que
/// foi aberto.
///
/// As faixas sao contiguas e crescentes -- o volume N+1 comeca no rowid
/// seguinte ao ultimo do N --, porque a ordem de digitacao manda: linha nova
/// vai sempre para o volume corrente, mesmo que a data dela seja de um periodo
/// ja fechado. Por isso achar o volume de um rowid e uma busca binaria, e nao
/// um indice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fronteira {
    pub primeiro_rowid: RowId,
    pub chave_periodo: i64,
}

/// Volume aberto e ainda sem nenhuma linha: nao ha periodo para gravar.
///
/// Acontece na criacao da tabela -- o volume 1 nasce antes da primeira
/// insercao. O primeiro registro adota o volume em vez de cortar um novo, para
/// a tabela nao nascer com um arquivo vazio.
pub const SEM_PERIODO: i64 = i64::MIN;

impl RegFile {
    pub fn criar(diretorio: impl AsRef<Path>, nome: &str, esquema: Schema) -> Result<RegFile> {
        let paginacao = esquema.paginacao();
        let bytes_esquema = esquema.serializar();
        let data_offset = alinhar(CAB_LEN as u64 + bytes_esquema.len() as u64, ALINHAMENTO);
        let slot_size = SLOT_CAB + esquema.payload_len();

        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, paginacao),
            esquema,
            slot_size,
            data_offset,
            slot_count: 0,
            live_count: 0,
            criado_em: agora(),
            proxima_sequencia: 0,
            recuperados: 0,
            fronteiras: Vec::new(),
        };
        if r.esquema.paginacao().modo.periodo().is_some() {
            r.fronteiras.push(Fronteira {
                primeiro_rowid: 1,
                chave_periodo: SEM_PERIODO,
            });
        }
        r.volumes.criar(1)?;
        r.gravar_cabecalho(1)?;
        Ok(r)
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str) -> Result<RegFile> {
        // A paginacao mora dentro do esquema, que mora dentro do primeiro
        // volume -- e a largura do sufixo faz parte dela. Para nao chutar,
        // acha-se o primeiro volume varrendo o diretorio e le-se o cabecalho
        // direto, antes de montar o conjunto de volumes.
        let primeiro = achar_primeiro_volume(diretorio.as_ref(), nome, EXT_REG)?;
        let nome_arq = primeiro.display().to_string();
        let bruto = std::fs::read(&primeiro)?;
        if bruto.len() < CAB_LEN {
            return Err(PhxError::Corrompido(format!("{nome_arq} truncado")));
        }
        let mut cab = [0u8; CAB_LEN];
        cab.copy_from_slice(&bruto[..CAB_LEN]);
        conferir_magic(&nome_arq, MAGIC_REG, &cab[0..8])?;

        let c = Campos(&cab);
        let versao = c.u16(8);
        if versao != VERSAO {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: nome_arq,
                encontrada: versao,
                suportada: VERSAO,
            });
        }
        if crc32(&cab[..124]) != c.u32(124) {
            return Err(PhxError::Corrompido(format!(
                "cabecalho de {nome_arq} com CRC invalido"
            )));
        }

        let slot_size = c.u32(16) as usize;
        let slot_count = c.u64(20);
        let live_count = c.u64(28);
        let proxima_sequencia = c.u64(36);
        let data_offset = c.u64(44);
        let schema_len = c.u32(52) as usize;
        let schema_crc = c.u32(56);
        let criado_em = c.u64(60) as i64;

        if bruto.len() < CAB_LEN + schema_len {
            return Err(PhxError::Corrompido(format!(
                "{nome_arq} nao contem o esquema inteiro"
            )));
        }
        let bytes_esquema = bruto[CAB_LEN..CAB_LEN + schema_len].to_vec();
        if crc32(&bytes_esquema) != schema_crc {
            return Err(PhxError::Corrompido(format!(
                "esquema de {nome_arq} com CRC invalido"
            )));
        }
        let esquema = Schema::desserializar(&bytes_esquema)?;

        let esperado = SLOT_CAB + esquema.payload_len();
        if slot_size != esperado {
            return Err(PhxError::Corrompido(format!(
                "slot_size {slot_size} em {nome_arq} nao bate com o esquema ({esperado})"
            )));
        }

        let mut r = RegFile {
            volumes: Volumes::novo(diretorio, nome, EXT_REG, esquema.paginacao()),
            esquema,
            slot_size,
            data_offset,
            slot_count,
            live_count,
            criado_em,
            proxima_sequencia,
            recuperados: 0,
            fronteiras: Vec::new(),
        };
        r.reler_fronteiras()?;
        Ok(r)
    }

    /// Remonta a tabela de fronteiras lendo o cabecalho de cada volume.
    ///
    /// So faz sentido na particao por periodo. Le poucos bytes por volume, uma
    /// vez, na abertura -- e volume e coisa que se conta em dezenas, nao em
    /// milhares, porque cada um guarda `registros_por_arquivo` linhas.
    fn reler_fronteiras(&mut self) -> Result<()> {
        self.fronteiras.clear();
        if self.esquema.paginacao().modo.periodo().is_none() {
            return Ok(());
        }
        for volume in self.volumes.existentes() {
            let mut cab = [0u8; CAB_LEN];
            self.volumes.ler(volume, 0, &mut cab)?;
            let c = Campos(&cab);
            self.fronteiras.push(Fronteira {
                primeiro_rowid: c.u64(76),
                chave_periodo: c.u64(84) as i64,
            });
        }
        // Um volume que existe mas nunca foi escrito na v3 vem com zero. Zero
        // nao e rowid: seria endereco 1 para tudo. Melhor recusar alto do que
        // devolver a linha errada em silencio.
        //
        // A tabela recem-criada e vazia nao cai aqui: o volume 1 dela ja nasce
        // com `primeiro_rowid = 1` e periodo indefinido.
        if let Some(i) = self.fronteiras.iter().position(|f| f.primeiro_rowid == 0) {
            return Err(PhxError::Corrompido(format!(
                "volume {} de {} nao tem fronteira gravada; a tabela foi criada \
                 antes da particao por periodo e precisa ser recriada",
                i + 1,
                self.volumes.nome()
            )));
        }
        Ok(())
    }

    /// As fronteiras de volume, para quem quiser mostra-las.
    pub fn fronteiras(&self) -> &[Fronteira] {
        &self.fronteiras
    }

    /// Toma o proximo valor da sequencia e avanca o contador.
    ///
    /// O cabecalho so vai para o disco no `sincronizar`, junto com os demais
    /// contadores da tabela. Se a maquina cair antes disso, o contador volta
    /// atras e valores ja gravados podem repetir -- e por isso que a
    /// sequencia nao serve como chave unica sozinha. Quem precisa de unicidade
    /// declara um indice `unico` sobre ela, e ai o proprio indice recusa a
    /// repeticao.
    pub fn proxima_da_sequencia(&mut self) -> u64 {
        self.proxima_sequencia = self.proxima_sequencia.max(1);
        let v = self.proxima_sequencia;
        self.proxima_sequencia += 1;
        v
    }

    /// Empurra o contador para depois de um valor gravado a mao.
    ///
    /// Sem isto, inserir a sequencia 500 na mao e depois deixar o motor
    /// numerar devolveria 1, 2, 3... por cima do que ja existe.
    pub fn anotar_sequencia(&mut self, usado: u64) {
        if usado >= self.proxima_sequencia {
            self.proxima_sequencia = usado + 1;
        }
    }

    /// Proximo valor que a sequencia vai devolver. 0 = ainda nao usada.
    pub fn sequencia_atual(&self) -> u64 {
        self.proxima_sequencia
    }

    fn gravar_cabecalho(&mut self, volume: u32) -> Result<()> {
        let bytes_esquema = self.esquema.serializar();
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(MAGIC_REG);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, volume);
        por_u32(&mut buf, 16, self.slot_size as u32);
        // Contadores da tabela inteira: so o volume 1 e autoritativo.
        if volume == 1 {
            por_u64(&mut buf, 20, self.slot_count);
            por_u64(&mut buf, 28, self.live_count);
            por_u64(&mut buf, 36, self.proxima_sequencia);
        }
        por_u64(&mut buf, 44, self.data_offset);
        por_u32(&mut buf, 52, bytes_esquema.len() as u32);
        por_u32(&mut buf, 56, crc32(&bytes_esquema));
        por_i64(&mut buf, 60, self.criado_em);
        por_i64(&mut buf, 68, agora());
        // A fronteira deste volume, na particao por periodo. Cada volume
        // carrega a sua, e por isso a tabela se remonta lendo os cabecalhos --
        // sem arquivo extra e sem bloco que cresce.
        if let Some(f) = self.fronteiras.get(volume as usize - 1) {
            por_u64(&mut buf, 76, f.primeiro_rowid);
            por_u64(&mut buf, 84, f.chave_periodo as u64);
        }
        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);

        self.volumes.escrever(volume, 0, &buf)?;
        self.volumes
            .escrever(volume, CAB_LEN as u64, &bytes_esquema)?;
        if self.volumes.tamanho(volume)? < self.data_offset {
            self.volumes.definir_tamanho(volume, self.data_offset)?;
        }
        Ok(())
    }

    /// Quantas leituras foram salvas pelo espelho desde que a tabela abriu.
    ///
    /// Nao e curiosidade: recuperacao silenciosa e a pior especie. Se este
    /// numero sobe, alguma coisa esta estragando dado, e alguem precisa saber.
    pub fn recuperados(&self) -> u64 {
        self.recuperados
    }

    /// Percorre todos os slots e conserta os que o espelho consegue salvar.
    ///
    /// Devolve (conferidos, reparados, perdidos). Repara nos DOIS sentidos:
    /// se o principal esta bom e o espelho nao, o espelho e reescrito -- senao
    /// a segunda chance de amanha ja nasceria queimada.
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        if !self.volumes.tem_espelho() {
            return Err(PhxError::Esquema(
                "esta tabela nao tem espelho: ligue \"espelho\" no config.json antes".into(),
            ));
        }
        let (mut conferidos, mut reparados, mut perdidos) = (0u64, 0u64, 0u64);
        let bom = slot_integro;
        for rowid in 1..=self.slot_count {
            let (volume, offset) = self.localizar(rowid);
            let mut principal = vec![0u8; self.slot_size];
            let mut copia = vec![0u8; self.slot_size];
            if self.volumes.ler(volume, offset, &mut principal).is_err() {
                perdidos += 1;
                continue;
            }
            conferidos += 1;
            let copia_ok = self
                .volumes
                .ler_do_espelho(volume, offset, &mut copia)
                .is_ok()
                && bom(&copia);
            match (bom(&principal), copia_ok) {
                (true, true) => {}
                // O principal quebrou e o espelho salvou.
                (false, true) => {
                    self.volumes.escrever(volume, offset, &copia)?;
                    reparados += 1;
                }
                // O espelho quebrou; o principal reescreve o espelho.
                (true, false) => {
                    self.volumes
                        .escrever_no_espelho(volume, offset, &principal)?;
                    reparados += 1;
                }
                (false, false) => perdidos += 1,
            }
        }
        self.volumes.sincronizar()?;
        Ok((conferidos, reparados, perdidos))
    }

    /// Liga o espelho `.bkp`. Chamado logo depois de abrir ou criar, e antes
    /// de qualquer escrita -- ligar no meio deixaria o espelho comecando pela
    /// metade, que e pior do que nao ter espelho nenhum.
    pub fn espelhar(&mut self) -> Result<()> {
        let volumes = std::mem::replace(
            &mut self.volumes,
            Volumes::novo(".", "", phxsql_core::EXT_REG, self.esquema.paginacao()),
        );
        self.volumes = volumes.com_espelho(phxsql_core::EXT_BKP);
        // Semeia SO o que ainda nao existe do outro lado.
        //
        // Copiar por cima de um espelho que ja existe seria destruir a copia
        // boa com a principal, que e exatamente o contrario do que ele serve.
        // Um teste pegou isso: estragar o principal e religar o espelho
        // apagava a segunda chance. Espelho fora de sincronia se acerta com
        // `reparar`, que olha os dois lados antes de escrever em qualquer um.
        for volume in self.volumes.existentes() {
            let tamanho = self.volumes.tamanho(volume)?;
            if self.volumes.tamanho_do_espelho(volume)? == tamanho {
                continue; // ja existe e tem o tamanho certo: nao toca
            }
            let mut buf = vec![0u8; tamanho as usize];
            self.volumes.ler(volume, 0, &mut buf)?;
            self.volumes.escrever_no_espelho(volume, 0, &buf)?;
        }
        self.volumes.sincronizar()?;
        Ok(())
    }

    pub fn tem_espelho(&self) -> bool {
        self.volumes.tem_espelho()
    }

    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }

    pub fn caminho(&self, volume: u32) -> PathBuf {
        self.volumes.caminho(volume)
    }

    pub fn volumes(&self) -> Vec<u32> {
        self.volumes.existentes()
    }

    pub fn paginacao(&self) -> Paginacao {
        self.esquema.paginacao()
    }

    /// Total de slots ja alocados, incluindo os excluidos.
    /// Tambem e o maior rowid ja atribuido.
    pub fn slots(&self) -> u64 {
        self.slot_count
    }

    /// Registros ativos.
    pub fn registros(&self) -> u64 {
        self.live_count
    }

    pub fn slot_size(&self) -> usize {
        self.slot_size
    }

    /// Volume e offset em que um rowid mora.
    ///
    /// Na particao por quantidade e uma divisao. Na particao por periodo o
    /// volume nao sai de conta -- ele depende de quando o periodo virou --,
    /// entao sai de uma busca binaria na tabela de fronteiras. Nos dois casos
    /// o offset dentro do volume continua sendo multiplicacao.
    fn localizar(&self, rowid: RowId) -> (u32, u64) {
        let (volume, slot) = match self.volume_por_fronteira(rowid) {
            Some(v) => (
                v,
                rowid - self.fronteiras[v as usize - 1].primeiro_rowid + 1,
            ),
            None => self.esquema.paginacao().localizar(rowid),
        };
        (
            volume,
            self.data_offset + (slot - 1) * self.slot_size as u64,
        )
    }

    /// Decide em que volume a linha nova entra, cortando se preciso.
    ///
    /// Corta em dois casos, e o segundo e o que a particao por periodo existe
    /// para fazer:
    ///
    /// 1. o volume corrente encheu (`registros_por_arquivo` continua sendo
    ///    teto, senao um mes movimentado estouraria o arquivo);
    /// 2. o periodo virou.
    ///
    /// O que ele NAO faz e mandar a linha para um volume anterior. Um
    /// lancamento de janeiro digitado em marco entra no volume de marco: a
    /// ordem de digitacao manda, e voltar significaria escrever no meio de um
    /// arquivo ja fechado.
    fn abrir_faixa_do_periodo(&mut self, rowid: RowId, chave: i64) -> Result<(u32, u64)> {
        let paginacao = self.esquema.paginacao();
        let corta = match self.fronteiras.last() {
            None => true,
            // Volume ainda vazio: ele ADOTA o periodo da primeira linha. Sem
            // isto a tabela nasceria com um volume 1 vazio e a primeira linha
            // iria para o volume 2.
            Some(f) if rowid == f.primeiro_rowid => {
                if f.chave_periodo != chave {
                    let ultimo = self.fronteiras.len() - 1;
                    self.fronteiras[ultimo].chave_periodo = chave;
                }
                false
            }
            Some(f) => {
                let no_volume = rowid - f.primeiro_rowid;
                no_volume >= paginacao.registros_por_arquivo || f.chave_periodo != chave
            }
        };
        if corta {
            if self.fronteiras.len() as u64 >= paginacao.max_arquivos as u64 {
                return Err(PhxError::LimiteExcedido(format!(
                    "tabela {} cheia: {} volumes, o teto do sufixo de {} digitos",
                    self.volumes.nome(),
                    paginacao.max_arquivos,
                    paginacao.digitos
                )));
            }
            self.fronteiras.push(Fronteira {
                primeiro_rowid: rowid,
                chave_periodo: chave,
            });
        }
        let volume = self.fronteiras.len() as u32;
        let f = self.fronteiras[volume as usize - 1];
        Ok((
            volume,
            self.data_offset + (rowid - f.primeiro_rowid) * self.slot_size as u64,
        ))
    }

    /// O ultimo volume que comeca em rowid menor ou igual ao pedido.
    ///
    /// `None` quando nao ha fronteiras -- ou seja, quando a particao e por
    /// quantidade e o volume sai de divisao.
    fn volume_por_fronteira(&self, rowid: RowId) -> Option<u32> {
        if self.fronteiras.is_empty() {
            return None;
        }
        let i = self
            .fronteiras
            .partition_point(|f| f.primeiro_rowid <= rowid);
        Some(i.max(1) as u32)
    }

    fn conferir_faixa(&self, rowid: RowId) -> Result<()> {
        if rowid == 0 || rowid > self.slot_count {
            return Err(PhxError::NaoEncontrado(format!(
                "rowid {rowid} fora da faixa 1..={} em {}",
                self.slot_count,
                self.volumes.nome()
            )));
        }
        Ok(())
    }

    /// Anexa um registro no fim e devolve seu rowid.
    pub fn inserir(&mut self, payload: &[u8]) -> Result<RowId> {
        self.inserir_no_periodo(payload, None)
    }

    /// Anexa, dizendo em que periodo a linha cai.
    ///
    /// A chave do periodo vem de cima porque o `.reg` so conhece bytes: quem
    /// sabe ler a coluna de data e a `Table`, que tem o esquema e os valores.
    /// Na particao por quantidade a chave e ignorada.
    pub fn inserir_no_periodo(&mut self, payload: &[u8], chave: Option<i64>) -> Result<RowId> {
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let rowid = self.slot_count + 1;
        let paginacao = self.esquema.paginacao();
        let por_periodo = paginacao.modo.periodo().is_some();

        if !por_periodo && !paginacao.cabe(rowid) {
            return Err(PhxError::LimiteExcedido(format!(
                "tabela {} cheia: capacidade de {} registros ({} por arquivo x {} arquivos)",
                self.volumes.nome(),
                paginacao.capacidade(),
                paginacao.registros_por_arquivo,
                paginacao.max_arquivos
            )));
        }

        let (volume, offset) = if por_periodo {
            self.abrir_faixa_do_periodo(rowid, chave.unwrap_or(0))?
        } else {
            self.localizar(rowid)
        };
        if self.volumes.garantir(volume)? {
            // Volume novo: ganha cabecalho e esquema proprios.
            self.gravar_cabecalho(volume)?;
        }

        let mut slot = vec![0u8; self.slot_size];
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, 1); // versao do registro
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;

        self.slot_count += 1;
        self.live_count += 1;
        self.gravar_cabecalho(1)?;
        Ok(rowid)
    }

    /// Le o payload de um registro. Devolve `None` se o slot foi excluido.
    pub fn ler(&mut self, rowid: RowId) -> Result<Option<Vec<u8>>> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut slot = vec![0u8; self.slot_size];
        self.volumes.ler(volume, offset, &mut slot)?;

        // Slot livre e resposta, nao defeito: o registro foi excluido.
        if slot[0] == STATUS_LIVRE {
            return Ok(None);
        }

        // Daqui para baixo o slot deveria estar ativo. Se o status nao for
        // nem LIVRE nem ATIVO, o cabecalho do slot esta corrompido -- e ANTES
        // esse caso caia no `return Ok(None)` acima, respondendo "esse
        // registro nao existe" para um registro que existe e esta inteiro do
        // outro lado. Agora ele desce para a segunda chance junto com a falha
        // de CRC, que e o mesmo problema com outro sintoma.
        let cabecalho_torto = !status_valido(slot[0]);
        let payload = slot[SLOT_CAB..].to_vec();
        if cabecalho_torto || crc32(&payload) != Campos(&slot).u32(4) {
            // A segunda chance: se ha espelho, o outro lado pode estar bom.
            if self.volumes.tem_espelho() {
                let mut copia = vec![0u8; self.slot_size];
                if self
                    .volumes
                    .ler_do_espelho(volume, offset, &mut copia)
                    .is_ok()
                    && copia[0] == STATUS_ATIVO
                {
                    let dele = copia[SLOT_CAB..].to_vec();
                    if crc32(&dele) == Campos(&copia).u32(4) {
                        self.recuperados += 1;
                        return Ok(Some(dele));
                    }
                }
            }
            return Err(PhxError::Corrompido(format!(
                "{} do registro {rowid} em {}{}",
                if cabecalho_torto {
                    format!("status invalido ({})", slot[0])
                } else {
                    "CRC nao confere".to_string()
                },
                self.volumes.caminho(volume).display(),
                if self.volumes.tem_espelho() {
                    " -- e o espelho tambem nao tem uma copia boa"
                } else {
                    " -- sem espelho para tentar; ligue \"espelho\" no config.json"
                }
            )));
        }
        Ok(Some(payload))
    }

    pub fn ativo(&mut self, rowid: RowId) -> Result<bool> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut b = [0u8; 1];
        self.volumes.ler(volume, offset, &mut b)?;
        Ok(b[0] == STATUS_ATIVO)
    }

    /// Regrava o payload de um registro existente, no mesmo slot.
    /// O rowid e a posicao fisica nao mudam.
    /// Devolve a nova versao do registro.
    pub fn atualizar(&mut self, rowid: RowId, payload: &[u8]) -> Result<u64> {
        self.conferir_faixa(rowid)?;
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let (volume, offset) = self.localizar(rowid);
        let mut slot = vec![0u8; self.slot_size];
        self.volumes.ler(volume, offset, &mut slot)?;
        if slot[0] != STATUS_ATIVO {
            return Err(PhxError::NaoEncontrado(format!(
                "registro {rowid} esta excluido"
            )));
        }
        let versao = Campos(&slot).u64(8).saturating_add(1);
        slot[..SLOT_CAB].fill(0);
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, versao);
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;
        self.gravar_cabecalho(1)?;
        Ok(versao)
    }

    /// Marca o registro como excluido. Devolve `false` se ja estava excluido.
    pub fn excluir(&mut self, rowid: RowId) -> Result<bool> {
        self.conferir_faixa(rowid)?;
        let (volume, offset) = self.localizar(rowid);
        let mut cab = [0u8; SLOT_CAB];
        self.volumes.ler(volume, offset, &mut cab)?;
        if cab[0] != STATUS_ATIVO {
            return Ok(false);
        }
        cab[0] = STATUS_LIVRE;
        self.volumes.escrever(volume, offset, &cab)?;
        self.live_count = self.live_count.saturating_sub(1);
        self.gravar_cabecalho(1)?;
        Ok(true)
    }

    /// Proximo registro ativo com rowid >= `desde`, na ordem de digitacao.
    pub fn proximo_ativo(&mut self, desde: RowId) -> Result<Option<(RowId, Vec<u8>)>> {
        let mut rowid = desde.max(1);
        while rowid <= self.slot_count {
            if let Some(p) = self.ler(rowid)? {
                return Ok(Some((rowid, p)));
            }
            rowid += 1;
        }
        Ok(None)
    }

    /// Confere o CRC de todos os registros ativos e a contagem do cabecalho.
    pub fn verificar(&mut self) -> Result<u64> {
        let mut vivos = 0u64;
        for rowid in 1..=self.slot_count {
            if self.ler(rowid)?.is_some() {
                vivos += 1;
            }
        }
        if vivos != self.live_count {
            return Err(PhxError::Corrompido(format!(
                "{}: cabecalho diz {} registros, varredura achou {vivos}",
                self.volumes.nome(),
                self.live_count
            )));
        }
        Ok(vivos)
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.volumes.sincronizar()
    }
}

fn alinhar(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// Acha o primeiro volume de um conjunto sem saber, de antemao, se a tabela e
/// paginada nem qual a largura do sufixo.
///
/// Procura primeiro `nome.ext` (tabela em arquivo unico); se nao existir,
/// varre o diretorio atras de `nome_<digitos>.ext` e devolve o menor.
fn achar_primeiro_volume(diretorio: &Path, nome: &str, ext: &str) -> Result<PathBuf> {
    let simples = diretorio.join(format!("{nome}.{ext}"));
    if simples.exists() {
        return Ok(simples);
    }
    let prefixo = format!("{nome}_");
    let mut candidatos: Vec<PathBuf> = std::fs::read_dir(diretorio)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if p.extension().and_then(|s| s.to_str()) != Some(ext) {
                return false;
            }
            match p.file_stem().and_then(|s| s.to_str()) {
                Some(base) => match base.strip_prefix(&prefixo) {
                    Some(sufixo) => {
                        !sufixo.is_empty() && sufixo.chars().all(|c| c.is_ascii_digit())
                    }
                    None => false,
                },
                None => false,
            }
        })
        .collect();
    candidatos.sort();
    candidatos.into_iter().next().ok_or_else(|| {
        PhxError::NaoEncontrado(format!(
            "nenhum volume de {nome}.{ext} em {}",
            diretorio.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-reg-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn esquema() -> Schema {
        Schema::new(
            "cadastroClientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(30)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    fn payload(esq: &Schema, n: u8) -> Vec<u8> {
        let mut p = vec![0u8; esq.payload_len()];
        p[esq.bitmap_len()] = n;
        p
    }

    #[test]
    fn insere_le_e_conta() {
        let d = dir_temp("insere");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        assert_eq!(r.inserir(&payload(&esq, 10)).unwrap(), 1);
        assert_eq!(r.inserir(&payload(&esq, 20)).unwrap(), 2);
        assert_eq!(r.inserir(&payload(&esq, 30)).unwrap(), 3);
        assert_eq!(r.slots(), 3);
        assert_eq!(r.registros(), 3);
        assert_eq!(r.ler(2).unwrap().unwrap(), payload(&esq, 20));
        assert_eq!(r.verificar().unwrap(), 3);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn exclusao_nao_reaproveita_slot_e_preserva_a_ordem() {
        let d = dir_temp("ordem");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=5u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        assert!(r.excluir(3).unwrap());
        assert!(!r.excluir(3).unwrap());
        assert_eq!(r.registros(), 4);
        assert_eq!(r.slots(), 5);
        assert_eq!(r.inserir(&payload(&esq, 6)).unwrap(), 6);
        assert!(r.ler(3).unwrap().is_none());

        let mut vistos = Vec::new();
        let mut rowid = 1;
        while let Some((id, p)) = r.proximo_ativo(rowid).unwrap() {
            vistos.push((id, p[esq.bitmap_len()]));
            rowid = id + 1;
        }
        assert_eq!(vistos, vec![(1, 1), (2, 2), (4, 4), (5, 5), (6, 6)]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn atualiza_no_mesmo_slot() {
        let d = dir_temp("update");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        let id = r.inserir(&payload(&esq, 1)).unwrap();
        assert_eq!(r.atualizar(id, &payload(&esq, 99)).unwrap(), 2);
        assert_eq!(r.ler(id).unwrap().unwrap()[esq.bitmap_len()], 99);
        assert_eq!(r.slots(), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn atualizar_excluido_e_erro() {
        let d = dir_temp("upd-excl");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        let id = r.inserir(&payload(&esq, 1)).unwrap();
        r.excluir(id).unwrap();
        assert!(r.atualizar(id, &payload(&esq, 2)).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn reabre_com_esquema_auto_descritivo() {
        let d = dir_temp("reabre");
        let esq = esquema();
        {
            let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
            r.inserir(&payload(&esq, 7)).unwrap();
            r.sincronizar().unwrap();
        }
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert_eq!(r.esquema(), &esq);
        assert_eq!(r.esquema().nome(), "cadastroClientes");
        assert_eq!(r.ler(1).unwrap().unwrap()[esq.bitmap_len()], 7);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rowid_fora_da_faixa_e_erro() {
        let d = dir_temp("faixa");
        let mut r = RegFile::criar(&d, "cadastroClientes", esquema()).unwrap();
        assert!(r.ler(0).is_err());
        assert!(r.ler(1).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn registro_adulterado_falha_no_crc() {
        let d = dir_temp("crc");
        let esq = esquema();
        let offset = {
            let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
            r.inserir(&payload(&esq, 5)).unwrap();
            r.sincronizar().unwrap();
            r.localizar(1).1
        };
        {
            let mut v = Volumes::novo(&d, "cadastroClientes", "reg", Paginacao::DESLIGADA);
            v.escrever(1, offset + SLOT_CAB as u64 + 1, b"\xFF")
                .unwrap();
        }
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert!(r.ler(1).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    // ------------------------------------------------------------ paginacao

    fn esquema_paginado(registros: u64, arquivos: u32) -> Schema {
        esquema()
            .com_paginacao(Paginacao::nova(registros, arquivos).unwrap())
            .unwrap()
    }

    #[test]
    fn paginacao_distribui_em_volumes_numerados() {
        let d = dir_temp("pag");
        let esq = esquema_paginado(10, 99);
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 0..25u32 {
            r.inserir(&payload(&esq, (n % 250) as u8)).unwrap();
        }
        assert_eq!(r.slots(), 25);
        assert_eq!(r.volumes(), vec![1, 2, 3]);
        assert!(d.join("cadastroClientes_001.reg").exists());
        assert!(d.join("cadastroClientes_002.reg").exists());
        assert!(d.join("cadastroClientes_003.reg").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn paginado_le_escreve_e_reabre_igual() {
        let d = dir_temp("pag-rw");
        let esq = esquema_paginado(10, 99);
        {
            let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
            for n in 1..=100u8 {
                r.inserir(&payload(&esq, n)).unwrap();
            }
            r.sincronizar().unwrap();
        }
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert_eq!(r.slots(), 100);
        assert_eq!(r.paginacao().registros_por_arquivo, 10);
        // Cada rowid volta com o conteudo certo, atravessando 10 volumes.
        for n in 1..=100u64 {
            assert_eq!(
                r.ler(n).unwrap().unwrap()[esq.bitmap_len()],
                n as u8,
                "rowid {n}"
            );
        }
        assert_eq!(r.verificar().unwrap(), 100);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn paginado_preserva_a_ordem_de_digitacao_entre_volumes() {
        let d = dir_temp("pag-ordem");
        let esq = esquema_paginado(4, 99);
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=20u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        r.excluir(7).unwrap();
        r.excluir(13).unwrap();

        let mut vistos = Vec::new();
        let mut rowid = 1;
        while let Some((id, p)) = r.proximo_ativo(rowid).unwrap() {
            vistos.push(p[esq.bitmap_len()]);
            rowid = id + 1;
        }
        let esperado: Vec<u8> = (1..=20u8).filter(|n| *n != 7 && *n != 13).collect();
        assert_eq!(vistos, esperado);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn tabela_cheia_para_de_aceitar() {
        let d = dir_temp("cheia");
        let esq = esquema_paginado(3, 2); // capacidade 6
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=6u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        let e = r.inserir(&payload(&esq, 7)).unwrap_err();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "erro foi {e}");
        assert_eq!(r.registros(), 6);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn todo_volume_se_descreve_sozinho() {
        let d = dir_temp("autodesc");
        let esq = esquema_paginado(5, 99);
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        for n in 1..=12u8 {
            r.inserir(&payload(&esq, n)).unwrap();
        }
        r.sincronizar().unwrap();

        // O volume 3 carrega assinatura, versao e esquema proprios.
        let mut v = Volumes::novo(&d, "cadastroClientes", "reg", esq.paginacao());
        let mut cab = [0u8; CAB_LEN];
        v.ler(3, 0, &mut cab).unwrap();
        assert_eq!(&cab[0..8], MAGIC_REG);
        let c = Campos(&cab);
        assert_eq!(c.u16(8), VERSAO);
        assert_eq!(c.u32(12), 3, "o volume sabe o proprio numero");
        let schema_len = c.u32(52) as usize;
        let mut bytes = vec![0u8; schema_len];
        v.ler(3, CAB_LEN as u64, &mut bytes).unwrap();
        assert_eq!(Schema::desserializar(&bytes).unwrap(), esq);
        std::fs::remove_dir_all(&d).unwrap();
    }
    #[test]
    fn o_espelho_salva_um_registro_estragado() {
        let d = dir_temp("espelho");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        assert!(r.tem_espelho());

        let mut ids = Vec::new();
        for i in 0..20u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            p[1] = i.wrapping_mul(3);
            ids.push(r.inserir(&p).unwrap());
        }
        r.sincronizar().unwrap();
        // O .bkp existe e tem o mesmo tamanho do .reg.
        let reg = d.join("cadastroClientes.reg");
        let bkp = d.join("cadastroClientes.bkp");
        assert!(bkp.is_file(), "o espelho nao foi criado");
        assert_eq!(
            std::fs::metadata(&reg).unwrap().len(),
            std::fs::metadata(&bkp).unwrap().len()
        );

        let antes = r.ler(7).unwrap().unwrap();
        drop(r);

        // Estraga um byte do payload do registro 7 SO no principal.
        let mut r2 = RegFile::abrir(&d, "cadastroClientes").unwrap();
        let (volume, offset) = r2.localizar(7);
        let mut slot = vec![0u8; r2.slot_size];
        r2.volumes.ler(volume, offset, &mut slot).unwrap();
        slot[SLOT_CAB] ^= 0xff;
        r2.volumes.escrever(volume, offset, &slot).unwrap();
        r2.sincronizar().unwrap();
        drop(r2);

        // Sem espelho: a leitura acusa corrupcao, como tem de acusar.
        let mut sem = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert!(sem.ler(7).is_err(), "sem espelho, tem de recusar");
        drop(sem);

        // Com espelho: a leitura volta certa, e o contador registra.
        let mut com = RegFile::abrir(&d, "cadastroClientes").unwrap();
        com.espelhar().unwrap();
        assert_eq!(com.recuperados(), 0);
        assert_eq!(com.ler(7).unwrap().unwrap(), antes, "o espelho nao salvou");
        assert_eq!(com.recuperados(), 1, "a recuperacao tem de aparecer");
        // Os vizinhos continuam saindo do principal, sem contar recuperacao.
        assert!(com.ler(6).unwrap().is_some());
        assert_eq!(com.recuperados(), 1);
    }

    #[test]
    fn status_torto_nao_apaga_o_registro_em_silencio() {
        // O defeito que este teste existe para impedir, achado com um servidor
        // de verdade e o .reg estragado a mao: um unico byte trocado no
        // cabecalho do slot fazia o registro DESAPARECER sem erro nenhum.
        //
        // A leitura via `slot[0] != ATIVO` e respondia "nao existe" -- que e a
        // resposta certa para um registro excluido e a resposta errada para um
        // registro inteiro que esta ali do lado, no espelho. E o `reparar`
        // considerava o slot bom pelo mesmo motivo, entao nem consertava.
        let d = dir_temp("status-torto");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        for i in 0..12u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            r.inserir(&p).unwrap();
        }
        r.sincronizar().unwrap();
        let antes = r.ler(7).unwrap().unwrap();
        drop(r);

        // Estraga SO o byte de status do registro 7, e so no principal.
        let mut r2 = RegFile::abrir(&d, "cadastroClientes").unwrap();
        let (volume, offset) = r2.localizar(7);
        let mut slot = vec![0u8; r2.slot_size];
        r2.volumes.ler(volume, offset, &mut slot).unwrap();
        assert_eq!(slot[0], STATUS_ATIVO);
        slot[0] = 0xFE; // nem livre, nem ativo: lixo
        r2.volumes
            .escrever_so_no_principal(volume, offset, &slot)
            .unwrap();
        r2.sincronizar().unwrap();
        drop(r2);

        // Sem espelho: tem de ACUSAR, nunca responder "nao existe".
        let mut sem = RegFile::abrir(&d, "cadastroClientes").unwrap();
        let erro = sem.ler(7);
        assert!(erro.is_err(), "status invalido virou 'registro nao existe'");
        assert!(
            format!("{}", erro.unwrap_err()).contains("status invalido"),
            "a mensagem tem de dizer o que aconteceu"
        );
        drop(sem);

        // Com espelho: a leitura volta certa e conta a recuperacao.
        let mut com = RegFile::abrir(&d, "cadastroClientes").unwrap();
        com.espelhar().unwrap();
        assert_eq!(com.ler(7).unwrap().unwrap(), antes, "o espelho nao salvou");
        assert_eq!(com.recuperados(), 1);

        // E o reparo tem de consertar o principal, nao dar o slot por bom.
        let (conferidos, reparados, perdidos) = com.reparar().unwrap();
        assert_eq!((conferidos, reparados, perdidos), (12, 1, 0));

        // Depois do reparo o principal se basta: sem espelho, le certo.
        drop(com);
        let mut so_principal = RegFile::abrir(&d, "cadastroClientes").unwrap();
        assert_eq!(so_principal.ler(7).unwrap().unwrap(), antes);
        assert_eq!(
            so_principal.recuperados(),
            0,
            "nao devia precisar do espelho"
        );
    }

    #[test]
    fn slot_livre_continua_sendo_resposta_e_nao_defeito() {
        // O contraponto do teste acima: excluir DEVE devolver "nao existe",
        // sem erro e sem consultar o espelho. Se o conserto tivesse passado do
        // ponto, toda exclusao viraria corrupcao.
        let d = dir_temp("livre-e-resposta");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        for i in 0..5u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            r.inserir(&p).unwrap();
        }
        r.excluir(3).unwrap();
        r.sincronizar().unwrap();

        assert!(r.ler(3).unwrap().is_none(), "excluido tem de devolver None");
        assert_eq!(r.recuperados(), 0, "exclusao nao pode acionar o espelho");
        let (_, reparados, perdidos) = r.reparar().unwrap();
        assert_eq!((reparados, perdidos), (0, 0), "nao ha o que reparar");
    }

    #[test]
    fn reparar_conserta_os_dois_lados() {
        let d = dir_temp("reparar");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        for i in 0..12u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            r.inserir(&p).unwrap();
        }
        r.sincronizar().unwrap();
        drop(r);

        // Estraga o registro 3 no principal e o 9 no espelho.
        let mut r = RegFile::abrir(&d, "cadastroClientes").unwrap();
        r.espelhar().unwrap();
        for (rowid, no_espelho) in [(3u64, false), (9u64, true)] {
            let (v, off) = r.localizar(rowid);
            let mut slot = vec![0u8; r.slot_size];
            if no_espelho {
                r.volumes.ler_do_espelho(v, off, &mut slot).unwrap();
                slot[SLOT_CAB] ^= 0x5a;
                r.volumes.escrever_no_espelho(v, off, &slot).unwrap();
            } else {
                r.volumes.ler(v, off, &mut slot).unwrap();
                slot[SLOT_CAB] ^= 0x5a;
                // escrever() duplica no espelho; aqui queremos SO o principal.
                r.volumes.escrever_so_no_principal(v, off, &slot).unwrap();
            }
        }
        r.sincronizar().unwrap();

        let (conferidos, reparados, perdidos) = r.reparar().unwrap();
        assert_eq!(conferidos, 12);
        assert_eq!(reparados, 2, "um de cada lado");
        assert_eq!(perdidos, 0);

        // Depois do reparo, tudo le sem precisar da segunda chance.
        let mut depois = RegFile::abrir(&d, "cadastroClientes").unwrap();
        depois.espelhar().unwrap();
        for rowid in 1..=12 {
            assert!(depois.ler(rowid).unwrap().is_some(), "rowid {rowid}");
        }
        assert_eq!(depois.recuperados(), 0, "nada precisou do espelho");
    }

    #[test]
    fn os_dois_lados_perdidos_nao_viram_dado_inventado() {
        let d = dir_temp("perdidos");
        let esq = esquema();
        let mut r = RegFile::criar(&d, "cadastroClientes", esq.clone()).unwrap();
        r.espelhar().unwrap();
        for i in 0..5u8 {
            let mut p = vec![0u8; esq.payload_len()];
            p[0] = i;
            r.inserir(&p).unwrap();
        }
        r.sincronizar().unwrap();
        // Estraga o 2 nos DOIS lados: nao ha o que salvar.
        let (v, off) = r.localizar(2);
        let mut slot = vec![0u8; r.slot_size];
        r.volumes.ler(v, off, &mut slot).unwrap();
        slot[SLOT_CAB] ^= 0xaa;
        r.volumes.escrever(v, off, &slot).unwrap(); // vai para os dois
        r.sincronizar().unwrap();

        assert!(
            r.ler(2).is_err(),
            "sem copia boa, tem de acusar e nao inventar"
        );
        let (_, reparados, perdidos) = r.reparar().unwrap();
        assert_eq!(reparados, 0);
        assert_eq!(perdidos, 1);
    }

    #[test]
    fn sem_espelho_reparar_recusa_em_vez_de_fingir() {
        let d = dir_temp("semespelho");
        let mut r = RegFile::criar(&d, "cadastroClientes", esquema()).unwrap();
        assert!(!r.tem_espelho());
        assert!(r.reparar().is_err());
    }
}
