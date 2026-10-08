//! `.log` -- o diario da tabela.
//!
//! Toda inclusao, alteracao e exclusao e registrada com data e hora. O arquivo
//! e append-only e sem indice: e um diario, nao uma tabela.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log = cadastroClientes
//! ```
//!
//! # Evento: 52 bytes de cabecalho (44 ate a versao 3), e talvez um corpo
//!
//! ```text
//! [carimbo i64 ms][operacao u8][flags u8][origem u16]
//! [rowid u64][versao u64][usuario u32]
//! [tam_imagem u32][crc32 u32][tempero u32]
//! [tx u64]                                  <- so na versao 4 (pedido 676)
//! [imagem ... tam_imagem bytes]
//! ```
//!
//! # O id de transacao (versao 4, pedido 676)
//!
//! Todo evento gravado numa mesma tomada da trava de escrita do servidor leva
//! o MESMO `tx` -- e um commit entre tabelas e uma tomada so. E com ele que a
//! replica junta os eventos de varias tabelas e aplica a transacao da origem
//! inteira, ou nada (`docs/FORMATO.md` §4, `docs/REPLICACAO.md`). O numero e
//! do PROCESSO, estritamente crescente: `max(ultimo + 1, relogio_ms << 16)`.
//! O relogio entra para que um processo que reinicia nao emita numero menor
//! que o de antes sem precisar persistir contador nenhum -- o mesmo motivo
//! do nonce sair do offset, e nao de um contador gravado a cada evento.
//!
//! A versao e por VOLUME, como a da cifra: um volume gravado na 2 ou na 3
//! continua com eventos de 44 bytes e `tx` zero ate a paginacao virar, porque
//! um arquivo append-only nao se reescreve. Zero quer dizer «sem id», e a
//! replica aplica esse evento sozinho, como sempre aplicou.
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
//! Ate a versao 1 o evento N morava no offset `cabecalho + N x 36`, e pular era
//! uma conta. Agora nao e: para chegar ao evento N e preciso caminhar pelos
//! anteriores lendo o tamanho de cada um. O `qtd_eventos` de cada volume no
//! cabecalho e o que salva a leitura -- um volume inteiro se pula sem abrir.
//!
//! Como o `.log` cresce para sempre, ele tambem e paginado em
//! `Tabela#001.log`, `Tabela#002.log`, ... pelo tamanho de volume do esquema.
//!
//! # A cifra do corpo (versao 3)
//!
//! Quando o `config.json` liga a cifra, um volume NOVO nasce na versao 3: o
//! cabecalho do arquivo cresce para 128 bytes e leva sal, iteracoes e a prova
//! da chave; o corpo de cada evento vai cifrado com ChaCha20-Poly1305 e ganha
//! 16 bytes de etiqueta.
//!
//! **O cabecalho do evento continua em claro**, e isso e escolha, nao
//! esquecimento: e o `tam_imagem` dele que diz onde comeca o proximo evento.
//! Cifra-lo faria a cura, o `verificar` e a contagem pararem de funcionar para
//! quem so tem o arquivo. Em troca ele entra como DADO ASSOCIADO da etiqueta,
//! entao trocar o rowid de um evento, ou mover o corpo de um para outro,
//! derruba a autenticacao. O que o cabecalho em claro custa e METADADO: quem
//! le sem a chave sabe QUE o rowid 42 mudou as 14h03, e nao sabe para que.
//!
//! Ver `crate::cofre` e `docs/SEGURANCA.md` §8.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::RowId;

use crate::cofre::{self, Cabecalho};
use crate::util::{agora_ms, por_i64, por_u16, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_LOG: &[u8; 8] = b"PHXLOG\0\0";
pub const EXT_LOG: &str = "log";

/// Bytes do CABECALHO de cada evento nos volumes das versoes 2 e 3. O corpo
/// vem depois, se houver.
pub const EVENTO_CAB: usize = 44;
/// Bytes do cabecalho do evento nos volumes da versao 4: os 44 de sempre e o
/// id de transacao (pedido 676).
pub const EVENTO_CAB_TX: usize = 52;
/// Onde o id de transacao mora no evento da versao 4.
const OFF_TX: usize = 44;
/// Onde ficam os quatro bytes de tempero do nonce. Zerados no volume em claro.
const OFF_TEMPERO: usize = 40;
/// Teto da imagem de uma linha, para um tamanho corrompido nao pedir 4 GiB.
///
/// Uma linha com anexos grandes pode passar disto; ai o evento vai sem imagem
/// e a replica busca a linha pelo `ler`. Perder a replicacao de uma linha
/// gigante e melhor que abrir espaco para um `tam_imagem` inventado alocar a
/// memoria toda da maquina.
pub const IMAGEM_MAX: u32 = 64 * 1024 * 1024;
/// Bit 0 do byte de flags: este evento tem imagem.
const FLAG_IMAGEM: u8 = 1;

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
    /// De que servidor a escrita NASCEU. Zero = escrita local.
    ///
    /// E o que mata o laco infinito do bidirecional: ao servir o fluxo para
    /// outro servidor, os eventos cuja origem e o proprio destino nao viajam
    /// de volta. Guardado nos 2 bytes que eram reservados no cabecalho -- todo
    /// evento gravado antes deste campo le zero, que e exatamente "local".
    pub origem: u16,
    /// Bytes que vem depois deste cabecalho, NO ARQUIVO. Zero = sem imagem.
    ///
    /// Num volume cifrado isto e a imagem cifrada MAIS os 16 bytes da
    /// etiqueta, e nao o tamanho do texto claro. E de proposito: quem caminha
    /// pelo arquivo precisa saber onde o proximo evento comeca sem ter a
    /// chave, e a imagem que a leitura devolve ja vem decifrada.
    pub tam_imagem: u32,
    /// O id da transacao que gravou este evento -- pedido 676. Zero = sem
    /// id: o evento mora num volume das versoes 2 ou 3, que nao tem onde
    /// guarda-lo.
    pub tx: u64,
    /// O volume onde ele mora e da versao 4 (cabecalho de 52 bytes). Diz
    /// quanto o evento ocupa; quem le de fora nao precisa dele.
    largo: bool,
}

/// A largura do cabecalho do evento num volume.
fn largura(cab: &Cabecalho) -> usize {
    if cab.com_tx {
        EVENTO_CAB_TX
    } else {
        EVENTO_CAB
    }
}

/// O CRC do cabecalho do evento: os bytes 0..36 de sempre e, na versao 4, o
/// id de transacao tambem -- um `tx` trocado no disco juntaria eventos de
/// transacoes diferentes na replica, e ninguem perceberia.
///
/// O tempero (40..44) continua fora, como sempre esteve: no volume em claro
/// ele e zero, e no cifrado ele ja entra no dado associado da etiqueta.
fn crc_do_cabecalho(cab: &[u8]) -> u32 {
    if cab.len() >= EVENTO_CAB_TX {
        let mut b = [0u8; 44];
        b[..36].copy_from_slice(&cab[..36]);
        b[36..44].copy_from_slice(&cab[OFF_TX..OFF_TX + 8]);
        crc32(&b)
    } else {
        crc32(&cab[..36])
    }
}

impl Evento {
    /// Data e hora do evento em ISO (`AAAA-MM-DD HH:MM:SS,mmm`).
    pub fn instante_iso(&self) -> String {
        phxsql_core::datahora::instante_iso(self.carimbo)
    }

    /// O evento ocupa isto no arquivo, cabecalho mais corpo.
    pub fn ocupa(&self) -> u64 {
        self.largura() as u64 + self.tam_imagem as u64
    }

    fn largura(&self) -> usize {
        if self.largo {
            EVENTO_CAB_TX
        } else {
            EVENTO_CAB
        }
    }

    /// O CRC cobre o cabecalho E a imagem.
    ///
    /// Cobrir so o cabecalho deixaria a imagem sem conferencia -- e a imagem e
    /// justamente o que a replica vai gravar como dado. Um byte trocado ali
    /// entraria na replica sem ninguem notar.
    fn escrever(&self, dst: &mut [u8], tempero: [u8; 4]) {
        debug_assert_eq!(dst.len(), self.largura());
        dst.fill(0);
        por_i64(dst, 0, self.carimbo);
        dst[8] = self.operacao.tag();
        dst[9] = if self.tam_imagem == 0 { 0 } else { FLAG_IMAGEM };
        por_u16(dst, 10, self.origem);
        por_u64(dst, 12, self.rowid);
        por_u64(dst, 20, self.versao);
        por_u32(dst, 28, self.usuario);
        por_u32(dst, 32, self.tam_imagem);
        dst[OFF_TEMPERO..OFF_TEMPERO + 4].copy_from_slice(&tempero);
        if self.largo {
            por_u64(dst, OFF_TX, self.tx);
        }
    }

    /// O dado associado da etiqueta: o cabecalho inteiro, menos o CRC.
    ///
    /// O CRC fica de fora porque ele depende do corpo, e o corpo depende da
    /// etiqueta, que depende do dado associado -- incluir os quatro bytes
    /// fecharia um circulo que nao se resolve.
    ///
    /// Na versao 4 o id de transacao entra junto: e cabecalho, e trocar o
    /// `tx` de um evento cifrado derruba a etiqueta como trocar o rowid.
    fn associado(cab: &[u8]) -> Vec<u8> {
        let mut aad = cab.to_vec();
        aad[36..40].fill(0);
        aad
    }

    fn tempero(cab: &[u8]) -> [u8; 4] {
        let mut t = [0u8; 4];
        t.copy_from_slice(&cab[OFF_TEMPERO..OFF_TEMPERO + 4]);
        t
    }

    /// Fecha o CRC, que so pode ser calculado com o corpo ja pronto.
    fn conferir_e_fechar(&self, dst: &mut [u8], corpo: &[u8]) {
        let mut crc = crc_do_cabecalho(dst);
        if !corpo.is_empty() {
            crc ^= crc32(corpo);
        }
        por_u32(dst, 36, crc);
    }

