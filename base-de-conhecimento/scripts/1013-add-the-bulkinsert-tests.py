# Add the BULKINSERT tests
# 29/08 02:56

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
s = s.replace('''    "inserir_lote",''', '''    "inserir_lote",
    // Reservar a tabela para carga e declarar intencao de gravar. Num servidor
    // somente-leitura ninguem vai carregar nada, e deixar reservar seria
    // deixar travar a tabela para uma escrita que nunca acontece.
    "bulkinsert",
    "carga",''', 1)
s = s.replace('''            // Carga em lote grava, e grava muito.
            "inserir_lote",''','''            // Carga em lote grava, e grava muito.
            "inserir_lote",
            // Reservar a tabela e declarar que vai gravar.
            "bulkinsert",''',1)
p.write_text(s)
