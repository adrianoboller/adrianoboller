# B) lista de comandos SQL que são do postgresql excenciais que não tem no Phxsql

## Resposta curta
Mandei **49 comandos do PostgreSQL(R) ao motor vivo: 48 recusados, 1 aceito**. Classifiquei
os recusados por critério escrito: **17 faltam e importam** para um cadastro comum — e
treze deles são a mesma raiz, *o `WHERE` que filtra e o avaliador de expressão* — e **8
faltam e não importam aqui**, com o motivo. Contra isso, **26 equivalências provadas pelo
protocolo nesta mesma corrida** (`INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`, `JOIN`,
`UNION`, `information_schema`…): as listas se cruzam, porque um comando pode faltar na
linguagem e existir no motor. Um gap não tem caminho nenhum: **não existe `CREATE INDEX`
depois de a tabela nascer**, e isso é decisão registrada.

## Exemplo exercitado

Corrida de **2026-09-07 16:49 UTC**, commit **a56a165**, `phxsqld` de pé na porta 6110.
**Fonte da lista:** o índice oficial de comandos,
https://www.postgresql.org/docs/17/sql-commands.html, lido nesta rodada.

**As três listas abaixo se cruzam, e é de propósito.** Um `INSERT` recusado está ao
mesmo tempo em (a) — falta na linguagem — e em (c) — existe no motor com outro nome.
Somar as três não dá o total de recusas, e uma soma que fechasse esconderia
justamente o que interessa: quantos gaps têm saída hoje.

### (a) Falta, e IMPORTA para um cadastro comum — 17

O critério é escrito: *o que uma tela de cadastro (incluir, alterar, listar,
procurar, apagar) e o driver que a serve usam todo dia.* Não entra o que só
aparece em relatório de BI.

| # | comando | a recusa REAL do motor, colada |
|--:|---|---|
| 1 | `INSERT INTO clientes (nome) VALUES ('Zeca')` | `SQL, coluna 1: INSERT ainda nao existe nesta camada -- so SELECT. A operacao equivalente ja funciona pelo protocolo` |
| 2 | `UPDATE clientes SET cidade = 'Itajai' WHERE id = 1` | `SQL, coluna 1: UPDATE ainda nao existe nesta camada -- so SELECT. …` |
| 3 | `DELETE FROM clientes WHERE id = 1` | `SQL, coluna 1: DELETE ainda nao existe nesta camada -- so SELECT. …` |
| 4 | `… WHERE id = 1 AND cidade = 'Blumenau'` | `SQL, coluna 41: o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, e nao ha planejador que decida por qual indice comecar` |
| 5 | `… WHERE id = 1 OR id = 2` | a mesma recusa da linha 4 |
| 6 | `… WHERE id IN (1, 2)` | `SQL, coluna 33: IN e uma lista de buscas; o motor faz cada uma, mas quem junta os resultados ainda nao existe` |
| 7 | `… WHERE id BETWEEN 1 AND 2` | `SQL, coluna 33: BETWEEN e faixa de indice, e a faixa ainda nao esta exposta no protocolo` |
| 8 | `… WHERE nome LIKE 'A%'` | `SQL, coluna 35: LIKE precisaria varrer comparando texto linha a linha; o varrer sabe fazer isso (\`onde\` com \`contem\`), mas so dentro da pagina que examina …` |
| 9 | `… WHERE cidade IS NULL` | `SQL, coluna 37: IS NULL nao tem filtro embaixo: nulo se ve lendo a linha` |
| 10 | `… WHERE cidade = 'Blumenau'` (coluna sem índice) | `WHERE cidade = ... exige um indice de uma coluna sobre cidade. Nao existe. O \`varrer\` filtra, mas dentro da pagina que ele EXAMINA …` |
| 11 | `SELECT cidade, COUNT(*) … GROUP BY cidade` | `SQL, coluna 21: esperava FROM, e veio "("` — e o `GROUP BY` sozinho: `GROUP BY geral nao existe embaixo. A tabulacao cruzada e a operacao pivotar, que e um caso e nao o geral` |
| 12 | `SELECT SUM(saldo) FROM clientes` | `SQL, coluna 8: SUM() nao tem quem calcule embaixo. So COUNT(*) passa, porque a contagem sai do cabecalho da tabela em O(1)` |
| 13 | `SELECT DISTINCT cidade FROM clientes` | `SQL, coluna 17: DISTINCT nao tem substrato: nenhuma operacao do protocolo elimina repetido numa varredura` |
| 14 | `SELECT saldo * 1.1 FROM clientes` | `SQL, coluna 14: esperava FROM, e veio "*"` |
| 15 | `SELECT upper(nome) FROM clientes` | `SQL, coluna 13: esperava FROM, e veio "("` |
| 16 | `SELECT 1` (o *ping* de todo driver) | `SQL, coluna 8: esperava nome de coluna, e veio "1"` |
| 17 | `SELECT * FROM clientes WHERE id = $1` (parâmetro) | `SQL, coluna 35: caractere '$' nao faz parte da linguagem` |

