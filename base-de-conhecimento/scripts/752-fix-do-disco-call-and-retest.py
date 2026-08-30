# Fix do_disco call and retest
# 28/08 19:46

import pathlib
p = pathlib.Path("crates/phxsql-store/tests/paginacao.rs")
s = p.read_text()
antigo = """    let bytes = esquema().serializar();
    let sem = Schema::do_disco(&bytes).unwrap();"""
novo = """    let sem = Schema::do_disco(
        "sem_ordem",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ],
        vec![],
    )
    .unwrap();"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
