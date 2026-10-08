//! O contador da `Sequence` na promocao de uma replica ATRASADA -- provado PELO
//! SOQUETE, entre um source e uma replica de verdade (pedido 229, c-pleno).
//!
//! # O defeito
//!
//! O contador mora no cabecalho do `.reg` e a replica so o empurrava com os
//! valores que APLICAVA. Replica atrasada, promovida, numerava de onde ELA
//! parou e reemitia numeros que o master ja tinha entregue a clientes (bloco 23
//! de `docs/AUTONUMBER.md`: cinco reemitidos). O contador nao viajava: o
//! `posicao` dizia quantos eventos a tabela tinha, e nao onde a sequencia
//! estava.
//!
//! # Como se atrasa uma replica sem matar nada
//!
//! Um repetidor de linha fica entre os dois. Com o portao fechado ele segura o
//! `replicar` (os eventos) e deixa passar o resto -- inclusive o `posicao`,
//! que e o que carrega o contador. E a replica que esta no ar, falando com o
//! source, e mesmo assim nao recebe as linhas: o atraso de vazao, o unico em
//! que um contador propagado ajuda (com o cabo cortado nada propaga, e nenhum
//! motor promete o contrario).

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "contador-na-promocao";

fn config_base(base: &std::path::Path) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O ESCAPE ESCRITO: a cifra do fio nasce exigida (pedido 370), e o
    // repetidor de linha precisa ler o pedido em claro para segurar so o
    // `replicar`. O que esta bateria mede e o contador, nao o fio.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    c
}

fn subir(c: Config) -> (Arc<Servidor>, u16) {
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    esperar_porta(porta);
    (s, porta)
}

fn esperar_porta(porta: u16) {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("a porta {porta} nao abriu em 5 s");
}

/// As fases do repetidor (pedido 703). Na `SEGURANDO` NENHUMA linha da replica
/// anda; na `DEPOIS` tudo anda e cada `posicao` que sai conta em
/// `posicoes_depois` -- por construcao, so as que o source respondeu quando os
/// inserts ja tinham acabado.
const LIVRE: usize = 0;
const SEGURANDO: usize = 1;
const DEPOIS: usize = 2;

/// O repetidor: cada linha do cliente vai ao destino e a resposta volta. Com o
/// portao FECHADO, a linha que pede `replicar` fica presa (sem resposta).
///
/// Pedido 703: o teste esperava 5 s por «uma rodada que comecou depois dos
/// inserts», mas nao controlava onde os inserts caiam na rodada. Um `posicao`
/// no meio deles adotava um contador parcial e o `replicar` dessa rodada ficava
/// preso no portao ate o silencio da replica (30 s) -- sem rodada nova, o
/// contador certo nao chegava. Medido: um sleep de 1,5 s entre o 1o e o 2o
/// insert faz o teste cair sozinho, 3 de 3 (contador 5, esperava 6). Era
/// corrida de ORDEM que a carga sorteia, nao lentidao; a fase `SEGURANDO`
/// fixa a ordem e `posicoes_depois` e o evento a esperar.
fn repetidor(
    destino: u16,
    fechado: Arc<AtomicBool>,
    fase: Arc<AtomicUsize>,
    posicoes_depois: Arc<AtomicUsize>,
) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for entrada in ouvinte.incoming() {
            let Ok(cliente) = entrada else { continue };
            let fechado = Arc::clone(&fechado);
            let fase = Arc::clone(&fase);
            let posicoes_depois = Arc::clone(&posicoes_depois);
            std::thread::spawn(move || {
                let Ok(origem) = TcpStream::connect(("127.0.0.1", destino)) else {
                    return;
                };
                let mut de_volta = cliente.try_clone().unwrap();
                let mut para_la = origem.try_clone().unwrap();
                let mut lado_do_cliente = BufReader::new(cliente);
                let mut lado_do_source = BufReader::new(origem);
                let mut linha = String::new();
                loop {
                    linha.clear();
                    if lado_do_cliente.read_line(&mut linha).unwrap_or(0) == 0 {
                        return;
                    }
                    while fase.load(Ordering::SeqCst) == SEGURANDO {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    // Lida ANTES de encaminhar: so conta o que o source vai
                    // responder depois de a fase `DEPOIS` ter comecado.
                    let conta = fase.load(Ordering::SeqCst) == DEPOIS
                        && linha.contains(r#""op":"posicao""#);
                    if linha.contains(r#""op":"replicar""#) {
                        while fechado.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                    }
                    if para_la.write_all(linha.as_bytes()).is_err() {
                        return;
                    }
                    let mut resposta = String::new();
                    if lado_do_source.read_line(&mut resposta).unwrap_or(0) == 0 {
                        return;
                    }
                    if conta {
                        posicoes_depois.fetch_add(1, Ordering::SeqCst);
                    }
                    if de_volta.write_all(resposta.as_bytes()).is_err() {
                        return;
                    }
                }
            });
        }
    });
    porta
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    Json::analisar(&resposta)
        .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

