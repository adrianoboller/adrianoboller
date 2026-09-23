# Agregado sobre junção: onde o substrato entra — parecer do papel J

**Data:** 23/09/2026 · **Papel:** J (pesquisador) · **Pedido:** 394
**Regime:** cláusula pétrea de 23/09/2026 — *«o pesquisador decide; o dono é o
impasse»*. **Este parecer DECIDE. Não sobe à mesa do dono.** O porquê está na
§4: 4 de 4 motores convergem, nenhuma pétrea nossa se opõe, e onde divergimos
deles a restrição que causa a divergência está nomeada.

---

## 1. A decisão, e o número que a decidiu

**Via escolhida: (b), com a segunda metade dela RECUSADA.**

- **ENTRA:** `consultar` ganha `por`, `agregados` e `tendo`. Passa a ser a
  porta do agregado sobre junção.
- **NÃO ENTRA:** `agrupar` **não** vira açúcar de `consultar`. Fica como está,
  e fica sendo o caminho rápido de uma tabela só.
- **RECUSADA:** a via (a) — `agrupar.de` recebendo um sub-pedido.

**O número que decidiu entre (a) e (b):** **0 contra 2** mexidas na maquinaria
do portão. A via (b) não cria conferência própria nenhuma, nem entrada nova em
`CLASSES`, nem ramo novo em `tabelas_do_pedido`. A via (a) cria **duas**
(§5.3). Com o desempate da §4: **4 de 4** motores põem `FROM`+`JOIN`+`GROUP BY`
no **mesmo nó** da requisição.

**O número que matou a segunda metade da via (b):** `recursos.max_linhas`
nasce **1.000** (`crates/phxsql-server/src/config.rs:3552`) e no `varrer` o
`max` quer dizer **linhas EXAMINADAS**, não devolvidas (comentário de
`varrer_a_pagina`, `servidor.rs`). Um `COUNT(*)` sobre uma tabela de 1.000.000
de linhas hoje conta **1.000.000** (o `op_agrupar` faz `t.varrer()` sem `max`,
`servidor.rs:12965`) e pela composição contaria **1.000** — número errado
calado, com cara de certo.

---

## 2. As hipóteses, escritas ANTES de medir

| # | Hipótese | O que a mediria | Veredito |
|---|---|---|---|
| **H1** | A via (a) repete o padrão do `pivotar`, logo cria conferência própria nova; a (b) não cria nenhuma. | Contar `pode_em` fora de `portoes_do_pedido`, por função. | **Vencedor certo, mecanismo ERRADO.** §5.3 e §6.1. |
| **H2** | Os quatro motores representam isto como *pipeline* (nó de agregação consumindo o nó de junção), logo a forma deles é a (a), o nó aninhado. | Ler `Agg`/`AggregateIterator`/`AGGR_OP`/`updateAccumulator` no fonte. | **MORREU.** O pipeline é verdade do **plano**, e o plano não é o que o cliente escreve. §6.2. |
| **H3** | `agrupar` pode virar açúcar de `consultar` sem perda — é a mesma pergunta com outra roupa. | Comparar o que cada caminho materializa e sob que teto. | **MORREU, com número.** §6.3. |
| **H4** *(terceira via, levantada depois de ler o fonte nosso)* | Nem (a) nem (b) puras: `consultar` ganha o agregado **e** `agrupar` sobrevive como caminho rápido, porque não temos planejador que escolha o físico. | Medir o custo do caminho materializado contra o caminho que flui do disco. | **VENCEU.** É a recomendação. §7. |

---

## 3. Matriz de evidência — os quatro motores, no fonte

A pergunta do pedido tinha duas camadas, e as duas responderam coisas
**diferentes**. Separá-las é o que decidiu o parecer.

### 3.1 Camada da REQUISIÇÃO (a AST — o que o cliente escreve)

**Um nó só carrega `FROM`(+junções), `WHERE`, `GROUP BY`, `HAVING`,
`ORDER BY`, `LIMIT` e a lista de projeção. Nos quatro. Sem exceção.**

