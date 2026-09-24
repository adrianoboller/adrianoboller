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

**Isto vale em TODO caminho que junta valores de duas origens, e passou a valer
em 23/09/2026** (pedido 392). A régua não foi escolha desta casa: comparar
`DECIMAL` é pelo **valor**, e os quatro motores convergem — 4 de 4, aceite
automático. PostgreSQL: *«Numeric values are physically stored without any
extra leading or trailing zeroes. Thus, the declared precision and scale of a
column are maximums, not fixed allocations»*. MySQL e MariaDB: o `decimal_cmp`
de `strings/decimal.c` corta os zeros à direita **dentro da própria comparação**
antes de comparar dígito a dígito. SQLite: *«Numeric values are always compared
numerically»*.

O que cada caminho fazia antes, medido pelo protocolo em 23/09/2026 com
`valor Decimal(12,2)` de um lado e `Decimal(12,4)` do outro:

| caminho | antes | agora |
|---|---|---|
| `unir` (`tabelas` e `partes`) | `10,5000` saía **`1050,00`** e `10,50` saía **`0,1050`** | o tipo leva em conta todos os braços |
| `juntar` (tabela) | já casava por valor | igual |
| `diferencas`, a chave | a chave de `b` saía pela escala de `a`: `10,5000` publicado como **`1050,00`** | cada lado pela escala dele |
| `diferencas`, a linha | `7,25` e `0,0725` (o mesmo `i128`, 725) contavam como **`iguais`** | `diferentes` |
| `pivotar`, a tabela de consulta | `7,25` casava `0,0725`; `10,50` não casava `10,5000`; junção por `Date` **nunca** casava | casa por valor |
| `consultar` (junção, `IN`, `EXISTS`) | **zero linhas** onde o SQL devolve uma | casa |

**O que NÃO mudou, de propósito:** na composição do `consultar` um `Decimal`
continua **não** casando com um inteiro — a canonização só acontece quando os
dois lados são `Decimal`. É o contrato que o pedido 237 escreveu, e trocá-lo de
carona seria mudar o que ninguém pediu. Os dois motores de junção divergem
nesse ponto (o `juntar` de tabela casa `12` com `12,00`, a composição não), e a
divergência está registrada em teste em vez de consertada de lado.

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

### O braço pode ser um PEDIDO, e não só um nome de tabela

```json
{"op":"unir", "database":"loja", "modo":"distinta",
 "partes":[{"op":"buscar",   "tabela":"g1", "indice":"porUf", "chave":["SC"]},
           {"op":"consultar","de":{"op":"varrer","tabela":"g2"},
                             "expressao":"uf = 'SC'",
                             "colunas":[{"coluna":"nome"}]}]}
```

Cada item de `"partes"` é um pedido que devolve linhas — `varrer`, `buscar`,
`agrupar`, `group_by` ou `consultar` —, o **mesmo** contrato que o `consultar`
já compõe no `de`, no `juntar[].de`, no `escalar[].de`, no `em[].de` e no
`existe[].de`. A união é o sexto uso dele, e não uma gramática nova.

**Por que o braço deixou de ser um nome.** `SELECT nome FROM a WHERE uf='SC'
UNION SELECT nome FROM b` não tinha onde acontecer: a união abria as tabelas
INTEIRAS. **Remedido contra o braço de verdade** em 23/09/2026 pela
`bancada/uniao/medir.py` — três corridas de sete repetições, 2×20.000 linhas,
`uf='SC'` a 1%, 400 linhas na resposta, carga da máquina 0,98: unir inteiras e
filtrar fora materializa 40.000 e custa **171,4 ms** (160,6–185,6); filtrar
dentro de cada braço materializa 400 e custa **2,7 ms** (2,5–3,4) —
**64,2×**, com as faixas sem se cruzarem (62,5× · 64,2× · 67,6× nas três
corridas). O ganho é do **índice** que o braço pode usar e que unir-inteiras
estruturalmente não pode; ele é função da seletividade, e a 100% seria ~1×.
Os números saem do `bancada/uniao/resultados.json`, com a data dentro do
arquivo — nenhum deles se digita aqui de memória.

**E o 118,7× que o pedido 393 anunciava não se reproduziu — o motivo é lei da
casa.** Naquela medição o lado filtrado eram dois `buscar` **soltos**, porque o
braço ainda não existia para ser medido. O braço de verdade paga a máquina da
união por cima da busca: empilhar, canonizar a chave do `distinta`, trazer a
linha JSON de volta para `Value` por nome, o guarda de profundidade. É
*«bancada compara trabalho igual, não só pergunta igual»* cobrando o próprio
pedido — o número velho comparava uma **simulação** com a operação. Fica
registrado com o número, para não voltar sem medição.

