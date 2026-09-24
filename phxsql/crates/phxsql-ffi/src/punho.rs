//! Os punhos opacos da ABI, a etiqueta que os identifica, e a blindagem
//! contra panico.
//!
//! # Por que a blindagem e obrigatoria, e nao um cuidado extra
//!
//! Um `panic` do Rust desenrolando a pilha para dentro de um quadro de C e
//! comportamento indefinido. Num aplicativo de celular isso nao aparece como
//! erro tratavel: aparece como o app fechando sozinho. Entao TODA funcao
//! exportada passa por [`blindado`] ou por [`com`], que sao os dois unicos
//! caminhos de entrada.
//!
//! # E por que capturar nao basta
//!
//! Capturar o panico salva o processo e NAO conserta o objeto: um panico no
//! meio de um `inserir` pode ter deixado o `.reg` com o cabecalho gravado e o
//! payload nao. Continuar chamando aquele punho espalharia a inconsistencia
//! -- e a mesma licao do `aplicar_evento`, que PARA quando a replica divergiu
//! em vez de seguir.
//!
//! Por isso o punho e ENVENENADO: depois de um panico capturado toda chamada
//! nele devolve `PHX_ERRO_ENVENENADO` sem tocar no motor. So o `fechar`
//! continua funcionando, porque o chamador precisa poder liberar a memoria.
//! O conserto e reabrir, e reabrir passa pela recuperacao de abertura.
//!
//! # Por que a conferencia nao le a memoria apontada
//!
//! Ate 24/09/2026 o punho se conferia lendo a etiqueta DENTRO dele -- e para
//! o punho ja liberado isso e ler memoria morta. No glibc a pagina continua
//! mapeada e a leitura "funcionava"; no musl (o alvo estatico de IoT e de
//! ARM) o `free` dos 3.464 bytes do punho de tabela devolve a pagina inteira
//! ao sistema (`munmap` de 4.096, visto no `strace`), e a rede que devia
//! recusar o punho morto derrubava o processo com SIGSEGV -- medido em
//! x86-64 musl nativo e em ARM64 sob qemu.
//! Hoje quem responde "este ponteiro e um punho vivo, e de que tipo?" e o
//! registro [`VIVOS`], e a memoria so e tocada depois de ele dizer sim. E a
//! mesma decisao que o `crates/phxsql-odbc/src/registro.rs` tomou para o
//! handle do ODBC.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{PoisonError, RwLock};

use crate::erro::{anotar, limpar, PHX_ERRO_ENVENENADO, PHX_ERRO_PANICO, PHX_ERRO_PONTEIRO};

/// Etiqueta de cada tipo de punho. Numeros diferentes e reconheciveis num
/// despejo de memoria.
pub const ETIQ_BASE: u64 = 0x5048_5842_4153_4501;
pub const ETIQ_ESQUEMA: u64 = 0x5048_5845_5351_4d02;
pub const ETIQ_TABELA: u64 = 0x5048_5854_4142_4c03;
pub const ETIQ_LINHA: u64 = 0x5048_584c_494e_4804;
pub const ETIQ_CURSOR: u64 = 0x5048_5843_5552_5305;
pub const ETIQ_IMAGEM: u64 = 0x5048_5849_4d47_4d06;

/// Os punhos vivos: endereco -> etiqueta, repartidos em [`GAVETAS`] gavetas.
///
/// Entra no [`Punho::novo`], sai no [`liberar`] ANTES de a memoria ser
/// solta. Punho liberado, inventado ou copiado nao esta aqui, e a recusa sai
/// sem tocar no endereco. O que continua passando e o endereco que o
/// alocador reocupou com um punho NOVO do mesmo tipo -- esse e um punho vivo
/// de verdade, so que outro, e o cabecalho de C diz isso.
///
/// Repartido porque a conferencia acontece em TODA chamada, e o contrato 5
/// do cabecalho promete punhos diferentes em threads diferentes. Um mapa so
/// atras de uma trava so pos quatro threads, cada uma no seu punho, na mesma
/// linha de cache: a chamada mais barata da ABI (`phx_tabela_colunas`) foi
/// de 5-11 ns para 167-256 ns por chamada; repartido, 19-43 ns (24/09/2026,
/// x86-64, 10 milhoes de chamadas por thread). Cada gaveta ocupa a propria
/// linha de cache, e a gaveta sai do endereco. `BTreeMap`, e nao `Vec`, pelo
/// pior caso: medidos com 10.000 punhos vivos, os dois empataram no ruido, e
/// so a arvore nao cresce linear.
static VIVOS: [Gaveta; GAVETAS] = [GAVETA_VAZIA; GAVETAS];