| Motor | Estrutura | Fonte, com linha | Campos no MESMO nó |
|---|---|---|---|
| PostgreSQL 17 | `Query` | `src/include/nodes/parsenodes.h:175,191,200,205,211,214` | `jointree` (FROM+WHERE) · `targetList` · `groupClause` · `havingQual` · `sortClause` · `limitCount` |
| MySQL 8.0 | `Query_block` | `sql/sql_lex.h:1910,1269,1189,1919,1320` | `m_table_list` · `group_list` · `m_having_cond` · `order_list` · `select_limit` |
| MariaDB 11.4 | `st_select_lex` | `sql/sql_lex.h:1239,1247,1146,1254,1257` | `table_list` · `group_list` · `having` · `order_list` · `limit_params` |
| SQLite (trunk) | `Select` | `src/sqliteInt.h:3636-3644` | `pSrc` (SrcList = FROM/join) · `pWhere` · `pGroupBy` · `pHaving` · `pOrderBy` · `pLimit` |

**Custo de ler a evidência:** quatro `curl` de cabeçalho público. Nenhum
motor precisou ser compilado.

**O que isto resolve para nós:** a forma da porta. O protocolo JSON do PhxSql
é a camada da **requisição** — o cliente dele é o tradutor SQL
(`crates/phxsql-sql/`) e a tela. O par dele nos quatro motores é a AST, não o
plano.

### 3.2 Camada do PLANO (o que o motor faz por dentro)

**Pipeline: a agregação consome a saída da junção. Nos quatro. Sem exceção —
e nunca o contrário.**

| Motor | Forma | Fonte, com linha |
|---|---|---|
| PostgreSQL | nó `Agg` cujo `plan.lefttree` é o nó de junção; o planejador encadeia `query_planner` → `create_grouping_paths` → `create_window_paths` → `create_distinct_paths` → `create_ordered_paths`, reatribuindo `current_rel` a cada passo | `src/include/nodes/plannodes.h:996-1032` e `:154`; `src/backend/optimizer/plan/planner.c:1641,1785,1803,1823,1838` |
| MySQL 8.0 | `AggregateIterator` embrulha o iterador de junção em `m_source` | `sql/iterators/composite_iterators.h:205,235` |
| MariaDB | `JOIN_TAB::aggr` é um `AGGR_OP` pendurado no laço da junção; a função de escrita é `end_send_group` | `sql/sql_select.h:457,1178-1186,772` |
| SQLite | `sqlite3WhereBegin(pParse, pTabList, …)` gera o laço sobre a SrcList (a junção); `updateAccumulator()` roda **dentro** dele e `finalizeAggFunctions()` **depois** de `sqlite3WhereEnd` | `src/select.c:8650,8823,8865` (com GROUP BY) e `:9003,9010,9024` (sem) |

**O que isto resolve para nós — e o que NÃO resolve.** Resolve a **ordem dos
passos**: junção antes de agregação, sempre. Não resolve a forma da porta,
porque **nenhum cliente escreve um plano**. O SQL não tem sintaxe para
«agregue este nó de junção»: quem quer agregar sobre junção escreve **um
`SELECT` só**. A forma aninhada existe nos quatro — é a tabela derivada,
`FROM (SELECT …) x` — mas ela é **outra coisa**: agrega sobre um resultado já
materializado. E **nós já a temos**: `consultar{de: agrupar{…}}` funciona
hoje, porque `agrupar` está em `OPS_QUE_DEVOLVEM_LINHAS`
(`servidor.rs:12173-12174`).

### 3.3 O que o PhxSql já tem, medido no nosso fonte

| Nosso | É o quê | Fonte |
|---|---|---|
| `Selecao` (AST do nosso SQL) | **já é** o nó dos quatro: `de` + `onde` + `agrupar_por` + `tendo` + `ordem_lista` + `limite` + `salto` + `projecao` | `crates/phxsql-sql/src/sintaxe.rs:393-421` |
| `Projecao::Agregada(Vec<ItemProjetado>)` | agregados **na lista de projeção**, como nos quatro | `crates/phxsql-sql/src/sintaxe.rs:83` |
| `op_consultar` | **já é** o nó dos quatro **menos duas coisas**: tem `de` + `juntar[]` + `escalar` + `existe` + `em` + `expressao` + `janela` + `ordem` + `pular`/`max` + `colunas`; faltam `por` e `agregados` | `servidor.rs:12306` e docs `:12275-12305` |
| O tradutor já bifurca | `sintaxe.rs` → `agrupar` (sem junção, `traduzir.rs:575`); `consulta.rs` → `consultar` (com junção, `:492,612`) | — |
| A recusa honesta do 394 já entrou | `SEM_SUBSTRATO_PARA_AGREGAR` + `recusa_de_agregacao_composta` | `crates/phxsql-sql/src/consulta.rs:1280,1291` |