**`"tabelas"` não sai, e continua querendo dizer o mesmo.** Guarda nova entra
pedida, não imposta. Mandar os dois campos no mesmo pedido **recusa**: são duas
formas de dizer a mesma coisa, e o motor não escolhe por quem pediu.

**As colunas do motor não empilham.** O `rowid` que o `varrer` põe na frente da
linha e as colunas de sistema que o esquema põe no fim ficam de fora dos dois
caminhos. Não é arrumação: elas são **únicas por linha**, e a chave do
`distinta` é a linha visível inteira — com elas dentro, o `UNION` devolveria o
mesmo que o `UNION ALL` anunciando `repetidas: 0`.

**O teto muda de alvo.** No caminho `tabelas` o `TETO_JUNCAO` (500.000) se
aplica à TABELA, então unir duas de dois milhões é impossível mesmo para pegar
dez linhas. No caminho `partes` o teto que vale é o `recursos.max_linhas` de
cada braço, e ele se aplica ao **recorte**.

**O que os braços-pedido CUSTAM, e é decisão do papel C (DBA).** O caminho
`tabelas` toma a trava global **exclusiva** e a segura por toda a
materialização. Remedido pela mesma bancada numa **janela fixa de 1,5 s** de
pressão contínua — sem janela fixa, «pior espera baixa» quer dizer só «a carga
acabou antes», e o caminho novo acaba antes —, com um `varrer(max=1)` numa
conexão vizinha:

| o leitor inocente | p95 | faixa | uniões na janela |
|---|---|---|---|
| sozinho | 0,39 ms | 0,38–0,40 | — |
| sob `unir` por `tabelas` | **115,33 ms** | 114,54–117,99 | 11 |
| sob `unir` por `partes` | **0,85 ms** | 0,82–0,86 | 567–603 |

São **135,7×** no p95, com as faixas sem se cruzarem — e o caminho novo aguentou
**~54× mais uniões** na mesma janela. O caminho `partes` **não toma trava
nenhuma**, e isso não é preferência: `travar_dados`
recusa a reentrância, então uma união que segurasse a trava e chamasse um
sub-pedido receberia «trava reentrante» no primeiro braço. **É o código que
obriga.** O preço é que os braços deixam de ser lidos do **mesmo instantâneo**:
um escritor pode entrar entre o braço A e o braço B. Três coisas atenuam e
nenhuma decide sozinha — o `consultar` já é assim nas junções desde sempre, o
isolamento entregue por padrão é `READ COMMITTED` (onde isso é legítimo), e
quem abre transação com **leitura repetível** segura a compartilhada em cada
tabela até o fim e fica coberto. Quem precisa dos braços do mesmo instantâneo
pede leitura repetível; quem não pedir sabe o que tem.

**A ordem dentro do braço escolhe quais linhas ele traz, não arruma a saída** —
é o que os três motores maduros dizem por extenso, e aqui o `max` do braço
continua sendo teto de leitura com `truncado`, não um `LIMIT`.

## O corte do sub-pedido aparece — e não vira recusa

Todo sub-pedido de uma composição (`consultar` com `de`, `juntar[].de`,
`escalar[].de`, `existe[].de`, `em[].de`, e o braço-pedido do `unir`) para em
`recursos.max_linhas`. Quando para, a composição responde sobre um **pedaço**:
o `IN` diz «não casa» à chave que casava, o `EXISTS` diz «não existe» à linha
que existia, e a conta sai com cara de resposta. Desde o pedido 419 o
`consultar` publica **`truncado`**, e o `unir` com `partes` soma o corte do
braço ao que já publicava.

Três decisões que ficam escritas porque elas foram medidas:

- **O corte não recusa.** Quem hoje compõe dentro do teto continua compondo
  igual; `truncado` é campo novo, e quem não o lê recebe o que recebia. O
  pedido 419 chegou prescrevendo trocar a comparação `>` por `>=` no guarda de
  memória do `linhas_do_sub_pedido`: medido, isso recusa `LIMITE_EXCEDIDO` a
  uma tabela que **cabe inteira** no teto (`len() == teto`) — e recusa
  justamente o resultado de quem foi cortado, que é o que se queria contar. A
  comparação continua `>`, e o teste que trava isso é
  `a_composicao_que_cabe_no_teto_nao_recusa_nem_acusa_corte`.
