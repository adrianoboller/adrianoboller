# O corte da força-bruta vetorial cai em 100 mil vetores, não nos milhões — e o número medido é ~15–30× pior que a especulação

Data da descoberta: 12/09/2026, ~15:25 UTC (a bancada `custo-do-vizinho`
fechou o sweep; medição V1+V2 da frente Vetorial).

## 1. O que aconteceu

A frente Vetorial (tipos de base, pedido do dono «faça todos os tipos»)
precisava do número que decide o formato **antes** de gravar qualquer byte:
a busca K-NN exata por força-bruta escalar basta, ou precisa de índice ANN?
Sem esse número, escolher entre `.vec` (só coluna, sem índice) e `.hnsw`
(grafo aproximado, formato complexo) seria chute — e V3+ está atrás do gate P0
e do aval do dono de qualquer jeito, então a medição é o que se pode fazer sem
tocar formato.

O núcleo (`crates/phxsql-core/src/vetor.rs`, produto interno / norma / cosseno /
euclidiana em `f32`, acumulado em `f64`, zero-dep) foi provado contra valor de
referência calculado por script independente. A bancada
(`crates/phxsql-store/examples/custo-do-vizinho.rs`, gravada em
`bancada/vetorial/resultados.json`) varreu N∈{1e4,1e5,1e6} × d∈{384,768,1536},
K=10, cosseno, máquina parada (carga 1min 0.79, 4 núcleos, 14 GiB livres,
`esta-medindo.sh` confirmou nada mais rodando).

## 2. O que eu concluí primeiro, e estava errado

O `docs/VETORES.md` §5 (escrito antes de medir) dizia: *«Para ~100 mil, um scan
exato… talvez poucos ms, e o `.hnsw` (que é APROXIMADO) só se paga nos
milhões.»* Ou seja: a expectativa era que a força-bruta serviria confortavelmente
até ~100 mil vetores mesmo na dimensão real de LLM, e o índice ANN só seria
necessário na escala de milhões. Isso pinta um mundo em que o tipo vetorial
poderia nascer sem índice para quase todo mundo, e o `.hnsw` seria um luxo de
quem tem milhões de vetores.

## 3. O que a medição disse

Com o corte «interativo» em ≤ 50 ms/consulta (explícito no medidor):

| N          | d=384      | d=768      | d=1536       |
|-----------:|-----------:|-----------:|-------------:|
| 10.000     | 3,40 ms    | 8,16 ms    | 17,32 ms     |
| 100.000    | 42,69 ms   | 82,83 ms   | 165,07 ms    |
| 1.000.000  | 410,94 ms  | 841,27 ms  | 1.656,63 ms  |

Custo cru de uma distância: d=384 → 0,809 µs; d=768 → 1,639 µs; d=1536 → 3,400 µs.

- **Cem mil × 1536 custa 165 ms, não «poucos ms».** A especulação era otimista
  por **~15–30×** (165 ms contra «poucos», e 82,8 ms em d=768 contra o mesmo
  «poucos»).
- **Não existe «força-bruta basta» universal.** O corte depende de N×d, e cai
  **já na primeira escala do sweep** (cem mil vetores) para d≥768. Só 100k×384
  (42,7 ms) e as três combinações de N=10 mil ficam abaixo dos 50 ms.
- **Veredito de formato:** corpus pequeno de dimensão baixa (≤~100k × 384) pode
  nascer sem índice; produção (100k+) em dimensões reais de LLM (768/1536)
  **pede ANN** — o `.hnsw` da §4 passa a ter gargalo medido que o justifica em
  produção, não só nos milhões. Recusa medida do «basta universal» é resultado
  tão válido quanto um «basta» teria sido, e é o que impede a proposta de voltar
  sem número.

## 4. A regra

«Basta» e «não basta» de uma busca linear não são propriedade do algoritmo —
são função de N×d contra um limite de latência explícito. Antes de decidir
formato por «a escala do dono é pequena», meça o produto N×d no ponto real de
uso, com o limite escrito no medidor; a intuição de ordem de grandeza sobre
custo escalar erra por uma a duas ordens de grandeza.

## 5. Como está guardado hoje

- Núcleo: `crates/phxsql-core/src/vetor.rs` (8 testes de valor de referência +
  panic por dimensão diferente + guarda de vetor nulo que não vira `NaN`).
- Bancada: `crates/phxsql-store/examples/custo-do-vizinho.rs` →
  `bancada/vetorial/resultados.json` (sweep completo, datado, mediana + faixa).
- Docs atualizados no mesmo commit: `docs/VETORES.md` §5 (tabela medida no lugar
  da especulação) e `docs/propostas/vetorial.md` §1 (premissa passa de «AINDA
  falta medir» a «MEDIDA», com o veredito de formato).
- Formato em disco **intocado**: nenhum `ColumnType::Vetor`, nenhum `.vec`,
  `FORMATO.md` não mudou. V3+ (a coluna e o arquivo) continua atrás do P0 e do
  aval do dono — medição não revoga gate. O buraco que fica é esse: o número
  existe, mas o motor vetorial não; ele só nasce quando o dono liberar o formato
  e o P0 fechar.
