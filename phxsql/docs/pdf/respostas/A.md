# A) lista de comandos SQL funcionais no phxsql

> Corrida em 2026-09-23T00:00:17Z UTC · commit `b7490f1` · `target/release/phxsqld`
> · reproduzido por `python3 bancada/sql-exemplos/exercitar.py`

## Resposta curta

O phxsql tem **um tradutor SQL** (`crates/phxsql-sql/`), ligado ao motor pela
operação `{"op":"sql","database":"...","texto":"..."}`. **Esta resposta
substitui inteira a de 07/09/2026 (commit a56a165)** — a lei do
`docs/pdf/LEIA-ME.md`: *doc antigo é ponto de partida, nunca fonte da
resposta*. Naquela corrida, `AND`/`LIKE`/`IN`/`BETWEEN`/`IS NULL`/`GROUP BY`/
`SUM`/`INSERT`/`UPDATE`/`DELETE` recusavam; hoje **todos passam**. **Nesta
rodada, 56 comandos mandados, 38 `ok`, 18 `erro`** (script `exercitar.py`
computa e imprime o resumo — não é conta de mão). O que ainda recusa: `CREATE
TABLE`/`CREATE DATABASE`/`ALTER TABLE` por SQL (são operações nativas do
protocolo), `DISTINCT`, `ORDER BY DESC`, `WHERE`/`ORDER BY` em coluna sem
índice, e **um comparador sozinho que não seja `=`** (`<>`, `<`, `<=`, `>` ou
`>=` como única condição do `WHERE`) — o índice só desce por igualdade, e uma
condição só não vira "expressão" (que resolveria a faixa por varredura). Os
**dois achados** da rodada anterior (mensagem de `CREATE DATABASE` idêntica à
de `CREATE TABLE`, e erro de E/S cru na tabela de três partes) estão **os dois
corrigidos** — ver seção própria.

## Exemplo exercitado

Tabela usada em toda a bateria: `loja.clientes` (`id Int8` obrigatória e
primária, `nome Str(40)`, `cidade Str(40)`; índice único `porId` só em `id`),
com 5 linhas gravadas antes de começar.

### SELECT — aceito (15, oito herdados + sete que mudaram de estado)

```
[OK  ] SELECT * simples
       SQL: SELECT * FROM clientes
       -> {"sql":"SELECT * FROM clientes","op":"sql","notas":["sem ORDER BY a ordem e a de DIGITACAO...","sem LIMIT o servidor aplica o teto dele..."],"registros":5,"visiveis":5,...}

[OK  ] SELECT com colunas e apelido
       SQL: SELECT id, nome AS quem FROM clientes
       -> {"sql":"SELECT id, nome AS quem FROM clientes","op":"sql",...,"colunas":["id","quem"],...}

[OK  ] SELECT com AS no proprio COUNT
       SQL: SELECT COUNT(*) AS quantos FROM clientes
       -> {"sql":"SELECT COUNT(*) AS quantos FROM clientes","op":"sql",...,"contagem":5,...}

[OK  ] COUNT(*)
       SQL: SELECT COUNT(*) FROM clientes
       -> {"sql":"SELECT COUNT(*) FROM clientes","op":"sql",...,"contagem":5}

[OK  ] WHERE = na coluna indexada
       SQL: SELECT * FROM clientes WHERE id = 1
       -> {"sql":"SELECT * FROM clientes WHERE id = 1","op":"sql","notas":["indice porId escolhido pelo WHERE -- e nao ha planejador..."],"encontrados":1,"linhas":[{"rowid":1,"id":1,"nome":"Ana","cidade":"Blumenau","softdeleted":false,...}]}

[OK  ] ORDER BY asc
       SQL: SELECT * FROM clientes ORDER BY id
       -> {"sql":"SELECT * FROM clientes ORDER BY id","op":"sql","notas":["ORDER BY atendido pelo indice porId -- a ordem sai do .ndx, sem ordenar nada",...],"registros":5,...}

[OK  ] LIMIT
       SQL: SELECT * FROM clientes LIMIT 2
       -> {..."devolvidas":2,"examinadas":2,"modo":"posicao","salto":"passo",...}

[OK  ] LIMIT + OFFSET
       SQL: SELECT * FROM clientes LIMIT 2 OFFSET 1
       -> {..."devolvidas":2,"examinadas":2,"modo":"posicao","salto":"bisseccao",...}

[OK  ] AND                                                    -- MUDOU (era ERRO em 07/09)
       SQL: SELECT * FROM clientes WHERE id = 1 AND nome = 'Ana'
       -> {"op":"sql","notas":["varredura com expressao: nao ha indice para esta forma",...],...}

[OK  ] LIKE                                                   -- MUDOU
       SQL: SELECT * FROM clientes WHERE nome LIKE 'A%'
       -> {"op":"sql","notas":["varredura com expressao: nao ha indice para esta forma",...],...}

[OK  ] IN                                                     -- MUDOU
       SQL: SELECT * FROM clientes WHERE id IN (1, 2)
       -> {"op":"sql","notas":["varredura com expressao: nao ha indice para esta forma",...],...}

[OK  ] BETWEEN                                                -- MUDOU
       SQL: SELECT * FROM clientes WHERE id BETWEEN 1 AND 3
       -> {"op":"sql","notas":["varredura com expressao: nao ha indice para esta forma",...],...}

[OK  ] IS NULL                                                -- MUDOU
       SQL: SELECT * FROM clientes WHERE cidade IS NULL
       -> {"op":"sql","notas":["varredura com expressao: nao ha indice para esta forma",...],...}

[OK  ] GROUP BY                                               -- MUDOU
       SQL: SELECT nome, COUNT(*) FROM clientes GROUP BY nome
       -> {"op":"sql","notas":["GROUP BY vira `agrupar`: 1 coluna(s) de agrupamento -- sem indice nenhum..."],"colunas":[{"nome":"nome","tipo":"Str(40)"},{"nome":"contagem","tipo":"UInt8"}],...}

[OK  ] SUM (agregado que nao e COUNT)                         -- MUDOU
       SQL: SELECT SUM(id) FROM clientes
       -> {"op":"sql","notas":["GROUP BY vira `agrupar`: 0 coluna(s) de agrupamento..."],"colunas":[{"nome":"soma_id","tipo":"Real8"}],"grupos":1,...}
```