    /// Le o cabecalho. `imagem` e `None` quando quem chama ainda nao a leu --
    /// e ai o CRC so pode ser conferido depois, com [`Evento::conferir`].
    ///
    /// `largo` diz se o volume e da versao 4 -- e o cabecalho do VOLUME que
    /// sabe, nao o evento.
    fn ler(src: &[u8], largo: bool) -> Result<Evento> {
        let largura = if largo { EVENTO_CAB_TX } else { EVENTO_CAB };
        if src.len() < largura {
            return Err(PhxError::Corrompido("evento de log truncado".into()));
        }
        let src = &src[..largura];
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
            origem: c.u16(10),
            tam_imagem,
            tx: if largo { c.u64(OFF_TX) } else { 0 },
            largo,
        };
        if tam_imagem == 0 {
            evento.conferir(src, &[])?;
        }
        Ok(evento)
    }

    /// Confere o CRC do par cabecalho + corpo COMO ELE ESTA NO ARQUIVO.
    ///
    /// A formula e a mesma da versao 2, byte por byte -- e por isso um `.log`
    /// gravado antes da cifra continua conferindo. Num volume cifrado ela cobre
    /// o corpo cifrado, que e o que esta no disco: e o que deixa a cura e o
    /// `verificar` andarem pelo arquivo inteiro SEM a chave.
    fn conferir(&self, cab: &[u8], imagem: &[u8]) -> Result<()> {
        let mut crc = crc_do_cabecalho(&cab[..self.largura()]);
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

/// Quatro bytes so deste evento, para o nonce.
///
/// # Por que eles existem
///
/// O numero de ordem do nonce e o OFFSET do evento no volume, e num arquivo
/// que so cresce dois eventos nunca comecam no mesmo lugar. Ha UMA excecao: uma
/// queda no meio da escrita deixa um rabo estragado, a cura corta esse rabo, e
/// o proximo evento entra no offset que o estragado ocupava. Esses quatro bytes
/// sorteados sao o que impede o par (chave, nonce) de se repetir ali.
///
/// Num volume em claro eles ficam zerados: nao ha nonce, e sortear bytes que
/// ninguem le mudaria o arquivo sem mudar nada.
fn tempero_novo(cab: &Cabecalho) -> [u8; 4] {
    if !cab.cifrado() {
        return [0u8; 4];
    }
    let mut t = [0u8; 4];
    t.copy_from_slice(&phxsql_core::senha::bytes_aleatorios(4));
    t
}

/// Onde um evento comeca no arquivo.
///
/// # Por que ela existe
///
/// Desde que o evento deixou de ter largura fixa, chegar ao evento N e caminhar
/// pelos N-1 anteriores lendo o cabecalho de cada um. Para quem le UMA vez isso
/// e o preco justo. Para quem le em lotes seguidos -- que e exatamente o que a
/// replicacao faz, «me de 500 a partir de P», com P andando de 500 em 500 --
/// custa N^2/2 leituras de cabecalho no total.
///
/// Medido em `--example custo-do-desde`, num diario de 100.000: ler 500 a
/// partir de 0 custa 1,11 us por evento, e a partir de 90.000 custa **72,65**.
/// Alcancar os 100.000 de 500 em 500 gastava **4,07 s so do lado de quem
/// serve** -- e era isso, e nao o que a replica aplica, que fazia a replicacao
/// parecer lenta.
///
/// A marca e uma **dica**, e nao uma verdade -- mas so enquanto ela e do
/// MESMO diario. Pedido 620: a tabela apagada e recriada (ou restaurada) tem
/// OUTRO `.log` no mesmo caminho, e o `offset` da marca velha cai no meio de
/// um evento da vida nova (erro de CRC) ou depois do `fim` dela (a varredura
/// devolve vazio e PULA os eventos novos -- e pular e dado errado, nao dica
/// ruim). Por isso a marca leva a [`AncoraDaMarca`]: o evento logo antes dela,
/// conferido byte a byte antes de a marca ser usada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcaDoDiario {
    /// Numero do evento que comeca aqui, contando de zero.
    pub evento: u64,
    pub volume: u32,
    pub offset: u64,
    /// O evento que a varredura leu logo antes desta posicao.
    pub ancora: AncoraDaMarca,
}

/// O ultimo evento lido antes de uma [`MarcaDoDiario`], para provar que o
/// diario em que ela vai ser usada e o mesmo em que ela nasceu -- pedido 620.
///
/// # Por que o cabecalho do evento, e nao um numero do arquivo
///
/// O cabecalho do `.log` nao tem identidade nenhuma (o sal so existe cifrado),
/// e inventar uma seria mudar o formato para responder uma pergunta que o
/// proprio diario ja responde: o cabecalho de um evento leva o carimbo em
/// milissegundos, o rowid, a versao, o tempero e o CRC da imagem. Outra vida
/// da tabela no mesmo `offset` com o mesmo cabecalho nao acontece -- e a
/// restauracao de um backup da MESMA vida, cujo diario e um prefixo
/// byte a byte deste, passa na conferencia e esta certa em passar: ate ali o
/// que a marca resume continua verdade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AncoraDaMarca {
    pub volume: u32,
    pub offset: u64,
    /// CRC-32 do cabecalho do evento: 44 bytes nas versoes 2 e 3, 52 na 4.
    pub selo: u32,
}

/// O evento que o `.log` ficou devendo a uma linha que ja esta no `.reg` --
/// pedido 498.
///
/// # O que ela guarda, e o que ela NAO guarda
///
/// Guarda o que so existia no instante da escrita: QUAL linha, QUE operacao,
/// quem, quando e de onde (o carimbo e a origem forcados do bidirecional
/// decidem conflito, e completar com o relogio da abertura elegeria o evento
/// errado), e se o evento levava imagem -- o interruptor e do servidor, e a
/// abertura da tabela ainda nao o conhece. NAO guarda a versao nem a imagem:
/// as duas se derivam da linha como ela esta no `.reg`, que nao mudou mais
/// desde a falha, porque a tabela recusa toda escrita enquanto deve.
///
/// # O formato, 16 bytes no cabecalho do volume 1
///
/// ```text
/// [operacao u8, bit 7 = com imagem][res u8][origem u16][usuario u32][rowid u64]
/// ```
///
/// e o carimbo no `alterado em` (32..40). Ver [`cofre::Marca`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventoDevido {
    pub operacao: Operacao,
    pub rowid: RowId,
    pub carimbo: i64,
    pub origem: u16,
    pub usuario: u32,
    pub com_imagem: bool,
}

/// Bit 7 do primeiro byte da marca: o evento devido levava imagem.
const MARCA_COM_IMAGEM: u8 = 0x80;

impl EventoDevido {
    fn marca(&self) -> cofre::Marca {
        let mut bytes = [0u8; cofre::MARCA_LEN];
        bytes[0] = self.operacao.tag() | if self.com_imagem { MARCA_COM_IMAGEM } else { 0 };
        por_u16(&mut bytes, 2, self.origem);
        por_u32(&mut bytes, 4, self.usuario);
        por_u64(&mut bytes, 8, self.rowid);
        cofre::Marca {
            carimbo: self.carimbo,
            bytes,
        }
    }

    fn da_marca(m: &cofre::Marca, nome: &str) -> Result<EventoDevido> {
        let c = Campos(&m.bytes);
        let operacao = Operacao::de_tag(m.bytes[0] & !MARCA_COM_IMAGEM).map_err(|_| {
            PhxError::Corrompido(format!(
                "{nome}: a marca de evento devido traz a operacao {}, que nao existe",
                m.bytes[0]
            ))
        })?;
        Ok(EventoDevido {
            operacao,
            rowid: c.u64(8),
            carimbo: m.carimbo,
            origem: c.u16(2),
            usuario: c.u32(4),
            com_imagem: m.bytes[0] & MARCA_COM_IMAGEM != 0,
        })
    }
}

/// O teto da imagem, contado no que vai AO ARQUIVO: num volume cifrado a
/// etiqueta de 16 bytes anda junto, e deixar a soma passar do teto faria a
/// leitura recusar o proprio evento que acabamos de gravar. Uma conta so, para
/// o [`LogFile::conferir_teto`] de antes da linha e o registro de depois.
fn conferir_imagem(tam_imagem: usize) -> Result<()> {
    if tam_imagem as u64 + cofre::ACRESCIMO as u64 > IMAGEM_MAX as u64 {
        return Err(PhxError::LimiteExcedido(format!(
            "imagem de {tam_imagem} bytes passa do teto de {IMAGEM_MAX} do diario"
        )));
    }
    Ok(())
}

/// O ultimo volume, quando ele nasceu sem cabecalho -- pedido 498.
///
/// Virar de volume e criar o arquivo e depois gravar o cabecalho; no disco
/// cheio o arquivo nasce e o cabecalho nao cabe. Ele nunca teve evento (o
/// evento vem depois do cabecalho), e sem esta saida o `.log` inteiro deixava
/// de abrir -- a tabela junto. So o ultimo, so acima do 1, e so com menos
/// bytes que o menor cabecalho: um volume com cabecalho legivel nao cai aqui.
fn sem_cabecalho(volumes: &mut Volumes, existentes: &[u32]) -> Result<Option<u32>> {
    let ultimo = *existentes.last().unwrap_or(&1);
    if ultimo > 1 && volumes.tamanho(ultimo)? < cofre::CAB_V2 as u64 {
        return Ok(Some(ultimo));
    }
    Ok(None)
}

/// Uma tabela cujo diario cresceu durante a tomada corrente da trava -- pedido
/// 207, a escrita com quorum.
///
/// `antes` e o total de eventos ANTES do primeiro evento desta tomada, e
/// `depois` o total depois do ultimo: o quorum manda `[antes, depois)` as
/// replicas e espera cada uma confirmar `depois`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tocada {
    pub diretorio: PathBuf,
    pub nome: String,
    pub antes: u64,
    pub depois: u64,
}

thread_local! {
    /// A anotacao esta ligada NESTA thread? Por thread porque quem liga e a
    /// tomada da trava de dados, e a tomada e de uma thread so: a lista que
    /// ela drena no fim tem de ser a das escritas DELA.
    static ANOTANDO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TOCADAS: std::cell::RefCell<Vec<Tocada>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Liga a anotacao das tabelas tocadas nesta thread, comecando de lista vazia.
///
/// Quem liga e a tomada da trava de dados do servidor, e so com o quorum de
/// escrita ligado: desligada, o custo no laco quente e a leitura de uma
/// `Cell` de thread por evento -- o portao vem antes do trabalho.
pub fn anotar_tocadas() {
    TOCADAS.with(|t| t.borrow_mut().clear());
    ANOTANDO.with(|a| a.set(true));
}

/// Desliga a anotacao e devolve o que ela juntou.
pub fn tomar_tocadas() -> Vec<Tocada> {
    ANOTANDO.with(|a| a.set(false));
    TOCADAS.with(|t| std::mem::take(&mut *t.borrow_mut()))
}

/// A anotacao esta ligada nesta thread? So para a prova de que ela desliga.
pub fn anotando_tocadas() -> bool {
    ANOTANDO.with(std::cell::Cell::get)
}

// ------------------------------------------------- o id de transacao (676)

/// O ultimo id de transacao emitido por este processo. 0 = nenhum ainda.
static ULTIMO_TX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// A unidade de transacao aberta NESTA thread: `None` fora de uma, e
    /// `Some(0)` aberta e ainda sem evento -- o id so se tira no primeiro
    /// evento, para que tomada que so le nao gaste numero.
    static UNIDADE: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
    /// O que a unidade aberta ja gravou: `(evento com id, evento sem id)`.
    /// Os dois juntos = o commit misto do pedido 684 (b).
    static MISTURA: std::cell::Cell<(bool, bool)> = const { std::cell::Cell::new((false, false)) };
}

