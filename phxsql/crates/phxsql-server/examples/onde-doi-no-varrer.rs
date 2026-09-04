//! Numa consulta filtrada, quanto e VARREDURA e quanto e TRANSPORTE?
//!
//! ```bash
//! cargo run --release -p phxsql-server --example onde-doi-no-varrer
//! ```
//!
//! # A premissa que este medidor existe para matar ou confirmar
//!
//! A grade da tela filtra o que esta NELA: pede `varrer max=2500`, recebe
//! 2.500 linhas e joga fora 2.475 no navegador. A proposta e por o predicado
//! no servidor. Antes de escrever uma linha dela, a lei desta casa manda
//! medir a premissa -- porque um `WHERE` no servidor **nao remove a
//! varredura**: sem indice, o motor le as mesmas 2.500 linhas do `.reg` para
//! decidir quais 25 passam. O que ele remove e so o que vem DEPOIS da
//! leitura: montar o JSON, serializar, mandar pelo fio e o cliente analisar.
//!
//! Entao o teto do ganho e a fatia de transporte. Se a varredura for 80% do
//! tempo, o `WHERE` compra 1,25x e a sprint muda de forma.
//!
//! # As quatro camadas, medidas na mesma tabela quente
//!
//! | camada | o que entra |
//! |--------|-------------|
//! | A `varredura`  | `pagina_por_posicao` + `ler` de cada linha. Sem JSON. |
//! | B `+ json`     | A + `linha_para_json` de cada linha (o `varrer` em processo). |
//! | C `+ texto`    | B + `Json::escrever()` da resposta inteira. |
//! | D `+ fio`      | C + escrever no soquete + o cliente ler e analisar. |
//!
//! A e o CHAO que o `WHERE` nao remove. D e o que a tela pagava. E a quinta
//! medida, `E`, e o que ela paga DEPOIS: o mesmo pedido pelo fio, com o
//! `"onde"` junto. O teto previsto e A + transporte das que casam; `E` diz o
//! que o teto virou de verdade.
//!
//! As rodadas sao INTERCALADAS (A,B,C,D, A,B,C,D, ...) e sai a mediana: medir
//! as quatro em sequencia faria a primeira pagar a arvore fria das outras.
//!
//! Argumentos: `<linhas> <teto> <rodadas> <um_em>` (padrao 100000, 2500, 7,
//! 100). O `um_em` e a SELETIVIDADE: `100` e uma linha em cem de Blumenau (o
//! filtro seletivo da tela), `2` e metade da tabela (o pouco seletivo). Rodar
//! os dois e o que separa «o WHERE compra 2x» de «o WHERE compra sempre».

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::mcp::Executor as _;
use phxsql_server::servidor::ExecutorLocal;
use phxsql_server::{Config, Servidor};
use phxsql_store::table::Visao;
use phxsql_store::Instancia;

/// Uma em cem e de Blumenau: e a seletividade que a tela sofre.
const CIDADES: [&str; 10] = [
    "Blumenau",
    "Itajai",
    "Joinville",
    "Curitiba",
    "Florianopolis",
    "Chapeco",
    "Lages",
    "Criciuma",
    "Brusque",
    "Tubarao",
];

fn linha(i: i64, um_em: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:07}")),
        // Uma em `um_em` e de Blumenau. O resto se espalha pelas outras nove.
        Value::Str(
            if i % um_em == 0 {
                CIDADES[0]
            } else {
                CIDADES[1 + (i as usize % 9)]
            }
            .into(),
        ),
        Value::Memo(format!(
            "ficha do cliente {i}, com o texto que mora no .memo e obriga a \
             segunda leitura de arquivo por linha"
        )),
    ]
}

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(30)),
            Column::new("ficha", ColumnType::Memo),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap()
}

fn mediana(v: &mut [u128]) -> u128 {
    v.sort_unstable();
    v[v.len() / 2]
}