**As linhas 4 a 15 são uma coisa só, e isso é medição e não opinião:** todas
esperam o mesmo par — *o `WHERE` que filtra de verdade* e *o avaliador de
expressão*. É o item 10 da lista de `docs/SPRINTS.md`, e ele já nomeia cinco
dependentes. A linha 16 e a 17 são de outra natureza: são o que um **driver**
manda antes de qualquer consulta do usuário.

E um gap que não é de linguagem, é de motor: **`CREATE INDEX` não tem
equivalente nenhum.**

```
[RECUSADO] CREATE INDEX porCidade ON clientes (cidade)
  SQL, coluna 8: CREATE nesta camada cria TRIGGER ou PROCEDURE.
                 Tabela se cria pela operacao criar_tabela do protocolo
```

Não há op `criar_indice` no catálogo das 123 operações. O índice se declara na
criação da tabela (`criar_tabela`, campo `indices`) e ponto — é decisão
registrada em `docs/PARECER-175-INDICE-NA-DECLARACAO.md`, não esquecimento. Mas
ela custa isto: **quem descobre no mês três que precisa procurar por `cidade`
tem de recriar a tabela**, e a linha 10 desta tabela é exatamente esse dia.

### (b) Falta, e NÃO importa aqui — 8, com o motivo

| comando | por que não é essencial neste motor |
|---|---|
| `CREATE VIEW` / `MATERIALIZED VIEW` | uma view é um `SELECT` guardado, e o `SELECT` desta camada ainda não faz o que uma view útil pediria (junção, expressão, agregado). Guardar hoje seria guardar a limitação com outro nome |
| `WITH` (CTE) e `WITH RECURSIVE` | `WITH nao e um comando desta camada`. É construção de relatório, não de tela de cadastro — e senta em cima do `GROUP BY` que não existe |
| *window functions* (`ROW_NUMBER() OVER …`) | idem: é o que uma ferramenta de BI gera sozinha. Está na lista como item 24, **dependente** do item 10 |
| subconsulta (`WHERE id IN (SELECT …)`) | o aplicativo de cadastro resolve com duas chamadas, e a forma de uma chamada só depende do mesmo planejador |
| `VACUUM`, `ANALYZE`, `CLUSTER` | compactar renumeraria rowid, e **rowid é endereço** — a ordem de digitação é pétrea. `VACUUM` aqui não é «ainda não», é «não» |
| `COPY`, `PREPARE`, `LISTEN/NOTIFY`, `DO` | `COPY nao e um comando desta camada` etc. Carga e exportação já existem por operação própria; o resto é do dialeto, não do cadastro |
| `CREATE EXTENSION`, FDW, *tablespace*, *publication*, *operator class*, *collation*, *domain*, *cast*, *aggregate*, *rule*, *event trigger* | são a metade do índice do PostgreSQL(R) que existe porque ele é extensível. O PhxSql é um motor de arquivos separados no modelo HFSQL: não há a que estender |
| `COMMENT ON` | `COMMENT nao e um comando desta camada`. O comentário de coluna existe aqui como `caption`/`descricao` no esquema — é campo, não comando |

`CREATE ROLE` fica aqui **com ressalva**: o direito por usuário e **por tabela**
já existe (pedido 124), e o papel é conveniência de administração. Ele é o item
14 da lista de sprints, não um buraco no dia a dia.

### (c) Existe, com outro nome ou outra forma — 26

Estas rodaram de verdade nesta corrida, pelo protocolo, e a resposta está no
`resultados.json` da bancada. **Sem esta tabela a lista de cima mentiria por
omissão:** «o PhxSql não tem `INSERT`» é verdade sobre a *linguagem* e falso
sobre o *motor*.

