# A recusa só queima o contador dentro do lote — e foi o cenário certo que faltava

*17/09/2026, 02:58 — a hora da descoberta, quando a quarta variante do estágio
`rownum` divergiu depois de as três primeiras não divergirem.*

## 1. O que aconteceu

O papel C (DBA) achou, **lendo o código**, que uma inserção recusada no source
queima um `rownum` sem gerar evento: `numerar_linha` consome o contador
(`table.rs:2361-2377` → `reg.rs:764-768`) antes da conferência de unicidade
(`table.rs:3100-3115`), e o evento do diário só é gravado no fim. A réplica
gera o `rownum` dela localmente, então os dois contadores se descolam — e o
retrato SHA-256 da bancada, que inclui o `rownum`, acusaria.

Escrevi a prova pelo soquete (`bancada/replicacao/achados-do-dba.py`), com
controle: 3 linhas, **uma inserção recusada por chave duplicada**, 2 linhas, e
o mesmo roteiro sem a recusa. A recusa aconteceu — `[SP000020] chave duplicada:
indice unico porId ja tem essa chave` — e o `rownum` **não divergiu**. Nem com
recusa por único secundário. Nem com recusa por coluna obrigatória.

Divergiu na quarta variante: a recusa **dentro de um `inserir_lote` com
`parar_no_erro: false`**.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o parecer estava errado — que a leitura do código tinha descrito um
caminho que o servidor não percorre, e que o achado ia para o relatório como
«hipótese que morreu medida». Já tinha a frase pronta: *três recusas em três
pontos diferentes do caminho, nenhuma queimou o contador.*

Estava errado, e o erro é o mesmo de sempre com outra roupa: eu tinha medido o
**cenário**, não o **mecanismo**. O contador que o parecer aponta é
`Reg::proximo_rownum`, que é estado **da instância aberta da tabela**. Entre
duas operações o servidor reabre a tabela e o contador nasce de novo do disco:
a queima existe, acontece exatamente onde o parecer disse, e é **descartada**
antes de qualquer um poder vê-la. Meu roteiro tinha uma recusa e uma inserção
seguinte em **operações separadas** — o único arranjo em que o defeito não
sobrevive.

A leitura de código estava certa. O que faltava era a pergunta seguinte: *entre
a queima e a linha que herdaria o número, a tabela é reaberta?*

## 3. O que a medição disse

`achados-do-dba.py --so rownum`, 02:58 UTC, binário `target/release/phxsqld` de
02:29:

| onde a recusa aconteceu | foi recusada? | queimou `rownum`? |
|---|---|---|
| única da **primária**, operação própria | sim | **não** |
| única **secundária**, operação própria | sim | **não** |
| **coluna obrigatória** faltando, operação própria | sim | **não** |
| única da primária **dentro de um `inserir_lote`**, `parar_no_erro: false` | a linha sim (`gravadas=5`, `recusadas=1`) | **SIM** |

```
  rowid |  id | rownum source | rownum replica
      1 |   1 |             1 |              1
      2 |   2 |             2 |              2
      3 |   3 |             3 |              3
      4 |   4 |             5 |              4  <<< DIVERGIU
      5 |   5 |             6 |              5  <<< DIVERGIU
retrato SHA-256   source 252fa89db5038769   replica d92da11a064d6f11   DIFERENTE
controle, as mesmas 5 linhas sem recusa     d92da11a064d6f11 nos dois   IGUAL
```

E o alcance real é **pior** que o do cenário do parecer, não melhor:
`inserir_lote` com `parar_no_erro: false` é o caminho de **importação e carga**,
onde linha recusada é rotina. O arranjo em que o defeito não aparece é o
digitar uma linha de cada vez.

## 4. A regra

**Cenário que não reproduz não absolve o mecanismo — procure o arranjo em que o
estado sobrevive.** Quando a prova nega um achado de leitura de código, a
pergunta seguinte não é «o parecer errou?», é «qual é o tempo de vida do estado
que ele aponta, e em que arranjo ele chega até a próxima operação?».

E o corolário para quem escreve a bancada: **varra o gatilho, não escolha um.**
Quatro variantes custaram quatro minutos e a diferença entre «hipótese morta» e
«defeito confirmado com o caminho de produção nomeado».

## 5. Como está guardado hoje

Guardado como **prova que roda**: `bancada/replicacao/achados-do-dba.py`,
estágio `rownum`, com as quatro variantes e o controle sem recusa — o veredito
é `ok` quando **alguma** variante queima o contador **e** o controle bate nos
dois lados. O resultado fica em `bancada/replicacao/achados-do-dba.json`.

O que **não** está guardado, e é o buraco: a carga do `bancada/replicacao/
medir.py` continua semeando ids únicos e **nunca falha uma inserção**, então o
`iguais_no_fim` dos quatro servidores continua provando menos do que o nome
promete. É o item 1 do §3 do parecer do DBA, e é uma linha na carga. Não entrou
nesta rodada porque mexer na semeadura do `medir.py` muda o número que a página
publica, e isso é decisão do orquestrador.

O defeito do motor está nomeado, com prova, em
`docs/propostas/bateria-replicacao-2026-09-17.md` §4.1.
