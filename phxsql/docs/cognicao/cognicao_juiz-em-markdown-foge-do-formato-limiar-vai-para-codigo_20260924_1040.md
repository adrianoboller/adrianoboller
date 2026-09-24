# Juiz em markdown acerta a resposta e foge do formato: limiar e formato vão para código

**Estado:** INFRUTÍFERO
**Causa:** a primeira versão do PhxJev deixava ao próprio juiz (o agente, lendo a skill) calcular a confiança, aplicar os limiares e escrever o bloco de saída; instrução em prosa não amarra formato nem aritmética.
**Prevenção:** o juiz só emite probabilidades em JSON; confiança, limiar, veredito e registro saem de `plugins/phxjev/scripts/phxjev.py`, que recusa JSON inválido e é provado por `plugins/phxjev/scripts/teste_phxjev.py`.

## O que aconteceu

Exercitado ao vivo (`claude --plugin-dir plugins/phxjev -p "/phxjev-perguntar noul ..."`),
o juiz respondeu certo — `nao 0,98`, com `reg.rs:17-20` e `reg.rs:2026` citados —
e entregou um bloco YAML próprio: `conf: alto` no lugar de número, sem a linha
`calibracao: nao medida` que a skill mandava em toda saída.

## O que eu concluí primeiro, e estava errado

Que uma tabela de limiares escrita na skill, «aplicada literalmente», bastava —
porque o pedido era um plugin em markdown. O Jev aplica limiar em código puro
por um motivo, e eu copiei o contrato sem copiar esse motivo.

## O que a medição disse

1 corrida antes: formato violado em 2 pontos. 1 corrida depois, mesma
pergunta, saída passando pelo script: cabeçalho, conf numérica e registro
corretos. 13 testes do script verdes; com o limiar do `real` zerado e a
exigência de evidência desligada, 2 falham. Uma corrida de cada lado é
anedota para o comportamento do juiz — por isso o estado da **correção** não
é FRUTÍFERO ainda; o que se registra aqui é a falha.

## A regra

Instrução em prosa decide o quê; formato, conta e limiar se aplicam em código.

## Como está guardado hoje

Pelo script e pelos testes dele. Buraco: nada impede o juiz de editar a saída
do script antes de mostrá-la — a skill manda copiar sem editar, e isso é prosa
de novo.
