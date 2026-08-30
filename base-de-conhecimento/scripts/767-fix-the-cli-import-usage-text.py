# Fix the CLI import usage text
# 28/08 20:01

import pathlib
p = pathlib.Path("MANUAL.txt")
s = p.read_text()
s = s.replace("""        phxsql importar <dir> <tabela> --arquivo dados.csv [--formato csv]
                        [--conferir] [--seguir]""",
"""        phxsql importar <dir> <tabela> <arquivo>
                        [--formato csv|txt|json|xml|html]
                        [--conferir]   le e mostra, sem gravar
                        [--seguir]     nao para na primeira linha recusada""")
p.write_text(s)
