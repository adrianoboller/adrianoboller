//! O cofre: a chave que cifra o corpo do `.log`, da `.trash` e do `.reason`.
//!
//! A primitiva esta em `phxsql_core::cifra` (ChaCha20-Poly1305, RFC 8439, com
//! todos os vetores oficiais nos testes de la). Este modulo e o que falta entre
//! a primitiva e os tres diarios: onde a chave mora, como ela vira cabecalho de
//! arquivo, e como um registro e selado e aberto.
//!
//! O desenho esta escrito em `docs/SEGURANCA.md` §8, e as decisoes que ele fixa
//! valem aqui como lei.
//!
//! # Pedida, nao imposta
//!
//! A cifra nasce DESLIGADA. Um arquivo escrito antes dela continua abrindo
//! como sempre -- a versao 2 se le igual, e a flag de cifrado e a unica coisa
//! que decide. Ligar e uma decisao do `config.json`, e vale para os volumes
//! criados DAQUI PARA A FRENTE: um `.log` que ja existe em claro continua em
//! claro, porque um arquivo append-only nao se reescreve. E por isso que a
//! resposta certa a "liguei a cifra e o diario velho continua legivel" e
//! "continua mesmo", e nao um conserto.
//!
//! # Por que um global, e nao um parametro
//!
//! E a mesma razao do teto do cache de paginas (`ndx::definir_cache_paginas`):
//! e uma decisao do PROCESSO, tomada uma vez no arranque. Como parametro, a
//! chave teria de atravessar servidor, instancia, database e tabela para
//! chegar em tres arquivos -- e as quatro camadas passariam a carregar um
//! segredo que nao e assunto delas. Camada que carrega segredo e camada que
//! pode vaza-lo.
//!
//! # O que o cofre NAO faz
//!
//! Nao protege contra quem le o `config.json` desta maquina: quem le o
//! `config.json` tem a senha. Protege o ARQUIVO COPIADO -- disco levado,
//! backup vazado, copia numa maquina que nao e esta.

use std::collections::HashMap;
use std::sync::Mutex;

use phxsql_core::cifra::{self, Sequencia, CHAVE_LEN, TAG_LEN, XNONCE_LEN};
use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::frogcript;
use phxsql_core::hash::iguais_em_tempo_constante;

use crate::util::{conferir_magic, por_i64, por_u32, por_u64, Campos};

/// Bytes do cabecalho de arquivo na versao 2 (em claro).
pub const CAB_V2: usize = 64;
/// Bytes do cabecalho de arquivo na versao 3 (com espaco para o sal e a prova).
pub const CAB_V3: usize = 128;
/// Bytes do sal do PBKDF2, gravado em claro no cabecalho.
pub const SAL_LEN: usize = 16;
/// Quanto o registro cresce quando e cifrado: a etiqueta do Poly1305.
pub const ACRESCIMO: usize = TAG_LEN;
/// Iteracoes de PBKDF2 adotadas quando o `config.json` nao diz outra coisa.
pub const ITERACOES_PADRAO: u32 = 210_000;
/// Piso de iteracoes. Abaixo disto o PBKDF2 vira enfeite.
pub const ITERACOES_MINIMAS: u32 = 10_000;

/// Bit 0 do byte de flags do cabecalho: o corpo dos registros vai cifrado.
const FLAG_CIFRADO: u8 = 1;
/// Bit 1: o pedaco cifrado e um pacote FrogCript, e nao um AEAD direto.
///
/// Fica no ARQUIVO, e nao so na configuracao, porque quem abre precisa saber
/// como o pedaco foi selado. Um `config.json` trocado de `frogcript` para
/// `aead` faria o motor tentar abrir como AEAD um pacote que nao e -- e o que
/// sairia seria "a etiqueta nao confere", que manda procurar corrupcao onde
/// so ha um modo trocado.
const FLAG_FROGCRIPT: u8 = 2;

/// Como o pedaco marcado e selado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modo {
    /// XChaCha20-Poly1305 direto. O texto cifrado tem o tamanho do claro, e
    /// so a etiqueta de 16 bytes se acrescenta. **E o padrao.**
    #[default]
    Aead,
    /// O envelope FrogCript: transposicao, duas camadas e a direcao
    /// escondida, por cima do mesmo AEAD.
    ///
    /// **Nao acrescenta forca criptografica** -- a transposicao e uma
    /// permutacao publica e fixa, e duas camadas com a mesma chave nao somam
    /// segredo. Acrescenta o FORMATO do autor, e custa 167 bytes por pedaco.
    /// Ver `docs/SEGURANCA.md` §10.4 e o cabecalho de
    /// `phxsql_core::frogcript`.
    FrogCript,
}

