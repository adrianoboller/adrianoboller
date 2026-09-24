# Juiz local pequeno não substitui o Claude, e o modelo maior não foi o melhor

**Estado:** PENDENTE

## O que aconteceu

A frente Ollama do PhxJev (pedido do dono, 24/09/2026) montou o juiz local do
fonte (`plugins/phxjev/bancada/montar-ollama.sh`) e o mediu contra o juiz
Claude nas mesmas 12 perguntas com verdade conferida no código ou medida
(`plugins/phxjev/bancada/casos.json`, `resultados.json`). Brier / acerto /
latência mediana: Claude 0,003 / 11 de 11 / ~200 s por comando; qwen2.5 1.5B
0,315 / 8 de 12 / 0,5 s; 3B 0,167 / 10 de 12 / 6,5 s; 7B 0,271 / 8 de 12 / 14,2 s.

## O que eu concluí primeiro, e estava errado

Que o tamanho resolveria: se o 1.5B errava, o 7B acertaria. O 7B ficou atrás
do 3B. E, antes disso, que o Claude acertava 91% — era a bancada comparando o
rótulo do `casos.json` com o rótulo que o juiz escolheu (`H1_...`) e pegando o
veredito do 321 sem desfecho. Com o desfecho como verdade: 11 de 11.

## O que a medição disse

As duas perguntas `ja_tratado` cuja verdade é «sim» (o conserto está no
trecho): 1.5B e 3B deram p ≈ 0,00 nas duas, o 7B errou uma. Confiança máxima
no erro é o pior perfil possível para um juiz cujo valor é saber quando
escalar. Com n = 12, a ordem entre os três locais é ruído; a distância para o
Claude (Brier 50× a 100× maior) não é.

A comparação **não é trabalho igual**, e está escrita no `resultados.json`: o
Claude leu o código com ferramentas; o local recebe só o trecho.

## A regra

Juiz local só entra no modo `auto` depois que a bancada, com n ≥ 50, der Brier
abaixo de 0,10 **e** nenhuma resposta errada acima de 0,9.

## Como está guardado hoje

Pela bancada (`comparar.py`, reproduzível) e pela ordem de não ligar o `auto`
escrita na skill (§8). Buraco: com 12 perguntas, «nenhum serve» é medido, mas
«o 3B é o melhor local» não é — precisa de mais casos antes de virar escolha.
