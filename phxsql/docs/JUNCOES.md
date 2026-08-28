# Junções e união

As sete figuras do diagrama clássico, mais `UNION` e `UNION ALL`.

Na tela não se escolhe por nome: clica-se no desenho de Venn, e o SQL
equivalente fica escrito embaixo de cada um. Quem sabe que quer «tudo de A e o
que casar de B» nem sempre lembra que isso se chama `LEFT JOIN`, mas reconhece
o desenho na hora.

| Figura | Operação | SQL equivalente |
|---|---|---|
| A ∩ B | `interna` | `INNER JOIN` |
| A inteiro | `esquerda` | `LEFT JOIN` |
| B inteiro | `direita` | `RIGHT JOIN` |
| A ∪ B | `completa` | `FULL OUTER JOIN` |
| A − B | `so_esquerda` | `LEFT JOIN … WHERE B.chave IS NULL` |
| B − A | `so_direita` | `RIGHT JOIN … WHERE A.chave IS NULL` |
| (A ∪ B) − (A ∩ B) | `so_dos_lados` | `FULL OUTER JOIN … WHERE A.chave IS NULL OR B.chave IS NULL` |

## Cinco modos, e não sete

`direita` é `esquerda` com os lados trocados, e `so_direita` é `so_esquerda`
com os lados trocados. Escrever os sete daria dois caminhos a mais para o mesmo
defeito aparecer.

A troca não é só economia de código: **ela decide qual tabela cabe na
memória**. O lado que a junção precisa inteiro — o `A` do `LEFT` — é o que se lê
linha a linha; o outro vira mapa. Num `RIGHT JOIN` de uma tabela enorme contra
um cadastro pequeno, trocar é o que faz o cadastro ser o mapa.

As colunas saem na ordem que o pedido pediu de qualquer jeito: quem escreveu
`A RIGHT JOIN B` quer ver A antes de B, mesmo que B seja o lado que streama.

## Três armadilhas do SQL que o motor respeita

### NULO nunca casa com NULO

Em SQL, `A.chave = B.chave` com um dos lados nulo não dá falso: dá
*desconhecido*, e a linha não casa. Uma linha de A com chave nula se comporta
como linha **sem par** — aparece no `LEFT`, some no `INNER`, e aparece no
`so_esquerda`.

Não é detalhe. Tratar nulo como um valor faria todas as linhas sem chave de A
casarem com todas as sem chave de B, e o resultado explodiria em produto
cartesiano com cara de junção.

O resultado conta e devolve quantas linhas de cada lado tinham chave nula
(`chave_nula_a`, `chave_nula_b`), e a tela mostra o aviso quando há alguma: um
`INNER` que trouxe menos do que se esperava costuma ter aí a explicação.

### Família errada é recusada na entrada

Juntar um `Int` com um `Str` não daria erro nenhum — daria **zero linhas**, que
é o pior resultado possível porque parece resposta. A conferência acontece
antes de ler qualquer linha, e a mensagem nomeia as duas colunas e as duas
famílias.

As famílias são: `booleano`, `numero` (todos os inteiros, `Real`, `Decimal` e
`Sequence`), `data`, `hora`, `instante`, `texto` (`Str` e `Memo`), `uuid`,
`uuid256`. Coluna binária não serve de chave: ela mora no `.bin`, e comparar
dois blocos inteiros custaria uma leitura a mais por comparação.

### Decimal casa por valor, não por escala

`12,34` com escala 2 e `12,3400` com escala 4 são o mesmo número e têm `i128`
diferente. A chave de comparação normaliza, senão as duas tabelas não casariam
por um zero à direita. Pela mesma razão o inteiro `12` casa com o decimal
`12,00`.

## Chave repetida multiplica

Junção não é consulta: se a chave `7` aparece três vezes em B, cada linha de A
com chave `7` produz três linhas. É o comportamento certo, e é também como uma
junção descuidada vira milhões de linhas — por isso há teto, e o resultado
traz `truncado: true` em vez de cortar calado.

## Chave composta

`chave` aceita uma coluna ou uma lista. A comparação é par a par, na ordem, e as
duas listas precisam ter o mesmo tamanho. O separador entre as partes impede
que `("ab","c")` case com `("a","bc")`.

Sem `chave`, a chave primária da tabela é usada. Sem chave primária, a operação
recusa em vez de chutar a primeira coluna — chutar daria número errado calado.

## União

```json
{"op":"unir", "database":"loja", "modo":"distinta",
 "tabelas":["clientes", "filial.clientes"]}
```

`distinta` é o `UNION` (tira as repetidas); `tudo` é o `UNION ALL`.

**Empilhar é por posição, e não por nome.** A primeira coluna de uma parte cai
na primeira da outra, e é o *tipo* que precisa bater — o nome sai da primeira
parte, como no SQL. Casar por nome pareceria mais amigável e seria uma
armadilha: duas tabelas com as mesmas colunas em ordem diferente empilhariam
trocando os valores, caladas.

No `UNION`, duas linhas todas nulas contam como repetidas — diferente da
junção, onde nulo nunca casa. As duas regras são do SQL, e são mesmo
diferentes: a junção compara *chaves*, a união compara *linhas*.

## As operações

| Operação | O que faz |
|---|---|
| `juntar` (`join`) | as sete figuras, entre duas tabelas do mesmo banco |
| `unir` (`union`) | empilha duas ou mais tabelas do mesmo banco |

As duas exigem `ler` no banco, e conferem de novo antes de abrir a segunda
tabela.

```json
{"op":"juntar", "database":"loja", "tipo":"esquerda",
 "a":{"tabela":"clientes",        "chave":"id",         "prefixo":"c"},
 "b":{"tabela":"filial.pedidos",  "chave":"cliente_id", "prefixo":"p"}}
```

O `prefixo` desambigua os nomes na saída: `clientes` e `pedidos` costumam ter
os dois uma coluna `id`, e sem prefixo a segunda apagaria a primeira em
qualquer mapa por nome — que é o que a grade da tela usa. Os dois lados com o
mesmo prefixo é erro.

## O que ainda não existe

- **Junção de mais de duas tabelas numa chamada.** Duas por vez.
- **Condição de junção que não seja igualdade.** `ON a.x > b.y` não existe: o
  *hash join* casa por igualdade, e desigualdade pede outro algoritmo.
- **`WHERE` sobre o resultado.** A tela filtra depois, na grade; o servidor
  ainda não.
- **`INTERSECT` e `EXCEPT`.** `so_esquerda` já é o `EXCEPT` por chave, e
  `interna` é o `INTERSECT` por chave — mas sobre a *linha inteira*, como o SQL
  faz, ainda não existem.
- **SQL escrito à mão.** Não há analisador; estas são operações do protocolo.
  A camada SQL continua no roteiro.
