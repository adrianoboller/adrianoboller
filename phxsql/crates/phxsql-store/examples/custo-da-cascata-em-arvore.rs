//! Quanto custa alterar a chave da AVO, por profundidade da cascata.
//!
//! # A premissa que este medidor existe para conferir
//!
//! O pedido 169 diz que planejar a arvore inteira antes da primeira escrita
//! «passa a abrir netas e bisnetas a cada alteracao de chave», e trata isso
//! como o custo novo da proposta. Antes de implementar o item, mede-se a
//! premissa do item -- inclusive quando o item e nosso.
//!
//! A suspeita a medir e a oposta: a arvore JA e percorrida hoje, porque
//! `aplicar_ao_alterar` grava a filha por um `atualizar` INTEIRO, e esse
//! `atualizar` planeja a propria cascata. O que mudaria nao seria «passar a
//! andar a arvore», e sim ANDAR ANTES de gravar em vez de andar depois.
//!
//! Roda com:
//! `cargo run --release --example custo-da-cascata-em-arvore -p phxsql-store`

use phxsql_core::schema::{AcaoRi, Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use std::time::Instant;

/// Irmas sem chave nenhuma, para a varredura dos esquemas custar o que custa
/// num diretorio real em vez de num de brinquedo.
const IRMAS: usize = 20;
/// Voltas de cada medicao.
const VOLTAS: usize = 40;

fn dir(rotulo: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("phx-arvore-{rotulo}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Uma tabela do elo: `id` unico, `pai` indexado, chave para o elo de cima.
fn elo(d: &std::path::Path, nome: &str, acima: Option<&str>) -> Table {
    let e = Schema::new(
        nome,
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("pai", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porPai", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap();
    let e = match acima {
        None => e,
        Some(mae) => e
            .com_chaves_estrangeiras(vec![ForeignKey::new(
                format!("fk_{mae}"),
                vec![1],
                mae,
                // O elo aponta para a coluna `pai` da mae, e nao para o `id`:
                // e assim que a cascata desce de verdade, um nivel puxando o
                // seguinte pela MESMA coluna que acabou de mudar.
                vec![if mae == "n0" {
                    "id".into()
                } else {
                    "pai".into()
                }],
            )
            .ao_alterar(AcaoRi::Cascata)])
            .unwrap(),
    };
    Table::criar(d, e).unwrap()
}

/// Irmas que nao participam de cascata nenhuma: elas so engordam a varredura.
fn irmas(d: &std::path::Path) {
    for i in 0..IRMAS {
        let e = Schema::new(
            format!("irma{i}"),
            vec![Column::new("id", ColumnType::Int4).obrigatoria()],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        Table::criar(d, e).unwrap();
    }
}

/// Monta uma corrente de `niveis` elos abaixo da avo e mede o `atualizar` da
/// avo. Devolve os microssegundos medianos.
fn medir(niveis: usize) -> f64 {
    let d = dir(&format!("n{niveis}"));
    irmas(&d);

    let mut avo = elo(&d, "n0", None);
    let r = avo.inserir(&[Value::Int(1), Value::Int(0)]).unwrap();
    avo.sincronizar().unwrap();

    // Cada elo aponta para a coluna que o elo de cima acabou de mudar, entao
    // a cascata desce a corrente inteira.
    let mut abaixo: Vec<Table> = Vec::new();
    for n in 1..=niveis {
        let mae = format!("n{}", n - 1);
        let mut t = elo(&d, &format!("n{n}"), Some(&mae));
        t.inserir(&[Value::Int(10 + n as i64), Value::Int(1)])
            .unwrap();
        t.sincronizar().unwrap();
        abaixo.push(t);
    }
    drop(abaixo);

    let mut us: Vec<f64> = Vec::with_capacity(VOLTAS);
    for v in 0..VOLTAS {
        // Vai e volta entre dois valores, para toda volta ser uma alteracao de
        // chave de verdade e a cascata ter sempre o que fazer.
        let novo = if v % 2 == 0 { 2 } else { 1 };
        let t0 = Instant::now();
        avo.atualizar(r, &[Value::Int(novo), Value::Int(0)])
            .expect("a cascata recusou -- o cenario mudou");
        us.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    std::fs::remove_dir_all(&d).ok();
    us[us.len() / 2]
}

fn main() {
    println!("Custo de alterar a chave da avo, por profundidade da cascata");
    println!("({IRMAS} irmas sem chave no diretorio, mediana de {VOLTAS} voltas)\n");
    println!("  niveis abaixo | mediana (us) | contra 0 nivel");
    println!("  --------------+--------------+---------------");
    let base = medir(0);
    println!("  {:>13} | {:>12.1} |          1,00x", 0, base);
    for n in 1..=4 {
        let t = medir(n);
        println!("  {:>13} | {:>12.1} | {:>14.2}x", n, t, t / base);
    }
    println!(
        "\nO que este numero respondeu: o custo CRESCE linearmente com a\n\
         profundidade, entao a arvore JA era percorrida antes do pedido 169 --\n\
         ele nao acrescentou a travessia, so a antecipou para antes da primeira\n\
         escrita. A premissa do pedido, que falava em «passar a abrir netas e\n\
         bisnetas», estava errada.\n\n\
         Medido antes de a conferencia da arvore existir, nesta maquina:\n\
         254,6 / 2.289,9 / 4.204,1 / 6.066,6 / 8.765,4 us para 0 a 4 niveis.\n\
         Depois dela: 268,6 / 2.584,4 / 5.676,7 / 8.302,1 / 11.821,3 --\n\
         1,13x com um nivel e ~1,35x de dois em diante, e sem cascata o custo\n\
         nao se mexe. A previsao era 2x, e errou para cima: a passada nova so\n\
         PLANEJA, e planejar e a metade barata."
    );
}
