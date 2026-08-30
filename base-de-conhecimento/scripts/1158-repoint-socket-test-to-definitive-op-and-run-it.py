# Repoint socket test to definitive op and run it
# 29/08 17:34

import pathlib
p = pathlib.Path("crates/phxsql-server/tests/sonda-da-replicacao.rs"); t = p.read_text()
t = t.replace("replicacao_sondar", "replicacao_testar")
t = t.replace("""//! `replicacao_testar` existe para o assistente de replicacao testar a conexao""",
"""//! `replicacao_testar` existe para o assistente de replicacao provar a conexao""")
p.write_text(t); print("teste reapontado para a op definitiva")
