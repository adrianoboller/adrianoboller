# Update the replication doc, part 1
# 28/08 20:33

import pathlib
p = pathlib.Path("docs/REPLICACAO.md")
s = p.read_text()

antigo = """# Replicação no PhxSql

**Pergunta:** dá para ter no PhxSql a replicação Source → Replica do MySQL(R)?

**Resposta curta:** dá, e o PhxSql já está a meio caminho — porque o `.log`
que você pediu **é exatamente o binlog**. O que falta é uma coisa só, e ela é
uma mudança de formato que vale fazer agora.

---
"""
novo = """# Replicação no PhxSql

**Pergunta:** dá para ter no PhxSql a replicação Source → Replica do MySQL(R)?

**Resposta:** dá, e desde a 0.15.0 **está funcionando**. O que faltava era uma
coisa só — a imagem da linha no `.log` — e ela entrou.

Quatro servidores no ar, com a medição em `bancada/replicacao/`:

```
Master 5800 ──┬──► Slave01 5801
              ├──► Slave02 5802
              └──► Slave03 5803
```

| | |
|---|---|
| Master, com a imagem no diário | 18.773 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
| Atraso de uma escrita até as três | 1,3 s a 2,1 s |
| Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |
| Retrato SHA-256 das quatro tabelas, no fim | idênticos |

O que ainda **não** existe está na seção 10, e um item mudou de lugar: a réplica
aplica mais devagar do que o master escreve, e sob carga sustentada fica atrás.

---
"""
assert antigo in s
s = s.replace(antigo, novo)

s = s.replace("""| Row-based binlog (imagem da linha) | **falta** | ver seção 3 |""",
              """| Row-based binlog (imagem da linha) | `.log` v2, atrás de `imagem_da_linha` | **existe** |""")

antigo = """---

## 3. A única peça que falta: a imagem da linha

O `.log` de hoje guarda 36 bytes por evento — carimbo, operação, rowid, versão,
usuário e CRC. Falta o conteúdo."""
novo = """---

## 3. A peça que faltava: a imagem da linha

Até a 0.14.0 o `.log` guardava 36 bytes por evento — carimbo, operação, rowid,
versão, usuário e CRC. Faltava o conteúdo: o evento dizia *que* o rowid 42
mudou, não dizia *para quê*."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """### Formato proposto (versão 2 do `.log`)

Cabeçalho do evento passa de 36 para **44 bytes**, e ganha um corpo:"""
novo = """### O formato (versão 2 do `.log`)

Cabeçalho do evento passou de 36 para **44 bytes**, e ganhou um corpo:"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """| 32 | 4 | **tamanho da imagem** |
| 36 | 4 | CRC-32 do cabeçalho e da imagem |
| 40 | 4 | reservado |
| 44 | N | **imagem da linha** |"""
novo = """| 32 | 4 | **tamanho da imagem** |
| 36 | 4 | CRC-32 do cabeçalho **e da imagem** |
| 40 | 4 | reservado |
| 44 | N | **imagem da linha** |

O CRC cobrir a imagem, e não só o cabeçalho, é o detalhe que importa: a imagem
é o que a réplica grava **como dado**. Um byte trocado ali entraria na réplica
sem ninguém notar.

E há um preço que o formato cobra: até a versão 1 o evento N morava no offset
`64 + N × 36`, e pular era uma conta. Agora não é — chegar ao evento N é
caminhar pelos anteriores lendo o tamanho de cada um. O que salva a leitura é o
`qtd_eventos` no cabeçalho de cada volume: um volume inteiro se pula sem abrir."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """Uma tabela com registro de 200 bytes passa a gastar ~244 bytes de diário por
alteração, em vez de 36. Por isso o `.log` já nasceu paginado, e por isso a
imagem fica atrás de um interruptor no `config.json`:

```json
"replicacao": { "imagem_da_linha": true }
```

Quem só quer auditoria deixa desligado e continua com 36 bytes por evento.
Quem quer replicar liga."""
novo = """Medido, mesma tabela e mesmas 100.000 linhas, só o interruptor mudando:

| `imagem_da_linha` | linhas/s | bytes por evento | `.log` |
|---|---:|---:|---:|
| desligada | 21.740 | 44 | 4,4 MB |
| ligada | 19.531 | 223 | 22,3 MB |

**10% mais devagar, e um diário 5,1× maior.** Por isso o `.log` já nasceu
paginado, e por isso a imagem fica atrás de um interruptor no `config.json`:

```json
"replicacao": { "imagem_da_linha": true }
```

Quem só quer auditoria deixa desligado e continua com 44 bytes por evento.
Quem quer replicar liga — e num servidor com `papel: source` ela **já vem
ligada**, porque um source sem imagem no diário é um source que não replica, e
descobrir isso pela réplica parada seria o pior jeito de descobrir. O arranque
avisa em voz alta se alguém desligar."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