As sete linhas marcadas `-- MUDOU` fecharam entre 08 e 09/2026 (item 2 da §7
de `docs/SQL.md`: expressão no `WHERE`, e item 3: `GROUP BY`/agregados). Nenhuma
delas existia quando a resposta anterior desta pergunta foi escrita.

### SELECT — ainda recusado (11 comandos testados, em 7 formas — os cinco comparadores de faixa colam a mesma recusa e viram um bloco só)

```
[ERRO] FROM banco.tabela (3 partes: so 2 aqui)                -- comportamento CORRETO, nao gap
       SQL: SELECT * FROM loja.clientes
       -> [SP000018] nao encontrado: a tabela loja.clientes nao existe em loja

[ERRO] WHERE <>  / <  / <=  / >  / >=   (comparador sozinho, sem AND)
       SQL: SELECT * FROM clientes WHERE id <> 1   (e os quatro irmãos)
       -> WHERE id <> ... nao tem substrato: o indice desce ate uma chave IGUAL,
          e a faixa ainda nao esta exposta no protocolo. So `=` passa por aqui

[ERRO] ORDER BY desc
       SQL: SELECT * FROM clientes ORDER BY id DESC
       -> ORDER BY id DESC nao tem substrato: o indice porId guarda essa coluna
          em ASC, e a direcao esta gravada no .ndx -- nao ha quem inverta a
          lista depois. Quem precisa das duas direcoes declara dois indices na
          criacao da tabela, um deles com a marca `desc`

[ERRO] WHERE em coluna SEM indice (cidade)
       -> WHERE cidade = ... exige um indice de uma coluna sobre cidade. Nao
          existe. Ha indice de coluna unica sobre: id

[ERRO] ORDER BY em coluna SEM indice (cidade)
       -> ORDER BY cidade exige um indice de uma coluna sobre cidade. Nao
          existe, e nao ha ordenador

[ERRO] DISTINCT  (frente viva nesta mesma rodada — reconferir antes de citar)
       -> DISTINCT nao tem substrato: nenhuma operacao do protocolo elimina
          repetido numa varredura

[ERRO] JOIN — mas é a FIXTURE, não o motor
       SQL: SELECT * FROM clientes JOIN pedidos ON clientes.id = pedidos.cliente_id
       -> nao encontrado: a tabela pedidos nao existe em loja
```

**O `WHERE`/`ORDER BY` sem índice e o comparador sozinho continuam exatamente
como em 07/09/2026** — nenhuma das duas coisas foi tocada pela rodada de
composição, e a razão é a mesma: não há planejador de índice (`docs/SQL.md`
§3). **O `JOIN` desta bateria não prova nada, num sentido ou noutro**: o
roteiro de `exercitar.py` nunca criou a tabela `pedidos`, então o `SELECT …
JOIN pedidos …` recusa por "tabela não existe", não por falta de tradutor — é
falha da fixture do script, registrada aqui para não morrer com a sessão (a
resposta B, seção (c), já prova `JOIN` ACEITO com duas tabelas reais).

