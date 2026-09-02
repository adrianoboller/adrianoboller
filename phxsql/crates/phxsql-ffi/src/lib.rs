//! # phxsql-ffi -- o PhxSql embutido, por ABI de C
//!
//! O `phxsql-store` **ja e** o banco embutido: ele cria tabela, insere, busca
//! por indice e varre sem soquete nenhum. O `phxsql-server` e um envelope de
//! rede em volta dele. Esta crate nao reescreve motor: **abre uma porta de
//! entrada nova** para o mesmo motor, na unica ABI que serve ao Android
//! (JNI), ao iOS (Swift/ObjC), ao Flutter, ao .NET e ao Python -- a de C.
//!
//! ```text
//!    app ──▶ phxsql-ffi ──▶ phxsql-store
//!            (ABI de C)       (motor)
//! ```
//!
//! # Por que biblioteca, e nao um "mini servidor" no aparelho
//!
//! O iOS **nao permite** processo de longa duracao em segundo plano nem app
//! escutando porta para outros apps; o Android **mata** processo em segundo
//! plano com liberdade. Um daemon com porta nao e dificil nesses dois: e
//! contra a forma do sistema. O desenho e o motivo estao em
//! `docs/EMBUTIDO.md`.
//!
//! # As regras desta fronteira, em uma tela
//!
//! * **Nenhum panico atravessa.** Toda funcao exportada passa por
//!   `punho::blindado` ou `punho::com`; um panico capturado ENVENENA o punho,
//!   porque capturar salva o processo e nao conserta o objeto.
//! * **Erro volta em codigo de retorno**, com a mensagem numa vaga por
//!   thread (`phx_ultimo_erro`). Os codigos sao os mesmos do `PhxError` --
//!   3002 e chave duplicada aqui e na porta de dados.
//! * **Quem alocou, libera.** A biblioteca nunca devolve ponteiro para o
//!   chamador chamar `free()`: em `.dll` do Windows isso derruba o processo.
//! * **Texto e UTF-8 com tamanho explicito.** Nunca `NUL`-terminado, porque
//!   dado de cliente tem byte zero e `strlen` o trunca em silencio.
//! * **Um punho, uma thread por vez.** A vaga de erro e por thread; o motor
//!   nao tem trava propria, e esta camada nao inventa uma escondida.

pub mod erro;
pub mod linha;
pub mod punho;
pub mod texto;
pub mod valor;

use std::sync::atomic::{AtomicU64, Ordering};

use phxsql_core::error::Result as PhxResult;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::value::Value;
use phxsql_core::RowId;
use phxsql_store::catalogo::{Database, Instancia};
use phxsql_store::log::Operacao;
use phxsql_store::table::Visao;
use phxsql_store::Table;

use erro::{anotar, do_motor, PHX_ERRO_PONTEIRO, PHX_ERRO_USO, PHX_NAO_HA, PHX_OK};
use linha::LinhaFFI;
use punho::{
    blindado, blindado_cru, com, conferir, liberar, Punho, ETIQ_BASE, ETIQ_CURSOR, ETIQ_ESQUEMA,
    ETIQ_IMAGEM, ETIQ_LINHA, ETIQ_TABELA,
};
use valor::PhxValor;

// ---------------------------------------------------------------- bandeiras

/// `phx_base_abrir`: cria o database se ele ainda nao existir.
pub const PHX_CRIAR: u32 = 1;

/// `phx_esquema_coluna`: a coluna nao aceita nulo.
pub const PHX_COL_OBRIGATORIA: u32 = 1;

/// `phx_esquema_indice`: chave unica / chave primaria.
pub const PHX_IDX_UNICO: u32 = 1;
pub const PHX_IDX_PRIMARIA: u32 = 2;

/// `phx_esquema_indice_coluna`: ordem decrescente / sem distinguir caixa.
pub const PHX_IDX_DESC: u32 = 1;
pub const PHX_IDX_SEM_CAIXA: u32 = 2;

/// O que uma varredura enxerga.
pub const PHX_VISAO_ATIVAS: u32 = 0;
pub const PHX_VISAO_EXCLUIDAS: u32 = 1;
pub const PHX_VISAO_TODAS: u32 = 2;

/// As operacoes do diario. Os numeros sao os mesmos que o `.log` grava.
pub const PHX_OP_INCLUSAO: u32 = 1;
pub const PHX_OP_ALTERACAO: u32 = 2;
pub const PHX_OP_EXCLUSAO: u32 = 3;

/// Quantos rowids o cursor de digitacao busca por vez.
///
/// O cursor NAO materializa a tabela inteira: ele anda por `pagina_depois_de`,
/// o keyset do PhxSql, em que continuar depois do rowid 500.000 e uma conta e
/// nao uma procura. Num aparelho pequeno a diferenca entre isto e um vetor de
/// um milhao de rowids e o app ser morto pelo sistema.
const LOTE_CURSOR: u64 = 256;

// ------------------------------------------------------------------- punhos

/// Numero de serie de cada tabela aberta.
///
/// E o que faz o cursor conseguir recusar a tabela errada sem guardar
/// ponteiro para ela -- ver `phx_cursor_proximo`.
static SERIE: AtomicU64 = AtomicU64::new(1);

pub struct BaseFFI {
    db: Database,
    /// Lista da ultima chamada de `phx_base_tabelas_qtd`. O padrao de duas
    /// chamadas existe para nao alocar do lado do C; releitura do diretorio a
    /// cada nome seria varrer o disco N vezes para listar N tabelas.
    tabelas: Vec<String>,
}

#[derive(Default)]
pub struct EsquemaFFI {
    nome: String,
    colunas: Vec<Column>,
    indices: Vec<IndexDef>,
}

