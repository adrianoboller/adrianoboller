//! `.ndx` -- os indices da tabela, em B+tree, todos no mesmo arquivo.
//!
//! # Por que a arvore nao conhece tipos
//!
//! As chaves chegam aqui ja codificadas por `phxsql_core::keyenc`, que preserva
//! ordem: comparar bytes com `memcmp` da o mesmo resultado que comparar os
//! valores logicos. A B+tree entao so compara bytes, e o mesmo codigo serve
//! para inteiro, data, decimal, texto, ASC, DESC e NOCASE.
//!
//! # Chave completa
//!
//! Cada entrada de folha guarda a "chave completa" = chave do usuario seguida
//! do rowid em big-endian:
//!
//! ```text
//! [chave codificada: key_len bytes][rowid: 8 bytes BE]
//! ```
//!
//! Como o rowid entra no fim e em BE, toda chave completa e unica e a
//! comparacao byte a byte tambem desempata por rowid. Indices duplicados
//! saem de graca, e o indice unico e imposto por uma consulta de prefixo
//! antes de inserir.
//!
//! # Paginas
//!
//! ```text
//! pagina 0      cabecalho (128 bytes) + diretorio de indices
//! pagina n>0    no da arvore
//!
//! cabecalho de pagina (32 bytes):
//!   [tipo u8][flags u8][qtd u16][proxima_folha u64]
//!   [pagina_anterior u64][filho_direita u64][crc32 u32]
//!
//! folha:   entrada = chave completa            (ck_len bytes)
//! interno: entrada = chave completa + filho    (ck_len + 8 bytes)
//! ```
//!
//! Num no interno, o filho da entrada `i` guarda as chaves MENORES que a
//! chave da entrada `i`; `filho_direita` guarda as maiores ou iguais a
//! ultima chave.
//!
//! # Remocao
//!
//! Remover tira a entrada da folha sem rebalancear a arvore. A busca continua
//! correta (folhas vazias apenas nao produzem resultado), mas paginas podem
//! ficar subocupadas depois de muitas exclusoes. A reconstrucao do indice
//! (feita pela compactacao da tabela) devolve a arvore ao formato compacto.

use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

use phxsql_core::cifra;
use phxsql_core::crc::{crc32, crc32_with};
use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::Schema;
use phxsql_core::RowId;

use crate::cofre;
use crate::util::{
    agora, conferir_magic, escrever_em, ler_exato, por_i64, por_u16, por_u32, por_u64, Campos,
};

pub const MAGIC_NDX: &[u8; 8] = b"PHXNDX\0\0";
const CAB_LEN: usize = 128;
const PAG_CAB: usize = 32;
const VERSAO: u16 = 1;
/// Versao do arquivo cuja PAGINA vai selada -- ver [`NdxFile::criar_selado`].
///
/// # Por que uma versao NOVA, e nao uma flag na 1
///
/// Pela mesma razao do `.reg` (`VERSAO_CIFRADO`): a pagina muda de tamanho
/// util, e um leitor da versao 1 que abrisse este arquivo leria a etiqueta
/// como entrada da folha e responderia busca com lixo. A versao faz ele
/// RECUSAR, que e a unica resposta honesta.
const VERSAO_SELADA: u16 = 2;
/// Onde o material de cifra mora no cabecalho, nos bytes que ja eram
/// reservados (53..124). O cabecalho **nao cresce**.
const MATERIAL_EM: usize = 56;
/// Bytes sorteados a cada gravacao de pagina, guardados nela.
///
/// Sao o que segura o par (chave, nonce) diferente quando a MESMA pagina e
/// reescrita: o endereco da pagina sozinho se repetiria em toda gravacao, e
/// nonce repetido com a mesma chave e a unica falha que quebra a cifra de
/// fluxo sem quebrar a matematica dela. E a mesma conta do slot do `.reg`.
const TEMPERO_LEN: usize = 8;
pub const PAGINA_PADRAO: usize = 4096;

#[allow(dead_code)]
const TIPO_LIVRE: u8 = 0;
const TIPO_FOLHA: u8 = 1;
const TIPO_INTERNO: u8 = 2;

/// Minimo de entradas por pagina para que a divisao funcione.
const MIN_ENTRADAS: usize = 4;

/// Tamanho do rowid anexado a toda chave.
pub const ROWID_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescritorIndice {
    pub nome: String,
    pub unico: bool,
    /// Bytes da chave codificada do usuario, sem o rowid.
    pub key_len: usize,
    pub raiz: u64,
    pub qtd_chaves: u64,
}

impl DescritorIndice {
    /// Bytes da chave completa (chave do usuario + rowid).
    pub fn ck_len(&self) -> usize {
        self.key_len + ROWID_LEN
    }
}

/// Quantas paginas ficam em RAM por arquivo `.ndx` aberto.
///
/// 2.048 paginas de 4 KiB dao 8 MiB por tabela aberta. O numero saiu de uma
/// varredura de quatro tamanhos (`--example ordem-da-chave`, e a tabela esta em
/// `docs/DESEMPENHO.md` §2.1): 2.048 e o joelho -- dobrar de novo compra 0,8 us
/// por linha e custa mais 8 MiB.
///
/// O servidor abre e fecha a tabela a cada operacao, entao o teto vale enquanto
/// a operacao dura -- e a operacao que importa aqui e a carga em lote, que
/// insere milhares de linhas dentro de uma unica abertura.
const PAGINAS_PADRAO: usize = 2048;

/// O teto vigente, que o `config.json` ajusta em `recursos.cache_paginas`.
///
/// # Por que um global, e nao um parametro
///
/// E um teto de RAM do PROCESSO, escolhido uma vez no arranque e nunca por
/// tabela. Como parametro, ele teria de atravessar quatro camadas de API --
/// servidor, instancia, database, tabela -- so para chegar aqui, e todas as
/// quatro passariam a carregar um numero que nao e assunto delas.
static PAGINAS_EM_CACHE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(PAGINAS_PADRAO);

/// Ajusta o teto do cache de paginas do `.ndx`, em paginas.
///
/// Vale para os arquivos abertos DAQUI PARA A FRENTE: quem ja esta aberto
/// continua com o teto que tinha. Como isto e chamado no arranque, antes de a
/// primeira tabela abrir, na pratica vale para tudo.
///
/// Zero e recusado -- zero seria "sem cache", e quem quer isso desliga por
/// medida e nao por acidente de digitacao. Fica no padrao.
pub fn definir_cache_paginas(paginas: usize) {
    if paginas > 0 {
        PAGINAS_EM_CACHE.store(paginas, std::sync::atomic::Ordering::Relaxed);
    }
}

