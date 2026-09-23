# C) lista de comandos SQL que são do mariadb excenciais que não tem no Phxsql

## Resposta curta
Mandei **27 comandos do MariaDB(R) ao motor vivo: 6 aceitos, 21 recusados** —
um a mais aceito do que a resposta anterior (07/09/2026, commit a56a165, que
media 5): **`INSERT … ON DUPLICATE KEY UPDATE` passou a funcionar** na rodada
de composição SQL (item 7 de `docs/propostas/comparativo-19.md`, fechado em
08-09/2026) e saiu da lista de faltas para a lista de aceitos. Dos 21 que ainda
recusam, **9 faltam e importam** — o bloco do driver (`SHOW TABLES`, `SHOW
DATABASES`, `SHOW CREATE TABLE`, `DESCRIBE`, `USE`), `REPLACE INTO`, `LIMIT`
com vírgula, `CHECK` e coluna gerada — e **12 faltam e não importam aqui**, com
o motivo. **9 equivalências pelo protocolo** provadas nesta mesma corrida.

## Exemplo exercitado

Corrida de **2026-09-22 23:43 UTC**, commit **f180e24**. **Fonte da lista:** a
Knowledge Base oficial, https://mariadb.com/kb/en/sql-statements/, mais as
páginas que o `docs/SPRINTS-MARIADB.md` já citava.

**Esta resposta substitui a de 07/09/2026** — a lei do `docs/pdf/LEIA-ME.md`:
*doc antigo é ponto de partida, nunca fonte da resposta*. A única mudança de
estado desde então é o `INSERT … ON DUPLICATE KEY UPDATE`; o resto da lista
está como estava, porque a rodada de composição SQL mirou `SELECT` (JOIN,
GROUP BY, subconsulta, CTE, janela) e não mexeu no vocabulário de sessão
(`SHOW`/`USE`/`SET`) nem em DDL (`ALTER … ADD CONSTRAINT`/`ADD COLUMN … AS`).

### O controle que muda a lista: cinco comandos do MariaDB(R) que FUNCIONAM

```
[ACEITO] CREATE TRIGGER tg BEFORE INSERT ON clientes FOR EACH ROW
           BEGIN SET NEW.cidade = 'Blumenau'; END
  {'gatilho': 'tg', 'tabela': 'clientes', 'quando': 'BEFORE', 'evento': 'INSERT', 'criado': True}

[ACEITO] DROP TRIGGER tg                  {'gatilho': 'tg', 'excluido': True}

[ACEITO] CREATE PROCEDURE p1() BEGIN DECLARE x INT DEFAULT 1; SET x = x + 1; END
  {'procedimento': 'p1', 'parametros': 0, 'criado': True}

[ACEITO] CALL p1()                        {'procedimento': 'p1', 'saida': {}}
[ACEITO] DROP PROCEDURE p1                {'procedimento': 'p1', 'excluido': True}
```

Isto está aqui por lei da casa: *o instrumento antes do veredito*. E o SEXTO,
que a resposta anterior ainda listava como recusa:

```
[ACEITO] INSERT INTO clientes (id,nome) VALUES (1,'X') ON DUPLICATE KEY UPDATE nome='X'
  {'op': 'inserir', 'afetadas': 1, 'rowid': 1,
   'notas': ['ON CONFLICT/ON DUPLICATE KEY ... UPDATE vira `inserir` com se_existir: "atualizar" -- o SET vai no campo "atualizar"']}
```

**As três listas abaixo se cruzam, e é de propósito.** Um comando recusado
pode ao mesmo tempo ter equivalente pelo protocolo — somar as três não dá o
total de recusas, e uma soma que fechasse esconderia justamente o que
interessa: quantos gaps têm saída hoje.

### (a) Falta, e IMPORTA para um cadastro comum — 9

| comando | a recusa REAL, colada |
|---|---|
| `SHOW TABLES` | `SQL, coluna 6: SHOW nesta camada lista TRIGGERS ou PROCEDURES; tabelas e colunas saem por sistabelas/siscolunas` |
| `SHOW DATABASES` | a mesma |
| `SHOW CREATE TABLE clientes` | a mesma |
| `DESCRIBE clientes` | `SQL, coluna 1: DESCRIBE nao e um comando desta camada` |
| `USE loja` | `SQL, coluna 1: USE nao e um comando desta camada` |
| `REPLACE INTO clientes (id, nome) VALUES (1, 'X')` | `SQL, coluna 1: REPLACE nao e um comando desta camada` |
| `SELECT * FROM clientes LIMIT 0, 2` (`LIMIT` com vírgula) | `SQL, coluna 31: sobrou "," depois do fim do comando; um comando por vez` |
| `ALTER TABLE clientes ADD CONSTRAINT c1 CHECK (saldo >= 0)` | `SQL, coluna 1: esperava SET depois do escopo tabela, veio "ADD"` |
| `ALTER TABLE clientes ADD COLUMN dobro DECIMAL(12,2) AS (saldo*2) PERSISTENT` (coluna gerada) | a mesma recusa de cima — o `ALTER TABLE` desta camada só fala com as diretivas (`docs/SQL.md` §2c) |

