# Texto de tela que envelhece diz o contrário da lei: só se acha lendo a tela com a lei na mão

**Estado:** PENDENTE

## O que aconteceu

Exercitando o cartão «Declarar chave estrangeira» do diagrama ER (pedido 190,
caso 45), a tela dizia, no cartão e na nota do diagrama, que a chave é
**«declarada, não imposta»** e que «uma filha órfã ainda entra». É o contrário
da decisão do dono («chave declarada NASCE conferida»): o motor recusa a filha
órfã, e o aviso de sucesso do **mesmo cartão** dizia «já conferida na
gravação». Na mesma tela, o menu «ao excluir a linha-mãe» oferecia
`cascata`, `anular` e `nada`, que o servidor recusa na declaração («nunca se
mata o pai que tem filhos»): três das quatro opções terminavam em erro.

Outros dois do mesmo naipe, na rodada: a ficha «versão» do «Sobre» saía sempre
«—», porque lia `ping.versao` e o servidor responde `ping.phxsql`; e o
`.then(() => irAba("estrutura"))` do nome da tabela na tela de dado pessoal
tomava o painel de novo e atropelava a tela pedida depois (a pintura tardia do
636, numa forma que o 636 não listou).

## O que eu concluí primeiro, e estava errado

Que o conferidor de textos (`textos-fora-da-fabrica`) cobria isto: ele conta
texto **cravado**, não texto **verdadeiro**. Um parágrafo que passou pela
fábrica nos seis idiomas continua mentindo nos seis. A catraca desce quando se
traduz, e nunca sobe quando a lei muda.

## O que a medição disse

O caso 45 reprova nos dois sentidos pela prova real (`prova-real-botoes.mjs`):
com o cartão repondo «declarada, não imposta» e com o menu repondo as quatro
opções. O caso 46 reprova com `p.versao` no lugar de `p.phxsql || p.versao`.
Nenhum dos quatro aparecia lendo o código sozinho: cada trecho estava
coerente consigo mesmo e incoerente com uma decisão tomada em outro arquivo.

## A regra

Quando uma pétrea ou uma decisão do dono muda o comportamento do motor,
**procure a tela que o descreve** — e exercite-a com a decisão na mão, lendo
o que ela diz e o que ela oferece. Texto de tela que descreve comportamento
tem dono, como o código tem.

## Como está guardado hoje

Os quatro estão consertados e travados pelos casos 42, 45 e 46. **Não está
guardado o caso geral:** nada varre as telas atrás de frases que contradizem
uma pétrea; o que há é o hábito de ler a tela com a lei aberta.
