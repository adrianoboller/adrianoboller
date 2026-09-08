# Acelerador de tabela em memoria (o `.tbm`) — o que ja existe, e o que a medicao decidiu

> **Status: AVALIADO, e o `.tbm` como PROPOSTO esta RECUSADO com numero
> (08/09/2026).** Este documento e a proposta medida ANTES de virar codigo, no
> molde do `CIFRA-DO-FIO.md` e do `VETORES.md`. O acelerador que o `.tbm`
> descreve **ja existe** — a `TabelaMemoria`/`SelectMemory`, **87x medido**. O
> arquivo `.tbm` novo nao se constroi. Sobram duas melhorias medidas e baratas,
> e uma alavanca colunar que espera **um** numero.

## 0. A lei que rege esta avaliacao

A ideia veio de fora: um "Memory Table Accelerator" com um arquivo `.tbm`
otimizado (dados compactados, dicionarios, zonas min-max), LSN, checkpoint,
dirty pages e MVCC, para acelerar SELECT/filtro/agregacao/join repetidos.
Receita de fora **se mede contra o nosso gargalo antes de virar plano**. Quatro
frentes mediram — tres DBAs (formato, concorrencia, o numero) e o pesquisador —
e convergiram. O que passa no crivo entra; o que nao passa fica de fora, medido.

## 1. O veredito, primeiro

O `.tbm` **como proposto** — um arquivo/imagem que "evita buscar paginas
repetidamente no SSD" — esta **recusado, com numero**. Cinco fatos medidos o
matam:

1. **Disco nao e tocado.** A carga analitica desta casa e de **CPU, nao de
   disco**: a §1 do `DESEMPENHO.md` mediu **0,0 MiB lidos** numa carga de 10
   milhoes (98% CPU). A pagina que o `.tbm` evitaria **ja esta em RAM**, no cache
   do nucleo. Nao ha SSD no caminho quente para um acelerador evitar.
2. **O `.ndx` e ignorado.** O caminho da agregacao (`op_pivotar` ->
   `pivot::cruzar`) **varre o `.reg` pela ordem de digitacao** (`proximo_ativo`)
   e **nunca desce o `.ndx`**. Medido, com controle nos dois sentidos: `varrer`
   inteiro = **0** toques no cache do `.ndx`; 5 `buscar` por chave = **16** (e ha
   `assert` que falha se der 0 — guarda de cegueira). O cache que comprou o
   **2,40x** e do `.ndx`; **esta consulta nao o toca**.
3. **O acelerador ja existe, e e melhor que o `.tbm`.** A `TabelaMemoria` /
   `SelectMemory` (`crates/phxsql-store/src/memoria.rs`) guarda as linhas **ja
   decodificadas** em RAM (`Vec<Option<Linha>>`) com mapas de igualdade por
   coluna — **87x medido** (`DESEMPENHO.md` §3). Um `.tbm` que guardasse
   **paginas codificadas** em RAM seria **estritamente pior**: continuaria pagando
   os ~31% de decode que a `TabelaMemoria` ja nao paga.
4. **Sem MVCC.** Um cache de leitura nao tem versao propria a manter — na escrita
   ele se invalida. O proprio `CachePaginas` do `.ndx` ja e cache residente **com
   pagina suja e checkpoint, zero MVCC**, coerente por escopo de trava + descarga.
   Ver §7.
5. **Sem mudanca de formato.** A `TabelaMemoria` e imagem em RAM, nao arquivo. Um
   `.tbm` *persistido em disco* seria so um derivado regeneravel (cabecalho +
   carimbo de posicao + «divergiu => descarta e reconstroi»), e se vale a pena e
   **pergunta de bancada**, nao de formato — economiza uma unica varredura de
   arranque.

## 2. Onde vai o tempo da consulta analitica citada — medido (DBA-3)

A consulta `SELECT Categoria, SUM(Estoque*Preco) FROM Produtos WHERE Ativo=TRUE
GROUP BY Categoria`, medida pelo `crates/phxsql-store/examples/onde-doi-na-agregacao.rs`
(pela porta `bancada/esta-medindo.sh`, binario recompilado, 3 reps por tamanho):

