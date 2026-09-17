# Cognição: dois contadores da mesma inserção, e só um é consumido depois da recusa

## 1. O que aconteceu

Revisando as garantias de dado da replicação (papel C, ordem do dono de
17/09/2026 02:27 UTC), fui conferir a promessa «a réplica reproduz o source
linha por linha, com os mesmos rowids, sem transmitir rowid nenhum»
(`docs/REPLICACAO.md` §5). Ela vale. O que não vale é a promessa irmã, que
ninguém escreveu mas todo mundo lê junto: **mesmo `rownum`**.

Uma inserção **recusada** no source queima um `rownum` e não grava evento no
`.log`. A réplica nunca vê a recusa, o contador dela não anda, e — porque o
`inserir` **sobrescreve** o `rownum` que veio na imagem pelo valor do contador
local (`table.rs:3074` → `table.rs:2374-2376`) — toda linha seguinte fica com um
número de ordem diferente do source. Para sempre.

Os dois contadores da mesma inserção têm pontos de consumo **em lados opostos**
do portão que recusa:

| contador | consumido em | antes ou depois da recusa da unicidade (`table.rs:3107-3115`)? |
|---|---|---|
| `rownum` | `table.rs:3074` | **antes** |
| rowid (slot do `.reg`) | `table.rs:3122-3127` | **depois** |

E o fail-stop da réplica confere **o de depois**: `aplicar_evento` compara o
rowid e para se não bater (`table.rs:4025-4030`). O rowid não diverge. A guarda
existe, funciona, e passa ao lado da divergência.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, e a segunda mudou a recomendação inteira.**

**Primeiro erro.** Concluí que `rownum` e rowid andam juntos, porque «o `.reg`
nunca reaproveita slot e o rowid é sempre `slot_count + 1`» — se o rowid bate, a
ordem de chegada bate. É plausível e é falso: os dois são contadores
**diferentes** (o `rownum` tem contador próprio de propósito, para não gastar a
única vaga de `Sequence` da tabela — `FORMATO.md:496-498`), e ninguém garantiu
que os dois fossem consumidos no mesmo ponto do fluxo. Não foram.

**Segundo erro, e o que mais importa.** Concluí que a bancada não poderia pegar
isso, porque o retrato SHA-256 hasheia «a linha» e o `rownum` é coluna de
sistema — imaginei que o `varrer` a esconderia. Fui ver:
`linha_para_json` percorre **todas** as colunas do esquema
(`valores.rs:1062-1071`), e `Schema::new` **sempre** acrescenta `softdeleted` e
`rownum` (`schema.rs:557-586`). **O `rownum` está no retrato.**

Isso inverte o conserto. Eu ia recomendar uma comparação nova; o que falta é
**uma linha na carga da bancada** — uma inserção repetida no meio da semeadura.
O aparelho de medição estava certo e completo; a carga é que nunca pisou no
caso. Errar isso me faria propor ferramenta nova para um buraco de carga.

## 3. O que a medição disse

Medido em 17/09/2026, binário isolado fora do repositório (deps por caminho,
`CARGO_TARGET_DIR` próprio, para não disputar a trava com as frentes paralelas).
Três linhas boas, **uma** inserção recusada por chave duplicada, duas linhas
boas, e tudo replicado por `diario_com_imagem` + `aplicar_evento`:

```
a recusa: [SP000020] chave duplicada: indice unico porId ja tem essa chave
rownum_atual do source depois da recusa: 5
eventos no diario do source: 5

  rowid |  id | rownum source | rownum replica
      1 |   1 |             1 |              1
      2 |   2 |             2 |              2
      3 |   3 |             3 |              3
      4 |   4 |             5 |              4  <<< DIVERGIU
      5 |   5 |             6 |              5  <<< DIVERGIU

rowids iguais: true
linhas com rownum DIFERENTE: 2 de 5
```

**5 eventos no diário para 6 números de ordem gastos.** Uma recusa, um furo, e
`rowids iguais: true` — a guarda olhando para o lado certo e vendo o campo
errado.

E o número que diz por que isso importa além do cosmético: na **partição
alfanumérica** o `rownum` é o **único** monotônico que sobra, porque o rowid
passa a dizer em que arquivo a linha mora e não quando ela chegou
(`FORMATO.md:1467-1476`). Ali um `rownum` deslocado faz o cursor
`desde_rownum` responder a linha errada.

## 4. A regra

**Quando uma guarda confere um contador, procure os OUTROS contadores da mesma
operação e pergunte de que lado do portão de recusa cada um é consumido.**
Contador consumido antes da recusa fura em silêncio; o consumido depois é o que
a guarda vê.

E o corolário, que é do papel F: **aparelho de medição completo com carga que
não pisa no caso prova o caso que a carga tem, não o que o nome promete.**
Antes de construir comparação nova, confira se a que existe já cobre o campo — e
o que falta é a carga.

## 5. Como está guardado hoje

**Não está**, e o buraco fica nomeado em três lugares:

- **Parecer:** `docs/propostas/parecer-dba-replicacao-2026-09-17.md` §2.1 (a
  garantia que não vale, com o cenário e a medição) e §3 (a tabela de oito itens
  que a bateria deveria exercitar, o `rownum` é o item 1 — «custa uma inserção
  repetida»).
- **Guarda:** nenhuma. Não há teste, no repositório inteiro, que ligue inserção
  recusada a `rownum` — `grep -rn rownum crates/phxsql-store/tests/` acha cinco
  arquivos e **nenhum** fala de recusa. O teste que mais perto chega,
  `a_replica_reproduz_o_source_linha_por_linha`
  (`crates/phxsql-store/tests/replicacao.rs:83`), tem o `rownum` no esquema
  (o `Schema::new` o acrescenta) e **nunca falha uma inserção**.
- **Conserto:** **não feito, e de propósito.** As duas saídas que não quebram
  pétrea — consumir o `rownum` depois das conferências, ou a réplica honrar o da
  imagem — mudam o significado de `numerar_linha` e tocam o bidirecional, onde o
  `rownum` é local por desenho (`servidor.rs:4222-4224`): é decisão do dono, e
  está na lista do §6 do parecer. A terceira saída, «devolver o contador na
  recusa», está **recusada com o motivo** no §5 do parecer (NÃO 1): número de
  ordem devolvido é slot reaproveitado com outro nome, e a ordem de digitação é
  sagrada.
