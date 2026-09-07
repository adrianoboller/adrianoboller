# 0.5) O fluxo do auto number id e do sequence — como está e como seria o ideal

*Medido em 2026-09-07 17:39 UTC, commit `395848f`, contra o motor vivo, por
`bancada/sequencias/sonda.py` — 25 blocos, gravados em
`bancada/sequencias/resultados.json`. O documento inteiro é o `docs/AUTONUMBER.md`.*

## Resposta curta

São **três** números crescentes por tabela, e não um: o **`rowid`** (posição
física do slot, byte 20), a coluna de sistema **`rownum`** (ordem de chegada,
byte 92) e a coluna do usuário **`Sequence`** (byte 36) — os três contadores no
**mesmo** cabeçalho de 128 bytes do volume 1, regravado a cada inserção por um
`write` de **0,50 µs**, sem `fsync` (o `fsync` espera a janela: **82,1 µs**,
164× mais). O `rownum` o motor impõe e ninguém ajusta; a `Sequence` o motor só
preenche quando o valor chega nulo, e quando o cliente escolhe um número o
contador **acompanha** — `id=100` faz o próximo sair **101**, que é o oposto do
PostgreSQL.

O fluxo tem **dois buracos medidos**. Dentro de uma transação a linha inserida
se lê com `id: null` e `rownum: null` até o `COMMIT` — então a ficha
mestre-detalhe não fecha, porque não há o id da mãe para pôr na filha. E no modo
bidirecional, com os dois servidores numerando a mesma faixa, **4 inserções
deixaram 2 linhas**: os ids colidem, «mais recente vence» resolve, e o trabalho
do outro lado some em silêncio.

## Exemplo exercitado

```text
=== 12. Transacao: o ROLLBACK devolve o numero, e o COMMIT e quem numera
  o que a transacao VE da propria linha: [(1, 1, 1), (2, None, None)]
  cabecalho durante a transacao: {'proxima_sequencia': 2, 'proximo_rownum': 2, ...}
  cabecalho depois do ROLLBACK: {'proxima_sequencia': 2, 'proximo_rownum': 2, ...}
  depois do COMMIT: [(1, 1, 1), (2, 2, 2)]
```

O `ROLLBACK` **devolve** o número — o PostgreSQL não devolve. Não é opção: o
`.reg` nunca reaproveita slot, então nada vai a disco antes do `COMMIT`, e quem
numera é a passada de confirmação (`transacao.rs:1240`).

```text
=== 24. Bidirecional: os dois numerando a mesma faixa
  -- estagio 1: os dois numeram sozinhos, na MESMA faixa
    4 insercoes -> alfa ficou com 2: ['de-beta-0', 'de-beta-1']
                   beta ficou com 2: ['de-beta-0', 'de-beta-1']
  -- estagio 2: faixas DISJUNTAS por `ajustar_sequencia` (o remendo de hoje)
    alfa q: [(1, 1, 1), (2, 2, 2), (3, 1000000, 3), (4, 1000001, 4)]
    contador de alfa depois da ida e volta: {'proxima_sequencia': 1000002, ...}
  -- estagio 3: a faixa disjunta sobrevive a primeira ida e volta?
    6 insercoes -> alfa ficou com 5: ['alfa-0', 'alfa-1', 'beta-0', 'beta-1', 'beta-2']
  -- estagio 4: a MESMA prova com chave `Uuid` v7
    4 insercoes -> alfa ficou com 4: ['alfa-0', 'alfa-1', 'beta-0', 'beta-1']
                   beta ficou com 4: ['alfa-0', 'alfa-1', 'beta-0', 'beta-1']
```

O remendo das faixas disjuntas **funciona uma rodada e morre na segunda**: o
`anotar_sequencia` empurra o contador local para depois do maior valor gravado —
inclusive o que veio do outro servidor. Alfa foi ajustada para 1 e beta para
1.000.000; depois da primeira ida e volta o contador de alfa estava em
**1.000.002**, que é o próximo de beta. Com chave `Uuid` v7 nada se perde.