### Verbos de escrita e DDL

```
[OK  ] INSERT via SQL                                         -- MUDOU (era ERRO)
       SQL: INSERT INTO clientes (id, nome) VALUES (9, 'Zeca')
       -> {"op":"sql","notas":["INSERT vira `inserir`..."],"afetadas":1,"rowid":6,"registros":6,"ok":true}

[OK  ] UPDATE via SQL                                         -- MUDOU
       SQL: UPDATE clientes SET nome = 'X' WHERE id = 1
       -> {"op":"sql","notas":["UPDATE por chave e TRES passos..."],...}

[OK  ] DELETE via SQL                                         -- MUDOU
       SQL: DELETE FROM clientes WHERE id = 1
       -> {"op":"sql","notas":["DELETE por chave: ...E o excluir SUAVE..."],...}

[ERRO] CREATE TABLE via SQL
       SQL: CREATE TABLE x (id INT)
       -> SQL, coluna 8: CREATE nesta camada cria TRIGGER ou PROCEDURE. Tabela
          se cria pela operacao criar_tabela do protocolo

[ERRO] CREATE DATABASE via SQL                                -- MENSAGEM CORRIGIDA (era a de CREATE TABLE)
       SQL: CREATE DATABASE outra
       -> SQL, coluna 8: CREATE DATABASE nao existe nesta camada — nao ha DDL
          de database no SQL em texto. Use a operacao criar_database do
          protocolo

[ERRO] ALTER TABLE via SQL
       SQL: ALTER TABLE clientes ADD COLUMN x INT
       -> SQL, coluna 1: esperava SET depois do escopo tabela, veio "ADD"

[ERRO] BULKINSERT(true)
       SQL: BULKINSERT(true)
       -> BULKINSERT e comando de SESSAO, e nao de instrucao: ele reserva a
          tabela para carga e a reserva morre com a conexao. Hoje se pede pela
          porta de dados, com a operacao bulkinsert
```

**`INSERT`/`UPDATE`/`DELETE` por chave fecharam em 08/09/2026** (`dml.rs`),
item 2 do roteiro de `docs/SQL.md` §4. `CREATE TABLE`/`ALTER TABLE`/
`BULKINSERT(true)` continuam sendo operações nativas do protocolo, sem forma
de texto — decisão, não esquecimento (`docs/SQL.md` §4 item 3 para o
`BULKINSERT`).

### Transação — aceita, com a conexão fechando o que abre

```
[OK  ] BEGIN                 -> transaction_state: ACTIVE
[OK  ] COMMIT                -> transaction_state: COMMITTED
[OK  ] BEGIN TRANSACTION     -> ACTIVE
[OK  ] COMMIT WORK           -> COMMITTED
[ERRO] ROLLBACK (sem transacao aberta)
       -> esta conexao nao tem transacao aberta; comece com {"op":"begin"}
          (ou BEGIN / START TRANSACTION pelo SQL)
[OK  ] START TRANSACTION     -> ACTIVE
[OK  ] SAVEPOINT antes_do_lote
[OK  ] ROLLBACK TO SAVEPOINT antes_do_lote
[OK  ] RELEASE SAVEPOINT antes_do_lote
[OK  ] COMMIT (fecha a transacao desta bateria)
[OK  ] BEGIN TRANSACTION SCOPE (clientes) SCOPE MODE STRICT TIMEOUT 5s
       LOCK TIMEOUT 500ms STATEMENT TIMEOUT 2s LOCK MODE AUTO
       -> ACTIVE, transaction_isolation: "escrita serializavel por tabela,
          leitura confirmada e nao bloqueante, sem leitura repetivel"
[OK  ] COMMIT (fecha de novo)
[ERRO] COMMIT AND CHAIN
       -> SQL, coluna 1: sobrou "AND" depois do comando de commit
[ERRO] SET TRANSACTION ISOLATION LEVEL SERIALIZABLE
       -> SQL, coluna 1: SET TRANSACTION ISOLATION LEVEL SERIALIZABLE nao
          existe aqui: ... O nivel se pede na ABERTURA: BEGIN ISOLATION LEVEL
          REPEATABLE READ ..., e READ COMMITTED e o padrao. SERIALIZABLE nao
          existe: o motor nao promete o nome que nao provou
```

Nada mudou nesta seção desde 07/09/2026 — todas as três recusas aqui são
**corretas por desenho** (§2b de `docs/SQL.md`), não gaps.

### Gatilho e procedimento — a mesma op `sql`

