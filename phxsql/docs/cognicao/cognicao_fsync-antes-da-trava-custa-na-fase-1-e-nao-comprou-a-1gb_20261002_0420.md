# O `fsync` antes da trava custa picos na fase 1, e a 1 GB não comprou a espera da fase 2

**Estado:** PENDENTE

## 1. O que aconteceu

Fecho do pedido 513 (passo 2a), bancada `bancada/backup/retrato-com-escritor.py`
a **1.123 MiB** (526.334 linhas, 3 voltas, 02/10/2026). A cognição de 01:20
concluiu, a 561 MiB, que mover o `fsync` do grosso para antes da trava tirava
~70 ms de espera da fase 2. A disputa de E/S da fase 1 ficou sem isolar; agora
a bancada separa as escritas pela janela `[t0, t0 + fase_1_ms]` e o mesmo
banco rodou com e sem o `sincronizar_fase_1` (binário com a chamada removida).

## 2. O que eu concluí primeiro, e estava errado

Escrevi, antes de medir, duas hipóteses: **H1** o `fsync` na fase 1 disputa
E/S com o escritor; **H2** ele devolve disco limpo a quem esperou a fase 2.
A cognição anterior tratava H2 como estabelecida (uma única comparação a 561
MiB, faixas de 3 voltas que quase se tocavam). A esperada era confirmar as
duas.

## 3. O que a medição disse

- **H1 confirmada:** máximo de uma escrita durante a fase 1, com `fsync` antes
  398 ms [370–439] (A) e 404 ms [312–1.563] (B); sem, 32 ms [11–72] e 89 ms
  [30–107]. 6 de 6 voltas de cada lado, faixas sem se cruzar.
- **H2 morreu a 1 GB:** depois da fase 1, com 1.076 ms [967–1.342] contra 995
  ms [799–1.005] (A) e 3.961 [3.567–4.042] contra 3.746 [3.711–4.294] (B):
  as faixas se cruzam, sem vencedor (pedido 155). O ganho de 561 MiB não
  reaparece.
- O `fsync` antes da trava, portanto, **troca** pico de ~0,4 s durante a
  cópia por nada medível depois dela, neste disco e neste tamanho.
- Fase 2 do cenário B: 27,6 % da cópia a 1 GB (25,4 % a 561 MiB) — o 2b não
  dispara.

## 4. A regra

Hipótese que "valeu" numa comparação com faixas coladas é palpite com
número, não evidência: refaça em outro tamanho antes de escrever a regra. E
medir a espera só no total do backup esconde em qual fase ela cai — separar
por janela de tempo foi o que mostrou o custo na fase 1.

## 5. Como está guardado hoje

Prevenção (avoid): não promover a "toda E/S fora da trava vai antes" sem a comparação por fase.
A chamada não tem guarda (desempenho; teste unitário não vê tempo): a
evidência é `bancada/backup/resultados.json`, rótulos `duas_passadas_1gb` e
`duas_passadas_1gb_sem_fsync_antes`. Decisão de manter ou reverter é do papel
C. Reprodução: `python3 bancada/backup/retrato-com-escritor.py --mb 1024`
com cada binário.