pub struct TabelaFFI {
    t: Table,
    serie: u64,
}

pub struct CursorFFI {
    serie: u64,
    visao: Visao,
    /// Rowids ja buscados e ainda nao entregues.
    lote: Vec<RowId>,
    pos: usize,
    /// O ultimo rowid entregue -- de onde a proxima pagina continua.
    ultimo: RowId,
    /// Cursor de indice ja materializou tudo: nao ha o que buscar de novo.
    materializado: bool,
    esgotado: bool,
}

pub struct ImagemFFI {
    bytes: Vec<u8>,
}

/// O que `phx_verificar` devolve.
///
/// Struct simples e nao punho: sao dez numeros de tamanho conhecido, e obrigar
/// o chamador a liberar um punho para ler dez `u64` seria cobrar um `free`
/// sem motivo.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PhxRelatorio {
    pub registros: u64,
    pub slots: u64,
    pub marcadas: u64,
    pub eventos: u64,
    pub descartadas: u64,
    pub motivos: u64,
    pub indices: u64,
    pub trilha: u64,
}

/// Um evento do diario, do jeito que o C le.
///
/// Os campos estao em ordem decrescente de alinhamento de proposito: assim a
/// struct nao ganha enchimento e o mesmo layout sai em 32 e em 64 bits.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PhxEvento {
    /// Milissegundos desde 1970-01-01T00:00:00Z.
    pub carimbo: i64,
    pub rowid: u64,
    pub versao: u64,
    pub operacao: u32,
    pub usuario: u32,
    /// De que servidor a escrita NASCEU. Zero = local. E o que mata o laco
    /// infinito do bidirecional.
    pub origem: u32,
    pub tam_imagem: u32,
}

// ------------------------------------------------------------------ auxilio

/// Escreve um numero no ponteiro de saida do chamador, se ele deu um.
///
/// # Safety
///
/// `p` tem de ser nulo ou apontar para um `T` gravavel.
unsafe fn saida<T>(p: *mut T, valor: T) {
    if !p.is_null() {
        std::ptr::write(p, valor);
    }
}

/// Traduz um `Result` do motor num codigo de retorno.
fn resultado<T>(r: PhxResult<T>, ok: impl FnOnce(T) -> i32) -> i32 {
    match r {
        Ok(v) => ok(v),
        Err(e) => do_motor(&e),
    }
}

/// Igual ao [`resultado`], mas «esse rowid nao existe» vira `PHX_NAO_HA` em
/// vez de erro.
///
/// # Por que so aqui, e por que e preciso
///
/// Perguntar pelo rowid 9999 tem DUAS respostas dentro do motor: slot livre
/// devolve `Ok(None)`, e rowid alem do fim devolve `NaoEncontrado`. A
/// diferenca e real la dentro -- e para quem chamou e a mesma frase, «nao ha
/// essa linha». Deixar as duas atravessarem faria o aplicativo mostrar caixa
/// vermelha para metade dos casos e lista vazia para a outra metade, sem
/// nenhum criterio que o programador dele consiga enxergar.
///
/// A dobra e SO nas funcoes enderecadas por rowid. No `phx_buscar` o mesmo
/// 3001 quer dizer «esse indice nao existe», que e defeito de quem chamou e
/// tem de doer.
fn resultado_do_rowid<T>(r: PhxResult<T>, ok: impl FnOnce(T) -> i32) -> i32 {
    match r {
        Ok(v) => ok(v),
        Err(phxsql_core::error::PhxError::NaoEncontrado(_)) => PHX_NAO_HA,
        Err(e) => do_motor(&e),
    }
}

fn visao_de(v: u32) -> Result<Visao, i32> {
    Ok(match v {
        PHX_VISAO_ATIVAS => Visao::Ativas,
        PHX_VISAO_EXCLUIDAS => Visao::Excluidas,
        PHX_VISAO_TODAS => Visao::Todas,
        outro => return Err(anotar(PHX_ERRO_USO, format!("visao desconhecida: {outro}"))),
    })
}

/// Le o vetor de valores que o C entregou, copiando cada um.
///
/// # Safety
///
/// `v` tem de apontar para `qtd` `PhxValor` legiveis, com os `dados` de cada
/// um validos.
unsafe fn valores(v: *const PhxValor, qtd: usize) -> Result<Vec<Value>, i32> {
    if qtd == 0 {
        return Ok(Vec::new());
    }
    if v.is_null() {
        return Err(anotar(PHX_ERRO_PONTEIRO, "vetor de valores nulo"));
    }
    // A reserva vem ANTES de formar a fatia, e isso e a ordem que importa:
    // uma contagem absurda vinda do C estoura AQUI, num panico que a fronteira
    // captura e devolve como erro. Na ordem contraria ela viraria primeiro uma
    // fatia de tamanho impossivel -- comportamento indefinido, que nao da erro
    // nenhum e derruba o aplicativo do cliente em outro lugar.
    let mut fora = Vec::with_capacity(qtd);
    let fatia = std::slice::from_raw_parts(v, qtd);
    for item in fatia {
        fora.push(valor::para_value(item)?);
    }
    Ok(fora)
}

// ========================================================================
//  Casa
// ========================================================================

/// A versao do motor, em texto.
///
/// # Safety
///
/// `destino` aponta para `cap` bytes gravaveis (ou `cap` e zero); `precisa` e
/// nulo ou aponta para um `usize`.
#[no_mangle]
pub unsafe extern "C" fn phx_versao(destino: *mut u8, cap: usize, precisa: *mut usize) -> i32 {
    blindado(|| texto::escrever(destino, cap, precisa, env!("CARGO_PKG_VERSION")))
}

