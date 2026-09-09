# As dezoito do comparativo — divisão em frentes e contratos

Ordem do dono, 08/09/2026, sobre a tabela «E o comparativo, medido contra
quem tem»: *«Falta esses itens.»* A fonte é `docs/COMPARATIVO.md` (18 de 19
faltando ou pela metade, medido em 07/09/2026), e a régua continua sendo a
mesma: **uma linha só vira TEM pelo medidor** — `bancada/comparativo/medir.py`
seguido de `documento.py` —, nunca à mão.

Este documento é o que as frentes compartilham antes de existir código: os
contratos JSON de cada peça nova, para que a camada SQL, o servidor e o motor
possam andar em paralelo sem inventar três formas para a mesma coisa.

## O que já existe por baixo, medido antes de dividir

Levantamento de 08/09/2026, com arquivo e linha, e ele muda o tamanho de
várias frentes:

- **Avaliador de expressões**: existe na linguagem dos gatilhos
  (`crates/phxsql-sql/src/rotina.rs`, `Expr`/`Op2`/`Funcao`), com aritmética,
  comparação, E/OU, `NEW.`/`OLD.` e oito funções. Não serve ao motor porque
  mora na crate de SQL, e o motor (`phxsql-store`) não depende dela. A
  expressão de esquema (CHECK, DEFAULT, calculada, índice) tem de morar no
  `phxsql-core`, que os dois enxergam.
- **Agregador**: `pivot.rs` já tem `Soma, Media, Contagem, Minimo, Maximo,
  ContagemDistinta` com acumulador exato para `Decimal`. `GROUP BY` genérico é
  o mesmo acumulador sem o formato linha × coluna.
- **Upsert por chave única**: `dblink/sincronia.rs::aplicar_para_ca` faz
  `buscar` no índice único e decide `atualizar` ou `inserir`. Só o DbLink o
  usa.
- **Composição de operações**: `pivotar` aceita um `juntar` aninhado; `juntar`
  só aceita tabela real. Não há op que receba o resultado de outra.
- **Diário com carimbo e imagem**: `log.rs::Evento{carimbo, operacao, rowid,
  versao}` e `diario_com_imagem`. O `replicar` entrega por posição, não por
  instante; o `restaurar_backup` não reaplica nada.
- **Permissão**: por base e por tabela (`usuarios.rs:592-618`), portão único
  em `portoes_do_pedido`. Nada por coluna.
- **Filtro do `varrer`**: `Filtro{coluna, op, valor}` — coluna contra literal,
  sem expressão.

## As frentes, e o modelo de cada uma

Nível e motivo, nunca o nome (decisão do dono, `docs/MODELOS.md`).

| frente | o que entrega | modelo | por quê |
|---|---|---|---|
| **F-NÚCLEO** (papéis C e B, o orquestrador) | `phxsql_core::expressao`; PSCH v9 (`padrao`, `check`, `calculada` por coluna; `onde` e expressão por índice); caminho de escrita da `Table`; índice parcial e por expressão; `criar_tabela` lendo os campos novos; `FORMATO.md` | forte | formato em disco e caminho de escrita |
| **F-SQL** | gramática e tradução: `GROUP BY` e agregados, expressão no `WHERE`, `WITH`, `IN (SELECT …)`, `ROW_NUMBER() OVER`, `CREATE/DROP VIEW`, `INSERT … ON CONFLICT` / `ON DUPLICATE KEY`, `?` | leve | tradução mecânica para contratos já escritos, provada por teste de unidade |
| **F-CONSULTA** | ops `agrupar`, `consultar`, `criar_visao`/`visoes`/`excluir_visao`, `diferencas`; `varrer.expressao`; `inserir.se_existir`; `sql.parametros`; catálogo | forte | toda sub-consulta passa pelo portão de permissão, e o portão é um só |
| **F-DIREITO** | direito por coluna | forte | segurança: quem esquecer uma op abre a porta dos fundos |
| **F-PITR** | restaurar a um instante | forte | durabilidade e ordem de reaplicação |
| **F-BANCADA** | sondas vivas para o que hoje é sonda de código; remedição; `SQL.md`, `MANUAL.txt`, `CHANGELOG`, `PENDENCIAS`, cognições | leve | roteirizado e verificável |

**Ficam para decisão do dono**, com o custo na mesa em vez de código:

