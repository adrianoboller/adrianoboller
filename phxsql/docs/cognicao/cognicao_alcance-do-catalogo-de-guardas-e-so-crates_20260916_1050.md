# O alcance do catálogo de guardas: cobre `crates/`, não `bancada/`

Descoberto em 16/09/2026, ~10:50 UTC, catalogando a catraca do `pkill` sem PID
(pedido 256, frente G1).

## 1. O que aconteceu

O pedido pedia para "catalogar a guarda em `bancada/guardas/catalogo.py`, com
o defeito que a motivou" — o defeito sendo `bancada/carga/bulkinsert.py`
chamando `pkill -x phxsqld` e derrubando o `phxsqld` de outra frente. Antes de
escrever a entrada, li `bancada/guardas/provar-guardas.py` para copiar o
formato certo, e a lista `COPIAR` do executor é:

```python
COPIAR = ["Cargo.toml", "Cargo.lock", "crates", "exemplos", "docs", "testes-web"]
```

`bancada/` não está nessa lista. O executor **copia a árvore inteira para um
cache** (`~/.cache/phx-guardas`) antes de repor qualquer `trecho`/`troca` e
rodar `cargo test` — e só copia os seis itens acima. Uma entrada cujo
`arquivo` apontasse para `bancada/carga/bulkinsert.py` seria escrita no
catálogo, nunca rodaria (o arquivo simplesmente não existiria na cópia), e o
executor a reportaria `QUEBRADA` ("o trecho não está mais no arquivo") — ou
pior, alguém leria a entrada no catálogo e acreditaria que a guarda existe.

## 2. O que eu concluí primeiro, e estava errado

Concluí primeiro que "catalogar a guarda" era só uma questão de formato —
escrever os campos `arquivo`/`trecho`/`troca`/`pacote`/`alvo`/`caem`/`seguem`
do jeito que as outras ~40 entradas fazem, e o defeito sendo em Python em vez
de Rust não devia importar, porque o pedido não distinguiu. Só ao ler o
executor (não só o `LEIA-ME.md`, que fala em termos gerais de "guarda") é que
vi que o mecanismo é estruturalmente amarrado a `cargo test` sobre uma cópia
de `crates/`. Catalogar ali uma guarda de `bancada/` não seria "meia
funcionalidade" — seria uma entrada que **nunca poderia ser provada**, o que é
pior: um catálogo que promete uma guarda que não roda é a mesma doença que
`docs/CATRACAS.md` já nomeia noutro contexto ("emitir nada quando a fonte
sumiu é a mesma doença do conferidor que diz «limpo» sem ter conferido").

## 3. O que a medição disse

`grep -n "COPIAR = " bancada/guardas/provar-guardas.py` → uma lista de 6
itens, sem `bancada`. Confirmado também que nenhuma das ~40 entradas
existentes em `catalogo.py` tem `arquivo` fora de `crates/` — todas apontam
para `crates/phxsql-server/src/...` ou `crates/phxsql-store/src/...`.

## 4. A regra

**O catálogo de guardas (`bancada/guardas/catalogo.py` + `provar-guardas.py`)
só alcança defeito reposto em `crates/`, `exemplos/`, `docs/` e
`testes-web/` — nunca em `bancada/`.** Uma dívida de script de bancada pede
outro dono: uma **catraca** (contagem estática, sem `cargo test`), documentada
em `docs/CATRACAS.md`, do jeito que a `TETO_TEMP_DIR_SOLTO` (§6) já é uma
contagem sobre `crates/*/src` sem rodar teste nenhum para descobrir o número.

## 5. Como está guardado hoje

A guarda do pedido 256 nasceu em `bancada/guardas/pkill-sem-pid.py` (uma
catraca independente, chamada pelo item 0b da bateria), e está catalogada em
`docs/CATRACAS.md` §11 — não em `bancada/guardas/catalogo.py`, com esta
cognição linkada de lá para quem perguntar "por que não está no catálogo?".
Se um dia `bancada/` entrar na lista `COPIAR` (por exemplo, para provar um
defeito de bancada como troca de Python em vez de Rust — o executor teria de
rodar `python3` em vez de `cargo test`, o que ele hoje não sabe fazer), esta
cognição é o lugar para atualizar o alcance, não reescrever do zero.
