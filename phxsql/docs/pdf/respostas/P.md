# P) leitura do manual do mysql fonte e help verificando gaps ainda existentes gerar lista de sprints que julgue importante

## Resposta curta
Não havia estudo do MySQL(R): parti do do MariaDB(R) e mandei **23 comandos escolhidos
onde o MySQL 8 diverge dele — 23 recusados, 0 aceitos** (**6 faltam e importam**, **10 não
importam aqui**, e **8 equivalências provadas** pelo protocolo). A leitura do manual
**corrigiu duas afirmações** que o estudo antigo carregava: papéis e `EXCEPT`/`INTERSECT`
**não são exclusivos do MariaDB(R)**, e a fonte dele é anterior ao MySQL 8. **Lista final:
8 sprints** — 5 compartilhados com a resposta O, 3 só do MySQL(R).

## Exemplo exercitado

Corrida de **2026-09-07 16:49 UTC**, commit **a56a165**. **Fonte:** o MySQL 8.4
Reference Manual — https://dev.mysql.com/doc/refman/8.4/en/sql-statements.html,
https://dev.mysql.com/doc/refman/8.4/en/create-role.html e
https://dev.mysql.com/doc/refman/8.4/en/set-operations.html — lidos nesta rodada.

### Onde o MySQL 8 diverge do MariaDB(R), no que importa — e o estado de cada um aqui

| assunto | MySQL 8.4 | MariaDB(R) | aqui, medido nesta corrida |
|---|---|---|---|
| **papéis** | **tem**: `CREATE ROLE [IF NOT EXISTS] role` (manual 8.4, página citada) | tem, desde a 10.0 | **não tem**: `CREATE ROLE app_ro` → `CREATE nesta camada cria TRIGGER ou PROCEDURE`; `SET ROLE app_ro` → `SET nao e um comando desta camada`; `GRANT app_ro TO leitor` → `GRANT nao e um comando desta camada` |
| **`EXCEPT` / `INTERSECT`** | **tem os dois**, com `ALL`/`DISTINCT` (manual 8.4: *«MySQL supports UNION, INTERSECT, and EXCEPT»*) | tem | **não tem**: `SELECT … EXCEPT SELECT …` → `sobrou "SELECT" depois do fim do comando; um comando por vez`. O `unir` tem `tudo` e `distinta`, e mais nada |
| **`JSON` como tipo de coluna** | **tem** tipo nativo, com armazenamento binário e `JSON_TABLE` | o `JSON` dele é apelido de `LONGTEXT` com `CHECK` | **não tem**: `CREATE TABLE j (dados JSON)` recusa, e `SELECT JSON_EXTRACT(nome,'$.a')` → `SQL, coluna 20: esperava FROM, e veio "("`. **Há analisador JSON próprio nesta casa** (escrito à mão, zero dependências), e é a matéria-prima — falta o avaliador de expressão que o use |
| **window functions** | tem (8.0) | tem (10.2) | **não tem**: `RANK() OVER (ORDER BY nome)` → `SQL, coluna 18: esperava FROM, e veio "("` |
| **CTE / CTE recursiva** | tem (8.0) | tem (10.2) | **não tem**: `WITH RECURSIVE r AS (…)` → `WITH nao e um comando desta camada` |
| **`LATERAL`** | **tem** (só ele) | não tem | **não tem**: `SELECT * FROM clientes, LATERAL (SELECT 1) x` → `sobrou "," depois do fim do comando` |
| **`VALUES` e `TABLE` como comando** | **tem** (só ele) | não tem | **não tem**: `VALUES ROW(1,2)` → `VALUES nao e um comando desta camada`; `TABLE clientes` → `TABLE nao e um comando desta camada` |
| **`SELECT … FOR SHARE` / `FOR UPDATE`** | tem, com `NOWAIT`/`SKIP LOCKED` | tem | **não tem a cláusula**, e tem **outra forma**: a janela de conflito por `versao` do `atualizar` — trava otimista em vez de pessimista. `FOR SHARE` → `sobrou "FOR" depois do fim do comando` |
| **agrupamento em cluster** | InnoDB Cluster / Group Replication (só ele) | Galera | **tem, com outro desenho**: cluster com árbitro, eleição por maioria e promoção automática (`docs/CLUSTER.md`). Não subi cluster nesta corrida — o `config` deste servidor isolado responde `{'papel': 'isolado', 'papel_configurado': 'isolado', 'id_servidor': '', 'somente_leitura': False, 'origens': {}}` |
| **sequências** | **não tem** | tem `CREATE SEQUENCE` | **tem pela metade**: uma por tabela — `{"op":"sequencias"}` → `{'database': 'loja', 'total': 3, 'sequencias': [{'tabela': 'chamados', 'coluna': None, 'proxima': 0, 'registros': 1, 'tem_sequencia': False}, …` |

