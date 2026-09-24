# O veneno dito uma vez por TRAVA cala o segundo pânico — e recuperar trabalho em curso pede saneamento

**Estado:** PENDENTE

*24/09/2026, 18:40 — pedido 458.*

## 1. O que aconteceu

O pedido 458 mandava levar a trava `transacoes` do `servidor.rs` para o molde
da `TravaDaGuarda` (436/447): recuperar do envenenamento e avisar uma vez. Um
pânico com ela na mão matava BEGIN, COMMIT e ROLLBACK de toda conexão até
reiniciar (26 tomadas recusando com `SP000010`, 14 calando com `if let Ok`).

## 2. O que eu concluí primeiro, e estava errado

- **«Basta trocar o `Mutex` pela `TravaDaGuarda` — o cluster já provou.»** O
  cluster guarda mapas que o desenrolar não entorta e cujo conteúdo continua
  verdadeiro depois do pânico. As transações guardam **trabalho em curso**: o
  conjunto de escrita de uma delas pode ter ficado pela metade no meio de um
  empilhamento. Recuperar o mapa sem desfazer isso deixa o COMMIT seguinte
  gravar meia operação. Medido com o saneamento reposto: a transação aberta no
  pânico **confirmou** (`"gravadas": 1`).
- **«Avisar uma vez por trava é o suficiente.»** O veneno do `Mutex` é
  permanente, e a marca «já dito» também era: o segundo pânico com a mesma
  trava na mão passaria calado. Com saneamento no meio, calado vira **sem
  saneamento** — a garantia que o primeiro pânico ganhou, o segundo perderia.

- **«A transação abortada segura as travas até o ROLLBACK, como o PostgreSQL
  faz com o bloco abortado.»** Escrevi isso no teste e no `SEGURANCA.md` sem
  medir. O papel C mediu no PG 16.13: a sessão `idle in transaction (aborted)`
  fica com 0 travas no `pg_locks`, e outra sessão atualiza a linha na hora — o
  PG solta no *abort*. Segurar até o ROLLBACK é escolha nossa, e agora está
  escrita como nossa.

## 3. O que a medição disse

- Sem recuperação: a conexão nova não abre transação (`transacoes-envenenadas-recusam-toda-conexao`).
- Sem saneamento: a transação aberta no pânico confirma o que o registro não afirma (`transacoes-recuperadas-sem-sanear`).
- Com os dois: a 8 abre, grava e confirma; a 7 recebe `TRANSACAO_ABORTADA` no COMMIT; a 9 grava depois do ROLLBACK da 7.
- Com o aviso «uma vez por trava» reposto (`veneno-dito-uma-vez-por-trava`): o primeiro pânico saneia, e a transação aberta no SEGUNDO confirma.

## 4. A regra

**Recuperar trava envenenada exige saber o que o pânico pode ter deixado pela
metade atrás dela: se é estado, recupere; se é trabalho em curso, saneie antes
de servir — e detecte cada pânico, não cada trava.** A detecção por evento sai
do `Drop` do guarda com `std::thread::panicking()`, com a regra do próprio
`Mutex`: só suja quem tomou em paz e caiu em pânico com ela na mão.

## 5. Como está guardado hoje

`TravaDaGuarda::nova_saneada` e a `Tomada` (`crates/phxsql-server/src/pulso.rs`);
o saneamento é `Transacoes::abortar_abertas` (`transacao.rs`). Testes em
`servidor::testes_trava_suja`; guardas `transacoes-recuperadas-sem-sanear`,
`transacoes-envenenadas-recusam-toda-conexao` e `trava-suja-sem-nome`, provadas
à mão (o provador inteiro não coube no disco da rodada).

**Onde o buraco ficou:** a transação abortada pelo saneamento segura as travas
dela até o ROLLBACK, e as outras 14 travas com disco atrás continuam recusando
até reiniciar — agora nomeadas, mas recusando. E o caso «guarda capturado e
solto fora do desenrolar» (a cognição de 24/09 03:50, pedido 456) não tem prova
aqui: nele nem o `Mutex` envenena, e a `Tomada` segue a mesma regra.
