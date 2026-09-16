# Chutar a tomada — `SIGKILL` onde ninguém varria

Pedido do dono, literal: *«O teste de chutar a tomada 🔌 é importante numa
transaction e num bulk-insert.»*

```bash
cargo build --release
python3 bancada/tomada/chutar-a-tomada.py            # ~15–25 min, grava resultados.json
PHX_TOMADA_RAPIDO=1 python3 bancada/tomada/chutar-a-tomada.py   # depuração, não publica
```

Sobe um `phxsqld` próprio na porta **7610** (`PHX_TOMADA_PORTA`), mata-o com
`SIGKILL` centenas de vezes — só o PID que ela mesma subiu, nunca `pkill` — e
reabre o banco depois de cada queda. Não mede tempo: mede **desfecho**. As
calibrações que imprime só decidem a faixa dos atrasos.

| arquivo | o que é |
|---|---|
| `chutar-a-tomada.py` | a bancada: cinco pontos, cada um com o resultado esperado escrito **antes** de rodar (`ESPERADO`), uma varredura de dezenas de atrasos e três rodadas por atraso |
| `resultados.json` | a última corrida completa, crua: data/hora UTC, versão e idade do binário, ponto × atrasos × desfechos, as corridas inválidas com o log, a contagem de `fsync` |

## O que a casa já tinha, e por que isto é outra coisa

`bancada/transacoes/provar.py` §7 mata no meio de **um** COMMIT.
`bancada/acid/` varre atrasos no meio de um COMMIT de duas tabelas (A2) e no
meio da cascata (A4), e conta `fsync` (D). `bancada/exclusao/` mata no meio da
janela de durabilidade da exclusão. Nenhuma chuta a tomada nestes pontos:

| ponto | onde a tomada cai | o que o código promete (lido antes de afirmar) |
|---|---|---|
| **1. `tx_aberta`** | dentro de uma transação aberta — 150 `inserir`, 20 `atualizar`, 10 `excluir`, `SAVEPOINT`, 100 `inserir`, `ROLLBACK TO`, 50 `inserir` — e **nunca** COMMIT | nada vai a disco antes do COMMIT (`transacao.rs` só toca o disco em `gravar_marca`). Zero rastro: contagem, slots e linhas iguais às de antes do BEGIN, nenhuma marca `.tx`, arranque calado |
| **2. `bulk`** | antes da reserva, no meio de 2.000 `inserir` reservados, e depois do `bulkinsert(false)` | a reserva mora na **memória** (`Cargas`); o que ela compra é a janela de durabilidade aberta. Cada `inserir` escreve `.reg` e `.ndx` na hora, sem `fsync`. Sob queda de **processo**: as confirmadas ficam (o `write` já foi ao núcleo) mais no máximo uma em voo; o `.ndx` fica com a marca de sujo e recusa até `reindexar`; o arranque **não diz nada**, porque não há marca `.tx` |
| **2c. `reindexar`** | no meio do `reindexar` — o conserto que a queda do ponto 2 exige | `NdxFile::criar` trunca e grava cabeçalho **limpo** com árvore vazia; depois varre o `.reg` inteiro; só então grava páginas (que levantam a marca de sujo). **Hipótese escrita antes de medir:** uma queda entre o `criar` e a primeira página deixa índice vazio e limpo — `buscar` cala, só `verificar` acusa |
| **3. `bulk_em_tx`** | BEGIN, reserva, 800 `inserir`, COMMIT, `bulkinsert(false)` | `op_bulkinsert` não olha a transação da sessão: deve aceitar. Registros 0 ou 800, nunca no meio. **Hipótese:** `bulkinsert(false)` sincroniza a tabela mas não drena `marcas_pendentes`, e a marca de um commit já durável fica no disco |
| **4. `apos_ok`** | logo depois do «ok» do COMMIT e do «ok» do `bulkinsert(false)` | tudo lá. É o D do ACID como controle positivo; a contagem de `fsync` antes do «ok» (por `strace` anexado ao PID) separa os dois: o COMMIT sincroniza a marca `.tx` e não o `.reg`/`.ndx`; o `bulkinsert(false)` sincroniza `.reg` e `.ndx` e marca nenhuma |

Só `fsync` é rastreado: nenhum lugar do motor chama `sync_data`
(`grep -rn sync_data crates/*/src` vazio), então `fdatasync` não acontece.

## O que a tomada NÃO prova, de propósito

**Queda de energia.** `SIGKILL` mata o processo; o núcleo fica com as páginas
sujas, e um `write` já entregue sobrevive. As linhas confirmadas no meio de um
BULKINSERT voltam depois da queda **e isso não é durabilidade** — é o cache do
núcleo. Quem mede durabilidade aqui é a contagem de `fsync` do ponto 4; o
`SIGKILL` prova o que a marca `.tx` e a marca de sujo do `.ndx` decidem.

**A reserva "solta" depois da queda** é verdade por construção — ela nunca
esteve no disco. A bancada a mede (`cargas` → 0) para que a afirmação não
fique sem número, mas o que protege a reserva contra cliente caído é a saída
da conexão e o prazo, provados em `bancada/carga/bulkinsert.py` e na guarda
`reserva-sobrevive-a-queda-da-ligacao`.