fn criar_pedidos(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
           "colunas":[{"nome":"id","tipo":"Sequence"},{"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true}]"#,
    );
}

fn inserir(porta: u16, quantas: usize) {
    for _ in 0..quantas {
        exigir(
            porta,
            r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"nome":"x"}"#,
        );
    }
}

/// Os ids da tabela, ou vazio enquanto ela nao nasceu (na replica, o `varrer`
/// responde `NAO_ENCONTRADO` ate a primeira rodada: «ainda nao chegou»).
fn ids(porta: u16) -> Vec<i64> {
    let r = pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"pedidos","max":100"#,
    );
    r.campo("resultado")
        .and_then(|x| x.campo("linhas"))
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect()
}

/// O `proxima` que `sequencias` mostra, ou -1 se a tabela ainda nao chegou.
fn proxima(porta: u16) -> i64 {
    let r = pedir(porta, r#""op":"sequencias","database":"loja""#);
    r.campo("resultado")
        .and_then(|x| x.campo("sequencias"))
        .and_then(Json::lista)
        .and_then(|l| l.iter().find(|s| s.texto_ou("tabela", "") == "pedidos"))
        .map(|s| s.inteiro_ou("proxima", -1))
        .unwrap_or(-1)
}

fn esperar(quanto: Duration, mut pronto: impl FnMut() -> bool) -> bool {
    let ate = Instant::now() + quanto;
    while Instant::now() < ate {
        if pronto() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

struct Par {
    _pastas: (DirTemp, DirTemp),
    fechado: Arc<AtomicBool>,
    fase: Arc<AtomicUsize>,
    posicoes_depois: Arc<AtomicUsize>,
    porta_source: u16,
    porta_replica: u16,
    _s: Arc<Servidor>,
    _r: Arc<Servidor>,
}

fn par(nome: &str) -> Par {
    let a = DirTemp::novo(&format!("{nome}-source"));
    let b = DirTemp::novo(&format!("{nome}-replica"));
    let mut cs = config_base(&a);
    cs.replicacao.papel = Papel::Source;
    cs.replicacao.id_servidor = "source-do-teste".into();
    cs.replicacao.imagem_da_linha = true;
    let (s, porta_source) = subir(cs);
    let fechado = Arc::new(AtomicBool::new(false));
    let fase = Arc::new(AtomicUsize::new(LIVRE));
    let posicoes_depois = Arc::new(AtomicUsize::new(0));
    let via = repetidor(
        porta_source,
        Arc::clone(&fechado),
        Arc::clone(&fase),
        Arc::clone(&posicoes_depois),
    );
    let mut cr = config_base(&b);
    cr.replicacao.papel = Papel::Replica;
    cr.replicacao.id_servidor = "replica-do-teste".into();
    cr.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: via,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
        espelho: false,
    }];
    let (r, porta_replica) = subir(cr);
    Par {
        _pastas: (a, b),
        fechado,
        fase,
        posicoes_depois,
        porta_source,
        porta_replica,
        _s: s,
        _r: r,
    }
}

/// **Prova real.** A replica tem 2 linhas, o source entregou 5 (ids 1 a 5) e
/// os 3 ultimos eventos ainda nao chegaram. Promovida, a replica numera o
/// proximo: tem de ser o 6, e nunca o 3 que o source ja deu a um cliente.
/// Sem o contador no `posicao`, o 3 sai -- e este teste cai.
#[test]
fn replica_atrasada_promovida_nao_reemite_o_que_o_master_entregou() {
    let p = par("atrasada");
    criar_pedidos(p.porta_source);
    inserir(p.porta_source, 2);
    assert!(
        esperar(Duration::from_secs(20), || ids(p.porta_replica).len() == 2),
        "a replica nao chegou a 2 linhas"
    );
    // Ordem, nao relogio (pedido 703): nada da replica anda enquanto os
    // inserts acontecem, e so depois se espera o EVENTO «um `posicao`
    // respondido depois dos inserts» e o contador que ele carrega. Os 60 s sao
    // so desistencia, nao afirmacao de rapidez.
    p.fase.store(SEGURANDO, Ordering::SeqCst);
    p.fechado.store(true, Ordering::SeqCst);
    inserir(p.porta_source, 3);
    p.fase.store(DEPOIS, Ordering::SeqCst);
    // O contador chega pelo `posicao`, que o portao do `replicar` nao segura.
    let paciencia = Duration::from_secs(60);
    assert!(
        esperar(paciencia, || p.posicoes_depois.load(Ordering::SeqCst) >= 1),
        "nenhum `posicao` passou depois dos inserts em {paciencia:?}"
    );
    let chegou = esperar(paciencia, || proxima(p.porta_replica) >= 6);
    assert_eq!(
        ids(p.porta_replica),
        vec![1, 2],
        "o atraso nao se armou: os eventos chegaram"
    );
    exigir(p.porta_replica, r#""op":"spare_promover""#);
    inserir(p.porta_replica, 1);
    let nova = *ids(p.porta_replica).iter().max().unwrap();
    assert!(
        chegou && nova >= 6,
        "a replica promovida deu o numero {nova} (o master ja entregou ate 5; \
         contador na replica: {})",
        proxima(p.porta_replica)
    );
}

/// O comportamento VELHO: sem atraso nenhum, a promocao continua dando o
/// proximo numero e so ele -- o contador propagado nao abre buraco extra.
#[test]
fn replica_em_dia_promovida_continua_do_proximo_sem_buraco() {
    let p = par("em-dia");
    criar_pedidos(p.porta_source);
    inserir(p.porta_source, 3);
    assert!(
        esperar(Duration::from_secs(20), || ids(p.porta_replica).len() == 3),
        "a replica nao chegou a 3 linhas"
    );
    exigir(p.porta_replica, r#""op":"spare_promover""#);
    inserir(p.porta_replica, 1);
    assert_eq!(ids(p.porta_replica), vec![1, 2, 3, 4]);
    // O source nao e tocado por nada disto.
    assert_eq!(ids(p.porta_source), vec![1, 2, 3]);
}

/// A sequencia NOMEADA nao replica (decisao de 02/10/2026, `AUTONUMBER.md`
/// C.5.3, 4 x 3): a promovida nao tem `.seq`, e pedir o proximo numero dela
/// RECUSA em vez de recomecar calada no `inicio`. Pina a decisao -- nao e
/// conserto: se um dia ela replicar, este teste tem de ser reescrito junto com
/// a decisao, e nao apagado.
#[test]
fn sequencia_nomeada_nao_replica_e_a_promovida_recusa_em_vez_de_recomecar() {
    let p = par("nomeada");
    criar_pedidos(p.porta_source);
    exigir(
        p.porta_source,
        r#""op":"criar_sequencia","database":"loja","sequencia":"nf","inicio":1000"#,
    );
    for _ in 0..3 {
        exigir(
            p.porta_source,
            r#""op":"proximo_da_sequencia","database":"loja","sequencia":"nf""#,
        );
    }
    inserir(p.porta_source, 1);
    assert!(
        esperar(Duration::from_secs(20), || ids(p.porta_replica).len() == 1),
        "a replica nao chegou"
    );
    exigir(p.porta_replica, r#""op":"spare_promover""#);
    let r = pedir(
        p.porta_replica,
        r#""op":"proximo_da_sequencia","database":"loja","sequencia":"nf""#,
    );
    assert!(
        !r.booleano_ou("ok", true),
        "a promovida devolveu um numero de uma sequencia que nunca viu: {}",
        r.escrever()
    );
}