- **Nível de isolamento acima de `READ COMMITTED`** — é a Sombra
  (`docs/SOMBRA.md`), que o dono deixou parada em 05/09. Aceitar `SET
  TRANSACTION ISOLATION LEVEL SERIALIZABLE` sem dar a garantia seria mentira
  com aparência de capacidade.
- **TLS no transporte** — a pétrea das zero dependências. Fazer TLS 1.3 em
  casa exige X.509/ASN.1, assinatura ECDSA P-256 (Ed25519 os navegadores não
  aceitam em certificado) e gestão de certificado; a cifra do fio (Noise) já
  protege a porta de dados. A auditoria de 0.18 propôs abrir exceção só na
  camada de rede.

## Os contratos

### A expressão, em texto

Uma gramática só, para CHECK, DEFAULT, coluna calculada, índice parcial,
índice por expressão, `varrer.expressao`, `agrupar.tendo` e
`consultar.expressao`:

- literais: `12`, `1.1`, `'texto'`, `TRUE`, `FALSE`, `NULL`;
- coluna pelo nome, sem distinguir caixa;
- `+ - * /`; `= <> != < <= > >=`; `AND OR NOT`; `IS [NOT] NULL`;
  `IN (lit, …)`; `BETWEEN a AND b`; `LIKE` com `%` e `_`; parênteses;
- funções: `UPPER LOWER TRIM LENGTH ROUND ABS COALESCE CONCAT` — as mesmas
  dos gatilhos, com a mesma semântica (`LENGTH` conta caracteres).
- Número: inteiro exato, decimal exato (a escala do literal), real. `NULL`
  propaga; comparação com `NULL` dá `NULL`. **CHECK passa quando dá `NULL`**
  (é o SQL); **filtro exclui quando dá `NULL`**.

API, em `phxsql_core::expressao`:

```rust
let e = Expressao::analisar("preco * 1.1 > 100")?;
e.colunas();                       // nomes referidos, para recusar na declaração
e.avaliar(&|nome| linha.get(nome)) // -> Result<Value>; o resolvedor devolve (Value, ColumnType)
e.avaliar_bool(&resolvedor)        // -> Result<Option<bool>>  (None = NULL)
expressao::coagir(valor, &tipo)    // -> Result<Value>, para DEFAULT, calculada e chave de índice
```

### `criar_tabela` — os campos novos

```json
{"colunas": [
   {"nome": "v", "tipo": "Int8", "padrao": "7"},
   {"nome": "w", "tipo": "Int8", "check": "w > 0"},
   {"nome": "b", "tipo": "Int8", "calculada": "a * 2"}],
 "indices": [
   {"nome": "so_positivo", "colunas": ["v"], "onde": "v > 0"},
   {"nome": "por_baixo", "colunas": ["lower(nome)"]}]}
```

- `padrao`: expressão avaliada no **inserir** quando a coluna vem nula. Só no
  inserir: o `atualizar` recebe a linha inteira, e nulo ali é nulo.
- `check`: avaliada no inserir e no atualizar com a linha inteira; `FALSE`
  recusa nomeando a coluna e a expressão; `NULL` passa.
- `calculada`: **sempre recalculada na gravação, e o valor que vier no pedido
  é ignorado.** Não é «calado»: a coluna não tem dado próprio por definição, e
  o motivo técnico é que o protocolo não distingue ausente de presente — o
  merge do `UPDATE` devolve o valor velho junto com a linha, e recusá-lo
  quebraria todo UPDATE.
- `onde` no índice: a linha só entra no índice quando a expressão dá `TRUE`.
  No `atualizar`, sai se saiu do filtro e entra se entrou.
- expressão de índice: expressão de **uma** coluna; a chave é o resultado,
  codificado com o tipo daquela coluna. Mais de uma coluna recusa.
- Recusa na **declaração** (a mesma decisão do `ao_excluir`): coluna
  inexistente na expressão, sintaxe inválida, expressão de índice com mais de
  uma coluna, `calculada` que referencia outra `calculada`.
- Na réplica nada disso é julgado nem recalculado (`julga_integridade`): a
  imagem que chega já veio com tudo aplicado na origem.

### `varrer`, `contar` e as paginações

```json
{"op": "varrer", "database": "b", "tabela": "c", "expressao": "preco * 1.1 > 100"}
```

Filtro avaliado por linha, junto com `onde`. Vale nos mesmos caminhos em que
`onde` vale — a frente conta os chamadores e põe a avaliação no lugar único
que decide «esta linha passa?».

### `agrupar`