- **Aquele guarda não dispara.** As quatro funções que atendem as cinco
  operações da composição recortam por `limite(p)` = `min(max pedido, teto)`:
  nenhuma consegue devolver **mais** que o teto. O guarda fica como rede da
  operação que entrar na lista sem recortar, e o comentário diz isso em vez de
  se declarar resolvido.
- **`max` pedido é `LIMIT`, não truncamento.** `{"max":1,"ordem":[…]}` é o
  idioma documentado do `escalar`; marcar `truncado` nele faria toda consulta
  correta acender a bandeira, e bandeira que acende sempre é bandeira que
  ninguém lê. Só conta o corte que o teto impôs — sub-pedido sem `max`, ou com
  `max` acima do teto. O `truncado` de um `consultar` aninhado, esse, viaja
  **sem** o crivo: um `max` no nível de fora não pode apagar o corte que
  aconteceu três níveis abaixo.

Cada operação avisa no dialeto que já tinha — `ha_mais` no `varrer`,
`encontrados` acima das linhas no `buscar`, `truncado` no `agrupar` e no
`consultar`. Nenhuma ganhou campo novo: dar a todas um `truncado` seria
escrever a mesma decisão com um segundo nome ao lado do primeiro. Quem traduz
os quatro dialetos é **uma** função, `parou_no_teto`, encostada na lista
`OPS_QUE_DEVOLVEM_LINHAS`; operação nova cai no ramo `_` dela e conta como
**cortada** até alguém dizer como ela avisa.

**Pedido 438 — quem VÊ o `truncado`.** O campo atravessa sem crivo o envelope
da op `sql` (`resposta_do_sql`), então qualquer SQL que rode pelo console
(`ui/claude.js`) ou pelo driver ODBC já o recebia — até esta rodada, nenhum
dos dois lia. Hoje os dois leem: o console mostra o aviso na mesma caixa
`.aviso` (âmbar) das `notas`, pela fábrica de idiomas (`tela.ia_res_truncado`);
o driver ODBC responde `SQL_SUCCESS_WITH_INFO`/`01000` em `SQLExecute`/
`SQLExecDirect` — nunca `01004`, que é o SQLSTATE de truncamento de **valor**
de coluna, não de conjunto de linhas (`docs/ODBC.md` §2,
`docs/cognicao/cognicao_sqlstate-do-truncado-em-composicao_20260924_1308.md`).

## As operações

| Operação | O que faz |
|---|---|
| `juntar` (`join`) | as sete figuras, entre duas tabelas do mesmo banco |
| `unir` (`union`) | empilha duas ou mais partes do mesmo banco — tabelas (`tabelas`) ou pedidos (`partes`) |

As duas exigem `ler` no banco, e conferem de novo antes de abrir a segunda
tabela. No `unir` com `"partes"` quem confere é o portão de sempre, uma vez por
braço, por dentro do `executar_derivado` — a conferência própria do `unir`
continua valendo para o caminho `tabelas`, **e não é duplicação**: o campo que
ela lê não existe num pedido com braços.

E uma armadilha medida em 23/09/2026, que fica escrita porque ela decide qual
teste vale: com o portão do braço **removido de propósito**, um braço `varrer`
ainda era recusado — mas **depois de ler a tabela negada**, porque o modelo da
linha é pedido logo em seguida por um `esquema` que ainda passava pelo portão.
Uma prova que confere só o veredito passaria com o defeito reposto. O braço
`agrupar` não tem essa recusa tardia (o modelo dele sai do próprio cabeçalho),
e por isso é ele que discrimina — com o defeito, a folha resumida sai. *Prova
real é medir quanto foi lido, não se recusou.*

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
  ainda não. Dentro de cada braço da união, sim — é o `"partes"`.
- **`UNION` com filtro ou projeção pelo SQL.** O contrato do braço existe desde
  o pedido 393; o tradutor do `phxsql-sql` ainda traduz só
  `SELECT * FROM a UNION [ALL] SELECT * FROM b`, sobre `tabelas`, e recusa o
  resto nomeando. Reescrevê-lo sobre `partes` é a frente seguinte.
- **`INTERSECT` e `EXCEPT`.** `so_esquerda` já é o `EXCEPT` por chave, e
  `interna` é o `INTERSECT` por chave — mas sobre a *linha inteira*, como o SQL
  faz, ainda não existem.
- **SQL escrito à mão.** Não há analisador; estas são operações do protocolo.
  A camada SQL continua no roteiro.
