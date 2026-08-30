# Update the comparison table
# 29/08 00:42

import pathlib
p = pathlib.Path("docs/DESEMPENHO.md")
s = p.read_text()
alvo = '''| Fase | PhxSql | MySQL(R) | |
|---|---:|---:|---|
| inserir 10.000.000 | 884,3 s | 115,3 s | **0,13×** — o buraco |
| buscar 20.000 por chave | 5,08 s | 2,67 s | 0,53× |
| excluir | 4,85 s | 5,44 s | **1,12×** |
| atualizar | 4,44 s | 6,06 s | **1,36×** |
| varrer faixa | 3,94 s | 18,97 s | **4,82×** |

A leitura sequencial é onde o formato de slot fixo paga: quase 5× o MySQL(R). A
inserção é onde ele cobra.'''
novo = '''| Fase | PhxSql 0.16.0 | PhxSql 0.17.0 | MySQL(R) | |
|---|---:|---:|---:|---|
| inserir 10.000.000 | 884,3 s | **303,0 s** | 115,2 s | 0,38× (era 0,13×) |
| buscar 20.000 por chave | 5,08 s | **2,62 s** | 2,60 s | **0,99×** — empate |
| varrer faixa | 3,94 s | **3,28 s** | 26,19 s | **7,98×** |
| atualizar | 4,44 s | **1,92 s** | 6,33 s | **3,30×** |
| excluir | 4,85 s | 8,16 s | 6,25 s | 0,77× — ver abaixo |

O cache de páginas mudou quatro das cinco linhas: a inserção ficou **2,92×** mais
rápida, a busca por chave **empatou** com o MySQL(R) (era metade da velocidade
dele), e a alteração passou de 1,36× para 3,30×.

**Sobre a exclusão, honestamente.** É a única fase em que o PhxSql *espera
disco*: 4,3 s de CPU para 8,16 s de relógio. Ela grava a linha inteira no
`.trash` e **sincroniza antes** de liberar o slot — a ordem que garante que a
linha nunca deixa de existir nos dois lugares ao mesmo tempo. O número anterior
(4,85 s) saiu de outra corrida, em outro estado de disco, e repetir a fase
sozinha na mesma máquina deu de **0,80 s a 2,76 s**. Ou seja: esta linha varia
demais entre corridas para sustentar «piorou» ou «melhorou», e nada no caminho
dela mudou nesta versão. Fica publicada como medida, com a instabilidade dita.

A leitura sequencial continua sendo onde o formato de slot fixo paga: **8× o
MySQL(R)**. A inserção é onde ele cobra — e cobra 3× menos que cobrava.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''| Varrer faixa | 4,8× o MySQL(R) | slot de largura fixa, sem página, sem MVCC |''',
              '''| Varrer faixa | 8,0× o MySQL(R) | slot de largura fixa, sem página, sem MVCC |''', 1)
p.write_text(s)
print("ok")