| tamanho | (a) leitura do slot + CRC | (b) decode em `Vec<Value>` | (c) filtro + agregacao | total (us/linha) |
|---|---:|---:|---:|---:|
| 20.000 | 0,72 (~64%) | 0,34 (~30%) | 0,065 (~6%) | ~1,10 |
| 200.000 | 0,73 (~64%) | 0,34 (~30%) | 0,070 (~6%) | ~1,13 |
| 1.000.000 | 0,73 (~62%) | 0,37 (~31%) | 0,080 (~7%) | ~1,18 |

E a honestidade que o medidor carrega escrita:

- **(a) nao e "o SSD".** E `seek`+`read_exact` do slot + um `vec![0u8; slot]` por
  linha (`reg.rs:1844`) + o CRC-32 do slot (~0,05 us de ~80 B). O disco nao entra
  — o dado vem do cache do nucleo.
- **Frio de DISCO e NAO MEDIDO** (sem root para `drop_caches`; com 16 GiB de RAM
  o `.reg` fica residente). O "frio" da tabela reaberta (+12% a +33%) e
  **aquecimento de CPU/alocador**, coerente com os 0,0 MiB lidos da §1.

## 3. O que da proposta JA EXISTE aqui (DBA-1, J)

| peca do `.tbm` | ja existe | numero | onde |
|---|---|---|---|
| paginas residentes do indice | cache de paginas do `.ndx` (CLOCK, teto 2.048 pag = 8 MiB) | **2,40x** | `DESEMPENHO.md` §1; `recursos.cache_paginas` |
| tabela residente com mapa por coluna | `TabelaMemoria` / `SelectMemory` (linhas decodificadas em RAM, mapa de igualdade) | **87x** o disco | `memoria.rs`; `servidor.rs:16483` |
| join lendo o lado pequeno inteiro em RAM | `juntar` = hash join, `HashMap` em RAM, com `TETO_JUNCAO` recusando lookup grande demais | — | `pivot.rs` / `juncao.rs` |
| leitores concorrentes sem se esperar | `RwLock` (pedido 187) | **3,8x–3,9x** de escala de leitura | `CONCORRENCIA.md` §16.6 |

Traduzindo: "paginas residentes" e "estatisticas por coluna para igualdade" **ja
estao entregues e medidas**. O buffer pool do InnoDB e o chunk/row cache do
Cassandra sao exatamente essas pecas — nao ha alavanca nova ai.

## 4. O que esta RECUSADO com numero (J, `VETORES.md` §0, `GPU.md` §7)

- **Column store como segundo motor de armazenamento** — recusado por ora,
  decidido e provado: «seriamos outro banco, e somos row-store». O `.tbm` como
  formato colunar em disco **e** esse segundo motor por outro nome, e o preco esta
  no fonte: achatar `Vec<Value>` -> colunar plano custa **uma passada inteira pela
  memoria — mais do que a varredura que aceleraria** (`GPU.md` §7).
- **SIMD/vetorizar o laco de `SUM`** — quase morto contra o nosso gargalo, que e
  **banda de memoria**, nao conta: `SUM` ja anda a **1,79x o pico do PCIe 3.0
  x16** (acima do teto de leitura da RAM), e a varredura em 4 nucleos rende
  **1,8x, nao 4x**. Vetorizar acelera a conta; aqui nao e a conta que doi.

## 5. As duas melhorias medidas e baratas — sem formato novo (DBA-3)

Estas duas o proprio numero pediu, e nenhuma toca o formato em disco:

1. **O `op_pivotar` le cada linha DUAS vezes** (`servidor.rs:9273`): faz
   `t.varrer()` (decodifica tudo), **descarta as linhas guardando so os rowids**,
   e depois re-`ler`+re-decodifica cada uma. Paga (a)+(b) ~2x. Tira-se o segundo
   decode e o caminho do pivot cai perto da metade — **de graca**, com prova real
   nos dois sentidos.
