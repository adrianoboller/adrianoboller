//! `.log` -- o diario da tabela.
//!
//! Toda inclusao, alteracao e exclusao e registrada com data e hora. O arquivo
//! e append-only e sem indice: e um diario, nao uma tabela.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log = cadastroClientes
//! ```
//!
//! # Evento: 44 bytes de cabecalho, e talvez um corpo
//!
//! ```text
//! [carimbo i64 ms][operacao u8][flags u8][res u16]
//! [rowid u64][versao u64][usuario u32]
//! [tam_imagem u32][crc32 u32][res u32]
//! [imagem ... tam_imagem bytes]
//! ```
//!
//! O carimbo e em milissegundos desde 1970-01-01T00:00:00Z, o que da
//! resolucao suficiente para ordenar operacoes dentro do mesmo segundo.
//!
//! # A imagem da linha, e por que ela e opcional
//!
//! Sem imagem o evento diz que o rowid 42 mudou; nao diz PARA QUE. Isso basta
//! para auditoria e nao basta para replicar -- uma replica precisa dos bytes.
//!
//! Com a imagem, um registro de 200 bytes gasta ~244 bytes de diario por
//! alteracao em vez de 36. E caro para quem so quer auditoria, e por isso o
//! interruptor esta no `config.json`: `replicacao.imagem_da_linha`.
//!
//! A imagem NAO e o texto do registro -- e o payload cru do `.reg`, os mesmos
//! bytes que a replica vai gravar, mais o CONTEUDO dos externos. Os ponteiros
//! do `.bin` e do `.memo` sao offsets locais e nao valem na outra maquina; e a
//! mesma razao de o `.trash` guardar conteudo e nao ponteiro.
//!
//! Exclusao nao leva imagem: o rowid basta.
//!
//! # O preco de o evento deixar de ter largura fixa
//!
//! Ate a versao 1 o evento N morava no offset `CAB_LEN + N x 36`, e pular era
//! uma conta. Agora nao e: para chegar ao evento N e preciso caminhar pelos
//! anteriores lendo o tamanho de cada um. O `qtd_eventos` de cada volume no
//! cabecalho e o que salva a leitura -- um volume inteiro se pula sem abrir.
//!
//! Como o `.log` cresce para sempre, ele tambem e paginado em
//! `Tabela_001.log`, `Tabela_002.log`, ... pelo tamanho de volume do esquema.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::RowId;

use crate::util::{agora, agora_ms, conferir_magic, por_i64, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_LOG: &[u8; 8] = b"PHXLOG\0\0";
pub const EXT_LOG: &str = "log";

const CAB_LEN: usize = 64;
/// Bytes do CABECALHO de cada evento. O corpo vem depois, se houver.
pub const EVENTO_CAB: usize = 44;
/// Teto da imagem de uma linha, para um tamanho corrompido nao pedir 4 GiB.
///
/// Uma linha com anexos grandes pode passar disto; ai o evento vai sem imagem
/// e a replica busca a linha pelo `ler`. Perder a replicacao de uma linha
/// gigante e melhor que abrir espaco para um `tam_imagem` inventado alocar a
/// memoria toda da maquina.
pub const IMAGEM_MAX: u32 = 64 * 1024 * 1024;
/// Bit 0 do byte de flags: este evento tem imagem.
const FLAG_IMAGEM: u8 = 1;
const VERSAO: u16 = 2;

/// O que aconteceu com o registro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operacao {
    Inclusao,
    Alteracao,
    Exclusao,
}

impl Operacao {
    fn tag(self) -> u8 {
        match self {
            Operacao::Inclusao => 1,
            Operacao::Alteracao => 2,
            Operacao::Exclusao => 3,
        }
    }

