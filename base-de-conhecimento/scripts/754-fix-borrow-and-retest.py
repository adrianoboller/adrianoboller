# Fix borrow and retest
# 28/08 19:47

import pathlib
p = pathlib.Path("crates/phxsql-store/tests/alfanumerica.rs")
s = p.read_text()
s = s.replace("""    t.excluir_de_vez(t.rowid_do_rownum(3).unwrap().unwrap(), "")
        .unwrap();""",
"""    let terceiro = t.rowid_do_rownum(3).unwrap().unwrap();
    t.excluir_de_vez(terceiro, "").unwrap();""")
p.write_text(s)