**A correção que a leitura obrigou:** o `docs/SPRINTS-MARIADB.md` §2 lista
papéis e `EXCEPT`/`INTERSECT` como exclusivos do MariaDB(R), citando
https://mariadb.com/kb/en/mariadb-vs-mysql-features/. Fui à página e ela de fato
diz isso — **e o manual do MySQL 8.4 diz o contrário**, nas duas páginas citadas
acima. Não é erro do estudo anterior: é a fonte dele que envelheceu. O efeito
prático é que **dois sprints ganham uma segunda fonte** em vez de serem
«diferencial de um concorrente só».

**As três listas abaixo se cruzam, e é de propósito.** Um `INSERT` recusado está ao
mesmo tempo em (a) — falta na linguagem — e em (c) — existe no motor com outro nome.
Somar as três não dá o total de recusas, e uma soma que fechasse esconderia
justamente o que interessa: quantos gaps têm saída hoje.

### (a) Falta, e IMPORTA para um cadastro comum — 6

| comando | a recusa REAL, colada |
|---|---|
| `SELECT * FROM information_schema.tables` | `[SP000010] erro de E/S: No such file or directory (os error 2)` — **e vem com `"repetir": true`** |
| `CREATE ROLE` / `SET ROLE` / `GRANT role TO user` | `CREATE nesta camada cria TRIGGER ou PROCEDURE` / `SET nao e um comando desta camada` / `GRANT nao e um comando desta camada` |
| `INSERT INTO … VALUES ('A'),('B')` (multi-linha) | `INSERT ainda nao existe nesta camada -- so SELECT. A operacao equivalente ja funciona pelo protocolo` |
| `SHOW VARIABLES LIKE 'version'` | `SHOW nesta camada lista TRIGGERS ou PROCEDURES; tabelas e colunas saem por sistabelas/siscolunas` |
| `START TRANSACTION READ ONLY` | `SQL, coluna 1: sobrou "READ" depois do comando de begin` |
| `GROUP_CONCAT(nome)` | `SQL, coluna 20: esperava FROM, e veio "("` |

**A primeira linha é a mais séria desta resposta inteira, e não é falta de
recurso: é defeito.** `information_schema` é o **primeiro** lugar aonde um
driver MySQL(R) vai, e a resposta é o erro cru do sistema operacional mandando
tentar de novo. O caminho irmão faz certo:

```
[ERRO] SELECT * FROM information_schema.tables   (schema.tabela)
  [SP000010] erro de E/S: No such file or directory (os error 2)     "repetir": true
[ERRO] SELECT * FROM naoexiste                    (só tabela)
  [SP000018] nao encontrado: nenhum volume de naoexiste.reg em …      "repetir": false
```

`SHOW VARIABLES` está na lista (a) pelo mesmo motivo: é o que o cliente manda
para descobrir a versão e o `sql_mode` antes de qualquer coisa.

`START TRANSACTION READ ONLY` merece uma nota boa: **a recusa está certa** —
sobra depois do comando é erro, e aceitar calado devolveria uma transação de
escrita a quem pediu leitura. O gap é a cláusula, não o comportamento.

### (b) Falta, e NÃO importa aqui — 10, com o motivo

