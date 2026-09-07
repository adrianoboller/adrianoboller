# A) lista de comandos SQL funcionais no phxsql

> Corrida em 2026-09-07T16:25:26Z UTC · commit `a56a165` · `target/release/phxsqld`
> · reproduzido por `python3 bancada/sql-exemplos/exercitar.py`

## Resposta curta

O phxsql tem **um tradutor SQL** (`crates/phxsql-sql/`), ligado ao motor pela
operação `{"op":"sql","database":"...","texto":"..."}`. Ele cobre `SELECT`
simples (projeção com apelido, `COUNT(*)`, `WHERE` de **igualdade** numa
coluna **indexada**, `ORDER BY` **ascendente** numa coluna indexada, `LIMIT`/
`OFFSET`), o vocabulário de transação (`BEGIN`/`START TRANSACTION`/`COMMIT`/
`ROLLBACK`/`SAVEPOINT` e a forma longa com `SCOPE`/`TIMEOUT`/`LOCK MODE`) e o
vocabulário de rotina (`CREATE`/`DROP`/`SHOW TRIGGER(S)`, `CREATE`/`DROP`/
`SHOW PROCEDURE(S)`, `CALL`). **`INSERT`, `UPDATE`, `DELETE`, `CREATE TABLE`,
`CREATE DATABASE` e `ALTER TABLE` não existem nesta camada** — são operações
nativas do protocolo (`inserir`, `atualizar`, `excluir`, `criar_tabela`,
`criar_database`, `acrescentar_coluna`), e o SQL text as recusa **nomeando o
motivo**, não com "sintaxe inválida". Nesta rodada, **56 comandos mandados,
28 `ok`, 28 `erro`** — cada `erro` é um comando que `docs/SQL.md` ou
`docs/TRIGGERS.md` já documentam como recusado, exceto os dois achados da
seção final.

## Exemplo exercitado

Tabela usada em toda a bateria: `loja.clientes` (`id Int8` obrigatória e
primária, `nome Str(40)`, `cidade Str(40)`; índice único `porId` só em `id`),
com 5 linhas gravadas antes de começar.

### SELECT — aceito

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
```

### SELECT — recusado por falta de substrato (documentado em `docs/SQL.md`)

```
[ERRO] WHERE <>
       -> [SP000018] esquema invalido: WHERE id <> ... nao tem substrato: o indice
          desce ate uma chave IGUAL, e a faixa ainda nao esta exposta no protocolo.
          So `=` passa por aqui
[ERRO] WHERE < / <= / > / >=   -- mesma recusa, mesmo motivo
[ERRO] ORDER BY desc
       -> ORDER BY id DESC nao tem substrato: o indice porId guarda essa coluna
          em ASC, e a direcao esta gravada no .ndx -- nao ha quem inverta a lista
[ERRO] WHERE em coluna SEM indice (cidade)
       -> WHERE cidade = ... exige um indice de uma coluna sobre cidade. Nao
          existe. [...] Ha indice de coluna unica sobre: id
[ERRO] ORDER BY em coluna SEM indice (cidade)
       -> ORDER BY cidade exige um indice de uma coluna sobre cidade. Nao existe,
          e nao ha ordenador
[ERRO] DISTINCT
       -> DISTINCT nao tem substrato: nenhuma operacao do protocolo elimina
          repetido numa varredura
[ERRO] AND
       -> o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids,
          e nao ha planejador que decida por qual indice comecar
[ERRO] LIKE
       -> LIKE precisaria varrer comparando texto linha a linha [...] mas so
          dentro da pagina que examina
[ERRO] IN
       -> IN e uma lista de buscas; o motor faz cada uma, mas quem junta os
          resultados ainda nao existe
[ERRO] BETWEEN
       -> BETWEEN e faixa de indice, e a faixa ainda nao esta exposta no protocolo
[ERRO] IS NULL
       -> IS NULL nao tem filtro embaixo: nulo se ve lendo a linha
[ERRO] JOIN
       -> junção ainda nao passa por aqui. O motor ja junta -- e a operacao
          juntar, com sete formas -- mas a traducao do JOIN e outra rodada
[ERRO] SUM(id)
       -> SUM() nao tem quem calcule embaixo. So COUNT(*) passa, porque a
          contagem sai do cabecalho da tabela em O(1)
```

### Verbos de escrita e DDL — recusados nomeando a operação nativa

```
[ERRO] INSERT INTO clientes (id, nome) VALUES (9, 'Zeca')
       -> INSERT ainda nao existe nesta camada -- so SELECT. A operacao
          equivalente ja funciona pelo protocolo
