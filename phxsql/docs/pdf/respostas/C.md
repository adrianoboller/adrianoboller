# C) lista de comandos SQL que são do mariadb excenciais que não tem no Phxsql

## Resposta curta
Mandei **27 comandos do MariaDB(R) ao motor vivo: 22 recusados, 5 aceitos** — e os cinco
aceitos são os que mais gente diria que faltavam: `CREATE TRIGGER`, `DROP TRIGGER`,
`CREATE PROCEDURE`, `CALL`, `DROP PROCEDURE`. Dos recusados, **8 faltam e importam** para
um cadastro e **6 faltam e não importam aqui**, com o motivo; e há **9 equivalências
provadas pelo protocolo** nesta mesma corrida. O gap mais caro não é recurso: é que
**`SHOW TABLES`, `SHOW DATABASES`, `DESCRIBE` e `USE` — o que todo cliente manda ao
conectar — recusam**, embora três dos quatro já tenham resposta pronta no protocolo.

## Exemplo exercitado

Corrida de **2026-09-07 16:49 UTC**, commit **a56a165**. **Fonte da lista:** a
Knowledge Base oficial, https://mariadb.com/kb/en/sql-statements/, lida nesta rodada,
mais as páginas que o `docs/SPRINTS-MARIADB.md` já citava.

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

Isto está aqui por lei da casa: *o instrumento antes do veredito*. Uma lista de
gaps que não prova primeiro o que existe é uma lista que ninguém pode conferir.

**As três listas abaixo se cruzam, e é de propósito.** Um `INSERT` recusado está ao
mesmo tempo em (a) — falta na linguagem — e em (c) — existe no motor com outro nome.
Somar as três não dá o total de recusas, e uma soma que fechasse esconderia
justamente o que interessa: quantos gaps têm saída hoje.

### (a) Falta, e IMPORTA para um cadastro comum — 8

| comando | a recusa REAL, colada |
|---|---|
| `SHOW TABLES` | `SQL, coluna 6: SHOW nesta camada lista TRIGGERS ou PROCEDURES; tabelas e colunas saem por sistabelas/siscolunas` |
| `SHOW DATABASES` | a mesma |
| `SHOW CREATE TABLE clientes` | a mesma |
| `DESCRIBE clientes` | `SQL, coluna 1: DESCRIBE nao e um comando desta camada` |
| `USE loja` | `SQL, coluna 1: USE nao e um comando desta camada` |
| `INSERT … ON DUPLICATE KEY UPDATE` | `SQL, coluna 1: INSERT ainda nao existe nesta camada -- so SELECT. …` |
| `REPLACE INTO` | `SQL, coluna 1: REPLACE nao e um comando desta camada` |
| `SELECT * FROM clientes LIMIT 0, 2` (`LIMIT` com vírgula) | `SQL, coluna 31: sobrou "," depois do fim do comando; um comando por vez` |

**Os cinco primeiros são um bloco, e é o bloco do driver.** `SHOW TABLES`,
`SHOW DATABASES`, `DESCRIBE` e `USE` são o que um cliente MySQL(R)/MariaDB(R)
(e o DBeaver, e o Excel por ODBC) manda **antes** de o usuário digitar
qualquer coisa. Os três primeiros já têm resposta pronta no protocolo — é
tradução de quatro linhas cada. O `USE` é diferente: ele guarda estado **na
conexão**, como o `BULKINSERT` e a transação, e o desenho disso já está escrito
em `docs/SQL.md` §2.

O `LIMIT 0, 2` é a única diferença puramente sintática da lista: o motor
entende `LIMIT n OFFSET m` e não a forma com vírgula, que é a que todo código
MySQL(R) do mundo escreve.

`ON DUPLICATE KEY` e `REPLACE` são o *upsert*, e importam porque numa carga
repetida é o que evita a escolha entre duplicar e falhar.

### (b) Falta, e NÃO importa aqui — 6, com o motivo