| SQL do PostgreSQL(R) | o que faz a mesma coisa aqui | prova desta corrida |
|---|---|---|
| `INSERT` | `{"op":"inserir"}` | `{'rowid': 4, 'registros': 4}` |
| `INSERT` de muitas | `{"op":"inserir_lote"}` | `{'database': 'loja', 'tabela': 'clientes', 'formato': 'lista', 'recebidas': 2, 'gravadas': 2, 'recusadas': 0, …` |
| `UPDATE … WHERE pk` | `{"op":"atualizar","rowid":…}` | `{'rowid': 1, 'versao': 2}` |
| `DELETE` | `{"op":"excluir"}` | `{'rowid': 2, 'excluido': True, 'modo': 'suave', 'na_lixeira': False, 'reversivel': True}` |
| — (não há em SQL padrão) | `{"op":"restaurar"}` desfaz o excluir suave | `{'rowid': 2, 'restaurado': True}` |
| `SELECT * FROM t` | `{"op":"varrer"}` | `{'registros': 3, 'visiveis': 3, 'marcadas': 0, 'devolvidas': 2, 'examinadas': 2, 'modo': 'posicao', …` |
| `SELECT … WHERE chave = ?` | `{"op":"buscar","indice":…,"chave":[…]}` | `{'encontrados': 1, 'linhas': [{'rowid': 1, 'id': 1, 'descricao': 'cafe', 'softdeleted': False, 'rownum': 1}]}` |
| `SELECT … WHERE col = ?` **sem índice** | `varrer` com `onde` | `{'registros': 3, 'visiveis': 3, 'marcadas': 0, 'devolvidas': 2, 'examinadas': 3, 'modo': 'posicao', …` — filtra, dentro do que examina |
| `… WHERE col LIKE '%x%'` | `varrer` com `onde` / `contem` | `{'registros': 3, 'visiveis': 3, 'marcadas': 0, 'devolvidas': 1, 'examinadas': 3, 'modo': 'posicao', …` |
| `… WHERE a = ? AND b = ?` | `varrer` com **duas** condições em `onde` | `{'registros': 3, 'visiveis': 3, 'marcadas': 0, 'devolvidas': 1, 'examinadas': 3, 'modo': 'posicao', …` — **o AND existe embaixo**, e é o `SELECT` que não o alcança |
| `CREATE TABLE` | `{"op":"criar_tabela"}` | `{'database': 'loja', 'schema': None, 'tabela': 'fornecedores', 'colunas': 4, 'indices': 1, 'paginada': False}` |
| `ALTER TABLE ADD COLUMN` | `{"op":"acrescentar_coluna"}` | `{'database': 'loja', 'tabela': 'fornecedores', 'coluna': 'uf', 'posicao': 2, 'colunas': 5, 'slots_reescritos': 0, …` |
| `ALTER TABLE RENAME` | `{"op":"renomear_tabela"}` | `{'database': 'loja', 'origem': 'fornecedores', 'destino': 'fornecedores2', 'arquivos': 8}` |
| `DROP TABLE` | `{"op":"excluir_tabela","confirmar":…}` | lista os 8 arquivos apagados |
| `\l` / `SHOW DATABASES` | `{"op":"bancos"}` | `['loja']` |
| `\dt` | `{"op":"tabelas"}` | `{'database': 'loja', 'schemas': [], 'tabelas': ['chamados', 'clientes', 'itens']}` |
| `\d tabela` | `{"op":"esquema"}` | o esquema inteiro, com os 8 arquivos da tabela |
| `information_schema.tables` | `{"op":"sistabelas"}` | `{'database': 'loja', 'total': 3, 'tabelas': [{'tabela': 'chamados', 'schema': '', 'registros': 1, 'slots': 1, …` |
| `information_schema.columns` | `{"op":"siscolunas"}` | `{'database': 'loja', 'total': 6, 'colunas': [{'tabela': 'clientes', 'posicao': 1, …` |
| `JOIN` | `{"op":"juntar"}` | `{'tipo': 'interna', 'sql': 'INNER JOIN', 'a': 'clientes', 'b': 'itens', …` — sete formas |
| `UNION` / `UNION ALL` | `{"op":"unir","modo":"distinta"\|"tudo"}` | `{'modo': 'distinta', 'sql': 'UNION', 'tabelas': ['clientes', 'clientes'], …` |
| `GROUP BY` cruzado | `{"op":"pivotar"}` | `{'database': 'loja', 'tabela': 'clientes', 'agregador': 'soma', 'campos_linha': ['cidade'], 'campos_coluna': [], 'valor': 'saldo', …` |
| `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT` | **passam pelo SQL**, e também são op própria | `BEGIN` → `{'transaction_id': 1788799736955, 'transaction_state': 'ACTIVE', 'transaction_start_time': '2026-09-07 16:48:56,998', 'transaction_isolation': 'escrita serializavel por tabela, leitura confirmada e nao bloqueante, …` |
| `SELECT … FOR UPDATE` | a janela de conflito por `versao` do `atualizar` — trava otimista em vez de pessimista | `{'rowid': 1, 'versao': 2}` |
| `GRANT` / `REVOKE` | `{"op":"usuarios"}` + o direito por base e **por tabela** na configuração | `[]` (servidor sem cadastro nesta corrida) |
| `EXPLAIN` | o campo `notas` que toda resposta do `sql` traz | `['indice porNome escolhido pelo WHERE -- e nao ha planejador: se houvesse dois candidatos, o primeiro declarado venceria']` |