/// A mensagem do ultimo erro **desta thread**. Vazia quando nada falhou.
///
/// # Safety
///
/// Mesmo contrato de [`phx_versao`].
#[no_mangle]
pub unsafe extern "C" fn phx_ultimo_erro(destino: *mut u8, cap: usize, precisa: *mut usize) -> i32 {
    // `blindado_cru`, e nao `blindado`: limpar a vaga aqui apagaria justamente
    // a mensagem que esta chamada existe para ler.
    blindado_cru(|| {
        let m = erro::ultimo();
        texto::escrever(destino, cap, precisa, &m)
    })
}

/// O nome simbolico de um codigo (`"DUPLICADO"`), para diagnostico legivel.
///
/// O ponteiro e estatico e `NUL`-terminado: e literal do binario, nunca se
/// libera. E a unica string da ABI com essa forma, e por um motivo -- ela nao
/// carrega dado de cliente, entao nao tem byte zero no meio.
#[no_mangle]
pub extern "C" fn phx_erro_nome(codigo: i32) -> *const u8 {
    // Sem blindagem: e um `match` sobre literais, nao ha o que entrar em
    // panico, e devolver ponteiro por um caminho de erro seria pior.
    erro::nome_do_codigo_c(codigo).as_ptr()
}

// ========================================================================
//  Base
// ========================================================================

/// Abre a raiz de dados e um database dentro dela.
///
/// Com `PHX_CRIAR` o database nasce se faltar -- que e o caso do primeiro
/// arranque do aplicativo no aparelho.
///
/// # Safety
///
/// Os pares `(ponteiro, tamanho)` tem de descrever memoria legivel; `saida`
/// aponta para um ponteiro gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_base_abrir(
    caminho: *const u8,
    caminho_tam: usize,
    nome: *const u8,
    nome_tam: usize,
    sinalizadores: u32,
    saida_base: *mut *mut Punho<BaseFFI>,
) -> i32 {
    blindado(|| {
        saida(saida_base, std::ptr::null_mut());
        if saida_base.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_base_abrir");
        }
        let caminho = match texto::texto(caminho, caminho_tam) {
            Ok(c) => c,
            Err(e) => return e,
        };
        let nome = match texto::texto(nome, nome_tam) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let inst = match Instancia::nova(caminho) {
            Ok(i) => i,
            Err(e) => return do_motor(&e),
        };
        let db = if sinalizadores & PHX_CRIAR != 0 {
            inst.garantir_database(nome)
        } else {
            inst.abrir_database(nome)
        };
        resultado(db, |db| {
            saida(
                saida_base,
                Punho::novo(
                    ETIQ_BASE,
                    BaseFFI {
                        db,
                        tabelas: Vec::new(),
                    },
                ),
            );
            PHX_OK
        })
    })
}

/// Libera a base. Tabelas abertas dela continuam validas -- cada `Table`
/// carrega os proprios arquivos.
///
/// # Safety
///
/// `p` e nulo ou um punho de base ainda nao liberado.
#[no_mangle]
pub unsafe extern "C" fn phx_base_fechar(p: *mut Punho<BaseFFI>) -> i32 {
    liberar(p, ETIQ_BASE)
}

/// Relista as tabelas e devolve quantas sao. Chame antes de
/// [`phx_base_tabela_nome`].
///
/// # Safety
///
/// `p` e um punho de base valido; `qtd` e nulo ou aponta para um `usize`.
#[no_mangle]
pub unsafe extern "C" fn phx_base_tabelas_qtd(p: *mut Punho<BaseFFI>, qtd: *mut usize) -> i32 {
    com(p, ETIQ_BASE, |b| {
        resultado(b.db.todas_as_tabelas(), |lista| {
            b.tabelas = lista;
            saida(qtd, b.tabelas.len());
            PHX_OK
        })
    })
}

/// O nome da i-esima tabela da ultima listagem.
///
/// # Safety
///
/// `p` e um punho de base valido; `destino`/`cap`/`precisa` como em
/// [`phx_versao`].
#[no_mangle]
pub unsafe extern "C" fn phx_base_tabela_nome(
    p: *mut Punho<BaseFFI>,
    i: usize,
    destino: *mut u8,
    cap: usize,
    precisa: *mut usize,
) -> i32 {
    com(p, ETIQ_BASE, |b| match b.tabelas.get(i) {
        Some(n) => texto::escrever(destino, cap, precisa, n),
        None => anotar(
            PHX_ERRO_USO,
            format!(
                "tabela {i} fora da faixa: a ultima listagem tinha {}",
                b.tabelas.len()
            ),
        ),
    })
}

// ========================================================================
//  Esquema
// ========================================================================

/// Comeca a montar um esquema.
///
/// # Safety
///
/// Par `(ponteiro, tamanho)` legivel; `saida_esq` gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_esquema_novo(
    nome: *const u8,
    nome_tam: usize,
    saida_esq: *mut *mut Punho<EsquemaFFI>,
) -> i32 {
    blindado(|| {
        saida(saida_esq, std::ptr::null_mut());
        if saida_esq.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_esquema_novo");
        }
        match texto::texto(nome, nome_tam) {
            Ok(n) => {
                saida(
                    saida_esq,
                    Punho::novo(
                        ETIQ_ESQUEMA,
                        EsquemaFFI {
                            nome: n.to_string(),
                            ..Default::default()
                        },
                    ),
                );
                PHX_OK
            }
            Err(e) => e,
        }
    })
}

