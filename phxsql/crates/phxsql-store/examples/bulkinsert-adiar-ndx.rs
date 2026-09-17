//! O `BULKINSERT` paga o `.ndx` linha a linha (divida escrita em
//! `crates/phxsql-server/src/carga.rs`). Adiar -- carregar so o `.reg`/`.log`
//! e reconstruir os indices no fim com `reindexar()` -- compra quanto, medido
//! na FORMA exata do `BULKINSERT`: tabela reservada, VAZIA, recebendo N
//! linhas de uma vez, sem leitura no meio (a objecao antiga -- indice
//! defasado que `buscar` veria -- nao existe aqui porque nao ha leitura
//! durante a reserva).
//!
//! ```bash
//! cargo run --release --example bulkinsert-adiar-ndx -p phxsql-store -- <n> [repeticoes]
//! ```
//!
//! Imprime, por repeticao, uma linha `RESULTADO {json}` com os tres tempos --
//! inline, adiado-carga, adiado-reindexar -- para uma bancada externa
//! calcular faixa min-max sobre VARIAS chamadas deste processo (a variancia
//! de processo unico nao substitui repeticao de verdade). Na PRIMEIRA
//! repeticao, alem de medir, ele CONFERE que os dois regimes terminam no
//! mesmo estado -- mesma contagem, mesmas respostas dos dois indices -- antes
//! de apagar as tabelas, e imprime `VERIFICACAO ok` ou explode dizendo o que
//! divergiu. Bancada que so mede tempo e nao confere resultado e bancada que
//! pode estar cronometrando um regime incompleto.
//!
//! # O truque para medir "adiar" sem o motor ter indice suspenso
//!
//! O motor ainda nao tem um modo "grava mas nao mantem o indice" (essa e
//! justamente a parte que falta, citada em `carga.rs`). Mas
//! `Table::reindexar()` **trunca o `.ndx` e reconstroi do `.reg` inteiro
//! sempre** (ve `table.rs`, `self.ndx = NdxFile::criar(...)` no comeco da
//! funcao) -- o tempo que ele leva depende so do que esta no `.reg` e dos
//! indices declarados, NUNCA do que o `.ndx` tinha antes. Por isso:
//!
//! - **carga adiada** = inserir as N linhas numa tabela com ZERO indices
//!   declarados (o `.reg`/`.log` puros, sem nenhum `.ndx` tocado);
//! - **reindexar** = medido de verdade com `reindexar()` sobre uma tabela
//!   IGUALMENTE cheia, com os dois indices declarados -- o tempo dele e
//!   identico ao que seria gasto reconstruindo a partir da tabela sem
//!   indices, porque `reindexar()` nunca reaproveita o `.ndx` anterior.
//!
//! Esta equivalencia e a mesma que `--example indice-adiado` ja usa (e que os
//! testes `paginacao_log_reindex.rs`/`ndx.rs` provam para `reindexar` e
//! `construir_em_lote`: o indice reconstruido responde IGUAL ao mantido ao
//! vivo). Este medidor soma uma conferencia PROPRIA, na forma exata do
//! `BULKINSERT` (tabela vazia, dois indices: um unico e um nao unico).

use std::time::Instant;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

/// Xorshift64*, para embaralhar sem crate de fora -- mesma formula de
/// `indice-adiado.rs`, para as duas bancadas gerarem a mesma forma de dado.
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

fn cidade_de(i: i64) -> &'static str {
    CIDADES[(i as usize) % CIDADES.len()]
}

fn linha_de(i: i64) -> Vec<Value> {
    vec![
        Value::Int(i),
        Value::Str(format!("Produto {i:08}")),
        Value::Str(cidade_de(i).into()),
    ]
}

/// As mesmas N linhas, em ordem EMBARALHADA -- o caso comum de uma carga de
/// verdade (arquivo importado, migracao de outro banco). A ordem crescente
/// nao entra aqui de proposito: `--example indice-adiado` ja mostrou que ela
/// e o caso raro e otimista (arvore ja ordenada), e o BULKINSERT existe para
/// o caso geral.
fn linhas_embaralhadas(n: i64) -> Vec<Vec<Value>> {
    let mut ids: Vec<i64> = (1..=n).collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for i in (1..ids.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        ids.swap(i, j);
    }
    ids.into_iter().map(linha_de).collect()
}

