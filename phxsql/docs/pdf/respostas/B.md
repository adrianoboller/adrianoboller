# B) lista de comandos SQL que são do postgresql excenciais que não tem no Phxsql

## Resposta curta
Mandei **49 comandos do PostgreSQL(R) ao motor vivo: 19 aceitos, 30 recusados** —
o inverso do que esta resposta dizia: até esta rodada ela media **1 aceito**, de
uma corrida de 07/09/2026 que ficou parada enquanto 429 commits mudavam o motor
por baixo dela. `INSERT`, `UPDATE`, `DELETE`, `JOIN`, `GROUP BY`, subconsulta,
`WITH`, `ROW_NUMBER() OVER`, `CREATE VIEW` e o upsert (`ON CONFLICT`) — que a
resposta antiga listava como recusados por falta de "avaliador de expressão e
planejador" — hoje são **ACEITO**, medidos nesta mesma corrida. Dos **30** que
ainda recusam: **15 faltam e importam** (DDL que um driver/DBeaver usa, três
formas de expressão que só funcionam no `WHERE` e ainda não na projeção, `HAVING`
com função de agregação, `DISTINCT`/`UNION` — ambos com frente aberta nesta
rodada, ver nota — e o parâmetro posicional `$1`) e **15 faltam e não importam
aqui**, com o motivo. **40 equivalências pelo protocolo** provadas nesta mesma
corrida. `CREATE INDEX` continua sem caminho nenhum, decisão registrada.

## Exemplo exercitado

Corrida de **2026-09-22 23:43 UTC**, commit **f180e24**, `phxsqld` de pé na
porta 6119. **Fonte da lista:** o índice oficial de comandos,
https://www.postgresql.org/docs/17/sql-commands.html.

**Esta resposta substitui INTEIRA a de 07/09/2026 (commit a56a165), e não a
completa** — é a mesma lei do `docs/pdf/LEIA-ME.md`: *doc antigo é ponto de
partida, nunca fonte da resposta*. Entre as duas corridas, a rodada de
composição SQL (pedidos 236, 244, 245, entre 08 e 16/09/2026) fechou `JOIN`
(cinco formas), `GROUP BY`/agregados, `WITH` de uma CTE, subconsulta no `FROM`,
`IN (SELECT …)`, `[NOT] EXISTS` correlacionado por igualdade, escalar não
correlacionada, `ROW_NUMBER() OVER`, `CREATE`/`DROP VIEW` e o upsert; e a rodada
de 08/09/2026 fechou `INSERT`/`UPDATE`/`DELETE` por chave e por faixa. Nenhuma
dessas dez formas existia na corrida anterior — a resposta de então dizia,
para as três de escrita, a mesma frase colada nas três: *"INSERT/UPDATE/DELETE
ainda nao existe nesta camada -- so SELECT"*.

