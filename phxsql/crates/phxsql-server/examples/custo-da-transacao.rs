//! O que uma transacao custa, e o que as travas dela compram.
//!
//! ```bash
//! cargo build --release --examples -p phxsql-server   # binario velho mede o passado
//! cargo run --release -p phxsql-server --example custo-da-transacao
//! ```
//!
//! Tres medidas, e cada uma responde a uma pergunta que virou decisao:
//!
//! 1. **O TETO do group commit.** Uma receita de fora se mede contra o nosso
//!    gargalo antes de virar plano. O group commit existe para amortizar o
//!    `fsync` entre commits concorrentes; o teto do que ele poderia comprar e
//!    a razao entre um commit COM `fsync` e o mesmo trabalho com o `fsync`
//!    amortizado. **Criterio de morte acordado antes de medir: abaixo de 1,5x
//!    a hipotese morre**, e a recusa vai para o `DESEMPENHO.md` com o numero.
//!
//! 2. **`LOCK MODE AUTO` contra `EXCLUSIVE`, com N caixas.** O caso que matou
//!    o exclusivo-por-padrao: quinhentos caixas vendendo, um mexendo no
//!    pedido 9001 e outro no 18223. Sem disputa real -- e trava de tabela
//!    criaria uma disputa artificial.
//!
//! 3. **Otimista (`versao`) contra pessimista (trava de linha), na MESMA
//!    linha.** Os dois resolvem escrita-contra-escrita, e nenhum e o certo
//!    sempre: o otimista nunca bloqueia e ganha em disputa baixa; o pessimista
//!    ganha quando muita gente disputa a mesma linha, onde o otimista vira
//!    tentativa e erro.
//!
//! O medidor fala com o servidor pela API interna, e nao pelo soquete: o que
//! se quer medir aqui e o comportamento das TRAVAS, e a rede so acrescentaria
//! ruido igual aos tres cenarios. A prova pelo soquete e outra, e mora em
//! `bancada/transacoes/provar.py`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Servidor};

/// Todo pedido leva o token: o despachar confere a chave da porta antes de
/// qualquer coisa, e o executor local passa pelo MESMO portao.
fn pedido(txt: &str) -> Json {
    let j = Json::analisar(txt).unwrap();
    let Json::Objeto(mut pares) = j else {
        panic!("o pedido tem de ser um objeto: {txt}");
    };
    pares.push(("token".to_string(), Json::texto_de("t")));
    Json::Objeto(pares)
}