/// Acrescenta uma coluna.
///
/// `largura` so vale para `PHX_COL_STR`; `precisao` e `escala`, so para
/// `PHX_COL_DECIMAL`. Os demais tipos ignoram os tres.
///
/// # Safety
///
/// `p` e um punho de esquema valido; `(nome, nome_tam)` legivel.
#[allow(clippy::too_many_arguments)] // um tipo de coluna tem mesmo estes parametros
#[no_mangle]
pub unsafe extern "C" fn phx_esquema_coluna(
    p: *mut Punho<EsquemaFFI>,
    nome: *const u8,
    nome_tam: usize,
    tipo: i32,
    largura: u32,
    precisao: u8,
    escala: u8,
    sinalizadores: u32,
) -> i32 {
    com(p, ETIQ_ESQUEMA, |e| {
        let nome = match texto::texto(nome, nome_tam) {
            Ok(n) => n,
            Err(c) => return c,
        };
        let ty = match valor::tipo_de_coluna(tipo, largura, precisao, escala) {
            Ok(t) => t,
            Err(c) => return c,
        };
        let mut c = Column::new(nome, ty);
        if sinalizadores & PHX_COL_OBRIGATORIA != 0 {
            c = c.obrigatoria();
        }
        e.colunas.push(c);
        PHX_OK
    })
}

/// Comeca um indice. As colunas dele entram por
/// [`phx_esquema_indice_coluna`].
///
/// # Safety
///
/// `p` e um punho de esquema valido; `(nome, nome_tam)` legivel.
#[no_mangle]
pub unsafe extern "C" fn phx_esquema_indice(
    p: *mut Punho<EsquemaFFI>,
    nome: *const u8,
    nome_tam: usize,
    sinalizadores: u32,
) -> i32 {
    com(p, ETIQ_ESQUEMA, |e| {
        let nome = match texto::texto(nome, nome_tam) {
            Ok(n) => n,
            Err(c) => return c,
        };
        let mut idx = IndexDef::new(nome, Vec::new());
        if sinalizadores & PHX_IDX_UNICO != 0 {
            idx = idx.unico();
        }
        if sinalizadores & PHX_IDX_PRIMARIA != 0 {
            idx = idx.primaria();
        }
        e.indices.push(idx);
        PHX_OK
    })
}

/// Acrescenta uma coluna ao indice em construcao -- o ultimo aberto por
/// [`phx_esquema_indice`]. `coluna` e a posicao dela no esquema.
///
/// # Safety
///
/// `p` e um punho de esquema valido.
#[no_mangle]
pub unsafe extern "C" fn phx_esquema_indice_coluna(
    p: *mut Punho<EsquemaFFI>,
    coluna: usize,
    sinalizadores: u32,
) -> i32 {
    com(p, ETIQ_ESQUEMA, |e| {
        if coluna >= e.colunas.len() {
            return anotar(
                PHX_ERRO_USO,
                format!(
                    "coluna {coluna} fora da faixa: o esquema tem {}",
                    e.colunas.len()
                ),
            );
        }
        let mut c = if sinalizadores & PHX_IDX_DESC != 0 {
            IndexColumn::desc(coluna)
        } else {
            IndexColumn::asc(coluna)
        };
        if sinalizadores & PHX_IDX_SEM_CAIXA != 0 {
            c = c.sem_caixa();
        }
        match e.indices.last_mut() {
            Some(idx) => {
                idx.colunas.push(c);
                PHX_OK
            }
            None => anotar(
                PHX_ERRO_USO,
                "nenhum indice aberto: chame phx_esquema_indice antes",
            ),
        }
    })
}

/// Libera o esquema.
///
/// # Safety
///
/// `p` e nulo ou um punho de esquema ainda nao liberado.
#[no_mangle]
pub unsafe extern "C" fn phx_esquema_liberar(p: *mut Punho<EsquemaFFI>) -> i32 {
    liberar(p, ETIQ_ESQUEMA)
}

// ========================================================================
//  Tabela
// ========================================================================

fn embrulhar_tabela(t: Table, saida_tab: *mut *mut Punho<TabelaFFI>) -> i32 {
    let serie = SERIE.fetch_add(1, Ordering::Relaxed);
    // SAFETY: `saida_tab` foi conferido pelo chamador.
    unsafe { saida(saida_tab, Punho::novo(ETIQ_TABELA, TabelaFFI { t, serie })) };
    PHX_OK
}

/// Cria a tabela a partir de um esquema montado.
///
/// O esquema **nao** e consumido: quem o criou continua dono e o libera. Um
/// esquema serve para criar a mesma tabela em varios databases.
///
/// `schema` e o schema logico dentro do database (subdiretorio); passe
/// `NULL, 0` para a raiz.
///
/// # Safety
///
/// `base` e `esq` sao punhos validos; `saida_tab` e gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_criar(
    base: *mut Punho<BaseFFI>,
    schema: *const u8,
    schema_tam: usize,
    esq: *mut Punho<EsquemaFFI>,
    saida_tab: *mut *mut Punho<TabelaFFI>,
) -> i32 {
    com(base, ETIQ_BASE, |b| {
        saida(saida_tab, std::ptr::null_mut());
        if saida_tab.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_tabela_criar");
        }
        let e = match conferir(esq, ETIQ_ESQUEMA) {
            Ok(e) => e,
            Err(c) => return c,
        };
        let schema = match texto::texto(schema, schema_tam) {
            Ok(s) => s,
            Err(c) => return c,
        };
        let esquema = match Schema::new(&e.nome, e.colunas.clone(), e.indices.clone()) {
            Ok(s) => s,
            Err(err) => return do_motor(&err),
        };
        let logico = if schema.is_empty() {
            None
        } else {
            Some(schema)
        };
        resultado(b.db.criar_tabela(logico, esquema), |t| {
            embrulhar_tabela(t, saida_tab)
        })
    })
}