/// O proximo id de transacao: estritamente maior que todos os anteriores
/// deste processo -- e, desde o pedido 684, que o ultimo de cada diario que
/// este processo ja abriu --, e maior que o relogio em milissegundos
/// deslocado 16 bits.
///
/// # Por que o relogio, e nao um contador gravado
///
/// A replica junta as tabelas pela ORDEM dos ids (`replica::Juntador`), entao
/// um processo que reinicia nao pode recomecar do zero. Gravar o contador
/// seria uma escrita a mais por commit -- a mesma que o `.log` tirou do
/// caminho ao levar o cabecalho so no `sincronizar`. O relogio da o piso de
/// graca: 65.536 ids por milissegundo antes de o contador passar a frente
/// dele, e passar a frente nao quebra nada (so deixa de usar o relogio).
///
/// # E o relogio que recua (pedido 684)
///
/// Sozinho, um relogio que RECUA entre um arranque e outro emitiria id menor
/// que o da vida anterior, e o diario daquela tabela sairia de ordem -- a
/// venda do trecho chegaria partida na replica sem ninguem contar. O piso do
/// DISCO fecha isso: cada `.log` que abre semeia o `ULTIMO_TX` com o maior id
/// que ele guarda ([`semear_tx`]), e o recuo vira numero
/// ([`recuos_do_relogio`]) em vez de silencio.
pub fn proximo_tx() -> u64 {
    let piso = (relogio_do_tx().max(0) as u64) << 16;
    let anterior = ULTIMO_TX
        .fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |u| Some(u.saturating_add(1).max(piso)),
        )
        .unwrap_or_else(|u| u);
    anterior.saturating_add(1).max(piso)
}

/// Quantos milissegundos o relogio do id anda deslocado do relogio do
/// sistema. Zero fora das provas: e a injecao que deixa o teste recuar o
/// relogio entre dois «arranques» sem dormir nem mexer no relogio da maquina.
static DESVIO_DO_RELOGIO: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Quantas vezes um diario aberto trouxe um id A FRENTE do relogio -- o
/// relogio recuou desde que aquele id foi emitido (pedido 684).
static RECUOS_DO_RELOGIO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Quantas tomadas gravaram evento SEM id (volume 2/3) e COM id (volume 4)
/// juntas -- o commit que a replica recebe partido (pedido 684, b).
static COMMITS_MISTOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn relogio_do_tx() -> i64 {
    agora_ms().saturating_add(DESVIO_DO_RELOGIO.load(std::sync::atomic::Ordering::Relaxed))
}