impl Modo {
    pub fn nome(&self) -> &'static str {
        match self {
            Modo::Aead => "aead",
            Modo::FrogCript => "frogcript",
        }
    }

    pub fn de_nome(n: &str) -> Result<Modo> {
        match n.trim().to_ascii_lowercase().as_str() {
            "" | "aead" => Ok(Modo::Aead),
            "frogcript" => Ok(Modo::FrogCript),
            outro => Err(PhxError::Esquema(format!(
                "cifra.modo {outro:?} nao existe: use \"aead\" (padrao) ou \"frogcript\""
            ))),
        }
    }
}

/// O nonce que a prova da chave usa.
///
/// `u64::MAX` de proposito: o numero de ordem de um registro e o OFFSET dele no
/// volume, e nenhum volume chega a 2^64-1 bytes. Assim a prova nunca divide o
/// par (chave, nonce) com registro nenhum.
const ORDEM_DA_PROVA: u64 = u64::MAX;

/// Uma chave de 32 bytes que nao se imprime.
///
/// O `Debug` redigido nao e decoracao: o cabecalho de volume e uma struct que
/// aparece em `{:?}` de diagnostico, e a regra da casa e que segredo nao vai
/// para log nem para resposta. Uma chave que se imprime vaza no dia em que
/// alguem acrescentar um `dbg!`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Chave([u8; CHAVE_LEN]);

impl std::fmt::Debug for Chave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Chave(oculta)")
    }
}

impl Chave {
    fn bytes(&self) -> &[u8; CHAVE_LEN] {
        &self.0
    }
}

/// A senha do cofre, e por quantas iteracoes ela passa.
///
/// Nao deriva `Debug` nem `Clone`: a senha so sai daqui por [`derivar`], que
/// devolve chave e nunca a senha.
struct Segredo {
    senha: String,
    iteracoes: u32,
    modo: Modo,
    ajuste: frogcript::Ajuste,
}

static COFRE: Mutex<Option<Segredo>> = Mutex::new(None);

/// Chaves ja derivadas, por (sal, iteracoes).
///
/// # Por que o cache existe
///
/// O servidor abre e fecha a tabela A CADA PEDIDO. Sem cache, cada pedido
/// pagaria um PBKDF2 de 210.000 iteracoes por volume aberto -- centenas de
/// milissegundos, o que transformaria a cifra numa decisao entre proteger o
/// diario e responder. A chave ja esta na memoria do processo de qualquer
/// jeito, entao guardar a derivacao nao piora o que o processo expoe.
static DERIVADAS: Mutex<Option<Derivadas>> = Mutex::new(None);

/// O que ja foi derivado: a chave de cada par (sal, iteracoes).
type Derivadas = HashMap<([u8; SAL_LEN], u32), Chave>;

fn envenenada() -> PhxError {
    PhxError::Esquema("a trava do cofre ficou envenenada por um panico anterior".into())
}

/// Liga a cifra para os volumes criados daqui em diante.
///
/// Senha vazia e recusada: uma senha vazia derivaria uma chave fixa a partir do
/// sal, o que e o mesmo que nao cifrar com um nome que diz o contrario.
pub fn definir(senha: &str, iteracoes: u32) -> Result<()> {
    definir_com(senha, iteracoes, Modo::Aead, frogcript::Ajuste::default())
}

/// Liga a cifra escolhendo o modo e o ajuste do FrogCript.
pub fn definir_com(
    senha: &str,
    iteracoes: u32,
    modo: Modo,
    ajuste: frogcript::Ajuste,
) -> Result<()> {
    if senha.is_empty() {
        return Err(PhxError::Esquema(
            "cifra ligada sem senha: preencha \"cifra.senha\" ou \"cifra.senha_env\"".into(),
        ));
    }
    if iteracoes < ITERACOES_MINIMAS {
        return Err(PhxError::Esquema(format!(
            "cifra.iteracoes {iteracoes} e baixo demais (minimo {ITERACOES_MINIMAS})"
        )));
    }
    *COFRE.lock().map_err(|_| envenenada())? = Some(Segredo {
        senha: senha.to_string(),
        iteracoes,
        modo,
        ajuste,
    });
    // O cache de chaves derivadas e por (sal, iteracoes) -- NAO por senha.
    // Deixando-o de pe, trocar a senha nao trocaria a chave de nenhum arquivo
    // ja aberto neste processo: `derivar` acharia a entrada do sal e
    // devolveria a chave da senha ANTIGA. Um servidor que aceitasse a senha
    // errada por ter aberto o arquivo antes seria pior que um que a recusa.
    if let Ok(mut d) = DERIVADAS.lock() {
        *d = None;
    }
    Ok(())
}

/// Desliga a cifra. Volumes ja cifrados deixam de abrir ate ela voltar.
pub fn desligar() {
    if let Ok(mut c) = COFRE.lock() {
        *c = None;
    }
    if let Ok(mut d) = DERIVADAS.lock() {
        *d = None;
    }
}

/// A cifra esta ligada neste processo?
pub fn ligado() -> bool {
    COFRE.lock().map(|c| c.is_some()).unwrap_or(false)
}

