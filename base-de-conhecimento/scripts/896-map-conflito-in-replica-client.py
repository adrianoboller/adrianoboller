# Map CONFLITO in replica client
# 28/08 23:53

import pathlib
p = pathlib.Path("crates/phxsql-server/src/replica.rs")
s = p.read_text()
s = s.replace('''                "LIMITE_EXCEDIDO" => PhxError::LimiteExcedido(texto),''',
'''                "LIMITE_EXCEDIDO" => PhxError::LimiteExcedido(texto),
                "CONFLITO" => PhxError::Conflito(texto),''', 1)
p.write_text(s)
print("ok")
