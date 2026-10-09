# Estudo L-001 — quantas rodadas faltam para a «% que falta» do PhxClaw chegar a zero

**Papel:** L (cientista) · **Pedido:** dono, 09/10/2026 · **Estado:** ver §8

## 1. Pré-registro (escrito às 15:26 UTC de 09/10/2026, ANTES de extrair a série)

O que eu já tinha visto antes de escrever isto: só o retrato de hoje do `SPRINTS.md` (39 sprints,
13 não concluídas = 33,3%, das quais 3 BLOQUEADA por recurso do dono). A série histórica não.

**Pergunta de decisão.** Com o escopo entrando como tem entrado, a «% que falta das sprints» chega
a zero num horizonte útil? Decide (a) o orquestrador (A): se aplica às sprints a regra (A) de
24/09 (sprint nova não bloqueante nasce ⏸ fora da conta) e se cobra o desbloqueio das
BLOQUEADAS; (b) o dono: o prazo da entrega v0.71 — isso é **produto (prazo)**, sobe a ele.

**Unidade.** *Rodada* = dia UTC com ≥1 commit que muda `phxclaw/docs/absorcao/SPRINTS.md`
(vários commits no mesmo dia são uma rodada; o último estado do dia vale). Sensibilidade: rodada
= commit. **Métrica principal:** `falta% = (total − CONCLUÍDA) / total`, lida pelo MESMO leitor do
dossiê (`tools/dossie/numeros.py::sprints`). **Fluxos por rodada:** `entrou` = ids novos na tabela;
`saiu` = ids que passaram a CONCLUÍDA (mais ids removidos sem concluir, contados à parte).
Série secundária: `docs/absorcao/absorcao.json` (itens totais e «no agente» somados nas fontes).

**Hipóteses** (todas sobre a contagem de sprints abertas `A_t = total − CONCLUÍDA`):

- **H0 — sem tendência:** `falta%` é passeio aleatório; o ingênuo (último valor) não perde para
  nenhum modelo de tendência no backtest.
- **H1 — queda linear:** `falta%` cai a taxa constante por rodada; a regressão de tendência ganha
  do ingênuo no backtest (MASE < 1) e a extrapolação dá N rodadas até zero.
- **H2 — equilíbrio:** a entrada de escopo λ acompanha o fechamento μ; o IC 95% por bootstrap da
  média de (μ − λ) por rodada contém zero ou é negativo, e P(não chegar a zero em 50 rodadas) > 50%.
- **H3 — converge só congelado:** com λ = 0, as abertas NÃO bloqueadas zeram em N rodadas
  (bootstrap de μ); as BLOQUEADA (dono) são piso que nenhuma rodada de trabalho remove.

**O que decide (limiares fixados aqui):**
- H1 contra H0: MASE (escala = erro do ingênuo de 1 passo dentro da amostra) do modelo de
  tendência < 1,0 no backtest de origem móvel, horizonte 1 rodada. MASE ≥ 1 → H1 morre.
- H2: IC 95% (10.000 reamostras) de mean(μ − λ) contém 0 → «não se pode afirmar convergência»;
  P(não converge em 50 rodadas) > 0,5 → H2 sustenta.
- H3: mediana e IC 90% de rodadas até zerar as não bloqueadas, com λ = 0.
- **Amostra mínima:** backtest só com ≥ 8 rodadas e ≥ 3 origens; abaixo disso a série é
  declarada curta, o estado fica PENDENTE e o intervalo sai largo, dito como largo.
- Horizonte de «não convergir»: 50 rodadas. Bootstrap: 10.000 reamostras, semente fixa 20261009.

## 2. Dados (proveniência)

| fonte | grão | pontos | período | lido por |
|---|---|---:|---|---|
| `phxclaw/docs/absorcao/SPRINTS.md`, tabela «Visão geral», em cada commit (`git show <h>:…`) | um commit que mudou o arquivo | 15 commits (16b9cf1b … 12cf9802) | 01/10 05:28 → 09/10 09:13 UTC | `tools/dossie/numeros.py::sprints()` — o leitor do dossiê, importado; 0 commits sem leitura |
| o mesmo, por sprint | commit × sprint | 15 × até 39 | idem | idem |
| `phxclaw/docs/absorcao/absorcao.json` em cada commit | commit × fonte | 16 commits, 3→7 fontes | 01/10 02:15 → 09/10 14:23 UTC | json da stdlib |
| `phxclaw/docs/sprints/Sessao_*_Sprint_*.md` (registro de rodada) | um arquivo; a fronteira é o commit que o criou | 6 (SP000013 … SP000038) | 02/10 02:33 → 09/10 14:24 UTC | `git log --diff-filter=A` |

