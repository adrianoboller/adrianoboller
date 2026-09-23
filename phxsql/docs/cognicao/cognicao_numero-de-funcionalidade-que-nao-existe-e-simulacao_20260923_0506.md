# Número de funcionalidade que ainda NÃO existe é simulação, e nasce inflado

**Descoberto em 23/09/2026, 05:06** — frente U (segunda volta), pedido 393.

## 1. O que aconteceu

O pedido 393 entrou no `PENDENCIAS.md`, no dossiê e no título do commit com
**118,7×**: unir as tabelas inteiras e filtrar fora custaria 230,3 ms
(219,4–266,8) contra 1,9 ms (1,9–2,4) filtrando dentro de cada braço. O número
estava medido, com faixa min–max e faixas que não se cruzavam — tudo o que
esta casa exige.

Implementado o braço, remedi. **Deu 64,2×** (171,4 ms contra 2,7 ms). O número
que justificou a frente caiu para pouco mais da metade.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a máquina estava mais carregada, ou que as duas colunas de sistema
novas da frente P (`rowstamp` e `rowtime`) tinham encarecido o lado (a). As
duas hipóteses morrem no próprio número: o lado (a) ficou **mais barato**
(230,3 → 171,4 ms), não mais caro — se fosse carga ou colunas a mais, ele teria
subido.

## 3. O que a medição disse

Quem mudou foi o lado **(b)**: 1,9 → 2,7 ms, +42%. E o motivo está no código da
sonda antiga (`sonda-perna.py`): ali o lado filtrado eram **dois `buscar`
soltos**, somados pelo Python — porque o braço não existia para ser medido.

O braço de verdade paga, por cima das mesmas duas buscas, a máquina da união:
empilhar posição a posição, canonizar a chave do `distinta`, trazer cada linha
JSON de volta para `Value` **por nome**, e o guarda de profundidade. Quatro
corridas independentes depois do conserto: 63,3 · 68,2 · 64,5 · 64,2 — estável,
e nunca perto de 118.

A conclusão **não** muda (o braço-pedido vale, e as faixas continuam sem se
cruzar); muda o tamanho, e muda quem pode citá-lo.

## 4. A regra

**Número que justifica uma funcionalidade que ainda não existe mede uma
SIMULAÇÃO, e a simulação não paga a máquina que a operação vai pagar. Ele entra
com a etiqueta de estimativa, e a implementação REMEDE contra a operação — no
mesmo passo, antes de fechar o pedido.**

É a lei «bancada compara trabalho igual, não só pergunta igual» com um alcance
que ela ainda não tinha: os dois erros que a fundaram comparavam **nosso motor
contra outro**; este compara **o nosso planejado contra o nosso de hoje**, e o
lado que não existe é sempre o que sai barato demais.

## 5. Como está guardado hoje

Nasceu a `bancada/uniao/medir.py`, que confere que as duas respostas são as
**mesmas linhas** antes de comparar tempo e grava `resultados.json` com a data
dentro. O `docs/JUNCOES.md` e o cabeçalho do `op_unir` citam o número **de lá**,
e dizem, com o motivo, que o 118,7× não se reproduziu.

**O buraco que fica:** o `PENDENCIAS.md`, o `pedidos.html`, o
`status-do-projeto.html` e o título do commit `709ed12` ainda dizem 118,7× — e
o título do commit não se conserta. Só o integrador mexe naqueles arquivos, e
está pedido no relatório da frente. A bancada **não** está declarada na tabela
do `docs/dossie/pagina-dos-testes.py`, então ela ainda não aparece como
«não medida» nem como medida: essa declaração é do papel H.
