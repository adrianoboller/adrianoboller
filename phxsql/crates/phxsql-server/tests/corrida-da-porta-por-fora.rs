//! Prova real do pedido 401: a corrida do `porta_livre()` velho existe e e
//! reproduzivel SEM depender de sorte de escalonador, e o conserto (pedir
//! porta 0 e ler a REAL de volta do proprio ouvinte) a torna IMPOSSIVEL POR
//! CONSTRUCAO -- nao so "mais dificil de acontecer".
//!
//! # Por que forcar em vez de esperar a corrida acontecer sozinha
//!
//! Um teste que espera dois testes de verdade colidirem por acaso e um teste
//! flocado -- exatamente o que este pedido existe para nao fazer. A tecnica
//! aqui e a de sempre para provar uma corrida TOCTOU (time-of-check a
//! time-of-use): a janela do padrao antigo e um INSTANTE ENTRE DUAS CHAMADAS
//! DE FUNCAO, e um instante entre duas chamadas e algo que o proprio teste
//! pode ocupar deterministicamente -- nao e preciso um segundo processo nem
//! um `sleep` torcendo para o escalonador cooperar.
//!
//! # O padrao antigo, copiado tal e qual
//!
//! `porta_livre_antiga()` abaixo e o `fn porta_livre() -> u16` que vivia em
//! ~26 arquivos de `tests/` deste crate antes do pedido 401: abre um
//! `TcpListener` em `127.0.0.1:0`, LE a porta que o sistema deu, SOLTA o
//! ouvinte, e devolve so o NUMERO. O numero e usado bem mais tarde -- no
//! `config.json` escrito em disco, e so entao o `Servidor::escutar()` do
//! processo (ou, nos testes por soquete, um `Servidor::novo(...).escutar()`
//! numa thread) da o `bind` de verdade.

mod comum;
use comum::DirTemp;

use std::net::TcpListener;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use phxsql_server::{Config, Servidor};

/// O padrao ANTIGO, tal e qual vivia nos ~26 arquivos que o pedido 401
/// fechou: reserva, LE, SOLTA -- e o numero fica orfao ate alguem usa-lo.
fn porta_livre_antiga() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// **VERMELHO, medido aqui -- nao suposto.**
///
/// Entre `porta_livre_antiga()` soltar o numero e o "servidor" (o `bind` que
/// `Servidor::escutar()` faria, mais tarde, com o mesmo numero escrito no
/// `config.json`) ligar nele de verdade, QUALQUER UM pode tomar o numero --
/// aqui, deterministicamente, sem depender de dois processos correrem ao
/// mesmo tempo. O bind do "servidor" falha com `AddrInUse`: e exatamente
/// o "Address already in use" ja medido em producao nesta casa
/// (`servico.rs`, CI de 03/09/2026, citado no historico do arquivo) e a
/// suspeita nomeada no pedido 401 para os dois vermelhos do
/// `cifra-das-portas-http.rs`.
#[test]
fn a_janela_do_padrao_antigo_deixa_qualquer_um_tomar_a_porta() {
    // 1. O padrao antigo "reserva" um numero -- e imediatamente o solta.
    let porta = porta_livre_antiga();

    // 2. A JANELA: entre o passo 1 e o passo 3 (o "bind de verdade" do
    // servidor), o numero esta l-i-v-r-e. Isto NAO e uma corrida por sorte:
    // e uma consequencia direta de `porta_livre_antiga()` ter devolvido um
    // `u16` solto, e nao um soquete preso. Qualquer chamador desta funcao,
    // neste instante, tem exatamente a mesma chance que o "servidor" de
    // pegar o numero -- e este teste ocupa essa chance no lugar dele.
    let ocupante = TcpListener::bind(("127.0.0.1", porta))
        .expect("a porta tinha de estar mesmo solta -- se isto falhar, o passo 1 nao soltou nada");

    // 3. O "servidor" tenta ligar no numero que `porta_livre_antiga()`
    // escolheu -- e encontra a porta OCUPADA por quem chegou primeiro na
    // janela. Isto e o defeito: nada no padrao antigo garante que o
    // "servidor" seja esse alguem.
    let erro = TcpListener::bind(("127.0.0.1", porta)).unwrap_err();
    assert_eq!(
        erro.kind(),
        std::io::ErrorKind::AddrInUse,
        "o padrao antigo tinha de deixar a porta tomavel por outro -- se \
         isto parar de falhar, o sistema operacional mudou de comportamento, \
         nao o defeito que este teste mede"
    );
    drop(ocupante);
}

