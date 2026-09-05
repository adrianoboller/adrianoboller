//! O que a grade ORDENADA custa para quem esta olhando a tela.
//!
//! ```bash
//! cargo run --release -p phxsql-server --example o-que-a-grade-ordenada-custa -- [linhas] [pagina] [rodadas]
//! ```
//!
//! # A premissa que este medidor existe para matar ou confirmar
//!
//! O pedido 188 tem o custo do MOTOR medido: uma grade ordenada toca 1.668
//! paginas do `.ndx` (6,52 MiB, 81% do teto do cache) onde a grade sem ordem
//! toca zero, porque `Table::pagina_por_indice` chama `varrer_indice` e so
//! depois recorta -- entao 50 linhas custam o mesmo que 1.000.
//!
//! **Numero de motor nao decide se ha frente.** Decide se o custo APARECE para
//! quem esta olhando a tela. Se nao aparecer, o pedido vira documentacao; se
//! aparecer, vira conserto. Este medidor responde essa pergunta, e responde em
//! VARIAS ESCALAS -- porque o custo e o do indice inteiro, entao ele cresce com
//! a TABELA e nao com a pagina.
//!
//! # As tres perguntas, medidas pelo fio e por dentro
//!
//! | rotulo | o que a tela faz |
//! |---|---|
//! | `sem ordem` | abrir a grade: `varrer max=<pagina>`, ordem de digitacao |
//! | `ORDENADA`  | clicar no cabecalho da coluna: `varrer max=<pagina> indice=porId` |
//! | `por chave` | digitar no campo de busca: `buscar` |
//!
//! Cada uma sai em duas camadas, e a diferenca entre elas e o TRANSPORTE:
//!
//! - `motor` -- a chamada dentro do processo, sem JSON e sem soquete;
//! - `fio` -- o pedido inteiro pelo soquete, com o cliente analisando a
//!   resposta, que e o que o navegador paga.
//!
//! O piso do transporte ja mordeu esta casa uma vez: um eco que dividia o GIL
//! com o cliente fez um servidor «custar menos que nada». Por isso as duas
//! camadas, e por isso a linha de CONTROLE -- um `ping` pelo mesmo soquete,
//! que e o chao do fio e nao mede motor nenhum.
//!
//! # O crivo: trabalho igual, nao so pergunta igual
//!
//! Uma grade sem ordem devolve as 50 primeiras na ORDEM DE DIGITACAO; uma
//! ordenada devolve as 50 primeiras na ORDEM DA CHAVE. Sao 50 linhas dos dois
//! lados e **nao e o mesmo trabalho** -- a razao entre elas nao mede um motor
//! contra o outro, mede **o preco de pedir ordem**. Quem quer saber se ha
//! defeito compara a grade ordenada contra o MINIMO que uma grade ordenada
//! exige, e esse minimo esta na ultima coluna: `ordenada minima`, que desce a
//! arvore uma vez e le so as `<pagina>` primeiras entradas da folha.
//!
//! # As rodadas sao intercaladas
//!
//! A,B,C, A,B,C, ... e sai a MEDIANA. Medir as tres em sequencia faria a
//! primeira pagar a arvore fria das outras duas. E a rodada 0 e jogada fora.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::servidor::Servidor;
use phxsql_server::Config;
use phxsql_store::ndx::{contadores_de_cache, PAGINA_PADRAO};
use phxsql_store::table::Visao;
use phxsql_store::Instancia;

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cliente {i:08}")),
        Value::Str(format!("Cidade {}", i % 500)),
    ]
}

fn mediana(v: &mut [u128]) -> u128 {
    v.sort_unstable();
    v[v.len() / 2]
}

