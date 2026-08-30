//! Como o erro volta pela fronteira: codigo de retorno, e a mensagem numa
//! vaga POR THREAD.
//!
//! A escolha esta justificada em `docs/EMBUTIDO.md` secao 3.2. O resumo: os
//! codigos do `PhxError` ja sao publicos e estaveis, entao o cliente de C
//! trata o mesmo numero que o cliente de rede; e uma struct de resultado
//! cobraria uma alocacao de texto em toda chamada, inclusive nas que deram
//! certo.
//!
//! Por thread, e nao global, porque duas threads escrevendo na mesma vaga
//! fazem uma ler a mensagem da outra. Por thread, e nao por punho, porque o
//! primeiro erro que alguem encontra e o `phx_base_abrir` falhando -- e ai
//! ainda nao existe punho onde guardar.

use std::cell::RefCell;

use phxsql_core::error::PhxError;

/// Deu certo.
pub const PHX_OK: i32 = 0;
/// Nao ha o que devolver: a linha nao existe, ou o cursor acabou.
///
/// **Nao e erro**, e essa distincao e o ponto: confundir "nao achei" com
/// falha e o que faz aplicativo mostrar caixa vermelha para uma resposta
/// legitima.
pub const PHX_NAO_HA: i32 = 1;

/// Um panico foi capturado na fronteira. O punho fica envenenado.
pub const PHX_ERRO_PANICO: i32 = -1;
/// Ponteiro nulo onde nao pode ser.
pub const PHX_ERRO_PONTEIRO: i32 = -2;
/// O texto recebido nao e UTF-8 valido.
pub const PHX_ERRO_UTF8: i32 = -3;
/// O buffer do chamador nao cabe. O tamanho necessario sai no `precisa`.
pub const PHX_ERRO_BUFFER: i32 = -4;
/// A chamada nao faz sentido: tipo desconhecido, indice fora da faixa,
/// cursor de outra tabela.
pub const PHX_ERRO_USO: i32 = -5;
/// O punho sofreu um panico e recusa trabalho. So `fechar` funciona.
pub const PHX_ERRO_ENVENENADO: i32 = -6;

thread_local! {
    /// A mensagem do ultimo erro DESTA thread.
    static ULTIMO: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Guarda a mensagem e devolve o codigo, para o chamador escrever
/// `return anotar(...)` numa linha so.
pub fn anotar(codigo: i32, mensagem: impl Into<String>) -> i32 {
    let m = mensagem.into();
    ULTIMO.with(|u| *u.borrow_mut() = m);
    codigo
}

/// Traduz um erro do motor no codigo publico dele, guardando o texto.
pub fn do_motor(e: &PhxError) -> i32 {
    anotar(e.codigo() as i32, e.to_string())
}

/// O que `phx_ultimo_erro` entrega. Vazio quando nada falhou nesta thread.
pub fn ultimo() -> String {
    ULTIMO.with(|u| u.borrow().clone())
}

/// Limpa a vaga. Toda entrada da ABI comeca por aqui, para que uma mensagem
/// velha nunca seja lida como se fosse do erro de agora.
pub fn limpar() {
    ULTIMO.with(|u| u.borrow_mut().clear());
}

/// O nome simbolico de um codigo, **ja com o `\0` no fim**, para o
/// `phx_erro_nome` poder entregar o ponteiro direto.
///
/// Estatico e nunca liberado: e literal do binario. Devolve `"?"` para codigo
/// que esta build nao conhece, em vez de nulo -- quem imprime diagnostico nao
/// deve ter de conferir ponteiro.
///
/// O `\0` mora NA TABELA, e nao num segundo vetor de literais: duas tabelas
/// para a mesma lista e o comeco de duas listas diferentes.
pub fn nome_do_codigo_c(codigo: i32) -> &'static str {
    match codigo {
        PHX_OK => "OK\0",
        PHX_NAO_HA => "NAO_HA\0",
        PHX_ERRO_PANICO => "PANICO\0",
        PHX_ERRO_PONTEIRO => "PONTEIRO\0",
        PHX_ERRO_UTF8 => "UTF8\0",
        PHX_ERRO_BUFFER => "BUFFER\0",
        PHX_ERRO_USO => "USO\0",
        PHX_ERRO_ENVENENADO => "ENVENENADO\0",
        1001 => "CORROMPIDO\0",
        1002 => "ASSINATURA_INVALIDA\0",
        1003 => "VERSAO_NAO_SUPORTADA\0",
        2001 => "ESQUEMA_INVALIDO\0",
        2002 => "TIPO_INVALIDO\0",
        3001 => "NAO_ENCONTRADO\0",
        3002 => "DUPLICADO\0",
        3003 => "LIMITE_EXCEDIDO\0",
        3004 => "CONFLITO\0",
        3005 => "SINAL\0",
        4001 => "ACESSO_NEGADO\0",
        4002 => "EM_CARGA\0",
        4003 => "REDIRECIONA\0",
        4004 => "SPARE_EM_ESPERA\0",
        5001 => "ERRO_DE_ES\0",
        6001 => "CANCELADO\0",
        _ => "?\0",
    }
}

/// O mesmo nome, sem o `\0`, para quem le do lado do Rust.
pub fn nome_do_codigo(codigo: i32) -> &'static str {
    nome_do_codigo_c(codigo).trim_end_matches('\0')
}