/// Potencia de dois: a gaveta sai dos bits altos de um hash multiplicativo.
const GAVETAS: usize = 64;

/// Alinhada em 128 bytes para duas gavetas nunca dividirem a linha de cache
/// (128 cobre o par de linhas que o prefetch de x86-64 puxa junto, e a linha
/// de 128 de alguns ARM).
#[repr(align(128))]
struct Gaveta(RwLock<BTreeMap<usize, u64>>);

// So existe para montar o `VIVOS` acima, que e `static`: cada copia do
// `const` e uma gaveta nova e vazia, que e exatamente o que se quer ali.
#[allow(clippy::declare_interior_mutable_const)]
const GAVETA_VAZIA: Gaveta = Gaveta(RwLock::new(BTreeMap::new()));

/// A gaveta de um endereco. Hash de Fibonacci em 64 bits -- e em `u64`, e
/// nao `usize`, para a constante caber no ARMv7 de 32 bits tambem.
fn gaveta(p: usize) -> &'static RwLock<BTreeMap<usize, u64>> {
    let i = (p as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - GAVETAS.trailing_zeros());
    &VIVOS[i as usize].0
}

/// A UNICA decisao sobre o que o registro disse -- `com`, `conferir` e
/// `liberar` passam todos por aqui, para a mensagem e o criterio nao
/// divergirem entre a chamada e a liberacao.
fn decidir(achada: Option<u64>, etiqueta: u64) -> Result<(), i32> {
    match achada {
        Some(e) if e == etiqueta => Ok(()),
        Some(_) => Err(anotar(
            PHX_ERRO_PONTEIRO,
            "punho invalido: e um punho vivo de OUTRO tipo, na posicao errada",
        )),
        None => Err(anotar(
            PHX_ERRO_PONTEIRO,
            "punho invalido: nao esta vivo -- ja foi liberado, ou nao saiu desta biblioteca",
        )),
    }
}

/// Um punho: etiqueta, veneno e o conteudo.
///
/// Quem decide se o punho vale e o [`VIVOS`], nao a etiqueta: ela continua
/// aqui para ser reconhecivel num despejo de memoria e para pegar punho VIVO
/// cuja memoria o chamador pisou (estouro de buffer vizinho).
pub struct Punho<T> {
    etiqueta: u64,
    envenenado: bool,
    pub dentro: T,
}

impl<T> Punho<T> {
    /// Embrulha e entrega o ponteiro cru que o C vai guardar.
    pub fn novo(etiqueta: u64, dentro: T) -> *mut Punho<T> {
        let p = Box::into_raw(Box::new(Punho {
            etiqueta,
            envenenado: false,
            dentro,
        }));
        gaveta(p as usize)
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(p as usize, etiqueta);
        p
    }
}

/// Nulo, registro, etiqueta e veneno -- nessa ordem, e a memoria do punho so
/// e lida depois de o registro dizer que ele esta vivo.
///
/// # Safety
///
/// `p` tem de ser nulo, ou um ponteiro qualquer: o que nao estiver no
/// [`VIVOS`] e recusado sem ser lido. O que estiver tem de ser usado por uma
/// thread so (contrato 5 do cabecalho).
unsafe fn aberto<'a, T>(p: *mut Punho<T>, etiqueta: u64) -> Result<&'a mut Punho<T>, i32> {
    if p.is_null() {
        return Err(anotar(PHX_ERRO_PONTEIRO, "punho nulo"));
    }
    let achada = gaveta(p as usize)
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&(p as usize))
        .copied();
    decidir(achada, etiqueta)?;
    let punho = &mut *p;
    if punho.etiqueta != etiqueta {
        return Err(anotar(
            PHX_ERRO_PONTEIRO,
            "punho corrompido: vivo no registro, mas a etiqueta na memoria foi pisada",
        ));
    }
    if punho.envenenado {
        return Err(anotar(
            PHX_ERRO_ENVENENADO,
            "este punho sofreu um panico e nao aceita mais trabalho; feche e reabra",
        ));
    }
    Ok(punho)
}

/// Roda uma entrada da ABI que nao tem punho (abrir, versao, ultimo erro).
///
/// Nao limpa a vaga de erro: quem chama e que decide, porque
/// `phx_ultimo_erro` justamente precisa dela intacta.
pub fn blindado_cru(f: impl FnOnce() -> i32) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(codigo) => codigo,
        Err(carga) => anotar(
            PHX_ERRO_PANICO,
            format!("panico na fronteira: {}", texto(carga.as_ref())),
        ),
    }
}

/// Igual ao [`blindado_cru`], limpando a vaga de erro antes.
pub fn blindado(f: impl FnOnce() -> i32) -> i32 {
    limpar();
    blindado_cru(f)
}

