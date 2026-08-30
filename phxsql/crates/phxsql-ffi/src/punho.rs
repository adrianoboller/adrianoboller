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

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::erro::{anotar, limpar, PHX_ERRO_ENVENENADO, PHX_ERRO_PANICO, PHX_ERRO_PONTEIRO};

/// Etiqueta de cada tipo de punho. Numeros diferentes e reconheciveis num
/// despejo de memoria.
pub const ETIQ_BASE: u64 = 0x5048_5842_4153_4501;
pub const ETIQ_ESQUEMA: u64 = 0x5048_5845_5351_4d02;
pub const ETIQ_TABELA: u64 = 0x5048_5854_4142_4c03;
pub const ETIQ_LINHA: u64 = 0x5048_584c_494e_4804;
pub const ETIQ_CURSOR: u64 = 0x5048_5843_5552_5305;
pub const ETIQ_IMAGEM: u64 = 0x5048_5849_4d47_4d06;

/// Um punho: etiqueta, veneno e o conteudo.
///
/// A etiqueta pega o caso comum de uso-depois-de-liberar e o punho do tipo
/// errado na posicao errada. Nao pega memoria liberada e reocupada com o
/// mesmo padrao de bytes -- e uma rede, nao um contrato, e o cabecalho de C
/// diz isso com todas as letras.
pub struct Punho<T> {
    etiqueta: u64,
    envenenado: bool,
    pub dentro: T,
}

impl<T> Punho<T> {
    /// Embrulha e entrega o ponteiro cru que o C vai guardar.
    pub fn novo(etiqueta: u64, dentro: T) -> *mut Punho<T> {
        Box::into_raw(Box::new(Punho {
            etiqueta,
            envenenado: false,
            dentro,
        }))
    }
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

/// Confere nulo, etiqueta e veneno de um punho SECUNDARIO -- aquele que a
/// chamada recebe alem do principal (o esquema no `criar_tabela`, o cursor no
/// `proximo`).
///
/// Sem `catch_unwind` proprio: quem envolve a chamada inteira e o [`com`] do
/// punho principal, e envenenar dois punhos por um panico so seria mentir
/// sobre qual deles ficou pela metade.
///
/// # Safety
///
/// `p` tem de ser nulo ou um ponteiro devolvido por [`Punho::novo`] com esta
/// mesma etiqueta e ainda nao liberado.
pub unsafe fn conferir<'a, T>(p: *mut Punho<T>, etiqueta: u64) -> Result<&'a mut T, i32> {
    if p.is_null() {
        return Err(anotar(PHX_ERRO_PONTEIRO, "punho nulo"));
    }
    let punho = &mut *p;
    if punho.etiqueta != etiqueta {
        return Err(anotar(
            PHX_ERRO_PONTEIRO,
            "punho invalido: etiqueta errada -- ja foi liberado, ou e de outro tipo",
        ));
    }
    if punho.envenenado {
        return Err(anotar(
            PHX_ERRO_ENVENENADO,
            "este punho sofreu um panico e nao aceita mais trabalho; feche e reabra",
        ));
    }
    Ok(&mut punho.dentro)
}

/// Roda uma entrada da ABI SOBRE um punho: confere nulo, etiqueta e veneno,
/// e envenena se o corpo entrar em panico.
///
/// # Safety
///
/// `p` tem de ser nulo ou um ponteiro devolvido por [`Punho::novo`] com esta
/// mesma etiqueta e ainda nao liberado.
pub unsafe fn com<T>(p: *mut Punho<T>, etiqueta: u64, f: impl FnOnce(&mut T) -> i32) -> i32 {
    limpar();
    if p.is_null() {
        return anotar(PHX_ERRO_PONTEIRO, "punho nulo");
    }
    let punho = &mut *p;
    if punho.etiqueta != etiqueta {
        return anotar(
            PHX_ERRO_PONTEIRO,
            "punho invalido: etiqueta errada -- ja foi liberado, ou e de outro tipo",
        );
    }
    if punho.envenenado {
        return anotar(
            PHX_ERRO_ENVENENADO,
            "este punho sofreu um panico e nao aceita mais trabalho; feche e reabra",
        );
    }
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
/// Mesmo contrato do [`com`].
pub unsafe fn liberar<T>(p: *mut Punho<T>, etiqueta: u64) -> i32 {
    limpar();
    if p.is_null() {
        // Liberar nulo e o que `free(NULL)` faz: nada, sem reclamar.
        return crate::erro::PHX_OK;
    }
    if (*p).etiqueta != etiqueta {
        return anotar(
            PHX_ERRO_PONTEIRO,
            "punho invalido: etiqueta errada -- liberar duas vezes, ou punho de outro tipo",
        );
    }
    // Zera antes de soltar: e o que faz a segunda liberacao ser recusada em
    // vez de derrubar o processo, no caso comum em que a memoria ainda nao
    // foi reocupada.
    (*p).etiqueta = 0;
    drop(Box::from_raw(p));
    crate::erro::PHX_OK
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
