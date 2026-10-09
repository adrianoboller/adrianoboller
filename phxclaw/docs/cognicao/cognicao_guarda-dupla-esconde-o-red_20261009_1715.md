# Guarda dupla para a mesma decisão esconde o RED

**Estado:** FRUTÍFERO

**Evidência:** `tests/desktop/ui_inspecao.mjs`, caso «desktop com politica desligada». Com o
desktop protegido em DOIS lugares do `inspecao.js` (não perguntar a política E `desligar()` recusar
no desktop), tirar um deles deu **106/106** — o RED não caiu. Com a decisão num lugar só (a
pergunta da política), tirar o `noDesktop ||` deu **101/110** (9 checagens do caso desktop caem);
reposto, 110/110.

## O que aconteceu

O bloqueio do inspetor tem de valer sempre no desktop (Tauri) e obedecer `ui.bloquear_inspecao`
no navegador. Escrevi a regra duas vezes, «por segurança»: no ponto que pergunta a política e no
`desligar()`. A prova reposta tirou uma das duas e continuou verde, porque a outra segurava.

## O que eu concluí primeiro, e estava errado

Que a segunda guarda era cinto e suspensório de graça. Não é: guarda que nenhum teste consegue
derrubar sozinha é guarda que ninguém sabe se ainda funciona, e a primeira limpeza de «código
repetido» tira a que o teste não vê. É a lei «função e comando vêm do mesmo motor» no tamanho de
um `if`.

## A regra

A decisão mora num ponto só, e o RED reposto tem de derrubar o teste. Se tirar a guarda não
reprova nada, há uma segunda guarda escondida — ou a guarda não guarda nada.

## Como está guardado hoje

`apps/phxclaw-ui/assets/inspecao.js`: o desktop decide na pergunta da política (comentário «num
lugar so»); `desligar()` não conhece o desktop. As três reposições estão no fim do
`ui_inspecao.mjs`, com o número de cada uma.
