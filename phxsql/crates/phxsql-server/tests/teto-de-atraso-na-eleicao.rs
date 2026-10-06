//! Pedido 313 -- o teto de atraso da eleicao (`maximum_lag_on_failover` do
//! Patroni), provado PELO SOQUETE com dois nos de verdade.
//!
//! # O cenario
//!
//! Cluster de tres: `no1` master, `no2` e `no3` replicas. O `no1` publica no
//! pulso a posicao 1.000 -- o que os clientes dele ja ouviram «gravei» -- e
//! cala. As duas replicas estao na posicao 0: mil eventos atras. Elas se veem
//! (2 de 3, maioria), e a eleicao abre.
//!
//! O `no1` aqui e o proprio teste mandando `cluster_pulso` pelo soquete,
//! como o laco de pulso de um master de verdade manda: e o jeito de ter um
//! master que publicou uma posicao e calou sem matar processo nenhum. A porta
//! dele fica FECHADA, entao a replicacao das duas replicas nao alcanca nada --
//! o atraso e real, e nao uma posicao inventada.
//!
//! # Os dois sentidos
//!
//! * com `cluster.atraso_maximo_na_eleicao` = 10, NINGUEM se promove, e a
//!   degradacao dos dois nos diz por que. O vermelho: sem o filtro do
//!   `cluster::eleger`, o `no2` (menor id no empate) se promove;
//! * sem o campo (o padrao, o arquivo de ontem), o `no2` se promove como
//!   sempre se promoveu.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teto-de-atraso-na-eleicao";

/// Sobe um no do cluster de tres. Fio em claro, pelo escape escrito: o que se
/// mede e a eleicao, nao o aperto de mao.
fn subir_no(
    base: &std::path::Path,
    este: &str,
    ouvinte: TcpListener,
    portas: &[(&str, u16)],
    extra: &str,
) -> Arc<Servidor> {
    let porta_este = ouvinte.local_addr().unwrap().port();
    let nos: Vec<String> = portas
        .iter()
        .map(|(id, p)| format!(r#"{{ "id": "{id}", "endereco": "127.0.0.1", "porta": {p} }}"#))
        .collect();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:{porta_este}",
              "base": "{base_dir}",
              "token": "{TOKEN}",
              "log_acessos": "{log}",
              "seguranca": {{ "blacklist": "{bl}" }},
              "dblink": "{dblink}",
              "jobs": "{jobs}",
              "web": {{ "ligado": false }},
              "cifra_fio": {{ "exigir": false }},
              "replicacao": {{ "papel": "replica", "id_servidor": "{este}" }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 3,
                "pulso_s": 1,
                "cifra": false{extra},
                "nos": [ {nos} ]
              }}
            }}"#,
            base_dir = bar(base.join("base")),
            log = bar(base.join("acessos.log")),
            bl = bar(base.join("blacklist.json")),
            dblink = bar(base.join("dblink.json")),
            jobs = bar(base.join("jobs.json")),
            nos = nos.join(", "),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    comum::no_ar_no_ouvinte(&s, ouvinte);
    s
}

fn pedir(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

/// (papel, degradacao) do no da `porta`.
fn estado(porta: u16) -> (String, Vec<String>) {
    let r = pedir(porta, r#""op":"cluster_estado""#).expect("cluster_estado");
    let r = r.campo("resultado").cloned().unwrap_or(Json::Nulo);
    let degradado: Vec<String> = r
        .campo("degradado")
        .and_then(Json::lista)
        .map(|l| {
            l.iter()
                .filter_map(|m| match m {
                    Json::Texto(t) => Some(t.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    (r.texto_ou("papel", "").to_string(), degradado)
}

/// O `no1` publicando a posicao 1.000 como master, ate as duas replicas o
/// reconhecerem -- e entao calando.
fn master_publica_e_cala(replicas: &[u16]) {
    let ate = Instant::now() + Duration::from_secs(10);
    let mut viram = vec![false; replicas.len()];
    while viram.iter().any(|v| !v) {
        assert!(
            Instant::now() < ate,
            "as replicas nao aceitaram o pulso do no1"
        );
        for (i, p) in replicas.iter().enumerate() {
            let r = pedir(
                *p,
                r#""op":"cluster_pulso","id":"no1","papel":"master","epoca":1,"posicao":1000"#,
            );
            if r.is_some_and(|j| j.booleano_ou("ok", false)) {
                viram[i] = true;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

struct Cluster {
    _nos: Vec<Arc<Servidor>>,
    _dirs: Vec<DirTemp>,
    portas: [u16; 2],
}

fn cluster(nome: &str, extra: &str) -> Cluster {
    let d2 = DirTemp::novo(&format!("{nome}-no2"));
    let d3 = DirTemp::novo(&format!("{nome}-no3"));
    let (o2, p2) = comum::ouvinte_reservado();
    let (o3, p3) = comum::ouvinte_reservado();
    let p1 = comum::porta_fechada();
    let portas = [("no1", p1), ("no2", p2), ("no3", p3)];
    let s2 = subir_no(&d2, "no2", o2, &portas, extra);
    let s3 = subir_no(&d3, "no3", o3, &portas, extra);
    master_publica_e_cala(&[p2, p3]);
    Cluster {
        _nos: vec![s2, s3],
        _dirs: vec![d2, d3],
        portas: [p2, p3],
    }
}

/// **A prova do 313.** Com o teto, ninguem se promove, e os dois DIZEM por
/// que -- nao e o silencio de um cluster quebrado.
#[test]
fn com_o_teto_a_replica_atrasada_nao_se_promove_e_diz_por_que() {
    let c = cluster("teto-313", r#", "atraso_maximo_na_eleicao": 10"#);
    // A janela e de 3 s: a eleicao abre por volta de 3 s depois do ultimo
    // pulso do no1. Espera-se a frase nos DOIS, e o papel e conferido ate
    // bem depois de a eleicao ter tido tempo de promover alguem.
    let ate = Instant::now() + Duration::from_secs(20);
    let mut disseram = [false; 2];
    while disseram.iter().any(|d| !d) {
        for (i, p) in c.portas.iter().enumerate() {
            let (papel, degradado) = estado(*p);
            assert_eq!(
                papel, "replica",
                "a replica 1.000 eventos atras se promoveu com o teto 10: {degradado:?}"
            );
            if degradado
                .iter()
                .any(|m| m.contains("atraso_maximo_na_eleicao") && m.contains("NAO promovo"))
            {
                disseram[i] = true;
            }
        }
        assert!(
            Instant::now() < ate,
            "a eleicao nao disse por que nao promoveu"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    // E continua sem promover nas rodadas seguintes.
    std::thread::sleep(Duration::from_secs(2));
    for p in c.portas {
        assert_eq!(estado(p).0, "replica");
    }
}

/// **O comportamento VELHO.** Sem o campo, a eleicao promove o menos
/// atrasado por mais longe que esteja -- o `no2`, pelo menor id no empate.
#[test]
fn sem_o_teto_a_eleicao_promove_como_sempre() {
    let c = cluster("sem-teto-313", "");
    let ate = Instant::now() + Duration::from_secs(20);
    loop {
        if estado(c.portas[0]).0 == "master" {
            break;
        }
        assert!(
            Instant::now() < ate,
            "sem teto, a eleicao deixou de promover: {:?}",
            estado(c.portas[0])
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(estado(c.portas[1]).0, "replica");
}
