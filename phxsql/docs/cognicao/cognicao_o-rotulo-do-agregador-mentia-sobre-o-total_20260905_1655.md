# O rótulo do agregador mentia sobre o total — e o irmão já fazia certo

**05/09/2026, 16h55** (hora da descoberta, não a do commit)

## 1. O que aconteceu

Escrevendo o primeiro lote da bateria de botões (`testes-web/casos/21-botoes-da-grade.mjs`),
o passo do `.phx-th-agg` — o botão que alterna o agregador da coluna, SUM →
AVG → COUNT → MIN → MAX — reprovou na primeira corrida:

```
.phx-th-agg (alternar o agregador da coluna): o agregador foi de SUM para AVG
e o total geral nao mudou («total geral2.016R$ 293.770,502.016») -- rotulo sem efeito
```

O manipulador em `ui/grid/phx-grid.js` trocava `c.agregador`, trocava o texto
do próprio botão, chamava o `log(...)` — **e não repintava**. O cabeçalho
passava a dizer AVG e o rodapé continuava mostrando a SOMA, até alguém virar a
página por outro motivo.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o defeito era do meu teste: *o total geral deve estar num `tfoot`,
e eu li o seletor errado.* Plausível — eu tinha mesmo errado dois seletores
naquele mesmo arquivo (o rodapé de grupo é `.phx-grodape`, e não
`.phx-rodape-grupo`, que eu inventei).

Estava errado. O total geral é uma **linha do `tbody`** (`tr.phx-total-geral`),
o seletor corrigido achou-a, e o número continuou o mesmo depois do clique.
O defeito era da grade.

O erro tem nome nesta casa: **diagnóstico plausível não é diagnóstico medido**,
e o errado sobrevive melhor quando há um erro de verdade por perto para
justificá-lo.

## 3. O que a medição disse

Com 63 linhas e `limite` somando **R$ 293.770,50**: depois de trocar de SUM
para AVG, o rodapé continuava em `R$ 293.770,50`. A média seria ~R$ 4.663.

E o irmão, medido no mesmo arquivo: o botão «total por grupo»
(`[data-rodape]`), que mexe **no mesmo rodapé**, chama `carrega()` na linha
seguinte à que muda o estado. O caminho certo já existia a nove linhas de
distância.

## 4. A regra

**Rótulo que contradiz o número embaixo dele é mentira sobre o dado** — a
mesma lei do «Blumenau» que aparecia «BLUMENAU». Botão que muda como um número
é calculado repinta o número, e o passo que o prova lê o **número**, nunca o
rótulo.

## 5. Como está guardado hoje

- O conserto é uma linha (`carrega();`) em `ui/grid/phx-grid.js`, com o
  comentário nomeando o irmão que já fazia certo.
- A guarda é o passo `.phx-th-agg` de `testes-web/casos/21-botoes-da-grade.mjs`,
  que lê `tr.phx-total-geral` antes e depois do clique.
- **Prova real nos dois sentidos**: tirando o `carrega()`, o caso reprova com
  a frase acima **nomeando o botão**, e os outros dois lotes de botões
  continuam verdes — a delimitação é o que separa uma acusação de um alarme.

**Onde o buraco ficou:** o mesmo padrão — mudar o estado de uma coluna e não
repintar — pode existir em outros manipuladores da grade que o lote não
alcançou. O que se sabe hoje é que os 18 botões da grade exercitados repintam;
o `--example botoes-sem-prova` lista os que ninguém clicou ainda.