/// Confere nulo, registro, etiqueta e veneno de um punho SECUNDARIO -- aquele que a
/// chamada recebe alem do principal (o esquema no `criar_tabela`, o cursor no
/// `proximo`).
///
/// Sem `catch_unwind` proprio: quem envolve a chamada inteira e o [`com`] do
/// punho principal, e envenenar dois punhos por um panico so seria mentir
/// sobre qual deles ficou pela metade.
///
/// # Safety
///
/// Mesmo contrato do [`aberto`].
pub unsafe fn conferir<'a, T>(p: *mut Punho<T>, etiqueta: u64) -> Result<&'a mut T, i32> {
    aberto(p, etiqueta).map(|punho| &mut punho.dentro)
}

/// Roda uma entrada da ABI SOBRE um punho: confere nulo, registro, etiqueta e
/// veneno, e envenena se o corpo entrar em panico.
///
/// # Safety
///
/// Mesmo contrato do [`aberto`].
pub unsafe fn com<T>(p: *mut Punho<T>, etiqueta: u64, f: impl FnOnce(&mut T) -> i32) -> i32 {
    limpar();
    let punho = match aberto(p, etiqueta) {
        Ok(punho) => punho,
        Err(codigo) => return codigo,
    };
    match catch_unwind(AssertUnwindSafe(|| f(&mut punho.dentro))) {
        Ok(codigo) => codigo,
        Err(carga) => {
            // O veneno entra AQUI, e nao no `blindado`: e o punho que pode ter
            // ficado pela metade, e e ele que precisa recusar a proxima
            // chamada.
            punho.envenenado = true;
            anotar(
                PHX_ERRO_PANICO,
                format!("panico na fronteira: {}", texto(carga.as_ref())),
            )
        }
    }
}

/// Confere e libera. Funciona mesmo com o punho envenenado -- e tem de
/// funcionar, senao um panico viraria vazamento.
///
/// # Safety
///
/// Mesmo contrato do [`aberto`].
pub unsafe fn liberar<T>(p: *mut Punho<T>, etiqueta: u64) -> i32 {
    liberar_depois_de(p, etiqueta, |_| crate::erro::PHX_OK)
}

/// Libera -- e, antes de soltar a memoria, roda `antes` no que o punho
/// guarda. Um caminho so com o [`liberar`], que e este com `antes` vazio.
///
/// Existe para o fechar que tem o que fazer no fim (o `phx_tabela_fechar` do
/// pedido 522 sincroniza a tabela cuja marca so este processo sustenta). O
/// punho SAI do registro antes de `antes` rodar, entao nenhuma outra thread o
/// usa no meio; e ele e liberado mesmo quando `antes` falha ou entra em
/// panico -- o codigo de `antes` e o que volta, e a memoria nao vaza. Punho
/// envenenado nao roda `antes`: ele ja nao aceita trabalho, e o `Drop` do que
/// ele guarda e quem decide o que vai ao disco.
///
/// # Safety
///
/// Mesmo contrato do [`aberto`].
pub unsafe fn liberar_depois_de<T>(
    p: *mut Punho<T>,
    etiqueta: u64,
    antes: impl FnOnce(&mut T) -> i32,
) -> i32 {
    limpar();
    if p.is_null() {
        // Liberar nulo e o que `free(NULL)` faz: nada, sem reclamar.
        return crate::erro::PHX_OK;
    }
    // Conferir e tirar do registro sob a MESMA trava de escrita: duas
    // threads liberando o mesmo punho nao passam as duas. E sai do registro
    // antes de a memoria ser solta, entao a segunda liberacao e recusada
    // pelo registro, sem ler o bloco que o alocador pode ja ter devolvido.
    {
        let mut vivos = gaveta(p as usize)
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        if let Err(codigo) = decidir(vivos.get(&(p as usize)).copied(), etiqueta) {
            return codigo;
        }
        vivos.remove(&(p as usize));
    }
    let mut caixa = Box::from_raw(p);
    let codigo = if caixa.envenenado {
        crate::erro::PHX_OK
    } else {
        blindado_cru(|| antes(&mut caixa.dentro))
    };
    // Zerar ja nao e o que recusa a segunda liberacao -- e o registro. Fica
    // para o despejo de memoria nao mostrar um punho morto com cara de vivo.
    caixa.etiqueta = 0;
    drop(caixa);
    codigo
}

/// O texto de um panico, quando ele e um dos dois formatos usuais.
fn texto(carga: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = carga.downcast_ref::<&str>() {
        return (*s).to_string();
    }
    if let Some(s) = carga.downcast_ref::<String>() {
        return s.clone();
    }
    "carga desconhecida".to_string()
}