**Consequência medida:** a nossa própria `Selecao` já tem a forma dos quatro.
O **único** lugar onde a forma diverge é o protocolo. Dar `por`/`agregados` ao
`consultar` torna o nó do protocolo **isomorfo** ao nosso `Selecao` — uma
tradução, não um embrulho. Pela via (a), `consulta.rs` teria de **embrulhar**
o `consultar` que já constrói dentro de um `agrupar`, e as duas estruturas
deixariam de casar campo a campo.

---

## 4. As réguas da casa, aplicadas na ordem

**Régua 1 — três maduros convergindo e nada nosso se opõe → ACEITE
AUTOMÁTICO, sem perguntar ao dono.**

Convergência medida: **4 de 4** na camada da requisição (§3.1) e **4 de 4** na
ordem dos passos (§3.2). Não é «três», é o conjunto inteiro, SQLite incluído.

**Régua 2 — média ponderada.** Não foi necessária. Vai escrita assim mesmo,
porque régua não aplicada tem de aparecer como não aplicada: se houvesse
divergência, «um nó só» somaria PG 4 + MariaDB 3 + MySQL 2 + SQLite 1 = **10**
contra **0**. Não houve lado contrário para somar.

**Régua 3 — o que sobe à mesa do dono.** Nada aqui sobe:

- **Choque com pétrea?** Não. Cinco pétreas conferidas uma a uma na §5.
- **Empate real?** Não. 10 × 0, e 0 × 2 no portão.
- **Produto (preço/prazo/SLA)?** Não. É contrato interno do protocolo.

**Nota de escopo, e ela é o motivo de este parecer existir.** O
`consulta.rs:1276-1281` escreve, no próprio fonte: *«Dar substrato muda o
CONTRATO de uma das duas ops, e isso e decisao do dono.»* Aquela frase é de
**antes** da pétrea de 23/09/2026. Sob a pétrea nova ela caducou: contrato de
protocolo que os quatro motores documentam **não é decisão do dono** — é
exatamente o que o dono mandou o papel J decidir. **Essa linha de comentário
está desatualizada e o pedido 394 deve registrar isso**; eu não a edito (§10).

---

## 5. Medido contra o NOSSO gargalo e as NOSSAS pétreas

### 5.1 O inventário do portão, recontado hoje

A lei da casa diz «7 das 116 operações». **Medido em 23/09/2026: são 140
operações** (`CAPABILITIES.json` → `operacoes: 140`) e **10 funções** chamam
`pode_em` sobre um alvo que **não** é o campo `"tabela"` do pedido:

| # | Função | Campo que o portão geral não lê | Linha em `servidor.rs` |
|---|---|---|---|
| 1 | `op_juntar` | `a.tabela`, `b.tabela` | 21110 |
| 2 | `op_unir` | `tabelas[]` / `partes[]` | 21514 |
| 3 | `op_pivotar` | `juntar[].tabela` | 13121 |
| 4 | `op_diferencas` | `a`, `b` (texto solto) | 21240 |
| 5 | `op_copiar_tabela` | `destino` | 11539 |
| 6 | `op_duplicar_tabela` | `destino` | 16499 |
| 7 | `op_renomear_tabela` | `destino` | 16532 |
| 8 | `op_dblink_ligar` | `local_tabela` | 21974 |
| 9 | `op_dblink_sincronizar` | `local_tabela` | 22057, 22064 |
| 10 | `op_posicao` | nome por database | 23529 |

**Não conta:** `op_selecionar_memoria` (`:23121`) lê o **mesmo**
`pedido["tabela"]` — é cinto, não campo escondido.

**O número da lei está velho: 7 de 116 → 10 de 140.** É achado de brinde
deste parecer, e a lei deve ser corrigida quando alguém tocar naquele
parágrafo.

### 5.2 `op_consultar` não paga conferência própria — e o motivo é medido

Não há um único `pode_em` entre `servidor.rs:12306` e `:12806`. O motivo está
no fonte: cada sub-pedido entra por `linhas_do_sub_pedido` (`:12166`), que
chama `executar_derivado` (`:12193`), que é

```
politica_do_pedido → portoes_do_pedido → aplicar_direito_por_coluna
```

(`servidor.rs:6907-6916`) — **o portão inteiro, o mesmo do `despachar`**. E o
`direito_coluna.rs:519-521` diz o mesmo com outras palavras: *«cada sub-pedido
paga a dele no `executar_derivado`»*.

