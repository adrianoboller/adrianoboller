# Secao 13 do DESEMPENHO.md
# 01/09 18:36

from pathlib import Path
p = Path("docs/DESEMPENHO.md")
s = p.read_text(encoding="utf-8")
alvo = "## Como refazer tudo"
assert s.count(alvo) == 1

secao = '''## 13. Os três a um milhão de linhas — e o piso que valia 59,6% de uma barra

`bancada/comparacao/medir.py`, três rodadas, tabela de 1.000.000 de linhas,
20.000 operações nas fases pontuais, os três motores **intercalados na mesma
rodada**. A medição crua está em `bancada/comparacao/um-milhao.json` e o
gráfico sai dela por `grafico.py`, que **recusa desenhar** se o arquivo não
existir.

### Por que uma bancada nova

Já havia duas: `bancada/medir.py` (PhxSql × MySQL(R)) e `bancada/sqlite/`
(PhxSql × SQLite(R)). Somar as duas tabelas daria três colunas e **nenhuma
comparação** — as medidas são de dias diferentes, com cargas diferentes na
máquina, e parte da diferença passaria a ser do ambiente. É o mesmo erro de
comparar escalas diferentes, com outra roupa.

### As medianas

| fase | PhxSql | SQLite(R) | MySQL(R) | MySQL(R) menos o piso |
|---|---:|---:|---:|---:|
| inserir 1.000.000 | 9,928 s | **2,557 s** | 12,342 s | — |
| buscar 20.000 | 0,164 s | 0,166 s | 2,481 s | 1,002 s |
| atualizar 20.000 | **0,277 s** | 1,028 s | 3,537 s | 2,058 s |
| excluir 20.000 | 1,053 s | **0,574 s** | 4,063 s | 2,583 s |

Por operação: buscar 8,2 µs contra 8,3 e 124,1; atualizar 13,8 contra 51,4 e
176,8; excluir 52,6 contra 28,7 e 203,1. Inserção: **100.724 linhas/s** contra
391.099 do SQLite(R) e 81.025 do MySQL(R).

### O achado: mais da metade da barra de busca do MySQL(R) não é o motor dele

Os três não têm a mesma forma, e não há como dar: o SQLite(R) é biblioteca em
processo, o `carga` do PhxSql também, e **o MySQL(R) é daemon que recebe texto
por soquete**. Não existe MySQL(R) embutido nesta máquina.

O que se pode fazer é medir o tamanho disso. 20.000 instruções que não fazem
trabalho nenhum (`DO 1;`), pelo mesmo caminho: **1,479 s**.

| fase | quanto da barra do MySQL(R) é piso |
|---|---:|
| buscar | **59,6%** |
| atualizar | 41,8% |
| excluir | 36,4% |

Sem esse número teríamos publicado *«o PhxSql busca 15,16× mais rápido que o
MySQL(R)»*. Descontado o piso são **6,12×** — ainda a nosso favor, e agora é um
número sobre motores em vez de um número sobre formatos. **Vitória que vem do
formato é a mentira mais convincente que existe**, e esta casa já a publicou
três vezes: duas a favor do outro motor e uma a favor do nosso.

### Onde perdemos, dito sem rodeio

**A inserção, para o SQLite(R), por 3,88×** (2,557 s contra 9,928 s). E o
**excluir, por 1,83×** (0,574 contra 1,053). Ganhamos o `atualizar` por 3,72×,
e o `buscar` **empata**: 164 ms contra 166 ms, com as faixas inteiramente
sobrepostas (151–215 contra 158–232 ms). Empate é empate — o gráfico foi
consertado para **não contornar vencedor quando as faixas se cruzam**, porque
contornar ali seria publicar ruído da máquina como resultado.

**E o disco:** 253,6 MiB contra 57,3 do SQLite(R) e 104,0 do MySQL(R) —
**4,42×** e **2,44×**. É o preço do modelo de arquivos separados, e no celular
essa é a pergunta inteira (`docs/MOBILE.md`).

### O que a escolha do esquema do SQLite(R) vale

Ele não tem tradução única para «chave em `id` mais índice em `cidade`»:
`id INTEGER PRIMARY KEY` são duas estruturas, `NOT NULL` mais `UNIQUE INDEX`
são três. Rodam as duas, e o publicado é o `rowid` — o que **casa com o
InnoDB** e o que **favorece o SQLite(R)**:

| fase | `rowid` | `2ind` | |
|---|---:|---:|---:|
| inserir | 2,557 s | 2,914 s | 1,14× |
| buscar | 0,166 s | 0,216 s | 1,31× |
| atualizar | 1,028 s | 1,074 s | 1,04× |
| excluir | 0,574 s | 0,744 s | 1,30× |

A escolha vale de 1,04× a 1,31× conforme a fase. Publicar a que nos favorece
teria melhorado três dos nossos quatro números sem o motor ter feito nada.

### A dispersão, que é por isso que o bigode existe

O `atualizar` do MySQL(R) foi **22,969 s na primeira rodada e 3,479 s na
terceira** — 6,6× entre corridas iguais, provavelmente o `buffer pool` ainda
digerindo o milhão recém-inserido. Uma rodada só teria decidido esse número, e
teria decidido errado nas duas direções possíveis.

### A regra 1 estava sendo violada, e nenhum tempo denunciava

Ao montar esta bancada apareceu um defeito na `bancada/medir.py`: ela grava
`'2024-10-04'` em **toda** linha, enquanto o `carga.rs` e a bancada do
SQLite(R) gravam `20000 + (i % 400)`. **Dado diferente, do mesmo tamanho** — e
invisível em qualquer medida de tempo.

O conserto não foi só gravar a data certa. Nasceu a fase `conferir` do
`carga.rs`, que soma o que existe na tabela e obriga os três a chegarem ao
**mesmo estado** antes de qualquer tempo ser publicado: contagem de linhas,
soma de `valor` e soma de `cadastro`, em três marcos — depois de inserir,
depois de atualizar e depois de excluir.

O marco do meio não é enfeite: `atualizar` e `excluir` mordem exatamente os
mesmos 20.000 alvos, então no marco final o efeito do `atualizar` já
desapareceu junto com as linhas excluídas. Sem ele, a fase `atualizar` não
teria prova nenhuma.

E os totais **conferem contra a forma fechada**, não só entre si: a soma de
`valor` de 1 a 1.000.000 é 410.099.600.000 e a de `cadastro` é 20.199.500.000,
calculadas à parte. Os três motores chegaram nas duas.

**Prova real nos dois sentidos:** repor a data constante faz a bancada reprovar
com `cadastro 400.000.000` contra `403.990.000` — e 400.000.000 é exatamente
20.000 linhas × dia 20.000, que é a assinatura do defeito. Sem o defeito, ela
publica.

### Como refazer

```bash
cargo build --release --examples -p phxsql-store   # a regra do binário velho
service mysql start
python3 bancada/comparacao/medir.py                # ~15 min
python3 bancada/comparacao/grafico.py
```

'''
p.write_text(s.replace(alvo, secao + alvo), encoding="utf-8")
print("DESEMPENHO.md: secao 13")
