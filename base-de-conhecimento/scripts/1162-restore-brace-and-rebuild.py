# Restore brace and rebuild
# 29/08 17:50

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = """            ("ja_existiam", Json::de_u64(existiam)),
        ]))
    /// O portao PROPRIO das operacoes de idioma."""
novo = """            ("ja_existiam", Json::de_u64(existiam)),
        ]))
    }

    /// O portao PROPRIO das operacoes de idioma."""
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("fechamento reposto")