2. **`BufReader` sobre o `.reg` no `varrer`** amortiza a `syscall`+alocacao que
   dominam os ~62% de (a). O motor **ja usa `BufReader`** noutro caminho
   (`reg.rs:2358`) — e estender ao `varrer` nao muda um byte em disco.

## 6. A unica alavanca nova, e o numero que ela ainda espera (J)

Um **cache colunar plano DENTRO do `SelectMemory`** — nao um segundo motor em
disco: arranjos contiguos por coluna (em vez de `Vec<Value>` linha a linha),
**min-max zone map** por bloco (hoje o `SelectMemory` so tem mapa de
**igualdade**; min-max da o pulo de bloco em `<`, `>`, `BETWEEN`), e dicionario
so para coluna de dominio fechado. O laco autovetorizado entra **de graca por
cima** de dado plano e contiguo. A prova de que o ganho e do **layout**, nao do
SIMD, ja existe de lado: ordenar 1M de linhas `Value` custa **213,8 ms**; as
mesmas chaves como `u64` plano, **19,9 ms — 10,7x**.

**Tres premissas, e nenhuma esta fechada** — por isso isto NAO vira codigo hoje:

1. **Quanto do vao de banda o layout plano recupera no caminho do FILTRO.** A
   varredura+filtro anda a ~3.468 MiB/s contra um teto de leitura de ~24.047 —
   um vao de ~7x. O `ORDER BY` provou o vao para a **ordenacao**; o **filtro/scan
   precisa do proprio numero**, na maquina parada. Se recuperar pouco, o item
   morre: a varredura ja e ~8x o MySQL, e talvez nao haja dor.
2. **A construcao amortiza.** Achatar custa mais que uma varredura; so paga se o
   **mesmo conjunto quente** for consultado **muitas vezes** entre uma carga e a
   proxima — e isso e a carga real do dono, nao esta maquina.
3. **Dicionario so se a base pedir** — colunas de dominio fechado (<=255 valores)
   somando >10% da largura do slot, numa base real (`SPRINTS-TERADATA.md`).

## 7. Concorrencia: sem MVCC, invalidar na escrita basta (DBA-2)

A Sombra (MVCC) foi medida e **nao compra velocidade no mundo padrao** (`por_lote`,
~1,00x); ela compra **leitura repetivel**, que e correcao. Um acelerador de
leitura nao precisa disso: a coerencia sai de **invalidar/reescrever na escrita**,
exatamente como o `CachePaginas` do `.ndx` ja faz (pagina suja, `sync_all` antes
de soltar a trava, CRC conferido do arquivo e nao do cache). O `.reg` continua a
fonte unica; a janela de conflito `"versao"` (pedido 123) resolve a escrita
concorrente. Amarrar um cache de leitura a um gestor de versoes seria pagar o
preco do MVCC — e arriscar **desfazer** o paralelismo de leitura que o `RwLock`
acabou de comprar (`SOMBRA.md` §3.4) — por um ganho que morreu medido.

## 8. A ordem, entao

1. Este documento (feito), e o medidor `onde-doi-na-agregacao.rs` que o sustenta.
2. As **duas melhorias baratas** da §5 (double-read do pivot; `BufReader` no
   `varrer`), com prova real nos dois sentidos — frente B, sem formato novo.
3. **So se um caso real pedir analitico pesado**: medir a **premissa 1** da §6 na
   maquina parada, e so entao o **cache colunar leve dentro do `SelectMemory`** —
   nunca um segundo motor, nunca um `.tbm` de paginas codificadas.

## Achado lateral (papel H — documentacao que mente)

O parametro `mapear` do `memoria_carregar` esta descrito no catalogo
(`catalogo.rs:1080`) como boolean que "mapeia o arquivo em vez de copiar", mas a
implementacao o le como **lista de colunas** para montar mapas de igualdade
(`servidor.rs:16360`), e **nao existe `mmap`** no repositorio. Configuracao/ajuda
que promete um mecanismo que nao existe e erra o tipo — corrigir a redacao para
casar com o que o codigo faz.