/// O modo e o ajuste vigentes, para carimbar um arquivo novo.
fn modo_vigente() -> (Modo, frogcript::Ajuste) {
    COFRE
        .lock()
        .ok()
        .and_then(|c| c.as_ref().map(|s| (s.modo, s.ajuste)))
        .unwrap_or_default()
}

/// As iteracoes vigentes, para carimbar um volume novo.
fn iteracoes_vigentes() -> Result<u32> {
    Ok(COFRE
        .lock()
        .map_err(|_| envenenada())?
        .as_ref()
        .map(|s| s.iteracoes)
        .unwrap_or(ITERACOES_PADRAO))
}

/// Deriva a chave deste sal, ou devolve a que ja foi derivada.
///
/// O erro de "nao ha chave" e explicito de proposito: um arquivo cifrado aberto
/// sem chave tem de dizer o que fazer, e nao devolver lixo.
pub fn derivar(sal: &[u8; SAL_LEN], iteracoes: u32, arquivo: &str) -> Result<Chave> {
    if let Some(c) = DERIVADAS
        .lock()
        .map_err(|_| envenenada())?
        .as_ref()
        .and_then(|m| m.get(&(*sal, iteracoes)).copied())
    {
        return Ok(c);
    }
    let chave = {
        let cofre = COFRE.lock().map_err(|_| envenenada())?;
        let Some(s) = cofre.as_ref() else {
            return Err(PhxError::Autorizacao(format!(
                "{arquivo} esta cifrado e este servidor nao tem a chave: \
                 preencha \"cifra\" no config.json com a mesma senha que gravou o arquivo"
            )));
        };
        Chave(cifra::chave_de_senha(&s.senha, sal, iteracoes))
    };
    DERIVADAS
        .lock()
        .map_err(|_| envenenada())?
        .get_or_insert_with(HashMap::new)
        .insert((*sal, iteracoes), chave);
    Ok(chave)
}

// ---------------------------------------------------------------------------
// O material de cifra, comum a TODO arquivo cifrado
// ---------------------------------------------------------------------------

/// Bytes que o material de cifra ocupa no cabecalho de um arquivo.
///
/// ```text
/// +0   flags     u8    bit 0: o conteudo vai cifrado
/// +4   iteracoes u32   do PBKDF2 que derivou a chave deste arquivo
/// +8   sal       16    sorteado por arquivo; nao e segredo
/// +24  prova     16    a etiqueta que diz se a senha e a certa
/// ```
pub const MATERIAL_LEN: usize = 40;

/// O material de cifra de um arquivo, ja interpretado.
///
/// # Por que um tipo, e nao tres copias do mesmo trecho
///
/// Porque ja sao cinco arquivos: os tres diarios trouxeram este desenho, e o
/// `.reg`, o `.ndx`, o `.memo` e o `.bin` chegaram depois. Repetir os offsets
/// do sal em sete lugares e repetir sete vezes a chance de errar um -- e um
/// offset errado aqui nao da erro, da arquivo que abre com a chave de outro.
#[derive(Debug, Clone, Copy)]
pub struct Material {
    sal: [u8; SAL_LEN],
    iteracoes: u32,
    chave: Option<Chave>,
    modo: Modo,
    /// So vale no modo FrogCript. Vem da configuracao e **nao vai ao
    /// arquivo**: e o autor quem pediu que salto e separador personalizados
    /// fossem tratados como parte do segredo, e segredo nao se grava ao lado
    /// do dado que ele protege.
    ajuste: frogcript::Ajuste,
}

impl Material {
    /// Nenhuma cifra. E o que todo arquivo em claro carrega.
    pub const EM_CLARO: Material = Material {
        sal: [0u8; SAL_LEN],
        iteracoes: 0,
        chave: None,
        modo: Modo::Aead,
        ajuste: frogcript::Ajuste::PADRAO,
    };

    /// O material de um arquivo NOVO: cifrado se o cofre estiver ligado.
    ///
    /// Cada arquivo sorteia o proprio sal, e por isso tem a propria chave. E
    /// o que deixa o numero de ordem do nonce ser um contador local -- o
    /// offset, o rowid, o numero da pagina: dois arquivos com o mesmo
    /// contador tem chaves diferentes.
    pub fn novo() -> Result<Material> {
        if !ligado() {
            return Ok(Material::EM_CLARO);
        }
        let mut sal = [0u8; SAL_LEN];
        sal.copy_from_slice(&phxsql_core::senha::bytes_aleatorios(SAL_LEN));
        let iteracoes = iteracoes_vigentes()?;
        let chave = derivar(&sal, iteracoes, "<arquivo novo>")?;
        let (modo, ajuste) = modo_vigente();
        Ok(Material {
            sal,
            iteracoes,
            chave: Some(chave),
            modo,
            ajuste,
        })
    }

    /// O modo com que este arquivo foi selado.
    pub fn modo(&self) -> Modo {
        self.modo
    }