[ERRO] UPDATE clientes SET nome = 'X' WHERE id = 1   -- mesma recusa, verbo UPDATE
[ERRO] DELETE FROM clientes WHERE id = 1             -- mesma recusa, verbo DELETE
[ERRO] CREATE TABLE x (id INT)
       -> CREATE nesta camada cria TRIGGER ou PROCEDURE. Tabela se cria pela
          operacao criar_tabela do protocolo
[ERRO] CREATE DATABASE outra          -- MESMA mensagem do CREATE TABLE (achado, ver abaixo)
[ERRO] ALTER TABLE clientes ADD COLUMN x INT
       -> SQL, coluna 1: ALTER nao e um comando desta camada
[ERRO] BULKINSERT(true)
       -> BULKINSERT e comando de SESSAO, e nao de instrucao: ele reserva a
          tabela para carga e a reserva morre com a conexao. Hoje se pede pela
          porta de dados, com a operacao bulkinsert
```

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
       -> SQL, coluna 1: SET nao e um comando desta camada
```

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

**Resumo desta corrida: 56 comandos, 28 `ok`, 28 `erro`.**

### Dois achados desta bateria (doc diverge do motor)

1. **`CREATE DATABASE outra` recebe a mensagem de `CREATE TABLE`.** A recusa
   diz *"CREATE nesta camada cria TRIGGER ou PROCEDURE. Tabela se cria pela
   operação `criar_tabela` do protocolo"* — o texto nomeia só a tabela, nunca
   o database, embora a recusa também valha para `CREATE DATABASE`. Quem lê
   essa frase depois de digitar `CREATE DATABASE` não encontra o próprio
   comando nela.
2. **`SELECT * FROM loja.clientes` (pensado como "banco.tabela") vaza um erro
   de E/S cru.** `docs/SQL.md` §"Endereço de três partes" diz que a forma de
   **duas** partes é `schema.tabela`, não `banco.tabela` — e é isso que
   aconteceu: o motor procurou a tabela `clientes` dentro do **schema**
   `loja` (que não existe) e devolveu `[SP000010] erro de E/S: No such file
   or directory (os error 2)`, em vez de uma recusa nomeando "schema não
   encontrado". É o padrão que o `CLAUDE.md` já cobra em outro caminho
   (`nenhum volume de clientes.reg em /tmp/…`): erro de sistema operacional
   vazando cru para quem só tinha a intenção de trocar de banco pelo `FROM`.

## O que NÃO existe, e é dispensa registrada

- **`INSERT`/`UPDATE`/`DELETE` por SQL não existem** — decisão documentada em
  `docs/SQL.md` §4: passo 2 do roteiro, ainda não feito. A operação
  equivalente já existe pelo protocolo (`inserir`/`atualizar`/`excluir`).
- **`CREATE TABLE`, `CREATE DATABASE`, `ALTER TABLE`, `FOREIGN KEY` por SQL
  não existem** — são operações nativas (`criar_tabela`, `criar_database`,
  `acrescentar_coluna`, `declarar_fk`), exercitadas no item G. A recusa do
  `CREATE` já nomeia o caminho certo; a do `ALTER` só diz "não é um comando
  desta camada" (sem apontar `acrescentar_coluna`) — recusa correta, mensagem
  mais pobre que a do `CREATE`.
- **Expressão em `WHERE` (`preco * 1.1 > 100`), planejador de dois índices,
  `GROUP BY` geral, subconsulta e CTE** — `docs/SQL.md` §3 já diz que não há
  substrato, e a sonda confirma: `AND`, `LIKE`, `IN`, `BETWEEN`, faixas
  (`<`/`<=`/`>`/`>=`/`<>`) e `JOIN` recusam nomeando a própria cláusula.
- **Nível de isolamento acima de `READ COMMITTED`** — `SET TRANSACTION
  ISOLATION LEVEL SERIALIZABLE` não é reconhecido nem pelo detector de
  transação nem pelo de rotina, e cai no catch-all genérico "SET não é um
  comando desta camada". `docs/SQL.md` §3 pede que essa recusa diga o nível
  real suportado — hoje ela não diz nada sobre isolamento, é o mesmo texto
  genérico de qualquer verbo desconhecido. Dispensa **não** registrada no
  doc: é o achado 2 lido de outro ângulo, e fica anotado aqui para quem for
  fechar o pedido.

## Como se refaz

```bash
python3 bancada/sql-exemplos/exercitar.py
```

Sobe um `phxsqld` próprio na porta 6100, roda as cinco seções (A, E, F, G, H)
e imprime a saída crua de cada comando. A tabela `clientes` e os 56 comandos
desta seção nascem nas primeiras linhas da função `item_a`.
