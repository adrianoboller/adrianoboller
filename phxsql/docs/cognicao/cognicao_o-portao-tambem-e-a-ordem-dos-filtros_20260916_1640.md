# O portão também é a ORDEM DOS FILTROS — e a parcela nomeada errado sobrevive melhor

**16/09/2026, 16:40.** Frente do pedido 259 (velocidade de IO no `excluir`),
papel B.

## 1. O que aconteceu

O diagnóstico do pedido 259 (`DESEMPENHO.md` §24.4) tinha dividido o `excluir`
em quatro parcelas e fechado a soma em 99,7%. Duas delas entraram nesta rodada
e o `excluir` caiu de **30,41 para 21,07 µs** (1,44×, mediana de 2 e de 6
corridas, mesma máquina, mesmo dia, `--example custo-do-excluir 200000 20000`).

A maior das duas não foi a que o pedido descrevia. O pedido dizia «a busca
reversa da integridade varre o diretório, 8,96 µs, e roda mesmo sem irmã nenhuma
e sem chave nenhuma» — e a conclusão natural disso é *arranjar um portão para a
busca reversa*, que esbarra na pétrea (a chave é declarada na filha; o catálogo
reverso guardado está recusado). O que a medição achou foi outra coisa: dentro
da varredura, `catalogo::tabelas_em` perguntava ao núcleo «isto é arquivo?» em
**toda** entrada do diretório e só depois olhava a extensão. Uma tabela tem oito
arquivos e só um é `.reg`: oito `statx` para jogar sete fora. O `d_type` que o
`getdents64` já trouxe responde de graça.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, e as duas por acreditar na descrição da parcela em vez de abrir o
código dela.**

A primeira: «a busca reversa custa porque varre o diretório, então ou se
arranja um portão que a evite, ou se paga». Errado pela metade — a varredura
custava o que custava porque estava mal ordenada por dentro, e metade do preço
dela nunca foi o preço da garantia. A escolha pétrea não precisou ser reaberta
para o número cair pela metade.

A segunda, e essa estava escrita no documento que eu mesmo fui usar como
partida: «o `.ndx` remover escreve página e cabeçalho na hora — o inserir passa
pelo write-back, o remover não». **O `remover` passa pelo write-back**: ele
chama `gravar_pagina`, a mesma função do `inserir`, e a página fica suja em RAM.
O que ele faz na hora é só o **cabeçalho** — e sem a condição
`if self.estrutura_mudou` que o irmão `inserir_ja_conferido`, dez linhas acima
no mesmo arquivo, tem com o motivo escrito ao lado («o contador não justifica
4 KiB por chave»).

O diagnóstico errado sobrevive bem porque o **alvo** dele estava certo: o
`.ndx` custa mesmo 25–30% do excluir. Se eu tivesse implementado «write-back no
remover» sem ler, teria descoberto que já existia — ou, pior, teria mexido no
despejo e atribuído a ele um ganho que veio do cabeçalho.

## 3. O que a medição disse

Premissa medida antes de implementar (em `std` puro, 20.000 repetições):

| | hoje | candidato | |
|---|---:|---:|---:|
| varredura, 1 tabela (8 arquivos) | 8,75 µs | 4,00 µs | 2,19× |
| varredura, 31 tabelas (248 arquivos) | 212,87 µs | 67,49 µs | 3,15× |
| por irmã: `File::open` + `metadata` + 2 `read` | 1,59 µs | — | |
| por irmã: só o `statx` | 0,50 µs | — | |

Depois, dentro do motor (N = 200.000, 20.000 exclusões):

| | antes | depois |
|---|---:|---:|
| excluir direto, tabela sozinha | 30,41 µs | **21,07 µs** |
| excluir com 30 irmãs | 438,00 µs | **283,70 µs** |
| `statx` por exclusão, sozinha | 9 | **1** |
| `statx` por exclusão, 30 irmãs | 309 | **61** |

E os dois candidatos que ficaram parados, medidos por sonda temporária e
revertidos: o critério do irmão aplicado ao `remover` do `.ndx` dá 21,07 →
**18,96 µs** (4 → 2 `write` no `.ndx`); o `cifra::sortear` no lugar do
`/dev/urandom` do `uuid.rs` dá 21,07 → **18,07 µs** (`Uuid::v7` 1,08 → 0,11 µs,
`openat` 5 → 1).

**Uma hipótese que morreu com número:** «dá para responder ‹nenhuma tabela deste
diretório declara chave estrangeira› sem a varredura». Não dá sem estado
guardado — a chave mora na filha e ninguém diz quem aponta para si, então a
resposta exige ler o esquema de cada irmã. O que dá é ler **mais barato**: a
irmã custa 13,5 µs por `RegFile::abrir` (ablação), dos quais 1,44 são
`Schema::desserializar` e ~1,6 são as chamadas de sistema — o resto é montagem
de estrutura que a pergunta «você aponta para mim?» não usa.

## 4. A regra

**Quando uma parcela cara tiver um nome, abra o código dela antes de aceitar o
nome.** O portão que falta raramente é um `if` novo na frente da função: quase
sempre é a ordem dos filtros lá dentro — o teste barato (texto, campo já na
mão, `d_type` já lido) vem antes do caro (chamada de sistema, leitura,
decodificação).

## 5. Como está guardado hoje

- `crates/phxsql-store/src/catalogo.rs`: a ordem dos filtros, com o motivo e o
  número no comentário da função.
- `crates/phxsql-store/src/table.rs`: `conferir_filhas_com`, o portão da leitura
  e as três decisões que uma refação apagaria calada (sobreposição da
  transação, coluna externa, `expect` em vez de `unwrap_or(&[])`).
- `docs/DESEMPENHO.md` §24.6: o antes/depois inteiro, as hipóteses mortas e os
  dois candidatos parados com o número.
- Teste: `a_varredura_barata_ve_o_mesmo_que_a_cara` (elo simbólico e diretório
  com nome de tabela), provado nos dois sentidos.
- **Onde o buraco ficou:** as duas provas reais desta frente ainda **não** estão
  no `bancada/guardas/catalogo.py`. Foram feitas à mão e conferidas nos dois
  sentidos, mas ficaram fora do catálogo de propósito — a frente de QA estava
  publicando a tabela das guardas no mesmo intervalo, e entrada nova no catálogo
  sem corrida completa deixaria o número publicado divergindo do catálogo. Os
  dois `trecho`/`troca` estão no relatório desta frente, prontos para entrar.