### 5.3 Quantas conferências próprias cada via cria — a resposta pedida

| | Via (a): `agrupar.de` | Via (b): `consultar.por/agregados` |
|---|---|---|
| `pode_em` próprio novo em `servidor.rs` | **0** | **0** |
| Ramo novo em `direito_coluna::tabelas_do_pedido` | **1** (o 8.º) | **0** |
| Re-decisão de `direito_coluna::CLASSES` | **1** (`agrupar` hoje é `Recusa`) | **0** |
| **Total de mexidas na maquinaria do portão** | **2** | **0** |

**Por que a via (a) precisa do ramo novo, e não é opinião.** Hoje
`tabelas_do_pedido("agrupar", p)` cai no `_ => {}` (`direito_coluna.rs:537`) e
devolve só `p["tabela"]`. Num pedido `{"op":"agrupar","de":{…}}` esse campo
está **vazio**, a lista sai **vazia**, e `aplicar_direito_por_coluna` entra no
ramo `None if alvos.is_empty()` (`servidor.rs`, ramo `PorColuna::Recusa`) e
**recusa todo usuário com qualquer regra de coluna**, dizendo *«esta operacao
nao diz que tabela alcanca»*. Sem o ramo novo, a via (a) nasce quebrada para
esses usuários.

**Por que a via (a) precisa re-decidir a classe.** `agrupar` é
`PorColuna::Recusa` (`direito_coluna.rs:149`) e o motivo está escrito ali:
*«o AGREGADO fala dela sem ela aparecer — `{"funcao":"maximo","coluna":"salario"}`
devolve o maior salario num campo chamado `maximo_salario`, e a peneira, que
procura pelo NOME da coluna, nao acha nada para tirar.»* Isso é verdade do
`agrupar` que lê a tabela **direto** — e **falso** do `agrupar` que receba
`de`, porque aí as linhas já vieram peneiradas. Uma operação com **duas**
semânticas de direito por coluna conforme o campo presente é precisamente a
forma da porta dos fundos que a pétrea manda evitar.

### 5.4 O direito por coluna fecha SOZINHO na via (b) — e isto é ganho, não empate

Medido em três pontos do fonte:

1. `modelo_da_tabela` (`servidor.rs:12247-12272`) monta o modelo do sub-pedido
   **pulando** as colunas de `colunas_sem_leitura` (`:12256,12264`).
2. `PorColuna::Le` peneira o valor da resposta **e** recusa a pergunta
   (`recusar_pergunta_sobre_coluna_negada`) antes dela.
3. Logo, num `consultar` com agregados, a coluna negada **não existe no
   modelo**. `agregados:[{"coluna":"salario"}]` cai em `resolver_ou_recusar`
   e recusa **nomeando** — sem peneira nova, sem classe nova.

**Consequência:** a via (b) dá de graça uma capacidade que hoje não existe —
**agregar sob regra de coluna**, onde o `agrupar` de hoje simplesmente recusa.
A via (a) não dá: o `agrupar` continuaria `Recusa`.

### 5.5 O caso do `pivotar`: conta CONTRA a via (a), e o motivo não é o aninhamento

A pergunta do pedido era se repetir o padrão do `pivotar` conta a favor ou
contra. **Medido: conta contra, e a causa não é aninhar.**

- `op_pivotar` paga conferência própria (`:13121`) porque abre as tabelas de
  consulta **direto**: `db.abrir_qualificada(nome)` (`:13163`), com `nome`
  vindo de `j.texto_ou("tabela", "")`. Ele aninha um **NOME**.
- `op_consultar` não paga nada porque aninha um **PEDIDO**, e pedido volta
  pelo portão único.
- `op_unir` prova os dois lados na mesma função: paga conferência própria pelo
  campo legado `tabelas` (nomes, `:21514`), e **não** paga pelo campo `partes`
  (pedidos), que o pedido 393 introduziu justamente com a frase *«Roteia por
  `executar_derivado`, então o portão continua sendo UM»*.

**A lei que sai daí, e é o achado deste parecer:** *aninhar um nome de tabela
custa uma conferência própria; aninhar um pedido custa zero.* A via (a) só
seria barata se o `de` fosse pedido — e aí ela vira uma cópia pior da
composição que já existe.

### 5.6 O fonte já previu esta decisão, e contra a via (a)

`servidor.rs:12819-12826`, no doc-comment do `op_agrupar`:

