# A marca do evento devido sobe só na falha, no lugar — e custa zero ao laço quente

**Estado:** PENDENTE

## O que aconteceu

Pedido 498, o resto: o `write` do `.log` que falha por disco cheio **depois**
de o `.reg` gravar deixava a linha sem evento. Contra o SO (tmpfs de 41
tamanhos, 256..896 KiB, com a imagem no diário), o executor de antes deixou
**309 linhas sem evento em 22 de 41 tamanhos**
(`bancada/catastrofes/resultados-498-antes.json`; o C2a do 496 tinha medido
294). A decisão do dono (30/09/2026) foi derrubar e completar, como o PANIC do
PostgreSQL. Faltava o arranque saber **qual** evento faltou.

## O que eu concluí primeiro, e estava errado

Que a marca tinha de subir **antes** da escrita do `.reg` e descer depois do
`anexar` (a H1 do pedido), porque «sem saber antes, o arranque não sabe o que
completar». Isso vale para o `SIGKILL` entre as duas gravações, e não para o
caso do pedido: no disco cheio o processo **sabe** que falhou, e sabe
exatamente o quê, no instante em que o `write` devolve ENOSPC. A marca pode
subir **ali**, sobrescrevendo o cabeçalho que já existe.

## O que a medição disse

- Um `pwrite` de 24 bytes no lugar custa **0,39–0,47 µs** (5 rodadas de
  200.000); a H1 pagaria dois por linha sobre **3,8 µs** da inserção sem
  índice (`--example onde-doi`) — 20% a mais para cobrir um caso que não é o
  do pedido.
- A H2 (sentinela em arquivo à parte) morre medida: no tmpfs cheio o arquivo
  novo nasce com **0 bytes** (`printf: I/O error`), e o mesmo tmpfs aceitou a
  sobrescrita de 5 bytes no meio de um arquivo existente.
- Depois do conserto, o mesmo roteiro: **0 linhas sem evento em 41
  tamanhos**, e o processo caiu pelo gancho nos **22** tamanhos em que o
  diário falhou depois do `.reg` (`bancada/catastrofes/resultados-498.json`).
- O laço quente, antes e depois intercalados (2×7 e 2×9 corridas de 50.000):
  medianas 3,9/4,3 µs contra 4,1/4,1 µs sem índice, e as faixas min–max se
  cruzam nas duas séries — nenhuma diferença medível.

## A regra

Quando o processo **sabe** que a gravação falhou, a marca de recuperação sobe
no instante da falha, sobrescrevendo bytes que já existem; subir antes de toda
escrita só se paga para cobrir a queda que o processo **não** vê.

## Como está guardado hoje

- `crates/phxsql-store/tests/diario-que-falha.rs` (cinco provas) e
  `servidor::testes_do_panico_sob_a_trava::diario_que_falha_depois_da_linha_derruba_e_a_abertura_completa`;
- sete guardas no `bancada/guardas/catalogo.py`, as sete PROVADAS em
  30/09/2026: `diario-que-falha-sem-marca-do-evento-devido`,
  `abertura-nao-completa-o-evento-devido`,
  `diario-que-falha-nao-derruba-o-servidor`,
  `disco-cheio-deixa-a-sentinela-do-509`,
  `exclusao-de-vez-sem-conferir-o-teto-do-diario`, e os dois irmãos que só
  apareceram procurando quem grava DEPOIS do `.reg` e ANTES do `.log` —
  `exclusao-de-vez-motivo-que-falha-pula-o-diario` e
  `insercao-fts-que-falha-pula-o-diario`: ali o `?` do observador pulava o
  diário sem marca e sem queda, porque o `.log` nem chegava a ser tentado;
- o roteiro contra o SO: `CENARIOS=498 bancada/catastrofes/prova.sh`.

**O buraco que ficou:** o `SIGKILL` (ou a queda de energia) **entre** o
`.reg` e o `.log` continua deixando a linha sem evento — é o caso que só a H1
compraria, a dois `pwrite` por linha. E um sistema de arquivos com cópia na
escrita (btrfs, ZFS) pode recusar até a sobrescrita no lugar: aí o processo
cai do mesmo jeito, e a mensagem diz que a marca **não** ficou.