Extrator: `docs/ciencia/extratores/serie_do_que_falta.py` (gera `target/ciencia/serie_do_que_falta.sqlite`,
fora do git). Modelos: `docs/ciencia/extratores/modelos_do_que_falta.py`. Rodado 2× em 09/10/2026:
dump das 5 tabelas idêntico (`cmp`) e saída dos modelos idêntica (semente 20261009). A saída inteira
está no Apêndice A, colada da corrida, não digitada.

## 3. Método, e por que este

- **Duas definições de rodada.** P (pré-registrada): dia UTC com commit no `SPRINTS.md` → **4
  pontos**. R (sensibilidade, achada depois de ver os dados, e por isso só sensibilidade): registro
  em `docs/sprints/` → **6 rodadas** + estado inicial. Por commit (15 pontos) só no backtest.
- **Backtest de origem móvel, 1 passo**, ingênuo × deriva × tendência (MQO); MAE e MASE (escala =
  erro do ingênuo no treino de cada origem).
- **H2:** bootstrap (10.000) da média de (saiu − entrou) por rodada.
- **Dois fluxos:** cada trajetória reamostra as rodadas observadas (incerteza do parâmetro) e
  sorteia pares (saiu, entrou) **juntos** — preserva o fato de rodada que fecha também abrir —;
  10.000 trajetórias, teto 50 rodadas. BLOQUEADA só sai se o dono destravar; como **0 saídas** foram
  observadas, o cenário «dono no teto» usa o limite superior Poisson exato de 95%.
- Proporções (rodada que fecha também abre): Wilson.

## 4. Resultado

**Série principal (def. P):** `falta%` no fim de cada rodada = 33,3 · 33,3 · 33,3 · 33,3. Em R:
33,3 · 37,1 · 33,3 · 33,3 · 33,3 · 33,3 · 33,3. A métrica está **parada em 13 de 39 desde 02/10
05:32** (5 rodadas R), enquanto a lacuna de absorção caiu de 54 para 18 itens nas mesmas rodadas.

| hipótese | número | limiar | veredito |
|---|---|---|---|
| H0 sem tendência | ingênuo é o melhor ou empata nos 3 backtests (por commit MAE 5,89 pp; R MAE 0) | — | **sustenta** (série curta, ver §5) |
| H1 queda linear | tendência por commit: MASE **1,31**, MAE 2,59× o do ingênuo; em R pior que o ingênuo | MASE < 1 | **INFRUTÍFERO** |
| H2 equilíbrio | (saiu − entrou)/rodada: P média 0,50, IC95 [−1,50; 3,00]; R −0,33, IC95 [−1,00; 0,00]. P(não zera em 50) = **100%** em P e em R | IC contém 0 e P > 50% | **sustenta** |
| H3 congelado | abertas não bloqueadas zeram: P mediana **3** rodadas, IC90 [1; >50], P(não) 5,9%; R mediana **19**, IC90 [5; >50], P(não) **34,2%** | — | **sustenta só para as não bloqueadas**; zero total, não |

Por que H2 dá 100%: (i) **3 sprints BLOQUEADA (dono)** — 0 saídas em 18 sprint-rodadas (Poisson
exato: taxa ≤ 0,166/sprint-rodada a 95%) — põem um piso de 3/39 = **7,7%** que nenhuma rodada de
trabalho remove; (ii) toda rodada que fechou sprint também abriu (P 2/2, Wilson95 [34,2%; 100%];
R 1/1, [20,7%; 100%]).

**Cenários (rodadas até `falta% = 0`):**

| cenário | def. P | def. R |
|---|---|---|
| escopo como tem entrado | P(não chega em 50) = 100% | 100% |
| escopo congelado, sem o dono | 100% (piso 7,7%) | 100% (piso 7,7%) |
| congelado + dono destravando no teto do dado | mediana 13, IC90 [5; >50], P(não) 6,5% | mediana 27, IC90 [9; >50], P(não) 34,2% |