> *«Nao ha conferencia propria aqui porque nao ha segunda tabela escondida em
> lugar nenhum, ao contrario do `juntar`, do `unir` e do `pivotar`. **No dia em
> que o `agrupar` ganhar um `de` aninhado, ele passa a precisar de uma** — e a
> pergunta que decide e "esta operacao nomeia tabela onde o portao nao olha?".»*

Não é evidência de fora: é o próprio código dizendo o preço da via (a) antes
de alguém a propor.

### 5.7 As pétreas, uma a uma

| Pétrea | Via (b) | Nota |
|---|---|---|
| Zero dependências externas | **não toca** | nenhuma crate |
| Ordem de digitação sagrada | **não toca** | leitura pura |
| Integridade primordial (nunca matar pai com filhos) | **não toca** | leitura pura |
| Portão de permissão é UM só | **reforça** | 0 conferências próprias novas (§5.3) |
| Guarda nova entra pedida, não imposta | **cumpre** | sem `por`/`agregados`, o `consultar` é byte a byte o de hoje |
| Mudança de formato entra cedo | **não toca** | nenhum byte de `.reg`/`.ndx`/`PSCH` muda |
| Senha nunca em texto puro | **não toca** | — |

**A via (a) flerta com a de formato**, e é preciso dizer: para agregar sobre
linhas nomeadas ela precisaria de um `Schema`, e `Schema::new`
(`crates/phxsql-core/src/schema.rs:887`) **injeta a coluna de sistema
`COLUNA_SOFTDELETED`** quando quem chama não a declara. Usar um tipo de
formato em disco para um resultado intermediário que não tem arquivo é
território do papel C, e o efeito é uma coluna fantasma no `esquema.colunas()`
que o agregador usa para tipar (`agrupar.rs:91`). **Risco nomeado, não
medido** — não executei.

---

## 6. As hipóteses que morreram, com o número que as matou

### 6.1 H1 — vencedor certo, mecanismo errado

Eu previ que a via (a) criaria **conferência própria** nova. **Medido: cria
zero** `pode_em` novo, desde que o `de` roteie por `executar_derivado`. O que
ela cria são **2** mexidas de outra natureza (§5.3): um ramo em
`tabelas_do_pedido` e uma re-decisão de `CLASSES`.

**É exatamente a forma do pedido 113** — alvo certo, causa errada — e vale
registrar porque a lei da casa manda: *medir a premissa do item vem antes de
implementar o item, inclusive quando o item é nosso*. Se eu tivesse parado na
hipótese, teria escrito no parecer um custo que não existe e perdido os dois
que existem.

### 6.2 H2 — a evidência que o pedido pediu, e que decide o CONTRÁRIO do que parecia

O pedido enquadrou assim: *«O plano deles tem um nó de junção alimentando um
nó de agregação (pipeline), ou a agregação carrega a junção dentro? Isso é a
evidência que decide entre (a) e (b).»*

**Medido: é pipeline nos quatro (§3.2), e essa evidência NÃO decide entre (a)
e (b).** Ela é verdade da camada errada. No PostgreSQL o `Agg` literalmente
carrega a junção no `lefttree` — ou seja, o plano dele **é** a forma (a). E
mesmo assim ninguém escreve isso: o cliente escreve um `SELECT` só, porque a
camada que o cliente toca é a `Query`, não o `Agg`.

**O que decide é a camada da requisição, e ali são 4 de 4 para (b).** A
hipótese morreu sobre um fato verdadeiro — que é o jeito mais caro de errar,
porque o fato sobrevive à refutação e volta na rodada seguinte. Fica
registrado para não voltar.

### 6.3 H3 — `agrupar` como açúcar de `consultar`: morreu com três números

| Medida | `op_agrupar` hoje | Pela composição |
|---|---|---|
| Linhas que o agregado enxerga | **a tabela inteira** — `t.varrer()` sem `max` (`servidor.rs:12965`); o `max` limita **grupos** (`:13045`) | **no máximo `recursos.max_linhas`**, que nasce **1.000** (`config.rs:3552`) |
| O que o `max` quer dizer no `varrer` | — | **linhas EXAMINADAS**, não devolvidas (comentário de `varrer_a_pagina`) |
| Detecção do corte | — | **não há.** `linhas_do_sub_pedido` recusa se `len() > teto` (`:12200`); um sub-pedido que parou **em** `teto` passa calado |
| Materialização | `Value` direto da página; só os **grupos** viram `Json` | `Vec<Linha>` inteiro, `Linha = Vec<(String, Json)>` (`consultar.rs:46`) — uma `String` de nome por célula por linha |