/// **VERDE: o mesmo ataque, sem janela nenhuma para acontecer -- ENQUANTO se
/// segura o que se pediu.**
///
/// # Correcao de uma hipotese que caiu medindo (registrada aqui de proposito)
///
/// A primeira versao deste teste tentou provar algo mais forte -- "o sistema
/// operacional nunca devolve o mesmo numero de porta efemera duas vezes,
/// mesmo sob disputa" -- lendo a porta e DEVOLVENDO so o `u16`, com o
/// `TcpListener` caindo em escopo. Medido nesta maquina: FALHOU na hora,
/// com duas de cem threads recebendo a MESMA porta. A causa nao e sorteio
/// ruim -- e que soltar o ouvinte devolve o numero ao sistema IMEDIATAMENTE,
/// e outra thread, ainda disputando, podia recebe-lo de volta. E a mesma
/// historia, medida de novo: "nunca repete" so vale enquanto NINGUEM solta.
///
/// Isso e exatamente o que `Servidor::escutar()` faz, e o que este teste
/// prova de verdade: o `TcpListener` do conserto do pedido 401 nunca e
/// solto entre o `bind` e o uso -- ele e o MESMO ouvinte que aceita conexao
/// depois. Aqui, cada thread MANTEM o ouvinte vivo ate o teste conferir
/// todos, exatamente como `Servidor` mantem o dele vivo pela vida inteira do
/// processo. Sob essa condicao -- a condicao real do conserto --, cem
/// threads disputando (`Barrier`, para forcar a disputa de verdade) recebem
/// cem numeros distintos.
#[test]
fn o_padrao_novo_nunca_colide_enquanto_segura_o_que_pediu() {
    const CONCORRENTES: usize = 100;
    let largada = Arc::new(Barrier::new(CONCORRENTES));

    let threads: Vec<_> = (0..CONCORRENTES)
        .map(|_| {
            let largada = Arc::clone(&largada);
            std::thread::spawn(move || {
                // Todas esperam aqui, e so soltam juntas -- e a disputa de
                // verdade que o `porta_livre()` velho corria sem controle
                // nenhum: N chamadores tentando a MESMA coisa no MESMO
                // instante.
                largada.wait();
                // Isto E' o padrao novo: nao ha `porta_livre()` para chamar
                // antes, e o OUVINTE (nao so o numero) e o que a thread
                // devolve -- ele fica vivo ate o `drop` no fim da funcao,
                // exatamente como o de `Servidor::escutar()` fica vivo ate o
                // processo parar.
                TcpListener::bind("127.0.0.1:0").unwrap()
            })
        })
        .collect();

    let ouvintes: Vec<TcpListener> = threads.into_iter().map(|t| t.join().unwrap()).collect();

    let mut vistas = std::collections::HashSet::new();
    for o in &ouvintes {
        let p = o.local_addr().unwrap().port();
        assert!(
            vistas.insert(p),
            "duas threads concorrentes receberam a MESMA porta ({p}) de \
             `bind(\"127.0.0.1:0\")` enquanto as DUAS ainda seguravam o \
             ouvinte -- isto quebraria a garantia em que o conserto do \
             pedido 401 se apoia, e o pedido teria de ser revisto"
        );
    }
    assert_eq!(ouvintes.len(), CONCORRENTES);
}

// ---------------------------------------------------------------------------
// Os dois testes de cima provam o MECANISMO com sockets crus. Os dois daqui
// provam que o `Servidor` de producao se comporta exatamente assim: o mesmo
// ataque que derruba o arranque com o padrao antigo nao alcanca o arranque
// com o padrao novo.
// ---------------------------------------------------------------------------