```json
{"op": "agrupar", "database": "b", "tabela": "c",
 "por": ["cidade"],
 "agregados": [{"funcao": "contagem", "apelido": "n"},
               {"funcao": "soma", "coluna": "preco", "apelido": "total"}],
 "onde": [], "expressao": "", "tendo": "n > 1",
 "ordem": [{"coluna": "n", "desc": true}], "max": 1000}
```

Resposta: `{"grupos": 3, "linhas": [{"cidade": "Blumenau", "n": 2, "total":
"20.00"}]}`. `funcao` ∈ `contagem soma media minimo maximo distintos` (as do
`pivotar`). `por` vazio = um grupo só. Apelido padrão: `contagem`,
`soma_preco`. `Decimal` sai como texto, como em todo lugar.

### `consultar` — composição

```json
{"op": "consultar", "database": "b",
 "de": {"op": "varrer", "tabela": "c"},
 "expressao": "preco > 10",
 "em": [{"coluna": "id", "de": {"op": "varrer", "tabela": "c"}, "campo": "id"}],
 "janela": [{"funcao": "row_number", "particao": ["cidade"],
             "ordem": [{"coluna": "id"}], "apelido": "n"}],
 "colunas": ["id", "nome", "n"],
 "ordem": [{"coluna": "id", "desc": false}], "pular": 0, "max": 1000}
```

- `de` é qualquer pedido que devolva `linhas`: `varrer`, `buscar`, `agrupar`,
  outro `consultar`. **Cada sub-pedido roda por `executar_derivado`** — o
  mesmo portão de permissão de qualquer cliente. É isto que faz do
  `consultar` uma composição e não uma porta dos fundos, e há teste com
  tabela negada no lado de dentro.
- `em`: `IN (SELECT campo FROM …)` — o sub-pedido roda primeiro e vira um
  conjunto.
- `janela`: `row_number` (e só ele nesta rodada), com partição e ordem.
- Teto de linhas em memória por sub-pedido: o `max_linhas` da configuração,
  o mesmo do `varrer`.
- Resposta: `{"devolvidas": n, "linhas": [...]}`.

### Junções e subconsultas no `consultar` — acréscimo do dono, 08/09 16:50

Ordem: *«Select com sub selects e where's com inner joins, joins…»*. Entra no
mesmo `consultar`, como passos a mais, e o avaliador do core passou a aceitar
o nome **qualificado** (`p.id`) como um token só, porque duas tabelas na
mesma linha têm duas colunas `id`.

```json
{"op": "consultar", "database": "b",
 "de": {"op": "varrer", "tabela": "pedidos"}, "apelido": "p",
 "juntar": [
   {"de": {"op": "varrer", "tabela": "clientes"}, "apelido": "c",
    "tipo": "interno",
    "em": [{"esquerda": "p.cliente_id", "direita": "c.id"}]}],
 "escalar": [{"nome": "media", "de": {"op": "agrupar", "tabela": "pedidos",
              "agregados": [{"funcao": "media", "coluna": "total"}]},
              "campo": "media_total"}],
 "expressao": "c.cidade = 'Blumenau' AND p.total > media",
 "colunas": ["p.id", {"coluna": "c.nome", "apelido": "cliente"}],
 "ordem": [{"coluna": "p.id"}], "max": 1000}
```

- **`apelido`** do `de` e de cada junção: depois de uma junção, toda coluna da
  linha se chama `apelido.coluna`. Nome sem prefixo resolve quando é único
  entre os lados; ambíguo recusa nomeando os dois.
- **`juntar`**: lista, aplicada na ordem (junção à esquerda, uma por vez).
  `tipo` ∈ `interno` (só quem casa) | `esquerdo` (a linha da esquerda fica,
  com as colunas da direita nulas). `em` é igualdade entre pares de colunas
  (junção por espalhamento em memória); qualquer outra condição vai para
  `expressao`. `direito`, `completo` e `cruzado` **recusam nomeando** nesta
  rodada — a direita se escreve trocando os lados.
- **`escalar`**: subconsulta **não correlacionada** que tem de devolver
  exatamente uma linha; o `campo` dela vira uma coluna com o `nome` dado,
  visível em `expressao`. Zero ou duas linhas recusa nomeando.
- **`em`** (já no contrato) continua sendo o `IN (SELECT …)`.
- **Correlação** (subconsulta que cita coluna de fora) e `EXISTS` **recusam
  nomeando** nesta rodada: exigiriam executar a subconsulta por linha.
