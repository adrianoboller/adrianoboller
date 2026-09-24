# Corpo de falso positivo tirado de uma fonte só mede essa fonte

**Estado:** PENDENTE

A prova virá da fatia F0 do 495: os três textos legítimos entram no teste de
SQL legítimo do `comando_empilhado`. O teste tem de falhar com o código de
hoje e passar com o conserto.

## O que aconteceu

O pedido 215 (contar injeção de SQL contra o IP) passou na prova de falso
positivo dele (5b), e nasceu desligado de fábrica. O papel J, no 495,
extraiu o SQL legítimo do repositório inteiro, 1.186 textos das crates, das
bancadas e da tela, e ligou o interruptor. O resultado está em
`bancada/seguranca/495/prova_215.py`: cinco pedidos legítimos pelo soquete
(`LIMIT 0, 2`, `RETURNING`, `EXCEPT`) **bloquearam 127.0.0.1 por 60 minutos**,
com o firewall disparado.

## O que eu concluí primeiro, e estava errado

Que o 5b tinha provado o falso positivo. Tinha provado só o corpo que usou.
Esse corpo não tinha nenhum comando com sobra depois do fim, e o
`comando_empilhado` não confere se houve `;`: ele acusa qualquer sobra que o
analisador não consome.

## O que a medição disse

- No corpo legítimo, o `comando_empilhado` acusa **17** textos, e **11** deles
  são um comando só.
- Um detector que analisa o léxico do motor acusa **1** texto em 1.186. Um que
  recorta o texto cru acusa **107**.

## A regra

O corpo de falso positivo sai do repositório inteiro, por um extrator (o
`extrair_legitimo.py`), e não de uma fonte escolhida à mão.

## Como está guardado hoje

Não está. O 215 continua acusando o comando único que tem sobra. O conserto é
a fatia F0 do 495, que abriu o pedido 501.