    fn de_tag(t: u8) -> Result<Operacao> {
        Ok(match t {
            1 => Operacao::Inclusao,
            2 => Operacao::Alteracao,
            3 => Operacao::Exclusao,
            outro => {
                return Err(PhxError::Corrompido(format!(
                    "operacao desconhecida no log: {outro}"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Operacao::Inclusao => "inclusao",
            Operacao::Alteracao => "alteracao",
            Operacao::Exclusao => "exclusao",
        }
    }
}

/// Um evento do diario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Evento {
    /// Milissegundos desde 1970-01-01T00:00:00Z.
    pub carimbo: i64,
    pub operacao: Operacao,
    pub rowid: RowId,
    /// Versao do registro depois da operacao.
    pub versao: u64,
    /// Identificacao de quem fez. Zero = nao informado.
    pub usuario: u32,
    /// Bytes da imagem que vem depois deste cabecalho. Zero = sem imagem.
    pub tam_imagem: u32,
}

impl Evento {
    /// Data e hora do evento em ISO (`AAAA-MM-DD HH:MM:SS,mmm`).
    pub fn instante_iso(&self) -> String {
        phxsql_core::datahora::instante_iso(self.carimbo)
    }

    /// O evento ocupa isto no arquivo, cabecalho mais corpo.
    pub fn ocupa(&self) -> u64 {
        EVENTO_CAB as u64 + self.tam_imagem as u64
    }

    /// O CRC cobre o cabecalho E a imagem.
    ///
    /// Cobrir so o cabecalho deixaria a imagem sem conferencia -- e a imagem e
    /// justamente o que a replica vai gravar como dado. Um byte trocado ali
    /// entraria na replica sem ninguem notar.
    fn escrever(&self, dst: &mut [u8; EVENTO_CAB], imagem: &[u8]) {
        dst.fill(0);
        por_i64(dst, 0, self.carimbo);
        dst[8] = self.operacao.tag();
        dst[9] = if imagem.is_empty() { 0 } else { FLAG_IMAGEM };
        por_u64(dst, 12, self.rowid);
        por_u64(dst, 20, self.versao);
        por_u32(dst, 28, self.usuario);
        por_u32(dst, 32, imagem.len() as u32);
        let mut crc = crc32(&dst[..36]);
        if !imagem.is_empty() {
            crc ^= crc32(imagem);
        }
        por_u32(dst, 36, crc);
    }

    /// Le o cabecalho. `imagem` e `None` quando quem chama ainda nao a leu --
    /// e ai o CRC so pode ser conferido depois, com [`Evento::conferir`].
    fn ler(src: &[u8]) -> Result<Evento> {
        if src.len() < EVENTO_CAB {
            return Err(PhxError::Corrompido("evento de log truncado".into()));
        }
        let c = Campos(src);
        let tam_imagem = c.u32(32);
        if tam_imagem > IMAGEM_MAX {
            return Err(PhxError::Corrompido(format!(
                "evento de log diz ter imagem de {tam_imagem} bytes, acima do teto"
            )));
        }
        let evento = Evento {
            carimbo: c.u64(0) as i64,
            operacao: Operacao::de_tag(src[8])?,
            rowid: c.u64(12),
            versao: c.u64(20),
            usuario: c.u32(28),
            tam_imagem,
        };
        if tam_imagem == 0 {
            evento.conferir(src, &[])?;
        }
        Ok(evento)
    }

    /// Confere o CRC do par cabecalho + imagem.
    fn conferir(&self, cab: &[u8], imagem: &[u8]) -> Result<()> {
        let mut crc = crc32(&cab[..36]);
        if !imagem.is_empty() {
            crc ^= crc32(imagem);
        }
        if crc != Campos(cab).u32(36) {
            return Err(PhxError::Corrompido(
                "evento de log com CRC invalido".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Cabecalho {
    volume: u32,
    fim: u64,
    qtd_eventos: u64,
}

pub struct LogFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// Usuario aplicado aos eventos gravados daqui em diante.
    pub usuario: u32,
}

impl LogFile {
    pub fn criar(diretorio: impl AsRef<Path>, nome: &str, paginacao: Paginacao) -> Result<LogFile> {
        let mut l = LogFile {
            volumes: Volumes::novo(diretorio, nome, EXT_LOG, paginacao),
            cabs: HashMap::new(),
            volume_atual: 1,
            usuario: 0,
        };
        l.volumes.criar(1)?;
        l.gravar_cab(Cabecalho {
            volume: 1,
            fim: CAB_LEN as u64,
            qtd_eventos: 0,
        })?;
        Ok(l)
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str, paginacao: Paginacao) -> Result<LogFile> {
        let volumes = Volumes::novo(diretorio, nome, EXT_LOG, paginacao);
        let existentes = volumes.existentes();
        if existentes.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhum volume de {}",
                volumes.caminho(1).display()
            )));
        }
        let volume_atual = *existentes.last().unwrap();
        let mut l = LogFile {
            volumes,
            cabs: HashMap::new(),
            volume_atual,
            usuario: 0,
        };
        l.cab(1)?;
        l.cab(volume_atual)?;
        // So o volume CORRENTE pode ter ficado atrasado: os anteriores foram
        // fechados quando a paginacao virou, e ali o cabecalho vai a disco na
        // hora.
        l.curar(volume_atual)?;
        Ok(l)
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let mut buf = [0u8; CAB_LEN];
        self.volumes.ler(volume, 0, &mut buf)?;
        let nome = self.volumes.caminho(volume).display().to_string();
        conferir_magic(&nome, MAGIC_LOG, &buf[0..8])?;
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
            qtd_eventos: c.u64(16),
        };
        self.cabs.insert(volume, cab);
        Ok(cab)
    }

    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        let mut buf = [0u8; CAB_LEN];
        buf[0..8].copy_from_slice(MAGIC_LOG);
        buf[8..10].copy_from_slice(&VERSAO.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, cab.volume);
        por_u64(&mut buf, 16, cab.qtd_eventos);
        por_u64(&mut buf, 24, cab.fim);
        por_i64(&mut buf, 32, agora());
        let crc = crc32(&buf[..56]);
        por_u32(&mut buf, 56, crc);
        self.volumes.escrever(cab.volume, 0, &buf)?;
        self.cabs.insert(cab.volume, cab);
        Ok(())
    }

    /// Registra um evento com o carimbo do relogio, sem imagem.
    pub fn registrar(&mut self, operacao: Operacao, rowid: RowId, versao: u64) -> Result<Evento> {
        self.registrar_com_imagem(operacao, rowid, versao, &[])
    }

    /// Registra um evento levando junto a imagem da linha.
    ///
    /// Imagem vazia grava o evento como sempre foi -- e e o que a exclusao
    /// manda, porque ali o rowid basta.
    pub fn registrar_com_imagem(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        versao: u64,
        imagem: &[u8],
    ) -> Result<Evento> {
        if imagem.len() as u64 > IMAGEM_MAX as u64 {
            return Err(PhxError::LimiteExcedido(format!(
                "imagem de {} bytes passa do teto de {IMAGEM_MAX} do diario",
                imagem.len()
            )));
        }
        let evento = Evento {
            carimbo: agora_ms(),
            operacao,
            rowid,
            versao,
            usuario: self.usuario,
            tam_imagem: imagem.len() as u32,
        };
        self.anexar(evento, imagem)?;
        Ok(evento)
    }

    fn anexar(&mut self, evento: Evento, imagem: &[u8]) -> Result<()> {
        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, evento.ocupa(), vazio);

        let cab = if virou {
            if paginacao.ligada() && volume > paginacao.max_arquivos {
                return Err(PhxError::LimiteExcedido(format!(
                    "diario de {} chegou ao teto de {} volumes",
                    self.volumes.nome(),
                    paginacao.max_arquivos
                )));
            }
            self.volumes.garantir(volume)?;
            let novo = Cabecalho {
                volume,
                fim: CAB_LEN as u64,
                qtd_eventos: 0,
            };
            self.gravar_cab(novo)?;
            self.volume_atual = volume;
            novo
        } else {
            atual
        };

        let mut buf = [0u8; EVENTO_CAB];
        evento.escrever(&mut buf, imagem);
        self.volumes.escrever(volume, cab.fim, &buf)?;
        if !imagem.is_empty() {
            self.volumes
                .escrever(volume, cab.fim + EVENTO_CAB as u64, imagem)?;
        }
        // O CABECALHO NAO VAI A DISCO AQUI, e essa e a diferenca que faz o
        // diario nao atrasar o `.reg`.
        //
        // O evento ja foi gravado -- ele e o que nao pode faltar. O cabecalho
        // e um CONTADOR: `fim`, onde o proximo entra, e `qtd_eventos`. Grava-lo
        // a cada evento era uma segunda chamada de escrita por linha inserida,
        // medida em 0,41 us, para levar a disco um numero que a leitura sabe
        // recalcular varrendo os proprios eventos.
        //
        // Ele passa a ir no `sincronizar`, junto com o resto. Se o processo
        // cair antes disso, o cabecalho fica ATRASADO em relacao aos eventos
        // que ja estao no arquivo -- e `abrir` cura isso varrendo para a
        // frente a partir do `fim` gravado, validando cada evento pelo CRC que
        // ele ja carrega. A varredura e limitada ao que entrou desde o ultimo
        // `sincronizar`, que e uma janela de centenas de eventos.
        //
        // O que NAO se faz aqui, de proposito: segurar o EVENTO em memoria.
        // Indice perdido se reconstroi do `.reg`; evento perdido nao se
        // reconstroi -- ele e a historia, e e a posicao de que a replicacao
        // depende.
        self.cabs.insert(
            volume,
            Cabecalho {
                volume,
                fim: cab.fim + evento.ocupa(),
                qtd_eventos: cab.qtd_eventos + 1,
            },
        );
        Ok(())
    }

    /// Varre para a frente a partir do `fim` gravado e conserta o cabecalho.
    ///
    /// Existe porque o cabecalho passou a ir a disco so no `sincronizar`: uma
    /// queda antes dele deixa eventos no arquivo que o cabecalho nao conta. Sem
    /// esta cura, a proxima gravacao ESCREVERIA POR CIMA deles.
    ///
    /// Cada evento carrega o proprio CRC, entao a varredura sabe onde parar: no
    /// primeiro que nao confere, ou no fim do arquivo. Regiao zerada nao passa
    /// -- o CRC-32 de 36 bytes zerados nao e zero.
    fn curar(&mut self, volume: u32) -> Result<u64> {
        let mut cab = self.cab(volume)?;
        let tamanho = self.volumes.tamanho(volume)?;
        let mut achados = 0u64;

        while cab.fim + EVENTO_CAB as u64 <= tamanho {
            let mut buf = [0u8; EVENTO_CAB];
            self.volumes.ler(volume, cab.fim, &mut buf)?;
            let evento = match Evento::ler(&buf) {
                Ok(e) => e,
                Err(_) => break,
            };
            if cab.fim + evento.ocupa() > tamanho {
                break;
            }
            if evento.tam_imagem > 0 {
                let mut imagem = vec![0u8; evento.tam_imagem as usize];
                self.volumes
                    .ler(volume, cab.fim + EVENTO_CAB as u64, &mut imagem)?;
                if evento.conferir(&buf, &imagem).is_err() {
                    break;
                }
            }
            cab = Cabecalho {
                volume,
                fim: cab.fim + evento.ocupa(),
                qtd_eventos: cab.qtd_eventos + 1,
            };
            achados += 1;
        }

        if achados > 0 {
            self.gravar_cab(cab)?;
        }
        Ok(achados)
    }

    /// Total de eventos em todos os volumes.
    pub fn total(&mut self) -> Result<u64> {
        let mut t = 0;
        for v in self.volumes.existentes() {
            t += self.cab(v)?.qtd_eventos;
        }
        Ok(t)
    }

    /// Le os eventos em ordem cronologica, do mais antigo para o mais recente.
    ///
    /// `pular` descarta os N primeiros; `limite` zero devolve todos.
    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        Ok(self
            .percorrer(pular, limite, false)?
            .into_iter()
            .map(|(e, _)| e)
            .collect())
    }