- Cada `de` — o principal, o de cada junção, o de cada `escalar`/`em` — roda
  por `executar_derivado`: o portão é o mesmo em todos, e o teste da tabela
  negada vale para cada um deles.

Na camada SQL: `SELECT p.id, c.nome AS cliente FROM pedidos p [INNER] JOIN
clientes c ON p.cliente_id = c.id [LEFT JOIN …] WHERE … [ORDER BY …] [LIMIT]`
→ o `consultar` acima. `ON` com `AND` de igualdades vira mais pares em `em`;
condição que não é igualdade de colunas vai para `expressao`. Subconsulta
escalar no WHERE (`preco > (SELECT AVG(preco) FROM c)`) → `escalar`. `JOIN
(SELECT …) AS y` → o `de` da junção é o sub-pedido.

### Visões

```json
{"op": "criar_visao", "database": "b", "nome": "v_c", "sql": "SELECT * FROM c"}
{"op": "visoes", "database": "b"}
{"op": "excluir_visao", "database": "b", "nome": "v_c"}
```

A op `sql` resolve `FROM v_c` para o plano da visão dentro de um `consultar`.
A visão guarda **texto**, e é analisada a cada uso — visão que aponta para
tabela que sumiu recusa na hora de usar, nomeando a tabela. Persistência por
banco, no molde que as diretivas já usam; a frente diz qual arquivo.

### `inserir` com `se_existir`

```json
{"op": "inserir", "database": "b", "tabela": "c", "linha": {"id": 1},
 "se_existir": "ignorar" | "atualizar", "indice": "porId"}
```

Resposta: `{"rowid": n, "ignorada": true}` ou `{"rowid": n, "atualizada":
true}` ou a de sempre. `indice` opcional: o primário, ou o único índice único
da tabela; ambíguo recusa nomeando os candidatos. É o `aplicar_para_ca` do
DbLink, extraído para um lugar só e usado pelos dois.

### `sql` com parâmetros

```json
{"op": "sql", "database": "b", "sql": "SELECT * FROM c WHERE id = ?", "parametros": [1]}
```

Cada `?` vira o literal da posição, **no léxico**, nunca por substituição de
texto. Contagem diferente recusa nomeando quantos vieram e quantos faltam.

### `diferencas`

```json
{"op": "diferencas", "database": "b", "a": "c1", "b": "c2", "indice": "porId", "max": 1000}
```

Resposta: `{"so_em_a": [[chave]], "so_em_b": [[chave]], "diferentes":
[{"chave": [1], "colunas": ["nome"], "a": {...}, "b": {...}}], "iguais": n}`.
As duas tabelas passam pelo portão (conferência própria, como no `juntar`).

### Direito por coluna

```json
{"tabelas": {"c": {"ler": true, "alterar": true,
                   "colunas": {"salario": {"ler": false, "alterar": false}}}}}
```

Sem `colunas`, **nada muda** — é o teste que mais importa. Leitura: a coluna
sai da resposta de `ler`, `varrer`, `buscar`, das paginações, do `sql` e do
`consultar`. Escrita: pedido que nomeia a coluna negada é recusado. Ops que
devolvem linha por outro caminho (`exportar`, `juntar`, `unir`, `pivotar`,
`diario`, …) **recusam** a tabela para quem tem restrição de coluna nela —
recusar é mais seguro que vazar, e a lista dessas ops é medida e escrita no
`docs/SEGURANCA.md`.

### PITR

```json
{"op": "restaurar_backup", "...": "...", "ate": "2026-09-08T15:00:00Z"}
```

Restaura a cópia e reaplica o diário **vivo** de cada tabela, do instante da
cópia até `ate`, pelo caminho de aplicação da réplica (aplica, não julga).
Exige imagem no diário; sem ela recusa dizendo qual interruptor ligar.
Resposta: por tabela, eventos reaplicados e o último carimbo aplicado.

## A prova real de cada linha

O medidor já existe para as treze linhas de SQL. Para as cinco sondas de
código que hoje devolvem `NAO` cravado (TLS, coluna, PITR, parâmetro,
diferenças), a F-BANCADA as troca por sondas **vivas** — que exercitam o
servidor e conferem o efeito, com o controle na mesma corrida — e só então
remede. Célula que virou TEM por edição do medidor sem prova de efeito é o
erro que a §1 do `COMPARATIVO.md` já pagou cinco vezes.

---

## Os limites nomeados — rodada de 09/09/2026 (pedidos 234 a 238)