**Os cinco primeiros são um bloco, e é o bloco do driver.** `SHOW TABLES`,
`SHOW DATABASES`, `SHOW CREATE TABLE`, `DESCRIBE` e `USE` são o que um cliente
MySQL(R)/MariaDB(R) (e o DBeaver, e o Excel por ODBC) manda **antes** de o
usuário digitar qualquer coisa. Os quatro primeiros já têm resposta pronta no
protocolo — é tradução de poucas linhas cada. O `USE` é diferente: ele guarda
estado **na conexão**, como o `BULKINSERT` e a transação, e o desenho disso já
está escrito em `docs/SQL.md` §2.

O `LIMIT 0, 2` é a única diferença puramente sintática da lista: o motor
entende `LIMIT n OFFSET m` e não a forma com vírgula, que é a que todo código
MySQL(R) do mundo escreve.

`REPLACE INTO`, `CHECK` e coluna gerada importam porque um cadastro comum usa
os três todo dia: upsert sem alvo de conflito, validação de faixa de valor, e
coluna calculada que não precisa ser mantida à mão.

### (b) Falta, e NÃO importa aqui — 12, com o motivo

| comando | por que não é essencial neste motor |
|---|---|
| `SELECT * FROM clientes WHERE MATCH(nome) AGAINST('Adriano')` | `funcao MATCH nao existe`. Existe por operação própria (`procurar_texto`, sobre o `.fts`) — ver (c) |
| `SELECT nome FROM clientes EXCEPT SELECT nome FROM clientes` | `sobrou "SELECT" depois do fim do comando`. O `unir` já tem a máquina de comparar linhas; `EXCEPT` é a mesma máquina ao contrário, sem candidato de uso urgente |
| `SELECT nome FROM clientes INTERSECT SELECT nome FROM clientes` | idem, espelho do de cima |
| `CREATE EVENT e1 ON SCHEDULE EVERY 1 DAY DO SELECT 1` | `CREATE nesta camada cria TRIGGER ou PROCEDURE`. Há agendador aqui, com outro nome: `job_salvar`/`job_rodar`, com aviso por e-mail. Comando novo seria segunda porta para a mesma coisa |
| `CREATE ROLE gerente` | `CREATE nesta camada cria TRIGGER ou PROCEDURE`. O direito por usuário e **por tabela** já existe (pedido 124); papel é conveniência de administração |
| `CREATE SEQUENCE s1 START WITH 100` | idem recusa. Sequência existe **por tabela**, automática (`{"op":"sequencias"}`) — objeto autônomo é o item 15 de `docs/SPRINTS.md` |
| `SELECT * FROM clientes FOR SYSTEM_TIME AS OF NOW()` (versionamento de sistema) | `sobrou "SYSTEM_TIME" …`. Item 22 da lista de sprints; a premissa que pode matá-lo é do dono: a imagem da linha nasce **desligada**, e ligá-la custa ~10% da vazão e 5× o diário |
| `CREATE FUNCTION f1() RETURNS INT RETURN 1` | recusa **nomeando o motivo**: `CREATE FUNCTION nao existe nesta camada — so TRIGGER e PROCEDURE. Funcao devolveria valor dentro de expressao SQL, e a camada SELECT nao avalia expressao [na projeção]` |
| `ANALYZE SELECT * FROM clientes` (o EXPLAIN que executa) | `ANALYZE nao e um comando desta camada`. As `notas` já dizem o índice escolhido e que não há planejador |
| `SET autocommit = 0` | `SET nao e um comando desta camada`. O tradutor não guarda estado de sessão; `BEGIN`/`COMMIT` explícitos fazem o mesmo papel |
| `LOAD DATA INFILE '/tmp/x.csv' INTO TABLE clientes` | `LOAD nao e um comando desta camada`. A carga existe por operação (`importar_conferir` + `inserir_lote` + `BULKINSERT`), e ler arquivo do disco do servidor a pedido do cliente é superfície de ataque que este motor não quer |
| `` SELECT `nome` FROM `clientes` `` (backtick) | `caractere '\`' nao faz parte da linguagem`. O aspeamento aqui é o do padrão, `"nome"` |

### (c) Existe, com outro nome ou outra forma — 9

