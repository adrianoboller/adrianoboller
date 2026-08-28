//! Cria uma tabela com `Uuid`, `Uuid256` e `Sequence`, para ver os tres tipos
//! funcionando -- na linha de comando, na interface web e pelo protocolo.
//!
//! ```bash
//! cargo run --example identificadores -- /caminho/dos/dados/MeuBanco
//! ```
//!
//! Existe porque criar tabela ainda so se faz escrevendo Rust: nao ha operacao
//! no protocolo nem comando na linha de comando para isso (esta registrado em
//! `docs/PENDENCIAS.md`). Enquanto nao houver, este exemplo e o caminho curto.

use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::uuid::{Uuid, Uuid256};
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::temp_dir()
            .join("phxsql-ids")
            .display()
            .to_string()
    });
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir)?;

    let esquema = Schema::new(
        "blocos",
        vec![
            // v7 no identificador: a ordem do indice vira a ordem de criacao,
            // e a insercao cai sempre na folha mais a direita da B+tree.
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            // Trinta e dois bytes: um SHA-256 cabe exato, sem virar texto.
            Column::new("hash", ColumnType::Uuid256).obrigatoria(),
            Column::new("anterior", ColumnType::Uuid256),
            // A altura do bloco. Nula na insercao = o motor numera.
            Column::new("altura", ColumnType::Sequence),
            Column::new("autor", ColumnType::Str(60)),
            Column::new("carimbo", ColumnType::DateTime),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porHash", vec![IndexColumn::asc(1)]).unico(),
            IndexDef::new("porAltura", vec![IndexColumn::asc(3)]).unico(),
        ],
    )?;

    let mut t = match Table::abrir(&dir, "blocos") {
        Ok(t) => {
            println!("tabela ja existia em {}", dir.display());
            t
        }
        Err(_) => {
            println!("criando blocos em {}", dir.display());
            Table::criar(&dir, esquema)?
        }
    };

    let comeco = t.slots();
    let mut anterior = Value::Null;
    for i in 0..10 {
        let hash = Uuid256::aleatorio();
        let rowid = t.inserir(&[
            Value::Uuid(Uuid::v7()),
            Value::Uuid256(hash),
            anterior.clone(),
            Value::Null,
            Value::Str(format!("mineirador-{}", i % 3)),
            Value::DateTime(agora_ms()),
        ])?;
        anterior = Value::Uuid256(hash);
        if i == 0 {
            println!("primeiro rowid: {rowid}");
        }
    }
    t.sincronizar()?;

    println!(
        "{} blocos gravados, proxima altura = {}",
        t.slots() - comeco,
        t.sequencia_atual()
    );

    // Percorrer o indice do id devolve na ordem de criacao, sem ordenar nada:
    // e o v7 fazendo o trabalho.
    for rowid in t.varrer_indice("porId")?.into_iter().take(3) {
        let l = t.ler(rowid)?.expect("bloco vivo");
        println!(
            "  altura {:>3}  id {}  hash {}",
            match l[3] {
                Value::UInt(n) => n.to_string(),
                _ => "?".into(),
            },
            match &l[0] {
                Value::Uuid(u) => u.to_string(),
                _ => "?".into(),
            },
            match &l[1] {
                Value::Uuid256(u) => u.to_string()[..16].to_string(),
                _ => "?".into(),
            }
        );
    }
    Ok(())
}

fn agora_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