**Aviso sobre DISTINCT e UNION.** Há frente viva mexendo nos dois nesta mesma
rodada em que esta resposta foi escrita. O que segue é o que a corrida acima
mediu **neste instante**: `DISTINCT` recusa com *"nao tem substrato: nenhuma
operacao do protocolo elimina repetido numa varredura"*, e `SELECT ... UNION
SELECT ...` recusa com *"sobrou UNION depois do fim do comando"* (a `op unir`
do protocolo, que une TABELAS INTEIRAS nomeadas, é outra coisa — ver `docs/SQL.md`
§1, nota). Se a frente viva fechar um dos dois antes da próxima corrida, é
`python3 bancada/gaps-sql/sondar.py postgresql` que dirá — não esta prosa.

**As três listas abaixo se cruzam, e é de propósito.** Um comando recusado como
texto SQL pode ao mesmo tempo ter equivalente pelo protocolo — somar as três
não dá o total de recusas, e uma soma que fechasse esconderia justamente o que
interessa: quantos gaps têm saída hoje.

### (a) Falta, e IMPORTA para um cadastro comum ou para o driver — 15

O critério: *o que uma tela de cadastro (incluir, alterar, listar, procurar,
apagar) ou o driver/DBeaver que a serve usam todo dia.* DDL entra aqui porque é
exatamente o que o editor de esquema do DBeaver manda — não o que o cadastro em
si executa em produção.

| # | comando | a recusa REAL do motor, colada |
|--:|---|---|
| 1 | `SELECT * FROM clientes WHERE cidade = 'Blumenau'` (sem índice) | `WHERE cidade = ... exige um indice de uma coluna sobre cidade. Nao existe. O varrer filtra, mas dentro da pagina que ele EXAMINA -- e um SELECT que respondesse sobre a primeira pagina teria a cara de ter respondido sobre a tabela. Ha indice de coluna unica sobre: id, nome` |
| 2 | `SELECT cidade FROM clientes GROUP BY cidade HAVING COUNT(*) > 1` | `expressao "COUNT ( * ) > 1": funcao COUNT nao existe (as que existem: UPPER, LOWER, TRIM, LENGTH/CHAR_LENGTH, ROUND, ABS, COALESCE/IFNULL e CONCAT)` — o `HAVING` enxerga os APELIDOS do `agrupar` (`contagem`, `total`…), não a chamada de função de novo |
| 3 | `SELECT DISTINCT cidade FROM clientes` *(frente viva)* | `SQL, coluna 17: DISTINCT nao tem substrato: nenhuma operacao do protocolo elimina repetido numa varredura` |
| 4 | `SELECT nome FROM clientes UNION SELECT nome FROM clientes` *(frente viva)* | `SQL, coluna 27: sobrou "UNION" depois do fim do comando; um comando por vez` |
| 5 | `SELECT CASE WHEN id = 1 THEN 'um' ELSE 'outro' END FROM clientes` | `SQL, coluna 13: esperava FROM, e veio "WHEN"` — o parser lê `CASE` como nome de coluna e desiste no `WHEN` |
| 6 | `SELECT saldo * 1.1 FROM clientes` (expressão na projeção) | `SQL, coluna 14: esperava FROM, e veio "*"` — a expressão do item 2 da §7 só vale no `WHERE`/`HAVING`/`ON`, não na lista de colunas |
| 7 | `SELECT upper(nome) FROM clientes` (função escalar na projeção) | `SQL, coluna 13: esperava FROM, e veio "("` — mesma raiz da linha 6 |
| 8 | `SELECT * FROM clientes WHERE id = $1` (parâmetro posicional) | `SQL, coluna 35: caractere '$' nao faz parte da linguagem` — o `?` desta casa já resolve o mesmo papel (`docs/SQL.md` §7.1), mas o driver de fio do PostgreSQL(R) manda `$1`/`$2` |
| 9 | `SELECT 1` (o *ping* de todo driver) | `SQL, coluna 8: esperava nome de coluna, e veio "1"` |
| 10 | `SELECT * FROM clientes WHERE id = 1 FOR UPDATE` | `expressao "id = 1 FOR UPDATE": sobrou FOR depois do fim da expressao` — a trava pessimista não existe; a otimista por `versao` existe (ver (c)) |
| 11 | `CREATE TABLE fornecedores (id integer, nome text)` | `SQL, coluna 8: CREATE nesta camada cria TRIGGER ou PROCEDURE. Tabela se cria pela operacao criar_tabela do protocolo` |
| 12 | `DROP TABLE clientes` | `SQL, coluna 6: DROP TABLE e a operacao excluir_tabela do protocolo, que exige repetir o nome no campo "confirmar"` |
| 13 | `ALTER TABLE clientes ADD COLUMN uf char(2)` | `SQL, coluna 1: esperava SET depois do escopo tabela, veio "ADD"` — o `ALTER TABLE` desta camada só fala com as diretivas (`docs/SQL.md` §2c), não com o esquema |
| 14 | `CREATE INDEX porCidade ON clientes (cidade)` | `SQL, coluna 8: CREATE nesta camada cria TRIGGER ou PROCEDURE. …` — **gap sem caminho nenhum**, ver abaixo |
| 15 | `DROP INDEX porNome` | `SQL, coluna 6: DROP nesta camada e de TRIGGER ou PROCEDURE` — não há op `excluir_indice` nenhuma; consequência de o índice só se declarar na criação da tabela |

E um gap que não é de linguagem, é de motor — **`CREATE INDEX` não tem
equivalente nenhum**, hoje como em 07/09/2026:

```
[RECUSADO] CREATE INDEX porCidade ON clientes (cidade)
  SQL, coluna 8: CREATE nesta camada cria TRIGGER ou PROCEDURE.
                 Tabela se cria pela operacao criar_tabela do protocolo