**Série secundária (absorção, lacuna = itens − «no agente», def. R):** 43 → 32 → 54 → 44 → 40 → 25
→ 18 itens (5,5% de 329). Escopo entra em blocos (fonte nova: n8n, +59, em 1 de 6 rodadas).
Piso do dono pelo léxico (frases casadas no `SPRINTS.md`: iMessage, voz ao vivo, RAPL, nuvem):
**7 itens**. Até o piso: congelado mediana 2 rodadas, IC90 [1; 3]; como tem entrado mediana 2,
IC90 [1; >50], P(não) 12,2%. Zero: 100% de não chegar sem o dono.

## 5. O que NÃO se pode concluir

- **A série é curta para o pré-registro**: 4 rodadas em P e 6 em R, abaixo das ≥ 8 fixadas. O
  backtest só passa o mínimo por commit (15 pontos, 12 origens), que não é a unidade decidida.
- **P e R discordam** (H3: 3 contra 19 rodadas): o primeiro dia (22 sprints fechadas, 14 delas
  remarcando trabalho já comitado em 4670bc20/f27402e5 — conferido pelo leitor do dossiê contra o
  `ids_saiu` do cubo) domina P. Não sei qual representa o futuro.
- A métrica é **binária por sprint**; as abertas são grandes (SP000032: 32 chaves; SP000035: 5
  ondas). Trabalho dentro delas não aparece — 5 rodadas de trabalho real com `falta%` parada.
- Na absorção, as 11 lacunas que sobram são as **difíceis** (6 do n8n «pela metade»); usar a taxa
  das fáceis já fechadas é otimista. E o piso de 7 é limite inferior: o SP000031 diz que os 3 «não»
  do VS Code são «de produto», o que o léxico não casa por id.
- Correlação «rodada que fecha também abre» com n = 2 não é lei; o experimento que separa é
  congelar o escopo por 3 rodadas e ver se o fechamento cai junto (se cair, a entrada é efeito do
  mesmo trabalho que fecha, não escopo independente).

## 6. Decisão recomendada

1. **A % das sprints não chega a zero por rodada de trabalho**: o piso de 7,7% é decisão do
   dono (SP000014, SP000015, SP000025). Sobe ao dono como **produto (prazo)**: destravar, ou tirar
   as três da conta com ⏸ e dizê-lo na página.
2. **Orquestrador (A):** aplicar a regra (A) de 24/09 às sprints — sprint nova não bloqueante nasce
   ⏸ — compra H3: mediana 3 a 19 rodadas para as não bloqueadas, contra «nunca» como tem entrado.
3. **Documentação (H)/dossiê:** publicar ao lado da `falta%` das sprints uma métrica de grão fino
   (chaves feitas/total), porque a atual ficou cega 5 rodadas.

## 7. Previsão datada (gravada 09/10/2026 ~15:50 UTC; conferir na primeira rodada registrada depois de SP000038, ou em 16/10/2026 se não houver)

| o quê | horizonte | def. P | def. R |
|---|---|---|---|
| `falta%` das sprints | 1 rodada | 33,3% [33,3; 36,8] | 33,3% [30,2; 36,6] |
| `falta%` das sprints | 3 rodadas | 33,3% [25,9; 36,8] | 33,3% [27,7; 39,5] |
| P(`falta%` < 33,3% na próxima rodada) — para Brier | 1 rodada | 0% | 16,4% |
| lacuna de absorção (% de itens) | 1 rodada | — | 2,4% [2,1; 17,0] |
| `falta%` = 0 | 50 rodadas | P(não) 100% | P(não) 100% |

Intervalos de 90% das trajetórias. Na conferência: registrar o valor lido pelo extrator, o Brier
das duas definições (qual calibra melhor decide a unidade de rodada do próximo estudo) e se o dono
mexeu nas BLOQUEADA.

## 8. Estado

**PENDENTE.** H1 INFRUTÍFERO (causa: métrica parada em 13/39 por 5 rodadas e escopo que entra junto
com o fecho; prevenção: não extrapolar tendência de métrica binária com < 8 rodadas). H2 sustenta,
H3 sustenta só para as não bloqueadas — nenhuma vira FRUTÍFERO antes da conferência da §7.