**O número de memória já estava medido nesta casa**, e eu o cito como citado,
não como medido por mim: `servidor.rs:12415-12417` registra **+561 MiB para
1.000.000 de linhas** materializadas na composição (medido em 1000 × 1000 com a
mesma chave) — **≈588 bytes por linha**.

**O que mata a hipótese não é a memória; é o número errado calado.** Um
`SELECT COUNT(*) FROM vendas` numa tabela de 1.000.000 de linhas responde
1.000.000 hoje e responderia **1.000** como açúcar, **sem dizer que cortou**.
Esta casa não faz isso.

**E há o motivo estrutural por trás do número:** os quatro motores têm **um**
nó de requisição porque têm um **planejador** que escolhe o caminho físico
depois. Nós não temos planejador — zero dependências, nenhum modelo de custo.
Sem planejador, a escolha física tem de estar **visível no protocolo**, ou
alguém a faz por engano.

---

## 7. A recomendação, e a divergência nomeada

**Entra a via (b) como SUBSTRATO. `agrupar` permanece, e permanece sendo a
escolha certa para uma tabela só.**

As duas portas passam a ter papéis nomeados no contrato:

| Porta | O que é | Quando |
|---|---|---|
| `agrupar` | flui do disco, não materializa linha, agrega sobre a tabela **inteira**, teto sobre **grupos** | `GROUP BY` sobre **uma** tabela |
| `consultar` com `por`/`agregados` | compõe, materializa sob `max_linhas`, agrega sobre o que a composição produziu | `GROUP BY` sobre **junção**, subconsulta no `FROM`, `WITH`, `UNION` |

### 7.1 Onde a nossa lógica DIVERGE da de origem, e qual restrição causa

*(A pergunta que a lei da casa manda responder. «Em lugar nenhum» seria a
prova de que a lógica passou pelos nossos dedos e não pela nossa cabeça.)*

1. **Duas portas onde eles têm uma.** Nos quatro há um nó de requisição só,
   porque o planejador escolhe o físico. **Restrição nossa:** não há
   planejador nem modelo de custo — consequência direta de *zero dependências
   externas* e do tamanho do motor. Sem ele, a escolha física sobe para o
   contrato, e a diferença fica **legível** em vez de ficar num `EXPLAIN` que
   não temos.

2. **Agregados em array próprio, não na lista de projeção.** Os quatro põem
   `SUM(x)` no `targetList`/`pEList`/`item_list` porque a projeção deles é uma
   **árvore de expressão**. A nossa `consultar.colunas` é uma **lista de
   nomes**. **Restrição nossa:** a projeção do protocolo não é linguagem de
   expressão. E a casa **já resolveu o problema idêntico**: `janela` é um
   array separado com `apelido`, referenciado por nome em `colunas`
   (`consulta.rs:1904`). Funções de janela são a irmã mais próxima dos
   agregados nos quatro motores; seguir o padrão interno vale mais que copiar
   o externo.

3. **O teto é sobre GRUPOS no `agrupar` e sobre LINHAS na composição.** Nos
   quatro, `LIMIT` é sempre sobre o resultado. **Restrição nossa:** a
   composição guarda cada sub-pedido inteiro em memória e o `agrupar` não —
   são custos diferentes e um teto só mentiria sobre um dos dois.

4. **`tendo` (HAVING) é conferido contra os nomes que existem DEPOIS de
   agrupar** (`servidor.rs:12939-12959`), recusando `preco > 10` num grupo de
   mil linhas. PostgreSQL e os outros aceitam colunas agrupadas e agregados
   com regras mais frouxas em torno de funcionalidade. **Restrição nossa:**
   recusa nomeada acima de aceitar torto — a mesma lei que o pedido 394
   invoca. Este comportamento **já existe** no `agrupar` e deve ser
   **reaproveitado**, não reescrito.

### 7.2 A ordem dos passos — e ela vem do fonte do PostgreSQL

O `op_consultar` já executa em ordem fixa (`servidor.rs:12275-12290`):
`de` → `juntar` → `escalar` → `existe` → `em` → `expressao` → **[vaga]** →
`janela` → `ordem` → `pular`/`max` → `colunas`.

A vaga é **entre `expressao` e `janela`**, e isso não é escolha: é o que
`planner.c:1641,1785,1803` faz — `query_planner` (FROM+JOIN+WHERE) →
`create_grouping_paths` → `create_window_paths`. Janela **depois** de
agrupar, nos quatro. `tendo` entra colado ao agrupamento, antes de `janela`.