```
[OK  ] CREATE TRIGGER normaliza BEFORE INSERT ON clientes FOR EACH ROW
       SET NEW.cidade = UPPER(TRIM(NEW.cidade))
       -> {"gatilho":"normaliza","tabela":"clientes","quando":"BEFORE","evento":"INSERT","criado":true}
[OK  ] SHOW TRIGGERS         -> lista com o corpo guardado verbatim
[OK  ] DROP TRIGGER normaliza
[OK  ] DROP TRIGGER IF EXISTS normaliza (ja excluido) -> "excluido":false, sem erro
[OK  ] CREATE PROCEDURE dobro(IN x INT, OUT y INT) SET y = x * 2
[OK  ] CALL dobro(21)        -> {"procedimento":"dobro","saida":{"y":42}}
[OK  ] SHOW PROCEDURES / SHOW PROCEDURE STATUS -- os dois devolvem a mesma lista
[OK  ] DROP PROCEDURE dobro
```

Nada mudou nesta seção desde 07/09/2026.

**Resumo desta corrida: 56 comandos, 38 `ok`, 18 `erro`** (impresso pelo
próprio `exercitar.py`, não contado à mão).

### Os dois achados da rodada anterior — os DOIS corrigidos

1. ~~`CREATE DATABASE outra` recebe a mensagem de `CREATE TABLE`~~ —
   **CORRIGIDO**. A recusa hoje nomeia o próprio comando: *"CREATE DATABASE
   nao existe nesta camada — nao ha DDL de database no SQL em texto. Use a
   operacao criar_database do protocolo"*.
2. ~~`SELECT * FROM loja.clientes` vaza um erro de E/S cru~~ — **CORRIGIDO**.
   A recusa hoje é *"nao encontrado: a tabela loja.clientes nao existe em
   loja"* — nomeada, sem caminho de disco, sem `[SP000010] erro de E/S`. É a
   mesma correção que `docs/SQL.md` §5 já registrava para o nome de tabela
   simples, agora medida também para o endereço de três partes.

### Um achado novo desta corrida: a fixture do `JOIN`, não o motor

O candidato de `JOIN` em `item_a` nunca criou a tabela `pedidos`, então a
recusa que ele produz (`nao encontrado: a tabela pedidos nao existe em loja`)
não prova nem desprova o tradutor de `JOIN`. Isso já era assim em 07/09/2026 e
não foi notado porque, naquela data, `JOIN` recusava por um motivo de
linguagem genuíno (`junção ainda nao passa por aqui`) que escondia o problema
da fixture — hoje que a linguagem aceita `JOIN`, a fixture furada aparece.
Prova de que `JOIN` funciona está na resposta B, seção (c).

## O que NÃO existe, e é dispensa registrada

- **`CREATE TABLE`, `CREATE DATABASE`, `ALTER TABLE`, `BULKINSERT(true)` por
  SQL não existem** — são operações nativas (`criar_tabela`, `criar_database`,
  `acrescentar_coluna`, `bulkinsert`), decisão documentada em `docs/SQL.md`.
- **Planejador de índice** — `WHERE`/`ORDER BY` em coluna sem índice, e um
  comparador de faixa (`<>`/`<`/`<=`/`>`/`>=`) **sozinho** (sem `AND`) ainda
  recusam. A composição de 08-09/2026 abriu a expressão para `AND`/`OR`/`IN`/
  `BETWEEN`/`LIKE`/`IS NULL`, mas uma única comparação continua classificada
  como forma "Simples" (`docs/SQL.md` §7 item 2), que só sabe `=`.
- **`DISTINCT`** — sem substrato nesta corrida; há frente viva mexendo nisso
  na mesma rodada em que esta resposta foi escrita. Reconfira com
  `bancada/gaps-sql/sondar.py postgresql` antes de citar esta linha depois de
  hoje.
- **Nível de isolamento acima de `READ COMMITTED` por `SET`** — `SET
  TRANSACTION ISOLATION LEVEL SERIALIZABLE` continua recusando; o caminho que
  funciona é `BEGIN ISOLATION LEVEL REPEATABLE READ` na abertura (`docs/SQL.md`
  §2b), fechado em 16/09/2026.
- **A fixture de `exercitar.py` não cria uma segunda tabela para `JOIN`** —
  falta do script, não do motor; registrado acima para a próxima rodada
  consertar antes de reusar este exemplo.

## Como se refaz

```bash
python3 bancada/sql-exemplos/exercitar.py
```

Sobe um `phxsqld` próprio na porta 6100 (ou `PHX_F1_PORTA`), roda as cinco
seções (A, E, F, G, H) e imprime a saída crua de cada comando, e o próprio
resumo de acertos/erros da seção A. A tabela `clientes` e os 56 comandos desta
seção nascem nas primeiras linhas da função `item_a`.