fn config_minima(dir: &std::path::Path, bind: &str) -> Config {
    let mut c = Config {
        bind: bind.to_string(),
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "prova-401".into(),
        ..Default::default()
    };
    c.web.ligado = false;
    c.cifra_fio.exigir = false;
    c
}

/// **VERMELHO, com o `Servidor` DE VERDADE.** Com o `bind` do `config.json`
/// escrito a partir de `porta_livre_antiga()` -- exatamente como TODOS os
/// arquivos que o pedido 401 corrigiu escreviam antes --, um ocupante que
/// toma a porta na janela faz `Servidor::escutar()` FALHAR o `bind` e
/// retornar `Err` sem nunca atender ninguem. `porta_dos_dados()` nunca sai de
/// `None`: e o mesmo retrato do `Servidor::novo(c).unwrap()`/`escutar()` que
/// os dois vermelhos do `cifra-das-portas-http.rs` (pedido 401) suspeitavam.
#[test]
fn vermelho_com_o_servidor_de_verdade_a_janela_derruba_o_arranque() {
    let dir = DirTemp::novo("corrida-401-vermelho");
    let porta = porta_livre_antiga();
    // O ocupante toma exatamente o numero que o `config.json` vai pedir --
    // a mesma janela do primeiro teste deste arquivo, so que agora o alvo e
    // o `bind` que `Servidor::escutar()` faz de verdade.
    let ocupante = TcpListener::bind(("127.0.0.1", porta)).unwrap();

    let c = config_minima(&dir, &format!("127.0.0.1:{porta}"));
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        // `escutar()` devolve `Err` aqui (o `?` do `TcpListener::bind` no
        // topo da funcao) -- exatamente o "nao consegui escutar em ...: \
        // Address already in use" que o `Servidor::escutar()` produz.
        let _ = copia.escutar();
    });

    // Prazo curto e de proposito: com o defeito, NUNCA vai ficar `Some`,
    // entao esperar mais so gastaria tempo da bateria sem mudar o veredito.
    let ate = Instant::now() + Duration::from_millis(500);
    while Instant::now() < ate && s.porta_dos_dados().is_none() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        s.porta_dos_dados(),
        None,
        "com o padrao antigo e o ocupante na janela, o servidor NUNCA liga -- \
         se isto vier `Some`, o `bind` conseguiu apesar do ocupante, o que \
         desmentiria a propria premissa deste teste"
    );
    drop(ocupante);
}

/// **VERDE: o mesmo ataque, com o `Servidor` DE VERDADE, sem efeito.**
///
/// `bind: "127.0.0.1:0"` -- o padrao que os ~26 arquivos deste crate usam
/// desde o pedido 401. O "ocupante" desta vez nem tem o que ocupar: nao ha
/// numero escolhido por fora para ele mirar, entao ele so pode tentar
/// competir pela MESMA coisa que o servidor -- uma porta 0 dele mesmo -- e
/// perde, porque `bind(0)` nunca devolve numeros repetidos (provado acima).
/// O servidor liga, `porta_dos_dados()` sai de `None`, e uma conexao de
/// verdade completa.
#[test]
fn verde_com_o_servidor_de_verdade_o_mesmo_ataque_nao_impede_o_arranque() {
    let dir = DirTemp::novo("corrida-401-verde");
    // O "ocupante" hostil ainda esta na maquina, competindo por portas
    // efemeras ao mesmo tempo -- so que agora por `bind(0)`, porque e' o
    // unico jogo que existe quando ninguem escolhe numero por fora.
    let _ocupante_concorrente = TcpListener::bind("127.0.0.1:0").unwrap();

    let c = config_minima(&dir, "127.0.0.1:0");
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });

    let porta = comum::porta_real(|| s.porta_dos_dados());
    let alvo: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    let conectou = loop {
        if std::net::TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            break true;
        }
        if Instant::now() > ate {
            break false;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        conectou,
        "o servidor tinha de ligar e atender em {alvo} mesmo com o ocupante \
         concorrente competindo por portas efemeras ao mesmo tempo"
    );
}
