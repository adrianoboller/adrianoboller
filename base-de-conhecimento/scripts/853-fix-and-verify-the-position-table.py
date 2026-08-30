# Fix and verify the position table
# 28/08 22:34

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """    const bancos = (await api("bancos")).bancos || [];"""
novo = """    // `bancos` responde uma LISTA de nomes, e nao um objeto com `bancos`
    // dentro. Ler o campo errado devolvia vazio, e a tela dizia "nenhuma
    // tabela ainda" numa réplica que tinha a tabela na árvore ao lado.
    const bancos = await api("bancos");"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
