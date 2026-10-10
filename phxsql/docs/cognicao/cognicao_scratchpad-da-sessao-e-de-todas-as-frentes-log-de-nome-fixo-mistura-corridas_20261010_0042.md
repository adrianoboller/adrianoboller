# O scratchpad da sessão é de todas as frentes: log de nome fixo mistura corridas

**Estado:** PENDENTE

## O que aconteceu

Na frente do pedido 770 (10/10/2026), a suíte rodou com a saída em
`<scratchpad>/t1.log` e devolveu 32 testes vermelhos — entre eles
`testes_firewall_e_mensagens::sem_bloco_seguranca_nada_muda`, com a linha
«protecao: tabela phxsys.protecao semeada (17 comandos em proteger)» na saída.
Essa frase **não existe** no fonte desta frente (`grep` em `crates/` inteiro: zero). A
linha `Compiling phxsql-server` do mesmo log apontava para
`.claude/worktrees/agent-a51e76ff148f124b5/` — outra frente. O scratchpad da
sessão é um diretório só para todos os subagentes; duas frentes gravaram no mesmo
`t1.log` e no mesmo `build0.log`.

## O que eu concluí primeiro, e estava errado

Que a base `23f5ef07` já estava vermelha, ou que a semeadura da `phxsys.protecao`
tinha vindo de um commit que esta frente não via — e quase fui montar a árvore da
base para provar. O log era de duas corridas misturadas, e nenhuma das duas
hipóteses tinha como se sustentar com a frase ausente do fonte.

## O que a medição disse

Com o log num subdiretório próprio (`<scratchpad>/ae84/t1.log`), a mesma árvore
deu outro quadro: os vermelhos eram todos do padrão que o 770 mudou de propósito
(TLS de fábrica, política de senha, sessão de 15 min) e nenhum da `phxsys.protecao`.
Depois dos ajustes, 4.517 testes verdes e 0 vermelhos.

## A regra

Frente paralela grava no scratchpad só dentro de um subdiretório com o id dela, e
antes de diagnosticar um log confere que a linha `Compiling` aponta para a própria
árvore.

## Como está guardado hoje

Não está: nada impede duas frentes de escreverem no mesmo nome. O buraco é do
briefing de frente (papel A), que não manda usar subdiretório próprio.
