# Fix the stale entry and commit
# 30/08 02:00

import io,re
p="phxsql/docs/PENDENCIAS.md"
s=io.open(p,encoding="utf-8").read()
i=s.index("| 1 | **Restaurar backup** |")
fim=s.index("\n",i)+1
novo = ("| 1 | ~~**Restaurar backup**~~ | **FECHADO nesta rodada** — virou o pedido 134. "
        "`restaurar_backup` com dois modos (com outro nome, que e o padrao, e por cima "
        "com a porta de dados parada), SHA-256 conferido antes de o destino ser tocado, "
        "e portao proprio para o banco que vem DENTRO do backup. Fica aqui riscado, e nao "
        "apagado, porque **esta lista ja se contradisse antes**: a §7 afirmava que a "
        "replicacao nao transportava evento enquanto o item 19 trazia a medicao do "
        "contrario. Item que fecha se risca na primeira leitura seguinte |\n")
s = s[:i] + novo + s[fim:]
io.open(p,"w",encoding="utf-8").write(s)
print("item 1 riscado")