/// Sobe o `ULTIMO_TX` do processo ate `tx`, o maior id que um diario guarda
/// -- pedido 684. Nunca desce (`fetch_max`): duas tabelas abrindo em
/// qualquer ordem dao o mesmo piso.
///
/// Conta o recuo quando o id do disco esta a frente do relogio de agora E
/// foi ele que subiu o piso: a segunda tabela da mesma vida, ja coberta pela
/// primeira, nao conta de novo o mesmo recuo.
pub fn semear_tx(tx: u64) {
    if tx == 0 {
        return;
    }
    let antes = ULTIMO_TX.fetch_max(tx, std::sync::atomic::Ordering::SeqCst);
    let agora = (relogio_do_tx().max(0) as u64) << 16;
    if tx > antes && tx > agora {
        RECUOS_DO_RELOGIO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Ver [`RECUOS_DO_RELOGIO`]. Vai ao `replicacao_estado`.
pub fn recuos_do_relogio() -> u64 {
    RECUOS_DO_RELOGIO.load(std::sync::atomic::Ordering::Relaxed)
}

/// Ver [`COMMITS_MISTOS`]. Vai ao `replicacao_estado`.
pub fn commits_mistos() -> u64 {
    COMMITS_MISTOS.load(std::sync::atomic::Ordering::Relaxed)
}

/// PROVA: desloca o relogio do id em `ms` (negativo = recua). Ver
/// [`DESVIO_DO_RELOGIO`].
#[doc(hidden)]
pub fn desviar_relogio_do_tx_para_teste(ms: i64) {
    DESVIO_DO_RELOGIO.store(ms, std::sync::atomic::Ordering::Relaxed);
}

/// PROVA: esquece o `ULTIMO_TX`, que e o que um processo novo tem -- o
/// «segundo arranque» dentro do mesmo binario de teste.
#[doc(hidden)]
pub fn esquecer_ultimo_tx_para_teste() {
    ULTIMO_TX.store(0, std::sync::atomic::Ordering::SeqCst);
}

// ------------------------------------- o teto da transacao (676, 685)

/// Quanto UMA transacao pode ocupar -- a constante UNICA da origem e da
/// replica (pedido 685).
///
/// # Por que um teto
///
/// A replica aplica a transacao da origem inteira ou nada (pedido 676), e
/// para isso a tem inteira na memoria antes de tomar a trava. Os maduros nao
/// tem este teto porque derramam em disco (o `relay log` do MySQL e do
/// MariaDB, o `logical_decoding_work_mem` do PostgreSQL); aqui ainda nao ha
/// onde derramar.
///
/// # Por que a ORIGEM recusa (decisao do dono, 07/10/2026)
///
/// Ate o 685 a transacao acima do teto chegava a replica em pedacos, contada.
/// O dono decidiu que «a venda chega inteira ou nao chega» vale sem excecao:
/// o `COMMIT` que passaria do teto e recusado na origem, antes da marca, com
/// nada gravado, dizendo o tamanho e que a carga deve ser dividida. E a mesma
/// constante dos dois lados, e por isso ela mora aqui, onde os dois a leem:
/// duas copias divergiriam no dia em que alguem mexesse numa.
///
/// 64 MiB: uma venda de supermercado sao alguns KiB; o teto existe para a
/// carga em massa, nao para o caso que o 325 descreve.
pub const TETO_DA_TRANSACAO: usize = 64 * 1024 * 1024;

/// O que um evento custa na conta do teto, alem da imagem: a `struct` e o
/// que o `VecDeque` da replica guarda dele. Conta folgada de proposito.
pub const CUSTO_DO_EVENTO: usize = 128;

/// O teto injetado pela prova; zero = o [`TETO_DA_TRANSACAO`].
static TETO_DE_TESTE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// O teto vigente: o [`TETO_DA_TRANSACAO`], ou o que a prova injetou.
pub fn teto_da_transacao() -> usize {
    match TETO_DE_TESTE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => TETO_DA_TRANSACAO,
        t => t,
    }
}

/// PROVA: um teto pequeno no lugar dos 64 MiB, para o mesmo processo que
/// sobe a origem e a replica. Zero volta ao de fabrica.
#[doc(hidden)]
pub fn definir_teto_da_transacao_para_teste(bytes: usize) {
    TETO_DE_TESTE.store(bytes, std::sync::atomic::Ordering::Relaxed);
}

/// O custo de um evento de imagem com `tam_imagem` bytes na conta do teto --
/// a MESMA conta na origem (que recusa) e na replica (que junta).
pub fn custo_na_transacao(tam_imagem: usize) -> usize {
    tam_imagem.saturating_add(CUSTO_DO_EVENTO)
}

/// Abre a unidade de transacao desta thread: todo evento gravado ate
/// [`fechar_unidade`] leva o mesmo id. Quem abre e a tomada da trava de
/// escrita do servidor -- que e uma por commit, e de uma thread so.
///
/// Custa uma escrita numa `Cell` de thread; a tomada que nao grava evento
/// nenhum nao gasta id.
pub fn abrir_unidade() {
    UNIDADE.with(|u| u.set(Some(0)));
    MISTURA.with(|m| m.set((false, false)));
}

/// Fecha a unidade desta thread. O evento gravado fora de unidade (a CLI, a
/// FFI, o `phxsql-store` usado direto) ganha um id so dele.
pub fn fechar_unidade() {
    UNIDADE.with(|u| u.set(None));
    if MISTURA.with(|m| m.replace((false, false))) == (true, true) {
        COMMITS_MISTOS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Ha unidade aberta nesta thread? -- pedido 701 (b).
///
/// A recuperacao de uma marca abre a PROPRIA unidade so quando ninguem abriu:
/// no reparo de um panico a tomada da trava ja tem a dela, e e a mesma que
/// gravou a primeira metade do grupo -- fecha-la ali partiria o grupo.
pub fn unidade_aberta() -> bool {
    UNIDADE.with(|u| u.get().is_some())
}

/// A unidade aberta que ainda nao gravou evento passa a levar o id `tx` --
/// pedido 701 (b). Devolve se adotou.
///
/// Existe para a marca que o arranque completa: a metade do grupo que entrou
/// antes da queda ja esta no diario com um id, e o resto, com um id novo,
/// chegaria a replica encadeada como OUTRA transacao -- o grupo em pedacos. O
/// id adotado ja foi emitido e o `.log` que o guarda semeou o
/// [`proximo_tx`] com ele ([`semear_tx`]), entao nao recua ordem nenhuma. Zero
/// e «sem id» (volume 2/3) e nao se adota; a unidade que ja gravou fica com o
/// dela.
pub fn adotar_tx_na_unidade(tx: u64) -> bool {
    if tx == 0 {
        return false;
    }
    UNIDADE.with(|u| {
        if u.get() == Some(0) {
            u.set(Some(tx));
            true
        } else {
            false
        }
    })
}

/// Anota na unidade aberta se o evento foi com id ou sem -- pedido 684 (b).
/// Fora de unidade nao ha commit para misturar.
fn anotar_mistura(com_id: bool) {
    if UNIDADE.with(std::cell::Cell::get).is_none() {
        return;
    }
    MISTURA.with(|m| {
        let (a, b) = m.get();
        m.set((a || com_id, b || !com_id));
    });
}

/// O id do evento que vai ser gravado agora.
fn tx_do_evento() -> u64 {
    UNIDADE.with(|u| match u.get() {
        Some(0) => {
            let t = proximo_tx();
            u.set(Some(t));
            t
        }
        Some(t) => t,
        None => proximo_tx(),
    })
}

pub struct LogFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// Ate onde a ultima varredura chegou, para a proxima nao recomecar.
    marca: Option<MarcaDoDiario>,
    /// Usuario aplicado aos eventos gravados daqui em diante.
    pub usuario: u32,
    /// O evento que este diario deve (pedido 498). De pe, nada mais se anexa
    /// -- ver [`LogFile::conferir_teto`] -- ate a abertura completar.
    devendo: Option<EventoDevido>,
}

impl LogFile {
    /// Os descritores abertos, para o `fsync` da criacao fora da trava --
    /// pedido 589. Ver [`crate::volume::Volumes::descritores`].
    pub(crate) fn descritores(&self) -> std::io::Result<Vec<(std::fs::File, std::path::PathBuf)>> {
        self.volumes.descritores()
    }
    pub fn criar(diretorio: impl AsRef<Path>, nome: &str, paginacao: Paginacao) -> Result<LogFile> {
        // O diario corta o volume no tamanho DELE, que nao e o do `.bin`. Ver
        // `crate::diario`: sem configuracao, manda o esquema, como sempre.
        let paginacao = crate::diario::paginacao(paginacao);
        let mut l = LogFile {
            volumes: Volumes::novo(diretorio, nome, EXT_LOG, paginacao),
            cabs: HashMap::new(),
            volume_atual: 1,
            marca: None,
            usuario: 0,
            devendo: None,
        };
        l.volumes.criar(1)?;
        l.gravar_cab(Cabecalho::novo_do_diario(1)?)?;
        Ok(l)
    }

    pub fn abrir(diretorio: impl AsRef<Path>, nome: &str, paginacao: Paginacao) -> Result<LogFile> {
        let paginacao = crate::diario::paginacao(paginacao);
        let volumes = Volumes::novo(diretorio, nome, EXT_LOG, paginacao);
        let existentes = volumes.existentes();
        if existentes.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhum volume de {}",
                volumes.caminho(1).display()
            )));
        }
        let mut volumes = volumes;
        let volume_atual = match sem_cabecalho(&mut volumes, &existentes)? {
            // O volume que nasceu sem cabecalho nunca teve evento: sai.
            Some(orfao) => {
                volumes.apagar_volume(orfao)?;
                orfao - 1
            }
            None => *existentes.last().unwrap(),
        };
        let mut l = LogFile {
            volumes,
            cabs: HashMap::new(),
            volume_atual,
            marca: None,
            usuario: 0,
            devendo: None,
        };
        let primeiro = l.cab(1)?;
        l.cab(volume_atual)?;
        // So o volume CORRENTE pode ter ficado atrasado: os anteriores foram
        // fechados quando a paginacao virou, e ali o cabecalho vai a disco na
        // hora.
        l.curar(volume_atual, true)?;
        l.devendo = l.devido_no(&primeiro)?;
        // O piso do DISCO para o id de transacao (pedido 684): depois da
        // cura, que conta o que entrou desde o ultimo `sincronizar`. So na
        // abertura com escrita -- e ela que precede todo evento novo desta
        // tabela, entao o diario dela nunca recebe id menor que o ultimo.
        semear_tx(l.cab(volume_atual)?.ultimo_tx);
        Ok(l)
    }

    /// Abre SEM escrever nada, e devolve `None` quando abrir exigiria escrever.
    ///
    /// O unico `write` de [`LogFile::abrir`] esta na CURA: um diario cujo
    /// cabecalho ficou para tras numa queda tem o fim recalculado e regravado.
    /// Isso e recuperacao, e recuperacao acontece sob a ficha exclusiva -- sob
    /// a compartilhada ha outros leitores nos mesmos arquivos, e o que os
    /// mantem seguros e ninguem escrever.
    pub fn abrir_sem_escrever(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<Option<LogFile>> {
        LogFile::abrir_para_ler_com(diretorio, nome, paginacao, false)
    }

    /// O [`LogFile::abrir_sem_escrever`] que NAO recusa a cauda alem do
    /// cabecalho: conta com ela na memoria (ver [`LogFile::curar_em_memoria`])
    /// -- pedido 330. So para quem le o DIARIO sob a ficha compartilhada: a
    /// tabela escrita desde o ultimo fecho da janela tem o cabecalho atras do
    /// arquivo, e sem isto toda fatia da absorcao do bidirecional recusava sob
    /// um escritor da mesma tabela, devolvendo a absorcao inteira a exclusiva.
    pub fn abrir_sem_escrever_com_a_cauda(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<Option<LogFile>> {
        LogFile::abrir_para_ler_com(diretorio, nome, paginacao, true)
    }

    fn abrir_para_ler_com(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
        cauda_em_memoria: bool,
    ) -> Result<Option<LogFile>> {
        let paginacao = crate::diario::paginacao(paginacao);
        let volumes = Volumes::novo(diretorio, nome, EXT_LOG, paginacao);
        let existentes = volumes.existentes();
        if existentes.is_empty() {
            return Err(PhxError::NaoEncontrado(format!(
                "nenhum volume de {}",
                volumes.caminho(1).display()
            )));
        }
        let mut volumes = volumes;
        // Apagar o volume que nasceu sem cabecalho e escrever.
        if sem_cabecalho(&mut volumes, &existentes)?.is_some() {
            return Ok(None);
        }
        let volume_atual = *existentes.last().unwrap();
        let mut l = LogFile {
            volumes,
            cabs: HashMap::new(),
            volume_atual,
            marca: None,
            usuario: 0,
            devendo: None,
        };
        // O evento devido (498) se completa escrevendo.
        if l.cab(1)?.marca.is_some() {
            return Ok(None);
        }
        l.cab(volume_atual)?;
        // A cura ANDA, e nao grava: se ela achou evento alem do fim que o
        // cabecalho declara, e porque o cabecalho precisa ser corrigido -- e
        // corrigir e escrever. Quem so le o diario conta com a cauda na
        // memoria em vez de recusar.
        if cauda_em_memoria {
            l.curar_em_memoria(volume_atual)?;
        } else if l.curar(volume_atual, false)? > 0 {
            return Ok(None);
        }
        Ok(Some(l))
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let cab = cofre::ler_cabecalho_do_volume(
            &mut self.volumes,
            volume,
            MAGIC_LOG,
            cofre::VERSAO_COM_TX,
        )?;
        self.cabs.insert(volume, cab);
        Ok(cab)
    }

    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        cofre::gravar_cabecalho_no_volume(&mut self.volumes, &cab, MAGIC_LOG)?;
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
        self.registrar_detalhado(operacao, rowid, versao, imagem, None, 0)
    }

    /// Registra um evento com carimbo e origem VINDOS DE FORA.
    ///
    /// E o caminho do bidirecional: um evento aplicado aqui tem de guardar o
    /// instante em que a escrita NASCEU no outro servidor, e nao o instante em
    /// que chegou -- e o carimbo que decide o conflito, e comparar hora de
    /// chegada elegeria sempre quem sincroniza por ultimo. A origem e o que
    /// impede o evento de voltar para o servidor de onde veio.
    ///
    /// `carimbo` em `None` usa o relogio local, que e o caso da escrita local.
    pub fn registrar_detalhado(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        versao: u64,
        imagem: &[u8],
        carimbo: Option<i64>,
        origem: u16,
    ) -> Result<Evento> {
        self.conferir_devendo()?;
        conferir_imagem(imagem.len())?;
        // O tamanho que vai ao arquivo pode ser maior que o da imagem: num
        // volume cifrado o corpo leva a etiqueta de 16 bytes atras dele.
        let atual = self.cab(self.volume_atual)?;
        let evento = Evento {
            carimbo: carimbo.unwrap_or_else(agora_ms),
            operacao,
            rowid,
            versao,
            usuario: self.usuario,
            origem,
            tam_imagem: atual.ocupa(imagem.len()) as u32,
            tx: tx_do_evento(),
            largo: atual.com_tx,
        };
        // O evento como FOI gravado: o volume velho zera o `tx` e encurta o
        // evento, e devolver o de antes diria um id que o disco nao tem.
        let evento = self.anexar(evento, imagem)?;
        // O PONTO UNICO onde o diario cresce (pedido 207): e aqui, e nao em
        // cada familia de escrita do servidor, que a tabela tocada se anota --
        // uma familia esquecida seria um commit que responde «gravei» sem
        // esperar quorum nenhum (guarda `quorum-escritor-sem-espera`).
        if ANOTANDO.with(std::cell::Cell::get) {
            self.anotar_tocada();
        }
        Ok(evento)
    }

    /// Anota este diario na lista da tomada corrente. O total vem dos
    /// cabecalhos ja em memoria (o do volume corrente acabou de ser escrito),
    /// e o erro de leitura de um volume antigo NAO derruba a escrita: a
    /// tabela entra com `antes == depois`, que o quorum trata como «nao sei
    /// confirmar» e diz `alcancado:false` -- calar seria pior.
    fn anotar_tocada(&mut self) {
        let total = self.total().ok();
        let diretorio = self.volumes.diretorio().to_path_buf();
        let nome = self.volumes.nome().to_string();
        TOCADAS.with(|t| {
            let mut t = t.borrow_mut();
            if let Some(x) = t
                .iter_mut()
                .find(|x| x.nome == nome && x.diretorio == diretorio)
            {
                x.depois = total.unwrap_or(x.antes);
                return;
            }
            let depois = total.unwrap_or(0);
            let antes = if total.is_some() {
                depois.saturating_sub(1)
            } else {
                0
            };
            t.push(Tocada {
                diretorio,
                nome,
                antes,
                depois,
            });
        });
    }

    /// O volume onde um evento de `ocupa` bytes vai entrar, e se ele vira de
    /// volume -- ou a recusa do teto. Uma decisao so, para o [`Self::anexar`]
    /// e para o [`Self::conferir_teto`]: a conferencia de antes nao pode
    /// responder diferente da gravacao de depois.
    fn destino(&mut self, ocupa: u64) -> Result<(u32, bool, Cabecalho)> {
        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let vazio = atual.fim <= atual.cab_len as u64;
        let (volume, virou) = paginacao.volume_externo(self.volume_atual, atual.fim, ocupa, vazio);
        if virou && paginacao.ligada() && volume > paginacao.max_arquivos {
            return Err(PhxError::LimiteExcedido(format!(
                "diario de {} chegou ao teto de {} volumes",
                self.volumes.nome(),
                paginacao.max_arquivos
            )));
        }
        Ok((volume, virou, atual))
    }

    /// Cabe mais um evento com imagem de `tam_imagem` bytes? Pedido 498: a
    /// tabela pergunta ANTES de gravar a linha, porque recusar depois deixava
    /// o valor novo no `.reg` sem diario -- replica divergindo e cascata
    /// pulada. Nao escreve nada.
    ///
    /// Recusa tambem a imagem acima do [`IMAGEM_MAX`] -- a MESMA conta do
    /// [`Self::registrar_detalhado`] -- e o diario que esta devendo um evento:
    /// anexar outro antes do devido poria na replica a alteracao de uma linha
    /// que ela ainda nao recebeu.
    pub fn conferir_teto(&mut self, tam_imagem: usize) -> Result<()> {
        self.conferir_devendo()?;
        conferir_imagem(tam_imagem)?;
        let atual = self.cab(self.volume_atual)?;
        let ocupa = largura(&atual) as u64 + atual.ocupa(tam_imagem) as u64;
        self.destino(ocupa).map(|_| ())
    }

    fn conferir_devendo(&self) -> Result<()> {
        match &self.devendo {
            None => Ok(()),
            Some(d) => Err(PhxError::Io(std::io::Error::other(format!(
                "o diario de {} deve o evento de {} da linha {}: a gravacao dele \
                 falhou depois de a linha estar no .reg. Nada mais se grava nesta \
                 tabela ate ela ser aberta de novo -- a abertura completa o evento \
                 a partir da linha (pedido 498)",
                self.volumes.nome(),
                d.operacao.nome(),
                d.rowid
            )))),
        }
    }

    /// O evento que este diario deve, se deve. Ver [`EventoDevido`].
    pub fn devido(&self) -> Option<EventoDevido> {
        self.devendo
    }

    fn devido_no(&self, cab: &Cabecalho) -> Result<Option<EventoDevido>> {
        let nome = self.volumes.caminho(1).display().to_string();
        cab.marca
            .as_ref()
            .map(|m| EventoDevido::da_marca(m, &nome))
            .transpose()
    }

    /// Registra o evento de uma linha que JA ESTA no `.reg` -- pedido 498.
    ///
    /// O `.log` que falha aqui deixaria a linha sem diario: replica
    /// divergindo e cascata pulada. A decisao do dono (30/09/2026) e a do
    /// PostgreSQL na falha de escrita do WAL: derrubar e completar. Entao,
    /// na falha:
    ///
    /// 1. a marca do evento devido vai ao cabecalho do volume 1, NO LUGAR --
    ///    sobrescrever bytes que ja existem nao pede espaco novo, e o disco
    ///    cheio e justamente o que nao tem. Um arquivo a parte (H2) morreria
    ///    ai: medido no tmpfs cheio, o arquivo novo nasce com 0 bytes;
    /// 2. o gancho do processo roda ([`crate::sincronia::diario_sem_evento`]):
    ///    o servidor aborta ali, com a trava na mao, sem responder;
    /// 3. a biblioteca, que nao cai, fica com o erro -- e o diario recusa
    ///    toda escrita ate a tabela ser reaberta.
    ///
    /// A abertura seguinte completa o evento pela linha
    /// ([`LogFile::completar_devido`]).
    ///
    /// # Por que a marca so sobe na falha, e nao antes de cada escrita
    ///
    /// Subi-la antes do `.reg` e baixa-la depois do evento (H1) cobriria
    /// tambem o `SIGKILL` entre os dois -- a dois `pwrite` por linha, medidos
    /// em 0,39-0,47 us cada, sobre 3,8 us da insercao sem indice. Aqui o
    /// processo SABE que falhou, e a marca sobe no mesmo lugar sem custar nada
    /// ao laco quente. O `SIGKILL` no meio fica como estava (ver o 498 no
    /// `PENDENCIAS.md`).
    pub fn registrar_depois_da_linha(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        versao: u64,
        imagem: &[u8],
        carimbo: Option<i64>,
        origem: u16,
    ) -> Result<Evento> {
        let carimbo = carimbo.unwrap_or_else(agora_ms);
        let e = match self.registrar_detalhado(
            operacao,
            rowid,
            versao,
            imagem,
            Some(carimbo),
            origem,
        ) {
            Ok(e) => return Ok(e),
            Err(e) => e,
        };
        // O diario ja devendo nao troca a marca: a primeira e a que a
        // abertura completa, e a tabela recusou esta escrita antes de gravar
        // a linha.
        if self.devendo.is_some() {
            return Err(e);
        }
        let marcou = self.marcar_devido(EventoDevido {
            operacao,
            rowid,
            carimbo,
            origem,
            usuario: self.usuario,
            com_imagem: !imagem.is_empty(),
        });
        let io = std::io::Error::other(match &marcou {
            Ok(()) => e.to_string(),
            Err(m) => format!("{e} (e a marca do evento devido tambem falhou: {m})"),
        });
        crate::sincronia::diario_sem_evento(&self.volumes.caminho(1), &io);
        Err(PhxError::Io(std::io::Error::other(format!(
            "a linha {rowid} esta no .reg e o diario nao gravou o evento de {} \
             dela ({io}). {} (pedido 498)",
            operacao.nome(),
            if marcou.is_ok() {
                "A marca ficou no cabecalho do .log e a proxima abertura da tabela \
                 completa o evento; ate la ela nao grava mais nada"
            } else {
                "A MARCA NAO FICOU: a proxima abertura nao sabera qual evento falta"
            }
        ))))
    }

    fn marcar_devido(&mut self, d: EventoDevido) -> Result<()> {
        self.devendo = Some(d);
        let c = self.cab(1)?;
        self.gravar_cab(c.com_marca(Some(d.marca())))
    }

    /// Completa o evento devido, com a versao e a imagem que a TABELA
    /// derivou da linha -- pedido 498. Sem marca, nao faz nada.
    ///
    /// A ordem e a que torna a repeticao inofensiva: o evento, o `fsync` do
    /// diario, e so entao a marca desce. Uma queda entre os dois deixa a
    /// marca de pe com o evento ja la, e a proxima abertura o reconhece pelo
    /// par (operacao, rowid, carimbo) no fim do diario, em vez de anexa-lo
    /// duas vezes -- nada mais se anexou depois dele, porque o diario
    /// devendo recusa.
    pub fn completar_devido(&mut self, versao: u64, imagem: &[u8]) -> Result<()> {
        let Some(d) = self.devendo else {
            return Ok(());
        };
        let total = self.total()?;
        let ja_esta = total > 0
            && self.ler(total - 1, 1)?.first().is_some_and(|e| {
                e.operacao == d.operacao && e.rowid == d.rowid && e.carimbo == d.carimbo
            });
        if !ja_esta {
            self.devendo = None;
            let usuario = std::mem::replace(&mut self.usuario, d.usuario);
            let feito = self.registrar_detalhado(
                d.operacao,
                d.rowid,
                versao,
                imagem,
                Some(d.carimbo),
                d.origem,
            );
            self.usuario = usuario;
            if let Err(e) = feito {
                self.devendo = Some(d);
                return Err(e);
            }
            self.sincronizar()?;
        }
        self.devendo = None;
        let c = self.cab(1)?;
        self.gravar_cab(c.com_marca(None))
    }

    fn anexar(&mut self, mut evento: Evento, imagem: &[u8]) -> Result<Evento> {
        let (volume, virou, atual) = self.destino(evento.ocupa())?;

        let cab = if virou {
            self.volumes.garantir(volume)?;
            // O volume NOVO sorteia o proprio sal, e por isso tem a propria
            // chave: e o que deixa o numero de ordem do nonce ser o offset
            // dentro do volume sem nunca repetir o par (chave, nonce).
            // O volume novo herda o maior id do anterior (pedido 684): sem
            // isto, a virada seguida de uma queda antes do primeiro
            // `sincronizar` deixaria o volume corrente dizendo «nenhum id», e
            // a abertura nao teria piso nenhum.
            let novo = Cabecalho::novo_do_diario(volume)?.com_tx_visto(atual.ultimo_tx);
            self.gravar_cab(novo)?;
            self.volume_atual = volume;
            novo
        } else {
            atual
        };
        // Virar de volume pode ter trocado a cifra (o volume velho em claro, o
        // novo cifrado): o tamanho que vai ao cabecalho e o do volume DESTINO.
        evento.tam_imagem = cab.ocupa(imagem.len()) as u32;
        // E a largura do evento tambem: o volume velho (2 ou 3) nao tem onde
        // guardar o id, e o evento entra nele com 44 bytes e `tx` zero.
        evento.largo = cab.com_tx;
        if !evento.largo {
            evento.tx = 0;
        }
        anotar_mistura(evento.largo);

        let tempero = tempero_novo(&cab);
        let mut cheio = [0u8; EVENTO_CAB_TX];
        let buf = &mut cheio[..largura(&cab)];
        evento.escrever(buf, tempero);
        let corpo = cab.selar(tempero, cab.fim, &Evento::associado(buf), imagem);
        evento.conferir_e_fechar(buf, &corpo);
        #[cfg(debug_assertions)]
        if let Some(erro) = crate::sincronia::falha_de_teste::disparar(
            &self.volumes.caminho(volume),
            crate::sincronia::falha_de_teste::Onde::GravacaoDoDiario,
        ) {
            return Err(PhxError::Io(erro));
        }
        self.volumes.escrever(volume, cab.fim, buf)?;
        if !corpo.is_empty() {
            self.volumes
                .escrever(volume, cab.fim + buf.len() as u64, &corpo)?;
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
            cab.com(cab.fim + evento.ocupa(), cab.quantos + 1)
                .com_tx_visto(evento.tx),
        );
        Ok(evento)
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
    fn curar(&mut self, volume: u32, gravar: bool) -> Result<u64> {
        let (achados, cab) = self.cauda_alem_do_cabecalho(volume)?;
        if achados > 0 && gravar {
            self.gravar_cab(cab)?;
        }
        Ok(achados)
    }

    /// A mesma varredura da cura, com o cabecalho corrigido SO na memoria
    /// deste `LogFile` -- pedido 330.
    ///
    /// Existe para quem le o diario sob a ficha compartilhada: com ela na mao
    /// nenhum escritor anexa, entao o arquivo esta parado, e os eventos alem
    /// do `fim` gravado sao eventos inteiros (o CRC de cada um confere) que so
    /// esperam o `sincronizar` para entrar no cabecalho. Contar com eles na
    /// memoria da a mesma vista que a abertura exclusiva daria depois de
    /// curar -- sem gravar um byte.
    fn curar_em_memoria(&mut self, volume: u32) -> Result<u64> {
        let (achados, cab) = self.cauda_alem_do_cabecalho(volume)?;
        if achados > 0 {
            self.cabs.insert(volume, cab);
        }
        Ok(achados)
    }

    /// Quantos eventos inteiros o arquivo tem alem do `fim` do cabecalho, e o
    /// cabecalho que os contaria. Nao grava nada.
    fn cauda_alem_do_cabecalho(&mut self, volume: u32) -> Result<(u64, Cabecalho)> {
        // O reparo pode cortar o rabo do arquivo: uma marca apontando para
        // dentro do que sumiu passaria a apontar para nada.
        self.marca = None;
        let mut cab = self.cab(volume)?;
        let tamanho = self.volumes.tamanho(volume)?;
        let mut achados = 0u64;

        let larg = largura(&cab);
        while cab.fim + larg as u64 <= tamanho {
            let mut cheio = [0u8; EVENTO_CAB_TX];
            let buf = &mut cheio[..larg];
            self.volumes.ler(volume, cab.fim, buf)?;
            let evento = match Evento::ler(buf, cab.com_tx) {
                Ok(e) => e,
                Err(_) => break,
            };
            if cab.fim + evento.ocupa() > tamanho {
                break;
            }
            if evento.tam_imagem > 0 {
                let mut imagem = vec![0u8; evento.tam_imagem as usize];
                self.volumes
                    .ler(volume, cab.fim + larg as u64, &mut imagem)?;
                if evento.conferir(buf, &imagem).is_err() {
                    break;
                }
            }
            // A cura anda pelo CRC, e nao pela chave: um volume cifrado tem de
            // se curar do mesmo jeito, e decifrar aqui obrigaria a ter a chave
            // so para saber onde o arquivo acaba.
            cab = cab
                .com(cab.fim + evento.ocupa(), cab.quantos + 1)
                .com_tx_visto(evento.tx);
            achados += 1;
        }
        Ok((achados, cab))
    }

    /// Total de eventos em todos os volumes.
    pub fn total(&mut self) -> Result<u64> {
        let mut t = 0;
        for v in self.volumes.existentes() {
            t += self.cab(v)?.quantos;
        }
        Ok(t)
    }

    /// Le os eventos em ordem cronologica, do mais antigo para o mais recente.
    ///
    /// `pular` descarta os N primeiros; `limite` zero devolve todos.
    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        Ok(self
            .percorrer(pular, limite, false, usize::MAX)?
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
        self.percorrer(pular, limite, true, usize::MAX)
    }

    /// [`LogFile::ler_com_imagem`] com TETO DE BYTES de imagem.
    ///
    /// Para de ler quando o proximo evento nao cabe mais em `teto_bytes` --
    /// e decide isso pelo cabecalho, ANTES de alocar a imagem. `limite` zero
    /// continua querendo dizer "todos os eventos", mas "todos" passa a caber
    /// no teto: e o que faz um `replicar` pedido com `max: 0` nao caminhar o
    /// diario inteiro para a memoria com a trava global na mao (revisao SEC
    /// de 17/09/2026, A2 -- o teto que cortava a RESPOSTA depois de ler tudo
    /// nao limitava a leitura). O primeiro evento entra sempre, custe o que
    /// custar: uma linha maior que o teto atrasa a replicacao em vez de
    /// para-la para sempre. A marca fica no evento que NAO coube, entao a
    /// chamada seguinte continua exatamente dali.
    pub fn ler_com_imagem_ate(
        &mut self,
        pular: u64,
        limite: u64,
        teto_bytes: usize,
    ) -> Result<Vec<(Evento, Vec<u8>)>> {
        self.percorrer(pular, limite, true, teto_bytes)
    }

    /// A varredura unica dos dois caminhos.
    ///
    /// Desde que o evento deixou de ter largura fixa, chegar ao evento N e
    /// caminhar pelos anteriores. O que ainda se pula de graca e o VOLUME
    /// inteiro: o `qtd_eventos` do cabecalho diz quantos ele tem, e se todos
    /// eles estao antes do `pular` o arquivo nem se abre.
    ///
    /// `teto_bytes` so vale com imagem (sem imagem nada se aloca), e conta o
    /// `tam_imagem` do cabecalho -- o que vai ser alocado -- e nao o texto
    /// claro, que num volume cifrado e 16 bytes menor.
    fn percorrer(
        &mut self,
        pular: u64,
        limite: u64,
        com_imagem: bool,
        teto_bytes: usize,
    ) -> Result<Vec<(Evento, Vec<u8>)>> {
        let mut saida = Vec::new();
        let mut somados = 0usize;

        // De onde comecar. A marca so serve para uma posicao que esteja DEPOIS
        // dela: caminhar para tras nao da, o evento nao tem largura fixa. E so
        // se ela for DESTE diario (pedido 620): a de outra vida da tabela
        // custa a varredura do comeco, nunca um evento pulado.
        let marca = self.marca.filter(|m| m.evento <= pular);
        let marca = match marca {
            Some(m) if self.marca_confere(&m) => Some(m),
            _ => None,
        };
        let (mut vistos, comeco) = match marca {
            Some(m) => (m.evento, Some(m)),
            None => (0, None),
        };
        // O ultimo evento entregue, que vira a ancora da marca seguinte.
        let mut ultimo: Option<AncoraDaMarca> = None;

        for volume in self.volumes.existentes() {
            if let Some(m) = comeco {
                if volume < m.volume {
                    continue; // ja contado dentro do `vistos` da marca
                }
            }
            let cab = self.cab(volume)?;
            // O volume inteiro se pula de graca pelo `quantos` do cabecalho --
            // mas so quando `vistos` esta no comeco dele, e nao no meio, que e
            // onde a marca pode ter parado.
            let no_comeco_do_volume = !matches!(comeco, Some(m) if m.volume == volume);
            if no_comeco_do_volume && vistos + cab.quantos <= pular {
                vistos += cab.quantos;
                continue;
            }
            let mut offset = match comeco {
                Some(m) if m.volume == volume => m.offset,
                _ => cab.cab_len as u64,
            };
            let nome = self.volumes.caminho(volume).display().to_string();
            let larg = largura(&cab);
            while offset + larg as u64 <= cab.fim {
                let mut cheio = [0u8; EVENTO_CAB_TX];
                let buf = &mut cheio[..larg];
                self.volumes.ler(volume, offset, buf)?;
                let evento = Evento::ler(buf, cab.com_tx)?;
                if vistos >= pular {
                    // O teto de BYTES, respondido pelo cabecalho antes de
                    // qualquer alocacao. O primeiro evento entra sempre.
                    let ocupa = evento.tam_imagem as usize;
                    if com_imagem && !saida.is_empty() && somados.saturating_add(ocupa) > teto_bytes
                    {
                        if let Some(ancora) = ultimo {
                            self.marca = Some(MarcaDoDiario {
                                evento: vistos,
                                volume,
                                offset,
                                ancora,
                            });
                        }
                        return Ok(saida);
                    }
                    somados = somados.saturating_add(ocupa);
                    let mut imagem = Vec::new();
                    if evento.tam_imagem > 0 {
                        imagem = vec![0u8; evento.tam_imagem as usize];
                        self.volumes
                            .ler(volume, offset + larg as u64, &mut imagem)?;
                        evento.conferir(buf, &imagem)?;
                        if com_imagem {
                            // O nonce sai do offset do evento no volume, que
                            // e a ordem que um arquivo append-only nunca
                            // reaproveita. Ver `cofre::nonce_de`.
                            imagem = cab.abrir(
                                Evento::tempero(buf),
                                offset,
                                &Evento::associado(buf),
                                &imagem,
                                &nome,
                            )?;
                        } else {
                            imagem.clear();
                        }
                    }
                    saida.push((evento, imagem));
                    let ancora = AncoraDaMarca {
                        volume,
                        offset,
                        selo: crc32(buf),
                    };
                    ultimo = Some(ancora);
                    if limite > 0 && saida.len() as u64 >= limite {
                        // A marca aponta para o PROXIMO, que e o que o leitor
                        // sequencial vai pedir na chamada seguinte.
                        self.marca = Some(MarcaDoDiario {
                            evento: vistos + 1,
                            volume,
                            offset: offset + evento.ocupa(),
                            ancora,
                        });
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += evento.ocupa();
            }
        }
        // Leu ate o fim: a marca fica logo depois do ultimo entregue. Sem
        // isto, quem le o diario inteiro em lotes curtos saia sem marca nenhuma
        // da ultima chamada, e a dica guardada era a de um lote atras -- ou
        // nenhuma, e ai nao havia ancora para provar de que vida ela era.
        if let Some(ancora) = ultimo {
            let (ultimo_ev, _) = saida.last().expect("ha ancora, ha evento");
            self.marca = Some(MarcaDoDiario {
                evento: vistos,
                volume: ancora.volume,
                offset: ancora.offset + ultimo_ev.ocupa(),
                ancora,
            });
        }
        Ok(saida)
    }

    /// A marca e DESTE diario? -- pedido 620.
    ///
    /// Confere o cabecalho do evento ancora contra o selo que a marca levou.
    /// Volume que nao existe, ancora depois do `fim` ou leitura que falha
    /// respondem `false`: a pergunta e «posso confiar nela», e na duvida a
    /// resposta custa uma varredura, nunca um evento.
    pub fn marca_confere(&mut self, m: &MarcaDoDiario) -> bool {
        let a = m.ancora;
        if !self.volumes.existe(a.volume) {
            return false;
        }
        let Ok(cab) = self.cab(a.volume) else {
            return false;
        };
        let larg = largura(&cab);
        if a.offset < cab.cab_len as u64 || a.offset + larg as u64 > cab.fim {
            return false;
        }
        let mut cheio = [0u8; EVENTO_CAB_TX];
        let buf = &mut cheio[..larg];
        if self.volumes.ler(a.volume, a.offset, buf).is_err() {
            return false;
        }
        if crc32(buf) != a.selo {
            return false;
        }
        // A posicao da marca tem de ser o fim do evento ancora (mesmo volume)
        // ou o comeco do volume seguinte -- senao a marca foi montada a mao.
        let Ok(ev) = Evento::ler(buf, cab.com_tx) else {
            return false;
        };
        let depois = a.offset + ev.ocupa();
        if m.volume == a.volume {
            m.offset == depois
        } else {
            m.volume > a.volume
                && depois == cab.fim
                && self
                    .cab(m.volume)
                    .is_ok_and(|c| m.offset == c.cab_len as u64)
        }
    }

    /// Onde a ultima varredura parou. Ver [`MarcaDoDiario`].
    ///
    /// O servidor abre e fecha a tabela a cada pedido, entao a marca morreria
    /// entre um `replicar` e o seguinte -- que sao justamente os dois pedidos
    /// em que ela vale. Exportar e reimportar deixa quem sabe que os pedidos
    /// sao seguidos guardar a dica, do mesmo jeito que a paginacao ja faz com
    /// o cursor.
    pub fn marca(&self) -> Option<MarcaDoDiario> {
        self.marca
    }

    /// Aceita uma dica de onde comecar. Ver [`MarcaDoDiario`].
    ///
    /// Nao se valida aqui, e de proposito: a conferencia custa uma leitura de
    /// disco e mora onde a marca e USADA ([`LogFile::marca_confere`], chamada
    /// pela varredura). Ate o pedido 620 este comentario dizia que a marca
    /// errada «nao entrega dado errado» -- e entregava: o offset depois do
    /// `fim` de um diario recriado fazia a varredura devolver vazio, e quem le
    /// em sequencia (o mapa de toques do bidirecional) pulava os eventos
    /// novos calado.
    pub fn definir_marca(&mut self, marca: Option<MarcaDoDiario>) {
        self.marca = marca;
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
            let mut offset = cab.cab_len as u64;
            let mut no_volume = 0u64;
            let larg = largura(&cab);
            while offset + larg as u64 <= cab.fim {
                let mut cheio = [0u8; EVENTO_CAB_TX];
                let buf = &mut cheio[..larg];
                self.volumes.ler(volume, offset, buf)?;
                let evento = Evento::ler(buf, cab.com_tx)?; // confere a operacao, e o CRC se nao ha imagem
                if evento.tam_imagem > 0 {
                    // Com imagem o CRC so fecha depois de le-la. Conferir so o
                    // cabecalho aqui deixaria de fora justamente os bytes que
                    // a replica grava como dado.
                    let mut imagem = vec![0u8; evento.tam_imagem as usize];
                    self.volumes
                        .ler(volume, offset + larg as u64, &mut imagem)?;
                    evento.conferir(buf, &imagem)?;
                }
                no_volume += 1;
                offset += evento.ocupa();
            }
            if no_volume != cab.quantos {
                return Err(PhxError::Corrompido(format!(
                    "{}: cabecalho diz {} eventos, varredura achou {no_volume}",
                    self.volumes.caminho(volume).display(),
                    cab.quantos
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

    /// Quantos arquivos o `.log` ja mandou ao disco de verdade. Ver
    /// `Volumes::sincronizados` -- conta o ARQUIVO, e nao a chamada.
    pub fn sincronizados(&self) -> u64 {
        self.volumes.sincronizados()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn dir_temp(rotulo: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("log-{rotulo}"))
    }

    /// Pedido 207: a anotacao das tabelas tocadas guarda a faixa `[antes,
    /// depois)` do diario por tabela, e desligada nao anota nada -- e o que
    /// mantem o laco quente sem custo quando o quorum nao foi pedido.
    #[test]
    fn a_anotacao_das_tocadas_guarda_a_faixa_e_desligada_nao_anota() {
        let d = dir_temp("tocadas");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        assert!(tomar_tocadas().is_empty(), "desligada, nada se anota");
        anotar_tocadas();
        l.registrar(Operacao::Inclusao, 2, 1).unwrap();
        l.registrar(Operacao::Inclusao, 3, 1).unwrap();
        let mut l2 = LogFile::criar(&d, "u", Paginacao::DESLIGADA).unwrap();
        l2.registrar(Operacao::Inclusao, 1, 1).unwrap();
        let t = tomar_tocadas();
        assert!(!anotando_tocadas(), "tomar desliga");
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].nome.as_str(), t[0].antes, t[0].depois), ("t", 1, 3));
        assert_eq!((t[1].nome.as_str(), t[1].antes, t[1].depois), ("u", 0, 1));
        l.registrar(Operacao::Inclusao, 4, 1).unwrap();
        assert!(tomar_tocadas().is_empty(), "depois de tomar, desligada");
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

    /// A origem mora nos 2 bytes que eram reservados: quem grava pelo caminho
    /// normal continua com zero (= local), e o caminho detalhado grava a que
    /// veio de fora, junto com o carimbo original do evento.
    #[test]
    fn origem_e_carimbo_forcados_viajam_e_voltam() {
        let d = dir_temp("origem");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar(Operacao::Inclusao, 1, 1).unwrap();
        l.registrar_detalhado(
            Operacao::Alteracao,
            1,
            2,
            b"imagem qualquer",
            Some(1_700_000_123_456),
            0xBEEF,
        )
        .unwrap();

        let eventos = l.ler(0, 0).unwrap();
        assert_eq!(eventos[0].origem, 0, "escrita local e origem zero");
        assert_eq!(eventos[1].origem, 0xBEEF);
        assert_eq!(
            eventos[1].carimbo, 1_700_000_123_456,
            "o carimbo do conflito e o do NASCIMENTO da escrita, nao o da chegada"
        );
        // E o CRC cobre o campo: um byte trocado na origem derruba o evento.
        assert_eq!(l.verificar().unwrap(), 2);
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
            // O rowid do primeiro evento: o volume da versao 4 tem cabecalho
            // de 128 bytes, em claro ou cifrado (pedido 676).
            v.escrever(1, cofre::CAB_V3 as u64 + 12, &[9u8; 8]).unwrap();
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert!(l.verificar().is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    /* ------------------------------------------ o teto de bytes na LEITURA

    Revisao SEC de 17/09/2026, A2: `replicar` com `"max":0` chegava aqui com
    `limite == 0`, que quer dizer "todos", e o teto de 16 MiB do servidor
    cortava a RESPOSTA depois de o diario inteiro ja estar na memoria -- com
    a trava global na mao. O teto desceu para quem aloca. */

    /// Vinte eventos de 1 KiB e um teto de 4 KiB: voltam QUATRO, e a segunda
    /// chamada continua exatamente do quinto. Com o teto desligado na
    /// varredura (o defeito reposto), voltam os vinte.
    #[test]
    fn percorrer_com_limite_zero_nao_le_tudo() {
        let d = dir_temp("teto-bytes");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 1..=20u64 {
            l.registrar_com_imagem(Operacao::Inclusao, i, 1, &vec![i as u8; 1024])
                .unwrap();
        }
        let lote = l.ler_com_imagem_ate(0, 0, 4 * 1024).unwrap();
        let bytes: usize = lote.iter().map(|(_, im)| im.len()).sum();
        assert_eq!(
            lote.len(),
            4,
            "limite zero com teto de 4 KiB devia parar no quarto"
        );
        assert_eq!(bytes, 4 * 1024, "o que veio e o que se alocou");
        // A marca ficou no evento que NAO coube: a chamada seguinte, pedindo
        // a partir dele, continua dali sem pular nem repetir nada.
        assert_eq!(l.marca().map(|m| m.evento), Some(4));
        let seguinte = l.ler_com_imagem_ate(4, 0, 4 * 1024).unwrap();
        assert_eq!(seguinte.len(), 4);
        assert_eq!(seguinte[0].0.rowid, 5);
        assert_eq!(seguinte[3].0.rowid, 8);
        // Sem teto, `limite` zero continua sendo "todos": e o contrato que a
        // CLI, o PITR e os testes desta crate usam, e ele nao muda.
        assert_eq!(l.ler_com_imagem(0, 0).unwrap().len(), 20);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O primeiro evento entra sempre, mesmo maior que o teto: uma linha
    /// gorda atrasa a replicacao em vez de para-la para sempre. E o teto so
    /// conta imagem -- sem imagem nada se aloca, e nada se corta.
    #[test]
    fn o_primeiro_evento_entra_sempre_e_o_teto_so_conta_imagem() {
        let d = dir_temp("teto-primeiro");
        let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        l.registrar_com_imagem(Operacao::Inclusao, 1, 1, &[7u8; 3000])
            .unwrap();
        l.registrar_com_imagem(Operacao::Inclusao, 2, 1, &[8u8; 10])
            .unwrap();
        let lote = l.ler_com_imagem_ate(0, 0, 100).unwrap();
        assert_eq!(lote.len(), 1, "o primeiro entra custe o que custar");
        assert_eq!(lote[0].1.len(), 3000);
        assert_eq!(l.ler_com_imagem_ate(1, 0, 100).unwrap().len(), 1);
        // O limite por EVENTOS continua mandando quando chega antes do teto.
        assert_eq!(l.ler_com_imagem_ate(0, 1, usize::MAX).unwrap().len(), 1);
        // Sem imagem, o mesmo diario inteiro passa por um teto de 1 byte.
        assert_eq!(l.ler(0, 0).unwrap().len(), 2);
        std::fs::remove_dir_all(&d).unwrap();
    }

    // ------------------------------------------- o id de transacao (676)

    /// Um volume da VERSAO 2 montado byte a byte, sem passar pelo
    /// `anexar` de hoje: cabecalho de 64 bytes e eventos de 44, com o CRC
    /// da formula de sempre (bytes 0..36 e o corpo). E o `.log` que um
    /// binario anterior ao 676 deixou no disco.
    fn diario_da_versao_2(d: &std::path::Path, pag: Paginacao, imagens: &[&[u8]]) {
        let mut v = Volumes::novo(d, "t", EXT_LOG, pag);
        v.criar(1).unwrap();
        let mut fim = cofre::CAB_V2 as u64;
        for (i, imagem) in imagens.iter().enumerate() {
            let mut e = [0u8; EVENTO_CAB];
            por_i64(&mut e, 0, 1_700_000_000_000 + i as i64);
            e[8] = 1; // inclusao
            e[9] = u8::from(!imagem.is_empty());
            por_u64(&mut e, 12, i as u64 + 1);
            por_u64(&mut e, 20, 1);
            por_u32(&mut e, 32, imagem.len() as u32);
            let mut crc = crc32(&e[..36]);
            if !imagem.is_empty() {
                crc ^= crc32(imagem);
            }
            por_u32(&mut e, 36, crc);
            v.escrever(1, fim, &e).unwrap();
            v.escrever(1, fim + EVENTO_CAB as u64, imagem).unwrap();
            fim += EVENTO_CAB as u64 + imagem.len() as u64;
        }
        let cab = Cabecalho::novo(1).unwrap().com(fim, imagens.len() as u64);
        assert!(!cab.com_tx && cab.cab_len == cofre::CAB_V2);
        cofre::gravar_cabecalho_no_volume(&mut v, &cab, MAGIC_LOG).unwrap();
    }

    /// **Pedido 676: o `.log` velho continua legivel, e gravavel.** O volume
    /// da versao 2 abre, entrega os eventos com a imagem e `tx` zero, passa
    /// no `verificar`, e o evento novo entra nele com 44 bytes -- um arquivo
    /// append-only nao se reescreve, entao o volume velho nao vira de versao
    /// no meio.
    #[test]
    fn o_diario_da_versao_2_continua_abrindo_e_crescendo_com_44_bytes() {
        let d = dir_temp("v2");
        diario_da_versao_2(&d, Paginacao::DESLIGADA, &[b"Blumenau", b"", b"Joinville"]);
        let tamanho = |d: &std::path::Path| std::fs::metadata(d.join("t.log")).unwrap().len();
        let antes = tamanho(&d);

        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.verificar().unwrap(), 3);
        let lidos = l.ler_com_imagem(0, 0).unwrap();
        assert_eq!(lidos.len(), 3);
        assert_eq!(lidos[0].1, b"Blumenau");
        assert_eq!(lidos[2].1, b"Joinville");
        assert_eq!(lidos[2].0.rowid, 3);
        assert!(
            lidos.iter().all(|(e, _)| e.tx == 0),
            "evento da versao 2 sem id"
        );

        l.registrar_com_imagem(Operacao::Alteracao, 2, 2, b"Itajai")
            .unwrap();
        l.sincronizar().unwrap();
        assert_eq!(
            tamanho(&d) - antes,
            EVENTO_CAB as u64 + 6,
            "o evento novo no volume velho tem de ter 44 bytes"
        );
        drop(l);
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert_eq!(l.verificar().unwrap(), 4);
        let ultimo = l.ler_com_imagem(3, 1).unwrap();
        assert_eq!(ultimo[0].1, b"Itajai");
        assert_eq!(
            ultimo[0].0.tx, 0,
            "o volume velho nao tem onde guardar o id"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O volume que vira a partir de um velho nasce na versao 4: e assim que
    /// uma tabela antiga ganha o id de transacao, sem conversao.
    #[test]
    fn o_volume_seguinte_ao_velho_nasce_na_versao_4() {
        let d = dir_temp("v2-vira");
        let pag = Paginacao::nova(10, 99)
            .unwrap()
            .com_bytes_por_arquivo(200)
            .unwrap();
        diario_da_versao_2(&d, pag, &[b"um", b"dois"]);
        let mut l = LogFile::abrir(&d, "t", pag).unwrap();
        for i in 3..=12u64 {
            l.registrar(Operacao::Inclusao, i, 1).unwrap();
        }
        l.sincronizar().unwrap();
        assert!(l.volumes().len() > 1);
        assert_eq!(l.verificar().unwrap(), 12);
        let todos = l.ler(0, 0).unwrap();
        assert!(todos[..2].iter().all(|e| e.tx == 0));
        assert!(
            todos.last().unwrap().tx > 0,
            "o volume novo tem de levar o id"
        );
        let mut v = Volumes::novo(&d, "t", EXT_LOG, pag);
        let ultimo = *v.existentes().last().unwrap();
        let cab = cofre::ler_cabecalho_do_volume(&mut v, ultimo, MAGIC_LOG, 4).unwrap();
        assert!(cab.com_tx && cab.cab_len == cofre::CAB_V3);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// **Pedido 684 (b): o commit que mistura volume sem id e volume com id
    /// e CONTADO.** A tomada que grava numa tabela ainda no volume 2/3 (tx
    /// zero) e numa ja na 4 chega partida a replica, e ate aqui sem aviso.
    /// A tomada so com volume novo nao conta: o numero e o do defeito.
    #[test]
    fn o_commit_que_mistura_volume_sem_id_e_com_id_e_contado() {
        let d = dir_temp("misto");
        diario_da_versao_2(&d, Paginacao::DESLIGADA, &[b"um"]);
        let mut velho = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        let mut novo = LogFile::criar(&d, "u", Paginacao::DESLIGADA).unwrap();

        let antes = commits_mistos();
        abrir_unidade();
        novo.registrar(Operacao::Inclusao, 1, 1).unwrap();
        novo.registrar(Operacao::Inclusao, 2, 1).unwrap();
        fechar_unidade();
        assert_eq!(commits_mistos(), antes, "tomada so com id nao e mista");

        abrir_unidade();
        let e_novo = novo.registrar(Operacao::Inclusao, 3, 1).unwrap();
        let e_velho = velho.registrar(Operacao::Inclusao, 2, 1).unwrap();
        fechar_unidade();
        assert!(e_novo.tx > 0 && e_velho.tx == 0);
        assert_eq!(
            commits_mistos(),
            antes + 1,
            "a tomada gravou com id e sem id e o commit misto nao foi contado"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// **Pedido 684: o maior id vai ao cabecalho, e o volume novo o herda.**
    /// E o que a abertura le para semear o piso sem caminhar o volume -- e a
    /// virada seguida de queda antes do primeiro evento do volume novo nao
    /// pode deixa-lo dizendo «nenhum id».
    #[test]
    fn o_maior_id_vai_ao_cabecalho_e_o_volume_novo_o_herda() {
        let d = dir_temp("ultimo-tx");
        let pag = Paginacao::nova(10, 99)
            .unwrap()
            .com_bytes_por_arquivo(200)
            .unwrap();
        let mut l = LogFile::criar(&d, "t", pag).unwrap();
        let mut ultimo = 0;
        for i in 1..=12u64 {
            ultimo = l.registrar(Operacao::Inclusao, i, 1).unwrap().tx;
        }
        l.sincronizar().unwrap();
        assert!(l.volumes().len() > 1);
        let mut v = Volumes::novo(&d, "t", EXT_LOG, pag);
        let n = *v.existentes().last().unwrap();
        let cab = cofre::ler_cabecalho_do_volume(&mut v, n, MAGIC_LOG, 4).unwrap();
        assert_eq!(cab.ultimo_tx, ultimo, "o cabecalho nao guardou o maior id");
        // O volume recem-virado, ainda sem evento, ja nasce com o id do
        // anterior.
        let novo = cofre::Cabecalho::novo_do_diario(n + 1)
            .unwrap()
            .com_tx_visto(cab.ultimo_tx);
        assert_eq!(novo.ultimo_tx, ultimo);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O binario ANTERIOR ao 676 le o cabecalho com versao maxima 3: o
    /// volume da versao 4 e recusado nomeando o arquivo e as duas versoes --
    /// e nao com «arquivo truncado», porque o cabecalho da 4 tem sempre 128
    /// bytes e o leitor velho volta com 128 quando ve versao >= 3.
    #[test]
    fn o_leitor_anterior_recusa_a_versao_4_dizendo_qual() {
        let d = dir_temp("v4-velho");
        LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
        let mut v = Volumes::novo(&d, "t", EXT_LOG, Paginacao::DESLIGADA);
        match cofre::ler_cabecalho_do_volume(&mut v, 1, MAGIC_LOG, 3) {
            Err(PhxError::VersaoNaoSuportada {
                arquivo,
                encontrada,
                suportada,
            }) => {
                assert_eq!((encontrada, suportada), (4, 3));
                assert!(arquivo.ends_with("t.log"), "{arquivo}");
            }
            outro => panic!("o leitor da versao 3 tinha de recusar a 4: {outro:?}"),
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// O CRC cobre o id: um `tx` trocado no disco juntaria na replica
    /// eventos de transacoes diferentes, e tem de derrubar a leitura.
    #[test]
    fn o_id_de_transacao_adulterado_falha_no_crc() {
        let d = dir_temp("tx-crc");
        {
            let mut l = LogFile::criar(&d, "t", Paginacao::DESLIGADA).unwrap();
            l.registrar(Operacao::Inclusao, 1, 1).unwrap();
            l.sincronizar().unwrap();
        }
        {
            let mut v = Volumes::novo(&d, "t", EXT_LOG, Paginacao::DESLIGADA);
            v.escrever(1, cofre::CAB_V3 as u64 + OFF_TX as u64, &[7u8; 8])
                .unwrap();
        }
        let mut l = LogFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert!(l.verificar().is_err(), "o tx adulterado passou no CRC");
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Uma unidade, um id -- em quantas tabelas for. Fora dela, cada evento
    /// tem o seu, e os ids so crescem.
    #[test]
    fn a_unidade_da_o_mesmo_id_as_tabelas_e_fora_dela_cada_evento_tem_o_seu() {
        let d = dir_temp("tx-unidade");
        let mut a = LogFile::criar(&d, "a", Paginacao::DESLIGADA).unwrap();
        let mut b = LogFile::criar(&d, "b", Paginacao::DESLIGADA).unwrap();
        let solto = a.registrar(Operacao::Inclusao, 1, 1).unwrap().tx;
        abrir_unidade();
        let x = a.registrar(Operacao::Inclusao, 2, 1).unwrap().tx;
        let y = b.registrar(Operacao::Inclusao, 1, 1).unwrap().tx;
        let z = b.registrar(Operacao::Inclusao, 2, 1).unwrap().tx;
        fechar_unidade();
        let depois = a.registrar(Operacao::Inclusao, 3, 1).unwrap().tx;
        let de_novo = b.registrar(Operacao::Inclusao, 3, 1).unwrap().tx;
        assert!(solto > 0);
        assert_eq!((x, y), (z, z), "a unidade deu ids diferentes");
        assert!(solto < x && x < depois && depois < de_novo);
        // E o que vai ao disco e o que voltou do registro.
        let lidos: Vec<u64> = b.ler(0, 0).unwrap().iter().map(|e| e.tx).collect();
        assert_eq!(lidos, vec![x, x, de_novo]);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