### E três coisas que a sonda topou sem procurar

**1. `WHERE id = 2` recusa quando a chave é `Sequence`, e passa quando é `Int8`.**
Esta é a forma mais comum de tabela de cadastro que existe.

```
[ERRO] SELECT * FROM clientes WHERE id = 2      (id é Sequence)
       [SP000018] tipo invalido: esperado numero da sequencia, recebido Texto("2")
[OK  ] SELECT * FROM itens    WHERE id = 1      (id é Int8)
       {'sql': 'SELECT * FROM itens WHERE id = 1', 'op': 'buscar', …
```

O motivo é conhecido e está escrito em `docs/SQL.md` §5: o tradutor guarda todo
literal numérico como **texto**, de propósito, e o motor foi **alargado** para
aceitar inteiro escrito como texto. **O alargamento alcançou `Int`, e não
alcançou `Sequence`** — que é o irmão, e é o tipo da chave primária de quase
toda tabela nascida pela tela. É o padrão que o `CLAUDE.md` já nomeia:
*conserto entra no caminho que o motivou, e o caminho IRMÃO fica.*

**2. `FROM schema.tabela` inexistente vaza o erro cru do sistema — e manda
repetir.**

```
[ERRO] SELECT * FROM filial.clientes
       [SP000010] erro de E/S: No such file or directory (os error 2)   … "repetir": true
[ERRO] SELECT * FROM naoexiste
       [SP000018] nao encontrado: nenhum volume de naoexiste.reg em …   … "repetir": false
```

Os dois erros são «a tabela não existe». O de baixo diz isso e diz que **não
adianta repetir**; o de cima manda o driver **tentar de novo** para sempre, e
não nomeia nada. E o caminho de cima é o que um driver ODBC/DBeaver percorre
primeiro, porque é ele que pergunta por `information_schema.tables`.

**3. O catálogo documenta valores que o motor recusa.**

```
[ERRO] {"op":"unir", …, "modo":"distinto"}     (o valor que o catálogo descreve)
       união desconhecida: "distinto" (use distinta ou tudo)
[ERRO] {"op":"pivotar","chave":"cidade","valor":"saldo","agregador":"somar"}
       (o EXEMPLO do próprio catálogo)
       agregador desconhecido: "somar" (use soma, media, contagem, minimo, maximo ou distintos)
```

O `pivotar` é o pior dos dois: o catálogo documenta os parâmetros `chave` e
`coluna`, e o motor quer `linhas` e `colunas`. **O exemplo colável do catálogo
não roda.** É a família do «configuração que não é lida mente», do outro lado:
aqui é documentação que descreve um motor que não existe.

## O que NÃO existe, e é dispensa registrada

- **Não há executor de expressão nem planejador de consulta**, e é isso, e não
  uma lista de verbos, que explica 13 das 17 faltas que importam. Enquanto não
  houver, `AND`, `IN`, `LIKE`, `BETWEEN`, `IS NULL`, `DISTINCT`, `GROUP BY`,
  `SUM`, `CASE` e `upper()` recusam **dizendo o nome da cláusula**, que é a
  decisão certa: aceitar a sintaxe e responder sobre a primeira página seria a
  resposta errada calada.
- **Não há `CREATE INDEX` fora da criação da tabela** — decisão registrada, não
  falta.
- **Não há `VACUUM`/`CLUSTER`/compactação**, e não haverá enquanto rowid for
  endereço.
- **Não medi o protocolo de fio do PostgreSQL(R) nesta rodada.** Ele existe
  (`crates/phxsql-server/src/pg/`, escrito à mão com SCRAM-SHA-256), e toda esta
  resposta é sobre o texto SQL que chega pela op `sql`, não sobre o que passa
  pelo fio. Quem quiser a lista pelo lado do fio precisa de outra sonda.
- **Não classifiquei o índice inteiro do PostgreSQL(R).** Ele lista ~180
  comandos; mandei 49 ao motor. Os que não mandei estão na categoria (b) por
  família (extensões, FDW, tablespaces, publications), e isso está dito em vez de
  escondido.

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py postgresql   # só este motor
python3 bancada/gaps-sql/sondar.py              # os cinco, e as equivalências
```

A sonda sobe um `phxsqld` na porta 6110, cria a base, roda o **controle
positivo** (nove comandos que têm de passar; se um falhar ela para, porque zero
sem controle não vale nada), manda cada candidato e derruba o servidor no fim.
Grava `bancada/gaps-sql/resultados.json`.
