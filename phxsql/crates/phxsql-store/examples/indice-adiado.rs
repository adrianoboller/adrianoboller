//! Adiar o `.ndx` numa carga acelera quanto, de verdade?
//!
//! ```bash
//! cargo run --release --example indice-adiado -- [linhas]
//! ```
//!
//! A ideia: durante uma carga, parar de manter os indices e reconstrui-los no
//! fim, de uma vez. Ela e boa por um motivo real -- reconstruir varrendo o
//! `.reg` uma vez custa menos que N descidas aleatorias na arvore. Este medidor
//! diz **quanto** menos, e onde a conta para de fechar.
//!
//! Tres caminhos, as mesmas linhas:
//!
//! - `hoje` -- indice unico e nao unico mantidos dentro da insercao;
//! - `adiar os dois` -- so o `.reg`, e `reindexar` no fim. E o TETO, e ele
//!   custa a garantia de unicidade: a conferencia de chave repetida acontece
//!   antes de gravar justamente porque o `.reg` nao reaproveita slot;
//! - `adiar so o nao unico` -- o caminho que NAO abre mao de nada, porque o
//!   indice unico e a propria decisao de aceitar ou recusar a linha.
//!
//! O tempo de reconstrucao e medido de verdade, com `reindexar()`, e entra na
//! conta. Adiar nao apaga trabalho -- move de lugar e faz em lote.
//!
//! # O que mudou desde a primeira corrida
//!
//! Este medidor ja disse que adiar valia **1,02x**, e a conclusao estava certa
//! para o `reindexar` daquele dia: ele inseria chave a chave, uma descida na
//! arvore por chave -- exatamente o trabalho que se queria evitar. Com a
//! construcao em lote (`NdxFile::construir_em_lote`) o `reindexar` passou a
//! encher folha por folha a partir das chaves ordenadas, e o mesmo medidor
//! passou a dizer **3,28x** para o teto e **1,59x** para o caminho que nao abre
//! mao da unicidade. Nao foi o adiamento que mudou; foi o preco do fim.

use std::time::Instant;

use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Xorshift64*, para embaralhar sem crate de fora.
struct Rng(u64);

impl Rng {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

const CIDADES: [&str; 8] = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
];

fn colunas() -> Vec<Column> {
    vec![
        Column::new("id", ColumnType::Int8).obrigatoria(),
        Column::new("produto", ColumnType::Str(40)).obrigatoria(),
        Column::new("cidade", ColumnType::Str(20)),
    ]
}

fn unico() -> IndexDef {
    IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()
}

fn nao_unico() -> IndexDef {
    IndexDef::new("porCidade", vec![IndexColumn::asc(2)])
}

fn linhas(n: i64, embaralhar: bool) -> Vec<Vec<Value>> {
    let mut ids: Vec<i64> = (1..=n).collect();
    if embaralhar {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for i in (1..ids.len()).rev() {
            let j = (rng.proximo() % (i as u64 + 1)) as usize;
            ids.swap(i, j);
        }
    }
    ids.into_iter()
        .map(|i| {
            vec![
                Value::Int(i),
                Value::Str(format!("Produto {i:08}")),
                Value::Str(CIDADES[(i as usize) % CIDADES.len()].into()),
            ]
        })
        .collect()
}

struct Medida {
    inserir: f64,
    reindexar: f64,
}

impl Medida {
    fn total(&self) -> f64 {
        self.inserir + self.reindexar
    }
}

