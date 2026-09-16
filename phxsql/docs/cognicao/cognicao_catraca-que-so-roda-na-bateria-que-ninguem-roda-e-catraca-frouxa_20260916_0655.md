# Cognição: catraca que só roda na bateria que ninguém roda é catraca frouxa — oito dias com o teto furado e três rodadas de portões verdes

- **Assunto:** a catraca `alcancam-fsync` do mapa da trava (`bancada/concorrencia/mapa-da-trava.py`, item 0 da bateria)
- **Descoberta:** 16/09/2026, 06:55 UTC (bissecção por worktree depois da bateria pedida pelo dono)
- **Arquivos:** `bancada/bateria/prova-bateria.py` (item 0), `bancada/concorrencia/mapa-da-trava.py`,
  `crates/phxsql-server/src/servidor.rs` (`op_restaurar_backup`, `reaplicar_diario_ate`),
  `bancada/bateria/corridas/bateria-20260916-0639.log`

## 1. O que aconteceu

O dono pediu «bateria de testes com status, logs, dossiê». A bateria de ponta a
ponta rodou inteira e o item 0 reprovou: **23** seções críticas do `servidor.rs`
alcançam `fsync` com a trava global na mão, contra o teto **22**. A última
corrida versionada da bateria era de **29/08** (`resultados.json`,
`quando: 2026-08-29`). Entre uma e outra, **211 commits** tocaram o
`servidor.rs`, o `phxsql-store` ou o próprio mapa.

## 2. O que eu concluí primeiro, e estava errado

Primeira hipótese: «foi a frente T» — ela acabou de entrar no `servidor.rs`
(semáforo, 503, `recusar_http_cheio` escrevendo no `acessos.log`), e o
`acessos.log` cheira a `fsync`. Rodei o mapa no commit **anterior** à T
(`caee601`): já dava 23. Segunda hipótese: «então foi a leitura repetível
desta madrugada» (a S nova em `dentro_da_transacao`). No commit de ontem
(`162e7f9`): 23 também. A resposta só saiu por **bissecção**, não por palpite:
o salto 22 → 23 está no merge `6245491` da frente `c19-pitr`, em **08/09 às
17:17** — a restauração até um instante reaplicando o diário vivo sob a trava.

Três rodadas de integração desde então tiveram `fmt`, `clippy` e a suíte
verdes, e nenhuma acusou nada, porque **a catraca não mora na suíte**: mora no
item 0 da bateria, e a bateria é um comando que alguém tem de lembrar de dar.

## 3. O que a medição disse

- Teto 22, medido 23; o salto em `6245491` (08/09 17:17); commit anterior
  `a8b46c2` (08/09 17:11) dava 22. Bissecção em 9 medições sobre 211 commits.
- Corridas da bateria versionadas: 29/08 e 16/09. **Dezoito dias** entre elas;
  **oito** com o teto furado.
- Os outros dois ERROs da mesma corrida (item 0b) não são defeito: o portão
  «está medindo?» achou a casca `bash -c` que lançou a bateria (ela carrega o
  comando inteiro na linha de comando) e uma bancada vizinha
  (`escolher-o-desenho.py 20`) de outra frente. Encontro de frentes, medido.

## 4. A regra

**Catraca só vale onde roda sozinha: se ela não está na suíte nem num
gerador de rodada, é lembrete, não guarda — e lembrete envelhece em silêncio.**

## 5. Como está guardado hoje

A pendência **#252** leva as duas decisões ao dono: a exceção nomeada para a
restauração (que aposenta a catraca e faz nascer outra no número do dia, como
manda a lei da régua) ou a reaplicação com a trava solta; e as catracas dos
dois mapas entrando na suíte ou no `numeros-do-projeto.py`. O log da corrida
está versionado em `bancada/bateria/corridas/`.

**Onde o buraco ficou:** enquanto o dono não decide, o teto continua furado e
a bateria continua vermelha no item 0 — e é assim que tem de aparecer na
página de testes: bancada reprovada, com a data, e não bancada ausente.
