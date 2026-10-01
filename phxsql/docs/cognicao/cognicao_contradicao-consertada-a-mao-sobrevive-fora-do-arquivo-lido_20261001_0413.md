# Contradição consertada à mão sobrevive no arquivo que ninguém releu

**Estado:** INFRUTÍFERO
**Causa:** o conserto das seis contradições do parecer (commit `2c77e6fe`) foi feito por leitura humana das frases que o parecer citou, e a busca parou nas redações citadas; a mesma capacidade dita com outra forma («traduz um `SELECT` simples» no `FORMATO.md`, «Compactação … | pendente» numa tabela do `README.md`) não era uma das frases citadas e ficou.
**Prevenção:** conserto de contradição de contrato fecha com `python3 docs/dossie/catraca-prosa-x-celula.py` verde, e não com a lista do parecer riscada; a catraca roda no fecho do `portao-dos-geradores.py` e no `bancada/catracas/todas.py`.

## O que aconteceu

A linha do pedido 335 registrou em 01/10/2026 as seis contradições
«conferidas contra o dossiê de hoje», com a compactação «corrigida nos quatro
lugares». A primeira corrida da catraca prosa × célula, na mesma data, achou
**três** contradições vivas: `docs/FORMATO.md` «a op `sql` traduz um `SELECT`
simples» e «o que falta é … a expressão em `WHERE`» (o comparativo mede `cte`,
`subconsulta`, `funcao_de_janela` e `expressao_no_where` como `tem`), e
`README.md` com «Compactação, modo exclusivo, TLS | pendente» (a célula é
recusada).

## O que eu concluí primeiro, e estava errado

Que bastava a catraca guardar o futuro: o dossiê de hoje passaria verde de
primeira, porque a rodada anterior dizia as seis resolvidas. Passou o dossiê;
os outros dois arquivos do contrato, não.

## O que a medição disse

3 achados em 3 arquivos lidos, 0 deles no dossiê; 1 citação histórica aceita
(dossiê, «a chave estrangeira é catálogo. Não é mais»). Depois do conserto à
mão: 0 achados; `--autoteste` com 5 frases antigas repostas, 5 derrubadas.
Reproduzido de novo em árvore limpa no HEAD `83a29eb0`: os mesmos 3 achados.

## A regra

Conserto de contradição se confere pela célula, em todos os arquivos que a
afirmam, e não pela frase que o revisor citou.

## Como está guardado hoje

`docs/dossie/catraca-prosa-x-celula.py` com o léxico em
`docs/dossie/contrato-das-capacidades.json`. O buraco que fica: redação que
nenhum sinal do léxico prevê, e arquivo fora dos três lidos.
