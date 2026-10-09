# Régua que lê «o arquivo» muda de número quando o arquivo se divide — e uma delas nunca tinha lido o `servidor.rs`

**Estado:** FRUTÍFERO

**Evidência:** `a0325c80`

**Validação (08/10/2026):** `python3 bancada/guardas/trecho-vivo.py --autoteste` roda hoje e termina em «todos passaram»; o conserto do `producao()` está no commit `a0325c80`. Promovido em 08/10/2026 pelo papel H.

**Alcance da pétrea, não lei nova:** o aprendizado é o alcance de «a receita de um número também envelhece» (papel H) e de «régua que passa a medir mais aposenta a catraca» (papel G): uma régua nunca tinha lido o `servidor.rs`, e isso só apareceu quando o arquivo se dividiu. **Cruzamento:** o #13 (`cognicao_regua-que-corta-o-teste-precisa-ver-todo-cfg-test_20261008_1730.md`) é o segundo defeito do mesmo `producao()` no mesmo dia; os dois são o alcance da mesma régua de corte por texto, e ler um sem o outro repete o erro.

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
a união; o teste da telemetria tira o `pub(super)` antes de cortar. **Atualizado em 08/10/2026:** o
«buraco continua» desta seção foi fechado pelo pedido 718: a catraca
`TETO_MENSAGEM_AMBIGUA` foi aposentada e nasceu
`TETO_MENSAGEM_AMBIGUA_COM_O_SERVIDOR_INTEIRO` (hoje 130 em `trecho-vivo.py`, depois
de 148 → 130 no pedido 657). O `producao()` corta por arquivo, não mais a união.