    /// O texto cifrado cabe no lugar do claro?
    ///
    /// Sim no AEAD, porque o ChaCha20 e cifra de fluxo e nao muda o tamanho.
    /// Nao no FrogCript, que devolve um pacote com quatro nonces e quatro
    /// etiquetas dentro -- e por isso, nesse modo, a faixa marcada do payload
    /// vai a ZEROS e o pacote inteiro mora no rabo do slot.
    pub fn no_lugar(&self) -> bool {
        !self.cifrado() || self.modo == Modo::Aead
    }

    /// Quantos bytes o slot precisa DEPOIS do payload, dado quanto dele e
    /// marcado.
    pub fn rabo(&self, marcado: usize) -> usize {
        if !self.cifrado() || marcado == 0 {
            return 0;
        }
        match self.modo {
            Modo::Aead => TAG_LEN,
            Modo::FrogCript => self.ocupa(marcado),
        }
    }

    pub fn cifrado(&self) -> bool {
        self.chave.is_some()
    }

    /// Quanto um pedaco de `n` bytes ocupa no disco depois de cifrado.
    ///
    /// No modo AEAD sao 16 bytes de etiqueta. No FrogCript sao 167 -- quatro
    /// nonces, quatro etiquetas, as duas pontas de direcao, o comprimento e o
    /// separador -- e o texto cifrado deixa de caber no lugar do claro, ver
    /// `reg.rs`.
    pub fn ocupa(&self, n: usize) -> usize {
        if n == 0 || !self.cifrado() {
            return n;
        }
        match self.modo {
            Modo::Aead => n + TAG_LEN,
            Modo::FrogCript => n + frogcript::ACRESCIMO + 4 * XNONCE_LEN,
        }
    }

    /// Grava flag, iteracoes, sal e prova em `buf`, a partir de `base`.
    ///
    /// `rotulo` e a parte ESTAVEL do cabecalho que a prova amarra -- magic,
    /// versao, tamanho de slot. Fica de fora o que muda a cada gravacao: uma
    /// prova que mudasse com os contadores teria de ser refeita e reconferida
    /// toda vez sem proteger nada a mais.
    pub fn gravar(&self, buf: &mut [u8], base: usize, rotulo: &[u8]) {
        let Some(chave) = self.chave else { return };
        buf[base] = FLAG_CIFRADO
            | if self.modo == Modo::FrogCript {
                FLAG_FROGCRIPT
            } else {
                0
            };
        por_u32(buf, base + 4, self.iteracoes);
        buf[base + 8..base + 8 + SAL_LEN].copy_from_slice(&self.sal);
        let prova = prova_do_material(&chave, rotulo, &buf[base..base + 24]);
        buf[base + 24..base + 24 + TAG_LEN].copy_from_slice(&prova);
    }

    /// Le o material de `buf` a partir de `base`, derivando a chave.
    ///
    /// Um arquivo sem a flag devolve [`Material::EM_CLARO`] sem tocar no
    /// cofre: e o caminho de todo arquivo escrito antes desta versao, e ele
    /// nao pode nem perguntar se ha chave.
    pub fn ler(buf: &[u8], base: usize, nome: &str, rotulo: &[u8]) -> Result<Material> {
        if buf.len() < base + MATERIAL_LEN || buf[base] & FLAG_CIFRADO == 0 {
            return Ok(Material::EM_CLARO);
        }
        let iteracoes = Campos(buf).u32(base + 4);
        if iteracoes < ITERACOES_MINIMAS {
            return Err(PhxError::Corrompido(format!(
                "{nome}: o cabecalho declara {iteracoes} iteracoes de PBKDF2, abaixo do piso"
            )));
        }
        let mut sal = [0u8; SAL_LEN];
        sal.copy_from_slice(&buf[base + 8..base + 8 + SAL_LEN]);
        let chave = derivar(&sal, iteracoes, nome)?;

        let esperada = prova_do_material(&chave, rotulo, &buf[base..base + 24]);
        if !iguais_em_tempo_constante(&esperada, &buf[base + 24..base + 24 + TAG_LEN]) {
            return Err(PhxError::Autorizacao(format!(
                "{nome}: a senha de \"cifra\" nao e a que gravou este arquivo"
            )));
        }
        let (_, ajuste) = modo_vigente();
        Ok(Material {
            sal,
            iteracoes,
            chave: Some(chave),
            // O MODO sai do arquivo, e nao da configuracao: e assim que um
            // `config.json` trocado depois nao transforma "modo errado" em
            // "etiqueta nao confere".
            modo: if buf[base] & FLAG_FROGCRIPT != 0 {
                Modo::FrogCript
            } else {
                Modo::Aead
            },
            ajuste,
        })
    }

