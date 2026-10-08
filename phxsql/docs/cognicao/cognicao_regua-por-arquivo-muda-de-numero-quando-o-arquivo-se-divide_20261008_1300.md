# Régua que lê «o arquivo» muda de número quando o arquivo se divide — e uma delas nunca tinha lido o `servidor.rs`

**Estado:** PENDENTE

## O que aconteceu

Passos 3 e 4 da divisão do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
o `impl Servidor` saiu para 25 `servidor/servico_*.rs`, só movimento e
`pub(super)`. Três leitores mudaram de número sem nada no código ter mudado:

- `trecho-vivo.py`, `TETO_MENSAGEM_AMBIGUA`: **81 → 105**.
- `bancada/catracas/todas.py`: «sem teste candidato» **22 → 23**
  (`TETO_DE_EVENTOS_POR_LOTE` foi para `servico_quorum_01.rs`, e o teste dela
  mora no irmão `testes_do_lote_de_replicacao.rs`, não num filho).
- `telemetria.rs::toda_operacao_com_ponto_de_cancelamento_esta_na_lista`:
  cortava o fonte por `"    fn op_"`; com `pub(super) fn op_` achou **zero**
  e caiu (caiu alto, pelo `>= 4`; teria passado calado sem esse piso).

## O que eu concluí primeiro, e estava errado

Que o 81 → 105 era a divisão «revelando» frases repetidas. Medido, era o
contrário: `producao()` corta cada arquivo no primeiro `#[cfg(test)]`, e no
`servidor.rs` ele está na **linha 21** (`use crate::apoio_teste::DirTemp`).
A régua via **20 linhas** do `servidor.rs` — nenhuma das 240 entradas
apontadas para ele contava. Os `servico_*.rs` não têm `#[cfg(test)]` no
topo, e ela passou a ver código pela primeira vez.

## O que a medição disse

Com a régua lendo o arquivo inteiro (sem o corte cego): **203 antes, 189
depois** — dividir baixa a repetição por arquivo sem melhorar nada. Lendo a
união das fontes do servidor (a regra do plano, §3): **81 antes, 81 depois,
o mesmo conjunto de ids**. O `todas.py` lendo a união: 22 = 22.

## A regra

Régua cuja unidade é «o arquivo» lê a **união** das fontes do servidor
(`bancada/fontes_do_servidor.py`); senão o número muda com a fronteira, não
com o código. E leitor que casa texto de declaração casa `fn nome(`, nunca a
indentação nem a visibilidade antes dele.

## Como está guardado hoje

`trecho-vivo.py` (`mensagem_ambigua`) e `todas.py` (`teste_que_confere`) leem
a união; o teste da telemetria tira o `pub(super)` antes de cortar. **O
buraco continua:** `TETO_MENSAGEM_AMBIGUA` segue cega ao servidor (a união
começa pelo `servidor.rs`, e o corte da linha 21 vale para ela). Consertar o
`producao()` muda a régua — pela regra do papel G, aposenta a catraca e nasce
outra no número do dia; decisão de QA, não desta frente.