fn dir_limpa(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-bulk-adiar-{}-{rotulo}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Medida {
    inline_total: f64,
    adiado_carga: f64,
    adiado_reindexar: f64,
}

impl Medida {
    fn adiado_total(&self) -> f64 {
        self.adiado_carga + self.adiado_reindexar
    }
}

/// Confere que as duas tabelas terminam no MESMO estado: mesma contagem de
/// linhas vivas, e os dois indices respondendo IGUAL -- unico id a id, nao
/// unico cidade a cidade (como CONJUNTO de rowids, porque a ordem dentro da
/// pagina do indice nao unico e a de insercao, e as duas tabelas inseriram em
/// ordens diferentes por dentro do reindexar em lote).
fn conferir_mesmo_estado(inline: &mut Table, adiado: &mut Table, n: i64) {
    let ri = inline.registros();
    let ra = adiado.registros();
    assert_eq!(ri, ra, "contagem diverge: inline={ri} adiado={ra}");
    assert_eq!(ri, n as u64, "contagem nao bate com N: {ri} != {n}");

    for i in 1..=n {
        let bi = inline.buscar("porId", &[Value::Int(i)]).unwrap();
        let ba = adiado.buscar("porId", &[Value::Int(i)]).unwrap();
        assert_eq!(
            bi, ba,
            "indice UNICO diverge para id={i}: inline={bi:?} adiado={ba:?}"
        );
        assert_eq!(bi.len(), 1, "id={i} deveria achar exatamente uma linha");
    }

    for cidade in CIDADES {
        let mut bi = inline
            .buscar("porCidade", &[Value::Str(cidade.into())])
            .unwrap();
        let mut ba = adiado
            .buscar("porCidade", &[Value::Str(cidade.into())])
            .unwrap();
        bi.sort_unstable();
        ba.sort_unstable();
        assert_eq!(
            bi,
            ba,
            "indice NAO UNICO diverge para cidade={cidade}: {} vs {} rowids",
            bi.len(),
            ba.len()
        );
    }
}

/// Regime (a): inline -- os dois indices mantidos durante a insercao, como o
/// `BULKINSERT` faz hoje.
fn medir_inline(ls: &[Vec<Value>]) -> (f64, Table) {
    let dir = dir_limpa("inline");
    let esquema = Schema::new("carga", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    let inicio = Instant::now();
    for l in ls {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    (inicio.elapsed().as_secs_f64(), t)
}

/// Regime (b): adiado -- carrega SEM nenhum indice declarado (o piso do que o
/// `.reg`/`.log` custam sozinhos), depois reconstroi os dois de uma vez numa
/// tabela IGUALMENTE cheia com os indices declarados. Ve o comentario do
/// cabecalho do arquivo para o porque de separar as duas tabelas ser
/// equivalente a ter um modo de indice suspenso de verdade.
fn medir_adiado(ls: &[Vec<Value>]) -> (f64, f64, Table) {
    let dir_sem_indice = dir_limpa("adiado-carga");
    let esquema_sem_indice = Schema::new("carga", colunas(), vec![]).unwrap();
    let mut t_sem_indice = Table::criar(&dir_sem_indice, esquema_sem_indice).unwrap();
    let inicio = Instant::now();
    for l in ls {
        t_sem_indice.inserir(l).unwrap();
    }
    t_sem_indice.sincronizar().unwrap();
    let carga = inicio.elapsed().as_secs_f64();
    let _ = std::fs::remove_dir_all(&dir_sem_indice);

    let dir_reindex = dir_limpa("adiado-reindex");
    let esquema_com_indice = Schema::new("carga", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir_reindex, esquema_com_indice).unwrap();
    for l in ls {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    // A insercao acima MANTEVE os indices ao vivo (nao ha modo suspenso), mas
    // isso nao contamina a medida: `reindexar()` trunca o `.ndx` e comeca do
    // zero, entao o tempo dele daqui pra baixo e o mesmo que teria custado
    // reconstruir a partir da tabela `t_sem_indice`, que so tem `.reg`/`.log`.
    let inicio = Instant::now();
    t.reindexar().unwrap();
    t.sincronizar().unwrap();
    let reindexar = inicio.elapsed().as_secs_f64();
    (carga, reindexar, t)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let n: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let repeticoes: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);

    eprintln!(
        "=== bulkinsert-adiar-ndx: N={n} linhas, {repeticoes} repeticao(oes), \
         chaves embaralhadas (caso comum) ==="
    );

    for rep in 1..=repeticoes {
        let ls = linhas_embaralhadas(n);

        let (inline_total, mut t_inline) = medir_inline(&ls);
        let (adiado_carga, adiado_reindexar, mut t_adiado) = medir_adiado(&ls);

        let m = Medida {
            inline_total,
            adiado_carga,
            adiado_reindexar,
        };

        if rep == 1 {
            conferir_mesmo_estado(&mut t_inline, &mut t_adiado, n);
            eprintln!(
                "VERIFICACAO ok: {n} linhas, indice unico (porId) e nao unico \
                 (porCidade, {} cidades) identicos entre inline e adiado",
                CIDADES.len()
            );
        }

        // Os diretorios das tabelas so morrem aqui -- depois de qualquer
        // conferencia da repeticao.
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("phx-bulk-adiar-{}-inline", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!(
            "phx-bulk-adiar-{}-adiado-reindex",
            std::process::id()
        )));

        println!(
            "RESULTADO {{\"n\":{n},\"repeticao\":{rep},\"inline_total_s\":{:.6},\
             \"adiado_carga_s\":{:.6},\"adiado_reindexar_s\":{:.6},\
             \"adiado_total_s\":{:.6},\"ganho\":{:.4}}}",
            m.inline_total,
            m.adiado_carga,
            m.adiado_reindexar,
            m.adiado_total(),
            m.inline_total / m.adiado_total()
        );
    }
}
