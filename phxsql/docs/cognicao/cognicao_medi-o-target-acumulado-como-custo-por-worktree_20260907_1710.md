# Cognição: medi o `target` acumulado como se fosse o custo de UMA worktree

**Descoberta:** 07/09/2026 17:10 UTC, ao montar o time de oito frentes da
rodada das diretivas HFSQL e do fluxo do auto number.

## 1. O que aconteceu

Antes de abrir as frentes, medi o disco: **7,6 GB livres** e `target/` com
**9,8 GB** (660 MB de release, 8,9 GB de debug). Concluí que «uma worktree por
frente, cada uma com o seu build, não cabe», e planejei pôr seis frentes de
Rust a editar **a mesma árvore**, serializando o `cargo` por `flock`.

Esse plano tinha dois defeitos que só apareceriam no encontro das frentes: o
build de uma frente enxerga a edição pela metade da outra (e a frente perde
tempo «consertando» arquivo que não é dela), e duas ferramentas de edição no
mesmo arquivo — o `servidor.rs` tem 24.975 linhas e quase toda frente o toca —
podem perder uma escrita em silêncio.

## 2. O que eu concluí primeiro, e estava errado

Que **o custo de uma worktree era da ordem do `target` principal** — 9,8 GB —
e que por isso seis worktrees pediam ~60 GB. Era um número lido no `du`, e o
`du` media outra coisa: o acúmulo de **dias** de builds de teste, com o cache
incremental de dezenas de binários que ninguém apaga.

## 3. O que a medição disse

| o que | medido | comando |
|---|---|---|
| fonte rastreado (o que uma worktree copia) | **~30 MB** | worktree antiga: `du -sh .claude/worktrees/agent-…` = 29 MB |
| `target/debug/incremental` | **4,2 GB** | `du -sh target/debug/incremental` |
| `target/debug/deps` (acumulado) | 2,8 GB | `du -sh target/debug/deps` |
| livre depois de apagar o incremental | **12 GB** | `df -h /` |
| ligadores cruzados (Windows, ARM64 musl, ARM32 musl) | os três ligam | `cargo build --release --target … -p phxsql-core` |

Com `CARGO_INCREMENTAL=0` e um `target` por worktree, um build de testes do
servidor custa uma fração daquilo — e **seis worktrees cabem** com folga sobre
o piso de 2 GB do zelador. A máquina tem 4 núcleos: a vaga única de compilação
enfileiraria seis frentes atrás de um build de dois minutos cada, e N vagas
poriam seis `rustc` de 2–3 GB a disputar 4 núcleos. Entrou **`cargo-da-frente.sh`**,
com **duas vagas** (`flock -n -E 99` na primeira, bloqueante na segunda) e
`-j2` em cada.

Um detalhe que decidiu o lugar das worktrees: o zelador apaga o `target` de
toda worktree em `.claude/worktrees/` que não tenha processo com `cwd` dentro
— e um agente **entre dois comandos** não tem. Elas ficaram em
`/home/user/frentes/`, fora do alcance dele, e quem as remove é o orquestrador
na integração.

## 4. A regra

**Custo por unidade se mede numa unidade, não no acumulado de todas — e o
cache incremental é o primeiro suspeito quando um `target` parece grande
demais para o código que ele compila.**

## 5. Como está guardado hoje

- `phxsql/cargo-da-frente.sh` — o ajudante das duas vagas, com o motivo no
  cabeçalho; viaja no repositório para não morrer com a sessão.
- `docs/MODELOS.md`, rodada de 07/09/2026 — o arranjo (seis worktrees em
  `/home/user/frentes/`, duas na árvore principal) e o porquê.
- **O buraco:** o zelador não enxerga `/home/user/frentes/`. Se uma sessão
  morrer no meio da rodada, as seis worktrees e os seis `target` ficam lá até
  alguém rodar `git worktree remove`. O `git worktree list` os mostra — é por
  ele que se acha.
