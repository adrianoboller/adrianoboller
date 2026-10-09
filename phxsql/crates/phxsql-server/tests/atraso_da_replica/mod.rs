//! Apoio das duas provas do atraso da replica (pedido 496, F7): um source e
//! uma replica fiel de verdade, conversando pelo soquete, no molde do
//! `continuidade-da-replica.rs`.
//!
//! # Por que dois arquivos, e nao dois testes num arquivo
//!
//! O alarme de servidor vai ao sedimento, que e do PROCESSO, e a ocorrencia
//! sem tarefa vai a camada do ultimo servidor que subiu
//! (`ocorrencias::do_processo`). Dois testes no mesmo binario correm em
//! paralelo, e os servidores de um continuam no ar depois de ele acabar: a
//! replica parada de um entregaria a ocorrencia dela na camada da replica em
//! dia do outro, e a prova do comportamento velho cairia por um alarme que
//! nao e dela. Cada arquivo de `tests/` e um processo.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

use crate::comum;

pub const TOKEN: &str = "atraso";

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
    // O ESCAPE ESCRITO, o mesmo da `continuidade-da-replica.rs`: o que se
    // mede aqui e o atraso, e nao a cifra do fio.
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

pub fn subir_source(base: &std::path::Path) -> (Arc<Servidor>, u16) {
    subir_source_com(base, true)
}

/// `protecao` e o interruptor de teste da camada de protecao (765/767).
pub fn subir_source_com(base: &std::path::Path, protecao: bool) -> (Arc<Servidor>, u16) {
    let mut c = config_base(base);
    c.protecao.ligada = protecao;
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "source-do-atraso".into();
    c.replicacao.imagem_da_linha = true;
    subir(c)
}

