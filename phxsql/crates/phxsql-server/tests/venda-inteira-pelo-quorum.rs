//! Pedido 681: a venda de varias tabelas chega INTEIRA tambem pelo lote do
//! quorum -- provado PELO SOQUETE, com um master e uma replica de cluster.
//!
//! # O defeito
//!
//! O 676 juntou o pull por transacao, e o caminho irmao ficou: o lote do
//! quorum (`replicar_aguardar`) trazia um lote por tabela do commit, e a
//! replica aplicava cada um sob a propria tomada da trava, com um `fsync`
//! entre eles. Um leitor da replica que entrasse entre dois lotes via os
//! itens sem a venda -- um estado que nunca existiu no master.
//!
//! # Como se ve, sem sorte demais
//!
//! O fio nao cai aqui: a resposta do quorum e uma linha so, e a replica a le
//! inteira antes de aplicar. O que se mede e o MEIO da aplicacao: um leitor
//! em laco fotografa a replica enquanto o master grava sessenta vendas, cada
//! uma num `COMMIT` com a venda, os itens e o pagamento. A foto e o
//! sanduiche do `venda-inteira-na-replica.rs` -- `vendas`, `itens`,
//! `pagamentos` e `vendas` de novo, valendo so se as duas `vendas` batem --,
//! porque tres pedidos sem ele dariam falso positivo: o grupo aplicado entre
//! eles produz uma foto que nunca existiu.
//!
//! Com o defeito reposto (cada lote do quorum aplicado sozinho, a forma de
//! antes), as tres tabelas sao tres tomadas separadas por dois `fsync`, e o
//! leitor cai entre elas: medido, varias fotos pela metade por corrida.
#![cfg(unix)]

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "venda-inteira-pelo-quorum";
// 60 x 10 = 600 itens: abaixo do `max_linhas` de fabrica (1000), que o
// `varrer` da contagem respeita.
const VENDAS: usize = 60;
const ITENS: usize = 10;