Ordem do dono, 09/09/2026: *«Next.»* A rodada anterior fechou 17 das 19 e
deixou seis pedidos nomeados. O 239 fica com o dono (isolamento acima de
`READ COMMITTED` e TLS). Os outros cinco são esta rodada, e os contratos
abaixo são o que as frentes compartilham antes de existir código.

### O modelo tipado de cada lado do `consultar` (pedido 237)

Os dois defeitos do 237 têm uma raiz só, medida em 09/09 lendo
`consultar.rs::valor_de_json` e `servidor.rs::linhas_do_sub_pedido`: o
`consultar` compõe **linhas JSON sem tipo**. Um `Decimal` viaja como texto
(`"20.00"`) e por isso `preco > media` compara texto com texto; e uma direita
vazia não tem de onde tirar os **nomes** das colunas, porque o protocolo
devolve linhas e não esquema. Os dois se fecham com uma peça:

- `linhas_do_sub_pedido` devolve, junto das linhas, o **modelo** do lado:
  `Vec<(nome, ColumnType)>`. Sai do esquema da tabela para `varrer`/`buscar`;
  de `por` + agregados para `agrupar` (`contagem`/`distintos` → `UInt8`;
  `soma`/`media`/`minimo`/`maximo` → o tipo da coluna agregada, e a frente
  diz e prova o que a média de inteiro vira); e do próprio modelo de saída
  para um `consultar` aninhado.
- `prefixar` prefixa modelo e linhas juntos; `juntar` recebe o modelo da
  direita e a linha sem casamento sai com **todas** as colunas da direita
  nulas, **inclusive com a direita vazia**. A forma da linha é a mesma em
  toda linha da resposta — é o padrão que já pagou três vezes nesta casa
  (coluna que some quebra quem lê por posição).
- A avaliação de `expressao` converte cada célula **pelo tipo do modelo**
  (`expressao::coagir`, o mesmo do DEFAULT e da chave de índice): `Decimal`
  compara como número. Uma coluna `Str` que contém `"10"` **continua texto** —
  rótulo se estiliza, dado nunca; adivinhar número pelo conteúdo é mentira
  sobre o dado.
- O `escalar` entra no modelo com o tipo que o sub-pedido dele diz.
- A resposta do `consultar` passa a trazer `"colunas": [{"nome", "tipo"}]`,
  porque é por ela que um `consultar` aninhado e o cliente sabem a forma da
  linha sem esperar a primeira linha chegar.

Prova real dos dois: `LEFT JOIN` com direita vazia devolve a linha com as
colunas da direita presentes e nulas (falha com o defeito reposto: colunas
ausentes); `preco > media` com `preco` Decimal `9.50` e `media` `10.00` dá
**falso** (falha com o defeito reposto: `"9.50" > "10.00"` textual é verdadeiro).

### Junções `direito`, `completo` e `cruzado` (pedido 236)

Entram no motor, e não por troca de lados no tradutor: `A JOIN B RIGHT JOIN C`
trocado vira um `consultar` aninhado com prefixo duplo, e isso é exatamente o
tipo de forma de linha que o 237 proíbe.

- `direito`: toda linha da direita fica; a que não casou sai com as colunas
  da esquerda nulas.
- `completo`: `esquerdo` ∪ as linhas da direita que não casaram.
- `cruzado`: **sem** `em` (e recusa se `em` vier, nomeando); produto. O teto
  `recursos.max_linhas` é conferido **antes** de materializar — esquerda ×
  direita acima do teto recusa nomeando os dois tamanhos — porque
  materializar cem milhões para depois recusar é o estrago, não a proteção.
- Tradutor SQL: `RIGHT [OUTER] JOIN`, `FULL [OUTER] JOIN`, `CROSS JOIN` → 1:1.

### `existe` — semijunção por espalhamento (pedido 236)

`EXISTS` correlacionado é, no caso que todo mundo escreve, uma semijunção por
igualdade — e semijunção por espalhamento custa uma passada em cada lado, sem
rodar a subconsulta por linha. É a divergência desta casa: correlação **só por
igualdade**, e o resto recusa nomeando, porque cada sub-pedido é materializado
inteiro pelo portão de permissão, e rodá-lo N vezes seria N passagens pelo
portão.

```json
"existe": [{"de": {"op": "varrer", "tabela": "pedidos"}, "apelido": "x",
            "em": [{"esquerda": "c.id", "direita": "x.cliente_id"}],
            "nao": false}]
```