/// A replica sobe DEPOIS do source: e ela a ultima a se instalar como a
/// camada de ocorrencias do processo, e e nela que o alarme sem tarefa cai.
///
/// Sem `somente_leitura` e com a lista de databases vazia: e o arranjo em que
/// a escrita local e aceita, e a escrita local e o jeito de PARAR a tabela
/// pelo caminho real (a continuidade rompe, pedido 300 (4)).
pub fn subir_replica(base: &std::path::Path, porta_do_source: u16) -> (Arc<Servidor>, u16) {
    let mut c = config_base(base);
    c.somente_leitura = false;
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-do-atraso".into();
    c.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: porta_do_source,
        token: TOKEN.into(),
        databases: vec![],
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
    subir(c)
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

pub fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
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

pub fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

pub fn preparar_source(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

pub fn inserir(porta: u16, id: i64) {
    exigir(
        porta,
        &format!(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{id}}}"#),
    );
}

/// Eventos de `loja/clientes` num servidor; -1 enquanto a tabela nao chegou.
pub fn eventos(porta: u16) -> i64 {
    let r = pedir(porta, r#""op":"posicao","database":"loja""#);
    r.campo("resultado")
        .and_then(|res| res.campo("tabelas"))
        .and_then(|t| t.campo("clientes"))
        .map(|c| c.inteiro_ou("eventos", -1))
        .unwrap_or(-1)
}

pub fn esperar_eventos(porta: u16, quantos: i64) {
    let ate = Instant::now() + Duration::from_secs(20);
    while Instant::now() < ate {
        if eventos(porta) == quantos {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "a replica nao chegou a {quantos} evento(s) em 20 s (esta em {})",
        eventos(porta)
    );
}

/// O atraso de `loja/clientes` na origem `fonte`, como o `replicacao_estado`
/// o expoe; `None` antes da primeira amostra.
pub fn atraso(porta_replica: u16) -> Option<Json> {
    exigir(porta_replica, r#""op":"replicacao_estado""#)
        .campo("origens")
        .and_then(|o| o.campo("fonte"))
        .and_then(|f| f.campo("atrasos"))
        .and_then(|a| a.campo("loja/clientes"))
        .cloned()
}

/// As linhas do `ocorrencias.log` da replica com o alarme do atraso.
pub fn ocorrencias_do_atraso(base_replica: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(base_replica.join("ocorrencias.log"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(r#""alarme":"replica_atrasada""#))
        .map(str::to_string)
        .collect()
}

/// Quantas vezes o alarme do atraso foi ao sedimento deste processo.
pub fn pedras_do_atraso() -> u64 {
    phxsql_server::aquario::alarme::sedimento()
        .into_iter()
        .find(|p| p.alarme == phxsql_server::aquario::Alarme::ReplicaAtrasada)
        .map_or(0, |p| p.vezes)
}

/// Uma ponte TCP entre a replica e o source, que o teste CORTA: o fio cai
/// de verdade (conexoes vivas fechadas, porta nova recusada), sem precisar
/// derrubar o servidor do source -- que nao tem como sair do ar no meio do
/// processo de teste.
pub struct Ponte {
    pub porta: u16,
    cortada: Arc<AtomicBool>,
    vivas: Arc<std::sync::Mutex<Vec<TcpStream>>>,
}

impl Ponte {
    pub fn ate(destino: u16) -> Ponte {
        let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        ouvinte.set_nonblocking(true).unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let cortada = Arc::new(AtomicBool::new(false));
        let vivas: Arc<std::sync::Mutex<Vec<TcpStream>>> = Arc::default();
        let (c, v) = (Arc::clone(&cortada), Arc::clone(&vivas));
        std::thread::spawn(move || {
            // O ouvinte morre com a thread: cortada, a porta passa a recusar.
            while !c.load(Ordering::SeqCst) {
                match ouvinte.accept() {
                    Ok((de_la, _)) => {
                        de_la.set_nonblocking(false).unwrap();
                        let Ok(para) = TcpStream::connect(("127.0.0.1", destino)) else {
                            continue;
                        };
                        let mut guardadas = v.lock().unwrap();
                        guardadas.push(de_la.try_clone().unwrap());
                        guardadas.push(para.try_clone().unwrap());
                        for (mut a, mut b) in [
                            (de_la.try_clone().unwrap(), para.try_clone().unwrap()),
                            (para, de_la),
                        ] {
                            std::thread::spawn(move || {
                                let _ = std::io::copy(&mut a, &mut b);
                                let _ = b.shutdown(std::net::Shutdown::Both);
                            });
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Ponte {
            porta,
            cortada,
            vivas,
        }
    }

    pub fn cortar(&self) {
        self.cortada.store(true, Ordering::SeqCst);
        for s in self.vivas.lock().unwrap().drain(..) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
}

/// O estado da origem `fonte` inteiro, como o `replicacao_estado` o expoe.
pub fn estado_da_fonte(porta_replica: u16) -> Json {
    exigir(porta_replica, r#""op":"replicacao_estado""#)
        .campo("origens")
        .and_then(|o| o.campo("fonte"))
        .cloned()
        .unwrap_or(Json::Nulo)
}

/// O mestre gravando: um `inserir` a cada `passo`, numa thread, ate a guarda
/// cair. Ids a partir de `primeiro`.
pub struct Escritor {
    parar: Arc<AtomicBool>,
    pub escritos: Arc<AtomicI64>,
    fio: Option<std::thread::JoinHandle<()>>,
}

impl Escritor {
    pub fn comecar(porta: u16, primeiro: i64, passo: Duration) -> Escritor {
        let parar = Arc::new(AtomicBool::new(false));
        let escritos = Arc::new(AtomicI64::new(0));
        let (p, e) = (Arc::clone(&parar), Arc::clone(&escritos));
        let fio = std::thread::spawn(move || {
            let mut id = primeiro;
            while !p.load(Ordering::SeqCst) {
                inserir(porta, id);
                id += 1;
                e.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(passo);
            }
        });
        Escritor {
            parar,
            escritos,
            fio: Some(fio),
        }
    }
}

impl Drop for Escritor {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::SeqCst);
        if let Some(f) = self.fio.take() {
            let _ = f.join();
        }
    }
}