| comando | por que não é essencial neste motor |
|---|---|
| `JSON_EXTRACT` e o tipo `JSON` | depende do avaliador de expressão. Antes dele, uma coluna `JSON` seria um `Str` com nome bonito |
| window functions, CTE, CTE recursiva | é o que uma ferramenta de BI gera sozinha; senta em cima do `GROUP BY` que não existe |
| `LATERAL`, `VALUES ROW(…)`, `TABLE t` | sintaxe de conveniência do dialeto; nenhuma tela de cadastro as escreve |
| `OPTIMIZE TABLE` | compactar renumera rowid, e rowid é endereço. **Não**, não «ainda não» |
| `FLUSH TABLES` | `FLUSH nao e um comando desta camada`. A durabilidade aqui é por janela configurada (`recursos.durabilidade: 'por_lote'`), não por comando do cliente |
| `LOCK TABLES clientes WRITE` | `LOCK nao e um comando desta camada`. O que ele compra numa carga, o `BULKINSERT` já dá — e com prazo (`{'bulkinsert': True, 'database': 'loja', 'tabela': 'itens', 'reservada': True, 'expira_em_s': 1800, 'prazo_min': 30}`) |
| `KILL 1` | `KILL nao e um comando desta camada`. Existe como operação: `encerrar_sessao` |
| `SHOW STATUS` | há coisa melhor por operação: `{"op":"estatisticas"}`, `{"op":"painel"}`, `{"op":"telemetria"}` |
| `CHECK TABLE` | existe por operação (`verificar`) |
| replicação por comando (`CHANGE REPLICATION SOURCE TO`, `START REPLICA`) | a replicação aqui é configuração e operação (`replicacao_estado`, `replicar`, `aplicar`, `cluster_pulso`), não vocabulário SQL |

### (c) Existe, com outro nome ou outra forma — 8

| MySQL(R) | aqui | prova desta corrida |
|---|---|---|
| `SHOW PROCESSLIST` | `{"op":"sessoes"}` | `{'quantas': 1, 'executando': 1, 'mais_longa_ms': 0, …` |
| `KILL <id>` | `{"op":"encerrar_sessao","id":…}` | operação do catálogo |
| `information_schema.TABLES` | `{"op":"sistabelas"}` | `{'database': 'loja', 'total': 3, 'tabelas': [{'tabela': 'chamados', 'schema': '', 'registros': 1, 'slots': 1, 'colunas': 4, 'indices': 1, …` |
| `information_schema.COLUMNS` | `{"op":"siscolunas"}` | `{'database': 'loja', 'total': 6, 'colunas': [{'tabela': 'clientes', 'posicao': 1, …` |
| `CHECK TABLE` / `OPTIMIZE TABLE` (a parte útil) | `{"op":"verificar"}` / `{"op":"reindexar"}` | `{'tabela': 'clientes', 'registros': 6, 'slots': 6, 'eventos': 9, 'indices': {'porId': 6, 'porNome': 6}, …` / `{'porId': 6, 'porNome': 6}` |
| `SELECT … FOR UPDATE` | a janela de conflito por `versao` | `{"op":"atualizar"}` → `{'rowid': 1, 'versao': 2}` |
| `LOCK TABLES … WRITE` (para carga) | `{"op":"bulkinsert"}` | `{'bulkinsert': True, 'database': 'loja', 'tabela': 'itens', 'reservada': True, 'expira_em_s': 1800, 'prazo_min': 30}` — e ele **não** desfaz: não é transação |
| `SET autocommit = 0` | `{"op":"begin"}` / `BEGIN` pelo SQL | `{'transaction_id': 1788799736955, 'transaction_state': 'ACTIVE', 'transaction_start_time': '2026-09-07 16:48:56,998', 'transaction_isolation': 'escrita serializavel por tabela, leitura confirmada e nao bloqueante, …` |

### A lista de sprints que eu proponho — 8

**Cinco são os mesmos da resposta O** e estão aqui só com o número da ordem de
lá, para não duplicar trabalho; **três são só do MySQL(R)**.

