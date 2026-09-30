# O rascunho da sessão é dividido entre agentes: diretório com nome genérico é de outro

**Estado:** INFRUTÍFERO

**Causa:** extraí um `git archive` do `HEAD` em `scratchpad/antes/` supondo o
rascunho só meu; ele já tinha uma árvore `antes/phxsql` de outra frente (datas
de 10:58, com `target/`), e o `tar -x` sobrescreveu o `crates/`, o
`Cargo.toml` e o `Cargo.lock` dela. O mesmo rascunho tinha `arvore-antes`,
`mutantes.py`, `provar.sh` e uma montagem minha esquecida em `mnt/`.

**Prevenção:** antes de escrever no rascunho, listar o que já está lá; e
trabalhar sempre num subdiretório com o número do pedido (`p498/`), nunca num
nome genérico como `antes/`, `mnt/` ou `prova.txt`. Montagem de teste se
desmonta no mesmo passo em que nasce.

## O que aconteceu

Pedido 498, na prova contra o SO: o executor de antes precisava da árvore do
`HEAD`. `mkdir -p scratchpad/antes && tar -x ...` não reclamou de nada — o
diretório existia e o `tar` sobrescreve em silêncio.

## O que eu concluí primeiro, e estava errado

Que o rascunho da sessão era isolado por agente, como a cópia de trabalho. Não
é: o caminho é o mesmo para todas as frentes da sessão.

## O que a medição disse

O `ls` depois do estrago mostrou 50 entradas no rascunho que não eram minhas,
e o `antes/phxsql` com `target/` e 25 itens, contra os 3 do meu `git archive`.

## A regra

No rascunho compartilhado, escreva só em subdiretório com o nome do pedido.

## Como está guardado hoje

Não está: é disciplina, sem guarda. O dano foi dito ao integrador no
relatório do 498 — a árvore `scratchpad/antes/phxsql` de outra frente tem
hoje o `crates/` do `HEAD` `ea7c2a2b`.
