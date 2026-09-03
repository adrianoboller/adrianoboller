<!-- GERADO por bancada/durabilidade/gerar-matriz.py a partir de
     bancada/durabilidade/resultado.json (medido em 2026-09-03 10:56:36).
     NÃO EDITE À MÃO -- rode prova.py e depois este script. -->

### P1 — antes do `fsync` da marca

| regime | queda determinística (meio da transação, sem `COMMIT`) | a corrida pegou o mesmo desfecho em |
|---|---|---|
| `por_operacao` | ABORTED (provado) — 0 linhas, 0 marca, `achadas=0` | 1 de 9 corridas da varredura |
| `por_lote (padrão)` | ABORTED (provado) — 0 linhas, 0 marca, `achadas=0` | 1 de 9 corridas da varredura |
| `sistema` | ABORTED (provado) — 0 linhas, 0 marca, `achadas=0` | 0 de 9 corridas da varredura |

### P2, P3, P4 — durante e depois da passada

Distribuição dos desfechos ao longo da varredura de atraso (cada célula é uma corrida com `SIGKILL` de verdade; `N/total` conta quantas das corridas caíram naquele ponto):

| regime | calibração (commit limpo) | distribuição medida |
|---|---:|---|
| `por_operacao` | 35.0 ms (1500+1500 linhas) | 1/9 P1 · 2/9 P2 · 3/9 P3 · 1/9 P4 |
| `por_lote (padrão)` | 33.7 ms (1500+1500 linhas) | 1/9 P1 · 3/9 P2 · 3/9 P3 · 2/9 P4 |
| `sistema` | 64.9 ms (1500+1500 linhas) | 3/9 P2 · 2/9 P3 · 4/9 P4 |

Em **nenhuma** das 27 corridas desta seção o relatório do arranque ficou ambíguo, e em nenhuma o número de linhas terminou fora de `{antes, antes+total}` — nunca metade.

### O eixo em que o regime REALMENTE muda o que se vê: quanto tempo a marca fica no disco depois de um commit que NÃO caiu

| regime | logo após o `COMMIT` | 50 ms depois | 1,25 s depois |
|---|---:|---:|---:|
| `por_operacao` | 0 marca(s) | 0 marca(s) | 0 marca(s) |
| `por_lote (padrão)` | 1 marca(s) | 1 marca(s) | 0 marca(s) |
| `sistema` | 1 marca(s) | 1 marca(s) | 1 marca(s) |

### P5 — no meio da cascata do `ao_alterar`

| regime | calibração (commit com a cascata) | veredito das corridas |
|---|---:|---|
| `por_operacao` | 200.0 ms | 4/7 CONSISTENTE · 3/7 PARCIAL_DENUNCIADO |
| `por_lote (padrão)` | 195.4 ms | 4/7 CONSISTENTE · 3/7 PARCIAL_DENUNCIADO |
| `sistema` | 185.6 ms | 4/7 CONSISTENTE · 3/7 PARCIAL_DENUNCIADO |

Em 21 corridas: **0** cascata(s) parcial(is) SEM aviso no relatório (o desfecho que reprovaria a prova).

### §5.5(c) — a marca cuja tabela não abre mais

| regime | a marca sobreviveu ao `SIGKILL`? | o relatório nomeou a tabela que falta? |
|---|---|---|
| `por_operacao` | True | sim (30 op.) |
| `por_lote (padrão)` | True | sim (30 op.) |
| `sistema` | True | sim (30 op.) |

