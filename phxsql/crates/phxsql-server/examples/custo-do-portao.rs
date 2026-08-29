//! Quanto custa o portao dos gatilhos no caminho de escrita?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example custo-do-portao
//! ```
//!
//! Tres cenarios, cada um num servidor LIMPO (a primeira versao media os
//! tres em sequencia na mesma tabela, e o primeiro pagava a arvore fria dos
//! outros — o artefato de ordem aparecia como custo do portao):
//!
//! 1. **sem gatilho nenhum** — o portao e um load atomico que da falso;
//! 2. **gatilho em OUTRA tabela** — o atomico da verdadeiro, e cada escrita
//!    paga a trava do registro e a procura (que nao acha nada);
//! 3. **gatilho BEFORE na propria tabela** — o custo cheio: linha vira JSON,
//!    o corpo roda, a coluna tocada volta convertida.
//!
//! Roda as rodadas INTERCALADAS (1,2,3, 1,2,3, …) e mostra a mediana, a
//! faixa e o espalhamento de cada cenario. A promessa que ele confere e a do
//! CLAUDE.md: instrumentacao desligada custa zero.
//!
//! E ele termina comparando a diferenca entre cenarios com o espalhamento
//! DENTRO de um cenario: enquanto a primeira couber na segunda, a conclusao
//! honesta e «o portao nao aparece», e nao um numero. O resultado esta em
//! `docs/TRIGGERS.md` — medido, nunca estimado.
//!
//! Argumentos: `<linhas por rodada> <rodadas>` (padrao 20000 e 5).

use std::time::Instant;

use phxsql_core::json::Json;
use phxsql_server::mcp::Executor as _;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Servidor};

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Sobe um servidor limpo, cria as duas tabelas e talvez um gatilho.
fn cenario(gatilho_em: Option<&str>) -> (ExecutorLocal, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "phx-portao-{}-{:?}",
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
    let s = ExecutorLocal::novo(Servidor::novo(c).unwrap(), "medidor");
    s.executar(&pedido(
        r#"{"token":"t","op":"criar_database","database":"m"}"#,
    ))
    .unwrap();
    for tabela in ["alvo", "outra"] {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"criar_tabela","database":"m","tabela":"{tabela}",
                "colunas":[{{"nome":"id","tipo":"Int8","obrigatoria":true}},
                           {{"nome":"nome","tipo":"Str(20)"}},
                           {{"nome":"cidade","tipo":"Str(20)"}}],
                "indices":[{{"nome":"porId","colunas":["id"],"unico":true,
                             "primario":true}}]}}"#
        )))
        .unwrap();
    }
    if let Some(tabela) = gatilho_em {
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"sql","database":"m","texto":"CREATE TRIGGER g BEFORE INSERT ON {tabela} FOR EACH ROW SET NEW.cidade = UPPER(TRIM(NEW.cidade))"}}"#
        )))
        .unwrap();
    }
    (s, dir)
}

/// Insere `n` linhas na `alvo` e devolve us/linha, depois de aquecer.
fn medir(gatilho_em: Option<&str>, n: u64) -> f64 {
    let (s, dir) = cenario(gatilho_em);
    let mut id = 0i64;
    for _ in 0..500 {
        id += 1;
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"alvo","linha":{{"id":{id},"nome":"aq","cidade":"x"}}}}"#
        )))
        .unwrap();
    }
    let inicio = Instant::now();
    for _ in 0..n {
        id += 1;
        s.executar(&pedido(&format!(
            r#"{{"token":"t","op":"inserir","database":"m","tabela":"alvo","linha":{{"id":{id},"nome":"linha","cidade":"bnu"}}}}"#
        )))
        .unwrap();
    }
    let por_linha = inicio.elapsed().as_micros() as f64 / n as f64;
    let _ = std::fs::remove_dir_all(&dir);
    por_linha
}

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(20_000);
    let rodadas: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(5);
    let cenarios: [(&str, Option<&str>); 3] = [
        ("1. sem gatilho nenhum (atomico = falso)", None),
        (
            "2. gatilho em OUTRA tabela (procura, nao acha)",
            Some("outra"),
        ),
        ("3. gatilho BEFORE na propria tabela", Some("alvo")),
    ];

    // As rodadas sao INTERCALADAS -- 1,2,3, 1,2,3, ... -- e nao um cenario
    // inteiro de cada vez. Medindo em bloco, qualquer deriva da maquina (o
    // disco esquentando, outro processo entrando) fica toda dentro de um
    // cenario e vira "custo" dele. Intercalado, ela atinge os tres igual.
    let mut medidas: Vec<Vec<f64>> = vec![Vec::new(); cenarios.len()];
    for _ in 0..rodadas {
        for (i, (_, gatilho_em)) in cenarios.iter().enumerate() {
            medidas[i].push(medir(*gatilho_em, n));
        }
    }

    println!("{n} linhas por rodada, {rodadas} rodadas intercaladas\n");
    let mut medianas = Vec::new();
    for (i, (rotulo, _)) in cenarios.iter().enumerate() {
        let mut m = medidas[i].clone();
        m.sort_by(|a, b| a.total_cmp(b));
        let mediana = m[m.len() / 2];
        medianas.push(mediana);
        println!(
            "{rotulo:48} mediana {mediana:7.2} us/linha   faixa {:.2}..{:.2}  (espalhamento {:.1}%)",
            m[0],
            m[m.len() - 1],
            (m[m.len() - 1] - m[0]) / m[0] * 100.0
        );
    }

    // A conclusao honesta e uma COMPARACAO, e nao um numero: enquanto a
    // diferenca entre cenarios couber dentro do espalhamento de um cenario
    // sozinho, o que este medidor mostra e que o portao NAO aparece -- e nao
    // que ele custa exatamente tanto.
    let pior_espalhamento = medidas
        .iter()
        .map(|m| {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for v in m {
                lo = lo.min(*v);
                hi = hi.max(*v);
            }
            hi - lo
        })
        .fold(0.0f64, f64::max);
    let diferenca = medianas[1] - medianas[0];
    println!(
        "\ndiferenca 2-1 = {diferenca:+.2} us/linha; maior espalhamento dentro de \
         um cenario = {pior_espalhamento:.2} us/linha"
    );
    println!(
        "{}",
        if diferenca.abs() < pior_espalhamento {
            "=> o portao NAO aparece acima do ruido do disco neste caminho."
        } else {
            "=> a diferenca passou do ruido: ha custo mensuravel, investigue."
        }
    );
}