```

Não há op `criar_indice` no catálogo. O índice se declara na criação da tabela
(`criar_tabela`, campo `indices`) — decisão registrada em
`docs/PARECER-175-INDICE-NA-DECLARACAO.md`, não esquecimento. O preço: quem
descobre no mês três que precisa de índice em `cidade` tem de recriar a tabela,
e a linha 1 desta lista é exatamente esse dia.

### (b) Falta, e NÃO importa aqui — 15, com o motivo

| comando | por que não é essencial neste motor |
|---|---|
| `TRUNCATE TABLE clientes` | `TRUNCATE nao e um comando desta camada`. Esvaziar uma tabela de produção sem WHERE não é operação de cadastro; existe `excluir_tabela` + `criar_tabela` para quem precisa mesmo |
| `GRANT SELECT ON clientes TO leitor` | `GRANT nao e um comando desta camada`. O direito por usuário e **por tabela** já existe na configuração (`{"op":"usuarios"}`) — ver (c) |
| `REVOKE SELECT ON clientes FROM leitor` | idem, espelho do de cima |
| `CREATE ROLE leitor` | `CREATE nesta camada cria TRIGGER ou PROCEDURE`. Não há papel (role); o direito é por usuário e por tabela, e cobre o caso de uso comum |
| `EXPLAIN SELECT * FROM clientes` | `EXPLAIN nao e um comando desta camada`. O campo `notas` que toda resposta do `sql` traz já diz o índice escolhido e que não há planejador — é o mesmo conteúdo, sem comando novo |
| `INSERT INTO clientes (nome) VALUES ('Zeca') RETURNING id` | `sobrou "RETURNING" depois do fim do comando`. A resposta de um `INSERT` traduzido **já** devolve `rowid` no envelope — `RETURNING id` seria sintaxe para algo que o protocolo já entrega sem pedir |
| `SET TRANSACTION ISOLATION LEVEL SERIALIZABLE` | recusa nomeando a alternativa: o nível se pede na ABERTURA (`BEGIN ISOLATION LEVEL REPEATABLE READ`), não por `SET` solto — o tradutor não guarda estado de sessão para um `SET` valer no próximo comando |
| `COPY clientes FROM '/tmp/x.csv' CSV` | `COPY nao e um comando desta camada`. Carga em massa já existe por operação (`importar_conferir` + `inserir_lote` + `BULKINSERT`) — ver (c); ler arquivo do disco do servidor a pedido do cliente é superfície de ataque que este motor não quer |
| `COMMENT ON TABLE clientes IS 'cadastro'` | `COMMENT nao e um comando desta camada`. O comentário de coluna já existe como `caption`/`descricao` no esquema — é campo, não comando |
| `CREATE SEQUENCE s1` | `CREATE nesta camada cria TRIGGER ou PROCEDURE`. Sequência existe **por tabela**, automática (`{"op":"sequencias"}`) — objeto de sequência autônomo é o item 15 de `docs/SPRINTS.md` |
| `ANALYZE clientes` | `ANALYZE nao e um comando desta camada`. As `notas` da resposta já dizem o índice escolhido — não há estatística separada para atualizar |
| `VACUUM clientes` | `VACUUM nao e um comando desta camada`, e não haverá enquanto `rowid` for endereço: compactar renumeraria slot, e a ordem de digitação é pétrea |
| `PREPARE p1 AS SELECT * FROM clientes` | `PREPARE nao e um comando desta camada`. O `?` já resolve o parâmetro sem *round-trip* de preparo; um *statement* nomeado de servidor não muda o resultado, só a forma de pedir |
| `SET search_path TO public` | `SET nao e um comando desta camada`. O schema já se escolhe pelo endereço de três partes no `FROM` (`docs/SQL.md`, "Endereço de três partes") |
| `SHOW server_version` | `SHOW nesta camada lista TRIGGERS ou PROCEDURES; tabelas e colunas saem por sistabelas/siscolunas`. A versão do servidor sai pelo catálogo (`{"op":"catalogo"}`), não por uma variável de sessão |

### (c) Existe, com outro nome ou outra forma — 40 equivalências provadas nesta corrida

Rodaram de verdade nesta corrida, pelo protocolo. **Sem esta tabela a lista de
cima mentiria por omissão:** «o PhxSql não tem `INSERT`» é falso desde sempre
sobre o *motor* — e agora é falso também sobre a *linguagem*, para as dez
formas que a rodada de composição fechou.

| SQL do PostgreSQL(R) | o que faz a mesma coisa aqui | prova desta corrida |
|---|---|---|
| `SELECT * FROM t` | `{"op":"varrer"}` | `{'registros': 4, 'visiveis': 3, 'devolvidas': 2, 'examinadas': 2, …}` |
| `SELECT … WHERE chave = ?` | `{"op":"buscar"}` | `{'encontrados': 1, 'linhas': [{'rowid': 1, 'id': 1, 'descricao': 'cafe', …}]}` |
| `SELECT … WHERE col = ?` sem índice | `varrer` com `onde` | `{'registros': 4, 'devolvidas': 1, 'examinadas': 3, …}` |
| `… WHERE col LIKE '%x%'` | `varrer` com `onde`/`contem` | `{'registros': 4, 'devolvidas': 0, 'examinadas': 3, …}` |
| `INSERT` | `{"op":"inserir"}` | `{'rowid': 5, 'registros': 5}` |
| `INSERT` de muitas | `{"op":"inserir_lote"}` | `{'recebidas': 2, 'gravadas': 2, 'recusadas': 0, …}` |
| `UPDATE … WHERE pk` | `{"op":"atualizar"}` | `{'rowid': 1, 'versao': 6}` |
| `DELETE` | `{"op":"excluir"}` | `{'rowid': 2, 'excluido': True, 'modo': 'suave', 'reversivel': True}` |
| — (não há em SQL padrão) | `{"op":"restaurar"}` desfaz o excluir suave | `{'rowid': 2, 'restaurado': True}` |
| `CREATE TABLE` | `{"op":"criar_tabela"}` | `{'tabela': 'fornecedores', 'colunas': 4, 'indices': 1, …}` |
| `ALTER TABLE RENAME` | `{"op":"renomear_tabela"}` | `{'origem': 'fornecedores', 'destino': 'fornecedores2', 'arquivos': 8}` |
| `DROP TABLE` | `{"op":"excluir_tabela"}` | lista os 8 arquivos apagados |
| `\l` / `SHOW DATABASES` | `{"op":"bancos"}` | `['loja']` |
| `\dt` / `SHOW TABLES` | `{"op":"tabelas"}` | `{'tabelas': ['chamados', 'clientes', 'itens']}` |
| `\d tabela` / `DESCRIBE` | `{"op":"esquema"}` | o esquema inteiro, com os 8 arquivos da tabela |
| `information_schema.tables` | `{"op":"sistabelas"}` | `{'total': 3, 'tabelas': [{'tabela': 'chamados', …}]}` |
| `information_schema.columns` | `{"op":"siscolunas"}` | `{'total': 6, 'colunas': [{'tabela': 'clientes', …}]}` |
| `JOIN` | `{"op":"juntar"}` (a versão avulsa, sete formas) — e agora **também** pelo SQL, via `consultar` | `{'tipo': 'interna', 'sql': 'INNER JOIN', 'a': 'clientes', 'b': 'itens', …}` |
| `UNION ALL` | `{"op":"unir","modo":"tudo"}` | `{'modo': 'tudo', 'sql': 'UNION ALL', 'tabelas': ['clientes', 'clientes'], …}` |
| `UNION` | `{"op":"unir","modo":"distinta"}` | `{'modo': 'distinta', 'sql': 'UNION', 'tabelas': ['clientes', 'clientes'], …}` |
| `GROUP BY` cruzado | `{"op":"pivotar"}` | `{'agregador': 'soma', 'campos_linha': ['cidade'], 'rotulos_linha': [...], …}` |
| `MATCH … AGAINST` (FTS) | `{"op":"procurar_texto"}` | `{'encontrados': 1, 'linhas': [{'corpo': 'a fenix renasce das cinzas', …}]}` |
| `CREATE SEQUENCE` / `nextval` | `{"op":"sequencias"}` | `{'sequencias': [{'tabela': 'clientes', 'coluna': 'id', 'proxima': 8, …}]}` |
| `ALTER SEQUENCE RESTART` | `{"op":"ajustar_sequencia"}` | `{'antes': 8, 'proxima': 5000, …}` |
| `COPY TO` / `SELECT INTO OUTFILE` | `{"op":"exportar"}` | `{'formato': 'csv', 'linhas': 1, 'bytes': 50, …}` |
| `LOAD DATA INFILE` (conferência) | `{"op":"importar_conferir"}` | `{'linhas_lidas': 1, 'desconhecidas': [], 'faltando': [], …}` |
| `CHECK TABLE` | `{"op":"verificar"}` | `{'registros': 7, 'slots': 7, 'indices': {'porId': 7, 'porNome': 7}, …}` |
| `OPTIMIZE` / `REINDEX` | `{"op":"reindexar"}` | `{'porId': 7, 'porNome': 7}` |
| `CHECKSUM TABLE` | `{"op":"checksum"}` | `{'checksum': '1e1a28f552b21afb', 'linhas': 1, …}` |
| `SHOW PROCESSLIST` | `{"op":"sessoes"}` | `{'quantas': 1, 'executando': 1, …}` |
| `SHOW GRANTS` / `pg_roles` | `{"op":"usuarios"}` | `{'ok': True, 'resultado': [], …}` (servidor sem cadastro nesta corrida) |
| catálogo de `pivotar` (o que ele documenta) | `{"op":"catalogo","pedida":"pivotar"}` | `{'operacao': {'nome': 'pivotar', 'resumo': 'Tabulação cruzada: soma, conta ou tira a média…', 'permissao': 'ler', …}}` |
| `LOCK TABLES … WRITE` (carga, ligar) | `{"op":"bulkinsert","ligado":true}` | `{'reservada': True, 'expira_em_s': 1800, 'prazo_min': 30}` |
| `UNLOCK TABLES` (carga, desligar) | `{"op":"bulkinsert","ligado":false}` | `{'liberada': True, 'durou_ms': 0, 'sincronizada': True}` |
| `BEGIN` | **passa pelo SQL desde 08/09/2026**, e também é op própria | `{'transaction_id': …, 'transaction_state': 'ACTIVE', …}` |
| `SAVEPOINT sp1` | idem | `{'savepoint': 'sp1', 'linhas': 0, …}` |
| `ROLLBACK TO SAVEPOINT sp1` | idem | `{'savepoint': 'sp1', 'descartadas': 0, 'linhas': 0, …}` |
| `COMMIT` | idem | `{'transaction_state': 'COMMITTED', 'gravadas': 0, …}` |
| `AS OF` / histórico da linha | `{"op":"diario"}` | `{'total': 14, 'eventos': [{'operacao': 'inclusao', 'rowid': 6, …}]}` |
| — (não há em SQL padrão) | `{"op":"lixeira"}` | conteúdo da lixeira suave da tabela |

E duas leituras a mais que esta mesma corrida sustenta, sem serem candidatos
próprios da fase de equivalências — por isso ficam fora da conta de 40 acima:

| SQL do PostgreSQL(R) | o que faz a mesma coisa aqui | prova desta corrida |
|---|---|---|
| `SELECT … FOR UPDATE` | a janela de conflito por `versao` do `atualizar` — trava otimista, não pessimista | `{'rowid': 1, 'versao': 6}`, do `UPDATE … WHERE pk` acima |
| `EXPLAIN` | o campo `notas` que toda resposta do `sql` traz | `['indice porId escolhido pelo WHERE -- e nao ha planejador: …']`, da linha `WHERE sobre chave Sequence` do controle |

**`ALTER TABLE ADD COLUMN` → `{"op":"acrescentar_coluna"}` não pôde ser
reprovado nesta corrida por um motivo da SONDA, não do motor**: o candidato
fixo do script pede um `DEFAULT` que usa a coluna `"SC"`, que a tabela de teste
desta rodada não tem (`o padrao de uf usa a coluna "SC", que a tabela nao
tem`). A operação em si continua provada em `crates/phxsql-server/src/servidor.rs`
(testes de `acrescentar_coluna`) e nas corridas anteriores — é a fixture de
`bancada/gaps-sql/sondar.py` que precisa de ajuste, registrado aqui para não
morrer com a sessão.

### E o que a sonda topou sem procurar

```
[OK  ] o WHERE sobre chave Sequence recusa (o irmao Int8 passa)
[OK  ] o mesmo WHERE sobre chave Int8 passa — o controle do de cima
[ERRO] FROM schema.tabela inexistente vaza erro cru do SO, com repetir:true
[ERRO] FROM tabela inexistente (sem schema) recusa direito, com repetir:false
[OK  ] o varrer faz AND de duas condicoes — o substrato existe
[OK  ] e o SELECT recusa a MESMA pergunta (WHERE cidade = .. AND nome = ..)
[OK  ] o .fts dobra acento: fênix acha fenix
[OK  ] e nao faz prefixo: fen nao acha fenix
```

Os dois primeiros e os dois últimos são os mesmos achados desde 07/09/2026 —
`WHERE id = 2` ainda recusa quando `id` é `Sequence` e passa quando é `Int8`
(o alargamento do `docs/SQL.md` §5 cobriu `Int`, e não o irmão `Sequence`), e o
`.fts` continua dobrando acento e não fazendo prefixo, como `docs/FTS.md`
promete. **O que É novo:** o `varrer` sempre soube fazer `AND` de duas
condições em `onde` (op crua); hoje o `SELECT` traduzido faz a MESMA pergunta
por expressão — as linhas 55-72 do controle acima (`WHERE AND`/`OR`/`IN`/
`BETWEEN`/`LIKE`/`IS NULL`, todas ACEITO) são a prova de que a lacuna que
motivava este achado, em 07/09/2026, fechou.

E o que valia como achado #3 em 07/09/2026 — *"o catálogo documenta valores
que o motor recusa"* (`{"op":"unir","modo":"distinto"}` e
`{"op":"pivotar","agregador":"somar"}`) — **continua valendo, sem mudança**:

```
[ERRO] {"op":"unir", …, "modo":"distinto"}     (o valor que o catalogo descreve)
       uniao desconhecida: "distinto" (use distinta ou tudo)
