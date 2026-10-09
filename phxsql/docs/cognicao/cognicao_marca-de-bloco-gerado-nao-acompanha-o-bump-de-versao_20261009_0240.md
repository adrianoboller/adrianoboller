# A marca do bloco gerado não acompanha o bump de versão — o gerador escreveu a 0.20.0 dentro da 0.19.0 e disse «gravado»

**Estado:** PENDENTE

- **Quando:** 2026-10-09, 02:40 UTC (fecho da 0.20.0, HEAD `9a25642f`)
- **Onde:** `docs/versao/intervalo-da-versao.py`, `CHANGELOG.md`

## O que aconteceu

`python3 docs/versao/intervalo-da-versao.py --gravar` imprimiu «gravado: 490
commits sobre a 0.19.0» e saiu 0. O bloco gravado estava na seção `## 0.19.0`,
porque as marcas `<!-- GERADO: intervalo-da-versao.py -->` tinham nascido ali
quando a 0.19.0 era a corrente. Depois do bump para 0.20.0 a seção velha passou
a dizer «490 commits sobre a 0.19.0», e o resumo da `## 0.20.0` apontava para
«o bloco gerado da seção `## 0.20.0`», que não existia. A mesma seção velha
ainda dizia **NÃO SELADA**, falso desde 23/09 (`805fb34`).

## O que eu concluí primeiro, e estava errado

Que o gerador estava certo porque os números estavam certos (490 batia com
`git rev-list --count 805fb34..HEAD`). O número estava certo; o **lugar** não.
O gerador procurava a marca onde quer que ela estivesse, e a posição da marca é
parte da receita — uma receita digitada uma vez, que envelheceu no bump.

## O que a medição disse

- Bloco da corrente (0.20.0): 490 commits sobre a 0.19.0, 2 desde o selo `1993bce`.
- Intervalo fechado da 0.19.0, agora gerado: `baff46e..805fb34` = **895**
  commits (a prosa de 23/09 dizia 894, medido e digitado naquele dia).

## A regra

Bloco gerado que pertence a uma seção se mede para a seção em que está; o
gerador confere o lugar, e quando o cria ou o move, diz que criou.

## Como está guardado hoje

O gerador mede cada bloco pela seção: na corrente contra o HEAD; numa versão
selada, o intervalo fechado (não muda mais). Se a corrente não tem bloco, ele
nasce abaixo do título e a saída diz «NASCEU». **Não há teste** que reponha a
marca na seção errada e confira a saída; o gerador está fora do `PLANO` do
portão de propósito (muda a cada commit).
