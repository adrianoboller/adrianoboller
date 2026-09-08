//! Onde vai o tempo de uma consulta analitica de agregacao?
//!
//! ```bash
//! cargo run --release --example onde-doi-na-agregacao -p phxsql-store -- [linhas]
//! ```
//!
//! # A pergunta que decide a proposta do `.tbm`
//!
//! A proposta do "Memory Table Accelerator" assume que o custo de
//!
//! ```sql
//! SELECT Categoria, SUM(Estoque*Preco) FROM Produtos WHERE Ativo=TRUE GROUP BY Categoria
//! ```
//!
//! e "buscar paginas repetidamente no SSD". Mas o cache de paginas do `.ndx`
//! ja comprou 2,40x (docs/DESEMPENHO.md §1), e a §1 tambem mediu que a carga
//! e' de CPU e nao de disco (870 s de CPU para 884 s de relogio, 0,0 MiB
//! lidos). Entao a premissa PRECISA ser medida antes de virar plano -- e a
//! forma de um pivot no PhxSql torna a pergunta ainda mais direta:
//!
//! **o caminho da agregacao (`op_pivotar` -> `pivot::cruzar`) VARRE o `.reg`
//! pela ordem de digitacao (`proximo_ativo`) e NUNCA desce o `.ndx`.** O
//! cache de leitura que comprou o 2,40x e do `.ndx`; esta consulta nao o toca.
//!
//! # O que este medidor separa
//!
//! O caminho real do `varrer`/pivot faz, por linha: ler o slot do `.reg`
//! (`seek`+`read_exact` do slot inteiro, com o **CRC-32 do slot** junto),
//! decodificar o payload em `Vec<Value>`, e passar a linha pela agregacao. As
//! tres pecas, medidas isoladas na MESMA tabela ja aberta:
//!
//! | peca | como se isola | o que entra |
//! |---|---|---|
//! | (a) pagina + CRC | `pagina(0, n, Ativas)` | le TODO slot do `.reg` e confere o CRC, sem decodificar |
//! | (a)+(b) | `varrer()` | o mesmo, mais o `decodificar` de cada linha |
//! | (b) decode | `varrer` − `pagina` | so o custo de virar bytes em `Vec<Value>` |
//! | (c) filtro+agregacao | laco sobre linhas JA decodificadas | `WHERE Ativo`, `Estoque*Preco`, agrupar por `Categoria`, somar |
//!
//! `pagina` serve para isolar (a) porque ela percorre os MESMOS slots que o
//! `varrer` -- via `proximo_ativo` -> `RegFile::ler`, que le o slot inteiro e
//! confere o CRC -- mas NAO decodifica nenhuma linha (lido no fonte antes de
//! medir: `table.rs::pagina` so olha o byte da coluna de sistema).
//!
//! # Frio x quente, e por que o `.ndx` nao entra
//!
//! Nao ha cache de paginas do `.reg` em espaco de usuario: `Volumes::ler`
//! (`volume.rs`) faz `seek`+`read_exact` direto no descritor. Entao "quente" e'
//! o cache de paginas do NUCLEO, e "frio (PhxSql)" e' a tabela reaberta -- que
//! zera o cache do `.ndx`. Se reabrir NAO mudar o tempo do `varrer`, e' a
//! prova de que esta consulta nao usa o `.ndx`: o controle abaixo mede os
//! toques de pagina do `.ndx` e mostra que o `varrer` faz ZERO, contra um
//! `buscar` que desce a arvore.
//!
//! (Um frio de DISCO de verdade exigiria despejar o cache do nucleo -- root e
//! `drop_caches` --, que esta sessao nao tem. Com 16 GiB de RAM e um `.reg` de
//! dezenas de MiB, o arquivo fica residente; entao o frio de disco desta
//! consulta e' NAO MEDIDO por falta de privilegio, e a §1 do DESEMPENHO.md ja
//! mostrou, na carga de 10 milhoes, que o processo le 0,0 MiB: o disco nao e'
//! o caminho quente.)

use std::collections::HashMap;
use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::{Table, Visao};

/// ~12 categorias, como uma tabela de produtos de verdade.
const CATEGORIAS: u64 = 12;