## Portão, vizinhos, reuso

Consulta `bancada/esta-medindo.sh` e **espera** (até 20 min, de 30 em 30 s)
enquanto houver bancada de tempo em curso: esta não mede tempo, mas a outra
mede, e cada tomada chutada aqui é carga lá. Compilação em curso é registrada
no resultado e não bloqueia. O estado do portão vai para `resultados.json`.

`subir`, `matar_de_verdade`, `Ligacao`, `ler_relatorio` e `marcas` vêm de
`bancada/durabilidade/prova.py`; o anexador de `strace` e o leitor de
`fsync` partidos em duas linhas vêm de `prova-do-fecho.py`. A armadilha do
reuso continua a mesma do ACID: `dur.Ligacao` fixa a porta no *default* do
`__init__`, então a classe é rebindada no módulo.

## As guardas (papel G), provadas RED→GREEN pelo `provar-guardas.py --so`

| id | família | o defeito reposto | quem cai |
|---|---|---|---|
| `recuperacao-deixa-a-marca-orfa` | transação | o arranque completa/descarta a marca `.tx` e a **deixa no disco** | `marca_que_nao_confere_e_commit_que_nunca_comecou` — e `a_recuperacao_completa_o_commit_e_nao_duplica` **segue verde**, porque a reaplicação é idempotente: a contagem não vê a órfã, só o disco vê |
| `recuperacao-nao-completa-o-commit` | transação | a marca válida é contada e apagada **sem completar** o commit | `a_recuperacao_completa_o_commit_e_nao_duplica` |
| `ndx-queda-com-cabecalho-limpo` | bulk | a marca de sujo do `.ndx` fica só em RAM; a queda deixa o índice atrasado **em silêncio** | `a_queda_sem_sincronizar_e_detectada_e_nao_silenciosa`, `a_marca_sai_depois_das_paginas_e_nao_antes` |
| `reserva-sobrevive-a-queda-da-ligacao` | bulk | a saída da conexão não solta a reserva | `testes_bulkinsert::a_queda_da_conexao_solta` |

O que **só o processo morto de verdade** pega — o índice que fica para trás
numa carga, a marca pendente, o arranque calado — está nesta bancada, não no
catálogo: é a mesma divisão que `recuperar-sem-reindexar` já registra com
`espera: "nada muda"`.

## Resultado da corrida de 16/09/2026 (07:36 UTC, 672,6 s)

Tudo abaixo saiu de `resultados.json` (gerado; não se edita). Binário
`phxsqld 0.18.0 (29a333fe2aee-sujo)`, compilado 07:26:23 UTC, SHA-256
`fe28f652a741aec4…`, congelado numa cópia para a corrida inteira porque havia
12 arquivos sujos em `crates/` de frentes vizinhas e um `cargo build
--release` em curso. O portão foi obedecido: **esperou 694 s** pela bancada
CRUD da colmeia antes de subir o primeiro servidor.

| ponto | atrasos × rodadas | calibração | desfechos | inválidas |
|---|---|---|---|---|
| `tx_aberta` | 24 × 3 = 72 | 72,3 ms | LOGO_APOS_O_BEGIN 3 · NO_MEIO_DOS_INSERTS 24 · NO_MEIO_DE_ATUALIZAR_EXCLUIR 5 · ENTRE_SAVEPOINT_E_ROLLBACK_TO 18 · DEPOIS_DO_ROLLBACK_TO 10 · TODAS_AS_OPS_E_SEM_COMMIT 12 | **0** — byte 52 = 0 em 72 |
| `bulk` (linha a linha) | 24 × 3 = 72 | 1.071,1 ms | ANTES_DA_RESERVA 3 · NO_MEIO_INDICE_LIMPO 17 · NO_MEIO_INDICE_SUJO 1 · APOS_O_OK_FINAL 51 | **0** — byte 52 = 1 em 1 (`bulk#1.5`, 244,5 ms: 1.712 confirmadas, 1.713 no `.reg` — a linha em voo) |
| `bulk_lote` (4 × `inserir_lote` de 500) | 24 × 3 = 72 | 27,9 ms | ANTES_DA_RESERVA 3 · NO_MEIO_INDICE_LIMPO 27 · NO_MEIO_INDICE_SUJO 35 · APOS_O_OK_FINAL 7 | **0** — byte 52 = 1 em 35, e as 35 recusadas pelo `buscar` |
| `reindexar` | 40 × 3 = 120 | 10,3 ms | INTEIRO_ANTES_DE_COMECAR 3 · SUJO_DETECTADO 111 · INTEIRO_DEPOIS_DE_TERMINAR 6 | **0** — byte 52 = 1 em 111 |
| `bulk_em_tx` | — | — | **recusado**: `[SP000018] bulkinsert nao entra em transacao: esta rodada empilha inserir, atualizar, excluir, restaurar. E ela NAO confirma a transacao aberta por conta propria -- termine com COMMIT ou ROLLBACK e repita` | — |
| `tx_em_bulk` (reserva, depois BEGIN) | 24 × 3 = 72 | 334,6 ms | sem marca (antes do COMMIT) 68 · P3 parcial completado 2 (768+32 e 506+294, `indices_reconstruidos 1`) · APOS_COMMIT_OK_ANTES_DE_SOLTAR 1 · APOS_BULKINSERT_FALSE_MARCA_REPORTADA 1 | **0** — nunca metade |
| `apos_ok` | 3 + 3 | — | COMMIT: 500/500/500, marca pendente reportada (`achadas 1, ja aplicadas 500`) · BULKINSERT(false): 500/500/500, índice limpo, arranque calado | **0** |

