//! O que o COMMIT de uma transacao GRANDE com mae e filha na mesma lista custa.
//!
//! ```bash
//! ./cargo-da-frente.sh build --release --offline --examples -p phxsql-server
//! target/release/examples/custo-da-pre-conferencia 10000 3
//! target/release/examples/custo-da-pre-conferencia 100000 3
//! target/release/examples/custo-da-pre-conferencia chaves 3   # o achado A2
//! ```
//!
//! Existe por causa do pedido 448: a pre-conferencia da lista inteira ANTES da
//! marca le por uma sobreposicao cujo `buscar` era LINEAR no que nasceu na
//! transacao -- mae e filha na mesma lista viram O(n^2), e o teto padrao e
//! 100.000 linhas. O parecer do DBA mandou medir antes de embarcar, e o numero
//! decide se a sobreposicao ganha um mapa por (indice, chave).
//!
//! Mede SO o `COMMIT`: o empilhar das n escritas fica fora do relogio, porque
//! o que o pedido muda e o que acontece entre o `preparar_a_marca` e a passada.
//! Duas arrumacoes da lista, porque a sobreposicao linear doi diferente em
//! cada uma -- e a bancada compara trabalho igual, nao so pergunta igual:
//!
//! * **intercalada** `[mae 1, filha->1, mae 2, filha->2, ...]`: a filha i ve
//!   i maes nascidas antes dela;
//! * **em blocos** `[mae 1..n/2, filha->1 .. filha->n/2]`: toda filha ve as
//!   n/2 maes -- o pior caso da busca linear.
//!
//! Cada cenario roda `rodadas` vezes num servidor novo, e sai mediana e a
//! faixa min-max: vencedor so quando as faixas nao se cruzam.

use std::sync::Arc;
use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_server::mcp::Executor as _;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Servidor};

fn pedido(txt: &str) -> Json {
    let j = Json::analisar(txt).unwrap();
    let Json::Objeto(mut pares) = j else {
        panic!("o pedido tem de ser um objeto: {txt}");
    };
    pares.push(("token".to_string(), Json::texto_de("t")));
    Json::Objeto(pares)
}

fn servidor(rotulo: &str) -> (Arc<Servidor>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "phx-pre-conferencia-{}-{rotulo}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let c = Config {
        base: dir.clone(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    (Servidor::novo(c).unwrap(), dir)
}

fn montar(e: &ExecutorLocal) {
    e.executar(&pedido(r#"{"op":"criar_database","database":"loja"}"#))
        .unwrap();
    e.executar(&pedido(
        r#"{"op":"criar_tabela","database":"loja","tabela":"clientes",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"nome","tipo":"Str(20)"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true}]}"#,
    ))
    .unwrap();
    e.executar(&pedido(
        r#"{"op":"criar_tabela","database":"loja","tabela":"pedidos",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"cliente_id","tipo":"Int8"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                       {"nome":"por_cliente","colunas":["cliente_id"]}],
            "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cliente_id"],
                                    "tabela_ref":"clientes","colunas_ref":["id"],
                                    "verificar":true}]}"#,
    ))
    .unwrap();
}

fn mae(i: u64) -> Json {
    pedido(&format!(
        r#"{{"op":"inserir","database":"loja","tabela":"clientes","linha":{{"id":{i},"nome":"c"}}}}"#
    ))
}

fn filha(i: u64) -> Json {
    pedido(&format!(
        r#"{{"op":"inserir","database":"loja","tabela":"pedidos","linha":{{"id":{i},"cliente_id":{i}}}}}"#
    ))
}

