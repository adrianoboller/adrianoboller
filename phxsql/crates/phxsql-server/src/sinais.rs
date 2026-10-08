//! A parada pedida pelo sistema operacional -- `SIGTERM` e `SIGINT` (pedido
//! 687).
//!
//! # Por que existe
//!
//! Sem tratador, os dois sinais matam o processo pelo padrao do nucleo: nada
//! do que a janela de durabilidade segurava vai ao disco, e o `.ndx` de toda
//! tabela escrita desde o ultimo fecho fica com o byte 52 em 1. Medido contra
//! o SO em 08/10/2026, 50 insercoes e `kill -TERM`: em `sistema` e `por_lote`
//! o `phxsql verificar` recusa com «ficou para tras numa queda»; em
//! `por_operacao`, que sincroniza a cada escrita, sai INTEGRA. Parada pedida
//! (o `systemctl stop`, o Ctrl-C do terminal) e a parada de todo dia, e ela
//! nao pode custar um `reindex`.
//!
//! # O meio, e por que ele pede FFI
//!
//! A `std` nao instala tratador de sinal: ela so ignora o `SIGPIPE` no
//! arranque. Nao ha `/proc`, arquivo ou chamada segura que faca isso. As
//! hipoteses medidas:
//!
//! * **H1, morta** -- so `std`. Nao existe.
//! * **H2, morta** -- `sigwait` numa thread dedicada, com o sinal bloqueado
//!   em todas as outras. Pede `pthread_sigmask` ANTES de qualquer thread
//!   nascer e o `sigset_t` com o tamanho da libc de cada sistema (128 bytes
//!   na glibc, 8 no macOS): tres funcoes e um leiaute declarados a mao,
//!   onde errar o leiaute e memoria pisada.
//! * **H3, a que ficou** -- `signal(2)` da libc que o binario ja carrega, com
//!   um tratador que so grava um atomico. UMA funcao declarada, de assinatura
//!   que nao muda entre sistemas Unix, e nenhuma crate: a petrea «zero
//!   dependencias externas» fala de crate, e a libc ja e o chao da propria
//!   `std`. O `unsafe` mora so aqui, em duas linhas.
//!
//! O tratador nao faz NADA alem do atomico, porque dentro de um tratador de
//! sinal so o que e seguro para sinal pode rodar: tomar mutex, alocar ou
//! escrever no `stderr` ali e o deadlock de quem foi interrompido segurando o
//! mesmo mutex. Quem trabalha e uma thread comum, que olha o atomico.
//!
//! # O segundo sinal mata
//!
//! A thread devolve os dois sinais ao padrao do nucleo ANTES de comecar a
//! parada. Se o fecho pendurar (um disco que nao responde), o segundo Ctrl-C
//! ou `kill -TERM` derruba o processo como antes -- o operador nunca fica
//! com um servidor que nao morre. E o que o PostgreSQL chama de parada
//! «rapida» seguida da «imediata».
//!
//! # Windows
//!
//! Fica como era: o Ctrl-C de console pede `SetConsoleCtrlHandler`, que nao
//! se confere nesta maquina. [`instalar`] devolve `false` e a parada la
//! continua sendo a queda que o arranque cura.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// O sinal que chegou, ou 0. Atomico sem trava: e a unica coisa que um
/// tratador de sinal pode tocar com seguranca.
static PEDIDO: AtomicI32 = AtomicI32::new(0);

/// Os numeros sao os mesmos em Linux, macOS e nos BSD (POSIX os fixa).
pub const SIGINT: i32 = 2;
pub const SIGTERM: i32 = 15;

#[cfg(unix)]
mod ffi {
    // `signal(2)`: `sighandler_t signal(int, sighandler_t)`. O tratador vai
    // como inteiro do tamanho de ponteiro porque `SIG_DFL` e 0 e `SIG_ERR` e
    // -1 -- nao sao funcoes, e um `fn` do Rust nao pode valer nenhum dos dois.
    unsafe extern "C" {
        pub fn signal(sinal: i32, tratador: usize) -> usize;
    }
    pub const SIG_DFL: usize = 0;
    pub const SIG_ERR: usize = usize::MAX;
}

#[cfg(unix)]
extern "C" fn ao_sinal(sinal: i32) {
    PEDIDO.store(sinal, Ordering::SeqCst);
}

/// Ja instalado neste processo? O tratador e do PROCESSO, e quem chama
/// `instalar` sobe uma thread que o vigia: duas chamadas seriam duas threads
/// disputando a mesma parada. Daqui sai o teto de uma por processo.
static INSTALADO: AtomicBool = AtomicBool::new(false);

/// Instala o tratador do `SIGTERM` e do `SIGINT`. `false` quando o sistema
/// recusou, quando ja estava instalado ou quando nao e Unix: so quem recebe
/// `true` sobe o vigia.
pub fn instalar() -> bool {
    if INSTALADO.swap(true, Ordering::SeqCst) {
        return false;
    }
    #[cfg(unix)]
    {
        let tratador = ao_sinal as extern "C" fn(i32) as usize;
        // SAFETY: `signal` e a da libc ligada ao binario; o tratador e uma
        // `extern "C" fn(i32)` que so grava um atomico sem trava, o que e
        // seguro dentro de um tratador de sinal.
        [SIGTERM, SIGINT]
            .iter()
            .all(|&s| unsafe { ffi::signal(s, tratador) } != ffi::SIG_ERR)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Devolve os dois sinais ao padrao do nucleo: o proximo mata.
pub fn devolver_ao_padrao() {
    #[cfg(unix)]
    for s in [SIGTERM, SIGINT] {
        // SAFETY: `SIG_DFL` e o valor que o proprio nucleo usa de fabrica.
        unsafe {
            ffi::signal(s, ffi::SIG_DFL);
        }
    }
}

/// O sinal que pediu a parada, se algum chegou.
pub fn pedido() -> Option<i32> {
    match PEDIDO.load(Ordering::SeqCst) {
        0 => None,
        s => Some(s),
    }
}

/// O nome para a linha do `stderr`.
pub fn nome(sinal: i32) -> &'static str {
    match sinal {
        SIGTERM => "SIGTERM",
        SIGINT => "SIGINT",
        _ => "sinal",
    }
}
