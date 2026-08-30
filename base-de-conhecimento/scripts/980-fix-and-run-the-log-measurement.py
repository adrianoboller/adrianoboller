# Fix and run the log measurement
# 29/08 01:46

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/custo-do-log.rs")
s = p.read_text()
s = s.replace('''use phxsql_core::schema::{Column, Schema};
use phxsql_core::types::ColumnType;
use phxsql_store::log::{LogFile, Operacao};

fn esquema() -> Schema {
    Schema::new(
        "diario",
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![],
    )
    .unwrap()
}
''','''use phxsql_core::paginacao::Paginacao;
use phxsql_store::log::{LogFile, Operacao};
''',1)
s = s.replace('let mut log = LogFile::criar(&dir, "t", &esquema()).unwrap();',
              'let mut log = LogFile::criar(&dir, "t", Paginacao::default()).unwrap();',1)
p.write_text(s)