/// Uma porta livre agora. Ha corrida entre soltar e prender, e ela e aceitavel
/// num medidor: se der ocupada, o proprio `escutar` reclama.
fn porta_livre() -> u16 {
    let o = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let p = o.local_addr().unwrap().port();
    drop(o);
    p
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n: i64 = a.get(1).map(|s| s.parse().unwrap()).unwrap_or(100_000);
    let pagina: u64 = a.get(2).map(|s| s.parse().unwrap()).unwrap_or(50);
    let rodadas: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(9);

    let base = std::env::temp_dir().join(format!("phx-grade-ordenada-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();

    // A tabela nasce pelo motor: semear pelo protocolo mediria o protocolo.
    let inst = Instancia::nova(&base).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let carga = Instant::now();
    {
        let mut t = db.criar_tabela(None, esquema()).unwrap();
        for i in 1..=n {
            t.inserir(&linha(i)).unwrap();
        }
    }
    eprintln!(
        "carga: {n} linhas em {:.1} s",
        carga.elapsed().as_secs_f64()
    );

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
    {
        let s = Arc::clone(&servidor);
        std::thread::spawn(move || {
            let _ = s.escutar();
        });
    }

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

    let cab = r#""token":"medidor","database":"loja","tabela":"clientes""#;
    let p_sem = format!(r#"{{"op":"varrer",{cab},"max":{pagina}}}"#);
    let p_ord = format!(r#"{{"op":"varrer",{cab},"max":{pagina},"indice":"porId"}}"#);
    let p_chave = format!(
        r#"{{"op":"buscar",{cab},"indice":"porId","chave":[{}]}}"#,
        n / 2
    );
    let p_ping = r#"{"op":"ping","token":"medidor"}"#.to_string();

    // Um pedido pelo fio, com a resposta ANALISADA -- que e o que o navegador
    // paga. Devolve os microssegundos e quantas linhas vieram.
    let mut pelo_fio = |pedido: &str| -> (u128, usize) {
        let t0 = Instant::now();
        writeln!(escrita, "{pedido}").unwrap();
        escrita.flush().unwrap();
        let mut volta = String::new();
        leitor.read_line(&mut volta).unwrap();
        let vindo = Json::analisar(volta.trim()).unwrap();
        let us = t0.elapsed().as_micros();
        assert!(
            vindo.campo("erro").is_none(),
            "o servidor recusou o pedido: {volta}"
        );
        let linhas = vindo
            .campo("resultado")
            .and_then(|r| r.campo("linhas").or(Some(r)))
            .and_then(Json::lista)
            .map(|l| l.len())
            .unwrap_or(0);
        (us, linhas)
    };

    let mut fio_sem = Vec::new();
    let mut fio_ord = Vec::new();
    let mut fio_chave = Vec::new();
    let mut fio_ping = Vec::new();
    let mut mot_sem = Vec::new();
    let mut mot_ord = Vec::new();
    let mut mot_min = Vec::new();
    let mut rowids_ord = 0usize;
    let mut linhas_sem = 0usize;
    let mut linhas_ord = 0usize;

    for r in 0..=rodadas {
        // -- pelo fio, intercalado
        let (a, ls) = pelo_fio(&p_sem);
        let (b, lo) = pelo_fio(&p_ord);
        let (cc, _) = pelo_fio(&p_chave);
        let (d, _) = pelo_fio(&p_ping);

        // -- por dentro, o mesmo trabalho sem JSON e sem soquete
        let mut t = db.abrir_qualificada("clientes").unwrap();
        let t0 = Instant::now();
        let (ids, _) = t.pagina_por_posicao(0, pagina, Visao::Ativas).unwrap();
        for &id in &ids {
            std::hint::black_box(t.ler(id).unwrap());
        }
        let e = t0.elapsed().as_micros();

        let t0 = Instant::now();
        let ids = t
            .pagina_por_indice("porId", Visao::Ativas, 0, pagina)
            .unwrap();
        for &id in &ids {
            std::hint::black_box(t.ler(id).unwrap());
        }
        let f = t0.elapsed().as_micros();

        // O MINIMO que uma grade ordenada exige: descer a arvore uma vez e ler
        // as `pagina` primeiras entradas da ordem. `intervalo` com `ate` na
        // primeira chave depois da pagina para na folha, e nao no fim do
        // indice -- e devolve exatamente as mesmas linhas, na mesma ordem.
        let corte = vec![Value::Int(pagina as i64 + 1)];
        let t0 = Instant::now();
        let ids = t.intervalo("porId", None, Some(&corte)).unwrap();
        let ids: Vec<_> = ids.into_iter().take(pagina as usize).collect();
        for &id in &ids {
            std::hint::black_box(t.ler(id).unwrap());
        }
        let g = t0.elapsed().as_micros();

        if r == 0 {
            // Fora do relogio: quantos rowids o `Vec` intermediario carregou.
            rowids_ord = t.varrer_indice("porId").unwrap().len();
            linhas_sem = ls;
            linhas_ord = lo;
            continue; // a rodada 0 paga a arvore fria; ela nao entra
        }
        fio_sem.push(a);
        fio_ord.push(b);
        fio_chave.push(cc);
        fio_ping.push(d);
        mot_sem.push(e);
        mot_ord.push(f);
        mot_min.push(g);
    }

    // As paginas do `.ndx` que cada caminho toca: o contador global e
    // alimentado no `Drop` do arquivo, entao cada medicao abre, le e FECHA.
    let paginas = |f: &mut dyn FnMut(&mut phxsql_store::table::Table)| -> u64 {
        let (_, f0, _) = contadores_de_cache();
        {
            let mut t = db.abrir_qualificada("clientes").unwrap();
            f(&mut t);
        }
        let (_, f1, _) = contadores_de_cache();
        f1 - f0
    };
    let pag_sem = paginas(&mut |t| {
        let (ids, _) = t.pagina_por_posicao(0, pagina, Visao::Ativas).unwrap();
        for &id in &ids {
            std::hint::black_box(t.ler(id).unwrap());
        }
    });
    let pag_ord = paginas(&mut |t| {
        let ids = t
            .pagina_por_indice("porId", Visao::Ativas, 0, pagina)
            .unwrap();
        for &id in &ids {
            std::hint::black_box(t.ler(id).unwrap());
        }
    });
    let pag_min = paginas(&mut |t| {
        let corte = vec![Value::Int(pagina as i64 + 1)];
        let ids = t.intervalo("porId", None, Some(&corte)).unwrap();
        for &id in ids.iter().take(pagina as usize) {
            std::hint::black_box(t.ler(id).unwrap());
        }
    });

    let ms = |v: &mut Vec<u128>| mediana(v) as f64 / 1000.0;
    let (f_sem, f_ord, f_chave, f_ping) = (
        ms(&mut fio_sem),
        ms(&mut fio_ord),
        ms(&mut fio_chave),
        ms(&mut fio_ping),
    );
    let (m_sem, m_ord, m_min) = (ms(&mut mot_sem), ms(&mut mot_ord), ms(&mut mot_min));

    println!("\n=== o que a grade ORDENADA custa na tela ===");
    println!("    {n} linhas | pagina de {pagina} | mediana de {rodadas} rodadas intercaladas\n");
    println!(
        "  {:>22}  {:>10}  {:>10}  {:>12}  {:>10}",
        "o que a tela pede", "motor ms", "fio ms", "paginas .ndx", "linhas"
    );
    println!("  {}", "-".repeat(72));
    println!(
        "  {:>22}  {m_sem:>10.2}  {f_sem:>10.2}  {pag_sem:>12}  {linhas_sem:>10}",
        "grade sem ordem"
    );
    println!(
        "  {:>22}  {m_ord:>10.2}  {f_ord:>10.2}  {pag_ord:>12}  {linhas_ord:>10}",
        "grade ORDENADA"
    );
    println!(
        "  {:>22}  {m_min:>10.2}  {:>10}  {pag_min:>12}  {:>10}",
        "ordenada MINIMA", "-", pagina
    );
    println!(
        "  {:>22}  {:>10}  {f_chave:>10.2}  {:>12}  {:>10}",
        "busca por chave", "-", "-", 1
    );
    println!(
        "  {:>22}  {:>10}  {f_ping:>10.2}  {:>12}  {:>10}",
        "CONTROLE (ping)", "-", 0, 0
    );

    let mib = rowids_ord as f64 * 8.0 / 1_048_576.0;
    println!("\n=== o que estes numeros medem ===\n");
    // `varrer_indice` continua sendo a varredura SEM teto -- ela e o caminho
    // que o `pagina_por_indice` usava, e esta aqui como REGUA: o `Vec` que ela
    // devolve e o que a grade ordenada carregava por leitura e por leitor.
    println!("  A varredura do indice inteiro (`varrer_indice`, o caminho de ANTES do");
    println!("  pedido 188) carrega {rowids_ord} rowids = {mib:.2} MiB de alocacao, POR");
    println!("  LEITURA e POR LEITOR, para devolver {pagina}. O `pagina_por_indice` de hoje");
    println!("  carrega o pedaco que a pagina precisa, e o numero disso e a coluna");
    println!("  `paginas .ndx` da tabela acima.");
    println!(
        "  Ela tocou {pag_ord} paginas do `.ndx` = {:.2} MiB, contra {pag_min} da minima.\n",
        pag_ord as f64 * PAGINA_PADRAO as f64 / 1_048_576.0
    );
    println!("  O que o usuario SENTE, descontado o chao do fio ({f_ping:.2} ms de ping):");
    println!(
        "    sem ordem {:.2} ms  ->  ORDENADA {:.2} ms   =  {:.1}x mais lenta",
        f_sem - f_ping,
        f_ord - f_ping,
        (f_ord - f_ping) / (f_sem - f_ping).max(0.001)
    );
    println!(
        "    e contra o MINIMO que a mesma pergunta exige: {:.2} ms -> {:.2} ms = {:.1}x",
        m_min,
        m_ord,
        m_ord / m_min.max(0.001)
    );
    println!(
        "\n  CRIVO: `sem ordem` e `ORDENADA` devolvem {pagina} linhas cada uma, e NAO e o\n  \
         mesmo trabalho -- uma na ordem de digitacao, a outra na ordem da chave. A\n  \
         razao entre as duas e o PRECO DE PEDIR ORDEM. A comparacao de trabalho\n  \
         igual e a ultima: `ORDENADA` contra `ordenada MINIMA`, mesmas linhas e\n  \
         mesma ordem."
    );

    let _ = std::fs::remove_dir_all(&base);
}