/// Insere as linhas com os indices dados; opcionalmente reconstroi no fim.
fn medir(rotulo: &str, indices: Vec<IndexDef>, ls: &[Vec<Value>], reconstruir: bool) -> Medida {
    let dir = std::env::temp_dir().join(format!("phx-adiado-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let esquema = Schema::new("precos", colunas(), indices).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();

    let inicio = Instant::now();
    for l in ls {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    let inserir = inicio.elapsed().as_secs_f64();

    let reindexar = if reconstruir {
        let inicio = Instant::now();
        t.reindexar().unwrap();
        t.sincronizar().unwrap();
        inicio.elapsed().as_secs_f64()
    } else {
        0.0
    };

    let _ = std::fs::remove_dir_all(&dir);
    Medida { inserir, reindexar }
}

fn linha(rotulo: &str, m: &Medida, n: i64, base: f64) {
    println!(
        "  {rotulo:<28} {:>7.2}s + {:>6.2}s = {:>7.2}s  {:>9.0} linhas/s  {:>6.2}x",
        m.inserir,
        m.reindexar,
        m.total(),
        n as f64 / m.total(),
        base / m.total()
    );
}

fn rodada(nome: &str, n: i64, embaralhar: bool) -> f64 {
    let ls = linhas(n, embaralhar);
    println!("\n=== {nome} ===\n");
    println!(
        "  {:<28} {:>7} + {:>6}   {:>7}  {:>9}  {:>6}",
        "", "inserir", "reindex", "total", "linhas/s", "ganho"
    );

    let hoje = medir("hoje", vec![unico(), nao_unico()], &ls, false);
    let base = hoje.total();
    linha("hoje (os dois na hora)", &hoje, n, base);

    // So o `.reg`, sem indice nenhum: e o que a carga custaria com o `.ndx`
    // parado.
    let teto = medir("teto", vec![], &ls, false);

    // A reconstrucao e medida sobre as N linhas de verdade, e nao sobre uma
    // tabela vazia -- e ela que entra na conta do adiamento.
    let dois = medir("adiar-dois", vec![unico(), nao_unico()], &ls, true);
    let so_unico = medir("so-unico", vec![unico()], &ls, true);

    let adiar_os_dois = Medida {
        inserir: teto.inserir,
        reindexar: dois.reindexar,
    };
    linha("adiar OS DOIS", &adiar_os_dois, n, base);

    let adiar_nao_unico = Medida {
        inserir: so_unico.inserir,
        // Reconstruir so o nao unico custa menos que reconstruir os dois; a
        // diferenca entre as duas reconstrucoes e o que ele custa sozinho.
        reindexar: dois.reindexar - so_unico.reindexar,
    };
    linha("adiar so o NAO UNICO", &adiar_nao_unico, n, base);

    println!(
        "\n  reconstruir os dois indices sobre {n} linhas: {:.2}s\
         \n  reconstruir so o unico:                      {:.2}s\
         \n  ou seja, o nao unico sozinho:                {:.2}s",
        dois.reindexar,
        so_unico.reindexar,
        dois.reindexar - so_unico.reindexar
    );
    dois.reindexar
}

/// O PISO de uma reconstrucao em lote de verdade.
///
/// `reindexar` hoje insere chave a chave -- uma descida na arvore por chave, o
/// mesmo trabalho do caminho de dentro. E por isso que adiar quase nao compra.
/// Uma reconstrucao EM LOTE seria outra coisa: varrer o `.reg`, codificar as
/// chaves, ORDENAR, e encher as folhas em sequencia, montando os niveis de
/// cima por cima. Nenhuma descida.
///
/// Este medidor faz as tres primeiras partes -- varrer, codificar, ordenar --
/// e cronometra. E o piso: encher folha em sequencia custa o CRC de cada
/// pagina, e sao poucas paginas.
fn piso_do_lote(ls: &[Vec<Value>], custo_de_hoje: f64) {
    let dir = std::env::temp_dir().join(format!("phx-piso-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esquema = Schema::new("precos", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    for l in ls {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    let paginas = t.paginas_indice();

    // A chave do indice unico e a coluna 0 (Int8), codificada pelo mesmo
    // `keyenc` que a arvore usa -- a codificacao preserva ordem, entao ordenar
    // os bytes e ordenar os valores.
    let tipo = ColumnType::Int8;
    let largura = largura_componente(&tipo).unwrap();

    let inicio = Instant::now();
    let linhas_lidas = t.varrer().unwrap();
    let mut chaves: Vec<(Vec<u8>, u64)> = Vec::with_capacity(linhas_lidas.len());
    for (rowid, linha) in &linhas_lidas {
        let mut buf = vec![0u8; largura];
        escrever_componente(&linha[0], &tipo, false, false, &mut buf).unwrap();
        chaves.push((buf, *rowid));
    }
    let varrer = inicio.elapsed().as_secs_f64();

    let inicio = Instant::now();
    chaves.sort_unstable();
    let ordenar = inicio.elapsed().as_secs_f64();

    let _ = std::fs::remove_dir_all(&dir);

    println!("\n=== o piso de uma reconstrucao EM LOTE ===\n");
    println!(
        "  varrer o `.reg` e codificar {} chaves .... {varrer:.2}s",
        chaves.len()
    );
    println!("  ordenar as chaves ....................... {ordenar:.2}s");
    println!("  paginas de indice a encher .............. {paginas}");
    // A varredura e UMA para os dois indices; a ordenacao e por indice. Um
    // lote de verdade ainda gravaria as paginas, mas em SEQUENCIA: sao poucos
    // milhares, a ~2,3 us de CRC cada.
    println!(
        "\n  Piso: {varrer:.2}s de varredura -- uma so, para os dois -- mais\n  \
         {ordenar:.2}s de ordenacao por indice, contra os {custo_de_hoje:.2}s que o\n  \
         `reindexar` cobra pelos dois. Foi para ca que a construcao em lote\n  \
         trouxe o `reindexar`: ele insere as chaves ORDENADAS, enchendo folha\n  \
         por folha, e nao mais uma descida na arvore por chave."
    );
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    println!("=== adiar o .ndx numa carga de {n} linhas ===");
    println!("\nO `reindexar` esta DENTRO da conta: adiar nao apaga trabalho,");
    println!("move de lugar e faz em lote. O ganho e a diferenca entre N descidas");
    println!("aleatorias na arvore e uma varredura do `.reg` com insercao em ordem.");

    rodada("chaves crescentes (arquivo ja ordenado)", n, false);
    let custo_de_hoje = rodada("chaves embaralhadas (o caso comum)", n, true);
    piso_do_lote(&linhas(n, true), custo_de_hoje);

    println!(
        "\n  A linha «adiar OS DOIS» e o TETO, e ela custa a garantia de\
         \n  unicidade: a conferencia de chave repetida acontece ANTES de gravar\
         \n  porque o `.reg` nunca reaproveita slot. Descobrir a duplicata depois\
         \n  deixaria um buraco permanente por linha recusada."
    );
}
