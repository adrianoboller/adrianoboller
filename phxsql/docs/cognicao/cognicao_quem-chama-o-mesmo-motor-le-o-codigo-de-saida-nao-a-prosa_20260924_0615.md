# Quem chama o mesmo motor lê o código de saída, não a prosa

Papel A (integração), 24/09/2026. Defeito achado na integração do pedido 476, antes do commit.

## 1. O que aconteceu

O pedido 476 criou o `bancada/catracas/todas.py`, um comando só para todas as
catracas em Python, e trocou os dois chamadores que tinham lista própria: o
`comunicacao.sh` e o item 0 da `bancada/bateria/prova-bateria.py`. Os dois passaram a
chamar o mesmo motor, mas cada um decidia o veredito de um jeito:

- o `comunicacao.sh` decidia pelo código de saída («nunca a prosa», escrito no
  comentário da própria frente);
- a bateria lia a prosa linha a linha, com um casador que exige dois espaços antes do
  caminho, e **não conferia o código de saída**.

O `todas.py` alinha o estado em 32 colunas. Um `QUEBRADA (medidor caiu: ...)` mais
longo que isso sai com um espaço só, não casa, e a catraca caída sumia da bateria.

## 2. O que eu concluí primeiro, e estava errado

Que «mesmo motor, nunca duas listas» fechava o assunto: se os dois chamadores rodam
o mesmo comando, não teriam como divergir. Divergem na **leitura** da resposta. A lei
unificou quem pergunta e deixou duplicado quem interpreta.

## 3. O que a medição disse

Numa cópia da árvore do commit, com uma régua que levanta `RuntimeError` de mensagem
longa:

- a bateria como a frente entregou saiu com `FALHAS: []`, ou seja, verde com uma catraca
  quebrada;
- com `confere(..., r.returncode, 0)` acrescentado, a mesma cópia dá
  `FALHAS: ['o \`todas.py\` sai 0 -- nenhuma catraca reprova nem quebra']`;
- na árvore limpa, `FALHAS: []`.

O `todas.py` acertou nos três casos (`rc=1`). Quem errou foi o chamador.

## 4. A regra

**Quem chama um motor decide pelo código de saída dele; o texto só serve para NOMEAR o
que falhou.** Um casador de prosa que decide o veredito é uma segunda implementação
do motor, e ela diverge da primeira no primeiro formato que ninguém previu.

É o alcance da lei «funções e comandos vêm do mesmo motor»: ela vale também para a
leitura da resposta, e não só para a pergunta.

## 5. Como está guardado hoje

O item 0 da bateria confere o código de saída do `todas.py`, e a prova acima está no
commit do 476. O casador de texto continua na bateria, só para dar um rótulo a cada
catraca. Não há régua que ache outro chamador lendo prosa no lugar do código de
saída: isso se acha procurando o chamador irmão, como na lei do caminho irmão.