- Aplicado **depois** de `juntar` e `escalar` e **antes** de `expressao`.
  Mantém as linhas da esquerda que têm (ou, com `"nao": true`, que não têm)
  casamento; não acrescenta coluna nenhuma.
- `em` obrigatório, com ao menos um par; `apelido` do lado de dentro
  obrigatório quando houver ambiguidade, e nome sem prefixo resolve quando é
  único naquele lado — a mesma regra do `juntar`.
- Cada `de` roda por `executar_derivado` — o portão é um só — **e entra em
  `direito_coluna::tabelas_do_pedido`**: quando o portão passa a olhar um
  campo novo, procure quem não tem esse campo. Teste com tabela negada
  dentro de `existe`, e o do comportamento velho: `consultar` sem `existe`
  não muda.
- Tradutor SQL: `[NOT] EXISTS (SELECT … FROM t [AS x] WHERE c1 AND c2 …)`.
  Termo `fora.col = x.col` (em qualquer ordem) vira par em `em`; termo que
  só cita colunas de dentro vai para a `expressao` do sub-pedido; termo que
  cita coluna de fora sem ser igualdade, ou dentro de um `OR`, **recusa
  nomeando**. Qualificador de fora é qualquer apelido ou tabela que não seja
  o de dentro; nome sem qualificador é de dentro. `IN (SELECT …)`
  correlacionado e subconsulta escalar correlacionada **continuam recusando**.

### `COUNT(coluna)` e `ORDER BY` qualificado (pedido 236)

- `{"funcao": "contagem", "coluna": "c"}` conta valores **não nulos**; sem
  `coluna` conta linhas. O tradutor gera; `COUNT(DISTINCT c)` → `distintos`.
- `ORDER BY p.id` → `{"coluna": "p.id"}`; o `consultar` resolve como resolve
  a projeção (único sem prefixo passa, ambíguo recusa nomeando os dois).

### Direito por coluna que cita coluna inexistente (pedido 235)

- Na **carga** do cadastro (arranque e as três operações de cadastro), cada
  `colunas.<c>` é conferida contra o esquema da tabela **quando a tabela
  existe**: coluna inexistente **recusa** nomeando usuário, base, tabela e
  coluna — e lista as que existem, para o erro de digitação se achar.
- Tabela que ainda não existe: **aceita**, com aviso — é a mesma decisão da
  chave estrangeira declarada antes da tabela (ordem de modelagem legítima).
  O aviso sai no log e na lista de avisos que `Usuario::de_json` já carrega.
- O teste que mais importa é o do comportamento velho: cadastro sem
  `colunas`, e cadastro cujas colunas existem, **nada muda**.

### O teste 234 e o estado global do teto de núcleos

`paralelo::TETO` é global de processo, e `Config::ler` chama
`recursos.aplicar()` — **todo** teste que lê um `config.json` redefine o teto
enquanto `threads_e_cpu_viram_o_teto_do_paralelo` confere o dele. Conserto por
contrato: o teste de `config` prova o **cálculo** (`recursos.nucleos()`), o de
`phxsql-core::paralelo` prova `definir_teto`/`nucleos` no binário em que
ninguém mais mexe no global, e a linha que os compõe fica revisada. Prova real:
reproduzir a colisão de propósito antes (duas threads, uma redefinindo o teto
em laço) e mostrar a suíte inteira verde N vezes depois. **Nunca** reordenar
a suíte para escapar.

### `SQL_C_WCHAR` no driver ODBC (pedido 238)

- Parâmetro de entrada `SQL_C_WCHAR`: UTF-16 na ordem nativa (como o gestor
  de drivers entrega) → UTF-8 na borda do driver, nas duas formas de tamanho
  (`SQL_NTS` e bytes), com par substituto; substituto inválido recusa
  nomeando a posição. O mesmo conversor serve o sentido de volta
  (`SQLGetData`/`SQLBindCol` com `SQL_C_WCHAR`), porque cliente que só fala
  UTF-16 fala nos dois sentidos.
- Prova real: `São João` de ida e volta num teste de unidade que monta o
  buffer UTF-16 à mão, e o JSON de `parametros` carregando o UTF-8.
- Parâmetro de **saída** continua recusando nomeando: exige a op do servidor
  devolver um valor além da linha, que nenhuma operação tem — decisão de
  protocolo, e fica no `PENDENCIAS.md` como o que sobrou do 238.