/// Um COMMIT de `n` escritas (n/2 maes e n/2 filhas). Devolve os ms do
/// COMMIT sozinho.
fn um_commit(n: u64, em_blocos: bool) -> f64 {
    let (s, dir) = servidor(if em_blocos { "blocos" } else { "intercalada" });
    let e = ExecutorLocal::com_ligacao(Arc::clone(&s), "medidor", 1);
    montar(&e);
    let pares = n / 2;
    e.executar(&pedido(r#"{"op":"begin"}"#)).unwrap();
    if em_blocos {
        for i in 1..=pares {
            e.executar(&mae(i)).unwrap();
        }
        for i in 1..=pares {
            e.executar(&filha(i)).unwrap();
        }
    } else {
        for i in 1..=pares {
            e.executar(&mae(i)).unwrap();
            e.executar(&filha(i)).unwrap();
        }
    }
    let comeco = Instant::now();
    let r = e.executar(&pedido(r#"{"op":"commit"}"#)).unwrap();
    let ms = comeco.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        r.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{}",
        r.escrever()
    );
    assert_eq!(r.inteiro_ou("gravadas", 0) as u64, pares * 2);
    drop(e);
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
    ms
}

fn faixa(mut v: Vec<f64>) -> (f64, f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2], v[v.len() - 1])
}

/// O cenario da revisao do DBA (achado A2): `n` filhas na lista e DEPOIS `n`
/// alteracoes de chave de mae, cada mae com uma filha no disco. Cada alteracao
/// replaneja a cascata abrindo a tabela das filhas -- e o que ela custa por
/// alteracao nao pode crescer com o tamanho da lista.
fn chaves_depois_das_filhas(n: u64) -> f64 {
    let (s, dir) = servidor("chaves");
    let e = ExecutorLocal::com_ligacao(Arc::clone(&s), "medidor", 1);
    e.executar(&pedido(r#"{"op":"criar_database","database":"loja"}"#))
        .unwrap();
    e.executar(&pedido(
        r#"{"op":"criar_tabela","database":"loja","tabela":"clientes",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"codigo","tipo":"Int8"},
                       {"nome":"nome","tipo":"Str(20)"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                       {"nome":"por_codigo","colunas":["codigo"],"unico":true}]}"#,
    ))
    .unwrap();
    e.executar(&pedido(
        r#"{"op":"criar_tabela","database":"loja","tabela":"pedidos",
            "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                       {"nome":"cod_cliente","tipo":"Int8"}],
            "indices":[{"nome":"pk","colunas":["id"],"unico":true,"primario":true},
                       {"nome":"por_cliente","colunas":["cod_cliente"]}],
            "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cod_cliente"],
                                    "tabela_ref":"clientes","colunas_ref":["codigo"]}]}"#,
    ))
    .unwrap();
    let lote = |tabela: &str, itens: Vec<String>| {
        pedido(&format!(
            r#"{{"op":"inserir_lote","database":"loja","tabela":"{tabela}","linhas":[{}]}}"#,
            itens.join(",")
        ))
    };
    e.executar(&lote(
        "clientes",
        (0..=n)
            .map(|i| format!(r#"{{"id":{i},"codigo":{i},"nome":"m"}}"#))
            .collect(),
    ))
    .unwrap();
    e.executar(&lote(
        "pedidos",
        (1..=n)
            .map(|i| format!(r#"{{"id":{},"cod_cliente":{i}}}"#, i + 10_000_000))
            .collect(),
    ))
    .unwrap();
    e.executar(&pedido(r#"{"op":"begin"}"#)).unwrap();
    for i in 1..=n {
        e.executar(&pedido(&format!(
            r#"{{"op":"inserir","database":"loja","tabela":"pedidos","linha":{{"id":{i},"cod_cliente":0}}}}"#
        )))
        .unwrap();
    }
    for i in 1..=n {
        e.executar(&pedido(&format!(
            r#"{{"op":"atualizar","database":"loja","tabela":"clientes","rowid":{},"linha":{{"id":{i},"codigo":{},"nome":"m"}}}}"#,
            i + 1,
            i + 1_000_000
        )))
        .unwrap();
    }
    let comeco = Instant::now();
    let r = e.executar(&pedido(r#"{"op":"commit"}"#)).unwrap();
    let ms = comeco.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        r.texto_ou("transaction_state", ""),
        "COMMITTED",
        "{}",
        r.escrever()
    );
    drop(e);
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
    ms
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("chaves") {
        let rodadas: usize = std::env::args()
            .nth(2)
            .and_then(|a| a.parse().ok())
            .unwrap_or(3);
        println!(
            "COMMIT com n filhas na lista e DEPOIS n alteracoes de chave de mae (achado A2), \
             {rodadas} rodadas"
        );
        for n in [2_000u64, 4_000, 8_000] {
            let v: Vec<f64> = (0..rodadas).map(|_| chaves_depois_das_filhas(n)).collect();
            let (min, med, max) = faixa(v);
            println!(
                "  n={n:<6} mediana {med:10.1} ms   faixa {min:10.1} - {max:10.1} ms   \
                 ({:.2} us/alteracao)",
                med * 1000.0 / n as f64
            );
        }
        return;
    }
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000);
    let rodadas: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(3);
    println!("COMMIT de {n} escritas (n/2 maes + n/2 filhas), {rodadas} rodadas intercaladas");
    let (mut inter, mut blocos) = (Vec::new(), Vec::new());
    for _ in 0..rodadas {
        inter.push(um_commit(n, false));
        blocos.push(um_commit(n, true));
    }
    for (nome, v) in [("intercalada", inter), ("em blocos", blocos)] {
        let (min, med, max) = faixa(v);
        println!(
            "  {nome:<12} mediana {med:10.1} ms   faixa {min:10.1} - {max:10.1} ms   ({:.2} us/escrita)",
            med * 1000.0 / n as f64
        );
    }
}