| SQL do MariaDB(R) | o que faz a mesma coisa aqui | prova desta corrida |
|---|---|---|
| `SHOW TABLES` | `{"op":"tabelas"}` | `{'database': 'loja', 'tabelas': ['chamados', 'clientes', 'itens']}` |
| `SHOW DATABASES` | `{"op":"bancos"}` | `['loja']` |
| `DESCRIBE` / `SHOW COLUMNS` | `{"op":"esquema"}` e `{"op":"siscolunas"}` | esquema completo, 6 colunas |
| `MATCH(col) AGAINST('palavra')` | `{"op":"procurar_texto"}` sobre o `.fts` | `{'encontrados': 1, 'linhas': [{'corpo': 'a fenix renasce das cinzas', …}]}` |
| `CREATE SEQUENCE` / `NEXTVAL` | `{"op":"sequencias"}` e `{"op":"ajustar_sequencia"}` — **uma por tabela** | `{'sequencias': [{'tabela': 'clientes', 'coluna': 'id', 'proxima': 8, …}]}` |
| `CHECK TABLE` | `{"op":"verificar"}` | `{'registros': 7, 'slots': 7, 'indices': {'porId': 7, 'porNome': 7}, …}` |
| `OPTIMIZE TABLE` (a parte útil) | `{"op":"reindexar"}` | `{'porId': 7, 'porNome': 7}` |
| `CHECKSUM TABLE` | `{"op":"checksum"}` | `{'checksum': '1e1a28f552b21afb', 'linhas': 1, 'slots': 1, 'ms': 0}` |
| `LOCK TABLES … WRITE` para carga | `{"op":"bulkinsert","ligado":true}` | `{'reservada': True, 'expira_em_s': 1800, 'prazo_min': 30}` — e o `BULKINSERT` **não** é transação: não desfaz |

E o **`.fts` medido nesta corrida**, porque uma equivalência que não se prova
não vale: ele **dobra acento** e **não faz prefixo**, exatamente como o
`docs/FTS.md` promete.

```
[OK] procurar_texto  palavra="fênix"  ->  {'encontrados': 1, 'linhas': [{'corpo': 'a fenix renasce das cinzas', …}]}   # dobra acento
[OK] procurar_texto  palavra="fen"    ->  {'encontrados': 0, 'linhas': []}                                             # não é prefixo
```

### O que a fonte oficial lista como exclusivo do MariaDB(R), e o estado de cada um aqui

Da página de comparação oficial (https://mariadb.com/kb/en/mariadb-vs-mysql-features/) —
**não retestada nesta corrida**, mantida da rodada anterior porque nenhuma
mudança do período tocou estes pontos (nenhum é `SELECT`/composição):

| exclusivo do MariaDB(R) | aqui |
|---|---|
| `CHECK` constraint | **não existe** — ver (a) linha 8; item 16 da lista de sprints |
| colunas geradas (`PERSISTENT`) | **não existe** — ver (a) linha 9; item 17 |
| colunas `INVISIBLE` | **não existe**, e está **recusado com motivo**: torná-las invisíveis mudaria a resposta de quem já lê `rownum`/`softdeleted` hoje |
| `CREATE SEQUENCE` como objeto | **existe pela metade**: uma sequência por tabela, sem objeto próprio; item 15 |
| tabelas com versionamento de sistema | **não existe**; item 22 |
| window functions / CTE recursiva | **`ROW_NUMBER() OVER` existe desde 08-09/2026**; `RANK`/`DENSE_RANK`/`SUM() OVER` e `WITH RECURSIVE` continuam recusando nomeando (`docs/SQL.md` §3) — **item MUDOU de estado desde 07/09/2026** |
| papéis | **não existe** o papel; o direito por usuário e **por tabela** existe; item 14 |
| `EXCEPT` / `INTERSECT` | **não existe** — `sobrou "SELECT" depois do fim do comando`; item 6, e o `unir` já tem a máquina de comparar linhas |
| `ALTER TABLE` instantâneo | **não se aplica**: o truque do InnoDB depende de linha de largura variável, e o slot daqui é fixo. Recusado com motivo técnico, não por prioridade |
| motores próprios (ColumnStore, Spider, CONNECT) | **fora da regra de zero dependências** |

## O que NÃO existe, e é dispensa registrada

- **Não existe `ALTER TABLE` em SQL**, em nenhuma forma: `ALTER nao e um comando
  desta camada`. As três coisas que um cadastro faz com ele —
  acrescentar coluna, renomear tabela, declarar chave — existem como operação
  (`acrescentar_coluna`, `renomear_tabela`, `declarar_fk`), medidas na seção (c)
  da resposta B.
- **Não existe `CHECK` declarativo nem coluna gerada.** O lugar onde eles
  caberiam já existe e está identificado — o gatilho `BEFORE` roda com a trava
  na mão, entre a conversão e a gravação, e o avaliador exato (`i128` com
  escala, sem `f64`) já está escrito no `rotina.rs`. É tradução, e não motor.
- **`ON DUPLICATE KEY UPDATE` mudou de estado nesta rodada** e saiu da lista de
  faltas — é o único item cuja categorização não é herdada da resposta
  anterior.
- **Não subi um MariaDB(R) para comparar comportamento.** Esta resposta compara
  o **manual** dele com o **motor** daqui, e diz isso: onde eu afirmo o que o
  MariaDB(R) faz, a fonte é a página citada; onde eu afirmo o que o PhxSql faz,
  a fonte é a saída colada.
- **Não exercitei o cliente MySQL(R) do DbLink** (`dblink_consultar`), que fala
  o protocolo de fio deles. Ele existe e é outra frente; esta resposta é sobre
  o texto SQL que chega pela op `sql`.

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py mariadb
```