    /// Cifra um pedaco com nonce de 24 bytes. Sem chave, devolve o proprio.
    ///
    /// # Por que o nonce estendido aqui, e o de 96 bits nos diarios
    ///
    /// Porque nos diarios o numero de ordem e o offset, que num arquivo que so
    /// cresce nunca se repete. Um slot do `.reg` e uma pagina do `.ndx` sao
    /// reescritos NO MESMO LUGAR, e ali o endereco nao serve de contador. Com
    /// 192 bits sobra espaco para o endereco E para bytes sorteados, e e a
    /// soma dos dois que fecha a porta.
    pub fn selar(&self, nonce: &[u8; XNONCE_LEN], aad: &[u8], claro: &[u8]) -> Vec<u8> {
        let Some(chave) = self.chave else {
            return claro.to_vec();
        };
        if claro.is_empty() {
            return Vec::new();
        }
        if self.modo == Modo::FrogCript {
            // O nonce e o dado associado nao entram: o pacote FrogCript sorteia
            // um nonce por camada, e a estrutura dele nao tem onde pendurar
            // AAD. E uma das coisas que o modo CUSTA, e esta no documento.
            return frogcript::cifrar_bytes(
                chave.bytes(),
                claro,
                frogcript::Direcao::Direta,
                self.ajuste,
            );
        }
        let (mut corpo, tag) = cifra::xselar(chave.bytes(), nonce, aad, claro);
        corpo.extend_from_slice(&tag);
        corpo
    }

    /// Abre um pedaco cifrado. Sem chave, devolve o proprio.
    pub fn abrir(
        &self,
        nonce: &[u8; XNONCE_LEN],
        aad: &[u8],
        guardado: &[u8],
        arquivo: &str,
    ) -> Result<Vec<u8>> {
        let Some(chave) = self.chave else {
            return Ok(guardado.to_vec());
        };
        if guardado.is_empty() {
            return Ok(Vec::new());
        }
        if self.modo == Modo::FrogCript {
            return frogcript::decifrar_bytes(chave.bytes(), guardado, self.ajuste)
                .map(|(claro, _)| claro)
                .map_err(|e| {
                    PhxError::Corrompido(format!(
                        "{arquivo}: o pacote FrogCript nao abriu -- ou o dado foi \
                         alterado, ou a chave, o salto ou o separador de \"cifra\" \
                         nao sao os que gravaram este arquivo ({e})"
                    ))
                });
        }
        if guardado.len() < TAG_LEN {
            return Err(PhxError::Corrompido(format!(
                "{arquivo}: pedaco cifrado sem a etiqueta de {TAG_LEN} bytes"
            )));
        }
        let corte = guardado.len() - TAG_LEN;
        let mut tag = [0u8; TAG_LEN];
        tag.copy_from_slice(&guardado[corte..]);
        cifra::xabrir(chave.bytes(), nonce, aad, &guardado[..corte], &tag).map_err(|_| {
            PhxError::Corrompido(format!(
                "{arquivo}: a etiqueta nao confere -- ou o dado foi alterado, ou a \
                 chave de \"cifra\" nao e a que gravou este arquivo"
            ))
        })
    }
}

/// A prova de que a chave e a certa, sem decifrar conteudo nenhum.
///
/// Mesmo papel da prova dos diarios: sem ela, uma senha errada no
/// `config.json` so apareceria na primeira leitura de conteudo -- que numa
/// tabela recem-criada seria nunca. Quem digitou errado descobriria com a
/// tabela ja cheia de linhas gravadas com a chave errada.
fn prova_do_material(chave: &Chave, rotulo: &[u8], cabecalho: &[u8]) -> [u8; TAG_LEN] {
    let mut aad = Vec::with_capacity(rotulo.len() + cabecalho.len());
    aad.extend_from_slice(rotulo);
    aad.extend_from_slice(cabecalho);
    let (_, tag) = cifra::xselar(chave.bytes(), &[0u8; XNONCE_LEN], &aad, &[]);
    tag
}

/// O nonce de um pedaco endereçavel: onde ele mora, quantas vezes foi
/// reescrito, e oito bytes sorteados nesta gravacao.
///
/// # Por que os tres, e nao um so
///
/// - **Onde mora** (volume + rowid, ou o numero da pagina) separa dois
///   pedacos diferentes do mesmo arquivo.
/// - **Quantas vezes** separa duas gravacoes do MESMO pedaco. E o contador
///   que o formato ja tem: a versao da linha, o contador da pagina.
/// - **Sorteado** e o que sobra de pe quando o contador volta. Uma gravacao
///   perdida no cache do sistema antes do `fsync` faz a proxima repetir o
///   contador -- e ai duas imagens diferentes teriam o mesmo par
///   (chave, nonce), que e a unica falha que quebra isto sem quebrar a
///   matematica. Com 64 bits sorteados, repetir exige colisao de aniversario.
pub fn nonce_de_pedaco(onde: u64, quem: u32, contador: u32, tempero: u64) -> [u8; XNONCE_LEN] {
    let mut n = [0u8; XNONCE_LEN];
    n[0..8].copy_from_slice(&onde.to_le_bytes());
    n[8..12].copy_from_slice(&quem.to_le_bytes());
    n[12..16].copy_from_slice(&contador.to_le_bytes());
    n[16..24].copy_from_slice(&tempero.to_le_bytes());
    n
}

