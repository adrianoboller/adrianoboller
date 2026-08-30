# Exempt aplicar from the read-only guard
# 28/08 20:14

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """    // Gravam o cadastro de ligacoes, que e arquivo deste servidor.
    "dblink_salvar",
    "dblink_excluir","""
novo = """    // Gravam o cadastro de ligacoes, que e arquivo deste servidor.
    "dblink_salvar",
    "dblink_excluir",
    // `aplicar` NAO entra aqui, e a ausencia e deliberada. Uma replica roda em
    // `somente_leitura` justamente para a aplicacao nao escrever nela -- e a
    // unica escrita que ela deve aceitar e a que vem do source. Barrar
    // `aplicar` aqui tornaria impossivel replicar para uma replica protegida,
    // que e a unica replica que se sustenta. Quem pode chamar `aplicar` ja
    // passou pelo portao do `administrar`."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