    /// O mesmo que [`LogFile::ler`], trazendo a imagem de cada evento.
    ///
    /// E o que a replicacao usa. Eventos gravados sem imagem voltam com o
    /// vetor vazio -- e ai a replica sabe que aquele evento nao da para
    /// aplicar, em vez de aplicar bytes que nao existem.
    pub fn ler_com_imagem(&mut self, pular: u64, limite: u64) -> Result<Vec<(Evento, Vec<u8>)>> {
        self.percorrer(pular, limite, true)
    }

    /// A varredura unica dos dois caminhos.
    ///
    /// Desde que o evento deixou de ter largura fixa, chegar ao evento N e
    /// caminhar pelos anteriores. O que ainda se pula de graca e o VOLUME
    /// inteiro: o `qtd_eventos` do cabecalho diz quantos ele tem, e se todos
    /// eles estao antes do `pular` o arquivo nem se abre.
    fn percorrer(
        &mut self,
        pular: u64,
        limite: u64,
        com_imagem: bool,
    ) -> Result<Vec<(Evento, Vec<u8>)>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            if vistos + cab.qtd_eventos <= pular {
                vistos += cab.qtd_eventos;
                continue;
            }
            let mut offset = CAB_LEN as u64;
            while offset + EVENTO_CAB as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_CAB];
                self.volumes.ler(volume, offset, &mut buf)?;
                let evento = Evento::ler(&buf)?;
                if vistos >= pular {
                    let mut imagem = Vec::new();
                    if evento.tam_imagem > 0 {
                        imagem = vec![0u8; evento.tam_imagem as usize];
                        self.volumes
                            .ler(volume, offset + EVENTO_CAB as u64, &mut imagem)?;
                        evento.conferir(&buf, &imagem)?;
                        if !com_imagem {
                            imagem.clear();
                        }
                    }
                    saida.push((evento, imagem));
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += evento.ocupa();
            }
        }
        Ok(saida)
    }

    /// Eventos de um registro especifico, em ordem cronologica.
    pub fn historico(&mut self, rowid: RowId) -> Result<Vec<Evento>> {
        Ok(self
            .ler(0, 0)?
            .into_iter()
            .filter(|e| e.rowid == rowid)
            .collect())
    }

    /// Confere o CRC de todos os eventos e a contagem dos cabecalhos.
    pub fn verificar(&mut self) -> Result<u64> {
        let mut total = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            let mut offset = CAB_LEN as u64;
            let mut no_volume = 0u64;
            while offset + EVENTO_CAB as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_CAB];
                self.volumes.ler(volume, offset, &mut buf)?;
                let evento = Evento::ler(&buf)?; // confere a operacao, e o CRC se nao ha imagem
                if evento.tam_imagem > 0 {
                    // Com imagem o CRC so fecha depois de le-la. Conferir so o
                    // cabecalho aqui deixaria de fora justamente os bytes que
                    // a replica grava como dado.
                    let mut imagem = vec![0u8; evento.tam_imagem as usize];
                    self.volumes
                        .ler(volume, offset + EVENTO_CAB as u64, &mut imagem)?;
                    evento.conferir(&buf, &imagem)?;
                }
                no_volume += 1;
                offset += evento.ocupa();
            }
            if no_volume != cab.qtd_eventos {
                return Err(PhxError::Corrompido(format!(
                    "{}: cabecalho diz {} eventos, varredura achou {no_volume}",
                    self.volumes.caminho(volume).display(),
                    cab.qtd_eventos
                )));
            }
            total += no_volume;
        }
        Ok(total)
    }

    pub fn caminho(&self, volume: u32) -> PathBuf {
        self.volumes.caminho(volume)
    }

    pub fn volumes(&self) -> Vec<u32> {
        self.volumes.existentes()
    }

    /// Leva os cabecalhos a disco e sincroniza.
    ///
    /// A ordem importa: o cabecalho vai ANTES do `fsync`, senao ele ficaria
    /// para a proxima janela e a cura teria de varrer duas.
    pub fn sincronizar(&mut self) -> Result<()> {
        let pendentes: Vec<Cabecalho> = self.cabs.values().copied().collect();
        for cab in pendentes {
            self.gravar_cab(cab)?;
        }
        self.volumes.sincronizar()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-log-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /* --------------------------------------- o cabecalho preguicoso e a cura

    O cabecalho do `.log` deixou de ir a disco a cada evento, para o diario
    nao atrasar o `.reg`. O EVENTO continua indo na hora -- e a diferenca
    entre as duas coisas e o que estes testes protegem.

    Uma queda antes do `sincronizar` deixa o cabecalho atrasado. Sem a cura,
    a proxima gravacao escreveria POR CIMA dos eventos que ja estavam la:
    nao seria evento invisivel, seria evento destruido. */

    /// O caso da queda: grava, some sem sincronizar, reabre. Nada pode faltar.
    #[test]
    fn queda_sem_sincronizar_nao_perde_evento() {
        let d = dir_temp("cura-queda");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=500u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
            // De proposito SEM `sincronizar`: e o que uma queda do processo faz.
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 500, "a cura perdeu evento");
        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.len(), 500);
        assert_eq!(eventos[0].rowid, 1);
        assert_eq!(eventos[499].rowid, 500);
        assert_eq!(l.verificar().unwrap(), 500);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// A cura tambem vale com imagem da linha, que e o modo da replicacao --
    /// ali o evento tem tamanho variavel, e a varredura precisa andar por
    /// `ocupa()` e nao por um passo fixo.
    #[test]
    fn a_cura_anda_por_evento_de_tamanho_variavel() {
        let d = dir_temp("cura-imagem");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=200u64 {
                // Imagens de tamanhos diferentes: passo fixo erraria na segunda.
                let imagem = vec![(i % 251) as u8; (i % 97) as usize];
                l.registrar_com_imagem(Operacao::Inclusao, i, 1, &imagem)
                    .unwrap();
            }
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 200);
        let com = l.ler_com_imagem(0, 0).unwrap();
        assert_eq!(com.len(), 200);
        assert_eq!(com[199].1.len(), 200 % 97);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O que a cura existe para impedir: gravar por cima. Depois de reabrir,
    /// o evento novo tem de entrar DEPOIS dos que ja estavam.
    #[test]
    fn depois_da_cura_o_novo_evento_nao_sobrescreve() {
        let d = dir_temp("cura-sobrescreve");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=50u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 51, 1).unwrap();
        l.sincronizar().unwrap();

        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.len(), 51, "o evento novo comeu os antigos");
        assert_eq!(eventos[49].rowid, 50);
        assert_eq!(eventos[50].rowid, 51);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Sincronizado, nao ha o que curar -- e reabrir tem de dar o mesmo.
    #[test]
    fn com_sincronizar_a_cura_nao_muda_nada() {
        let d = dir_temp("cura-nada");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=30u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
            l.sincronizar().unwrap();
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 30);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn registra_as_tres_operacoes_em_ordem() {
        let d = dir_temp("tres");
        let mut l = LogFile::criar(&d, "cadastroClientes", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        l.registrar(Operacao::Alteracao, 1, 2).unwrap();
        l.registrar(Operacao::Exclusao, 1, 2).unwrap();

        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.len(), 3);
        assert_eq!(eventos[0].operacao, Operacao::Inclusao);
        assert_eq!(eventos[1].operacao, Operacao::Alteracao);
        assert_eq!(eventos[2].operacao, Operacao::Exclusao);
        assert_eq!(eventos[1].versao, 2);
        assert_eq!(l.total().unwrap(), 3);
        assert_eq!(l.verificar().unwrap(), 3);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn carimbo_tem_data_e_hora() {
        let d = dir_temp("carimbo");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        let e = l.registrar(Operacao::Inclusao, 7, 1).unwrap();
        assert!(e.carimbo > 1_700_000_000_000, "carimbo em ms recente");
        let iso = e.instante_iso();
        // AAAA-MM-DD HH:MM:SS,mmm
        assert_eq!(iso.len(), 23, "formato inesperado: {iso}");
        assert_eq!(&iso[4..5], "-");
        assert_eq!(&iso[10..11], " ");
        assert_eq!(&iso[19..20], ",");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn historico_de_um_registro() {
        let d = dir_temp("hist");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        l.registrar(Operacao::Inclusao, 2, 1).unwrap();
        l.registrar(Operacao::Alteracao, 1, 2).unwrap();
        l.registrar(Operacao::Exclusao, 2, 1).unwrap();

        let h = l.historico(1).unwrap();
        assert_eq!(h.len(), 2);
        assert!(h.iter().all(|e| e.rowid == 1));
        assert_eq!(h[0].operacao, Operacao::Inclusao);
        assert_eq!(h[1].operacao, Operacao::Alteracao);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn usuario_e_gravado_quando_informado() {
        let d = dir_temp("usuario");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.usuario = 42;
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        assert_eq!(l.ler(0, 0).unwrap()[0].usuario, 42);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn reabre_e_continua_o_diario() {
        let d = dir_temp("reabre");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            for i in 1..=10u64 {
                l.registrar(Operacao::Inclusao, i, 1).unwrap();
            }
            l.sincronizar().unwrap();
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.total().unwrap(), 10);
        l.registrar(Operacao::Exclusao, 5, 1).unwrap();
        assert_eq!(l.total().unwrap(), 11);
        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.last().unwrap().operacao, Operacao::Exclusao);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn pular_e_limitar() {
        let d = dir_temp("pagina");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 1..=100u64 {
            l.registrar(Operacao::Inclusao, i, 1).unwrap();
        }
        let p = l.ler(10, 5).unwrap();
        assert_eq!(p.len(), 5);
        assert_eq!(p[0].rowid, 11);
        assert_eq!(p[4].rowid, 15);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn diario_tambem_pagina() {
        let d = dir_temp("pag");
        // Volumes de 200 bytes: cabecalho 64 + 3 eventos de 36 = 172.
        let pag = Paginacao::nova(10, 99)
            .unwrap()
            .com_bytes_por_arquivo(200)
            .unwrap();
        let mut l = LogFile::criar(&d, "t", pag).unwrap();
        for i in 1..=20u64 {
            l.registrar(Operacao::Inclusao, i, 1).unwrap();
        }
        assert!(l.volumes().len() > 1, "deveria ter passado de volume");
        assert_eq!(l.total().unwrap(), 20);
        // A leitura atravessa os volumes na ordem cronologica.
        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos.len(), 20);
        for (i, e) in eventos.iter().enumerate() {
            assert_eq!(e.rowid, i as u64 + 1);
        }
        assert_eq!(l.verificar().unwrap(), 20);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn evento_adulterado_falha_no_crc() {
        let d = dir_temp("crc");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            l.registrar(Operacao::Inclusao, 1, 1).unwrap();
            l.sincronizar().unwrap();
        }
        {
            let mut v = Volumes::novo(&d, "t", EXT_LOG, Paginacao::DESLIGADA);
            v.escrever(1, CAB_LEN as u64 + 12, &[9u8; 8]).unwrap();
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert!(l.verificar().is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
