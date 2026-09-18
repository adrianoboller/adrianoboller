# A guarda cobria o MOTIVO, e a porta irmã ficou aberta

18/09/2026, 17:55. Papel A (integração), fechando a leva dos pedidos 358, 366,
367, 370, 373 e 376.

## 1. O que aconteceu

Acrescentei quatro pedidos ao `docs/PENDENCIAS.md` (377–380) e marquei dois
como feitos (366 e 376). Rodei o gerador:

```
369 pedidos: 278 feitos, 14 parciais, 77 planejados
```

O **mesmo número de antes** de eu acrescentar os quatro. E com três linhas de
êxito: página gravada, painel do dossiê regravado, contagem gravada de volta.

O arquivo tinha **380** linhas de pedido. O gerador via **369** — e não dizia
que via menos.

Esse gerador **já tinha** uma guarda contra exatamente este estrago, escrita
quando o pedido 150 passou meses invisível por causa de um `⏳` que não estava
na legenda. A guarda existia, estava certa, e não pegou nada: nestas onze
linhas o símbolo de estado estava **certo**.

## 2. O que eu concluí primeiro, e estava errado

**Três vezes seguidas**, e as três do mesmo naipe — diagnóstico plausível que
eu quase escrevi antes de medir:

1. **«É o `☑️`.»** Suspeitei do variation selector U+FE0F: símbolo que parece
   igual e não casa. Plausível, e errado. Medido linha a linha, o `☑️` casava
   — o que faltava era o **pipe de fechamento**. Seis linhas nasceram sem ele;
   cinco outras porque o meu próprio script de fecho escreveu o
   `**FECHADO em 18/09/2026…**` como **quinta coluna**, sem fechar.

2. **«Conta os pipes.»** Escrevi a guarda nova contando pipes. Ela parou o
   leitor — e a mensagem saiu mentindo: *«5 pipes onde precisam ser 5»*. É que
   a linha com quinta coluna e sem fecho tem **cinco** pipes, iguaizinha à
   linha certa. O que separa as duas é a **célula depois do último pipe**:
   vazia na certa, com texto na torta. Guarda que para pelo motivo errado
   ensina o motivo errado a quem for consertar.

3. **A prova passou por engano.** Escrevi a prova real com três defeitos
   repostos, deu verde, e arranquei a guarda para medir o vermelho: **só um
   dos três caiu**. Os outros dois continuavam sendo pegos por guardas
   **velhas** — porque eu tinha escrito a linha da quinta coluna **com** o
   pipe final, e assim ela casa o regex e cai na guarda do pipe cru. As cinco
   linhas reais não tinham fecho nenhum. Eu tinha reproduzido um defeito
   *parecido* com o de origem, não o de origem.

## 3. O que a medição disse

| medida | número |
|---|---:|
| linhas de pedido no `PENDENCIAS.md` | 380 |
| pedidos que o gerador contava | 369 |
| invisíveis | **11** |
| — sem o pipe de fechamento | 6 |
| — quinta coluna **e** sem fecho | 5 |
| linhas de êxito que ele imprimia mesmo assim | 3 |

Depois do conserto: `380 pedidos: 284 feitos, 14 parciais, 82 planejados` — e
a conta fecha sozinha: +6 feitos (358, 366, 367, 370, 373, 376) e +5
planejados (375, 377, 378, 379, 380).

E a divisão de trabalho entre as guardas, medida arrancando a nova e rodando a
prova: dos quatro caminhos catalogados, a guarda nova é a única que pega
**dois**; os outros dois já eram das velhas.

## 4. A regra

**Guarda se escreve contra o EFEITO, não contra o motivo — porque o efeito é
um só e os motivos são muitos.** Aqui o efeito é «pedido que existe no arquivo
e não existe na página», e há quatro caminhos até ele: símbolo fora da
legenda, pipe cru no meio, coluna a mais, e fecho que falta. A guarda do 150
cobria um motivo; os outros três continuaram abertos por meses.

E o corolário, que é o do gerador que faz menos do que promete: **quem conta
tem de dizer contra o quê contou.** Um gerador que lê 380 linhas e devolve 369
sem uma palavra é a mesma família do `pagina-dos-pedidos.py` que gravava a
página e pulava o painel — êxito anunciado sobre trabalho pela metade.

## 5. Como está guardado hoje

- **A guarda**, em `docs/dossie/pagina-dos-pedidos.py`: linha com número de
  pedido e estado **da legenda** que mesmo assim não casa a forma **para** o
  leitor, nomeando a linha, o pedido e a causa medida (célula depois do último
  pipe, ou número de colunas).
- **A prova real**, em `docs/dossie/prova-do-leitor-de-pedidos.py`: o arquivo
  são passa e os **quatro** caminhos repostos param o leitor. Cada caso traz,
  no próprio catálogo, o pedido que o pagou e qual guarda o pega.
- **O portão** (`portao-dos-geradores.py`) passou a rodar essa prova. O que
  ele já fazia era reprovar derivado velho — e foi por isso que ele ficou
  verde o tempo todo: o derivado estava em dia com um leitor que contava
  menos. Portão que só confere o resultado não vê a guarda que parou de
  guardar.