// ---------------------------------------------------------------------------
// O cabecalho de volume, comum aos tres diarios
// ---------------------------------------------------------------------------

/// O cabecalho de um volume de diario, ja interpretado.
///
/// Os tres arquivos tem o mesmo desenho de cabecalho -- e desde que a versao 3
/// carrega sal e prova, repetir o codigo em tres lugares seria repetir tres
/// vezes a chance de errar um offset.
#[derive(Debug, Clone, Copy)]
pub struct Cabecalho {
    pub volume: u32,
    /// Onde o proximo registro entra.
    pub fim: u64,
    /// Quantos registros este volume tem.
    pub quantos: u64,
    /// 64 na versao 2, 128 na versao 3.
    pub cab_len: usize,
    /// O sal do PBKDF2 deste volume. Zerado quando ele nao e cifrado.
    sal: [u8; SAL_LEN],
    iteracoes: u32,
    /// A chave deste volume, quando ele e cifrado.
    pub chave: Option<Chave>,
}

impl Cabecalho {
    /// Um volume novo, com o material de cifra que o cofre mandar agora.
    ///
    /// Cada volume sorteia o PROPRIO sal, e por isso cada volume tem a propria
    /// chave. E o que faz o numero de ordem do nonce poder ser o offset dentro
    /// do volume: dois volumes com o mesmo offset tem chaves diferentes.
    pub fn novo(volume: u32) -> Result<Cabecalho> {
        if !ligado() {
            return Ok(Cabecalho {
                volume,
                fim: CAB_V2 as u64,
                quantos: 0,
                cab_len: CAB_V2,
                sal: [0u8; SAL_LEN],
                iteracoes: 0,
                chave: None,
            });
        }
        let mut sal = [0u8; SAL_LEN];
        sal.copy_from_slice(&phxsql_core::senha::bytes_aleatorios(SAL_LEN));
        let iteracoes = iteracoes_vigentes()?;
        let chave = derivar(&sal, iteracoes, "<volume novo>")?;
        Ok(Cabecalho {
            volume,
            fim: CAB_V3 as u64,
            quantos: 0,
            cab_len: CAB_V3,
            sal,
            iteracoes,
            chave: Some(chave),
        })
    }

    /// Um cabecalho com os mesmos parametros de cifra, e outros contadores.
    pub fn com(&self, fim: u64, quantos: u64) -> Cabecalho {
        Cabecalho {
            fim,
            quantos,
            ..*self
        }
    }

    pub fn cifrado(&self) -> bool {
        self.chave.is_some()
    }

    /// O tamanho que este registro ocupa no arquivo, dado o tamanho em claro.
    pub fn ocupa(&self, corpo_em_claro: usize) -> usize {
        if corpo_em_claro == 0 || !self.cifrado() {
            corpo_em_claro
        } else {
            corpo_em_claro + ACRESCIMO
        }
    }

    /// Cifra o corpo de um registro. Sem chave, devolve o proprio corpo.
    ///
    /// `tempero` sao quatro bytes que so este registro tem, e `ordem` e o
    /// offset dele no volume. Ver [`nonce_de`].
    pub fn selar(&self, tempero: [u8; 4], ordem: u64, aad: &[u8], claro: &[u8]) -> Vec<u8> {
        let Some(chave) = self.chave else {
            return claro.to_vec();
        };
        if claro.is_empty() {
            return Vec::new();
        }
        let (mut corpo, tag) = cifra::selar(chave.bytes(), &nonce_de(tempero, ordem), aad, claro);
        corpo.extend_from_slice(&tag);
        corpo
    }

    /// Abre o corpo de um registro. Sem chave, devolve o proprio corpo.
    pub fn abrir(
        &self,
        tempero: [u8; 4],
        ordem: u64,
        aad: &[u8],
        guardado: &[u8],
        arquivo: &str,
    ) -> Result<Vec<u8>> {
        let Some(chave) = self.chave else {
            return Ok(guardado.to_vec());
        };
        if guardado.is_empty() {
            return Ok(Vec::new());
        }
        if guardado.len() < ACRESCIMO {
            return Err(PhxError::Corrompido(format!(
                "{arquivo}: registro cifrado sem a etiqueta de {ACRESCIMO} bytes"
            )));
        }
        let corte = guardado.len() - ACRESCIMO;
        let mut tag = [0u8; TAG_LEN];
        tag.copy_from_slice(&guardado[corte..]);
        cifra::abrir(
            chave.bytes(),
            &nonce_de(tempero, ordem),
            aad,
            &guardado[..corte],
            &tag,
        )
        .map_err(|_| {
            PhxError::Corrompido(format!(
                "{arquivo}: a etiqueta do registro nao confere -- ou o dado foi \
                 alterado, ou a chave de \"cifra\" nao e a que gravou este arquivo"
            ))
        })
    }
}