/// Abre uma tabela existente. Aceita `schema.tabela`.
///
/// # Safety
///
/// `base` e um punho valido; `(nome, nome_tam)` legivel; `saida_tab`
/// gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_abrir(
    base: *mut Punho<BaseFFI>,
    nome: *const u8,
    nome_tam: usize,
    saida_tab: *mut *mut Punho<TabelaFFI>,
) -> i32 {
    com(base, ETIQ_BASE, |b| {
        saida(saida_tab, std::ptr::null_mut());
        if saida_tab.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_tabela_abrir");
        }
        let nome = match texto::texto(nome, nome_tam) {
            Ok(n) => n,
            Err(c) => return c,
        };
        resultado(b.db.abrir_qualificada(nome), |t| {
            embrulhar_tabela(t, saida_tab)
        })
    })
}

/// Fecha a tabela. Cursores abertos sobre ela param de funcionar, e dizem
/// isso com `PHX_ERRO_USO` em vez de tocar em memoria morta.
///
/// # Safety
///
/// `p` e nulo ou um punho de tabela ainda nao liberado.
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_fechar(p: *mut Punho<TabelaFFI>) -> i32 {
    liberar(p, ETIQ_TABELA)
}

/// Quantas linhas a VISAO enxerga.
///
/// A visao e parametro, e nao um padrao escondido, porque "quantas linhas tem
/// a tabela" tem tres respostas assim que existe exclusao suave: as que a
/// tela mostra, as marcadas, e os slots ocupados. Um contador que devolvesse
/// so um dos tres faria a tela dizer 2 e listar 1 -- e ninguem acha isso
/// lendo o codigo.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `qtd` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_registros(
    p: *mut Punho<TabelaFFI>,
    visao: u32,
    qtd: *mut u64,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let v = match visao_de(visao) {
            Ok(v) => v,
            Err(c) => return c,
        };
        resultado(x.t.contar(v), |n| {
            saida(qtd, n);
            PHX_OK
        })
    })
}

/// Quantas colunas o esquema tem.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `qtd` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_colunas(p: *mut Punho<TabelaFFI>, qtd: *mut usize) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        saida(qtd, x.t.esquema().colunas().len());
        PHX_OK
    })
}

/// O nome da i-esima coluna.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `destino`/`cap`/`precisa` como em
/// [`phx_versao`].
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_coluna_nome(
    p: *mut Punho<TabelaFFI>,
    i: usize,
    destino: *mut u8,
    cap: usize,
    precisa: *mut usize,
) -> i32 {
    com(p, ETIQ_TABELA, |x| match x.t.esquema().colunas().get(i) {
        Some(c) => texto::escrever(destino, cap, precisa, &c.nome),
        None => anotar(PHX_ERRO_USO, format!("coluna {i} fora da faixa")),
    })
}

/// O tipo da i-esima coluna, num dos `PHX_COL_*`.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `tipo` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_tabela_coluna_tipo(
    p: *mut Punho<TabelaFFI>,
    i: usize,
    tipo: *mut i32,
) -> i32 {
    com(p, ETIQ_TABELA, |x| match x.t.esquema().colunas().get(i) {
        Some(c) => {
            saida(tipo, valor::codigo_de_coluna(c.ty));
            PHX_OK
        }
        None => anotar(PHX_ERRO_USO, format!("coluna {i} fora da faixa")),
    })
}

/// Descarrega tudo em disco.
///
/// # Safety
///
/// `p` e um punho de tabela valido.
#[no_mangle]
pub unsafe extern "C" fn phx_sincronizar(p: *mut Punho<TabelaFFI>) -> i32 {
    com(p, ETIQ_TABELA, |x| resultado(x.t.sincronizar(), |_| PHX_OK))
}

/// Confere a integridade da tabela e devolve os contadores.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `rel` nulo ou aponta para um
/// `PhxRelatorio` gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_verificar(p: *mut Punho<TabelaFFI>, rel: *mut PhxRelatorio) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        resultado(x.t.verificar(), |r| {
            saida(
                rel,
                PhxRelatorio {
                    registros: r.registros,
                    slots: r.slots,
                    marcadas: r.marcadas,
                    eventos: r.eventos,
                    descartadas: r.descartadas,
                    motivos: r.motivos,
                    indices: r.indices.len() as u64,
                    trilha: r.trilha,
                },
            );
            PHX_OK
        })
    })
}

// ========================================================================
//  Dado
// ========================================================================

/// Insere uma linha e devolve o rowid.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `(vals, qtd)` descreve `PhxValor`
/// legiveis; `rowid` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_inserir(
    p: *mut Punho<TabelaFFI>,
    vals: *const PhxValor,
    qtd: usize,
    rowid: *mut u64,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let v = match valores(vals, qtd) {
            Ok(v) => v,
            Err(c) => return c,
        };
        resultado(x.t.inserir(&v), |id| {
            saida(rowid, id);
            PHX_OK
        })
    })
}

/// Grava por cima, sem conferir versao. **E o comportamento de sempre**, e
/// continua sendo: quem nao conhece a janela de conflito grava como antes.
///
/// # Safety
///
/// Mesmo contrato de [`phx_inserir`].
#[no_mangle]
pub unsafe extern "C" fn phx_atualizar(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    vals: *const PhxValor,
    qtd: usize,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let v = match valores(vals, qtd) {
            Ok(v) => v,
            Err(c) => return c,
        };
        resultado(x.t.atualizar(rowid, &v), |_| PHX_OK)
    })
}

/// Grava **se** a linha ainda estiver na versao esperada; senao devolve 3004
/// (`CONFLITO`).
///
/// Guarda PEDIDA, nao imposta: num aplicativo de celular a janela entre abrir
/// a ficha e tocar em salvar e de minutos, e e ai que ela vale. Impo-la a todo
/// chamador quebraria quem foi escrito antes dela.
///
/// # Safety
///
/// Mesmo contrato de [`phx_inserir`].
#[no_mangle]
pub unsafe extern "C" fn phx_atualizar_se(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    vals: *const PhxValor,
    qtd: usize,
    versao_esperada: u64,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let v = match valores(vals, qtd) {
            Ok(v) => v,
            Err(c) => return c,
        };
        resultado(x.t.atualizar_se(rowid, &v, versao_esperada), |_| PHX_OK)
    })
}