```text
=== 14. Particao por LETRA: o `rownum` NAO cresce com o `rowid`
  (rowid, Sequence, rownum): [(1, 2, 2), (2, 3, 3), (18001, 1, 1), (25001, 4, 4)]
  cabecalho do balde _A (o volume 1): {'proxima_sequencia': 5, 'proximo_rownum': 5, ...}

=== 16. Ajustar para TRAS SEM indice unico: repete calado
  (rowid, Sequence, rownum): [(1, 1, 1), (2, 2, 2), (3, 3, 3), (4, 1, 4)]
  ids: [1, 2, 3, 1] -- repetidos: 1

=== 19. O teto REAL do numero no protocolo: 2^53, e nao 2^64-1
  "linhas":[{"rowid":1,"id":9007199254740992,"c":"9007199254740992",...},
            {"rowid":2,"id":9007199254740992,"c":"9007199254740993",...},
            {"rowid":3,"id":9007199254740996,"c":"9007199254740995",...}]
  linhas em que o gravado DIVERGE do mandado: 2 de 3

=== 23. A promocao de uma replica ATRASADA
  a fonte morta tinha  : {'proxima_sequencia': 707, ...}
  a promovida tinha    : {'proxima_sequencia': 702, ...}
  e a insercao dela saiu: (8, 702, 8)     -> 5 numeros reemitidos
```

Quatro coisas nesses blocos: a partição por letra separa posição de chegada
(Silva com rowid 18001 e `rownum` 1); ajustar o contador para trás **sem** índice
único repete o id sem erro nenhum; o número acima de 2⁵³ se corrompe calado
porque o `Json` da casa só tem `Numero(f64)` (`json.rs:20`); e uma réplica
atrasada promovida continua de onde **ela** parou, reemitindo os cinco números
que a origem morta já tinha dado.

O que **não** move nenhum dos três: exclusão suave, restauração, exclusão física,
`BULKINSERT`, `reindexar` e `verificar`. Uma queda do processo (`SIGKILL`) também
não: o contador voltou reaberto em 8, exatamente como estava.

## O que NÃO existe, e é dispensa registrada

- **`CREATE SEQUENCE` solto**, com `nextval`/`currval`/`setval`, compartilhado
  entre tabelas — não existe, por formato: o contador é do `.reg`. É o sprint 11
  do `docs/SPRINTS-MARIADB.md`, e este documento **mediu a premissa que ele
  pedia**: um número durável em arquivo próprio custa **83,5 µs** de
  `fdatasync`, contra 0,50 µs do contador de hoje — **167×**. É por isso que o
  `CACHE` do MariaDB existe, e é por isso que ele entraria **desligado** aqui:
  para nota fiscal, buraco na numeração não é aceitável.
- **`START WITH` / `INCREMENT BY` / `MINVALUE` / `MAXVALUE` / `CYCLE`** — o passo
  é 1, o início é 1, não há teto declarado nem volta.
- **`IDENTITY … GENERATED ALWAYS`** — hoje o valor do cliente é sempre aceito e o
  contador o segue; não há como recusá-lo.
- **`auto_increment_increment` / `_offset`** (a paridade do MariaDB) — e a falta
  **custa dado** no modo bidirecional, medido acima.
- **`RETURNING id`** — o protocolo devolve o `rowid`; para saber a `Sequence`
  gerada relê-se a linha. E não há `INSERT` na camada SQL: «só `SELECT`».
- **`SELECT … WHERE id = 2` com chave `Sequence`** — recusa hoje («esperado
  numero da sequencia, recebido Texto("2")») e passa quando a chave é `Int8`. É
  o **pedido 223**, aberto, `valores.rs:707`.
- **`Uuid` que nasce sozinho** — a `Sequence` nula ganha número; o `Uuid` nulo dá
  «coluna id e obrigatoria e recebeu NULL». A assimetria não tem motivo de
  formato, e fechá-la é o que torna o `Uuid` v7 uma alternativa usável para quem
  tem dois masters.
- **Reparo do contador** — o `verificar` reconta `marcadas` varrendo e **não**
  reconta a `Sequence`; um contador atrasado não tem caminho de conserto
  automático. E o cabeçalho adulterado à mão é pego pelo **CRC-32**, que trava
  `verificar`, `reparar` e `inserir` juntos.

## Como se refaz

```bash
PHX_SONDA_PORTA=7830 PHX_SONDA_BIN=target/release/phxsqld \
  python3 bancada/sequencias/sonda.py
```
