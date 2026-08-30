# Add the bulk-load entries and recount
# 28/08 20:02

import pathlib
p = pathlib.Path("docs/PENDENCIAS.md")
s = p.read_text()
antigo = """| ☑️ | 67 | **Botão e menu Tabelas**"""
novo = """| ☑️ | 108 | **Carga em lote — várias linhas de uma vez** | `inserir_lote` no protocolo, `phxsql importar` na linha de comando. Medido com 20.000 linhas pela rede: **2.715 → 25.985 linhas/s (9,6×)**. O ganho não é do disco: é de abrir a tabela, tomar a trava e sincronizar UMA vez em vez de vinte mil |
| ☑️ | 109 | **Tela para colar JSON, CSV, TXT, HTML ou XML** | os cinco formatos, com o motor adivinhando qual é. A primeira linha manda, e as colunas casam pelo **nome** e não pela posição. `importar_conferir` mostra o que entendeu antes de gravar; o botão de gravar só acende depois disso |
| ☑️ | 67 | **Botão e menu Tabelas**"""
assert antigo in s
s = s.replace(antigo, novo)
s = s.replace("**84 feitos · 5 parciais · 6 planejados**, de 95 pedidos.",
              "**87 feitos · 5 parciais · 5 planejados**, de 97 pedidos.")
p.write_text(s)
print("ok")