**Marcadores de seção existentes para orientar quem implementar:** `juntar`
`:12317`, `escalar` `:12453`, `existe` `:12538`, `em` `:12597`, `janela`
`:12670`.

---

## 8. Esboço do contrato JSON

### 8.1 Campos novos em `consultar` — três, todos opcionais

```json
{
  "op": "consultar",
  "database": "loja",

  "de":     { "op": "varrer", "tabela": "pedidos" },
  "apelido": "p",
  "juntar": [ { "de": { "op": "varrer", "tabela": "clientes" },
                "apelido": "c", "tipo": "interno",
                "em": [ { "esquerda": "p.cliente_id", "direita": "c.id" } ] } ],
  "expressao": "c.uf = 'SC'",

  "por": ["c.cidade"],

  "agregados": [
    { "funcao": "contagem",                      "apelido": "n" },
    { "funcao": "soma",   "coluna": "p.total",   "apelido": "faturado" }
  ],

  "tendo": "n > 1",

  "janela": [],
  "ordem":  [ { "coluna": "faturado", "desc": true } ],
  "max": 100,
  "colunas": ["c.cidade", "n", "faturado"]
}
```

| Campo | Tipo | Ausente = | Regra |
|---|---|---|---|
| `por` | lista de nomes de coluna (aceita qualificado `c.cidade`) | sem agrupamento | resolve por `resolver_ou_recusar` contra o modelo corrente — **função que já existe**; lista vazia **com** `agregados` = agregado global (uma linha), igual a `agrupar` com `por: []` |
| `agregados` | lista de `{funcao, coluna?, apelido?}` | sem agregação | `funcao` ∈ `soma`·`media`·`contagem`·`minimo`·`maximo`·`distintos` (`pivot.rs:35-60`); `coluna` obrigatória para todas menos `contagem` (`Agregador::precisa_de_valor`); `apelido` cai no `apelido_padrao` (`agrupar.rs`) |
| `tendo` | texto | sem peneira | conferido contra `por` ∪ apelidos, **nunca** contra o esquema — reusa `servidor.rs:12939-12959` |

**Combinação que RECUSA nomeando:** `agregados` presente e `por` ausente
**junto de** `colunas` citando uma coluna que não é `por` nem apelido. É o
`GROUP BY` com coluna não agregada, já decidido por média ponderada nesta
casa: **erra** (PG 4 + MySQL 2 = 6 contra MariaDB 3 + SQLite 1 = 4,
`docs/propostas/semantica-4-motores.md`). **Decisão já tomada — não se
remede.**

### 8.2 O que o portão tem de conferir: **NADA DE NOVO**

E isso é o resultado, não uma omissão. A conferência, item a item:

| Pergunta do portão | Onde já é respondida |
|---|---|
| Este usuário pode ler a tabela de cada lado? | em cada sub-pedido, por `executar_derivado` → `portoes_do_pedido` (`:6907`) |
| Alguma coluna é negada a ele? | em cada sub-pedido, por `aplicar_direito_por_coluna`; o modelo nem a nomeia (`:12256,12264`) |
| `consultar` precisa de ramo em `tabelas_do_pedido`? | **já tem**, `direito_coluna.rs:522-536` — e `por`/`agregados`/`tendo` nomeiam **coluna**, nunca tabela |
| `consultar` muda de classe em `CLASSES`? | **não.** Continua `PorColuna::Nenhum` (`:120`), pelo motivo escrito ali |
| Alguma tabela fica escondida num campo novo? | **não.** Os três campos novos carregam nome de coluna e texto |

**A pergunta da pétrea, respondida em uma linha:** *«esta operação nomeia
tabela onde o portão não olha?»* — **não**, e continua não nomeando depois
da mudança.

### 8.3 Resposta

Mesma forma de hoje (`servidor.rs:12795-12805`) — `devolvidas`, `achadas`,
`colunas`, `linhas`, `ms`. O `colunas` passa a trazer os apelidos dos
agregados com o tipo vindo de `pivot::tipo_do_agregado`/`decimal_e_escala`
(`servidor.rs:12921-12928`) — **um lugar, não dois**, como o `agrupar` já faz.

### 8.4 O que o tradutor SQL faz (pedido 394)