/// A versao atual da linha, para depois passar ao [`phx_atualizar_se`].
/// `PHX_NAO_HA` quando a linha nao existe.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `versao` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_versao_da_linha(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    versao: *mut u64,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        resultado_do_rowid(x.t.versao(rowid), |v| match v {
            Some(v) => {
                saida(versao, v);
                PHX_OK
            }
            None => PHX_NAO_HA,
        })
    })
}

/// Exclusao FISICA, com motivo. `saiu` recebe 1 quando havia o que excluir.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `(motivo, motivo_tam)` legivel; `saiu`
/// nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_excluir(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    motivo: *const u8,
    motivo_tam: usize,
    saiu: *mut u8,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let m = match texto::texto(motivo, motivo_tam) {
            Ok(m) => m,
            Err(c) => return c,
        };
        resultado(x.t.excluir_de_vez(rowid, m), |b| {
            saida(saiu, b as u8);
            PHX_OK
        })
    })
}

/// Exclusao SUAVE: marca a linha. E o excluir que volta.
///
/// # Safety
///
/// Mesmo contrato de [`phx_excluir`].
#[no_mangle]
pub unsafe extern "C" fn phx_excluir_suave(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    motivo: *const u8,
    motivo_tam: usize,
    saiu: *mut u8,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let m = match texto::texto(motivo, motivo_tam) {
            Ok(m) => m,
            Err(c) => return c,
        };
        resultado(x.t.excluir_suave(rowid, m), |b| {
            saida(saiu, b as u8);
            PHX_OK
        })
    })
}

/// Desfaz uma exclusao suave.
///
/// # Safety
///
/// Mesmo contrato de [`phx_excluir`].
#[no_mangle]
pub unsafe extern "C" fn phx_restaurar(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    motivo: *const u8,
    motivo_tam: usize,
    saiu: *mut u8,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let m = match texto::texto(motivo, motivo_tam) {
            Ok(m) => m,
            Err(c) => return c,
        };
        resultado(x.t.restaurar(rowid, m), |b| {
            saida(saiu, b as u8);
            PHX_OK
        })
    })
}

/// Le a linha do rowid. `PHX_NAO_HA` quando ela nao existe -- e isso **nao**
/// e erro.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `saida_linha` e gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_ler(
    p: *mut Punho<TabelaFFI>,
    rowid: u64,
    saida_linha: *mut *mut Punho<LinhaFFI>,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        saida(saida_linha, std::ptr::null_mut());
        if saida_linha.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_ler");
        }
        resultado_do_rowid(x.t.ler(rowid), |l| match l {
            Some(valores) => {
                saida(
                    saida_linha,
                    Punho::novo(ETIQ_LINHA, LinhaFFI::nova(valores)),
                );
                PHX_OK
            }
            None => PHX_NAO_HA,
        })
    })
}

/// A vista dos valores de uma linha. **Nao copia**: os ponteiros valem ate o
/// [`phx_linha_liberar`].
///
/// # Safety
///
/// `p` e um punho de linha valido; `vals` e `qtd` sao nulos ou gravaveis.
#[no_mangle]
pub unsafe extern "C" fn phx_linha_valores(
    p: *mut Punho<LinhaFFI>,
    vals: *mut *const PhxValor,
    qtd: *mut usize,
) -> i32 {
    com(p, ETIQ_LINHA, |l| {
        let v = l.vista();
        saida(vals, v.as_ptr());
        saida(qtd, v.len());
        PHX_OK
    })
}

/// Libera a linha. Depois disto os `PhxValor` da vista apontam para memoria
/// morta.
///
/// # Safety
///
/// `p` e nulo ou um punho de linha ainda nao liberado.
#[no_mangle]
pub unsafe extern "C" fn phx_linha_liberar(p: *mut Punho<LinhaFFI>) -> i32 {
    liberar(p, ETIQ_LINHA)
}

/// Rowids com a chave exata de um indice, no buffer do chamador.
///
/// Buffer pequeno devolve `PHX_ERRO_BUFFER` com o total em `achados` -- nada
/// e truncado em silencio.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `(indice, indice_tam)` legivel;
/// `(chave, chave_qtd)` descreve `PhxValor` legiveis; `rowids` aponta para
/// `cap` `u64` gravaveis.
#[allow(clippy::too_many_arguments)] // busca com chave composta pede mesmo isto
#[no_mangle]
pub unsafe extern "C" fn phx_buscar(
    p: *mut Punho<TabelaFFI>,
    indice: *const u8,
    indice_tam: usize,
    chave: *const PhxValor,
    chave_qtd: usize,
    rowids: *mut u64,
    cap: usize,
    achados: *mut usize,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let nome = match texto::texto(indice, indice_tam) {
            Ok(n) => n,
            Err(c) => return c,
        };
        let k = match valores(chave, chave_qtd) {
            Ok(k) => k,
            Err(c) => return c,
        };
        resultado(x.t.buscar(nome, &k), |ids| {
            saida(achados, ids.len());
            if rowids.is_null() || cap < ids.len() {
                return anotar(
                    erro::PHX_ERRO_BUFFER,
                    format!("cabem {cap} rowids e a busca achou {}", ids.len()),
                );
            }
            std::ptr::copy_nonoverlapping(ids.as_ptr(), rowids, ids.len());
            PHX_OK
        })
    })
}