| comando | por que não é essencial neste motor |
|---|---|
| `SELECT … FOR SYSTEM_TIME AS OF` (tabelas com versionamento) | `SQL, coluna 28: sobrou "SYSTEM_TIME" …`. É o item 22 da lista de sprints, e a premissa que pode matá-lo é do dono: a imagem da linha nasce **desligada**, e ligá-la custa ~10% da vazão e 5× o diário |
| `CREATE EVENT` (event scheduler) | `CREATE nesta camada cria TRIGGER ou PROCEDURE`. Há agendador aqui, com outro nome: `job_salvar`/`job_rodar`, com aviso por e-mail. Comando novo seria segunda porta para a mesma coisa |
| `CREATE FUNCTION` | recusa **nomeando o motivo**: `CREATE FUNCTION nao existe nesta camada — so TRIGGER e PROCEDURE. Funcao devolveria valor dentro de expressao SQL, e a camada SELECT nao avalia expressao`. Antes do avaliador, uma função seria um valor que ninguém pode usar |
| `ANALYZE SELECT …` (o EXPLAIN que executa) | `ANALYZE nao e um comando desta camada`. É o item 9 da lista, e a premissa dele é honesta: *o EXPLAIN diz algo que as `notas` já não digam?* Hoje as notas já dizem o índice escolhido e que não há planejador |
| `LOAD DATA INFILE` | `LOAD nao e um comando desta camada`. A carga existe por operação (`importar_conferir` + `inserir_lote` + `BULKINSERT`), e ler arquivo do disco do servidor a pedido do cliente é superfície de ataque que este motor não quer |
| backtick (`` `nome` ``) | `SQL, coluna 8: caractere '\`' nao faz parte da linguagem`. O aspeamento aqui é o do padrão, `"nome"` |

### (c) Existe, com outro nome ou outra forma — 9

| SQL do MariaDB(R) | o que faz a mesma coisa aqui | prova desta corrida |
|---|---|---|
| `SHOW TABLES` | `{"op":"tabelas"}` | `{'database': 'loja', 'schemas': [], 'tabelas': ['chamados', 'clientes', 'itens']}` |
| `SHOW DATABASES` | `{"op":"bancos"}` | `['loja']` |
| `DESCRIBE` / `SHOW COLUMNS` | `{"op":"esquema"}` e `{"op":"siscolunas"}` | esquema completo, 6 colunas |
| `MATCH(col) AGAINST('palavra')` | `{"op":"procurar_texto"}` sobre o `.fts` | `{'encontrados': 1, 'linhas': [{'rowid': 1, 'id': 1, 'corpo': 'a fenix renasce das cinzas', 'softdeleted': False, …` |
| `CREATE SEQUENCE` / `NEXTVAL` | `{"op":"sequencias"}` e `{"op":"ajustar_sequencia"}` — **uma por tabela** | `{'database': 'loja', 'total': 3, 'sequencias': [{'tabela': 'chamados', 'coluna': None, 'proxima': 0, 'registros': 1, 'tem_sequencia': False}, {'tabela': 'clientes', 'coluna': 'id', 'proxima': 7, 'registros': 6, 'tem_sequencia': True}, …]}` |
| `CHECK TABLE` | `{"op":"verificar"}` | `{'tabela': 'clientes', 'registros': 6, 'slots': 6, 'eventos': 9, 'indices': {'porId': 6, 'porNome': 6}, …` |
| `OPTIMIZE TABLE` (a parte útil) | `{"op":"reindexar"}` | `{'porId': 6, 'porNome': 6}` |
| `CHECKSUM TABLE` | `{"op":"checksum"}` | `{'database': 'loja', 'tabela': 'itens', 'checksum': '1e1a28f552b21afb', 'linhas': 1, 'slots': 1, 'ms': 0}` |
| `LOCK TABLES … WRITE` para carga | `{"op":"bulkinsert","ligado":true}` | `{'bulkinsert': True, 'database': 'loja', 'tabela': 'itens', 'reservada': True, 'expira_em_s': 1800, 'prazo_min': 30}` — e o `BULKINSERT` **não** é transação: não desfaz |

E o **`.fts` medido nesta corrida**, porque uma equivalência que não se prova
não vale: ele **dobra acento** e **não faz prefixo**, exatamente como o
`docs/FTS.md` promete.

```
[OK] procurar_texto  palavra="fênix"  ->  {'encontrados': 1, 'linhas': [{'rowid': 1, 'id': 1, 'corpo': 'a fenix renasce das cinzas', 'softdeleted': False, …   # dobra acento
[OK] procurar_texto  palavra="fen"    ->  {'encontrados': 0, 'linhas': []}   # não é prefixo
```

### O que a fonte oficial lista como exclusivo do MariaDB(R), e o estado de cada um aqui

Da página de comparação oficial (https://mariadb.com/kb/en/mariadb-vs-mysql-features/):

| exclusivo do MariaDB(R) | aqui |
|---|---|
| `CHECK` constraint | **não existe** — `ALTER nao e um comando desta camada`; item 16 da lista de sprints |
| colunas geradas (`PERSISTENT`) | **não existe** — mesma recusa; item 17 |
| colunas `INVISIBLE` | **não existe**, e está **recusado com motivo**: torná-las invisíveis mudaria a resposta de quem já lê `rownum`/`softdeleted` hoje |
| `CREATE SEQUENCE` como objeto | **existe pela metade**: uma sequência por tabela, sem objeto próprio; item 15 |
| tabelas com versionamento de sistema | **não existe**; item 22 |
| window functions / CTE recursiva | **não existe**; dependem do avaliador |
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