fn esquema() -> Schema {
    Schema::new(
        "produtos",
        vec![
            Column::new("Id", ColumnType::Int8).obrigatoria(),
            Column::new("Categoria", ColumnType::Str(24)).obrigatoria(),
            Column::new(
                "Preco",
                ColumnType::Decimal {
                    precisao: 15,
                    escala: 2,
                },
            ),
            Column::new("Estoque", ColumnType::Int4),
            Column::new("Ativo", ColumnType::Bool),
        ],
        // So o indice da chave -- a consulta VARRE, nao usa indice. Ele existe
        // para o controle poder mostrar um caminho que TOCA o `.ndx`.
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// Gerador previsivel: metade Ativo, precos e estoques espalhados.
fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Cat {:02}", i as u64 % CATEGORIAS)),
        Value::Decimal(((i % 100_000) + 100) as i128), // escala 2: 1,00 .. 1000,99
        Value::Int(i % 500),
        Value::Bool(i % 2 == 0),
    ]
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}
fn faixa(v: &[f64]) -> (f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (s[0], s[s.len() - 1])
}

/// O laco (c): filtro + `Estoque*Preco` + agrupar por `Categoria` + somar.
/// Fiel ao `pivot::cruzar` -- chave String por linha, soma em i128 escalado
/// (Decimal soma exato, sem perder centavo). Devolve tambem um resumo para o
/// `black_box` nao deixar o compilador jogar o laco fora.
fn agregar(linhas: &[(u64, Vec<Value>)]) -> (usize, i128) {
    let mut por_categoria: HashMap<String, i128> = HashMap::new();
    for (_, l) in linhas {
        // WHERE Ativo=TRUE
        let ativo = matches!(l[4], Value::Bool(true));
        if !ativo {
            continue;
        }
        let estoque = match &l[3] {
            Value::Int(v) => *v as i128,
            _ => 0,
        };
        let preco = match &l[2] {
            Value::Decimal(d) => *d,
            _ => 0,
        };
        let categoria = match &l[1] {
            Value::Str(s) => s.clone(),
            _ => String::new(),
        };
        *por_categoria.entry(categoria).or_insert(0) += estoque * preco;
    }
    let total: i128 = por_categoria.values().sum();
    (por_categoria.len(), total)
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    let reps = 5usize;

    let dir = std::env::temp_dir().join(format!("phx-agreg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let mut t = Table::criar(&dir, esquema()).unwrap();
        for i in 1..=n {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    println!("=== onde doi na AGREGACAO, {n} linhas ===\n");
    println!("  consulta: SELECT Categoria, SUM(Estoque*Preco)");
    println!("            FROM Produtos WHERE Ativo=TRUE GROUP BY Categoria\n");
    println!("  caminho real hoje: op_pivotar -> pivot::cruzar, que VARRE o");
    println!("  `.reg` (proximo_ativo) e NUNCA desce o `.ndx`.\n");

    // ---- CONTROLE: o `varrer` toca o `.ndx`? E o `buscar`?
    // Vem primeiro. Um medidor que responde "o cache nao ajuda" pode estar
    // certo ou estar cego; o buscar TEM de tocar o `.ndx`, entao o controle
    // separa as duas coisas.
    let (varr_faltas, busca_faltas) = {
        let mut t = Table::abrir(&dir, "produtos").unwrap();
        let a = t.estatisticas_paginas();
        std::hint::black_box(t.varrer().unwrap());
        let b = t.estatisticas_paginas();
        for k in [1i64, n / 4, n / 2, (3 * n) / 4, n] {
            std::hint::black_box(t.buscar("porId", &[Value::Int(k)]).unwrap());
        }
        let c = t.estatisticas_paginas();
        // estatisticas_paginas = (acertos, faltas, gravadas) do cache do `.ndx`
        ((b.1 - a.1) + (b.0 - a.0), (c.1 - b.1) + (c.0 - b.0))
    };
    println!("  CONTROLE -- toques no cache do `.ndx` (acertos+faltas):");
    println!("    o `varrer` inteiro ....... {varr_faltas:>10}   <- a consulta analitica");
    println!("    5 `buscar` por chave ..... {busca_faltas:>10}   <- desce a arvore\n");
    assert!(
        busca_faltas > 0,
        "o medidor esta cego: buscar por chave TEM de tocar o `.ndx`"
    );

    // ---- FRIO x QUENTE do `varrer` (a+b): reabrir zera o cache do `.ndx`.
    let mut frios = Vec::new();
    for _ in 0..reps {
        let mut t = Table::abrir(&dir, "produtos").unwrap();
        let i = Instant::now();
        std::hint::black_box(t.varrer().unwrap());
        frios.push(i.elapsed().as_secs_f64() * 1e6 / n as f64);
    }

    // ---- As pecas, na MESMA tabela aberta, intercaladas, com aquecimento.
    let mut t = Table::abrir(&dir, "produtos").unwrap();
    std::hint::black_box(t.pagina(0, n as u64, Visao::Ativas).unwrap()); // aquece
    let base = std::hint::black_box(t.varrer().unwrap()); // linhas p/ o laco

    let (mut v_pag, mut v_varr, mut v_loop) = (vec![], vec![], vec![]);
    for _ in 0..reps {
        let i = Instant::now();
        let ids = t.pagina(0, n as u64, Visao::Ativas).unwrap();
        std::hint::black_box(&ids);
        v_pag.push(i.elapsed().as_secs_f64() * 1e6 / n as f64);

        let i = Instant::now();
        let linhas = t.varrer().unwrap();
        std::hint::black_box(&linhas);
        v_varr.push(i.elapsed().as_secs_f64() * 1e6 / n as f64);

        let i = Instant::now();
        std::hint::black_box(agregar(&base));
        v_loop.push(i.elapsed().as_secs_f64() * 1e6 / n as f64);
    }

    let (m_pag, m_varr, m_loop) = (
        mediana(v_pag.clone()),
        mediana(v_varr.clone()),
        mediana(v_loop.clone()),
    );
    let m_frio = mediana(frios.clone());
    let m_decode = (m_varr - m_pag).max(0.0);
    let total = m_varr + m_loop;
    let (fp0, fp1) = faixa(&v_pag);
    let (fv0, fv1) = faixa(&v_varr);
    let (fl0, fl1) = faixa(&v_loop);
    let (ff0, ff1) = faixa(&frios);

    // Estimativa do CRC: cada slot cabe em quantas paginas de 4 KiB.
    // (informativo -- o que decide e' a subtracao pagina vs decode vs laco)
    println!("  === a reparticao, us por linha (mediana de {reps}, [min-max]) ===\n");
    println!(
        "  (a) pagina + CRC do slot ...... {m_pag:>7.3}  [{fp0:.3}-{fp1:.3}]   {:>5.1}%",
        m_pag / total * 100.0
    );
    println!(
        "  (b) decode do payload ......... {m_decode:>7.3}                       {:>5.1}%",
        m_decode / total * 100.0
    );
    println!(
        "  (c) filtro + agregacao ........ {m_loop:>7.3}  [{fl0:.3}-{fl1:.3}]   {:>5.1}%",
        m_loop / total * 100.0
    );
    println!("  {}", "-".repeat(58));
    println!("  total da consulta ............. {total:>7.3}  us/linha  (a+b+c)\n");
    println!("      varrer (a+b) quente ......... {m_varr:>7.3}  [{fv0:.3}-{fv1:.3}]");
    println!(
        "      varrer (a+b) FRIO (reabre) .. {m_frio:>7.3}  [{ff0:.3}-{ff1:.3}]   {:+.1}% vs quente",
        (m_frio / m_varr - 1.0) * 100.0
    );

    println!("\n  === o veredito ===\n");
    let paginavel = m_pag / total * 100.0;
    let decode_loop = (m_decode + m_loop) / total * 100.0;
    println!("  A parte que um `.tbm` (que so evita a PAGINA) poderia comprar e' (a):");
    println!("  {paginavel:.1}% do tempo. E ela ja e' servida pelo cache do NUCLEO quando");
    println!(
        "  quente (frio vs quente = {:+.1}%), e a §1 mostrou 0,0 MiB lidos na",
        (m_frio / m_varr - 1.0) * 100.0
    );
    println!("  carga de 10 milhoes. Decode+agregacao (b+c) = {decode_loop:.1}%, e ISSO");
    println!("  so a leitura colunar/vetorizada resolve -- nao um cache que evita a");
    println!("  pagina. Numero, nao opiniao: rode de novo e confira.");

    drop(t);
    let _ = std::fs::remove_dir_all(&dir);
}
