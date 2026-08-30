# Update README and recount tests
# 28/08 20:36

import pathlib
p = pathlib.Path("README.md")
s = p.read_text()
antigo = """## Carga em lote"""
novo = """## Replicação: Master e espelhos

A réplica **procura** o master; o master não empurra nada. É o desenho do
MySQL(R), e existe por causa do firewall: o master abre uma porta de entrada e
não precisa alcançar ninguém de volta.

```json
"replicacao": { "papel": "source", "imagem_da_linha": true }
```

O `.log` sempre foi o binlog; o que faltava era a **imagem da linha** dentro do
evento — o payload cru do `.reg` mais o *conteúdo* dos anexos, porque os
ponteiros são offsets desta máquina.

Medido com quatro servidores (`bancada/replicacao/`):

| | |
|---|---|
| Master, com a imagem no diário | 18.773 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
| Atraso de uma escrita até as três | 1,3 s a 2,1 s |
| Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |
| Retrato SHA-256 das quatro tabelas, no fim | idênticos |

O `rowid` **não é transmitido**: o `.reg` nunca reaproveita slot, então uma
réplica que aplicou tudo na ordem chega ao mesmo número sozinha. Se não chegar,
divergiu — e a replicação para ali em vez de espalhar.

## Carga em lote"""
assert antigo in s
s = s.replace(antigo, novo)
s = s.replace("""O motor de armazenamento está completo e testado: **363 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 200), **567 no projeto inteiro**, sem
nenhuma dependência externa (só a `std`) — o que faz o projeto compilar
offline.""",
"""O motor de armazenamento está completo e testado: **369 testes** só nele
(`phxsql-core` 163 + `phxsql-store` 206), **573 no projeto inteiro**, sem
nenhuma dependência externa (só a `std`) — o que faz o projeto compilar
offline.""")
p.write_text(s)
print("ok")
