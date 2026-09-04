//! Quanta RAM de cache do `.ndx` uma leitura REALMENTE ocupa.
//!
//! ```bash
//! cargo run --release --example quanto-cache-uma-leitura-usa -p phxsql-store -- [linhas]
//! ```
//!
//! ## Por que esta pergunta existe
//!
//! O cache de paginas do `.ndx` e POR `Table`, nao do processo: 2.048 paginas
//! de 4 KiB, **8 MiB de teto por tabela aberta**. E o comentario que justifica
//! esse teto, no `ndx.rs`, diz:
//!
//! > «O servidor abre e fecha a tabela a cada operacao, entao o teto vale
//! > enquanto a operacao dura.»
//!
//! Essa frase so e verdadeira porque a trava global SERIALIZA tudo. Com um
//! `RwLock`, N leitores simultaneos sao N x o que cada um ocupa -- e o teto que
//! a trava segurava de graca passa a ser um numero que alguem tem de escolher.
//!
//! **8 MiB e o TETO, nao o uso.** O cache enche por pagina TOCADA. Medir o uso
//! real decide se ha problema, e pode nao haver -- que e o resultado mais util
//! possivel, porque mata a preocupacao com numero em vez de com opiniao.
//!
//! ## Como ele mede, e por que assim
//!
//! Os contadores globais (`contadores_de_cache`) so sao alimentados no `Drop`
//! do arquivo, de proposito -- a nota no `ndx.rs` explica. Entao cada medicao
//! abre, le, FECHA, e olha o delta. `faltas` e o numero de paginas DISTINTAS
//! que entraram no cache; vezes 4 KiB da a RAM.
//!
//! Uma leitura que nao toca o indice devolve zero, e isso tambem e resposta:
//! quer dizer que aquele caminho nao paga nada de cache nenhum.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::ndx::{contadores_de_cache, PAGINA_PADRAO};

/// O teto padrao do cache, em paginas -- o mesmo `PAGINAS_PADRAO` do `ndx.rs`.
const TETO_PAGINAS: u64 = 2048;
use phxsql_store::table::{Table, Visao};

fn esquema() -> Schema {
    Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
            Column::new("cidade", ColumnType::Str(20)),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Nome {i:08}")),
        Value::Str(format!("Cidade {}", i % 500)),
    ]
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    let dir = std::env::temp_dir().join(format!("phx-cachemem-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    {
        let mut t = Table::criar(&dir, esquema()).unwrap();
        for i in 1..=n {
            t.inserir(&linha(i)).unwrap();
        }
        t.sincronizar().unwrap();
    }

    println!("=== quanta RAM de cache do `.ndx` uma leitura ocupa ===");
    println!("    {n} linhas semeadas | pagina de {PAGINA_PADRAO} bytes | teto de 2048 paginas = 8 MiB\n");

    // CONTROLE POSITIVO, e ele vem PRIMEIRO. Um medidor que responde zero em
    // tudo pode estar dizendo a verdade ou estar cego, e as duas coisas sao
    // indistinguiveis sem um caso que TEM de dar diferente de zero. A insercao
    // desce a arvore -- raiz, no interno, folha -- entao ela nao pode dar zero.
    // Foi assim que a primeira versao deste medidor foi pega: ela media so o
    // `pagina()`, que percorre o `.reg` e nunca toca o `.ndx`, e publicava
    // «zero» como se fosse resposta sobre o cache.
    let (a_semeadura, f_semeadura, _) = contadores_de_cache();
    println!("  CONTROLE POSITIVO -- a semeadura de {n} linhas:");
    println!(
        "    {a_semeadura} acertos, {f_semeadura} faltas. Se isto fosse zero, o medidor estaria cego\n"
    );
    assert!(
        f_semeadura > 0,
        "o medidor esta cego: a insercao desce a arvore e TEM de tocar pagina"
    );

    println!(
        "  {:>28}  {:>9}  {:>9}  {:>11}  {:>8}",
        "leitura", "acertos", "carregou", "residente", "do teto"
    );
    println!("  {}", "-".repeat(74));

    let mut pior = 0u64;
    let mut medir = |rotulo: &str, f: &mut dyn FnMut(&mut Table)| {
        let (a0, f0, _) = contadores_de_cache();
        {
            let mut t = Table::abrir(&dir, "clientes").unwrap();
            f(&mut t);
        } // o Drop e quem alimenta os contadores globais
        let (a1, f1, _) = contadores_de_cache();
        let (acertos, faltas) = (a1 - a0, f1 - f0);
        // `faltas` conta pagina CARREGADA, nao residente: acima do teto o
        // cache despeja e recarrega. Confundir as duas foi erro meu na
        // primeira versao, e publicaria «13 MiB de RAM» onde o cache nunca
        // passa de 8. Residente e o minimo entre as duas coisas.
        let residentes = faltas.min(TETO_PAGINAS);
        let bytes = residentes * PAGINA_PADRAO as u64;
        pior = pior.max(residentes);
        println!(
            "  {rotulo:>28}  {acertos:>9}  {faltas:>9}  {:>8.2} MiB  {:>7.1}%",
            bytes as f64 / 1_048_576.0,
            residentes as f64 / TETO_PAGINAS as f64 * 100.0
        );
    };

    // sem indice: `pagina` percorre o `.reg` com `proximo_ativo`. Zero AQUI e
    // resposta, e nao cegueira -- o controle acima ja provou que o medidor ve.
    for (rotulo, limite) in [
        ("grade sem ordem (50)", 50u64),
        ("grade sem ordem (1.000)", 1_000),
    ] {
        medir(rotulo, &mut |t: &mut Table| {
            let ids = t.pagina(0, limite, Visao::Ativas).unwrap();
            for id in ids {
                std::hint::black_box(t.ler(id).unwrap());
            }
        });
    }

    // com indice: e o que a tela faz quando alguem clica no cabecalho da coluna
    for (rotulo, limite) in [
        ("grade ORDENADA (50)", 50u64),
        ("grade ORDENADA (1.000)", 1_000),
    ] {
        medir(rotulo, &mut |t: &mut Table| {
            let ids = t
                .pagina_por_indice("porId", Visao::Ativas, 0, limite)
                .unwrap();
            for id in ids {
                std::hint::black_box(t.ler(id).unwrap());
            }
        });
    }

    medir("busca por chave", &mut |t: &mut Table| {
        std::hint::black_box(t.buscar("porId", &[Value::Int(n / 2)]).unwrap());
    });
    medir("varredura do indice inteiro", &mut |t: &mut Table| {
        std::hint::black_box(t.varrer_indice("porId").unwrap());
    });

    let _ = std::fs::remove_dir_all(&dir);

    println!("\n=== o veredito ===\n");
    let mib = pior as f64 * PAGINA_PADRAO as f64 / 1_048_576.0;
    println!(
        "  A leitura mais cara desta bancada deixou {pior} paginas residentes = {mib:.2} MiB,"
    );
    println!("  contra um teto de 8,00 MiB por tabela aberta. Acima do teto o cache");
    println!("  despeja: a RAM para de crescer e o TRABALHO continua -- releitura.\n");
    for leitores in [2usize, 4, 8, 16] {
        println!(
            "  {leitores:>2} leitores simultaneos: ate {:>6.1} MiB pelo uso medido, {:>6.1} MiB pelo teto",
            mib * leitores as f64,
            8.0 * leitores as f64
        );
    }
}