`consulta.rs` deixa de chamar `recusa_de_agregacao_composta` e passa a
preencher, no pedido que **já constrói**, os três campos a partir de
`Selecao.agrupar_por` (`:1218`), `Selecao.tendo` (`:1219`) e
`Projecao::Agregada` (`sintaxe.rs:83`) — que **já existem e hoje saem
vazios**. É preenchimento, não estrutura nova. A coluna fantasma `"SUM"` de
`item_de_projecao_composta` (`consulta.rs:~998-1024`) morre no mesmo passo,
porque o terceiro braço que falta passa a ter para onde ir.

---

## 9. O que eu NÃO medi — e o que decidiria na bancada

**Modo honesto: nenhum número deste parecer saiu de uma corrida minha.** Não
compilei nem rodei bancada — há quatro frentes vivas escrevendo em
`table.rs`, `catalogo.rs`, `direito_coluna.rs`, `config.rs`, `servidor.rs`,
`usuarios.rs`, e um `cargo build` meu disputaria a árvore. Os números são de
**leitura de fonte** (exatos: constantes, assinaturas, contagens) e de
**medições anteriores citadas com a linha** (o 561 MiB). Está dito onde cada
um é qual.

| Não medido | O que decidiria na bancada |
|---|---|
| Custo real de agregar 1e6 linhas pelo `agrupar` × pela composição | `--example onde-doi` estendido: `agrupar` puro contra `consultar{de:varrer}` com o mesmo `por`, com faixa min–max e a regra do pedido 155 (vencedor só se contorna quando as faixas não se cruzam) |
| Custo da conversão `Value`↔`Json` por célula na composição | contar as conversões por dentro, como o medidor de toques de página já faz — **não estimar** |
| Se `Schema::new` injetando `COLUNA_SOFTDELETED` quebraria a via (a) na prática | só importa se a via (a) voltar; está registrada como risco nomeado, não como falha medida |
| Quantos `SELECT` reais do Query Designer (pedido 305) o contrato novo passa a aceitar | repetir a medição do papel E — hoje **0 de 8**, das quais **4** caem no 394 — depois de implementado. É o número que prova o ganho, e ele só existe depois |

---

## 10. Lacunas e pendências que este parecer abre

1. **A lei da casa diz «7 das 116 operações»; medido hoje são 10 de 140**
   (§5.1). Não editei o `CLAUDE.md` — não é meu arquivo. Fica registrado.
2. **`consulta.rs:1276-1281` afirma «isso e decisao do dono»**, escrito antes
   da pétrea de 23/09/2026. Caducou (§4). O comentário precisa mudar quando a
   implementação entrar. Não o toquei.
3. **`linhas_do_sub_pedido` não detecta sub-pedido que parou EM `max_linhas`**
   (`:12200` compara `>` e não `>=`, e o sub-pedido cortado não diz que
   cortou). Não é do escopo deste parecer, mas a composição publica hoje
   resultado parcial sem dizer. **Vale pedido próprio.** Achado lendo, não
   medindo — não executei o caso.
4. **`agrupar` continua `PorColuna::Recusa`.** Com a via (b), quem tem regra
   de coluna passa a ter um caminho que funciona (`consultar` com agregados),
   e a mensagem de recusa do `agrupar` deveria apontar para ele. Hoje ela
   sugere outra coisa (`dc::saida("agrupar")` = `Saida::Resumo`).

---

## 11. Uma linha por pergunta do pedido

| Pergunta | Resposta medida |
|---|---|
| Como cada motor representa agregado sobre junção? | **Requisição:** um nó só, 4/4 (§3.1). **Plano:** pipeline, 4/4 (§3.2). |
| Pipeline ou agregação carregando a junção? | Pipeline — **e isso é da camada errada para decidir o protocolo** (§6.2). |
| Quantas conferências próprias cada via cria? | (a) **2** mexidas no portão; (b) **0** (§5.3). |
| A via (a) repete o padrão do `pivotar` — a favor ou contra? | **Contra**, e a causa medida é aninhar **nome** em vez de **pedido** (§5.5). |
| Via escolhida | **(b)**, sem a metade do açúcar. |
| Número que decidiu | **0 × 2** no portão; **10 × 0** na ponderada que nem foi precisa; **4/4** de convergência. |
| Hipótese que morreu | **H2** — o pipeline é verdade e não decide (§6.2). E **H3** — o açúcar custaria `1.000` no lugar de `1.000.000` (§6.3). |
| Sobe à mesa do dono? | **Não.** Aceite automático. |
