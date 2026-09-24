# Tirar a memória do `core`: o filtro, e não o `PR_SET_DUMPABLE` — que tira do servidor o próprio `/proc/self/io`

**Estado:** PENDENTE

*24/09/2026, 15:45 — pedido 504.*

## 1. O que aconteceu

A H5 do pedido 451 aborta o processo de propósito, e o pedido 504 perguntava se
o `core` desse `abort` leva a chave do cofre. Medido neste contêiner:
`core_pattern` = `core`, `ulimit -c` mole 0 e duro `unlimited`,
`coredump_filter` de fábrica `00000033`. Com o limite aberto (o caso de quem
depura), o `phxsqld` com o cofre ligado recebeu `kill -ABRT` depois de derivar
a chave de um volume de verdade.

## 2. O que eu concluí primeiro, e estava errado

- **«O limite mole 0 já protege.»** Protege por acaso: é o padrão do
  contêiner, e não do produto. Basta um `ulimit -c unlimited` para o `core`
  sair com tudo.
- **«`prctl(PR_SET_DUMPABLE, 0)` é o conserto limpo: cala o `core` inteiro.»**
  Cala, e muda o dono de `/proc/self/*` para root. O servidor que roda como
  usuário próprio perde o `/proc/self/io` (modo 0400) — de onde a telemetria
  lê os bytes de disco do processo.

## 3. O que a medição disse

- Antes: `core` de **14.585.856 bytes**, com a senha do cofre **3 vezes**.
- Com `0` em `/proc/self/coredump_filter` no início do `main`: `core` de
  **61.440 bytes**, a senha **0 vezes**.
- `PR_SET_DUMPABLE`, como uid 65534: `/proc/self/io` abre antes do `prctl` e
  responde **`EACCES`** depois.

## 4. A regra

Para tirar segredo do `core`, filtre as regiões (`coredump_filter`) no começo
do `main`, antes de ler o `config.json`; não troque o dono do `/proc` do
próprio processo para isso.

## 5. Como está guardado hoje

- `crates/phxsql-server/src/main.rs`, `tirar_a_memoria_do_core`.
- Prova: `crates/phxsql-server/tests/core-sem-segredo.rs` (filtro do processo
  vivo e a senha contada no `core`); guarda `core-leva-a-senha-do-cofre`.
- `docs/SEGURANCA.md` §27.
- **O que ficou de fora:** os registradores de cada thread continuam no
  `core` (inclusive os vetoriais), e os outros binários não mudaram.
