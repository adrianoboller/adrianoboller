# A UNIÃO nos quatro motores, medida contra o NOSSO gargalo

Pesquisa do papel J (22/09/2026), pedida pelo integrador ao priorizar os
comandos SQL. A pergunta que a motivou é de **contrato**: a op `unir` recebe
hoje uma lista de **nomes de tabela**, e a proposta é passar a receber **um
pedido por braço**.

Cada afirmação sobre os quatro motores saiu da **documentação normativa deles**,
com URL. Cada afirmação sobre o PhxSql saiu do **fonte**, com arquivo e linha,
ou de uma **corrida contra o motor vivo**, com o número e o instrumento.

A régua é a do `CLAUDE.md`:

- **Convergência dos três maduros** (PostgreSQL + MariaDB + MySQL) = aceite
  automático, sem perguntar — quando nada nosso se opõe.
- **Onde divergem**, decide a média ponderada: PG **4**, MariaDB **3**,
  MySQL **2**, SQLite **1**.
- **Pétrea nossa ganha da convergência**, e o choque **aparece**.

---

## 0. O instrumento, antes do veredito

Tudo o que este documento chama de «medido» saiu de
`target/release/phxsqld` **de 22/09/2026 21:06**, rodado em soquete local
(sonda em `/tmp/.../scratchpad/sonda-uniao.py` e `sonda-perna.py`, fora do
repositório — são matéria-prima, não base de conhecimento).

**A procedência do binário foi conferida**, porque nesta casa «medidor com
binário velho mede o passado»: o único commit do `servidor.rs` posterior a ele
é `bc495f6` (22:11), e `git show` mostra que ele é **inserção pura** nas linhas
21132 e 25856 — fora de `op_unir` (20979), `op_consultar` (12104),
`op_varrer` (17532), `travar_dados` (1580) e de `valores.rs` inteiro (último
commit 18/09). `juncao.rs` está em `5c40125`, de 28/08. **Toda linha de `juncao.rs` citada neste documento foi conferida contra o HEAD (`2fadc0c`), e não contra a árvore de trabalho** — que estava suja no momento da escrita. O binário é válido
**exatamente** para os caminhos medidos aqui, e para nenhum outro.

**Aviso de rodada:** ao escrever isto, duas frentes vivas mexiam na árvore —
`juncao.rs` (com um `// DEFEITO REPOSTO` no meio, prova real em curso) e
`phxsql-sql/{sintaxe,traduzir}.rs` (`SELECT DISTINCT`). **Não medi contra a
árvore suja**, de propósito: seria publicar o estado pela metade de outra
pessoa. Medi contra o binário de HEAD-1, cuja procedência está acima.

---

## 1. A forma: braço é árvore de consulta ou relação materializada?

