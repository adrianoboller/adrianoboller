//! Catraca: quantos `fsync` um FECHO DE JANELA custa. So desce.
//!
//! `Table::sincronizar` manda para o disco `.trash`, `.bin`, `.memo`, `.log`,
//! `.reason`, `.ndx` (duas vezes -- o `.ndx` sincroniza o arquivo principal e
//! o espelho de paginas sujas por dentro) e `.reg`, nessa ordem
//! (`crates/phxsql-store/src/table.rs`, `Table::sincronizar`). No caminho do
//! FECHO DE JANELA -- um `Table` reaberto so para sincronizar, sem ler nem
//! escrever nada antes, que e' o que `descarregar_sujas_com` faz -- o numero
//! de hoje e' **7**, nao 8: falta o `.reg`, e falta pelo MESMO defeito que
//! `fecho-da-janela-sincroniza-o-reg.rs` prova ao lado (`Volumes::sincronizar`
//! so toca o que esta em `abertos`, e este `Table` nunca abriu o `.reg`).
//!
//! Medido nesta rodada em tres escalas de semeadura -- 20, 2.000 e 200.000
//! linhas -- porque um numero medido uma vez so e' o mesmo risco que o
//! `~20 toques de pagina` do `.ndx` que `DESEMPENHO.md` ja pagou: 7 em toda
//! escala e' o que separa "e' o caminho fixo de arquivos" de "e' proporcional
//! a alguma coisa que cresceu no teste". As tres bateram em 7 -- fixo,
//! independente do tamanho da tabela, porque o fecho da janela e' POR
//! ARQUIVO e nao por linha.
//!
//! Quatro desses sete arquivos nao mudaram nada com um `inserir` comum:
//! `.trash` (so a exclusao escreve), `.reason` (idem), `.bin` e `.memo` (so
//! coluna externa escreve). Sincronizar um arquivo que ninguem sujou e' a
//! divida que esta catraca existe para cobrar -- e ela so desce: quando um
//! conserto aprender a pular o `fsync` de um arquivo que nao mudou desde o
//! ultimo `sincronizar`, o numero medido cai e o teto desce no mesmo commit.
//!
//! # A catraca NAO cobre o defeito do `.reg` -- de proposito
//!
//! Esta catraca mede DESPERDICIO (fsync de arquivo que nao mudou), nao
//! CORRECAO (fsync que falta e deveria estar la). As duas sao independentes:
//! reduzir desperdicio pode tirar `.trash`/`.reason`/`.bin`/`.memo` da conta e
//! baixar o teto para 3; consertar o `.reg` acrescenta UM fsync necessario e
//! sobe o numero de verdade para 8. A lei "catraca so desce, nunca sobe" nao
//! se aplica ao segundo caso do jeito ingenuo -- subir o TETO para acomodar
//! um numero que cresceu por CORRECAO seria a mesma armadilha que
//! `bancada/guardas/LEIA-ME.md` ja nomeou para o `TETO_TABELA_NA_MAO`: a
//! saida e' APOSENTAR `TETO_FSYNC_POR_FECHO_V1` (7) e fazer nascer
//! `TETO_FSYNC_POR_FECHO_V2` (8) no MESMO commit que liga o fsync que falta
//! -- nunca so subir este numero. Ate la, 7 e' o teto, e ele reprova
//! qualquer coisa ACIMA de 7 -- inclusive um oitavo fsync solto por
//! descuido, que nao seria o conserto do `.reg`.

mod comum;

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;

const VAR_DIR: &str = "PHX_CATRACA_FSYNC_DIR";

/// So desce. Ver a documentacao do modulo para o numero e a ressalva sobre
/// consertos que legitimamente pedem um V2.
pub const TETO_FSYNC_POR_FECHO_V1: usize = 7;

fn esquema() -> Schema {
    Schema::new(
        "sonda",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

fn linha(i: i64) -> Vec<Value> {
    vec![Value::Int(i), Value::Str(format!("nome {i}"))]
}

fn semear(dir: &std::path::Path, linhas: i64) {
    let mut t = Table::criar(dir, esquema()).unwrap();
    for i in 1..=linhas {
        t.inserir(&linha(i)).unwrap();
    }
    t.sincronizar().unwrap();
    let mut t = Table::abrir(dir, "sonda").unwrap();
    t.inserir(&linha(linhas + 1)).unwrap();
    // De proposito: sem sincronizar. E' o estado que o fecho da janela
    // encontra -- o mesmo cenario da guarda irma.
}

/// O corpo tracado -- so o fecho da janela. `#[ignore]`: nao entra na
/// bateria normal, so' roda reexecutado pelo teste de verdade.
#[test]
#[ignore = "corpo tracado da sonda -- so roda reexecutado por \
            fecho_de_janela_nao_pode_custar_mais_fsync_do_que_hoje, com \
            PHX_CATRACA_FSYNC_DIR setada"]
fn sonda_fecho_de_janela() {
    let Ok(dir) = std::env::var(VAR_DIR) else {
        return;
    };
    let mut t = Table::abrir(&dir, "sonda").expect("abrir a tabela semeada");
    t.sincronizar().expect("fechar a janela");
}

/// Mede o fecho de janela nas tres escalas e confere que nenhuma passou do
/// teto. Ver a documentacao do modulo para o porque de tres escalas.
#[test]
fn fecho_de_janela_nao_pode_custar_mais_fsync_do_que_hoje() {
    for linhas in [20i64, 2_000, 200_000] {
        let d = comum::DirTemp::novo(&format!("catraca-fsync-{linhas}"));
        semear(&d, linhas);

        let log = d.join("strace.log");
        let Some(saida) =
            comum::tracar_syscalls("sonda_fecho_de_janela", "fsync", VAR_DIR, &d, &log)
        else {
            eprintln!("strace nao esta instalado nesta maquina -- catraca pulada");
            return;
        };

        let fsyncs = saida.lines().filter(|l| l.contains("fsync(")).count();
        assert!(
            fsyncs <= TETO_FSYNC_POR_FECHO_V1,
            "com {linhas} linha(s) semeada(s), o fecho da janela gastou \
             {fsyncs} fsync(s); o teto e' {TETO_FSYNC_POR_FECHO_V1}. So \
             desce -- se o fsync a mais e' de um CONSERTO de verdade (o \
             `.reg` que falta), aposente TETO_FSYNC_POR_FECHO_V1 e crie um \
             V2 no mesmo commit; nunca so suba este numero. Log completo em \
             {}",
            log.display()
        );
    }
}