| ordem | sprint | tam. | por quê |
|--:|---|:--:|---|
| 1 | **A tabela que não existe recusa dizendo isso, com `repetir: false`** (= O-2) | **P** | aqui a prioridade **sobe**: `information_schema.<qualquer>` é o primeiro pedido de um driver MySQL(R), e é exatamente o caminho `schema.tabela` que vaza o erro cru e manda repetir |
| 2 | **`information_schema` como tabela consultável por `SELECT`** — novo | **M** | não basta consertar o erro: o driver quer `SELECT … FROM information_schema.TABLES`, e a resposta já existe pronta em `sistabelas`/`siscolunas`. É tradução de nome, com o portão de permissão continuando um só. **Premissa:** as colunas que o cliente lê de fato (o driver ODBC daqui já sabe quais) |
| 3 | **O `WHERE` que filtra, e a segunda condição** (= O-4) | **M** | mesma justificativa, e `GROUP_CONCAT`/`JSON_EXTRACT`/window functions do MySQL(R) todos esperam por ele |
| 4 | **`INSERT`/`UPDATE`/`DELETE` por chave primária, em SQL** (= O-5) | **M** | com o acréscimo do MySQL(R): `INSERT` **multi-linha** (`VALUES (…),(…)`) mapeia direto no `inserir_lote`, que já existe e é 16,3× a linha a linha |
| 5 | **`SHOW VARIABLES` e `SHOW STATUS` mínimos** — novo | **P** | é o aperto de mão do cliente MySQL(R). Não precisa das 600 variáveis: precisa de `version`, `sql_mode` e do que o driver lê para decidir o dialeto. **Premissa:** quais o cliente realmente pede — mede-se ligando um e olhando o Profiler |
| 6 | **Papéis no modelo de direitos** (= O-10) | **M** | **sobe de prioridade**: com o manual do MySQL 8.4 na mesa, papel deixa de ser diferencial de um motor e passa a ser o que os dois grandes têm |
| 7 | **`EXCEPT` e `INTERSECT`** (= O-9) | **P** | **sobe pelo mesmo motivo**, e continua barato: a máquina do `unir` já compara linhas |
| 8 | **`START TRANSACTION READ ONLY` e o nível de isolamento pedido pelo nome** — novo | **P** | hoje `SET TRANSACTION ISOLATION LEVEL SERIALIZABLE` recusa com `SET nao e um comando desta camada` — genérico. O `docs/SQL.md` §3 já manda outra coisa: a recusa tem de **dizer o nível real**, e ele existe pronto na resposta do `begin` (`'escrita serializavel por tabela, leitura confirmada e nao bloqueante, sem leitura repetivel'`). Um `Ok` que promete o que o motor não faz seria pior; uma recusa que não explica é só ruim |

## O que NÃO existe, e é dispensa registrada

- **Não existe nenhum estudo anterior do MySQL(R)** nesta casa, e esta resposta
  não o substitui: ela é a **delta** sobre o do MariaDB(R), medida contra o
  motor. Um estudo próprio leria o capítulo 15 inteiro; eu li o índice dele e
  três páginas, e mandei 23 comandos.
- **Não subi um MySQL(R) para comparar comportamento.** Onde afirmo o que o
  MySQL 8.4 faz, a fonte é a página citada; onde afirmo o que o PhxSql faz, a
  fonte é a saída colada. Não há número de desempenho nesta resposta, e é de
  propósito: a bancada que compara os dois é outra (`bancada/comparacao`), e
  medir aqui daria número novo sem método.
- **Não exercitei o cliente MySQL(R) do DbLink** (`dblink_salvar` com
  `"motor":"mysql"`, `dblink_consultar`), que fala o protocolo de fio deles e
  existe. Esta resposta é sobre o texto SQL que **entra** pela op `sql`, não
  sobre o que **sai** pelo DbLink.
- **`ACID compliant` não aparece aqui, e não vai aparecer:** há transação, e o
  nível está medido em `docs/ACID.md` — leitura não repetível, fantasma e
  *write skew* acontecem; leitura suja, não.

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py mysql
```