`fsync` antes do «ok», por `strace 6.8` anexado ao PID: **COMMIT = `{tx: 1}`**
(nem `.reg` nem `.ndx`: a marca é o bilhete); **`bulkinsert(false)` =
`{reg: 1, ndx: 2, trash: 1, bin: 1, memo: 1, log: 1, reason: 1}`** e marca
nenhuma. 22 conferências, 0 falhas.

### As duas hipóteses, julgadas

**«Queda entre o `criar()` e a primeira página deixa índice vazio e limpo» —
MORTA.** 111 quedas no meio do `reindexar`, **111 com byte 52 = 1**. O
`NdxFile::criar` grava a página-raiz por `gravar_pagina`, que levanta a
marca antes de a árvore existir; não há janela limpa. O que a passada rápida
tinha «confirmado» era o instrumento: ver a cognição do recorte, abaixo.

**«A marca do COMMIT feito na tabela reservada fica pendente e
`bulkinsert(false)` não a drena» — CONFIRMADA.** Sem queda nenhuma:
`transacao_1789545411721.tx` no disco depois do COMMIT, depois do
`bulkinsert(false)` (que já fez os 8 `fsync`) e 300 ms depois. Com a tomada
chutada depois do «ok» final, o arranque reporta `achadas 1 / ja aplicadas
800` para um commit que já era durável. Não perde nem duplica linha
(reaplicação idempotente pelo rowid); é ruído de relatório e uma marca que
sobrevive a mais do que devia. `op_bulkinsert(false)` chama `t.sincronizar()`
e tira a tabela de `sujas`, mas quem drena `marcas_pendentes` é o
`descarregar_sujas`, que ele não chama. Achado para o papel B.

### O que a tomada ensinou sobre o BULKINSERT

* Com `inserir` linha a linha o servidor abre e fecha a tabela **a cada
  pedido**, e o `fechar` leva as páginas do `.ndx` ao núcleo: em 72 quedas só
  **1** caiu dentro de um pedido com a marca levantada. Com `inserir_lote` a
  tabela fica aberta o lote inteiro e a marca apareceu em **35 de 72**.
* Queda no meio de um lote deixa o lote **parcial** no `.reg` (fora de
  transação ele não é atômico) e o `.ndx` sujo. A tabela **recusa** toda
  operação de índice com «reconstrua com `reparar indice`» até um `reindexar`
  manual — e **o arranque não diz nada**, porque não há marca `.tx`. Nunca em
  silêncio para o cliente; em silêncio para quem opera. Se isso deve virar
  varredura de byte 52 no arranque é decisão de desenho (custa ler o cabeçalho
  de toda tabela ao subir), registrada aqui e não decidida aqui.
* O mesmo vale para a tomada no meio do próprio `reindexar`: sujo, detectado,
  recusando até o próximo `reindexar` — que completa (120 de 120 acharam as
  10.000 chaves depois).

### O que o instrumento ensinou (e errou primeiro)

* **Recorte antes do casamento cega o instrumento.** `buscar()` devolvia o erro
  cortado em 160 caracteres e `indice_sujo()` procurava «reparar indice» no
  que sobrava; o texto acabava em «nao e co». A passada rápida classificou 4
  de 6 quedas como «índice limpo e vazio EM SILÊNCIO» — o achado mais grave
  possível — e era o instrumento. Hoje o texto vai inteiro ao casador e o
  byte 52 é lido do arquivo antes de reabrir.
  `docs/cognicao/cognicao_recorte-antes-do-casamento-cega-o-instrumento_20260916_0722.md`
* **A primeira troca da guarda do `.ndx` não repunha o defeito.** Ela deixava
  `self.sujo = true` em RAM e só tirava o `gravar_cabecalho()`; o executor
  respondeu NAO PEGOU, porque o cabeçalho é regravado a cada `inserir` (o
  `qtd_chaves`) e carrega o `sujo` da memória — a marca chegava ao disco por
  outra escrita. Repor de verdade é ninguém levantar a marca: 2/2 caíram.
  `docs/cognicao/cognicao_defeito-reposto-que-nao-repoe_20260916_0801.md`

Achado é achado, não vergonha: corrida inválida vai para
`pontos.<nome>.invalidas` com o atraso, a classe, o progresso e o relatório
do arranque. Nesta corrida a lista está vazia nos seis pontos.
