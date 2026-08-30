# Rebuild and restart both servers
# 28/08 21:33

import pathlib
p = pathlib.Path("crates/phxsql-server/src/replica.rs")
s = p.read_text()
s = s.replace('''                "AUTORIZACAO" => PhxError::Autorizacao(texto),''',
              '''                "ACESSO_NEGADO" => PhxError::Autorizacao(texto),''')
s = s.replace('''                "TIPO" => PhxError::Tipo(texto),''',
              '''                "TIPO_INVALIDO" => PhxError::Tipo(texto),''')
p.write_text(s)