// ========================================================================
//  Cursor
// ========================================================================

/// Abre um cursor na ORDEM DE DIGITACAO, com a visao escolhida.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `saida_cur` e gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_cursor_abrir(
    p: *mut Punho<TabelaFFI>,
    visao: u32,
    saida_cur: *mut *mut Punho<CursorFFI>,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        saida(saida_cur, std::ptr::null_mut());
        if saida_cur.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_cursor_abrir");
        }
        let v = match visao_de(visao) {
            Ok(v) => v,
            Err(c) => return c,
        };
        saida(
            saida_cur,
            Punho::novo(
                ETIQ_CURSOR,
                CursorFFI {
                    serie: x.serie,
                    visao: v,
                    lote: Vec::new(),
                    pos: 0,
                    ultimo: 0,
                    materializado: false,
                    esgotado: false,
                },
            ),
        );
        PHX_OK
    })
}

/// Abre um cursor na ORDEM DE UM INDICE.
///
/// Diferente do de digitacao, este **materializa** a ordem do `.ndx` de uma
/// vez: a arvore nao tem "continue depois desta chave" barato como o `.reg`
/// tem. Num aparelho pequeno e uma tabela grande, prefira o de digitacao.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `(indice, indice_tam)` legivel;
/// `saida_cur` gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_cursor_abrir_indice(
    p: *mut Punho<TabelaFFI>,
    indice: *const u8,
    indice_tam: usize,
    saida_cur: *mut *mut Punho<CursorFFI>,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        saida(saida_cur, std::ptr::null_mut());
        if saida_cur.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "saida nula em phx_cursor_abrir_indice");
        }
        let nome = match texto::texto(indice, indice_tam) {
            Ok(n) => n,
            Err(c) => return c,
        };
        resultado(x.t.varrer_indice(nome), |ids| {
            saida(
                saida_cur,
                Punho::novo(
                    ETIQ_CURSOR,
                    CursorFFI {
                        serie: x.serie,
                        visao: Visao::Ativas,
                        lote: ids,
                        pos: 0,
                        ultimo: 0,
                        materializado: true,
                        esgotado: false,
                    },
                ),
            );
            PHX_OK
        })
    })
}

/// O proximo rowid. `PHX_NAO_HA` quando acabou.
///
/// Recebe os **dois** punhos de proposito: assim o cursor nao guarda ponteiro
/// para a tabela, e um cursor que sobreviva a ela nao tem para onde apontar.
/// Cruzar os punhos devolve `PHX_ERRO_USO`, que e um erro diagnosticavel --
/// a alternativa seria um uso-depois-de-liberar.
///
/// # Safety
///
/// `p` e `c` sao punhos validos; `rowid` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_cursor_proximo(
    p: *mut Punho<TabelaFFI>,
    c: *mut Punho<CursorFFI>,
    rowid: *mut u64,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let cur = match conferir(c, ETIQ_CURSOR) {
            Ok(c) => c,
            Err(code) => return code,
        };
        if cur.serie != x.serie {
            return anotar(PHX_ERRO_USO, "este cursor foi aberto por outra tabela");
        }
        loop {
            if cur.pos < cur.lote.len() {
                let id = cur.lote[cur.pos];
                cur.pos += 1;
                cur.ultimo = id;
                saida(rowid, id);
                return PHX_OK;
            }
            if cur.esgotado || cur.materializado {
                return PHX_NAO_HA;
            }
            match x.t.pagina_depois_de(cur.ultimo, LOTE_CURSOR, cur.visao) {
                Ok(ids) => {
                    if ids.is_empty() {
                        cur.esgotado = true;
                        return PHX_NAO_HA;
                    }
                    // Lote curto significa fim de tabela; guardar isso evita
                    // uma varredura inteira do `.reg` so para descobrir que
                    // nao ha mais nada.
                    if (ids.len() as u64) < LOTE_CURSOR {
                        cur.esgotado = true;
                    }
                    cur.lote = ids;
                    cur.pos = 0;
                }
                Err(e) => return do_motor(&e),
            }
        }
    })
}

/// Libera o cursor.
///
/// # Safety
///
/// `p` e nulo ou um punho de cursor ainda nao liberado.
#[no_mangle]
pub unsafe extern "C" fn phx_cursor_liberar(p: *mut Punho<CursorFFI>) -> i32 {
    liberar(p, ETIQ_CURSOR)
}

// ========================================================================
//  Replicacao -- os ganchos
// ========================================================================

/// Liga (ou desliga) a imagem da linha no diario.
///
/// Sem imagem o `.log` diz QUE o rowid 42 mudou e nao diz PARA QUE -- basta
/// para auditoria e nao basta para replicar. Ligado, um registro de 200 bytes
/// gasta ~244 bytes de diario por alteracao em vez de 36; e por isso e
/// interruptor, e nao padrao.
///
/// # Safety
///
/// `p` e um punho de tabela valido.
#[no_mangle]
pub unsafe extern "C" fn phx_imagem_no_diario(p: *mut Punho<TabelaFFI>, ligado: u8) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        x.t.ligar_imagem_no_diario(ligado != 0);
        PHX_OK
    })
}

/// Quantos eventos o diario ja tem. E a POSICAO da sincronia: o aparelho
/// manda este numero e o servidor devolve o que veio depois.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `qtd` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_diario_qtd(p: *mut Punho<TabelaFFI>, qtd: *mut u64) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        resultado(x.t.eventos(), |n| {
            saida(qtd, n);
            PHX_OK
        })
    })
}

