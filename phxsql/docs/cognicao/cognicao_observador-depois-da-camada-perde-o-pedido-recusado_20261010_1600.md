# Observador posto depois da camada de proteção perde justamente o pedido recusado

**Estado:** PENDENTE

- **Quando:** 2026-10-10, 16:00
- **Onde:** `crates/phxsql-server/src/servidor/servico_permissao_01.rs`
  (`executar_e_contar_escrita_local`), `crates/phxsql-server/src/perfis.rs`
- **Pedido:** 765, fatia P7

## O que aconteceu

O perfil habitual (P7) entrou primeiro no fim do `executar_e_contar_escrita_local`,
ao lado do observador de injeção, «depois do trabalho, com o desfecho na mão».
Só que a primeira linha da função é `self.protecao_do_pedido(...)?`: o comando
da lista de perigo sem a sessão liberada volta ali, com `?`, e nunca chega ao fim.

## O que eu concluí primeiro, e estava errado

Que o lugar certo de um observador é depois do `executar`, como o do 495 — «lá se
vê o desfecho dos dois lados». Para o observador de injeção é verdade, porque
ele precisa das classes que o `executar` entrega. Para o perfil, o desfecho não
importa, e o `?` de cima fazia o perfil **nunca ver** o `excluir_tabela`
recusado — que é o pedido mais fora do hábito que existe.

## O que a medição disse

Lido no código, não medido em número: entre o topo da função e o ponto do
observador há um retorno antecipado (`?` da camada) e o perfil só registraria o
que passou por ela. Movido para antes da camada, o aceite (300 leituras e
`excluir` em `folha`) segue verde, e o comando recusado passa a entrar no perfil.

## A regra

Antes de pendurar um observador no fim de uma função, procure os `?` e os
`return` entre o topo e ele: o que sai cedo é o que ele não vai ver.

## Como está guardado hoje

O comentário no ponto da chamada diz por que ela vem antes da camada, e o teste
`o_comando_perigoso_recusado_tambem_sai_do_perfil` (`excluir_tabela` recusado
com 4009 → uma `ForaDoPerfil`) é a guarda `perfil-depois-da-camada` do
catálogo, que repõe a ordem antiga. Fica PENDENTE até a corrida do provador
ficar registrada no `ultima-corrida.json` integrado.