[ERRO] {"op":"pivotar", …, "agregador":"somar"}     (o EXEMPLO do proprio catalogo)
       agregador desconhecido: "somar" (use soma, media, contagem, minimo, maximo ou distintos)
```

## O que NÃO existe, e é dispensa registrada

- **Não há planejador de consulta** (qual índice usar com dois candidatos, ou
  como decidir por `WHERE` sobre coluna sem índice) — isso explica as linhas 1
  e 8-10 da tabela (a), e é decisão dita em `docs/SQL.md` §3, não esquecimento.
- **Não há expressão nem função escalar na PROJEÇÃO do `SELECT`** — só no
  `WHERE`/`HAVING`/`ON` (item 2 da §7 de `docs/SQL.md`). `CASE`, `saldo * 1.1`
  e `upper(nome)` na lista de colunas recusam pelo mesmo motivo.
- **Não há `CREATE INDEX` fora da criação da tabela** — decisão registrada, não
  falta.
- **Não há `VACUUM`/`CLUSTER`/compactação**, e não haverá enquanto `rowid` for
  endereço.
- **Não medi o protocolo de fio do PostgreSQL(R) nesta rodada.** Ele existe
  (`crates/phxsql-server/src/pg/`, escrito à mão com SCRAM-SHA-256); esta
  resposta é sobre o texto SQL que chega pela op `sql`, não sobre o que passa
  pelo fio.
- **Não classifiquei o índice inteiro do PostgreSQL(R).** Ele lista ~180
  comandos; mandei 49 ao motor, os mesmos de sempre mais os que a rodada de
  composição tornou dignos de reteste.

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py postgresql   # só este motor
python3 bancada/gaps-sql/sondar.py              # os cinco, e as equivalências
```

A sonda sobe um `phxsqld` na porta 6110 (ou `PHX_GAPS_PORTA`), cria a base,
roda o **controle positivo** (nove comandos que têm de passar; se um falhar ela
para, porque zero sem controle não vale nada), manda cada candidato e derruba o
servidor no fim. Grava `bancada/gaps-sql/resultados.json`. **Cuidado com
concorrência**: esta corrida saiu na porta 6119, e não na 6110 padrão, porque
outra frente ocupava a porta ao rodar em paralelo — e o `resultados.json`
compartilhado pode ser sobrescrito por outra corrida simultânea; quando isso
importar, redirecione a saída do script para um arquivo próprio antes de
confiar no JSON.