**Próxima hipótese (L-002):** a fração de **chaves** feitas (coluna «Chaves» e detalhe do estado no
`SPRINTS.md`) cai por rodada com MASE < 1 contra o ingênuo, onde a `falta%` por sprint não cai.

## Apêndice A — saída de `modelos_do_que_falta.py` (09/10/2026, semente 20261009)

```
== 1. series por rodada
-- P sprints (dia): 4 pontos
   2026-10-01 total  33  falta  11  33,3%  entrou 18  saiu 22
   2026-10-02 total  39  falta  13  33,3%  entrou  6  saiu  4
   2026-10-06 total  39  falta  13  33,3%  entrou  0  saiu  0
   2026-10-09 total  39  falta  13  33,3%  entrou  0  saiu  0
-- R sprints (registro): 7 pontos
   inicio     total  33  falta  11  33,3%  entrou  0  saiu  0
   SP000013   total  35  falta  13  37,1%  entrou  2  saiu  0
   SP000031   total  39  falta  13  33,3%  entrou  4  saiu  4
   SP000035   total  39  falta  13  33,3%  entrou  0  saiu  0
   SP000036   total  39  falta  13  33,3%  entrou  0  saiu  0
   SP000037   total  39  falta  13  33,3%  entrou  0  saiu  0
   SP000038   total  39  falta  13  33,3%  entrou  0  saiu  0
-- R absorcao (lacuna): 7 pontos
   inicio     total 270  falta  43  15,9%  entrou  0  saiu  0
   SP000013   total 270  falta  32  11,9%  entrou  0  saiu 11
   SP000031   total 329  falta  54  16,4%  entrou 59  saiu 37
   SP000035   total 329  falta  44  13,4%  entrou  0  saiu 10
   SP000036   total 329  falta  40  12,2%  entrou  0  saiu  4
   SP000037   total 329  falta  25  7,6%  entrou  0  saiu 15
   SP000038   total 329  falta  18  5,5%  entrou  0  saiu  7

== 2. backtest de origem movel, 1 passo (minimo 3 pontos de treino)
-- sprints por commit (sensib.): 15 pontos, 12 origens
   ingenuo    MAE 5,89  rel.ingenuo 1,00  MASE 0,41 (n=12)
   deriva     MAE 9,00  rel.ingenuo 1,53  MASE 0,69 (n=12)
   tendencia  MAE 15,29  rel.ingenuo 2,59  MASE 1,31 (n=12)
-- sprints P: 4 pontos, 1 origens
   ingenuo    MAE 0,00  rel.ingenuo 0,00  MASE — (n=0)
   deriva     MAE 0,00  rel.ingenuo 0,00  MASE — (n=0)
   tendencia  MAE 0,00  rel.ingenuo 0,00  MASE — (n=0)
-- sprints R: 7 pontos, 4 origens
   ingenuo    MAE 0,00  rel.ingenuo 0,00  MASE 0,00 (n=4)
   deriva     MAE 0,00  rel.ingenuo 0,00  MASE 0,00 (n=4)
   tendencia  MAE 0,54  rel.ingenuo inf  MASE 0,22 (n=4)
-- absorcao R (lacuna, itens): 7 pontos, 4 origens
   ingenuo    MAE 9,00  rel.ingenuo 1,00  MASE 0,68 (n=4)
   deriva     MAE 9,37  rel.ingenuo 1,04  MASE 0,68 (n=4)
   tendencia  MAE 13,24  rel.ingenuo 1,47  MASE 1,01 (n=4)

== 3. H2: bootstrap da media de (saiu - entrou) por rodada, 10.000 reamostras
   P sprints   n=4 liquido/rodada [4, -2, 0, 0]  media 0,50  IC95 [-1,50; 3,00]
   R sprints   n=6 liquido/rodada [-2, 0, 0, 0, 0, 0]  media -0,33  IC95 [-1,00; 0,00]
   R absorcao  n=6 liquido/rodada [11, -22, 10, 4, 15, 7]  media 4,17  IC95 [-6,83; 11,50]

== 4. simulacao de dois fluxos (10.000 trajetorias, teto 50 rodadas)
   estado inicial (HEAD do SPRINTS.md): abertas nao bloqueadas 10, bloqueadas 3, concluidas 26; p(entrada nasce bloqueada) P=0,042 R=0,000
   saidas de BLOQUEADA observadas: 0 em 18 sprint-rodadas (def R); taxa sup. 95% (Poisson exato) 0,166/sprint-rodada
   [P] como tem entrado          alvo=tudo            P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  falta% h1 33,3 [33,3; 36,8]  h3 33,3 [25,8; 36,8]  P(h1<33,3%)=0,0%
   [P] como tem entrado          alvo=nao_bloqueadas  P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  falta% h1 33,3 [33,3; 36,8]  h3 33,3 [25,9; 36,8]  P(h1<33,3%)=0,0%
   [P] congelado                 alvo=tudo            P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  falta% h1 33,3 [7,7; 33,3]  h3 7,7 [7,7; 33,3]  P(h1<33,3%)=49,5%
   [P] congelado                 alvo=nao_bloqueadas  P(nao chega em 50)=5,9%  rodadas med 3 IC90 [1; 51]  falta% h1 23,1 [7,7; 33,3]  h3 7,7 [7,7; 33,3]  P(h1<33,3%)=51,1%
   [P] congelado + dono no teto  alvo=tudo            P(nao chega em 50)=6,5%  rodadas med 13 IC90 [5; 51]  falta% h1 33,3 [7,7; 33,3]  h3 7,7 [2,6; 33,3]  P(h1<33,3%)=49,5%
   [R] como tem entrado          alvo=tudo            P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  falta% h1 33,3 [30,2; 36,6]  h3 33,3 [27,7; 39,5]  P(h1<33,3%)=16,4%
   [R] como tem entrado          alvo=nao_bloqueadas  P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  falta% h1 33,3 [30,2; 36,6]  h3 33,3 [27,7; 39,5]  P(h1<33,3%)=16,7%
   [R] congelado                 alvo=tudo            P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  falta% h1 33,3 [23,1; 33,3]  h3 33,3 [12,8; 33,3]  P(h1<33,3%)=16,6%
   [R] congelado                 alvo=nao_bloqueadas  P(nao chega em 50)=34,2%  rodadas med 19 IC90 [5; 51]  falta% h1 33,3 [23,1; 33,3]  h3 33,3 [12,8; 33,3]  P(h1<33,3%)=17,1%
   [R] congelado + dono no teto  alvo=tudo            P(nao chega em 50)=34,2%  rodadas med 27 IC90 [9; 51]  falta% h1 33,3 [23,1; 33,3]  h3 33,3 [12,8; 33,3]  P(h1<33,3%)=16,5%

   absorcao: lexico do dono casado {'canal_imessage': 'iMessage', 'voz_wake': 'voz ao vivo', 'telemetria_energia': 'RAPL', 'ambientes_nuvem': 'nuvem'}; itens da lacuna atual no piso do dono: 7
   [R absorcao] como tem entrado   alvo=tudo            P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  lacuna% h1 2,4 [2,1; 17,0]
   [R absorcao] como tem entrado   alvo=nao_bloqueadas  P(nao chega em 50)=12,2%  rodadas med 2 IC90 [1; 51]  lacuna% h1 2,4 [2,1; 17,0]
   [R absorcao] congelado          alvo=tudo            P(nao chega em 50)=100,0%  rodadas med — IC90 [—; —]  lacuna% h1 2,4 [2,1; 4,3]
   [R absorcao] congelado          alvo=nao_bloqueadas  P(nao chega em 50)=0,0%  rodadas med 2 IC90 [1; 3]  lacuna% h1 2,4 [2,1; 4,3]
   [P] rodadas que fecharam sprint e tambem abriram: 2/2  Wilson95 [34,2%; 100,0%]
   [R] rodadas que fecharam sprint e tambem abriram: 1/1  Wilson95 [20,7%; 100,0%]

   rodadas R que fecharam >=1 sprint: 1/6  Wilson95 [3,0%; 56,4%]
   rodadas R que abriram >=1 sprint: 2/6  Wilson95 [9,7%; 70,0%]
```
