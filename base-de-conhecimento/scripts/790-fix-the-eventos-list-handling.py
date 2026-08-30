# Fix the eventos list handling
# 28/08 20:13

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace("""            .ok_or_else(|| PhxError::Esquema("informe \\"eventos\\" como lista".into()))?
            .clone();""",
"""            .ok_or_else(|| PhxError::Esquema("informe \\"eventos\\" como lista".into()))?
            .to_vec();""")
s = s.replace("        for e in &eventos {\n            let operacao = match e.texto_ou(",
              "        for e in eventos.iter() {\n            let operacao = match e.texto_ou(")
p.write_text(s)
