# A prova do laço de quedas precisa que o pânico se REPITA no segundo arranque — o índice marcado pela primeira queda o impedia

**Estado:** PENDENTE

*24/09/2026, 16:20 — pedido 502.*

## 1. O que aconteceu

O pedido 502 é um laço: o job em pânico com a trava de ESCRITA na mão aborta
o processo (a thread `relogio-jobs` é de serviço e vai ao piso da H5), o
supervisor sobe de novo, o relógio roda o vencido logo na partida, e aborta de
novo. O conserto tem duas peças: a corrida numa thread filha (a morte é vista
pelo `join`, então repara em vez de abortar) e a lápide (a corrida que ainda
assim derrubou o processo — o reparo que falha — conta como a última no
arranque seguinte).

A prova do laço sobe um processo filho com o reparo armado para falhar, espera
o `SIGABRT`, e sobe um segundo filho no mesmo diretório, com o mesmo pânico
armado.

## 2. O que eu concluí primeiro, e estava errado

- **«Com a lápide tirada, o segundo arranque cai de novo.»** Não caiu: o job
  inseria numa tabela COM índice, e a primeira queda (pânico depois do contador
  do `.reg`) deixou o `.ndx` com o byte 52 em 1. No segundo arranque o mesmo
  `inserir` era RECUSADO antes do ponto do pânico — sem pânico, sem queda —, e
  a prova ficava vermelha só por outro motivo (a abertura sem fechamento no
  histórico), sem mostrar o laço que o pedido descreve. Teste que acerta pelo
  motivo errado é meio passo de teste que passa por engano.
- **«A filha sozinha fecha o 502.»** Fecha o caso comum (reparo que dá certo).
  O piso da H5 continua abortando quando o reparo falha, e aí o laço voltava
  inteiro sem a lápide.

## 3. O que a medição disse

- Job numa tabela SEM índice (`loja.carga`): sem a lápide, **`SIGABRT` também
  no segundo arranque** («LACO DE QUEDAS»); com ela, o segundo arranque fica de
  pé e o histórico diz «nunca terminou».
- Com índice, sem a lápide: o segundo arranque **fica de pé** — o mesmo
  vermelho só aparecia contando as aberturas (2 em vez de 1).
- A corrida na thread de serviço (sem a filha): `SIGABRT` na primeira volta do
  relógio, para o job e para o backup.

## 4. A regra

Numa prova de laço de quedas, confira que o gatilho SE REPETE no arranque
seguinte com o estado que a primeira queda deixou; se o estado deixado impede o
gatilho, a prova mede outra coisa.

## 5. Como está guardado hoje

- Provas: `servidor::testes_do_panico_sob_a_trava::corrida_de_job_que_derrubou_o_processo_nao_roda_de_novo_no_arranque`
  (que também conta as aberturas: 1) e o irmão do backup.
- Guardas: `job-que-derrubou-roda-de-novo-no-arranque`,
  `backup-que-derrubou-roda-de-novo-no-arranque`,
  `job-corre-na-thread-de-servico`, `backup-corre-na-thread-de-servico`.
- A decisão de não rodar de novo tem os três maduros no fonte: `pg_cron`
  (`MarkPendingRunsAsFailed`), MySQL e MariaDB
  (`Event_queue::get_top_for_execution_if_time` grava o `LAST_EXECUTED` antes
  de executar) — `docs/JOBS.md`.
