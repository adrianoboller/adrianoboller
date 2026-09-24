# Cognição — arquivo de saída lido cedo não é corrida perdida

**Descoberto em** 23/09/2026, 23:10 · pedido 437 · papel A (integrador)

## 1. O que aconteceu

Uma corrida de `cargo test --workspace` derrubou
`telemetria::testes::as_threads_do_so_se_medem_e_nunca_sao_menos_que_as_registradas`
com um vermelho; a corrida seguinte do mesmo comando voltou verde.

Para medir a taxa, o integrador lançou um laço de 30 corridas do binário da
lib com `nohup … &`, em segundo plano. Leu o arquivo de saída pouco depois, viu
só a linha de cabeçalho e concluiu que o processo tinha morrido. Lançou um
segundo laço, de 15, esperou este até o fim, e publicou no pedido 437 «0
vermelhos em 15 corridas».

O laço de 30 não tinha morrido: terminou às 23:10:20 com «0 vermelhos em 30
corridas» escrito no próprio arquivo de saída. O número real, somando os dois
laços, é **0 em 45**. O pedido foi corrigido no commit `520a0cb`
(«437: a taxa era 0 em 45, nao 0 em 15 -- medicao lancada e medicao a colher»).

## 2. O que eu concluí primeiro, e estava errado

Concluí que um arquivo de saída curto, lido logo depois do `nohup … &`,
significava que o processo tinha caído — e por isso lancei um segundo laço
menor em vez de simplesmente esperar o primeiro. Não conferi o PID nem `ps`
antes de decidir que a corrida tinha morrido: decidi pela ausência de linhas,
que é exatamente a assinatura de uma corrida **em andamento**, não de uma
corrida morta.

## 3. O que a medição disse

| medida | número |
|---|---|
| vermelhos publicados no pedido 437, na primeira redação | 0 em **15** |
| vermelhos reais, somando os dois laços (30 + 15) | 0 em **45** |
| momento em que o laço de 30 terminou, com o resultado já no arquivo | 23:10:20 |
| corridas do laço de 30 que o integrador chegou a contar antes de desistir dele | 0 (leu só o cabeçalho) |

## 4. A regra

**Arquivo de saída lido cedo não é corrida perdida — é corrida que ainda não
acabou, e quem desiste dela publica o número menor.** Antes de lançar um
segundo laço para substituir o primeiro, confira se o processo do primeiro
ainda está vivo (`ps`/PID), não só o que já está escrito no arquivo.

## 5. Como está guardado hoje

- `docs/PENDENCIAS.md`, pedido 437: publica os dois números (o «15» da
  primeira redação e o «45» medido) e explica por que mudou, em vez de
  substituir o valor calado.
- Commit `520a0cb` carrega a correção e a lição no próprio corpo da mensagem.
- **Não guardado ainda**: não há guarda automática que impeça publicar um
  número de bancada enquanto o processo que a produz continua vivo — a
  prevenção aqui foi só esta cognição. Papel G, se quiser fechar o buraco.
