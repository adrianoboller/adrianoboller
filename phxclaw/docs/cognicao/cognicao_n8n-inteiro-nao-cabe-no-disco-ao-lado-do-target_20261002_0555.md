# O n8n inteiro não cabe no disco ao lado do `target/` do workspace

**Estado:** INFRUTÍFERO (a prova real com n8n de verdade não rodou; causa e prevenção abaixo)

## O que aconteceu

SP000033 pedia subir um n8n real (`npm i n8n`) numa pasta do scratchpad, medindo o tamanho
antes e parando abaixo de 1 GB livre. Antes: 4.335 MB livres. `npm install n8n@2.41.5
--ignore-scripts` chegou a **2.162 MB em `node_modules` + 1.245 MB no cache do npm**, ainda
incompleto, com **436 MB** livres (um `cargo check` concorrente também gastou). Interrompi e
apaguei; o disco voltou a 3.528 MB.

## O que eu concluí primeiro, e estava errado

Que medir antes era `npm view n8n dist.unpackedSize` (26 MB): o pacote é pequeno. O que pesa
são as 156 dependências diretas e a árvore delas — o número do registro não diz nada sobre o
que a instalação ocupa, e eu só vi o tamanho real quando o disco já estava no limite.

## O que a medição disse

Pelo menos 3,4 GB consumidos sem terminar; o n8n completo precisa de mais que isso, e o
`target/` do workspace ocupa o resto do disco. Não há como ter os dois aqui.

## A regra

Instalação grande de terceiro se mede por tentativa com teto (laço de `df`), não pelo tamanho
do pacote; e a prova fica NÃO VALIDADA, escrita com o número, em vez de "testado" por
inferência.

## Prevenção

Antes de instalar algo com mais de ~50 dependências diretas: `df` em laço no fundo com
interrupção automática no piso; e os testes Rust contra um servidor falso com as respostas da
documentação ficam escritos ANTES da tentativa, para a entrega não depender dela.