/// O teto vigente, em paginas.
pub fn cache_paginas() -> usize {
    PAGINAS_EM_CACHE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Acertos, faltas e gravacoes de pagina do processo inteiro.
///
/// # Por que os contadores sobem so no FECHAMENTO
///
/// Cada `.ndx` aberto ja conta os proprios acertos num `u64` comum, que nao
/// custa nada porque nao e atomico e nao e compartilhado. Somar cada toque
/// direto num atomico global custaria uma instrucao sincronizada por toque --
/// e sao 10,86 toques por linha inserida, medidos, num caminho onde a linha
/// inteira leva 7,5 us. Instrumentacao que cobra do caminho quente e
/// instrumentacao que muda o que ela mede.
///
/// Entao os contadores do arquivo sobem para ca UMA VEZ, quando ele fecha. O
/// preco disso e a granularidade: o servidor abre e fecha a tabela a cada
/// operacao, entao o numero anda por OPERACAO, e nao por linha. Uma carga de
/// cinco mil linhas so aparece quando ela termina -- o que a tela de
/// telemetria diz, em vez de fingir tempo real.
static CACHE_ACERTOS: AtomicU64 = AtomicU64::new(0);
static CACHE_FALTAS: AtomicU64 = AtomicU64::new(0);
static CACHE_GRAVACOES: AtomicU64 = AtomicU64::new(0);

/// (acertos, faltas, gravacoes de pagina) desde que o processo subiu.
pub fn contadores_de_cache() -> (u64, u64, u64) {
    (
        CACHE_ACERTOS.load(std::sync::atomic::Ordering::Relaxed),
        CACHE_FALTAS.load(std::sync::atomic::Ordering::Relaxed),
        CACHE_GRAVACOES.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Os `.ndx` cujo byte 52 em 1 ESTE processo sabe coerente no nucleo -- pedido
/// 522 -- com o CRC do cabecalho que o `fechar` deixou.
///
/// # Por que existe
///
/// O `fechar` levava as paginas ao nucleo e gravava o byte 52 em 0 sem
/// `fsync`. Para a queda do PROCESSO isso bastava; para a da MAQUINA, nao: o
/// cabecalho e a pagina 0, e nada o ordena depois das outras. Medido pelo
/// papel C contra o SO (ext4 sobre loop com provisionamento fino, 3/3): com a
/// escrita de fundo recusada, o nucleo guardou o cabecalho limpo e perdeu as
/// paginas -- byte 52 = 0 e `CRC invalido na pagina 3` depois de remontar.
///
/// Agora so o `sincronizar`, depois dos dois `fsync`, baixa o byte. O
/// `fechar` deixa o 1 e ATESTA aqui que ele e coerente no nucleo; a proxima
/// abertura NESTE processo -- o servidor abre e fecha a tabela a cada pedido
/// -- confia nele. Um processo novo nao tem o atestado, le o 1 e manda
/// reconstruir, que e o preco: depois de uma queda, `reindexar` das tabelas
/// tocadas desde o ultimo fecho da janela.
///
/// # Por que com o CRC do cabecalho, e nao so o caminho
///
/// O atestado vale para o CONTEUDO que o `fechar` deixou, e nao para quem
/// morar no mesmo caminho depois: uma restauracao por cima ou uma copia de
/// outro instante poem ali outro `.ndx`, talvez marcado de verdade. O
/// cabecalho carrega a raiz, os contadores, o numero de paginas e o instante
/// da gravacao; outro arquivo nao repete o CRC dele, e o atestado deixa de
/// valer sozinho -- sem ninguem precisar lembrar de apaga-lo.
///
/// O caminho, esse, o atestado NAO protege sozinho: renomear e o mesmo
/// inode, com o mesmo CRC, e a chave e que muda. Quem move ou copia um
/// `.ndx` leva o atestado junto por [`levar_atestado`] -- o B1 do parecer do
/// papel C sobre o 522.
///
/// # Por que do processo, e nao do punho
///
/// E o mesmo corte do [`crate::volume::familias_devendo_em`]: quem fecha e
/// quem reabre sao punhos diferentes do mesmo processo. Os testes rodam em
/// threads do mesmo processo, e a chave por caminho absoluto separa os
/// diretorios de cada um.
static ATESTADOS: std::sync::Mutex<std::collections::BTreeMap<PathBuf, u32>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Envenenamento aqui nao engana ninguem: a pior perda e um atestado, e sem
/// ele a abertura manda reconstruir -- o lado seguro.
fn atestados() -> std::sync::MutexGuard<'static, std::collections::BTreeMap<PathBuf, u32>> {
    ATESTADOS.lock().unwrap_or_else(|e| e.into_inner())
}

/// A chave do atestado: o caminho ABSOLUTO lexico, a mesma chave das
/// familias do `Volumes`. As recusas do `sincronia` guardam esta E a resolvida
/// no disco (pedido 523); o atestado fica so na lexica de proposito. O caminho que ja e
/// absoluto -- o do servidor, que o `Table::abrir` resolve -- nao aloca nada.
fn com_a_chave<T>(caminho: &Path, f: impl FnOnce(&Path) -> T) -> T {
    match crate::volume::absoluto_lexico(caminho) {
        Some(a) => f(&a),
        None => f(caminho),
    }
}

fn atestar(caminho: &Path, crc: u32) {
    com_a_chave(caminho, |c| {
        atestados().insert(c.to_path_buf(), crc);
    });
}

fn retirar_atestado(caminho: &Path) {
    com_a_chave(caminho, |c| {
        atestados().remove(c);
    });
}

/// Troca o CRC atestado, e SO se o atestado ainda for o de `de`: quem o tirou
/// no meio -- um punho que comecou a escrever -- ganha.
fn trocar_atestado(caminho: &Path, de: u32, para: u32) {
    com_a_chave(caminho, |c| {
        if let Some(atual) = atestados().get_mut(c) {
            if *atual == de {
                *atual = para;
            }
        }
    });
}

fn atestado(caminho: &Path, crc: u32) -> bool {
    com_a_chave(caminho, |c| atestados().get(c) == Some(&crc))
}

/// O atestado ACOMPANHA o arquivo que muda de caminho -- pedido 522, B1 do
/// parecer do papel C.
///
/// # O defeito que isto fecha
///
/// O atestado e guardado pelo caminho, e `renomear_tabela`,
/// `duplicar_tabela` e `copiar_tabela_para` poem o `.ndx` num caminho novo
/// sem `fsync` nenhum. Sem isto, a tabela escrita desde o ultimo fecho da
/// janela chegava ao destino com o 1 e sem atestado, e recusava TODA
/// operacao dizendo «arquivo corrompido» -- sem queda nenhuma (medido pelo
/// papel C: 9 recusas em 9, e 0 em 3 antes do 522).
///
/// # Por que levar o atestado, e nao sincronizar a origem antes
///
/// Sincronizar seria `fsync` novo com a trava global na mao -- as tres
/// operacoes rodam sob ela no servidor --, e a catraca `alcancam-fsync-2`
/// existe para isso nao entrar sem decisao. E levar e o que a verdade pede:
/// renomear e o MESMO inode, e a copia saiu do nucleo junto do `.reg` dela,
/// sob a mesma trava -- o que o atestado dizia da origem vale para o
/// destino, e so ate o processo cair, como na origem.
///
/// # Quando ele NAO vai junto
///
/// Quando a origem nao estava atestada (nada a levar), e quando o cabecalho
/// que chegou ao destino nao e o atestado -- alguem escreveu na origem entre
/// o fecho e a copia. O destino, entao, abre mandando reconstruir, que e o
/// lado seguro. Um atestado velho que morasse no caminho de destino sai
/// sempre: o arquivo que esta la agora nao e o dele.
///
/// `mover` tira o atestado da origem (o renomear); a copia o deixa la.
pub(crate) fn levar_atestado(de: &Path, para: &Path, mover: bool) {
    retirar_atestado(para);
    let crc = com_a_chave(de, |c| {
        let mut a = atestados();
        if mover {
            a.remove(c)
        } else {
            a.get(c).copied()
        }
    });
    let Some(crc) = crc else {
        return;
    };
    if crc_do_cabecalho_no_arquivo(para) == Some(crc) {
        atestar(para, crc);
    }
}

/// O CRC que o cabecalho de `caminho` traz no arquivo, se ele se le.
fn crc_do_cabecalho_no_arquivo(caminho: &Path) -> Option<u32> {
    let mut arquivo = File::open(caminho).ok()?;
    let mut cab = [0u8; CAB_LEN];
    ler_exato(&mut arquivo, 0, &mut cab).ok()?;
    Some(Campos(&cab).u32(124))
}

/// Esquece os atestados debaixo de `diretorio`: o que um processo NOVO veria.
/// **So para teste** -- e a unica forma de provar, dentro de um processo, o
/// que a abertura depois de uma queda le.
#[doc(hidden)]
pub fn esquecer_atestados_para_teste(diretorio: &Path) {
    com_a_chave(diretorio, |d| atestados().retain(|c, _| !c.starts_with(d)));
}

/// O byte 52 deste `.ndx` esta em 1 no arquivo? Le so o cabecalho.
///
/// Existe para o arranque achar, sem abrir tabela nenhuma, o indice que a
/// queda deixou marcado (pedido 522): abrir a `Table` inteira de cada tabela
/// da base so para perguntar isto custaria dez arquivos por tabela.
/// Cabecalho que nao confere e `Err`, e quem pergunta o trata como marcado.
pub fn marcado_no_arquivo(caminho: impl AsRef<Path>) -> Result<bool> {
    let caminho = caminho.as_ref();
    let mut arquivo = File::open(caminho)?;
    let mut cab = [0u8; CAB_LEN];
    ler_exato(&mut arquivo, 0, &mut cab)?;
    conferir_magic(&caminho.display().to_string(), MAGIC_NDX, &cab[0..8])?;
    if crc32(&cab[..124]) != Campos(&cab).u32(124) {
        return Err(PhxError::Corrompido(format!(
            "cabecalho de {} com CRC invalido",
            caminho.display()
        )));
    }
    Ok(cab[52] != 0)
}

/// As paginas do `.ndx` que ficam em RAM.
///
/// # De onde vem o ganho
///
/// Toda insercao DESCE a arvore: raiz, no interno, folha. Sao tres `pread` de
/// uma pagina inteira mais tres CRC-32 de pagina inteira -- e a raiz e a mesma
/// pagina em todas as insercoes da carga. Guardar a pagina lida tira o nucleo e
/// o CRC do caminho de quem ja passou por ali.
///
/// # A gravacao NAO atravessa mais (0.18.0)
///
/// Ate a 0.17.0 toda gravacao ia ao arquivo na hora, e este comentario
/// explicava por que: segurar pagina suja trocaria uma garantia por desempenho
/// **sem avisar**. O write-back entrou justamente quando passou a AVISAR: a
/// marca de sujo no byte 52 do cabecalho vai ao disco antes da primeira pagina
/// suja e so sai depois de todas, entao uma queda e detectada na abertura e o
/// indice recusa responder ate ser reconstruido -- barato desde a construcao
/// em lote. A troca que era inaceitavel em silencio ficou aceitavel declarada.
/// Historia completa em `docs/FORMATO.md` (a marca) e `docs/CONCORRENTES.md`.
///
/// # A politica de despejo
///
/// Segunda chance (CLOCK): a pagina despejada e a mais antiga que nao foi
/// usada desde que entrou. Fila simples nao serviria -- a raiz, que e a mais
/// visitada de todas, sairia junto com as outras assim que o teto enchesse.
struct CachePaginas {
    paginas: HashMap<u64, Entrada>,
    fila: VecDeque<u64>,
    teto: usize,
    acertos: u64,
    faltas: u64,
}

struct Entrada {
    bytes: Vec<u8>,
    usada: bool,
    /// A pagina mudou em RAM e ainda nao foi ao arquivo.
    suja: bool,
}

impl CachePaginas {
    fn nova(teto: usize) -> CachePaginas {
        CachePaginas {
            paginas: HashMap::with_capacity(teto.min(1024)),
            fila: VecDeque::with_capacity(teto.min(1024)),
            teto,
            acertos: 0,
            faltas: 0,
        }
    }

    fn pegar(&mut self, n: u64) -> Option<Vec<u8>> {
        match self.paginas.get_mut(&n) {
            Some(e) => {
                e.usada = true;
                self.acertos += 1;
                Some(e.bytes.clone())
            }
            None => {
                self.faltas += 1;
                None
            }
        }
    }

    /// Poe a pagina no cache. `suja` diz se ela ainda nao foi ao arquivo.
    ///
    /// Devolve a pagina SUJA que teve de sair para abrir lugar, se houve --
    /// quem chama e que sabe escrever no arquivo, e e ele que paga o CRC. Uma
    /// pagina limpa despejada nao devolve nada: o arquivo ja a tem.
    fn por(&mut self, n: u64, bytes: &[u8], suja: bool) -> Option<(u64, Vec<u8>)> {
        if let Some(e) = self.paginas.get_mut(&n) {
            e.bytes.clear();
            e.bytes.extend_from_slice(bytes);
            e.usada = true;
            e.suja |= suja;
            return None;
        }
        let mut despejada = None;
        while self.paginas.len() >= self.teto {
            match self.fila.pop_front() {
                None => break,
                Some(velha) => match self.paginas.get_mut(&velha) {
                    None => {}
                    Some(e) if e.usada => {
                        e.usada = false;
                        self.fila.push_back(velha);
                    }
                    Some(_) => {
                        let e = self.paginas.remove(&velha).unwrap();
                        if e.suja {
                            despejada = Some((velha, e.bytes));
                        }
                        break;
                    }
                },
            }
        }
        self.paginas.insert(
            n,
            Entrada {
                bytes: bytes.to_vec(),
                usada: false,
                suja,
            },
        );
        self.fila.push_back(n);
        despejada
    }

    /// Uma copia das sujas, em ordem de pagina, SEM limpar nenhuma.
    ///
    /// # Por que a flag so desce em [`CachePaginas::gravada`] (pedido 512)
    ///
    /// Isto se chamava `tirar_sujas` e baixava a flag de TODAS antes de quem
    /// chama gravar a primeira. O `descarregar` para no primeiro erro, entao a
    /// pagina que o disco cheio recusou -- e todas as seguintes -- saia da
    /// lista do que falta gravar sem ter ido a lugar nenhum. O segundo fecho
    /// achava nada sujo, e o `.ndx` ia ao disco com o byte 52 em 0 sobre uma
    /// pagina de zeros: `CRC invalido` em toda busca, e a abertura sem saber
    /// que tinha de reconstruir (medido pelo papel C num tmpfs de 512 KiB). E
    /// o `fsync` que mente, dentro do nosso proprio cache.
    fn sujas(&self) -> Vec<(u64, Vec<u8>)> {
        let mut fora: Vec<(u64, Vec<u8>)> = self
            .paginas
            .iter()
            .filter(|(_, e)| e.suja)
            .map(|(n, e)| (*n, e.bytes.clone()))
            .collect();
        // Em ordem de pagina: escrever para frente no arquivo em vez de saltar.
        fora.sort_unstable_by_key(|(n, _)| *n);
        fora
    }

    /// A pagina `n` chegou ao arquivo: so agora ela sai do que falta gravar.
    fn gravada(&mut self, n: u64) {
        if let Some(e) = self.paginas.get_mut(&n) {
            e.suja = false;
        }
    }

    /// A pagina despejada que NAO chegou ao arquivo volta, suja.
    ///
    /// E o irmao do `sujas` no despejo (pedido 512): o `por` a tira do cache
    /// antes de quem chama grava-la, e o erro a deixava sem lugar nenhum --
    /// nem no arquivo, nem na RAM. O cache passa do teto por uma pagina ate o
    /// proximo despejo, e e o preco certo: o outro era a pagina.
    fn devolver(&mut self, n: u64, bytes: Vec<u8>) {
        self.paginas.insert(
            n,
            Entrada {
                bytes,
                usada: true,
                suja: true,
            },
        );
        self.fila.push_back(n);
    }

    /// Tira a pagina do cache. A pagina que volta da lista de livres vai ser
    /// reescrita do zero, e o conteudo velho dela nao vale mais nada.
    fn esquecer(&mut self, n: u64) {
        self.paginas.remove(&n);
    }
}

pub struct NdxFile {
    arquivo: File,
    caminho: PathBuf,
    page_size: usize,
    /// O material de cifra deste arquivo: sal, iteracoes e a chave derivada.
    ///
    /// [`cofre::Material::EM_CLARO`] em todo `.ndx` -- a arvore da TABELA
    /// continua em claro por decisao registrada (`SEGURANCA.md` §11.3).
    /// Cifrado so no `.fts` de tabela com coluna marcada indexada por texto,
    /// e so quando o cofre esta ligado: ver [`NdxFile::criar_selado`].
    material: cofre::Material,
    qtd_paginas: u64,
    pagina_livre: u64,
    indices: Vec<DescritorIndice>,
    cache: CachePaginas,
    gravacoes: u64,
    /// A pagina 0 esta atrasada em relacao ao que ha em RAM.
    ///
    /// O cabecalho guarda quatro coisas: `qtd_paginas`, `pagina_livre`, a raiz
    /// de cada indice e a `qtd_chaves` de cada indice. As tres primeiras sao
    /// ESTRUTURA -- sem elas a arvore nao se acha -- e mudam raramente: uma
    /// alocacao de pagina a cada ~118 chaves, uma troca de raiz a cada nivel
    /// novo. A quarta e um CONTADOR, e `verificar` sabe recalcula-lo varrendo.
    ///
    /// Antes, toda chave inserida gravava 4 KiB no offset 0 -- com dois
    /// indices, 8 KiB por linha, so para adiantar um contador. E a terceira vez
    /// que este projeto encontra o mesmo defeito: o `.reg` reserializava o
    /// esquema por linha (DESEMPENHO.md 2.0) e o `.log` gravava o cabecalho por
    /// evento (2.2). **Cabecalho de arquivo nao pertence ao caminho quente.**
    estrutura_mudou: bool,
    /// Ha pagina suja em RAM, e o cabecalho no disco ja diz isso.
    ///
    /// # A rede de seguranca do write-back
    ///
    /// Com paginas sujas, uma queda deixa o `.ndx` atrasado em relacao ao
    /// `.reg`: a arvore pode ter chave faltando. Isso, sozinho, seria o pior
    /// defeito possivel -- busca respondendo errado sem ninguem notar.
    ///
    /// A marca desfaz isso: ela vai ao cabecalho ANTES da primeira pagina suja
    /// e so sai depois de todas irem ao disco. Quem abre um `.ndx` com a marca
    /// levantada sabe que ele nao presta e recusa responder, mandando
    /// reconstruir -- que desde a 0.17.0 custa 0,31 s por milhao de chaves.
    ///
    /// E o mesmo desenho do Aria, que compra a garantia de volta com tres
    /// bytes de "nao fechei direito" (`ma_locking.c:460`) mais reparo na
    /// abertura, em vez do redo log do InnoDB.
    ///
    /// Desde o pedido 522 «ir ao disco» quer dizer `fsync`: so o
    /// `sincronizar` baixa a marca. O `fechar` deixa o 1 e atesta, para este
    /// processo, que ele e coerente no nucleo -- ver [`ATESTADOS`].
    sujo: bool,
    /// O arquivo foi aberto com a marca de sujo e SEM atestado deste processo:
    /// a arvore nao e confiavel.
    precisa_reconstruir: bool,
    /// Este punho ja mudou a arvore (ou pos o `.reg` a frente dela) desde que
    /// abriu, ou desde o ultimo `fechar`/`sincronizar` -- pedido 522.
    ///
    /// Existe para o atestado sair ANTES da primeira mudanca e voltar so no
    /// fechamento limpo: um panico no meio deixa o arquivo sem atestado, e a
    /// reabertura neste mesmo processo manda reconstruir -- o que ela faria
    /// depois de uma queda no mesmo ponto. E e um `bool` do punho, e nao uma
    /// pergunta ao registro, porque [`NdxFile::levantar_marca`] roda a cada
    /// pagina suja: o registro so e tocado na primeira.
    mudou_desde_o_fecho: bool,
    /// O CRC-32 do cabecalho que esta no arquivo, como este punho o leu ou o
    /// gravou por ultimo. E a identidade que o atestado carrega.
    crc_do_cabecalho: u32,
    /// Quantas escritas estao abertas e ainda nao terminaram (pedido 456).
    ///
    /// # Por que o `Drop` precisa disto, e nao de `thread::panicking()`
    ///
    /// Uma escrita que muda a arvore -- ou que poe o `.reg` a frente dela --
    /// passa por estados em que a arvore em RAM esta RASGADA: a metade
    /// esquerda de uma divisao ja no cache e a direita ainda nao, ou a linha
    /// ja viva no `.reg` e a chave ainda fora do indice. Um panico nesse meio
    /// desenrola a pilha, e o `Drop` descarregava as paginas no estado em que
    /// o panico as deixou e baixava o byte 52: a arvore rasgada ia ao disco
    /// marcada LIMPA. Um `SIGKILL` no mesmo ponto perde as paginas e deixa o
    /// byte em 1 -- o panico era pior que a queda.
    ///
    /// Decidir por `panicking()` nao alcanca o FFI: o `punho::com` captura o
    /// panico, e o `liberar` roda o `Drop` DEPOIS, com `panicking()` falso. O
    /// que sabe que a escrita nao terminou e a propria escrita, e por isso o
    /// estado mora aqui.
    escritas_em_voo: u32,
    /// Uma escrita parou no meio com ERRO, depois de ja ter mexido: a arvore
    /// nao presta ate o `reindexar`. E o irmao do panico que devolve `Err` --
    /// ninguem desenrola, mas o estado do meio e o mesmo.
    escrita_interrompida: bool,
    /// Quantas vezes a arvore mudou em RAM. Serve so para separar o erro que
    /// RECUSOU antes de mexer (a arvore continua inteira) do que interrompeu
    /// depois de mexer (rasgada).
    mudancas_na_arvore: u64,
    /// A cascata do `ao_alterar` ainda nao terminou nesta tabela filha
    /// (pedido 490): a mae ja foi para a chave nova, e alguma linha daqui
    /// pode estar na velha.
    ///
    /// # Por que nao e uma `escritas_em_voo` a mais
    ///
    /// A arvore desta filha NAO esta rasgada: cada linha da cascata abre e
    /// fecha a sua janela, e entre duas linhas indice e `.reg` concordam. O
    /// que falta e a cascata, e isso a arvore nao conserta. Por isso esta
    /// marca nao segura o `sincronizar` nem o `fechar` -- a neta confere a
    /// chave desta filha num segundo descritor, e precisa do byte 52 em 0 no
    /// disco entre duas linhas (a janela do passo inteiro foi medida e
    /// recusava toda cascata de tres niveis). Ela so muda o `Drop`: o handle
    /// que morre com ela ligada -- o desenrolar de um panico -- deixa o disco
    /// como um `SIGKILL` deixaria, com o byte em 1, e a tabela recusa ate o
    /// `reindexar`. Mesmo desenho da `escritas_em_voo`: quem sabe que nao
    /// terminou e o proprio trabalho, e nao `thread::panicking()`.
    cascata_em_voo: bool,
}

// ---------------------------------------------------------------- paginas

fn pag_tipo(p: &[u8]) -> u8 {
    p[0]
}
fn pag_qtd(p: &[u8]) -> usize {
    Campos(p).u16(2) as usize
}
fn pag_set_qtd(p: &mut [u8], v: usize) {
    por_u16(p, 2, v as u16);
}
fn pag_prox(p: &[u8]) -> u64 {
    Campos(p).u64(4)
}
fn pag_set_prox(p: &mut [u8], v: u64) {
    por_u64(p, 4, v);
}
fn pag_ant(p: &[u8]) -> u64 {
    Campos(p).u64(12)
}
fn pag_set_ant(p: &mut [u8], v: u64) {
    por_u64(p, 12, v);
}
fn pag_dir(p: &[u8]) -> u64 {
    Campos(p).u64(20)
}
fn pag_set_dir(p: &mut [u8], v: u64) {
    por_u64(p, 20, v);
}

/// CRC da pagina, calculado sobre tudo menos os proprios 4 bytes do CRC.
///
/// `corpo` e onde a area util termina: `page_size` num arquivo em claro, e
/// `page_size - rabo` num arquivo selado -- o tempero e a etiqueta ficam de
/// FORA porque o CRC cobre o claro, e eles so existem depois de cifrar. Em
/// claro `corpo == page_size` e a conta e byte a byte a mesma de sempre.
fn pag_crc(p: &[u8], corpo: usize) -> u32 {
    crc32_with(crc32(&p[..28]), &p[32..corpo])
}
fn pag_selar(p: &mut [u8], corpo: usize) {
    let c = pag_crc(p, corpo);
    por_u32(p, 28, c);
}

/// O dado associado da pagina selada: o numero dela e o cabecalho de 32 bytes.
///
/// O cabecalho entra inteiro porque e ele que fica EM CLARO no disco -- tipo,
/// quantidade, vizinhas, filho da direita e o CRC do claro. Sem ele na
/// etiqueta, trocar `qtd` de uma pagina nao seria detectado pela cifra, so
/// pelo CRC; com ele, as duas conferencias amarram a mesma pagina. O numero
/// impede trocar a pagina 7 pela 9 inteiras, que teriam cabecalhos plausiveis.
fn aad_da_pagina(n: u64, cabecalho: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(8 + cabecalho.len());
    aad.extend_from_slice(&n.to_le_bytes());
    aad.extend_from_slice(cabecalho);
    aad
}

/// A parte ESTAVEL do cabecalho que a prova do material amarra.
///
/// Fica de fora o que muda a cada gravacao -- contadores, raiz, marca de sujo
/// --, pelo mesmo motivo do `.reg`: uma prova que mudasse com os contadores
/// teria de ser refeita e reconferida toda vez sem proteger nada a mais.
fn rotulo_da_prova(versao: u16, page_size: usize) -> Vec<u8> {
    let mut r = Vec::with_capacity(MAGIC_NDX.len() + 6);
    r.extend_from_slice(MAGIC_NDX);
    r.extend_from_slice(&versao.to_le_bytes());
    r.extend_from_slice(&(page_size as u32).to_le_bytes());
    r
}

/// Quanto de cada folha a construcao em lote enche, em porcento.
///
/// **80, e o numero e medido** (`--example indice-em-lote`, um milhao de chaves
/// e mais 10% inseridas depois no MEIO da faixa). Nao e nem a folga classica de
/// 70 nem o instinto de encher tudo:
///
/// ```text
///   enchimento   paginas   varrer   crescer   paginas novas
///          70%      6.028    0,035s    0,804s              0
///          80%      5.271    0,028s    0,770s              0   <- o joelho
///          90%      4.683    0,026s    0,901s          2.342
///         100%      4.213    0,023s    0,984s          2.110
/// ```
///
/// 70 nao compra nada: insercao aleatoria ja assenta perto de 69% de ocupacao
/// sozinha, e um resultado classico de B-tree. De 90 para cima a folha nao tem
/// mais folga, e crescer passa a alocar milhares de paginas e a ficar mais
/// LENTO do que a arvore mais frouxa -- a varredura mais rapida nao paga isso.
///
/// 80 e a ocupacao mais densa que ainda absorve 10% de crescimento sem alocar
/// uma pagina, e por isso e a mais rapida das duas pontas que importam.
const ENCHIMENTO_PADRAO: usize = 80;

/// Tamanho da fatia `i` ao repartir `total` em `partes` o mais iguais possivel.
///
/// As primeiras `total % partes` fatias levam uma a mais. Existe para nao
/// terminar com uma folha de uma chave so depois de encher todas as outras.
fn fatia(total: usize, partes: usize, i: usize) -> usize {
    total / partes + usize::from(i < total % partes)
}

fn nova_pagina(page_size: usize, tipo: u8) -> Vec<u8> {
    let mut p = vec![0u8; page_size];
    p[0] = tipo;
    p
}

// -------------------------------------------------------------- entradas

fn folha_entrada(p: &[u8], i: usize, ck_len: usize) -> &[u8] {
    &p[PAG_CAB + i * ck_len..PAG_CAB + (i + 1) * ck_len]
}

fn interno_chave(p: &[u8], i: usize, ck_len: usize) -> &[u8] {
    let ent = ck_len + 8;
    &p[PAG_CAB + i * ent..PAG_CAB + i * ent + ck_len]
}

fn interno_filho(p: &[u8], i: usize, ck_len: usize) -> u64 {
    let ent = ck_len + 8;
    Campos(p).u64(PAG_CAB + i * ent + ck_len)
}

fn interno_set_filho(p: &mut [u8], i: usize, ck_len: usize, filho: u64) {
    let ent = ck_len + 8;
    por_u64(p, PAG_CAB + i * ent + ck_len, filho);
}

/// Primeira posicao cuja entrada e >= `alvo`.
fn lower_bound_folha(p: &[u8], ck_len: usize, alvo: &[u8]) -> usize {
    let (mut lo, mut hi) = (0usize, pag_qtd(p));
    while lo < hi {
        let mid = (lo + hi) / 2;
        if folha_entrada(p, mid, ck_len) < alvo {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Indice do filho a seguir num no interno. `qtd` significa `filho_direita`.
fn escolher_filho(p: &[u8], ck_len: usize, alvo: &[u8]) -> usize {
    let (mut lo, mut hi) = (0usize, pag_qtd(p));
    while lo < hi {
        let mid = (lo + hi) / 2;
        if alvo < interno_chave(p, mid, ck_len) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

// ------------------------------------------------------------- NdxFile

impl NdxFile {
    /// Cria o `.ndx` com uma arvore vazia para cada indice do esquema.
    pub fn criar(caminho: impl AsRef<Path>, esquema: &Schema) -> Result<NdxFile> {
        Self::criar_com_pagina(caminho, esquema, PAGINA_PADRAO)
    }

    /// Cria o arquivo com a PAGINA selada, quando o cofre esta ligado.
    ///
    /// # Cifrar o ARMAZENAMENTO, e nao a chave dentro do indice
    ///
    /// A chave continua em claro DENTRO da pagina -- e e por isso que a ordem
    /// da B+tree sobrevive e nenhuma capacidade de busca se perde. O que some
    /// do disco e a pagina inteira: quem copia o arquivo nao ve termo nenhum.
    /// E o desenho do PostgreSQL, do MariaDB e do MySQL para proteger indice,
    /// e entrou por aceite automatico dos tres maduros -- nenhuma pétrea
    /// nossa se opoe, e o modelo de ameaca e o que `cofre.rs` ja declara:
    /// disco levado, backup vazado, copia numa maquina que nao e esta.
    ///
    /// # Por que so o `.fts` chama
    ///
    /// Porque o pedido 340 e do `.fts`, e o `.ndx` da tabela e outra decisao,
    /// registrada e em vigor (`SEGURANCA.md` §11.3). O mecanismo aqui serve
    /// aos dois; ligar o `.ndx` muda o formato de TODA tabela indexada e paga
    /// a cifra no laco quente do `inserir` -- e isso se decide medido e com o
    /// DBA, nao de passagem.
    ///
    /// Cofre desligado devolve um arquivo em claro, versao 1, igual ao de
    /// antes: a cifra e do PROCESSO e nao deste caminho.
    pub fn criar_selado(caminho: impl AsRef<Path>, esquema: &Schema) -> Result<NdxFile> {
        // `em_aead`: a pagina tem tamanho FIXO e o pacote FrogCript e 167
        // bytes maior que o claro -- ele nao cabe dentro da pagina que
        // cifraria. A restricao e de formato, e esta escrita em `Material`.
        let material = cofre::Material::novo()?.em_aead();
        Self::criar_com(caminho, esquema, PAGINA_PADRAO, material)
    }

    pub fn criar_com_pagina(
        caminho: impl AsRef<Path>,
        esquema: &Schema,
        page_size: usize,
    ) -> Result<NdxFile> {
        Self::criar_com(caminho, esquema, page_size, cofre::Material::EM_CLARO)
    }

    fn criar_com(
        caminho: impl AsRef<Path>,
        esquema: &Schema,
        page_size: usize,
        material: cofre::Material,
    ) -> Result<NdxFile> {
        if !page_size.is_power_of_two() || page_size < 512 {
            return Err(PhxError::Esquema(format!(
                "page_size {page_size} invalido: use potencia de 2 >= 512"
            )));
        }
        let caminho = caminho.as_ref().to_path_buf();
        // O arquivo vai ser truncado: o atestado do que morava aqui nao vale
        // para o que vai nascer (pedido 522).
        retirar_atestado(&caminho);
        let arquivo = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&caminho)?;

        let mut n = NdxFile {
            arquivo,
            caminho,
            page_size,
            material,
            qtd_paginas: 1, // pagina 0 = cabecalho + diretorio
            pagina_livre: 0,
            indices: Vec::new(),
            cache: CachePaginas::nova(cache_paginas()),
            gravacoes: 0,
            estrutura_mudou: false,
            sujo: false,
            precisa_reconstruir: false,
            mudou_desde_o_fecho: false,
            crc_do_cabecalho: 0,
            escritas_em_voo: 0,
            escrita_interrompida: false,
            mudancas_na_arvore: 0,
            cascata_em_voo: false,
        };
        n.arquivo.set_len(page_size as u64)?;

        for (i, idx) in esquema.indices().iter().enumerate() {
            let key_len = esquema.largura_chave(i)?;
            let ck_len = key_len + ROWID_LEN;
            n.validar_capacidade(ck_len, &idx.nome)?;
            let raiz = n.alocar_pagina()?;
            let mut folha = nova_pagina(page_size, TIPO_FOLHA);
            n.gravar_pagina(raiz, &mut folha)?;
            n.indices.push(DescritorIndice {
                nome: idx.nome.clone(),
                unico: idx.unico,
                key_len,
                raiz,
                qtd_chaves: 0,
            });
        }
        n.gravar_cabecalho()?;
        Ok(n)
    }

    pub fn abrir(caminho: impl AsRef<Path>) -> Result<NdxFile> {
        let caminho = caminho.as_ref().to_path_buf();
        let mut arquivo = OpenOptions::new().read(true).write(true).open(&caminho)?;
        let nome = caminho.display().to_string();

        let mut cab = [0u8; CAB_LEN];
        ler_exato(&mut arquivo, 0, &mut cab)?;
        conferir_magic(&nome, MAGIC_NDX, &cab[0..8])?;

        let c = Campos(&cab);
        let versao = c.u16(8);
        if versao != VERSAO && versao != VERSAO_SELADA {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: nome,
                encontrada: versao,
                suportada: VERSAO_SELADA,
            });
        }
        if crc32(&cab[..124]) != c.u32(124) {
            return Err(PhxError::Corrompido(format!(
                "cabecalho de {nome} com CRC invalido"
            )));
        }

        let page_size = c.u32(12) as usize;
        let qtd_indices = c.u32(16) as usize;
        let qtd_paginas = c.u64(20);
        let pagina_livre = c.u64(28);
        let dir_len = c.u32(36) as usize;
        let dir_crc = c.u32(40);
        // Byte 52: a marca de sujo. Arquivo escrito antes da 0.18.0 tem zero
        // ali, e zero e "limpo" -- que e a verdade para quem so escrevia
        // atraves. Nao ha migracao a fazer.
        let sujo = cab[52] != 0;
        // O 1 que um `fechar` DESTE processo deixou nao e queda: as paginas
        // estao no nucleo, e o cabecalho e o mesmo que ele gravou (pedido
        // 522). Depois de um `fsync` recusado no diretorio o nucleo deixa de
        // ser testemunha -- pode ter descartado o que nao foi ao disco --, e o
        // atestado nao vale mais.
        let crc_do_cabecalho = c.u32(124);
        let coerente_no_nucleo = sujo
            && atestado(&caminho, crc_do_cabecalho)
            && crate::sincronia::recusado_em(&caminho).is_none();

        // A chave sai daqui, e a prova dentro do material recusa a senha
        // errada AGORA -- e nao na primeira descida da arvore, que num indice
        // recem-criado seria nunca. Versao 1 nao chega a perguntar pelo
        // cofre: e o caminho de todo arquivo escrito antes desta versao.
        let material = if versao == VERSAO_SELADA {
            cofre::Material::ler(
                &cab,
                MATERIAL_EM,
                &nome,
                &rotulo_da_prova(versao, page_size),
            )?
        } else {
            cofre::Material::EM_CLARO
        };
        if versao == VERSAO_SELADA && !material.cifrado() {
            // A versao promete pagina selada e a flag do material diz que
            // nao ha cifra. Seguir leria etiqueta como entrada de folha e
            // responderia busca com lixo -- calado, que e o pior de tudo.
            return Err(PhxError::Corrompido(format!(
                "{nome} declara pagina selada e nao traz material de cifra no cabecalho"
            )));
        }

        let mut dir = vec![0u8; dir_len];
        ler_exato(&mut arquivo, CAB_LEN as u64, &mut dir)?;
        if crc32(&dir) != dir_crc {
            return Err(PhxError::Corrompido(format!(
                "diretorio de indices de {nome} com CRC invalido"
            )));
        }

        let mut indices = Vec::with_capacity(qtd_indices);
        let mut pos = 0usize;
        for _ in 0..qtd_indices {
            if pos + 2 > dir.len() {
                return Err(PhxError::Corrompido(format!(
                    "diretorio de indices de {nome} truncado"
                )));
            }
            let nl = u16::from_le_bytes(dir[pos..pos + 2].try_into().unwrap()) as usize;
            if pos + 2 + nl + 21 > dir.len() {
                return Err(PhxError::Corrompido(format!(
                    "diretorio de indices de {nome} truncado"
                )));
            }
            pos += 2;
            let nome_idx = String::from_utf8(dir[pos..pos + nl].to_vec())
                .map_err(|e| PhxError::Corrompido(format!("nome de indice invalido: {e}")))?;
            pos += nl;
            let unico = dir[pos] != 0;
            pos += 1;
            let key_len = u32::from_le_bytes(dir[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            let raiz = u64::from_le_bytes(dir[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let qtd_chaves = u64::from_le_bytes(dir[pos..pos + 8].try_into().unwrap());
            pos += 8;
            indices.push(DescritorIndice {
                nome: nome_idx,
                unico,
                key_len,
                raiz,
                qtd_chaves,
            });
        }

        Ok(NdxFile {
            arquivo,
            caminho,
            page_size,
            material,
            qtd_paginas,
            pagina_livre,
            indices,
            cache: CachePaginas::nova(cache_paginas()),
            gravacoes: 0,
            estrutura_mudou: false,
            sujo,
            precisa_reconstruir: sujo && !coerente_no_nucleo,
            mudou_desde_o_fecho: false,
            crc_do_cabecalho,
            escritas_em_voo: 0,
            escrita_interrompida: false,
            mudancas_na_arvore: 0,
            cascata_em_voo: false,
        })
    }

    /// Bytes que a selagem cobra no fim de cada pagina: o tempero e a
    /// etiqueta. Zero num arquivo em claro, e ai nada muda de lugar.
    fn rabo(&self) -> usize {
        if self.material.cifrado() {
            TEMPERO_LEN + self.material.acrescimo()
        } else {
            0
        }
    }

    /// Onde a area util da pagina termina.
    ///
    /// E o unico numero que precisa saber da cifra: capacidade de folha,
    /// capacidade de no interno e CRC saem daqui. Em claro ele e o
    /// `page_size` inteiro, e todas as contas voltam a ser as de sempre.
    fn corpo(&self) -> usize {
        self.page_size - self.rabo()
    }

    /// Este arquivo grava a pagina selada?
    pub fn selado(&self) -> bool {
        self.material.cifrado()
    }

    /// Quantas entradas cabem numa FOLHA deste indice.
    ///
    /// Sai daqui, e nao de uma conta refeita no medidor: o custo do selo e um
    /// numero publicado, e receita de numero copiada envelhece calada. Quem
    /// quiser o custo divide a capacidade selada pela em claro -- as duas
    /// saem desta mesma funcao.
    pub fn capacidade_de_folha(&self, idx: usize) -> usize {
        match self.indices.get(idx) {
            Some(d) => (self.corpo() - PAG_CAB) / d.ck_len(),
            None => 0,
        }
    }

    fn validar_capacidade(&self, ck_len: usize, nome: &str) -> Result<()> {
        let cap_folha = (self.corpo() - PAG_CAB) / ck_len;
        let cap_interno = (self.corpo() - PAG_CAB) / (ck_len + 8);
        if cap_folha < MIN_ENTRADAS || cap_interno < MIN_ENTRADAS {
            return Err(PhxError::Esquema(format!(
                "indice {nome}: chave de {ck_len} bytes e grande demais para paginas de {} bytes \
                 (cabem {cap_folha} por folha, minimo {MIN_ENTRADAS})",
                self.page_size
            )));
        }
        Ok(())
    }

    fn serializar_diretorio(&self) -> Vec<u8> {
        let mut d = Vec::new();
        for i in &self.indices {
            let nb = i.nome.as_bytes();
            d.extend_from_slice(&(nb.len() as u16).to_le_bytes());
            d.extend_from_slice(nb);
            d.push(i.unico as u8);
            d.extend_from_slice(&(i.key_len as u32).to_le_bytes());
            d.extend_from_slice(&i.raiz.to_le_bytes());
            d.extend_from_slice(&i.qtd_chaves.to_le_bytes());
        }
        d
    }

    fn gravar_cabecalho(&mut self) -> Result<()> {
        let dir = self.serializar_diretorio();
        if CAB_LEN + dir.len() > self.page_size {
            return Err(PhxError::LimiteExcedido(format!(
                "diretorio de {} indices nao cabe na pagina 0 de {} bytes",
                self.indices.len(),
                self.page_size
            )));
        }
        let versao = if self.material.cifrado() {
            VERSAO_SELADA
        } else {
            VERSAO
        };
        let mut buf = vec![0u8; self.page_size];
        buf[0..8].copy_from_slice(MAGIC_NDX);
        buf[8..10].copy_from_slice(&versao.to_le_bytes());
        buf[10..12].copy_from_slice(&(CAB_LEN as u16).to_le_bytes());
        por_u32(&mut buf, 12, self.page_size as u32);
        por_u32(&mut buf, 16, self.indices.len() as u32);
        por_u64(&mut buf, 20, self.qtd_paginas);
        por_u64(&mut buf, 28, self.pagina_livre);
        por_u32(&mut buf, 36, dir.len() as u32);
        por_u32(&mut buf, 40, crc32(&dir));
        por_i64(&mut buf, 44, agora());
        // Byte 52: a marca de sujo. Ficava zerado ate a 0.17.0, e zero quer
        // dizer "limpo" -- entao um `.ndx` escrito antes desta versao continua
        // sendo lido com o significado certo, sem migracao.
        buf[52] = u8::from(self.sujo);
        // 56..96: o material de cifra, nos bytes que ja eram reservados. Em
        // claro isto nao escreve nada, e o cabecalho fica byte a byte o que
        // sempre foi.
        self.material.gravar(
            &mut buf,
            MATERIAL_EM,
            &rotulo_da_prova(versao, self.page_size),
        );
        let crc = crc32(&buf[..124]);
        por_u32(&mut buf, 124, crc);
        buf[CAB_LEN..CAB_LEN + dir.len()].copy_from_slice(&dir);
        escrever_em(&mut self.arquivo, 0, &buf)?;
        self.estrutura_mudou = false;
        self.crc_do_cabecalho = crc;
        Ok(())
    }

    fn ler_pagina(&mut self, n: u64) -> Result<Vec<u8>> {
        if n == 0 || n >= self.qtd_paginas {
            return Err(PhxError::Corrompido(format!(
                "pagina {n} fora do arquivo {}",
                self.caminho.display()
            )));
        }
        if let Some(p) = self.cache.pegar(n) {
            return Ok(p);
        }
        let mut p = vec![0u8; self.page_size];
        ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
        // A cifra vem ANTES do CRC porque o CRC e do claro: ele existe para
        // achar bit trocado no disco, e trocar a ordem faria ele conferir
        // texto cifrado contra um numero que nunca cobriu texto cifrado.
        self.abrir_pagina(n, &mut p)?;
        let corpo = self.corpo();
        // O CRC e conferido na LEITURA DO ARQUIVO, e nao na do cache: a pagina
        // que esta em RAM ja passou por aqui, e conferir de novo pagaria o
        // mesmo CRC que este cache existe para nao pagar.
        if pag_crc(&p, corpo) != Campos(&p).u32(28) {
            return Err(PhxError::Corrompido(format!(
                "CRC invalido na pagina {n} de {}",
                self.caminho.display()
            )));
        }
        self.guardar_no_cache(n, &p, false)?;
        Ok(p)
    }

    /// Decifra a pagina lida do disco, no lugar. Em claro, nao faz nada.
    ///
    /// ```text
    /// [0..32]           cabecalho da pagina, EM CLARO
    /// [32..corpo]       entradas, cifradas
    /// [corpo..+8]       tempero sorteado NESTA gravacao
    /// [corpo+8..fim]    etiqueta Poly1305
    /// ```
    ///
    /// O cabecalho fica em claro de proposito, e nao e descuido: ele nao
    /// guarda termo nenhum -- tipo, quantidade, vizinhas, filho da direita e
    /// o CRC do claro --, e e por ele que a lista de paginas livres se
    /// percorre sem chave e que `reparar` sabe o que tem na mao. O que o
    /// adversario do `cofre.rs` quer esta nas ENTRADAS, e elas somem.
    fn abrir_pagina(&self, n: u64, p: &mut [u8]) -> Result<()> {
        if !self.material.cifrado() {
            return Ok(());
        }
        let corpo = self.corpo();
        let tempero = Campos(p).u64(corpo);
        let nonce = cofre::nonce_de_pedaco(n, 0, 0, tempero);
        let aad = aad_da_pagina(n, &p[..PAG_CAB]);
        let mut guardado = p[PAG_CAB..corpo].to_vec();
        guardado.extend_from_slice(&p[corpo + TEMPERO_LEN..]);
        let claro =
            self.material
                .abrir(&nonce, &aad, &guardado, &self.caminho.display().to_string())?;
        p[PAG_CAB..corpo].copy_from_slice(&claro);
        // O rabo volta a zero em RAM para a pagina em memoria ser identica a
        // que `nova_pagina` produz: tempero e etiqueta sao do DISCO, e cada
        // gravacao sorteia os proprios. Deixa-los ali faria duas paginas de
        // mesmo conteudo divergirem em RAM sem motivo.
        p[corpo..].fill(0);
        Ok(())
    }

    /// Poe a pagina no cache e grava a que for despejada SUJA.
    ///
    /// Existe como caminho unico porque o despejo nao pode ser perdido em
    /// nenhum dos dois lados. Foi assim que o primeiro write-back quebrou: o
    /// `ler_pagina` chamava o cache e jogava fora o retorno, entao uma pagina
    /// recem-alocada que so existia suja em RAM era despejada e sumia -- o
    /// arquivo ficava com os zeros do `set_len`, e a leitura seguinte batia num
    /// CRC invalido. A suite inteira passou; quem pegou foi a medicao.
    fn guardar_no_cache(&mut self, n: u64, p: &[u8], suja: bool) -> Result<()> {
        if let Some((velha, mut bytes)) = self.cache.por(n, p, suja) {
            if let Err(e) = self.escrever_pagina(velha, &mut bytes) {
                self.cache.devolver(velha, bytes);
                return Err(e);
            }
        }
        Ok(())
    }

    /// A pagina mudou. Ela fica SUJA em RAM; o CRC e o `write` sao adiados.
    ///
    /// # Por que adiar, e o que isso troca
    ///
    /// Antes, toda pagina tocada era selada e escrita na hora: 2,06 gravacoes
    /// por linha, cada uma pagando o CRC-32 da pagina inteira. Numa carga a
    /// mesma folha recebe centenas de chaves seguidas, e pagava-se o CRC uma
    /// vez por chave em vez de uma vez por folha.
    ///
    /// E como o InnoDB e o Aria fazem: a mini-transacao do InnoDB so marca a
    /// pagina suja (`mtr0mtr.cc:338`) e o checksum sai na descarga
    /// (`buf0flu.cc:1243`); o Aria tem `PCBLOCK_CHANGED` (`ma_pagecache.c:177`)
    /// e `PAGECACHE_WRITE_DELAY` (`ma_page.c:255`). Medido aqui: **13,1 -> 7,2
    /// us por linha**.
    ///
    /// O preco e a garantia que o `FORMATO.md` descrevia: antes, uma queda do
    /// PROCESSO nao atrasava o `.ndx` em relacao ao `.reg`, porque o `write` ja
    /// tinha entregue a pagina ao nucleo. Agora atrasa -- e por isso existe a
    /// marca de sujo no cabecalho, que faz a queda ser DETECTADA. Indice
    /// atrasado se reconstroi do `.reg`; indice atrasado em silencio, nao.
    fn gravar_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        // A marca vai ao arquivo ANTES da primeira pagina suja existir. Ao
        // contrario, uma queda no meio deixaria cabecalho limpo com paginas
        // faltando -- que e exatamente o defeito que ela existe para impedir.
        self.levantar_marca()?;
        self.mudancas_na_arvore += 1;
        self.guardar_no_cache(n, p, true)
    }

    /// Poe o byte 52 em 1 NO ARQUIVO, se ainda nao estiver.
    ///
    /// Um lugar so para as duas portas que sobem a marca -- a primeira pagina
    /// suja e o [`NdxFile::comecar_escrita`] --, para a decisao «sobe antes de
    /// escrever» nao divergir de si mesma. Se o cabecalho nao for ao disco, a
    /// marca em RAM volta a 0: marca em RAM que o disco nao tem faria a
    /// proxima pagina suja pular a subida, e a queda seguinte sairia calada.
    ///
    /// E tira o atestado do processo antes da primeira mudanca (pedido 522):
    /// daqui ate o `fechar`, o 1 do arquivo volta a querer dizer «pode estar
    /// para tras». Mesmo com o byte ja em 1 -- aberto atestado, o arquivo nao
    /// muda, mas a verdade sobre ele muda, e um panico no meio tem de deixar a
    /// reabertura mandando reconstruir.
    fn levantar_marca(&mut self) -> Result<()> {
        if !self.mudou_desde_o_fecho {
            retirar_atestado(&self.caminho);
            self.mudou_desde_o_fecho = true;
        }
        if self.sujo {
            return Ok(());
        }
        self.sujo = true;
        if let Err(e) = self.gravar_cabecalho() {
            self.sujo = false;
            return Err(e);
        }
        Ok(())
    }

    /// Sela e escreve de verdade. So o despejo e o `sincronizar` chamam.
    fn escrever_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        #[cfg(debug_assertions)]
        if let Some(e) = crate::sincronia::falha_de_teste::disparar(
            &self.caminho,
            crate::sincronia::falha_de_teste::Onde::PaginaDoIndice,
        ) {
            return Err(PhxError::Io(e));
        }
        let corpo = self.corpo();
        pag_selar(p, corpo);
        if self.material.cifrado() {
            // Um buffer novo, e nao cifrar `p` no lugar: `p` volta ao cache
            // como pagina EM CLARO. Cifrar no lugar poria texto cifrado no
            // cache, e a proxima leitura serviria isso como entrada de folha.
            let mut disco = p.to_vec();
            // Oito bytes sorteados NESTA gravacao. Sem eles, reescrever a
            // mesma pagina repetiria o par (chave, nonce) -- e num cifrador
            // de fluxo isso entrega o XOR dos dois conteudos a quem tem as
            // duas copias do arquivo. E a mesma conta do slot do `.reg`.
            let tempero = cifra::sortear_u64();
            por_u64(&mut disco, corpo, tempero);
            let nonce = cofre::nonce_de_pedaco(n, 0, 0, tempero);
            let aad = aad_da_pagina(n, &disco[..PAG_CAB]);
            let selado = self.material.selar(&nonce, &aad, &disco[PAG_CAB..corpo]);
            let claro_len = corpo - PAG_CAB;
            disco[PAG_CAB..corpo].copy_from_slice(&selado[..claro_len]);
            disco[corpo + TEMPERO_LEN..].copy_from_slice(&selado[claro_len..]);
            escrever_em(&mut self.arquivo, n * self.page_size as u64, &disco)?;
        } else {
            escrever_em(&mut self.arquivo, n * self.page_size as u64, p)?;
        }
        self.gravacoes += 1;
        Ok(())
    }

    /// Leva todas as paginas sujas ao arquivo. A que falhar -- e as que vem
    /// depois dela -- continua suja: ver [`CachePaginas::sujas`].
    fn descarregar(&mut self) -> Result<()> {
        for (n, mut bytes) in self.cache.sujas() {
            self.escrever_pagina(n, &mut bytes)?;
            self.cache.gravada(n);
        }
        Ok(())
    }

    /// Quantas paginas o cache serviu, quantas vieram do arquivo, e quantas
    /// foram gravadas.
    ///
    /// Existe para o medidor nao ter de CITAR um `strace` de outro dia: o
    /// numero de toques de pagina por linha inserida e medido aqui dentro, e
    /// envelhece junto com o codigo em vez de envelhecer calado.
    /// Poe o contador de chaves num valor qualquer. **So para teste.**
    ///
    /// Existe porque a conferencia precisa provar que ela ainda para quando a
    /// varredura acha MENOS chaves do que o diretorio diz -- e nao ha como
    /// chegar nesse estado por fora sem corromper o arquivo a mao.
    #[doc(hidden)]
    pub fn forjar_contador_para_teste(&mut self, idx: usize, qtd: u64) {
        if let Some(d) = self.indices.get_mut(idx) {
            d.qtd_chaves = qtd;
        }
    }

    pub fn estatisticas_paginas(&self) -> (u64, u64, u64) {
        (self.cache.acertos, self.cache.faltas, self.gravacoes)
    }

    fn alocar_pagina(&mut self) -> Result<u64> {
        if self.pagina_livre != 0 {
            let n = self.pagina_livre;
            let mut p = vec![0u8; self.page_size];
            ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
            // Le o cabecalho CRU, sem decifrar, e continua certo num arquivo
            // selado: a lista de livres mora no byte 4 do cabecalho da
            // pagina, que fica em claro. Andar nela nao precisa de chave, que
            // e metade da razao de o cabecalho nao ser cifrado.
            self.pagina_livre = pag_prox(&p);
            // A pagina volta da lista de livres para ser reescrita do zero: o
            // que o cache tem dela e o conteudo de antes de ela ser liberada.
            self.cache.esquecer(n);
            self.estrutura_mudou = true;
            self.mudancas_na_arvore += 1;
            return Ok(n);
        }
        let n = self.qtd_paginas;
        self.qtd_paginas += 1;
        self.mudancas_na_arvore += 1;
        self.arquivo
            .set_len(self.qtd_paginas * self.page_size as u64)?;
        self.estrutura_mudou = true;
        Ok(n)
    }

    pub fn indices(&self) -> &[DescritorIndice] {
        &self.indices
    }

    pub fn indice_por_nome(&self, nome: &str) -> Option<usize> {
        self.indices.iter().position(|i| i.nome == nome)
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn paginas(&self) -> u64 {
        self.qtd_paginas
    }

    /// Leva tudo ao disco, e a ORDEM aqui e a garantia.
    ///
    /// Primeiro as paginas sujas e um `fsync` delas; so entao o cabecalho sem a
    /// marca de sujo, e outro `fsync`. Escrever o cabecalho limpo antes de as
    /// paginas estarem no disco abriria a janela exata que a marca existe para
    /// fechar: uma queda da maquina no meio deixaria "esta tudo bem" gravado
    /// por cima de uma arvore incompleta.
    ///
    /// Sao dois `fsync` por `sincronizar` -- que acontece uma vez por carga, e
    /// nao por linha. E e o UNICO caminho que grava o byte 52 em 0 (pedido
    /// 522): o `fechar` o deixa em 1.
    pub fn sincronizar(&mut self) -> Result<()> {
        // Antes da porta de baixo, e nao dentro dela (pedido 509): a porta
        // responde Ok sem tocar no disco, e depois de um `fsync` recusado
        // neste diretorio seria exatamente por ali que o fecho repetido
        // voltaria a dizer «sincronizado».
        crate::sincronia::conferir(&self.caminho)?;
        // A mesma porta do `fechar`, e ela faltava aqui (pedido 457): um
        // arquivo aberto ja sujo passava por este caminho e saia com o byte 52
        // em 0 sem ter sido reconstruido -- bastava um `atualizar` que nao
        // troca chave, que nem toca o indice, seguido do fecho da janela.
        if !self.pode_baixar_a_marca() {
            return Ok(());
        }
        self.descarregar()?;
        self.arquivo.flush()?;
        crate::sincronia::sync_all(&self.arquivo, &self.caminho)?;

        self.sujo = false;
        self.gravar_cabecalho()?;
        self.arquivo.flush()?;
        crate::sincronia::sync_all(&self.arquivo, &self.caminho)?;
        // O disco ja diz 0: o atestado nao tem mais o que atestar. Sai para o
        // registro do processo nao crescer com toda tabela que um dia fechou.
        retirar_atestado(&self.caminho);
        self.mudou_desde_o_fecho = false;
        Ok(())
    }

    /// Leva as paginas sujas ao nucleo e ATESTA o 1, SEM `fsync` -- e sem
    /// baixar a marca (pedido 522).
    ///
    /// # O que mudou, e por que
    ///
    /// Ate o pedido 522 este era o fechamento limpo: descarregava e gravava o
    /// byte 52 em 0, sem `fsync`, com o argumento de que o `write` ja tinha
    /// entregue tudo ao nucleo. Vale para a queda do PROCESSO; nao vale para a
    /// da MAQUINA nem para a escrita de fundo que o disco recusa: o nucleo
    /// grava as paginas na ordem dele, e o cabecalho e a pagina 0. Medido pelo
    /// papel C contra o SO (parecer 509+512, P4, 3/3): o nucleo guardou o
    /// cabecalho limpo e perdeu as paginas, e a abertura seguinte leu byte 52
    /// = 0 com `CRC invalido na pagina 3` e `precisa_reconstruir` falso.
    ///
    /// Agora o 0 so se grava depois dos dois `fsync` do `sincronizar`. Aqui as
    /// paginas vao ao nucleo, o cabecalho vai com a marca AINDA em 1 e os
    /// contadores em dia, e o processo guarda o atestado de que esse 1 e
    /// coerente no nucleo -- a proxima abertura AQUI confia nele. Um processo
    /// novo le o 1 sem atestado e manda reconstruir: e o preco, e ele e o
    /// certo, porque o processo novo nao sabe se a maquina caiu no meio.
    ///
    /// # O que nao mudou
    ///
    /// Queda do processo continua nao perdendo nada que ja passou por aqui --
    /// o `write` entregou ao nucleo. O que mudou foi o que o arquivo PROMETE
    /// depois disso.
    pub fn fechar(&mut self) -> Result<()> {
        // Um arquivo aberto JA sujo nao se limpa fechando: nada foi
        // reconstruido, e a arvore continua sem as chaves que faltam. So o
        // `reindexar`, que recria o arquivo, tira a marca -- senao bastaria
        // alguem abrir e fechar para o defeito virar invisivel. E a escrita
        // que nao terminou tambem nao se atesta fechando (pedido 456).
        if !self.pode_baixar_a_marca() {
            return Ok(());
        }
        self.descarregar()?;
        // O cabecalho vai quando algo mudou: a estrutura, ou os contadores
        // que toda escrita mexe. A marca vai como esta -- em 1 se alguma
        // escrita a levantou, e nunca de 1 para 0.
        let crc_antes = self.crc_do_cabecalho;
        if self.mudou_desde_o_fecho || self.estrutura_mudou {
            self.gravar_cabecalho()?;
        }
        if self.mudou_desde_o_fecho {
            // So agora, com a ultima pagina e o cabecalho no nucleo: antes
            // disto uma reabertura aqui leria arvore que ainda estava so em
            // RAM neste punho.
            if self.sujo {
                atestar(&self.caminho, self.crc_do_cabecalho);
            }
            self.mudou_desde_o_fecho = false;
        } else if self.sujo && self.crc_do_cabecalho != crc_antes {
            // Aberto atestado, e so o cabecalho mudou -- o `verificar` que
            // acerta um contador. O atestado acompanha o cabecalho novo, senao
            // a proxima abertura aqui o acharia diferente e mandaria
            // reconstruir uma arvore que ninguem tocou.
            trocar_atestado(&self.caminho, crc_antes, self.crc_do_cabecalho);
        }
        Ok(())
    }

    /// O punho vai ser trocado por um arquivo RECRIADO no mesmo caminho: daqui
    /// em diante ele nao leva nada ao arquivo, nem atesta nada.
    ///
    /// Existe por causa da ordem de uma atribuicao: `self.ndx =
    /// NdxFile::criar(..)` cria (e trunca) o arquivo novo ANTES de o velho
    /// sair, e o `Drop` do velho rodava o `fechar` por cima do arquivo novo --
    /// paginas e cabecalho de uma arvore que nao existe mais, e, desde o
    /// pedido 522, um atestado para ela. E o irmao do `reindexar`: quem
    /// chama as mesmas pecas na mesma ordem.
    pub fn abandonar(&mut self) {
        self.precisa_reconstruir = true;
    }

    /// A arvore em RAM pode ir ao disco marcada LIMPA -- ou, pelo `fechar`,
    /// ao nucleo com o atestado deste processo?
    ///
    /// Tres «nao», todos pelo mesmo motivo -- a arvore pode estar atras do
    /// `.reg` ou rasgada, e so o `reindexar` a conserta: aberta ja suja,
    /// escrita em voo (um panico no meio) e escrita interrompida (um erro no
    /// meio). Nos tres NADA desce: nem as paginas, que gravariam o estado do
    /// meio como se fosse o fim, nem a marca, que diria que ele presta, nem o
    /// atestado, que diria o mesmo a quem reabrir aqui.
    ///
    /// E o quarto, que e do DISCO e nao da arvore (pedido 509): um `fsync`
    /// recusado neste diretorio. Na prova do papel C quem recusou foi o
    /// `.log`, e este `Drop` gravou o cabecalho limpo por cima das paginas
    /// que o nucleo ja tinha perdido -- o byte 52 em 0 e `CRC invalido` depois
    /// de remontar. A arvore em RAM esta inteira; o que nao presta mais e a
    /// palavra do disco sobre ela.
    ///
    /// E um lugar so para `fechar`, `sincronizar` e o `Drop` -- os tres que
    /// baixam a marca. Foi a porta escrita em um e esquecida no irmao que
    /// abriu o pedido 457.
    fn pode_baixar_a_marca(&self) -> bool {
        !self.precisa_reconstruir
            && self.escritas_em_voo == 0
            && !self.escrita_interrompida
            && crate::sincronia::recusado_em(&self.caminho).is_none()
    }

    /// Abre uma escrita que vai por o `.reg` a frente desta arvore.
    ///
    /// Duas coisas, e as duas ANTES da primeira escrita de quem chama:
    ///
    /// 1. recusa se a arvore ja nao e confiavel. Recusar aqui, e nao na
    ///    primeira operacao de indice, e recusar antes de o `.reg` gravar --
    ///    depois, a recusa deixaria a linha no `.reg` e a chave fora;
    /// 2. sobe o byte 52 NO ARQUIVO (pedido 456, camada 1). A primeira pagina
    ///    suja ja subia, mas ela vem DEPOIS do `.reg`: um `SIGKILL` entre o
    ///    slot gravado e a primeira chave deixava a linha viva fora do indice
    ///    com o byte em 0. Quando a escrita suja pagina -- o caso de quem
    ///    chama isto --, a subida so muda de lugar: o cabecalho vai ao disco
    ///    uma vez, como ia.
    ///
    /// E liga o estado de escrita em voo, que so [`NdxFile::terminar_escrita`]
    /// desliga. Enquanto ligado, nem `fechar`, nem `sincronizar`, nem o `Drop`
    /// levam pagina ao disco ou baixam a marca.
    pub fn comecar_escrita(&mut self) -> Result<()> {
        self.conferir_confiavel()?;
        self.levantar_marca()?;
        self.escritas_em_voo += 1;
        Ok(())
    }

    /// Fecha a escrita aberta por [`NdxFile::comecar_escrita`].
    ///
    /// `em_dia` diz se a arvore e o `.reg` voltaram a concordar. Falso e a
    /// escrita que parou no meio com erro: a arvore passa a recusar ate o
    /// `reindexar`, e o byte 52 nao desce mais -- a proxima abertura manda
    /// reconstruir, que e o que ela faria depois de uma queda no mesmo ponto.
    ///
    /// Um panico nunca chega aqui, e e esse o desenho: o estado fica ligado
    /// sozinho, sem ninguem precisar perguntar se a thread esta desenrolando.
    pub fn terminar_escrita(&mut self, em_dia: bool) {
        self.escritas_em_voo = self.escritas_em_voo.saturating_sub(1);
        if !em_dia {
            self.escrita_interrompida = true;
        }
    }

    /// Esta tabela filha entra na cascata do `ao_alterar` -- ver o campo
    /// `cascata_em_voo`. So o `Drop` olha para isto.
    pub fn comecar_cascata(&mut self) {
        self.cascata_em_voo = true;
    }

    /// A cascata terminou nesta filha, ou parou com um erro que ja se disse.
    pub fn terminar_cascata(&mut self) {
        self.cascata_em_voo = false;
    }

    /// Roda uma mudanca da arvore com a escrita em voo ligada.
    ///
    /// Nao sobe a marca por conta propria: toda pagina suja ja passa por
    /// `gravar_pagina`, que sobe. O que isto acrescenta e o estado, e com ele
    /// o panico no meio de uma divisao de pagina deixa de ir ao disco.
    ///
    /// Erro ANTES de a arvore mudar e recusa -- a arvore continua inteira, e
    /// quem chama pode seguir. Erro DEPOIS e interrupcao, e a arvore para de
    /// responder: o despejo que falhou no meio de uma divisao ja perdeu uma
    /// pagina, e seguir respondendo seria responder errado.
    fn na_janela<T>(&mut self, mudar: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.escritas_em_voo += 1;
        let antes = self.mudancas_na_arvore;
        let r = mudar(self);
        let em_dia = r.is_ok() || self.mudancas_na_arvore == antes;
        self.terminar_escrita(em_dia);
        r
    }

    /// O arquivo foi aberto com a marca de sujo levantada.
    ///
    /// A arvore pode ter chave faltando; reconstrua com `reindexar` antes de
    /// confiar em qualquer resposta dela.
    /// O byte 52 esta em 1 no arquivo e so ESTE processo sabe que a arvore
    /// presta -- o atestado do `fechar`, ou a escrita deste punho. Um
    /// processo novo leria o mesmo arquivo e mandaria reconstruir.
    ///
    /// Existe para quem fecha e nao volta -- o `phx_tabela_fechar` do
    /// embutido (pedido 522, B2) -- saber se o `sincronizar` e o que deixa a
    /// tabela abrindo no proximo processo.
    pub fn marca_so_neste_processo(&self) -> bool {
        self.sujo && !self.precisa_reconstruir
    }

    pub fn precisa_reconstruir(&self) -> bool {
        self.precisa_reconstruir
    }

    /// Recusa operar sobre um indice que ficou para tras numa queda.
    fn conferir_confiavel(&self) -> Result<()> {
        if self.precisa_reconstruir {
            return Err(PhxError::Corrompido(format!(
                "o indice de {} ficou para tras numa queda e nao e confiavel: \
                 reconstrua com `reparar indice` antes de usar",
                self.caminho.display()
            )));
        }
        // O mesmo estado da queda, visto de dentro do processo: a escrita que
        // parou no meio deixou a arvore que a proxima abertura vai recusar.
        // Recusar ja, e nao so depois de reabrir, e o que impede um lote que
        // segue no erro de gravar as linhas seguintes numa arvore rasgada.
        if self.escrita_interrompida {
            return Err(PhxError::Corrompido(format!(
                "uma escrita no indice de {} parou no meio e a arvore nao e \
                 confiavel: reconstrua com `reparar indice` antes de usar",
                self.caminho.display()
            )));
        }
        Ok(())
    }

    /// Monta a chave completa: chave do usuario + rowid em big-endian.
    pub fn chave_completa(chave: &[u8], rowid: RowId) -> Vec<u8> {
        let mut ck = Vec::with_capacity(chave.len() + ROWID_LEN);
        ck.extend_from_slice(chave);
        ck.extend_from_slice(&rowid.to_be_bytes());
        ck
    }

    /// O descritor de um indice -- e o portao por onde toda operacao passa.
    ///
    /// A guarda de "ficou para tras numa queda" mora AQUI, e nao espalhada por
    /// `inserir`, `buscar`, `varrer`, `intervalo`, `remover` e `verificar`.
    /// Espalhada, a que alguem esquecesse viraria a porta dos fundos -- e a
    /// porta dos fundos de uma marca de confiabilidade e uma busca respondendo
    /// errado em silencio. E a mesma licao do portao de permissao.
    fn descritor(&self, idx: usize) -> Result<&DescritorIndice> {
        self.conferir_confiavel()?;
        self.indices
            .get(idx)
            .ok_or_else(|| PhxError::NaoEncontrado(format!("indice {idx} inexistente")))
    }

    // ------------------------------------------------------------ insercao

    /// Insere `chave` (codificada, sem rowid) apontando para `rowid`.
    /// Insere conferindo a unicidade antes.
    pub fn inserir(&mut self, idx: usize, chave: &[u8], rowid: RowId) -> Result<()> {
        if self.descritor(idx)?.unico && self.existe(idx, chave)? {
            let nome = self.descritor(idx)?.nome.clone();
            return Err(PhxError::Duplicado(format!(
                "indice unico {nome} ja tem essa chave"
            )));
        }
        self.inserir_ja_conferido(idx, chave, rowid)
    }

    /// Insere SEM conferir unicidade, para quem ja conferiu.
    ///
    /// Existe por medicao. A `Table` precisa conferir antes de gravar no
    /// `.reg` -- descobrir a duplicidade depois exigiria desfazer, e o slot
    /// desfeito ficaria morto para sempre, porque o `.reg` nao reaproveita
    /// slot. So que o `inserir` conferia de novo aqui dentro, e cada
    /// conferencia e uma descida inteira na arvore: duas descidas para
    /// responder a mesma pergunta, em toda insercao de todo indice unico.
    ///
    /// Quem chamar isto assume a conferencia. Chamar sem ter conferido mete
    /// chave repetida num indice unico, e o indice passa a mentir.
    pub fn inserir_ja_conferido(&mut self, idx: usize, chave: &[u8], rowid: RowId) -> Result<()> {
        let d = self.descritor(idx)?.clone();
        if chave.len() != d.key_len {
            return Err(PhxError::Corrompido(format!(
                "indice {}: chave de {} bytes, esperado {}",
                d.nome,
                chave.len(),
                d.key_len
            )));
        }

        let ck = Self::chave_completa(chave, rowid);
        self.na_janela(|n| n.inserir_na_arvore(idx, &d, &ck))
    }

    /// A descida e a subida da insercao, ja dentro da escrita em voo.
    fn inserir_na_arvore(&mut self, idx: usize, d: &DescritorIndice, ck: &[u8]) -> Result<()> {
        if let Some((promovida, nova)) = self.inserir_rec(d.raiz, ck, d.ck_len())? {
            let nova_raiz = self.alocar_pagina()?;
            let mut p = nova_pagina(self.page_size, TIPO_INTERNO);
            pag_set_qtd(&mut p, 1);
            let ck_len = d.ck_len();
            p[PAG_CAB..PAG_CAB + ck_len].copy_from_slice(&promovida);
            interno_set_filho(&mut p, 0, ck_len, d.raiz);
            pag_set_dir(&mut p, nova);
            self.gravar_pagina(nova_raiz, &mut p)?;
            self.indices[idx].raiz = nova_raiz;
            self.estrutura_mudou = true;
        }
        self.indices[idx].qtd_chaves += 1;
        // O contador nao justifica 4 KiB por chave: ele vai no `sincronizar`,
        // e `verificar` sabe recalcula-lo. A ESTRUTURA vai na hora.
        if self.estrutura_mudou {
            self.gravar_cabecalho()?;
        }
        Ok(())
    }

    /// Devolve `Some((chave_promovida, pagina_nova))` quando a pagina dividiu.
    fn inserir_rec(
        &mut self,
        pagina: u64,
        ck: &[u8],
        ck_len: usize,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let mut p = self.ler_pagina(pagina)?;
        match pag_tipo(&p) {
            TIPO_FOLHA => self.inserir_folha(pagina, &mut p, ck, ck_len),
            TIPO_INTERNO => {
                let pos = escolher_filho(&p, ck_len, ck);
                let filho = if pos < pag_qtd(&p) {
                    interno_filho(&p, pos, ck_len)
                } else {
                    pag_dir(&p)
                };
                match self.inserir_rec(filho, ck, ck_len)? {
                    None => Ok(None),
                    Some((promovida, nova)) => {
                        // Reler: a recursao pode ter mexido em outras paginas.
                        let mut p = self.ler_pagina(pagina)?;
                        self.inserir_interno(pagina, &mut p, pos, &promovida, nova, ck_len)
                    }
                }
            }
            outro => Err(PhxError::Corrompido(format!(
                "pagina {pagina} com tipo desconhecido {outro}"
            ))),
        }
    }

    fn inserir_folha(
        &mut self,
        pagina: u64,
        p: &mut [u8],
        ck: &[u8],
        ck_len: usize,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let qtd = pag_qtd(p);
        let pos = lower_bound_folha(p, ck_len, ck);
        if pos < qtd && folha_entrada(p, pos, ck_len) == ck {
            return Err(PhxError::Duplicado(
                "chave completa ja existe no indice".into(),
            ));
        }
        let cap = (self.corpo() - PAG_CAB) / ck_len;

        if qtd < cap {
            let inicio = PAG_CAB + pos * ck_len;
            let fim_dados = PAG_CAB + qtd * ck_len;
            p.copy_within(inicio..fim_dados, inicio + ck_len);
            p[inicio..inicio + ck_len].copy_from_slice(ck);
            pag_set_qtd(p, qtd + 1);
            self.gravar_pagina(pagina, p)?;
            return Ok(None);
        }

        // Divisao: monta a lista completa e reparte.
        let mut entradas: Vec<Vec<u8>> = (0..qtd)
            .map(|i| folha_entrada(p, i, ck_len).to_vec())
            .collect();
        entradas.insert(pos, ck.to_vec());
        let meio = entradas.len() / 2;

        let nova = self.alocar_pagina()?;
        let prox_antiga = pag_prox(p);

        let mut esq = nova_pagina(self.page_size, TIPO_FOLHA);
        pag_set_qtd(&mut esq, meio);
        pag_set_ant(&mut esq, pag_ant(p));
        pag_set_prox(&mut esq, nova);
        for (i, e) in entradas[..meio].iter().enumerate() {
            esq[PAG_CAB + i * ck_len..PAG_CAB + (i + 1) * ck_len].copy_from_slice(e);
        }

        let mut dir = nova_pagina(self.page_size, TIPO_FOLHA);
        pag_set_qtd(&mut dir, entradas.len() - meio);
        pag_set_ant(&mut dir, pagina);
        pag_set_prox(&mut dir, prox_antiga);
        for (i, e) in entradas[meio..].iter().enumerate() {
            dir[PAG_CAB + i * ck_len..PAG_CAB + (i + 1) * ck_len].copy_from_slice(e);
        }

        let promovida = entradas[meio].clone();
        self.gravar_pagina(pagina, &mut esq)?;
        // A metade esquerda ja esta no cache e a direita nao: a arvore em RAM
        // esta rasgada, com metade das chaves da folha fora dela.
        panico_de_teste::passar(panico_de_teste::Ponto::NoMeioDaDivisao);
        self.gravar_pagina(nova, &mut dir)?;

        // A folha seguinte passa a apontar para a nova como anterior.
        if prox_antiga != 0 {
            let mut seguinte = self.ler_pagina(prox_antiga)?;
            pag_set_ant(&mut seguinte, nova);
            self.gravar_pagina(prox_antiga, &mut seguinte)?;
        }

        Ok(Some((promovida, nova)))
    }

    fn inserir_interno(
        &mut self,
        pagina: u64,
        p: &mut [u8],
        pos: usize,
        promovida: &[u8],
        nova_filha: u64,
        ck_len: usize,
    ) -> Result<Option<(Vec<u8>, u64)>> {
        let qtd = pag_qtd(p);
        let ent = ck_len + 8;
        let cap = (self.corpo() - PAG_CAB) / ent;

        if qtd < cap {
            if pos < qtd {
                let filho_que_dividiu = interno_filho(p, pos, ck_len);
                let inicio = PAG_CAB + pos * ent;
                let fim = PAG_CAB + qtd * ent;
                p.copy_within(inicio..fim, inicio + ent);
                p[inicio..inicio + ck_len].copy_from_slice(promovida);
                interno_set_filho(p, pos, ck_len, filho_que_dividiu);
                interno_set_filho(p, pos + 1, ck_len, nova_filha);
            } else {
                let antiga_direita = pag_dir(p);
                let inicio = PAG_CAB + qtd * ent;
                p[inicio..inicio + ck_len].copy_from_slice(promovida);
                interno_set_filho(p, qtd, ck_len, antiga_direita);
                pag_set_dir(p, nova_filha);
            }
            pag_set_qtd(p, qtd + 1);
            self.gravar_pagina(pagina, p)?;
            return Ok(None);
        }

        // Monta a lista logica (chave, filho) ja com a nova entrada.
        let mut entradas: Vec<(Vec<u8>, u64)> = (0..qtd)
            .map(|i| {
                (
                    interno_chave(p, i, ck_len).to_vec(),
                    interno_filho(p, i, ck_len),
                )
            })
            .collect();
        let mut direita = pag_dir(p);

        if pos < qtd {
            let filho_que_dividiu = entradas[pos].1;
            entradas[pos].1 = nova_filha;
            entradas.insert(pos, (promovida.to_vec(), filho_que_dividiu));
        } else {
            entradas.push((promovida.to_vec(), direita));
            direita = nova_filha;
        }

        let meio = entradas.len() / 2;
        let (chave_promovida, filho_meio) = entradas[meio].clone();

        let mut esq = nova_pagina(self.page_size, TIPO_INTERNO);
        pag_set_qtd(&mut esq, meio);
        for (i, (k, f)) in entradas[..meio].iter().enumerate() {
            esq[PAG_CAB + i * ent..PAG_CAB + i * ent + ck_len].copy_from_slice(k);
            interno_set_filho(&mut esq, i, ck_len, *f);
        }
        pag_set_dir(&mut esq, filho_meio);

        let resto = &entradas[meio + 1..];
        let mut dirp = nova_pagina(self.page_size, TIPO_INTERNO);
        pag_set_qtd(&mut dirp, resto.len());
        for (i, (k, f)) in resto.iter().enumerate() {
            dirp[PAG_CAB + i * ent..PAG_CAB + i * ent + ck_len].copy_from_slice(k);
            interno_set_filho(&mut dirp, i, ck_len, *f);
        }
        pag_set_dir(&mut dirp, direita);

        let nova = self.alocar_pagina()?;
        self.gravar_pagina(pagina, &mut esq)?;
        self.gravar_pagina(nova, &mut dirp)?;
        Ok(Some((chave_promovida, nova)))
    }

    // ---------------------------------------------------- construcao em lote

    /// Monta a arvore inteira de um indice a partir das chaves, de uma vez.
    ///
    /// # Por que existe
    ///
    /// `inserir` desce a arvore uma vez por chave. Reconstruir um indice de um
    /// milhao de linhas assim custa um milhao de descidas -- exatamente o
    /// trabalho do caminho de dentro, so que de novo. E por isso que adiar o
    /// indice numa carga e reconstruir no fim comprava 1,02x: o `reindexar`
    /// pagava o mesmo preco em outro lugar.
    ///
    /// Aqui nao ha descida nenhuma. As chaves sao ordenadas, as folhas sao
    /// enchidas em SEQUENCIA, e os niveis de cima sao montados por cima dos de
    /// baixo. Cada pagina e escrita uma vez, na ordem do arquivo.
    ///
    /// # O que ele exige
    ///
    /// **O indice tem de estar vazio.** Isto e uma construcao, e nao um remendo
    /// numa arvore existente: aproveitar as paginas de uma arvore antiga pediria
    /// devolve-las a lista de livres uma a uma, e quem chama aqui (`reindexar`)
    /// acabou de truncar o arquivo. Recusar e melhor que vazar paginas em
    /// silencio.
    pub fn construir_em_lote(&mut self, idx: usize, chaves: Vec<u8>) -> Result<()> {
        self.construir_em_lote_com(idx, chaves, ENCHIMENTO_PADRAO)
    }

    /// O mesmo, com a regra da unicidade vinda de quem conhece o esquema.
    ///
    /// O `.ndx` sabe que o indice e unico, mas nao sabe onde comeca cada
    /// componente da chave -- e e disso que depende «esta chave tem NULL, e
    /// NULL nao colide». Quem sabe e a `Table`, e ela passa a MESMA pergunta
    /// que o `inserir` e o `atualizar` fazem (pedido 448, achado A4): sem
    /// isto, o `reindexar` recusava a tabela que o `inserir` tinha aceitado.
    pub fn construir_em_lote_participando(
        &mut self,
        idx: usize,
        chaves: Vec<u8>,
        participa: &dyn Fn(&[u8]) -> bool,
    ) -> Result<()> {
        let d = self.descritor(idx)?.clone();
        self.na_janela(|n| n.construir_com(idx, &d, chaves, ENCHIMENTO_PADRAO, participa))
    }

    /// O mesmo, escolhendo quanto de cada folha encher, em porcento.
    ///
    /// Existe separado porque o numero e uma troca medivel, e nao uma verdade:
    /// veja `ENCHIMENTO_PADRAO`. O medidor e `--example indice-em-lote`.
    pub fn construir_em_lote_com(
        &mut self,
        idx: usize,
        chaves: Vec<u8>,
        enchimento: usize,
    ) -> Result<()> {
        let d = self.descritor(idx)?.clone();
        // A construicao inteira e uma escrita em voo: um panico no meio dela
        // deixaria meia arvore, e o `Drop` a gravaria como o indice inteiro.
        self.na_janela(|n| n.construir(idx, &d, chaves, enchimento))
    }

    fn construir(
        &mut self,
        idx: usize,
        d: &DescritorIndice,
        chaves: Vec<u8>,
        enchimento: usize,
    ) -> Result<()> {
        self.construir_com(idx, d, chaves, enchimento, &|_| true)
    }

    fn construir_com(
        &mut self,
        idx: usize,
        d: &DescritorIndice,
        chaves: Vec<u8>,
        enchimento: usize,
        participa: &dyn Fn(&[u8]) -> bool,
    ) -> Result<()> {
        let ck_len = d.ck_len();
        if !(1..=100).contains(&enchimento) {
            return Err(PhxError::Esquema(format!(
                "enchimento {enchimento} invalido: use de 1 a 100 por cento"
            )));
        }
        if chaves.len() % ck_len != 0 {
            return Err(PhxError::Corrompido(format!(
                "indice {}: lote de {} bytes nao e multiplo da chave de {ck_len}",
                d.nome,
                chaves.len()
            )));
        }
        let total = chaves.len() / ck_len;
        if total > u32::MAX as usize {
            return Err(PhxError::Esquema(format!(
                "indice {}: {total} chaves passam do teto de {} por lote",
                d.nome,
                u32::MAX
            )));
        }

        // A arvore precisa estar vazia, e a folha que ela ja tem vira a
        // primeira do lote -- senao ela vazaria, sem entrar na lista de livres.
        let raiz_atual = self.ler_pagina(d.raiz)?;
        if pag_tipo(&raiz_atual) != TIPO_FOLHA || pag_qtd(&raiz_atual) != 0 {
            return Err(PhxError::Esquema(format!(
                "indice {}: construir em lote exige indice vazio",
                d.nome
            )));
        }
        if total == 0 {
            return Ok(()); // a folha vazia que ja esta la e a arvore certa
        }

        let em = |i: u32| {
            let a = i as usize * ck_len;
            &chaves[a..a + ck_len]
        };

        // Ordena uma PERMUTACAO, e nao as chaves: mover 4 bytes por troca em vez
        // da chave inteira, e sem uma alocacao por chave. Num indice de dez
        // milhoes isso e a diferenca entre ~200 MiB e mais de meio giga.
        //
        // A codificacao preserva ordem, entao ordenar os bytes e ordenar os
        // valores -- e o rowid no fim desempata chave repetida de indice nao
        // unico, o que torna a ordem total.
        let mut ordem: Vec<u32> = (0..total as u32).collect();
        ordem.sort_unstable_by(|a, b| em(*a).cmp(em(*b)));

        for par in ordem.windows(2) {
            let (x, y) = (em(par[0]), em(par[1]));
            if x == y {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: mesma chave completa duas vezes no lote",
                    d.nome
                )));
            }
            if d.unico && x[..d.key_len] == y[..d.key_len] && participa(&x[..d.key_len]) {
                return Err(PhxError::Duplicado(format!(
                    "indice unico {}: chave repetida no lote",
                    d.nome
                )));
            }
        }

        // ------------------------------------------------------------ folhas
        let cap_folha = (self.corpo() - PAG_CAB) / ck_len;
        let por_folha = (cap_folha * enchimento / 100).max(1);
        // Reparte em partes IGUAIS em vez de encher ate o teto e deixar o resto
        // na ultima: com 101 chaves e teto 100 sairiam 100 e 1, e a folha de uma
        // chave so divide na primeira insercao seguinte.
        let qtd_folhas = total.div_ceil(por_folha);

        // As paginas das folhas sao reservadas ANTES de escrever qualquer uma:
        // assim cada folha ja nasce sabendo o numero da seguinte, e nenhuma
        // precisa ser relida para ganhar o `prox`.
        let mut paginas = Vec::with_capacity(qtd_folhas);
        paginas.push(d.raiz);
        for _ in 1..qtd_folhas {
            paginas.push(self.alocar_pagina()?);
        }

        let mut filhos: Vec<(u64, Vec<u8>)> = Vec::with_capacity(qtd_folhas);
        let mut lida = 0usize;
        for f in 0..qtd_folhas {
            let quantas = fatia(total, qtd_folhas, f);
            let mut p = nova_pagina(self.page_size, TIPO_FOLHA);
            pag_set_qtd(&mut p, quantas);
            pag_set_ant(&mut p, if f == 0 { 0 } else { paginas[f - 1] });
            pag_set_prox(
                &mut p,
                if f + 1 < qtd_folhas {
                    paginas[f + 1]
                } else {
                    0
                },
            );
            for j in 0..quantas {
                let a = PAG_CAB + j * ck_len;
                p[a..a + ck_len].copy_from_slice(em(ordem[lida + j]));
            }
            filhos.push((paginas[f], em(ordem[lida]).to_vec()));
            self.gravar_pagina(paginas[f], &mut p)?;
            lida += quantas;
        }

        // ---------------------------------------------------- niveis de cima
        let ent = ck_len + 8;
        let cap_interno = (self.corpo() - PAG_CAB) / ent;
        let max_filhos = cap_interno + 1;

        while filhos.len() > 1 {
            let qtd_nos = filhos.len().div_ceil(max_filhos);
            let mut acima: Vec<(u64, Vec<u8>)> = Vec::with_capacity(qtd_nos);
            let mut lido = 0usize;
            for n in 0..qtd_nos {
                let quantos = fatia(filhos.len(), qtd_nos, n);
                let grupo = &filhos[lido..lido + quantos];
                let pagina = self.alocar_pagina()?;
                let mut p = nova_pagina(self.page_size, TIPO_INTERNO);
                // `escolher_filho` manda para `filho[i]` quem for MENOR que
                // `chave[i]`; entao a chave separadora e a primeira do filho
                // seguinte, que e o mesmo que a divisao promove.
                pag_set_qtd(&mut p, quantos - 1);
                for (i, (f, _)) in grupo[..quantos - 1].iter().enumerate() {
                    let a = PAG_CAB + i * ent;
                    p[a..a + ck_len].copy_from_slice(&grupo[i + 1].1);
                    interno_set_filho(&mut p, i, ck_len, *f);
                }
                pag_set_dir(&mut p, grupo[quantos - 1].0);
                self.gravar_pagina(pagina, &mut p)?;
                acima.push((pagina, grupo[0].1.clone()));
                lido += quantos;
            }
            filhos = acima;
        }

        self.indices[idx].raiz = filhos[0].0;
        self.indices[idx].qtd_chaves = total as u64;
        self.gravar_cabecalho()?;
        Ok(())
    }

    // -------------------------------------------------------------- busca

    /// Desce ate a folha que deve conter `alvo` e devolve (pagina, posicao).
    /// Desce ate a folha onde a chave entraria.
    ///
    /// Devolve a folha JUNTO com o numero e a posicao. Antes ela era lida aqui
    /// e jogada fora, e quem chamava lia de novo -- uma pagina inteira a mais
    /// por busca, com o CRC junto.
    fn descer(&mut self, raiz: u64, alvo: &[u8], ck_len: usize) -> Result<(u64, usize, Vec<u8>)> {
        let mut pagina = raiz;
        loop {
            let p = self.ler_pagina(pagina)?;
            match pag_tipo(&p) {
                TIPO_FOLHA => {
                    let pos = lower_bound_folha(&p, ck_len, alvo);
                    return Ok((pagina, pos, p));
                }
                TIPO_INTERNO => {
                    let pos = escolher_filho(&p, ck_len, alvo);
                    pagina = if pos < pag_qtd(&p) {
                        interno_filho(&p, pos, ck_len)
                    } else {
                        pag_dir(&p)
                    };
                }
                outro => {
                    return Err(PhxError::Corrompido(format!(
                        "pagina {pagina} com tipo desconhecido {outro}"
                    )))
                }
            }
        }
    }

    /// Ha ao menos uma entrada com esta chave?
    ///
    /// E o que a conferencia de unicidade precisa saber, e so isso. O `buscar`
    /// junta TODOS os rowids num vetor para depois alguem perguntar se o vetor
    /// esta vazio -- num indice unico a resposta cabe numa comparacao, e num
    /// indice comum juntar mil rowids para descartar os mil e trabalho jogado
    /// fora.
    pub fn existe(&mut self, idx: usize, chave: &[u8]) -> Result<bool> {
        let d = self.descritor(idx)?.clone();
        if chave.len() != d.key_len {
            return Err(PhxError::Corrompido(format!(
                "indice {}: chave de {} bytes, esperado {}",
                d.nome,
                chave.len(),
                d.key_len
            )));
        }
        let ck_len = d.ck_len();
        let inicio = Self::chave_completa(chave, 0);
        let (_pagina, pos, folha) = self.descer(d.raiz, &inicio, ck_len)?;

        // A chave pode cair exatamente no fim de uma folha: a primeira entrada
        // com esse prefixo estaria na folha seguinte.
        if pos < pag_qtd(&folha) {
            return Ok(folha_entrada(&folha, pos, ck_len)[..d.key_len] == *chave);
        }
        let proxima = pag_prox(&folha);
        if proxima == 0 {
            return Ok(false);
        }
        let p = self.ler_pagina(proxima)?;
        if pag_qtd(&p) == 0 {
            return Ok(false);
        }
        Ok(folha_entrada(&p, 0, ck_len)[..d.key_len] == *chave)
    }

    /// Todos os rowids cuja chave e exatamente `chave`.
    /// Como o rowid entra no fim da chave completa, o resultado sai ordenado
    /// por rowid, ou seja, na ordem de digitacao.
    pub fn buscar(&mut self, idx: usize, chave: &[u8]) -> Result<Vec<RowId>> {
        let d = self.descritor(idx)?.clone();
        if chave.len() != d.key_len {
            return Err(PhxError::Corrompido(format!(
                "indice {}: chave de {} bytes, esperado {}",
                d.nome,
                chave.len(),
                d.key_len
            )));
        }
        let inicio = Self::chave_completa(chave, 0);
        self.coletar(&d, &inicio, |e| &e[..d.key_len] == chave)
    }

    /// Rowids no intervalo `[de, ate]` (ambos opcionais, `ate` inclusivo).
    pub fn intervalo(
        &mut self,
        idx: usize,
        de: Option<&[u8]>,
        ate: Option<&[u8]>,
    ) -> Result<Vec<RowId>> {
        let d = self.descritor(idx)?.clone();
        let inicio = match de {
            Some(k) => Self::chave_completa(k, 0),
            None => vec![0u8; d.ck_len()],
        };
        let ate = ate.map(|k| k.to_vec());
        self.coletar(&d, &inicio, move |e| match &ate {
            Some(limite) => &e[..limite.len()] <= limite.as_slice(),
            None => true,
        })
    }

    /// Todos os rowids do indice, na ordem do indice.
    pub fn varrer(&mut self, idx: usize) -> Result<Vec<RowId>> {
        Ok(self.varrer_apos(idx, None, 0)?.0)
    }

    /// Quantas paginas do `.ndx` este arquivo TOCOU desde que foi aberto --
    /// acerto de cache e falta somados.
    ///
    /// Existe para a prova real do pedido 188, e o «somados» e o ponto: o que
    /// se quer medir e o TRABALHO de andar pela arvore, e uma varredura que
    /// visita mil paginas ja em RAM andou por mil paginas do mesmo jeito.
    /// Contar so as faltas mediria a RAM, que e outra pergunta -- e mediria
    /// zero na segunda corrida, fazendo uma guarda passar por engano.
    pub fn paginas_tocadas(&self) -> u64 {
        self.cache.acertos + self.cache.faltas
    }

    /// Um PEDACO da ordem do indice: ate `teto` rowids a partir de `apos`.
    ///
    /// # Por que a varredura ganhou um teto
    ///
    /// Porque quem lia o indice pedia sempre o indice INTEIRO, mesmo quando
    /// queria cinquenta linhas. Medido pelo fio numa tabela de 1.000.000, com
    /// a pagina de 50: a grade ordenada custava **54,81 ms** e tocava **8.335
    /// paginas** do `.ndx` (32,56 MiB) para devolver 50 linhas, contra **3**
    /// paginas do minimo que a mesma pergunta exige. Ver o pedido 188 e
    /// `--example o-que-a-grade-ordenada-custa`.
    ///
    /// # O cursor e a CHAVE COMPLETA, e nao a pagina
    ///
    /// `apos` e a ultima chave completa devolvida, e a proxima chamada desce a
    /// arvore de novo por ela. Guardar `(pagina, posicao)` seria mais barato
    /// -- uma descida a menos -- e seria um ponteiro para dentro de uma
    /// estrutura que a proxima escrita reorganiza. A chave completa carrega o
    /// rowid no fim, entao ela e unica e sobrevive a divisao de folha.
    ///
    /// Devolve os rowids, a ULTIMA chave completa lida (por onde continuar) e
    /// se o indice ACABOU. `teto` zero quer dizer sem teto.
    pub fn varrer_apos(
        &mut self,
        idx: usize,
        apos: Option<&[u8]>,
        teto: usize,
    ) -> Result<(Vec<RowId>, Option<Vec<u8>>, bool)> {
        let d = self.descritor(idx)?.clone();
        let ck_len = d.ck_len();
        let inicio = match apos {
            Some(ck) => ck.to_vec(),
            None => vec![0u8; ck_len],
        };
        let (mut pagina, mut pos, mut folha) = self.descer(d.raiz, &inicio, ck_len)?;
        // `descer` para NA entrada de `apos`, que ja foi devolvida na chamada
        // anterior. Sem este passo o cursor devolveria a mesma linha para
        // sempre, e a pagina nunca encheria.
        if apos.is_some() {
            pos += 1;
        }
        let mut saida = Vec::new();
        while pagina != 0 {
            // A primeira folha vem da descida; as seguintes se leem aqui.
            let p = folha;
            let qtd = pag_qtd(&p);
            while pos < qtd {
                let e = folha_entrada(&p, pos, ck_len);
                saida.push(u64::from_be_bytes(e[d.key_len..].try_into().unwrap()));
                pos += 1;
                if teto > 0 && saida.len() >= teto {
                    return Ok((saida, Some(e.to_vec()), false));
                }
            }
            pagina = pag_prox(&p);
            pos = 0;
            if pagina == 0 {
                break;
            }
            folha = self.ler_pagina(pagina)?;
        }
        Ok((saida, None, true))
    }

    fn coletar<F>(
        &mut self,
        d: &DescritorIndice,
        inicio: &[u8],
        mut aceita: F,
    ) -> Result<Vec<RowId>>
    where
        F: FnMut(&[u8]) -> bool,
    {
        let ck_len = d.ck_len();
        let (mut pagina, mut pos, mut folha) = self.descer(d.raiz, inicio, ck_len)?;
        let mut saida = Vec::new();
        while pagina != 0 {
            // A primeira folha vem da descida; as seguintes se leem aqui.
            let p = folha;
            let qtd = pag_qtd(&p);
            while pos < qtd {
                let e = folha_entrada(&p, pos, ck_len);
                if !aceita(e) {
                    return Ok(saida);
                }
                let rowid = u64::from_be_bytes(e[d.key_len..].try_into().unwrap());
                saida.push(rowid);
                pos += 1;
            }
            pagina = pag_prox(&p);
            pos = 0;
            if pagina == 0 {
                break;
            }
            folha = self.ler_pagina(pagina)?;
        }
        Ok(saida)
    }

    // ------------------------------------------------------------ remocao

    /// Remove a entrada (`chave`, `rowid`). Devolve `false` se nao existia.
    pub fn remover(&mut self, idx: usize, chave: &[u8], rowid: RowId) -> Result<bool> {
        let d = self.descritor(idx)?.clone();
        self.na_janela(|n| n.remover_da_arvore(idx, &d, chave, rowid))
    }

    fn remover_da_arvore(
        &mut self,
        idx: usize,
        d: &DescritorIndice,
        chave: &[u8],
        rowid: RowId,
    ) -> Result<bool> {
        let ck_len = d.ck_len();
        let ck = Self::chave_completa(chave, rowid);
        let (pagina, pos, mut p) = self.descer(d.raiz, &ck, ck_len)?;
        let qtd = pag_qtd(&p);
        if pos >= qtd || folha_entrada(&p, pos, ck_len) != ck.as_slice() {
            return Ok(false);
        }
        let inicio = PAG_CAB + pos * ck_len;
        let fim = PAG_CAB + qtd * ck_len;
        p.copy_within(inicio + ck_len..fim, inicio);
        p[fim - ck_len..fim].fill(0);
        pag_set_qtd(&mut p, qtd - 1);
        self.gravar_pagina(pagina, &mut p)?;
        self.indices[idx].qtd_chaves = self.indices[idx].qtd_chaves.saturating_sub(1);
        self.gravar_cabecalho()?;
        Ok(true)
    }

    // --------------------------------------------------------- verificacao

    /// Confere CRC de todas as paginas e a ordenacao das folhas de cada
    /// indice. Devolve a quantidade de chaves encontrada por indice.
    pub fn verificar(&mut self) -> Result<Vec<(String, u64)>> {
        // Esta nao passa por `descritor`: le `self.indices[i]` direto.
        self.conferir_confiavel()?;
        let mut saida = Vec::new();
        for i in 0..self.indices.len() {
            let d = self.indices[i].clone();
            let ck_len = d.ck_len();
            let inicio = vec![0u8; ck_len];
            let (mut pagina, _, _) = self.descer(d.raiz, &inicio, ck_len)?;
            let mut anterior: Option<Vec<u8>> = None;
            let mut total = 0u64;
            while pagina != 0 {
                let p = self.ler_pagina(pagina)?; // ja confere CRC
                for j in 0..pag_qtd(&p) {
                    let e = folha_entrada(&p, j, ck_len);
                    if let Some(ant) = &anterior {
                        if e <= ant.as_slice() {
                            return Err(PhxError::Corrompido(format!(
                                "indice {} fora de ordem na pagina {pagina}",
                                d.nome
                            )));
                        }
                    }
                    anterior = Some(e.to_vec());
                    total += 1;
                }
                pagina = pag_prox(&p);
            }
            // Os dois sentidos NAO sao a mesma coisa, e confundi-los faria
            // esta conferencia gritar corrupcao numa arvore sadia.
            //
            // Varredura MAIOR que o contador: o contador ficou para tras, que e
            // o que uma queda entre dois `sincronizar` deixa -- a arvore tem
            // todas as chaves, e o numero e que esta velho. Conserta-se.
            //
            // Varredura MENOR: falta chave na arvore. Isso e corrupcao, e
            // continua parando aqui.
            if total < d.qtd_chaves {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: diretorio diz {} chaves e a varredura achou so {total}",
                    d.nome, d.qtd_chaves
                )));
            }
            if total > d.qtd_chaves {
                self.indices[i].qtd_chaves = total;
                self.estrutura_mudou = true;
            }
            saida.push((d.nome, total));
        }
        Ok(saida)
    }
}

impl Drop for NdxFile {
    /// Rede de seguranca do fechamento limpo -- e SO do limpo.
    ///
    /// Quem esquecer de chamar `fechar` ou `sincronizar` ainda tem as paginas
    /// levadas ao arquivo aqui. E se a gravacao FALHAR, a marca de sujo fica
    /// levantada -- que e a resposta certa: o indice realmente nao presta, e a
    /// proxima abertura vai dizer isso em vez de responder errado.
    ///
    /// Com uma escrita em voo -- o `Drop` que roda no desenrolar de um panico,
    /// ou o do punho do FFI liberado depois de um --, o `fechar` nao leva nada
    /// ao disco (pedido 456): o disco fica exatamente como um `SIGKILL` no
    /// mesmo ponto o deixaria, com o byte 52 em 1, e a proxima abertura manda
    /// reconstruir.
    ///
    /// Com a cascata do `ao_alterar` em voo (pedido 490), o mesmo: a arvore
    /// daqui esta inteira, mas a cascata nao, e o disco tem de dizer isso. O
    /// byte SOBE aqui, porque entre duas linhas da cascata ele pode estar em
    /// 0 -- o `sincronizar` que a neta precisa o baixou --, e sem subir o
    /// panico continuaria mais calado que a queda.
    fn drop(&mut self) {
        if self.cascata_em_voo {
            self.escrita_interrompida = true;
            let _ = self.levantar_marca();
        }
        let _ = self.fechar();
        // Os contadores deste arquivo entram na conta do processo aqui, e nao
        // a cada toque de pagina: ver a nota em `contadores_de_cache`.
        let o = std::sync::atomic::Ordering::Relaxed;
        CACHE_ACERTOS.fetch_add(self.cache.acertos, o);
        CACHE_FALTAS.fetch_add(self.cache.faltas, o);
        CACHE_GRAVACOES.fetch_add(self.gravacoes, o);
    }
}

/// Pontos nomeados onde um TESTE manda a escrita entrar em panico.
///
/// Existe para a prova do pedido 456 conferir o DISCO depois de um panico de
/// verdade no meio de uma escrita, e nao o veredito de uma funcao: o defeito
/// era o `Drop` gravar o estado do meio, e so um panico no meio o mostra.
///
/// # Por que `debug_assertions`, e nao `cfg(test)`
///
/// `cfg(test)` so vale dentro do proprio crate, e a prova tambem passa pelo
/// FFI, que compila este crate como dependencia comum. Em `release` o gancho
/// nao existe: `passar` vira uma funcao vazia e o laco quente nao paga nem a
/// leitura da variavel da thread -- instrumentacao desligada custa zero.
///
/// A arma e POR THREAD: os testes rodam em paralelo, e um gancho global
/// derrubaria a escrita de um teste vizinho.
#[doc(hidden)]
pub mod panico_de_teste {
    /// Onde o panico acontece. Cada um e um estado do meio que o `Drop` de
    /// antes gravava como se fosse o fim.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Ponto {
        /// `NdxFile::inserir_folha`: a metade esquerda da folha dividida ja
        /// esta no cache; a direita e o pai, nao.
        NoMeioDaDivisao,
        /// `Table::inserir`: o slot e o contador do `.reg` ja gravados, e
        /// nenhuma chave no `.ndx`.
        InserirDepoisDoContador,
        /// `Table::atualizar` e o irmao `marcar`: o `.reg` com a linha nova, e
        /// o `.ndx` com a chave velha.
        AtualizarDepoisDoReg,
        /// `Table::excluir_de_vez`: as chaves ja sairam do `.ndx`, e o slot
        /// continua vivo no `.reg`.
        ExcluirEntreRemoverEExcluir,
        /// `Table::reindexar`: o `.ndx` ja recriado VAZIO, e o `.reg` varrido
        /// sem nenhuma arvore montada ainda.
        NoMeioDoReindexar,
        /// A cascata do `ao_alterar` (pedido 490): a mae ja gravada na chave
        /// nova, a filha `r` ja acompanhou, e a `r + 1` ainda nao.
        CascataEntreFilhas,
        /// O irmao do de cima: a mae ja gravada na chave nova, e o texto, o
        /// diario e a trilha dela ainda por fazer -- nenhuma filha acompanhou.
        CascataDepoisDaMae,
    }

    #[cfg(debug_assertions)]
    thread_local! {
        static ARMADO: std::cell::Cell<Option<Ponto>> = const { std::cell::Cell::new(None) };
        static PAUSA: std::cell::RefCell<Option<(Ponto, u32, String)>> =
            const { std::cell::RefCell::new(None) };
    }

    /// Arma o ponto NESTA thread. Dispara uma vez so, e desarma sozinho.
    pub fn armar(p: Ponto) {
        #[cfg(debug_assertions)]
        ARMADO.with(|a| a.set(Some(p)));
        #[cfg(not(debug_assertions))]
        let _ = p;
    }

    /// Arma uma PAUSA SEM FIM na `n`-esima passagem por `p`, NESTA thread
    /// (pedido 540): a thread diz `aviso` no erro padrao e para para sempre,
    /// com tudo o que tiver na mao -- a trava de dados inclusive.
    ///
    /// E o processo PARADO no meio da escrita, que a prova de `SIGKILL` mata.
    /// O panico nao serve para ela: o desenrolar roda o reparo da trava, e o
    /// que se prova ali e a queda SEM desenrolar nenhum. A contagem existe
    /// porque o ponto e o mesmo para a mae e para cada filha da cascata, e o
    /// estado que importa e o do meio -- uma filha gravada, a outra nao.
    ///
    /// O aviso vai pelo erro padrao, e a parada e `park`, de proposito: o
    /// mapa da trava (`bancada/concorrencia/mapa-da-trava.py`) le este
    /// arquivo como codigo de producao -- ele so separa `cfg(test)`, e este
    /// gancho e `debug_assertions` porque o `cfg(test)` nao atravessa crate --,
    /// e um `sleep` ou um `fs::write` aqui entrariam em toda secao que grava
    /// uma linha. Em `release` o `passar` e vazio e nada disto existe.
    pub fn armar_pausa(p: Ponto, n: u32, aviso: &str) {
        #[cfg(debug_assertions)]
        PAUSA.with(|a| *a.borrow_mut() = Some((p, n.max(1), aviso.to_string())));
        #[cfg(not(debug_assertions))]
        let _ = (p, n, aviso);
    }

    /// Desarma, para o teste cujo panico nao aconteceu nao contaminar o
    /// seguinte na mesma thread.
    pub fn desarmar() {
        #[cfg(debug_assertions)]
        {
            ARMADO.with(|a| a.set(None));
            PAUSA.with(|a| *a.borrow_mut() = None);
        }
    }

    /// O ponto de passagem, no caminho de producao.
    #[inline(always)]
    pub(crate) fn passar(p: Ponto) {
        #[cfg(debug_assertions)]
        {
            if ARMADO.with(|a| a.get()) == Some(p) {
                ARMADO.with(|a| a.set(None));
                panic!("panico de teste em {p:?}");
            }
            let parar = PAUSA.with(|a| {
                let mut a = a.borrow_mut();
                match a.as_mut() {
                    Some((q, n, _)) if *q == p && *n > 1 => {
                        *n -= 1;
                        None
                    }
                    Some((q, _, _)) if *q == p => a.take().map(|(_, _, aviso)| aviso),
                    _ => None,
                }
            });
            if let Some(aviso) = parar {
                eprintln!("{aviso}");
                loop {
                    std::thread::park();
                }
            }
        }
        #[cfg(not(debug_assertions))]
        let _ = p;
    }
}