/// Le eventos a partir de `pular`, no buffer do chamador.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `saida_ev` aponta para `cap` `PhxEvento`
/// gravaveis; `lidos` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_diario_ler(
    p: *mut Punho<TabelaFFI>,
    pular: u64,
    saida_ev: *mut PhxEvento,
    cap: usize,
    lidos: *mut usize,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        saida(lidos, 0usize);
        if cap == 0 {
            return PHX_OK;
        }
        if saida_ev.is_null() {
            return anotar(PHX_ERRO_PONTEIRO, "buffer de eventos nulo com cap > 0");
        }
        resultado(x.t.diario(pular, cap as u64), |eventos| {
            for (i, e) in eventos.iter().enumerate() {
                std::ptr::write(saida_ev.add(i), evento_c(e));
            }
            saida(lidos, eventos.len());
            PHX_OK
        })
    })
}

fn evento_c(e: &phxsql_store::log::Evento) -> PhxEvento {
    PhxEvento {
        carimbo: e.carimbo,
        rowid: e.rowid,
        versao: e.versao,
        operacao: match e.operacao {
            Operacao::Inclusao => PHX_OP_INCLUSAO,
            Operacao::Alteracao => PHX_OP_ALTERACAO,
            Operacao::Exclusao => PHX_OP_EXCLUSAO,
        },
        usuario: e.usuario,
        origem: e.origem as u32,
        tam_imagem: e.tam_imagem,
    }
}

/// Um evento **com** os bytes que a outra ponta vai gravar.
///
/// `saida_img` recebe `NULL` quando o evento nao tem imagem (a exclusao nao
/// leva: o rowid basta).
///
/// # Safety
///
/// `p` e um punho de tabela valido; `ev` e `saida_img` sao nulos ou
/// gravaveis.
#[no_mangle]
pub unsafe extern "C" fn phx_diario_evento_com_imagem(
    p: *mut Punho<TabelaFFI>,
    pular: u64,
    ev: *mut PhxEvento,
    saida_img: *mut *mut Punho<ImagemFFI>,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        saida(saida_img, std::ptr::null_mut());
        resultado(x.t.diario_com_imagem(pular, 1), |mut lista| {
            if lista.is_empty() {
                return PHX_NAO_HA;
            }
            let (e, imagem) = lista.remove(0);
            saida(ev, evento_c(&e));
            if !imagem.is_empty() {
                saida(
                    saida_img,
                    Punho::novo(ETIQ_IMAGEM, ImagemFFI { bytes: imagem }),
                );
            }
            PHX_OK
        })
    })
}

/// A vista dos bytes de uma imagem. Vale ate o [`phx_imagem_liberar`].
///
/// # Safety
///
/// `p` e um punho de imagem valido; `dados` e `tam` sao nulos ou gravaveis.
#[no_mangle]
pub unsafe extern "C" fn phx_imagem_bytes(
    p: *mut Punho<ImagemFFI>,
    dados: *mut *const u8,
    tam: *mut usize,
) -> i32 {
    com(p, ETIQ_IMAGEM, |i| {
        saida(dados, i.bytes.as_ptr());
        saida(tam, i.bytes.len());
        PHX_OK
    })
}

/// Libera a imagem.
///
/// # Safety
///
/// `p` e nulo ou um punho de imagem ainda nao liberado.
#[no_mangle]
pub unsafe extern "C" fn phx_imagem_liberar(p: *mut Punho<ImagemFFI>) -> i32 {
    liberar(p, ETIQ_IMAGEM)
}

/// Aplica um evento vindo da outra ponta.
///
/// O `.reg` nunca reaproveita slot, entao aplicar todos os eventos NA ORDEM
/// produz rowids identicos aos da origem, sem negociar nada. Se o rowid que
/// sair aqui nao bater com o do evento, esta ponta ja divergiu -- e a chamada
/// PARA, em vez de espalhar a divergencia.
///
/// # Safety
///
/// `p` e um punho de tabela valido; `(imagem, imagem_tam)` legivel;
/// `saiu` nulo ou gravavel.
#[no_mangle]
pub unsafe extern "C" fn phx_aplicar_evento(
    p: *mut Punho<TabelaFFI>,
    operacao: u32,
    rowid: u64,
    imagem: *const u8,
    imagem_tam: usize,
    saiu: *mut u64,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        let op = match operacao {
            PHX_OP_INCLUSAO => Operacao::Inclusao,
            PHX_OP_ALTERACAO => Operacao::Alteracao,
            PHX_OP_EXCLUSAO => Operacao::Exclusao,
            outro => {
                return anotar(PHX_ERRO_USO, format!("operacao desconhecida: {outro}"));
            }
        };
        let img = match texto::bytes(imagem, imagem_tam) {
            Some(b) => b,
            None => return anotar(PHX_ERRO_PONTEIRO, "imagem nula com tamanho > 0"),
        };
        resultado(x.t.aplicar_evento(op, rowid, img), |id| {
            saida(saiu, id);
            PHX_OK
        })
    })
}

/// Carimbo e ORIGEM do proximo evento gravado por esta tabela.
///
/// A origem e o que mata o laco infinito do bidirecional: ao servir o fluxo
/// para outra ponta, os eventos cuja origem e a propria ponta de destino nao
/// viajam de volta. Zero = escrita local.
///
/// # Safety
///
/// `p` e um punho de tabela valido.
#[no_mangle]
pub unsafe extern "C" fn phx_forcar_proximo_evento(
    p: *mut Punho<TabelaFFI>,
    carimbo: i64,
    origem: u32,
) -> i32 {
    com(p, ETIQ_TABELA, |x| {
        if origem > u16::MAX as u32 {
            return anotar(
                PHX_ERRO_USO,
                format!("origem cabe em 16 bits; veio {origem}"),
            );
        }
        x.t.forcar_proximo_evento(carimbo, origem as u16);
        PHX_OK
    })
}

#[cfg(test)]
mod testes;