/// O nonce de um registro: quatro bytes so dele, mais o offset no volume.
///
/// # Por que o offset, e nao um contador
///
/// Repetir o par (chave, nonce) e o unico jeito de quebrar isto sem quebrar a
/// matematica. O offset e o contador que o arquivo JA TEM e que nunca se
/// reaproveita: num arquivo que so cresce, dois registros nao comecam no mesmo
/// lugar. Um contador a parte teria de ser persistido, e persistido a cada
/// registro -- que e exatamente a escrita que o `.log` tirou do caminho.
///
/// # E o unico caso em que o offset se repetiria
///
/// Uma queda no meio da escrita deixa um rabo estragado; a cura corta esse
/// rabo e o proximo registro entra no mesmo offset. Os quatro bytes de
/// `tempero` fecham essa porta: no `.log` eles sao sorteados por evento, e na
/// `.trash` e no `.reason` sao os quatro ultimos bytes do UUID v7 do registro,
/// que o formato ja promete unico.
pub fn nonce_de(tempero: [u8; 4], ordem: u64) -> [u8; cifra::NONCE_LEN] {
    Sequencia::nova(tempero).nonce(ordem)
}

/// Le o cabecalho de volume a partir dos bytes crus.
///
/// `bruto` precisa ter ao menos [`CAB_V2`] bytes; quando a versao for 3, ele
/// precisa ter [`CAB_V3`]. Quem chama le 64 primeiro e volta com 128 se for o
/// caso -- um `.log` recem-criado na versao 2 tem exatamente 64 bytes, e pedir
/// 128 de cara faria a leitura falhar no fim do arquivo.
pub fn ler_cabecalho(
    bruto: &[u8],
    magic: &'static [u8; 8],
    nome: &str,
    versao_maxima: u16,
) -> Result<Cabecalho> {
    conferir_magic(nome, magic, &bruto[0..8])?;
    let c = Campos(bruto);
    let versao = c.u16(8);
    if versao == 0 || versao > versao_maxima {
        return Err(PhxError::VersaoNaoSuportada {
            arquivo: nome.to_string(),
            encontrada: versao,
            suportada: versao_maxima,
        });
    }
    let cab_len = c.u16(10) as usize;
    let esperado = if versao >= 3 { CAB_V3 } else { CAB_V2 };
    if cab_len != esperado {
        return Err(PhxError::Corrompido(format!(
            "{nome}: cabecalho da versao {versao} diz ter {cab_len} bytes, e sao {esperado}"
        )));
    }
    if bruto.len() < cab_len {
        return Err(PhxError::Corrompido(format!(
            "{nome}: cabecalho de {cab_len} bytes truncado em {}",
            bruto.len()
        )));
    }
    let off_crc = cab_len - 8;
    if crc32(&bruto[..off_crc]) != c.u32(off_crc) {
        return Err(PhxError::Corrompido(format!(
            "cabecalho de {nome} com CRC invalido"
        )));
    }

    let mut cab = Cabecalho {
        volume: c.u32(12),
        fim: c.u64(24),
        quantos: c.u64(16),
        cab_len,
        sal: [0u8; SAL_LEN],
        iteracoes: 0,
        chave: None,
    };
    if versao >= 3 && bruto[40] & FLAG_CIFRADO != 0 {
        cab.sal.copy_from_slice(&bruto[48..48 + SAL_LEN]);
        cab.iteracoes = c.u32(44);
        if cab.iteracoes < ITERACOES_MINIMAS {
            return Err(PhxError::Corrompido(format!(
                "{nome}: o cabecalho declara {} iteracoes de PBKDF2, abaixo do piso",
                cab.iteracoes
            )));
        }
        let chave = derivar(&cab.sal, cab.iteracoes, nome)?;
        conferir_prova(&chave, bruto, cab_len, nome)?;
        cab.chave = Some(chave);
    }
    Ok(cab)
}

/// Escreve o cabecalho de volume. Devolve 64 ou 128 bytes, conforme a versao.
pub fn gravar_cabecalho(cab: &Cabecalho, magic: &[u8; 8]) -> Vec<u8> {
    let mut buf = vec![0u8; cab.cab_len];
    buf[0..8].copy_from_slice(magic);
    let versao: u16 = if cab.cifrado() { 3 } else { 2 };
    buf[8..10].copy_from_slice(&versao.to_le_bytes());
    buf[10..12].copy_from_slice(&(cab.cab_len as u16).to_le_bytes());
    por_u32(&mut buf, 12, cab.volume);
    por_u64(&mut buf, 16, cab.quantos);
    por_u64(&mut buf, 24, cab.fim);
    por_i64(&mut buf, 32, crate::util::agora());
    if let Some(chave) = cab.chave {
        buf[40] = FLAG_CIFRADO;
        por_u32(&mut buf, 44, cab.iteracoes);
        buf[48..48 + SAL_LEN].copy_from_slice(&cab.sal);
        let prova = prova_da_chave(&chave, &buf);
        buf[64..64 + TAG_LEN].copy_from_slice(&prova);
    }
    let off_crc = cab.cab_len - 8;
    let crc = crc32(&buf[..off_crc]);
    por_u32(&mut buf, off_crc, crc);
    buf
}