fn config_do_no(
    base: &Path,
    este: &str,
    papel: &str,
    portas: &[(String, u16)],
) -> std::path::PathBuf {
    let porta_este = portas.iter().find(|(id, _)| id == este).unwrap().1;
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    let nos: Vec<String> = portas
        .iter()
        .map(|(id, p)| format!(r#"{{ "id": "{id}", "endereco": "127.0.0.1", "porta": {p} }}"#))
        .collect();
    let caminho = base.join("config.json");
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
              "replicacao": {{ "papel": "{papel}", "id_servidor": "{este}", "imagem_da_linha": true }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 30,
                "pulso_s": 1,
                "cifra": false,
                "quorum_minimo": 1,
                "quorum_prazo_ms": 10000,
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
    caminho
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().ok()?;
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

/// Uma conexao que FICA -- a transacao vive na sessao.
struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let fluxo = TcpStream::connect(("127.0.0.1", porta)).unwrap();
        fluxo.set_nodelay(true).unwrap();
        fluxo
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        Ligacao {
            escrita: fluxo.try_clone().unwrap(),
            leitor: BufReader::new(fluxo),
        }
    }

    fn exigir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        let j = Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"));
        assert!(j.booleano_ou("ok", false), "{corpo} -> {r}");
        j
    }
}

fn contar(porta: u16, tabela: &str) -> usize {
    tentar(
        porta,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":5000"#),
    )
    .and_then(|r| r.campo("resultado").cloned())
    .and_then(|r| r.campo("linhas").and_then(Json::lista).map(<[Json]>::len))
    .unwrap_or(0)
}

/// O sanduiche -- ver o cabecalho e o `retrato` do `venda-inteira-na-replica.rs`.
fn retrato(porta: u16) -> (usize, usize, usize) {
    loop {
        let antes = contar(porta, "vendas");
        let itens = contar(porta, "itens");
        let pagamentos = contar(porta, "pagamentos");
        if contar(porta, "vendas") == antes {
            return (antes, itens, pagamentos);
        }
    }
}

fn replicas_no_canal(porta: u16) -> usize {
    tentar(porta, r#""op":"replicacao_estado""#)
        .and_then(|r| r.campo("resultado").cloned())
        .and_then(|r| r.campo("quorum").cloned())
        .and_then(|q| match q.campo("replicas") {
            Some(Json::Objeto(p)) => Some(p.len()),
            _ => None,
        })
        .unwrap_or(0)
}

/// **A prova real do 681 no quorum.** Sessenta vendas, cada uma um `COMMIT` de
/// tres tabelas confirmado pela replica; nenhuma foto da replica, tirada no
/// meio da aplicacao, mostra uma venda pela metade.
///
/// **Defeito reposto** (cada lote do quorum aplicado sozinho -- a guarda
/// `quorum-aplica-lote-a-lote` do catalogo): o leitor fotografa itens sem a
/// venda e o teste cai na asercao das fotos.
#[test]
fn a_venda_pelo_quorum_nunca_aparece_pela_metade() {
    let (o1, p1) = comum::ouvinte_reservado();
    let (o2, p2) = comum::ouvinte_reservado();
    let portas = vec![("no1".to_string(), p1), ("no2".to_string(), p2)];
    let d1 = DirTemp::novo("quorum-venda-no1");
    let d2 = DirTemp::novo("quorum-venda-no2");
    let c1 = config_do_no(&d1, "no1", "source", &portas);
    let c2 = config_do_no(&d2, "no2", "replica", &portas);
    let master = Servidor::novo(Config::ler(&c1).unwrap()).unwrap();
    comum::no_ar_no_ouvinte(&master, o1);
    let replica = Servidor::novo(Config::ler(&c2).unwrap()).unwrap();
    comum::no_ar_no_ouvinte(&replica, o2);

    let mut l = Ligacao::nova(p1);
    l.exigir(r#""op":"criar_database","database":"loja""#);
    for tabela in ["vendas", "itens", "pagamentos"] {
        l.exigir(&format!(
            r#""op":"criar_tabela","database":"loja","tabela":"{tabela}",
               "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                          {{"nome":"venda","tipo":"Int8"}}]"#
        ));
    }
    let ate = Instant::now() + Duration::from_secs(40);
    while replicas_no_canal(p1) < 1 {
        assert!(
            Instant::now() < ate,
            "a replica nao abriu o canal do quorum"
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    // O leitor em laco, ate o escritor acabar.
    let parar = Arc::new(AtomicBool::new(false));
    let pela_metade: Arc<Mutex<Vec<(usize, usize, usize)>>> = Arc::default();
    let fotos = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let leitor = {
        let (parar, pela_metade, fotos) = (
            Arc::clone(&parar),
            Arc::clone(&pela_metade),
            Arc::clone(&fotos),
        );
        std::thread::spawn(move || {
            while !parar.load(Ordering::SeqCst) {
                let (v, i, pg) = retrato(p2);
                fotos.fetch_add(1, Ordering::Relaxed);
                if i != v * ITENS || pg != v {
                    pela_metade.lock().unwrap().push((v, i, pg));
                }
            }
        })
    };

    let mut item = 0;
    for v in 1..=VENDAS {
        l.exigir(r#""op":"begin","database":"loja""#);
        l.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"vendas","linha":{{"id":{v},"venda":{v}}}"#
        ));
        for _ in 0..ITENS {
            item += 1;
            l.exigir(&format!(
                r#""op":"inserir","database":"loja","tabela":"itens","linha":{{"id":{item},"venda":{v}}}"#
            ));
        }
        l.exigir(&format!(
            r#""op":"inserir","database":"loja","tabela":"pagamentos","linha":{{"id":{v},"venda":{v}}}"#
        ));
        let r = l.exigir(r#""op":"commit""#);
        let q = r
            .campo("resultado")
            .and_then(|x| x.campo("quorum"))
            .or_else(|| r.campo("quorum"))
            .cloned()
            .unwrap_or(Json::Nulo);
        assert!(
            q.booleano_ou("alcancado", false),
            "o COMMIT {v} nao alcancou o quorum: {}",
            r.escrever()
        );
    }
    parar.store(true, Ordering::SeqCst);
    leitor.join().unwrap();

    let metades = pela_metade.lock().unwrap().clone();
    assert!(
        metades.is_empty(),
        "a replica do quorum mostrou {} foto(s) com a venda PELA METADE em {} -- \
         (vendas, itens, pagamentos), as primeiras: {:?}",
        metades.len(),
        fotos.load(Ordering::Relaxed),
        &metades[..metades.len().min(5)]
    );
    assert_eq!(retrato(p2), (VENDAS, VENDAS * ITENS, VENDAS));
    let _ = tentar(p2, r#""op":"servico_parar""#);
    let _ = tentar(p1, r#""op":"servico_parar""#);
}
