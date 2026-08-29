//! O registro global de handles.
//!
//! Handle ODBC e um ponteiro opaco para o aplicativo -- e ponteiro de verdade
//! seria um convite a uso-depois-de-liberar dentro do NOSSO codigo quando o
//! aplicativo errar a ordem das chamadas (e ele erra: e uma API de 30 anos).
//! Aqui o handle e uma CHAVE num mapa global: handle liberado ou inventado
//! simplesmente nao esta no mapa, e a resposta e SQL_INVALID_HANDLE em vez de
//! memoria alheia.

use crate::conexao::Canal;
use crate::resultado::Resultado;
use crate::tipos::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Um registro de diagnostico, como o SQLGetDiagRec entrega.
#[derive(Debug, Clone)]
pub struct Diag {
    pub estado: String,
    pub mensagem: String,
    /// O codigo de erro do servidor, quando o erro veio de la; zero no resto.
    pub nativo: i32,
}

/// Uma coluna amarrada por SQLBindCol. Os ponteiros ficam guardados como
/// numeros porque o mapa mora atras de um Mutex (que exige Send) e porque
/// ninguem os segue fora da chamada de SQLFetch -- e la o contrato da ABI
/// diz que eles ainda valem.
#[derive(Debug, Clone)]
pub struct Amarra {
    pub coluna: u16,
    pub tipo_c: SqlSmallint,
    pub buf: usize,
    pub cap: SqlLen,
    pub indicador: usize,
}

#[derive(Default)]
pub struct Ambiente {
    pub versao_odbc: i32,
    pub diag: Vec<Diag>,
}

pub struct Ligacao {
    /// O canal e compartilhado com os comandos por Arc para a rede acontecer
    /// FORA da trava do registro: um SELECT lento nao pode congelar todos os
    /// outros handles do processo.
    pub canal: Option<Arc<Mutex<Canal>>>,
    pub database: String,
    pub servidor: String,
    pub usuario: String,
    pub diag: Vec<Diag>,
}

#[derive(Default)]
pub struct Comando {
    /// Handle da ligacao dona, para achar o canal na hora de executar.
    pub dono: usize,
    /// O texto guardado pelo SQLPrepare, que o SQLExecute roda. Nao ha
    /// parametros nem plano: preparar aqui e so guardar -- e o que permite ao
    /// isql e companhia, que so falam prepare/execute, funcionarem.
    pub preparado: Option<String>,
    pub resultado: Option<Resultado>,
    /// Proxima linha do fetch (0-based). `linha_atual` e cursor-1.
    pub cursor: usize,
    /// Quantos bytes de cada celula da linha atual ja foram entregues pelo
    /// SQLGetData -- e o que permite ler um memo em pedacos.
    pub entregues: Vec<usize>,
    pub amarras: Vec<Amarra>,
    pub diag: Vec<Diag>,
}

pub enum Punho {
    Ambiente(Ambiente),
    Ligacao(Ligacao),
    Comando(Comando),
}

// BTreeMap e nao HashMap porque `BTreeMap::new` e const: o registro nasce
// pronto, sem once-lock nem inicializacao preguicosa para errar.
static PUNHOS: Mutex<BTreeMap<usize, Punho>> = Mutex::new(BTreeMap::new());
static PROXIMO: AtomicUsize = AtomicUsize::new(1);

pub fn criar(p: Punho) -> usize {
    let id = PROXIMO.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut mapa) = PUNHOS.lock() {
        mapa.insert(id, p);
    }
    id
}

pub fn remover(id: usize) -> Option<Punho> {
    PUNHOS.lock().ok()?.remove(&id)
}

/// Roda `f` com o punho travado. `None` quer dizer handle invalido.
pub fn com<R>(id: usize, f: impl FnOnce(&mut Punho) -> R) -> Option<R> {
    let mut mapa = PUNHOS.lock().ok()?;
    mapa.get_mut(&id).map(f)
}

pub fn id_de(h: SqlHandle) -> usize {
    h as usize
}

pub fn como_handle(id: usize) -> SqlHandle {
    id as SqlHandle
}
