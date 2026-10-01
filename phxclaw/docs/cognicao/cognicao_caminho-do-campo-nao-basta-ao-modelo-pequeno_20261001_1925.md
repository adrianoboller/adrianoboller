# O caminho do campo não bastou ao modelo pequeno; a descrição do campo sim

**Estado:** PENDENTE (medido uma vez, N=3 rodadas × 5 casos; promove com uma segunda corrida
que repita o 3 de 8 ou com o modelo de 3B)

## O que aconteceu

A SP000028 pôs no portão o validador de esquema que devolve ao modelo todos os erros em JSON,
cada um com o caminho do campo, o que veio e o esperado. Medido com qwen2.5:1.5b
(`phxclaw avaliar`, 5 casos × 3 rodadas, 01/10/2026), o erro dominante era o mesmo nas três
versões: `create_document`/`create_presentation` **sem `path`**.

## O que eu concluí primeiro, e estava errado

Que o caminho do campo (`{"field":"path","got":"missing","expected":"required field (string)"}`)
bastava para o modelo consertar — era a hipótese do PydanticAI e da triagem. Medido: **0 de 10**
tarefas com chamada inválida tiveram o conserto na chamada seguinte; o 1.5b repetiu a chamada
idêntica até o corte de repetição.

## O que a medição disse

Com a `description` do esquema junto do campo ausente (`required field (string): relative path
ending in .pptx`), **3 de 8** tarefas consertaram na tentativa seguinte — 1 por rodada, nas três
(faixa 1–1 contra 0–0 antes: não se cruzam). Antes do portão: 0 de 5. A taxa de argumento
inválido e o acerto por rodada continuaram com faixas cruzadas (sem vencedor).

## A regra

Para o campo que FALTA, o modelo não tem nada do que mandou para ler; o erro tem de trazer o que
pôr ali, não só onde. Mensagem de erro ao modelo se mede por **conserto na tentativa seguinte**,
não por taxa de erro: a taxa sobe quando o modelo passa a tentar mais.
