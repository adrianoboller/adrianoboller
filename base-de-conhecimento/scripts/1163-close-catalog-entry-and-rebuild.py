# Close catalog entry and rebuild
# 29/08 17:50

import pathlib
p = pathlib.Path("crates/phxsql-server/src/catalogo.rs"); t = p.read_text()
velho = """        exemplo: r#"{"op":"mensagens_semear"}"#,
        nome: "idiomas","""
novo = """        exemplo: r#"{"op":"mensagens_semear"}"#,
        ferramenta_mcp: false,
    },
    Operacao {
        nome: "idiomas","""
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("entrada do catalogo fechada")
