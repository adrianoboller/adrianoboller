# Restore missing brace and rebuild
# 29/08 17:23

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = """            ("nos", Json::Lista(nos)),
        ]))
    /// Uma passada bidirecional: puxa do outro lado e aplica POR CHAVE."""
novo = """            ("nos", Json::Lista(nos)),
        ]))
    }

    /// Uma passada bidirecional: puxa do outro lado e aplica POR CHAVE."""
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("fechamento reposto")