| motor | o braço é… | braço pode ter `ORDER BY`/`LIMIT` próprio? | fonte |
|---|---|---|---|
| **PostgreSQL** (4) | *«any `SELECT` statement without an `ORDER BY`, `LIMIT`, `FOR …` clause»* — árvore de consulta completa fora essas cláusulas | **sim, entre parênteses**: *«`ORDER BY` and `LIMIT` can be attached to a subexpression if it is enclosed in parentheses»* | [SELECT](https://www.postgresql.org/docs/current/sql-select.html) |
| **MySQL** (2) | *«A query block … is any SQL statement that returns a result set, such as `SELECT`»* (também `TABLE` e `VALUES`) | **sim, entre parênteses** — e *«If `ORDER BY` appears without `LIMIT` within a query block, it is optimized away because it has no effect in any case»* | [Set Operations](https://dev.mysql.com/doc/refman/8.4/en/set-operations.html) |
| **MariaDB** (3) | `SELECT` completo — *«Individual `SELECT` statements can contain their own `ORDER BY` and `LIMIT` clauses»* | **sim** — *«the individual queries need to be wrapped between parentheses»* | [UNION](https://mariadb.com/kb/en/union/) |
| **SQLite** (1) | `select-core` completo, mas *«As the components of a compound SELECT must be simple SELECT statements, **they may not contain ORDER BY or LIMIT clauses**»* | **não, nunca** | [SELECT §3](https://www.sqlite.org/lang_select.html#compound_select_statements) |

**Os quatro concordam no essencial:** o braço é uma **árvore de consulta** —
`SELECT` com o seu próprio `FROM`, `WHERE` e projeção —, **nunca** um nome de
tabela. Nenhum dos quatro tem sequer sintaxe para `UNION` de dois nomes: não
existe `UNION tabela`, existe `UNION SELECT … FROM tabela`.

> **Aceite automático (3 de 3 maduros, e na verdade 4 de 4).** Nada nosso se
> opõe: o braço do `unir` deve ser um **pedido**, não um nome. A op `unir`
> de hoje (`servidor.rs:20993`, que lê só o campo `tabela` de cada item) é a
> única das cinco famílias de composição desta casa que ainda pede um nome.

### 1.1 Onde `ORDER BY` e `LIMIT` entram — a divergência, e a régua

Duas perguntas diferentes, e só a segunda diverge.

**(a) O `ORDER BY`/`LIMIT` global vale para o resultado inteiro?** Os **quatro**
dizem sim, sem exceção. SQLite: *«ORDER BY and LIMIT clauses may only occur at
the end of the entire compound SELECT»*; MySQL: *«place the `ORDER BY` or
`LIMIT` after the last statement»*; MariaDB: *«The `UNION` can have global
`ORDER BY` and `LIMIT` clauses, which affect the whole result set»*; PG:
*«Without parentheses, these clauses will be taken to apply to the result of
the `UNION`»*. **Convergência 4/4 — aceite automático.**

**(b) O braço pode ter `ORDER BY`/`LIMIT` próprio?**

| lado | motores | peso |
|---|---|---|
| **pode** (entre parênteses) | PostgreSQL 4 + MariaDB 3 + MySQL 2 | **9** |
| **não pode** | SQLite 1 | **1** |

**9 a 1 — e, antes da régua, isto já é convergência dos três maduros: aceite
automático.** O braço pode carregar a ordem e o teto dele. A régua ponderada
nem precisa ser invocada; registro o placar porque o SQLite é o voto vencido e
a recusa medida poupa a pergunta depois.

E a nota que os três ainda dão de brinde, e que vale como desenho: **a ordem do
braço não determina a ordem do resultado.** MySQL diz isso por extenso
(*«implies nothing about the order in which the rows appear in the final
result»*, e sem `LIMIT` ela é *optimized away*); MariaDB diz o mesmo (*«These
only limit records read by that specific SELECT»*). Ou seja, a ordem no braço
serve para **escolher quais linhas o braço traz**, nunca para arrumar a saída.

---

## 2. Os tipos das colunas quando os braços diferem — **aqui perdemos, e é grave**

### 2.1 O que os quatro fazem

| motor | regra | fonte |
|---|---|---|
| **PostgreSQL** (4) | algoritmo em 6 passos: *«If the non-unknown inputs are not all of the same type **category**, fail»* (passo 4); senão escolhe um candidato e *«Convert all inputs to the final candidate type»* (passo 6) | [10.5](https://www.postgresql.org/docs/current/typeconv-union-case.html) |
| **MySQL** (2) | *«If the data types of corresponding result columns do not match, **the types and lengths of the columns in the result take into account the values retrieved by all of the query blocks**»* — com o exemplo `REPEAT('a',1) UNION REPEAT('b',20)` saindo com largura 20 | [Set Operations](https://dev.mysql.com/doc/refman/8.4/en/set-operations.html) |
| **MariaDB** (3) | *«the type and length of the columns in the result take into account the values returned by all of the SELECTs, **so there is no need for explicit casting**»* | [UNION](https://mariadb.com/kb/en/union/) |
| **SQLite** (1) | tipagem dinâmica; *«No affinity transformations are applied to any values when comparing rows»* | [SELECT §3](https://www.sqlite.org/lang_select.html#compound_select_statements) |

**Os nomes das colunas** saem do **primeiro** braço nos quatro (MySQL: *«taken
from the column names of the first query block»*; MariaDB idem). **Convergência
— e nisto já estamos certos**: `juncao.rs:633` («Os nomes saem da primeira
parte, como no SQL»).

**O TIPO, porém, leva em conta TODOS os braços** nos três maduros — 3 de 3,
**aceite automático**. Nós tomamos o do **primeiro**.

### 2.2 O nosso `conferir_uniao` bate com PG no passo 4, e para aí

`juncao::conferir_uniao` (`juncao.rs:579`) confere (i) quantidade de colunas e
(ii) **`familia(&ty)`** — e a nossa `familia` (`juncao.rs:193`) é, quase termo a
termo, a **type category** do passo 4 do PostgreSQL: `Int1..Int8`, `UInt*`,
`Real*`, `Decimal{…}` e `Sequence` caem todos em `"numero"`; `Str(_)` e `Memo`
em `"texto"`. Convergimos com o peso-4 sem saber.

O que **falta** é o passo 6 — converter. Nós não convertemos: o cabeçalho
declara o tipo do primeiro braço e as linhas dos outros saem por ele.

### 2.3 Medido: o cabeçalho do primeiro braço **corrompe o dinheiro do segundo**

Duas tabelas, mesma família (`"numero"`), escalas diferentes:
`d2.valor Decimal(12,2)` com **10,50**; `d4.valor Decimal(12,4)` com **10,5000**.
Cada uma lida sozinha devolve o seu valor certo. Unidas:

```
d2 sozinha  -> valor '10.50'
d4 sozinha  -> valor '10.5000'

{"op":"unir","tabelas":["d2","d4"],"modo":"tudo"}  ->  [[1,'10.50'], [2,'1050.00']]
{"op":"unir","tabelas":["d4","d2"],"modo":"tudo"}  ->  [[2,'10.5000'], [1,'0.1050']]
```

**10,5000 sai como 1050,00. 10,50 sai como 0,1050.** Cem vezes para cima num
sentido, cem vezes para baixo no outro, **sem erro, sem aviso, sem `truncado`**.

A causa está no fonte, em duas linhas que se encontram:

- `Value::Decimal` guarda o **inteiro escalado**, e o texto sai de
  `decimal_para_texto(valor, escala)` (`phxsql-core/src/carga.rs:543`), que
  divide por `10^escala`;
- `valores.rs:794` escolhe essa escala pelo **tipo do cabeçalho**
  (`(Value::Decimal(n), ColumnType::Decimal{escala,..})`), e no `unir` o
  cabeçalho é o do **primeiro braço** (`juncao.rs:633-642`).

`conferir_uniao` deixa passar porque as duas são família `"numero"` — está
**certa** pelo passo 4 do PG; o que falta é o passo 6.

**Este é o IRMÃO do conserto que está acontecendo agora.** A frente viva em
`juncao.rs` corrigiu a escala **na chave** do `Distinta` (o comentário dela diz,
com todas as letras, *«duas partes podem declarar escalas diferentes na mesma
posição»*) e não tocou na escala **da saída**. É literalmente a lei da casa:
*conserto entra no caminho que o motivou, e o caminho irmão fica* — e irmão
aqui é quem lê a mesma escala do mesmo lugar.

> **Aceite automático (PG 4 + MariaDB 3 + MySQL 2 = 3 de 3 maduros): o tipo do
> resultado leva em conta todos os braços.** No mínimo, a **maior escala** e o
> **maior comprimento** entre os braços. Nada nosso se opõe — a pétrea da
> integridade não alcança, e a pétrea que **reforça** é outra: «rótulo se
> estiliza, dado nunca». Um cabeçalho que muda o valor do dado não é estilo,
> é mentira sobre o dado, e esta é a versão numérica do «BLUMENAU».

---

## 3. O `UNION` distinto: materializar para desduplicar, e a que custo

| motor | como desduplica | quem paga |
|---|---|---|
| PostgreSQL | `HashAggregate`/`HashSetOp` ou `Sort`+`Unique`, escolhido pelo planejador | memória do `work_mem`, com derrame em disco |
| MySQL / MariaDB | tabela temporária com índice único (MariaDB: *«The server can in most cases execute `UNION ALL` without creating a temporary table»* — ou seja, o `UNION` **com** desduplicação cria) | tabela temporária |
| SQLite | ordena ou usa índice transitório | temporário |
| **PhxSql** | `HashSet<String>` da linha canonizada (`juncao.rs:645,658-672`) | **memória, sem derrame** |

**Neste ponto o nosso desenho é o do Cassandra do `CASSANDRA.md`: já
convergimos sem saber.** Um `HashSet` de chave canônica é exatamente o
`HashSetOp` do PG, sem o derrame. **Não há receita alheia a importar aqui** — e
essa é uma recusa, não um elogio: o que os maduros têm a mais é o **derrame em
disco**, que compra consultas maiores que a RAM e custa um formato de arquivo
temporário novo. Não recomendo. Nós temos o `TETO_JUNCAO` (500.000 linhas,
`servidor.rs:24738`) recusando **antes** de materializar, que é a escolha
honesta para quem tem zero dependências: recusar nomeando em vez de derramar.

**O NULO na desduplicação: convergência, e já estamos certos.** SQLite é o
único que escreve por extenso — *«For the purposes of determining duplicate
rows for the results of compound SELECT operators, NULL values are considered
equal to other NULL values and distinct from all non-NULL values»* — e PG diz
o mesmo por referência (*«eliminates duplicate rows … in the same way as
DISTINCT»*). O nosso `juncao.rs:662-664` marca o nulo como `"\u{1}∅"`, e o
`docs/JUNCOES.md:101` já registra a regra e a diferença com a junção.
**Convergência 4/4, já cumprida — nada a fazer.**

### 3.1 O defeito do `rownum`: medido, confirmado, e **já sendo consertado** — não re-abrir

Medi, e confirmo com o número:

```
a = (1,'papel') (2,'caneta') (3,'tinta')      b = (3,'tinta') (4,'cola')
{"op":"unir","tabelas":["a","b"],"modo":"distinta"}
  -> quantas: 5, repetidas: 0, linhas: [[1,papel],[2,caneta],[3,tinta],[3,tinta],[4,cola]]
{"op":"unir","tabelas":["a","b"],"modo":"tudo"}
  -> quantas: 5
```

**O `distinta` devolveu o mesmo que o `tudo`**, anunciando `repetidas: 0`, com
a linha `(3,'tinta')` duplicada na resposta. O SQL manda 4 e 1.

A causa: a chave da desduplicação percorria a linha **inteira**, e a linha
inteira carrega as colunas de sistema — `rownum` entra em todo esquema
(`phxsql-core/src/schema.rs:796`) e é único por linha dentro da tabela. Elas só
eram cortadas **depois**, na volta (`juncao.rs:695-704`).

**Este achado é da frente viva de `juncao.rs`, não meu** — ela já tem o conserto
escrito e a prova real em curso, com exatamente o mesmo número («cinco linhas e
`repetidas: 0` onde o SQL manda quatro e uma»). Minha medição é **confirmação
independente contra o binário de antes do conserto**, e o registro aqui existe
para **impedir que este item vire pedido duplicado**.

O que a minha medição acrescenta é **por que ele sobreviveu tanto tempo**, e
isso vale guardar: no mesmo teste, o `UNION distinta` de duas tabelas de 20.000
linhas carregadas com conteúdo idêntico na mesma ordem devolveu
`quantas: 20000, repetidas: 20000` — **desduplicou perfeitamente**. Porque os
`rownum` casaram 1 para 1. **O caso óbvio de teste — unir uma tabela com uma
cópia idêntica — passa**, e passa por coincidência. Teste que passa por engano
é pior que teste que falta, e este passava por engano em escala.

---

## 4. O NOSSO gargalo, medido

Esta é a seção que decide, e é a que a lei da casa exige antes de qualquer
plano. Os 83,5% no `.ndx` são do caminho de **escrita**; o `unir` é leitura, e
tem gargalo próprio. Medi dois.

### 4.1 A trava global EXCLUSIVA, segurada por toda a materialização

`op_unir` toma `self.travar_dados()` (`servidor.rs:21020`) e só a solta no fim.
E `travar_dados` é `self.dados.**write**()` (`servidor.rs:1599`) — a
**exclusiva**. Enquanto a união materializa todos os braços, **nenhum leitor
entra**.

Medido — um `varrer(max=1)` numa tabela de 3 linhas, sondando de outra conexão:

| cenário | mediana | p95 | **máximo** |
|---|---|---|---|
| sozinho no servidor | 0,20 ms | 0,28 ms | 5,54 ms |
| com 4 `unir` de 2×20.000 correndo | 0,21 ms | 3,88 ms | **122,27 ms** |

Reproduzido em duas corridas independentes (máximos de 118,87 ms e 122,27 ms).
**Uma união de duas tabelas de 20 mil linhas para o servidor inteiro por ~120
ms.** A mediana não se move porque a sonda passa nas frestas; o que a união
compra é uma **parada de 120 ms** para quem tiver o azar de chegar durante ela.

E o contraste que dá o caminho: `op_varrer` **já** tenta a trava
**compartilhada** primeiro (`travar_dados_para_ler`, `servidor.rs:17536`), e
`op_consultar` **não segura trava nenhuma** enquanto compõe — cada sub-pedido
toma e solta a sua, por dentro do `executar_derivado`. A composição desta casa
já é a barata; o `unir` é que ficou de fora dela.

### 4.2 O filtro que não existe: 118,7× pelo mesmo resultado

A pergunta: `SELECT id,nome FROM g1 WHERE uf='SC' UNION SELECT id,nome FROM g2
WHERE uf='SC'`. Duas tabelas de 20.000, 1% de seletividade, **400 linhas na
resposta**. Dois caminhos para a **mesma resposta** — a sonda compara as duas
listas e só depois compara tempo (**`MESMA RESPOSTA? SIM`**), porque nesta casa
a bancada compara trabalho igual e não só pergunta igual:

| caminho | linhas materializadas | mediana | min–max |
|---|---|---|---|
| (a) `unir` inteiras + filtrar fora — **o que hoje é possível** | 40.000 | **230,3 ms** | 219,4–266,8 |
| (b) filtrar DENTRO de cada braço — **o que a proposta compra** | 400 | **1,9 ms** | 1,9–2,4 |

**118,7× a favor do braço filtrado**, com as faixas min–max **sem se cruzarem**
(a regra do pedido 155: vencedor só se declara fora do ruído — este está).

**A honestidade que este número exige, em três pontos:**

1. O caminho (b) usa um índice (`porUf`) que o caminho (a) **estruturalmente
   não pode usar** — o `unir` de hoje não tem onde receber um filtro. Isso não
   é artefato da bancada: **é o achado**.
2. O ganho é **função da seletividade**. A 1% dá 118,7×; a 100% dá ~1×. O
   número não é «o `unir` é 118× lento», é «o `unir` obriga a pagar a tabela
   inteira quando se quer 1% dela».
3. O caminho (b) não inclui o empilhamento e a desduplicação das 400 linhas,
   que a proposta ainda pagaria — na ordem de centenas de microssegundos para
   400 linhas, contra os 229 ms de diferença. Não muda a ordem de grandeza.
   **Raciocinado, não medido**: mediria empilhando as 400 no mesmo `unir`.

### 4.3 O teto que o filtro no braço também compra

`materializar` (`servidor.rs:20646`) recusa a tabela acima de `TETO_JUNCAO`
= **500.000** linhas. Hoje esse teto se aplica à **tabela**; com filtro no
braço, aplicar-se-ia ao **recorte**. Uma tabela de 2 milhões de linhas é hoje
**impossível** de unir, mesmo que se queira dez linhas dela. A recusa até
ensina («Troque a ordem dos lados ou filtre antes») — mas **não há onde filtrar
antes**, e uma mensagem que manda fazer o que o contrato não permite é a mesma
família do campo de configuração que ninguém lê.

---

## 5. A recomendação

### 5.1 Sim, o braço deve ser um pedido — e o palpite do integrador está certo, com uma correção que o melhora

O palpite era: *«o braço deve aceitar o mesmo objeto que o `consultar` já
recebe, para não nascer uma segunda gramática de consulta dentro do
protocolo.»*

**Confirmo a intenção e corrijo o alvo:** o braço deve aceitar o que o
**`linhas_do_sub_pedido` já aceita** (`servidor.rs:11964`) — que é um
**super-conjunto** do `consultar`, e por isso a correção é um ganho e não uma
ressalva:

```
const OPS_QUE_DEVOLVEM_LINHAS: &[&str] =
    &["varrer", "buscar", "agrupar", "group_by", "consultar"];
```

Três razões, todas medidas ou lidas no fonte:

1. **Não é segunda gramática — é a gramática que já existe, e em cinco lugares.**
   `consultar` usa `linhas_do_sub_pedido` no `de`, em `juntar[].de`, em
   `escalar[].de`, em `existe[].de` e em `em[].de`. O `unir` seria o **sexto
   uso do mesmo contrato**, não o primeiro de um novo. Mirar só o `consultar`
   deixaria de fora `buscar` — que é justamente o que comprou os 118,7× do
   §4.2, porque é ele que usa índice.

2. **O portão de permissão continua sendo UM.** `linhas_do_sub_pedido` roteia
   por `executar_derivado` (`servidor.rs:6716`), que chama
   `politica_do_pedido` + `portoes_do_pedido` + `aplicar_direito_por_coluna` —
   o irmão do `despachar`, documentado como tal no próprio fonte. Hoje o `unir`
   paga **conferência própria** (`servidor.rs:21009-21017`) porque a tabela
   está numa lista que o portão geral não lê. Com braços-pedido, a conferência
   passa a acontecer onde já acontece para todo o resto.

3. **É o mesmo movimento que a frente irmã acabou de fazer.** O `SELECT
   DISTINCT` que está entrando no `phxsql-sql` agora **não inventou op nova**:
   mapeou-se sobre o `agrupar` que já existia. Reusar contrato em vez de criar
   contrato é o padrão vivo desta rodada, não uma preferência minha.

Forma sugerida — **o campo novo ao lado do velho, nunca no lugar dele**:

```
{"op":"unir", "database":"loja", "modo":"distinta",
 "partes":[ {"op":"buscar",  "tabela":"g1", "indice":"porUf", "chave":"SC"},
            {"op":"consultar","de":{"op":"varrer","tabela":"g2"},
                              "expressao":"uf = 'SC'"} ],
 "ordem":[{"coluna":"nome"}], "max":100}
```

`tabelas` (lista de nomes) **continua valendo e continua significando o mesmo**.
Isto não é cortesia: é a pétrea «guarda nova entra pedida, não imposta» — e o
teste que mais importa é o do comportamento **velho**, não o do novo. Há
clientes do `tabelas` no repositório hoje (`rest.rs:1014`, `catalogo.rs:766`,
`bancada/gaps-sql/sondar.py:376`, `bancada/diretivas/provar.py:317`).

O nome `partes` tem fundamento no próprio código: `juncao::unir` já chama os
braços de `partes`, a resposta já devolve `por_parte`, e o `profiler.rs:1945`
**já escreve um pedido de exemplo com `"partes":[{"tabela":…}]`** — um campo que
o motor hoje ignora. A nomenclatura já vazou para a casa antes do contrato.

### 5.2 O que entra por convergência, sem perguntar ao dono

| ponto | placar | entra |
|---|---|---|
| braço é pedido, não nome | 4/4 | **sim** |
| `ordem`/`max` global valem para o resultado da união | 4/4 | **sim** |
| braço pode ter `ordem`/`max` próprios | PG4+Mar3+MySQL2 = 9 × SQLite 1 | **sim** |
| ordem do braço **não** arruma a saída | 3 maduros | **sim** — documentar |
| nomes das colunas vêm do 1º braço | 4/4 | **já temos** |
| tipo do resultado considera **todos** os braços | 3 maduros | **sim — e é conserto de defeito vivo (§2.3)** |
| dois NULOS são a mesma linha | 4/4 | **já temos** |

### 5.3 Os três choques, que aparecem em vez de ficarem calados

**(i) A trava única morre, e com ela o instantâneo comum.** `travar_dados`
guarda `COM_A_TRAVA` e **erra na reentrância** (`servidor.rs:1583`). Um
`op_unir` que segurasse a trava e chamasse `linhas_do_sub_pedido` receberia
`trava_reentrante()` no primeiro braço. **Não é preferência: é o código que
obriga.** A consequência boa está medida no §4.1 (a parada de 120 ms acaba); a
consequência a pagar é que os braços deixam de ser lidos do **mesmo
instantâneo** — um escritor pode entrar entre o braço A e o braço B.
Três coisas atenuam, e nenhuma decide sozinha: (a) `consultar` **já** tem
exatamente essa propriedade para junções, desde sempre; (b) quem pede
`leitura_repetivel` (16/09) segura a S em cada tabela até o fim e fica coberto;
(c) o padrão entregue é `READ COMMITTED`, onde isto é legítimo.
**Isto é palavra do papel C (DBA), não minha** — é garantia de dado, e eu
registro o custo, não o decido.

**(ii) `("unir", PorColuna::Recusa)` deve FICAR, e por um motivo novo.**
`direito_coluna.rs:133` recusa a união inteira para quem tem direito por
coluna. O comentário ao lado fala de *«colunas prefixadas de duas tabelas»* —
o que descreve o `juntar`, não o `unir`, que é posicional. Com braços-pedido
ficará ainda mais tentador limpar a recusa, porque cada braço passa a aplicar a
peneira sozinho. **Não limpe.** A união empilha **por posição**
(`docs/JUNCOES.md:93-99`), e esconder uma coluna **muda a aridade do braço**:
`modelo_da_tabela` (`servidor.rs:12054`) tira as negadas, então um usuário que
não lê `saldo` receberia um braço de 3 colunas contra outro de 4. No melhor
caso `conferir_uniao` recusa com uma mensagem sobre contagem de colunas que
quem pediu não tem como entender; no pior, com aridades iguais por acaso, os
valores **deslizam de coluna, calados** — o estrago exato contra o qual o
`docs/JUNCOES.md:96` já avisa. É o risco que a pétrea nomeia: numa faxina, essa
recusa **parece duplicação**, e nenhum teste do portão acusa.

**(iii) O `UNION` não existe no SQL, e a op não fecha a lacuna sozinha.**
`phxsql-sql/src/sintaxe.rs:394` tem `"UNION"` **só como palavra reservada**;
não há tradução (`grep -i union crates/phxsql-sql/src/*.rs` dá 2 acertos, e um
é o comentário do `lib.rs:47` dizendo que não há). O `unir` de hoje é
alcançável **apenas pelo protocolo nativo**. Como o dono mandou priorizar SQL,
registro a ordem que o §4.2 sustenta: **o contrato do braço vem antes do
tradutor**, porque um tradutor escrito sobre `tabelas` só saberia traduzir
`SELECT * FROM a UNION SELECT * FROM b` — e teria de ser reescrito no dia
seguinte. Isto toca a frente viva do `phxsql-sql`: **é coordenação do papel A,
não decisão minha.**

---

## 6. O que foi avaliado e **RECUSADO**, com o número

Esta seção existe para a mesma proposta não voltar sem medição.

1. **Derrame em disco para o `UNION` distinto** (o temporário do MySQL/MariaDB,
   o `work_mem` do PG). **Recusado.** Compra consulta maior que a RAM e custa um
   formato de arquivo temporário novo — com zero dependências, escrito aqui.
   O `TETO_JUNCAO` de 500.000 (`servidor.rs:24738`) já recusa **antes** de
   materializar, que é a escolha honesta. E o §4.2 mostra que o problema real
   não é o tamanho do resultado (400 linhas), é o tamanho do que se **lê para
   chegar nele** (40.000): o filtro no braço resolve o caso que o derrame
   resolveria, sem formato novo.

2. **Trocar o `HashSet<String>` por outra estrutura de desduplicação.**
   **Recusado — já convergimos.** É o `HashSetOp` do PG por outro nome. Medido:
   `UNION distinta` de 2×20.000 custou 153 ms contra 147 ms do `UNION ALL` das
   mesmas tabelas — **a desduplicação custa ~4%**. O gargalo não está ali; está
   nos 40.000 que se materializaram para responder sobre 400.

3. **Casar as colunas por NOME em vez de por posição.** **Recusado, e não é
   novidade** — `docs/JUNCOES.md:95-99` já registra a decisão com o motivo
   («duas tabelas com as mesmas colunas em ordem diferente empilhariam trocando
   os valores, caladas»), e os quatro motores são posicionais. Registro aqui
   porque um braço-pedido, que traz nomes explícitos na projeção, faz a ideia
   parecer nova. Não é.

4. **`DEFERRABLE`/adiamento ou qualquer reabertura de MVCC para dar
   instantâneo comum aos braços.** **Fora do escopo e vedado**: a Sombra está
   parada por decisão do dono, e a via (b) escolhida em 16/09 (leitura
   repetível pela trava, **pedida**) já cobre quem precisa. Convergência não
   reabre o que o dono fechou.

5. **Espalhar a conferência de permissão pelos braços.** **Recusado pela
   pétrea**: «não espalhe o portão por quarenta operações». Com braços-pedido o
   `executar_derivado` já é o portão; a conferência própria do `unir` fica
   **só** para o caminho `tabelas`, que continua existindo.

---

## 7. Lacunas — o que eu **não** medi, e o que decidiria cada número

Declaro, porque número citado é número que não se mede:

1. **Não medi o caminho proposto**, porque ele não existe. Os 118,7× do §4.2
   são o **limite superior** do ganho: dois `buscar` mais a união de 400 linhas
   do lado de fora. O número real fica entre 1,9 ms e 230,3 ms, muito mais perto
   do primeiro. **Decide na bancada:** o mesmo roteiro, com `partes` implementado.
2. **Não medi o custo da própria decomposição** — quantos microssegundos custam
   dois `executar_derivado` com `politica_do_pedido` +`portoes_do_pedido` +
   `aplicar_direito_por_coluna`, contra um `travar_dados` só. **Raciocinado, não
   medido**: a lição do Profiler diz que o portão sem usuário sai antes de
   alocar, e o `lock` sem disputa custou 13,2 ns nesta casa — deve ser ruído
   contra os 95 ms do `unir`. **Decide na bancada:** `--example onde-doi` no
   caminho do `consultar` com dois sub-pedidos triviais.
3. **Não medi a conversão de tipo do §2.3 em volume.** Sei que o valor sai
   errado (medido); não sei o que custa reconciliar escala e comprimento por
   coluna antes de empilhar. **Decide na bancada:** unir duas tabelas de
   1.000.000 com `Decimal` de escalas diferentes, com e sem a reconciliação.
4. **Não li o fonte C do MySQL nem do MariaDB** para o `UNION` — fiquei na
   documentação normativa deles, que nos quatro pontos deste documento é
   explícita e citada. Onde ela **cala** eu disse que cala (o NULO na
   desduplicação, em PG/MySQL/MariaDB) e apoiei a conclusão no SQLite, que
   escreve por extenso, e no nosso comportamento, que já está certo.
5. **Não medi a árvore de trabalho de hoje**, por decisão registrada no §0:
   duas frentes estavam com o código no meio de uma prova real.

---

## 8. Onde esta lógica DIVERGE da de origem, e qual restrição nossa causa a divergência

A pergunta que a lei manda responder — e sem ela nada disto é nosso.

1. **O braço aceita `varrer`/`buscar`/`agrupar`/`consultar`, e não «um
   SELECT».** Restrição: **não temos SELECT** como unidade de contrato — temos
   um protocolo JSON de operações. A unidade reusável aqui é a op que devolve
   linhas, e ela já estava inventada (`linhas_do_sub_pedido`). Nos quatro
   motores o braço é um nó da árvore do parser; aqui é uma **op do protocolo**,
   que existe e é despachável sozinha.
2. **Sem parênteses, e sem a ambiguidade que eles resolvem.** PG, MySQL e
   MariaDB precisam de parênteses porque a gramática textual não sabe se
   `ORDER BY` é do braço ou do todo. Em JSON a pergunta não existe: `ordem`
   dentro do item de `partes` é do braço, `ordem` no objeto de fora é do
   resultado. **Herdamos o comportamento e jogamos fora o meio** — que é
   exatamente o que a lei da convergência manda («o aceite é do comportamento,
   não do meio»).
3. **`MAX` do braço é teto de leitura com `truncado: true`, não `LIMIT`.**
   Restrição: o nosso `limite_pivot` corta **durante** o acúmulo
   (`juncao.rs:674-677`) e **diz que cortou**. Os quatro cortam no fim e não
   dizem. Mantemos o nosso, porque uma lista truncada em silêncio faz quem
   confere acreditar que viu tudo — e isso já está escrito no `op_juntar`.
4. **Recusamos onde o PG converteria.** PG passo 6 converte tipos dentro da
   categoria; a recomendação do §2.3 é adotar o **tipo mais largo**, não a
   conversão implícita geral do PG. Restrição: **zero dependências** e um
   `Value` sem coerção automática — e a integridade primordial desta casa
   prefere recusar cedo a converter calado. Adotamos o **resultado** (o dado não
   mente) sem adotar a **máquina de coerção** (seis passos com categorias
   preferidas e conversões implícitas registradas por tipo).
5. **Não derramamos em disco; recusamos em 500.000.** Restrição: zero
   dependências e a decisão de não inventar formato de arquivo temporário.
   Divergência assumida, com o número no §6.1.
6. **`ao_excluir`, `rowid` e ordem de digitação não aparecem aqui — e isso é
   um resultado.** A união é leitura pura: não cria filho, não mata pai, não
   grava slot. **Nenhuma das três pétreas de integridade foi tocada por
   nenhuma das receitas dos quatro motores** — é a primeira pesquisa desta casa
   em que isso acontece, e vale registrar, porque foi a ordem de digitação que
   matou quatro das oito receitas de MVCC. Aqui ela não mata nenhuma.

---

## 9. Resumo para o integrador

- **Estenda o `unir` — sim.** O braço-pedido está medido em **118,7×** pelo
  mesmo resultado, com as faixas sem se cruzarem, e mata de brinde a parada de
  **122 ms** que a trava exclusiva impõe ao servidor inteiro.
- **A forma é `linhas_do_sub_pedido`, não «o objeto do `consultar`»** — o
  `consultar` está dentro dela, e o `buscar`, que é quem usa índice, ficaria de
  fora do palpite original.
- **`tabelas` não sai.** Guarda nova entra pedida.
- **Há um defeito vivo, medido, que não é da proposta e não pode esperar por
  ela**: a escala do `Decimal` do cabeçalho corrompe o valor dos outros braços
  em 100× (§2.3). É o **irmão** do conserto que a frente de `juncao.rs` está
  fazendo agora, e ela não o alcança.
- **Não abra pedido para o `rownum` na chave do `UNION`** (§3.1): já está
  sendo consertado, com o mesmo número.
- **Três choques para a mesa, não para o silêncio**: a trava única morre (papel
  C decide), a `PorColuna::Recusa` do `unir` precisa **ficar** e agora por um
  motivo novo (papel G registra), e o `UNION` do SQL depende deste contrato
  (papel A coordena com a frente viva do `phxsql-sql`).