/// A etiqueta que prova que a chave e a certa, sem decifrar registro nenhum.
///
/// # Por que ela existe
///
/// Sem prova, uma senha errada so apareceria na PRIMEIRA leitura de corpo --
/// e num arquivo sem registro nenhum, nunca. Quem digitou a senha errada no
/// `config.json` descobriria dias depois, com o diario ja cheio de eventos
/// gravados com a chave errada e os antigos ilegiveis.
///
/// O dado associado e so a parte ESTAVEL do cabecalho: os contadores mudam a
/// cada gravacao, e uma prova que mudasse com eles teria de ser recalculada
/// (e reconferida) toda vez sem proteger nada a mais.
fn prova_da_chave(chave: &Chave, buf: &[u8]) -> [u8; TAG_LEN] {
    let mut aad = Vec::with_capacity(16 + 24);
    aad.extend_from_slice(&buf[0..16]);
    aad.extend_from_slice(&buf[40..64]);
    let (_, tag) = cifra::selar(
        chave.bytes(),
        &nonce_de([0, 0, 0, 0], ORDEM_DA_PROVA),
        &aad,
        &[],
    );
    tag
}

fn conferir_prova(chave: &Chave, buf: &[u8], cab_len: usize, nome: &str) -> Result<()> {
    debug_assert_eq!(cab_len, CAB_V3);
    let esperada = prova_da_chave(chave, buf);
    if esperada[..] != buf[64..64 + TAG_LEN] {
        return Err(PhxError::Autorizacao(format!(
            "{nome}: a senha de \"cifra\" nao e a que gravou este arquivo"
        )));
    }
    Ok(())
}

/// Le o cabecalho do volume, seja ele da versao 2 ou da 3.
///
/// Le 64 bytes primeiro e volta com 128 quando a versao pede: um volume da
/// versao 2 recem-criado tem EXATAMENTE 64 bytes, e pedir 128 de cara faria a
/// leitura falhar no fim do arquivo -- que e o comportamento velho quebrando
/// por causa do formato novo.
pub fn ler_cabecalho_do_volume(
    volumes: &mut crate::volume::Volumes,
    volume: u32,
    magic: &'static [u8; 8],
) -> Result<Cabecalho> {
    let nome = volumes.caminho(volume).display().to_string();
    let mut buf = vec![0u8; CAB_V2];
    volumes.ler(volume, 0, &mut buf)?;
    if u16::from_le_bytes([buf[8], buf[9]]) >= 3 {
        buf.resize(CAB_V3, 0);
        volumes.ler(volume, 0, &mut buf)?;
    }
    ler_cabecalho(&buf, magic, &nome, 3)
}

/// Grava o cabecalho do volume no proprio volume.
pub fn gravar_cabecalho_no_volume(
    volumes: &mut crate::volume::Volumes,
    cab: &Cabecalho,
    magic: &'static [u8; 8],
) -> Result<()> {
    let buf = gravar_cabecalho(cab, magic);
    volumes.escrever(cab.volume, 0, &buf)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O unico teste que cabe AQUI: ele nao toca no cofre global.
    ///
    /// Todo o resto -- ligar a cifra, gravar um cabecalho da versao 3, provar
    /// que a chave errada e recusada -- vive em `tests/cifra-dos-diarios.rs`,
    /// e a razao e o global. `cargo test` roda os testes do mesmo binario em
    /// paralelo: um teste que liga a cifra aqui dentro faria o `.log` de outro
    /// teste nascer cifrado no meio da corrida. Um arquivo de teste de
    /// integracao roda em OUTRO processo, e ali o global e so dele.
    ///
    /// E o que a `Sequencia` existe para garantir, provado em numeros.
    #[test]
    fn o_nonce_nunca_se_repete_dentro_do_volume() {
        let mut vistos = std::collections::HashSet::new();
        // Offsets como os de um diario real: cabecalho, e um registro atras do
        // outro, sem nunca voltar.
        let mut offset = CAB_V3 as u64;
        for i in 0..20_000u64 {
            let tempero = [(i >> 24) as u8, (i >> 16) as u8, (i >> 8) as u8, i as u8];
            assert!(
                vistos.insert(nonce_de(tempero, offset)),
                "nonce repetido no offset {offset}"
            );
            offset += 44 + (i % 97);
        }
        // E o caso que a queda cria: o MESMO offset outra vez. So o tempero
        // muda -- e e ele que segura o nonce diferente.
        assert_ne!(nonce_de([1, 2, 3, 4], 4096), nonce_de([9, 9, 9, 9], 4096));
    }
}