fn dir_limpo(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phx-custo-tx-{}-{rotulo}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn servidor(rotulo: &str) -> (Arc<Servidor>, std::path::PathBuf) {
    servidor_com(rotulo, phxsql_server::config::Durabilidade::PorLote)
}

fn servidor_com(
    rotulo: &str,
    durabilidade: phxsql_server::config::Durabilidade,
) -> (Arc<Servidor>, std::path::PathBuf) {
    let dir = dir_limpo(rotulo);
    let mut c = Config {
        base: dir.clone(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.recursos.durabilidade = durabilidade;
    (Servidor::novo(c).unwrap(), dir)
}

/// Um cliente com ligacao propria -- e a ligacao que faz a transacao ser dela.
fn cliente(s: &Arc<Servidor>, ligacao: u64) -> ExecutorLocal {
    ExecutorLocal::com_ligacao(Arc::clone(s), "medidor", ligacao)
}

fn montar(e: &ExecutorLocal, linhas: u64) {
    use phxsql_server::mcp::Executor as _;
    e.executar(&pedido(r#"{"op":"criar_database","database":"loja"}"#))
        .unwrap();
    e.executar(&pedido(
        r#"{"op":"criar_tabela","database":"loja","tabela":"pedidos",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"valor","tipo":"Int8"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]}"#,
    ))
    .unwrap();
    if linhas == 0 {
        return;
    }
    let itens: Vec<String> = (1..=linhas)
        .map(|i| format!(r#"{{"id":{i},"valor":0}}"#))
        .collect();
    e.executar(&pedido(&format!(
        r#"{{"op":"inserir_lote","database":"loja","tabela":"pedidos","linhas":[{}]}}"#,
        itens.join(",")
    )))
    .unwrap();
}

// ------------------------------------------------ 1. o teto do group commit

/// `n` commits de UMA linha.
///
/// `por_operacao` reproduz o comportamento ANTERIOR ao group commit byte a
/// byte: a janela de durabilidade fecha em toda gravacao, entao cada commit
/// paga o `fsync` da tabela mais o da marca. `por_lote` -- o padrao -- e o
/// comportamento de hoje, em que o `fsync` da tabela entra na janela e a marca
/// espera por ele.
fn commits(n: u64, durabilidade: phxsql_server::config::Durabilidade) -> f64 {
    use phxsql_server::mcp::Executor as _;
    let (s, _dir) = servidor_com("commits", durabilidade);
    let e = cliente(&s, 1);
    montar(&e, 0);
    let comeco = Instant::now();
    for i in 1..=n {
        e.executar(&pedido(r#"{"op":"begin"}"#)).unwrap();
        e.executar(&pedido(&format!(
            r#"{{"op":"inserir","database":"loja","tabela":"pedidos","linha":{{"id":{i},"valor":1}}}}"#
        )))
        .unwrap();
        e.executar(&pedido(r#"{"op":"commit"}"#)).unwrap();
    }
    comeco.elapsed().as_secs_f64() * 1000.0 / n as f64
}

/// O MESMO trabalho com o `fsync` amortizado: `n` linhas, UM commit.
///
/// E o teto do que o group commit poderia comprar -- e o teto e generoso de
/// proposito, porque o group commit de verdade ainda paga um `fsync` por
/// GRUPO, e este nao paga quase nenhum.
fn commits_com_fsync_amortizado(n: u64) -> f64 {
    use phxsql_server::mcp::Executor as _;
    let (s, _dir) = servidor("sem-fsync");
    let e = cliente(&s, 1);
    montar(&e, 0);
    let comeco = Instant::now();
    e.executar(&pedido(r#"{"op":"begin"}"#)).unwrap();
    for i in 1..=n {
        e.executar(&pedido(&format!(
            r#"{{"op":"inserir","database":"loja","tabela":"pedidos","linha":{{"id":{i},"valor":1}}}}"#
        )))
        .unwrap();
    }
    e.executar(&pedido(r#"{"op":"commit"}"#)).unwrap();
    comeco.elapsed().as_secs_f64() * 1000.0 / n as f64
}

/// `n` insercoes SOLTAS, sem transacao nenhuma.
///
/// E a linha de base que decompoe o numero de cima: a insercao comum passa
/// pela janela de durabilidade e nao escreve marca nenhuma. O que sobrar entre
/// ela e o commit e o que a TRANSACAO acrescentou.
fn insercoes_soltas(n: u64) -> f64 {
    use phxsql_server::mcp::Executor as _;
    let (s, _dir) = servidor("soltas");
    let e = cliente(&s, 1);
    montar(&e, 0);
    let comeco = Instant::now();
    for i in 1..=n {
        e.executar(&pedido(&format!(
            r#"{{"op":"inserir","database":"loja","tabela":"pedidos","linha":{{"id":{i},"valor":1}}}}"#
        )))
        .unwrap();
    }
    comeco.elapsed().as_secs_f64() * 1000.0 / n as f64
}

/// So a MARCA: gravar `transacao_<id>.tx`, sincronizar e apagar.
///
/// Isola o `fsync` do ponto de compromisso -- o unico que a transacao NAO
/// pode adiar, porque e ele que decide se ela aconteceu.
fn so_a_marca(n: u64) -> f64 {
    let dir = dir_limpo("marca");
    let escritas = vec![phxsql_server::transacao::Escrita {
        database: "loja".into(),
        tabela: "pedidos".into(),
        acao: phxsql_server::transacao::Acao::Inserir,
        rowid: 1,
        linha: vec![
            phxsql_core::value::Value::Int(1),
            phxsql_core::value::Value::Int(2),
        ],
        linha_antiga: Vec::new(),
        motivo: String::new(),
    }];
    let comeco = Instant::now();
    for i in 1..=n {
        let c = phxsql_server::transacao::gravar_marca(&dir, i, 0, &escritas).unwrap();
        std::fs::remove_file(c).unwrap();
    }
    comeco.elapsed().as_secs_f64() * 1000.0 / n as f64
}

// ------------------------------------------------- 2. AUTO contra EXCLUSIVE

/// N caixas, cada um numa transacao, cada um numa LINHA diferente.
///
/// Devolve `(ms totais, quantos passaram, quantos foram barrados)`.
fn caixas(modo: &str, caixas: u64, linhas: u64) -> (f64, u64, u64) {
    use phxsql_server::mcp::Executor as _;
    let (s, _dir) = servidor(&format!("caixas-{modo}"));
    montar(&cliente(&s, 999), linhas);

    let passaram = Arc::new(AtomicU64::new(0));
    let barrados = Arc::new(AtomicU64::new(0));
    let comeco = Instant::now();
    let mut linhas_de_execucao = Vec::new();
    for c in 0..caixas {
        let s = Arc::clone(&s);
        let modo = modo.to_string();
        let (p, b) = (Arc::clone(&passaram), Arc::clone(&barrados));
        linhas_de_execucao.push(std::thread::spawn(move || {
            let e = cliente(&s, c + 1);
            // Cada caixa mexe na SUA linha -- e a disputa artificial e
            // exatamente o que se quer medir.
            let alvo = (c % linhas) + 1;
            let abriu = e.executar(&pedido(&format!(
                r#"{{"op":"begin","database":"loja","scope":["pedidos"],
                     "lock_mode":"{modo}","lock_timeout":"200ms"}}"#
            )));
            if abriu.is_err() {
                b.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let r = e.executar(&pedido(&format!(
                r#"{{"op":"atualizar","database":"loja","tabela":"pedidos","rowid":{alvo},
                     "linha":{{"id":{alvo},"valor":{c}}}}}"#
            )));
            if r.is_ok() && e.executar(&pedido(r#"{"op":"commit"}"#)).is_ok() {
                p.fetch_add(1, Ordering::Relaxed);
            } else {
                b.fetch_add(1, Ordering::Relaxed);
                let _ = e.executar(&pedido(r#"{"op":"rollback"}"#));
            }
        }));
    }
    for t in linhas_de_execucao {
        let _ = t.join();
    }
    (
        comeco.elapsed().as_secs_f64() * 1000.0,
        passaram.load(Ordering::Relaxed),
        barrados.load(Ordering::Relaxed),
    )
}

// ------------------------------------------- 3. otimista contra pessimista

/// N clientes disputando a MESMA linha.
///
/// `otimista`: sem transacao, mandando `"versao"` e repetindo quando o
/// servidor recusa -- que e o controle que ja existe desde o pedido 123.
/// `pessimista`: com transacao, esperando a trava da linha.
///
/// Devolve `(ms totais, quantos passaram, quantas TENTATIVAS foram gastas)`.
fn mesma_linha(otimista: bool, clientes: u64) -> (f64, u64, u64) {
    use phxsql_server::mcp::Executor as _;
    let (s, _dir) = servidor(if otimista { "otimista" } else { "pessimista" });
    montar(&cliente(&s, 999), 1);

    let passaram = Arc::new(AtomicU64::new(0));
    let tentativas = Arc::new(AtomicU64::new(0));
    let comeco = Instant::now();
    let mut fios = Vec::new();
    for c in 0..clientes {
        let s = Arc::clone(&s);
        let (p, t) = (Arc::clone(&passaram), Arc::clone(&tentativas));
        fios.push(std::thread::spawn(move || {
            let e = cliente(&s, c + 1);
            for _ in 0..50 {
                t.fetch_add(1, Ordering::Relaxed);
                if otimista {
                    // Le a versao, grava com ela, repete se o servidor recusar.
                    let atual = e
                        .executar(&pedido(
                            r#"{"op":"ler","database":"loja","tabela":"pedidos","rowid":1,"com_versao":true}"#,
                        ))
                        .unwrap();
                    let versao = atual.campo("versao").and_then(Json::inteiro).unwrap_or(0);
                    let r = e.executar(&pedido(&format!(
                        r#"{{"op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
                             "versao":{versao},"linha":{{"id":1,"valor":{c}}}}}"#
                    )));
                    if r.is_ok() {
                        p.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                } else {
                    let _ = e.executar(&pedido(
                        r#"{"op":"begin","database":"loja","scope":["pedidos"],"lock_timeout":"2s"}"#,
                    ));
                    let r = e.executar(&pedido(&format!(
                        r#"{{"op":"atualizar","database":"loja","tabela":"pedidos","rowid":1,
                             "linha":{{"id":1,"valor":{c}}}}}"#
                    )));
                    if r.is_ok() && e.executar(&pedido(r#"{"op":"commit"}"#)).is_ok() {
                        p.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    let _ = e.executar(&pedido(r#"{"op":"rollback"}"#));
                }
            }
        }));
    }
    for f in fios {
        let _ = f.join();
    }
    (
        comeco.elapsed().as_secs_f64() * 1000.0,
        passaram.load(Ordering::Relaxed),
        tentativas.load(Ordering::Relaxed),
    )
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(200);
    let quantos_caixas: u64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(64);
    let rodadas = 5;

    println!("== 1. o TETO do group commit ==");
    println!("{n} commits de UMA linha, {rodadas} rodadas intercaladas\n");
    // Intercaladas: medir um cenario inteiro de cada vez poe toda a deriva da
    // maquina dentro de um deles, e ela vira "custo".
    let (mut com, mut sem) = (Vec::new(), Vec::new());
    let mut antes = Vec::new();
    for _ in 0..rodadas {
        antes.push(commits(n, phxsql_server::config::Durabilidade::PorOperacao));
        com.push(commits(n, phxsql_server::config::Durabilidade::PorLote));
        sem.push(commits_com_fsync_amortizado(n));
    }
    let antes = mediana(antes);
    let (mut soltas, mut marcas) = (Vec::new(), Vec::new());
    for _ in 0..rodadas {
        soltas.push(insercoes_soltas(n));
        marcas.push(so_a_marca(n));
    }
    let (com, sem) = (mediana(com), mediana(sem));
    let (soltas, marcas) = (mediana(soltas), mediana(marcas));
    println!("  ANTES: fsync por commit .. {antes:8.3} ms   (por_operacao: a janela nao existe)");
    println!("  HOJE:  fsync na janela ... {com:8.3} ms   (por_lote: o group commit)");
    println!(
        "  ganho do group commit .... {:8.2}x",
        antes / com.max(f64::MIN_POSITIVE)
    );
    println!("  1 linha no commit grande . {sem:8.3} ms   (fsync amortizado)");
    println!("  insercao SOLTA, sem tx ... {soltas:8.3} ms   (janela de durabilidade)");
    println!("  so a marca .tx ........... {marcas:8.3} ms   (o ponto de compromisso)");
    let teto = com / sem.max(f64::MIN_POSITIVE);
    println!("\n  TETO bruto ............... {teto:8.2}x");
    // O que a marca cobra NAO e amortizavel: ela E o ponto de compromisso, e
    // adia-la e adiar a decisao de que a transacao aconteceu. O teto honesto
    // desconta esse piso.
    let piso = marcas + sem;
    let teto_real = com / piso.max(f64::MIN_POSITIVE);
    println!("  piso irredutivel ......... {piso:8.3} ms   (marca + trabalho)");
    println!("  TETO honesto ............. {teto_real:8.2}x");
    println!(
        "  criterio de morte: 1,50x  ->  {}",
        if teto_real >= 1.5 {
            "a hipotese VIVE"
        } else {
            "a hipotese MORRE: o que sobra e a marca, e ela nao se adia"
        }
    );

    println!("\n== 2. LOCK MODE AUTO contra EXCLUSIVE ==");
    println!("{quantos_caixas} caixas, cada um numa linha diferente\n");
    for modo in ["AUTO", "EXCLUSIVE"] {
        let mut tempos = Vec::new();
        let (mut ok, mut nao) = (0, 0);
        for _ in 0..rodadas {
            let (ms, p, b) = caixas(modo, quantos_caixas, quantos_caixas);
            tempos.push(ms);
            ok = p;
            nao = b;
        }
        println!(
            "  {modo:<10} {:8.1} ms   passaram {ok:3}   barrados {nao:3}",
            mediana(tempos)
        );
    }

    println!("\n== 3. otimista (versao) contra pessimista (trava), MESMA linha ==");
    println!("{quantos_caixas} clientes na linha 1\n");
    for (rotulo, otimista) in [("otimista", true), ("pessimista", false)] {
        let mut tempos = Vec::new();
        let (mut ok, mut tent) = (0, 0);
        for _ in 0..rodadas {
            let (ms, p, t) = mesma_linha(otimista, quantos_caixas);
            tempos.push(ms);
            ok = p;
            tent = t;
        }
        println!(
            "  {rotulo:<12} {:8.1} ms   passaram {ok:3}   tentativas {tent:4}",
            mediana(tempos)
        );
    }
    println!("\nOs numeros vao para docs/DESEMPENHO.md -- medidos, nunca estimados.");
}
