# O scratchpad da sessão é de todas as frentes dela

**Estado:** PENDENTE

## O que aconteceu

Frente 765/766/779 (papel B), 09/10/2026: a saída dos portões foi gravada em
`<scratchpad>/portoes2.txt`. O veredito lido de lá dizia `FALHOU catracas` e
`== veredito (/…/agent-aecf99b44ffeb7d0e/phxsql)` — a árvore de **outra**
frente. As frentes paralelas da mesma sessão recebem o **mesmo** diretório de
scratchpad, e a outra gravou um arquivo de mesmo nome por cima, com o dela
ainda rodando por baixo do meu. Antes disso, o `red.py` da frente apareceu
«mudado no disco» sem ninguém daqui tê-lo editado.

## O que eu concluí primeiro, e estava errado

Que a catraca `trecho-vivo` reprovava na minha árvore — rodada à mão logo
depois, ela passava (`rc=0`). O vermelho era de outra árvore.

## O que a medição disse

`grep "== veredito"` nos dois arquivos: o `portoes1.txt` nomeia a minha
árvore; o `portoes2.txt`, a da outra frente. O veredito verdadeiro da minha
corrida estava no `target/portoes/suite-*.log` da minha árvore (o portão grava
ali desde o pedido 675) e no código de saída do processo.

## A regra

Em scratchpad de sessão, todo arquivo leva o nome da frente
(`a88-portoes.txt`), e o veredito se lê do log que o portão grava na própria
árvore, nunca de uma cópia de stdout num diretório compartilhado.

## Como está guardado hoje

Só nesta cognição. **O buraco:** nada impede duas frentes de escreverem o mesmo
nome no scratchpad; o `portoes.sh` imprime a árvore no veredito, e foi isso que
denunciou a troca — quem só olha a última linha não vê.
