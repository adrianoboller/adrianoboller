//! Gera um arquivo de cada formato, para conferir com leitor de verdade.
//!
//! ```text
//! cargo run --example prova-exportar -- /tmp/saida
//! ```

use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::exportar::{Coluna, Formato, Planilha};

fn main() {
    let destino = std::env::args().nth(1).unwrap_or_else(|| "/tmp".into());
    std::fs::create_dir_all(&destino).unwrap();

    let colunas = vec![
        Coluna {
            nome: "id".into(),
            ty: ColumnType::Int4,
        },
        Coluna {
            nome: "nome do cliente".into(),
            ty: ColumnType::Str(40),
        },
        Coluna {
            nome: "cidade".into(),
            ty: ColumnType::Str(30),
        },
        Coluna {
            nome: "limite".into(),
            ty: ColumnType::Decimal {
                precisao: 12,
                escala: 2,
            },
        },
        Coluna {
            nome: "nascimento".into(),
            ty: ColumnType::Date,
        },
        Coluna {
            nome: "cadastro".into(),
            ty: ColumnType::DateTime,
        },
        Coluna {
            nome: "ativo".into(),
            ty: ColumnType::Bool,
        },
    ];
    let linhas = vec![
        vec![
            Value::Int(1),
            Value::Str("Adriano \"Boller\" & Cia; Ltda".into()),
            Value::Str("Joinville".into()),
            Value::Decimal(1_500_050),
            Value::Date(1_896),
            Value::DateTime(1_787_000_000_000),
            Value::Bool(true),
        ],
        vec![
            Value::Int(2),
            Value::Str("Maria <Silva>".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(230_000),
            Value::Date(6_880),
            Value::DateTime(1_780_000_000_000),
            Value::Bool(true),
        ],
        vec![
            Value::Int(3),
            Value::Str("João Pereira — acentuação".into()),
            Value::Str("São Paulo".into()),
            Value::Null,
            Value::Date(7_506),
            Value::Null,
            Value::Bool(false),
        ],
    ];

    let p = Planilha {
        titulo: "Clientes".into(),
        subtitulo: "loja · exportado do PhxSql".into(),
        colunas,
        linhas: &linhas,
    };

    for f in [
        Formato::Csv,
        Formato::Txt,
        Formato::Json,
        Formato::Xml,
        Formato::Html,
        Formato::Xlsx,
        Formato::Docx,
    ] {
        let bytes = p.gerar(f).unwrap();
        let caminho = format!("{destino}/clientes.{}", f.extensao());
        std::fs::write(&caminho, &bytes).unwrap();
        println!("{:>5} {:>8} bytes  {caminho}", f.extensao(), bytes.len());
    }
}
