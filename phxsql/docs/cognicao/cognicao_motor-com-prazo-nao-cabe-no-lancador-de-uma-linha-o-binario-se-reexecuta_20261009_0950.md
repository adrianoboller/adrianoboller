# Motor com prazo não cabe no lançador de uma linha: o binário se reexecuta

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-server/tests/queda-nao-prende-a-trava.rs::o_filho_do_gancho_nasce_do_lancador_com_as_garantias_do_motor`; `crates/phxsql-server/tests/queda-nao-prende-a-trava.rs::o_sigkill_com_o_gancho_ligado_nao_deixa_a_trava_no_filho_do_gancho`

## O que aconteceu

Pedido 759, irmão do 758. O gancho do operador e o comando de firewall
(`gancho::rodar`) criavam o filho direto do `phxsqld`, com a `.phxsql.trava`
na mão. O `flock` é da descrição aberta e o `spawn` a copia até o `exec`: com
o servidor morto por `SIGKILL` nessa janela, a abertura gravável seguinte
ouvia `InstanciaOcupada`. O `Lancador` do 758 é um `sh` que roda o `df` por
linha, e não serve aqui: o gancho precisa de `env_clear`, stdin, prazo e
`kill`+`wait`, e escrever isso em `sh` seria a segunda cópia do motor.

## O que eu concluí primeiro, e estava errado

1. Que o gancho, com caminho absoluto (um `exec` só, sem a busca nas 12 pastas
   do `PATH` que alargava a janela do `df`), quase nunca pegaria a janela.
   Medido: 31 recusas em 800 quedas, 8 corridas vermelhas em 8.
2. Que a varredura do `/proc/*/fd` na hora da recusa mostraria o detentor, como
   no 758 (`comm=vigia-disco ppid=1`). Não mostrou ninguém em 9 de 9 recusas:
   o filho já tinha chegado ao `exec` (o `CLOEXEC` solta a cópia) entre a
   recusa e a varredura. A recusa é transitória e mesmo assim é recusa: quem
   abre a pasta logo depois da queda ouve «ocupada».

## O que a medição disse

- Spawn direto: 31/800 (3, 6, 1, 2, 9, 1, 4, 5 por 100); gancho desligado,
  0/300 — o defeito é do gancho, não do `df`.
- Com o lançador (`phxsqld --lancador-de-ganchos`, nascido no
  `Servidor::novo` antes da primeira trava): 0 em 1.100, com o gancho rodando
  100 vezes em 100 quedas (sem a contagem, um gancho que nunca rodasse
  passaria verde).
- RED reposto (o `rodar` ignorando o lançador): 22/600, 6 corridas vermelhas
  em 6.

## A regra

Quando o filho precisa de um motor inteiro (prazo, `kill`, ambiente, entrada),
o lançador que nunca segurou a trava é o PRÓPRIO binário reexecutado, que
chama a mesma função do motor — não um `sh` que reescreve o motor.

## Como está guardado hoje

Guarda `gancho-filho-direto-com-a-trava` (provada: o pai do gancho tem de ser
o lançador, pelo `/proc`). O teste das 100 quedas é por probabilidade e fica
fora do `caem`. Buraco dito: sem o executável habilitado (binário de teste da
lib, lançador que não nasceu), o `rodar` volta ao spawn direto — as guardas
`gancho-*` da lib provam o motor por esse caminho, e só os testes de
integração com o `phxsqld` real passam pelo lançador.