/// Uma porta que esta livre agora. Ha corrida entre soltar e prender, e ela e
/// aceitavel num medidor: se der ocupada, o proprio `escutar` reclama.
fn porta_livre() -> u16 {
    let o = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = o.local_addr().unwrap().port();
    drop(o);
    p
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: i64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(100_000);
    let teto: u64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(2500);
    let rodadas: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(7);
    let um_em: i64 = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(100);

    let base = std::env::temp_dir().join(format!(
        "phx-onde-doi-varrer-{}-{um_em}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    // A tabela nasce pelo motor, e nao pelo servidor: 100.000 `inserir` pelo
    // protocolo mediriam o protocolo, e o que se quer medir e a leitura.
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let carga = Instant::now();
    {
        let mut t = db.criar_tabela(None, esquema()).unwrap();
        for i in 1..=n {
            t.inserir(&linha(i, um_em)).unwrap();
        }
    }
    eprintln!("carga: {n} linhas em {} ms", carga.elapsed().as_millis());

    // O servidor, com tudo o que nao e a porta de dados DESLIGADO -- a web e o
    // REST prenderiam portas que este medidor nao usa.
    let porta = porta_livre();
    let mut c = Config {
        base: base.clone(),
        bind: format!("127.0.0.1:{porta}"),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        token: "medidor".into(),
        max_linhas: 1_000_000,
        ..Config::default()
    };
    c.web.ligado = false;
    c.rest.ligado = false;
    let servidor = Servidor::novo(c).unwrap();
    let local = ExecutorLocal::novo(Arc::clone(&servidor), "medidor");
    {
        let s = Arc::clone(&servidor);
        std::thread::spawn(move || {
            let _ = s.escutar();
        });
    }

    // Espera a porta abrir em vez de dormir um tempo escolhido no olho.
    let alvo = format!("127.0.0.1:{porta}");
    let mut fluxo = None;
    for _ in 0..200 {
        if let Ok(f) = TcpStream::connect(&alvo) {
            fluxo = Some(f);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let fluxo = fluxo.expect("o servidor nao abriu a porta de dados");
    fluxo.set_nodelay(true).unwrap();
    let mut leitor = BufReader::new(fluxo.try_clone().unwrap());
    let mut escrita = fluxo;

    let pedido_socket = format!(
        r#"{{"op":"varrer","token":"medidor","database":"loja","tabela":"clientes","max":{teto}}}"#
    );
    let pedido_json = Json::analisar(&pedido_socket).unwrap();
    // O MESMO pedido com o predicado junto -- o que a tela manda hoje.
    let pedido_onde = format!(
        r#"{{"op":"varrer","token":"medidor","database":"loja","tabela":"clientes","max":{teto},"onde":[{{"coluna":"cidade","op":"=","valor":"Blumenau"}}]}}"#
    );

    let mut a_us: Vec<u128> = Vec::new();
    let mut b_us: Vec<u128> = Vec::new();
    let mut c_us: Vec<u128> = Vec::new();
    let mut d_us: Vec<u128> = Vec::new();
    let mut e_us: Vec<u128> = Vec::new();
    let mut bytes = 0usize;
    let mut bytes_com_onde = 0usize;
    let mut casaram = 0usize;

    // Uma rodada a mais no comeco, jogada fora: e a que paga a arvore fria.
    for r in 0..=rodadas {
        // A -- so a varredura: a pagina e a leitura de cada linha.
        let t0 = Instant::now();
        let mut t = db.abrir_qualificada("clientes").unwrap();
        let (rowids, _) = t.pagina_por_posicao(0, teto, Visao::Ativas).unwrap();
        let mut lidas = Vec::with_capacity(rowids.len());
        for &rowid in &rowids {
            if let Some(l) = t.ler(rowid).unwrap() {
                lidas.push((rowid, l));
            }
        }
        let ta = t0.elapsed().as_micros();

        // Quantas linhas da pagina passariam pelo filtro. Conta uma vez, fora
        // do relogio de A: o que se quer de A e o chao da leitura.
        if r == 0 {
            let pos = t
                .esquema()
                .colunas()
                .iter()
                .position(|c| c.nome == "cidade")
                .unwrap();
            casaram = lidas
                .iter()
                .filter(|(_, l)| matches!(&l[pos], Value::Str(s) if s == "Blumenau"))
                .count();
        }

        // B -- A + montar o JSON de cada linha (o que o `varrer` faz hoje).
        let t0 = Instant::now();
        let resposta = local.executar(&pedido_json).unwrap();
        let tb = t0.elapsed().as_micros();

        // C -- B + serializar a resposta inteira em texto.
        let t0 = Instant::now();
        let texto = resposta.escrever();
        let tc = tb + t0.elapsed().as_micros();
        bytes = texto.len();

        // D -- tudo, pelo fio: o cliente manda, o servidor responde, o cliente
        // le a linha e ANALISA -- que e o que a tela paga de verdade.
        let t0 = Instant::now();
        writeln!(escrita, "{pedido_socket}").unwrap();
        escrita.flush().unwrap();
        let mut volta = String::new();
        leitor.read_line(&mut volta).unwrap();
        let vindo = Json::analisar(volta.trim()).unwrap();
        let td = t0.elapsed().as_micros();
        assert_eq!(
            vindo
                .campo("resultado")
                .and_then(|r| r.campo("linhas"))
                .and_then(Json::lista)
                .map(|l| l.len())
                .unwrap_or(0),
            teto as usize,
            "a resposta do fio nao trouxe a pagina inteira"
        );

        // E -- tudo, pelo fio, COM o predicado. E o mesmo caminho de D, com a
        // peneira do lado de la: a varredura fica inteira e o transporte
        // encolhe para as que casam.
        let t0 = Instant::now();
        writeln!(escrita, "{pedido_onde}").unwrap();
        escrita.flush().unwrap();
        let mut volta = String::new();
        leitor.read_line(&mut volta).unwrap();
        bytes_com_onde = volta.trim().len();
        let vindo = Json::analisar(volta.trim()).unwrap();
        let te = t0.elapsed().as_micros();
        let res = vindo.campo("resultado").unwrap();
        assert_eq!(
            res.inteiro_ou("examinadas", -1),
            teto as i64,
            "o filtro mudou o tamanho da varredura"
        );
        assert_eq!(res.inteiro_ou("devolvidas", -1), casaram.max(1) as i64);

        if r > 0 {
            e_us.push(te);
            a_us.push(ta);
            b_us.push(tb);
            c_us.push(tc);
            d_us.push(td);
        }
    }

    let a = mediana(&mut a_us) as f64;
    let b = mediana(&mut b_us) as f64;
    let c = mediana(&mut c_us) as f64;
    let d = mediana(&mut d_us) as f64;
    let e = mediana(&mut e_us) as f64;

    println!("\n=== varrer max={teto} numa tabela de {n} linhas, 1 em {um_em} casando ===");
    println!(
        "resposta no fio: {} bytes | {casaram} das {teto} linhas sao de Blumenau",
        bytes
    );
    println!();
    println!(
        "  A varredura (pagina + ler)      {a:10.0} us   {:5.1}%",
        100.0 * a / d
    );
    println!(
        "  B + montar o JSON              +{:10.0} us   {:5.1}%",
        b - a,
        100.0 * (b - a) / d
    );
    println!(
        "  C + serializar em texto        +{:10.0} us   {:5.1}%",
        c - b,
        100.0 * (c - b) / d
    );
    println!(
        "  D + fio e analise no cliente   +{:10.0} us   {:5.1}%",
        d - c,
        100.0 * (d - c) / d
    );
    println!("  ---------------------------------------------------");
    println!("  TOTAL de hoje (D)               {d:10.0} us   100.0%");
    println!();
    println!(
        "  VARREDURA .............. {:5.1}%   (o WHERE nao remove)",
        100.0 * a / d
    );
    println!(
        "  TRANSPORTE ............. {:5.1}%   (o WHERE remove quase tudo)",
        100.0 * (d - a) / d
    );

    // O teto do ganho: a varredura fica inteira, e o transporte encolhe na
    // proporcao das linhas que sobram. Nao e uma promessa -- e o limite.
    let fracao = casaram as f64 / teto as f64;
    let previsto = a + (d - a) * fracao;
    println!();
    println!(
        "  teto previsto:  {d:.0} -> ~{previsto:.0} us = {:.2}x",
        d / previsto
    );
    println!(
        "  MEDIDO (E):     {d:.0} -> {e:.0} us = {:.2}x   | {} -> {} bytes no fio",
        d / e,
        bytes,
        bytes_com_onde
    );

    let _ = std::fs::remove_dir_all(&base);
}
