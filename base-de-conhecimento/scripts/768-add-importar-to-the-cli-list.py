# Add importar to the CLI list
# 28/08 20:01

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
antigo = """    phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]"""
novo = """    phxsql importar  <dir> <tabela> <arquivo>
                     [--formato csv|txt|json|xml|html] [--conferir] [--seguir]
        Carrega um arquivo inteiro de uma vez -- CSV, TXT com TAB, JSON, XML
        ou HTML. Sem --formato, o motor adivinha pelo conteudo. --conferir
        mostra o que entendeu sem gravar nada.

    phxsql log       <dir> <tabela> [--rowid <n>] [--max <n>]"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
