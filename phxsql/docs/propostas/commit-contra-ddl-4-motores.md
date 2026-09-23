# `COMMIT` contra DDL concorrente — os quatro motores, três deles MEDIDOS aqui

Pesquisa do papel **J**, 23/09/2026, pelo ciclo da pétrea «o pesquisador decide;
o dono é o impasse». Pedida pelo pedido **426** do `docs/PENDENCIAS.md`.

**O que muda nesta rodada:** PostgreSQL 16.13, MySQL 8.0.46 e SQLite 3.45.1
estão **instalados nesta máquina** e foram **exercitados** — duas sessões
concorrentes, comando exato e saída exata abaixo. **MariaDB não está
instalado**: tudo o que aparece dele é **LIDO** (KB e `errmsg-utf8.txt` do
fonte), e está marcado assim em toda linha. Onde eu li em vez de medir, digo.

A pergunta: **quando uma transação aberta esbarra numa mudança de esquema
concorrente, o que os motores fazem?**

---

## 0. As hipóteses, escritas ANTES de medir

Estão como foram escritas, em
`scratchpad/HIPOTESES.txt`, às 21:39 UTC, antes da primeira sessão subir.

| # | hipótese | veredito medido |
|---|---|---|
| **H1** | «o DDL espera»: nos três maduros o DDL fica atrás de uma trava de metadados presa pela transação até o fim; o `COMMIT` **nunca** falha por DDL concorrente | **VIVA** nos três maduros (PG 4,10 s e MySQL 4,03 s de espera, medidos). **MORTA** no SQLite, que **recusa** o DDL em 0,005 s em vez de enfileirá-lo |
| **H2** | «aborta inteira, com classe dedicada e repetível»: o choque sai com SQLSTATE de **classe 40** (serialization failure), oficialmente repetível | **MORTA, e é o achado que mais poupa trabalho.** Nenhum dos quatro usa classe 40 para o choque com DDL. PG usa **55P03** (classe 55) e **0A000** (classe 0A); MySQL/MariaDB usam **1412** e **1205**, ambos **HY000** — a classe genérica. Só o abraço mortal de verdade é 40001. **O sinal de «repita» mora no CÓDIGO e no TEXTO do erro, não na classe.** Quem desenhar isto no PhxSql não deve sair procurando classe nova |
| **H3** | «alguém sai pela metade»: pelo menos um dos quatro aplica parte e falha o resto, como o PhxSql hoje | **MORTA pela metade**, e a metade que sobra é a que decide. Nenhum motor sai pela metade **por decisão sua**. O MySQL sai pela metade **por decisão do cliente** — medido nos dois sentidos: `COMMIT` depois do erro grava a metade, `ROLLBACK` depois do mesmo erro deixa **zero** |
| **H4** | «quando falha, falha na INSTRUÇÃO e não no `COMMIT`» | **VIVA nos quatro.** Única exceção: o SQLite admite `SQLITE_BUSY` no próprio `COMMIT` — e aí, por documentação oficial, **a transação fica ativa** e o `COMMIT` se repete. Continua sem metade |

---

## 1. O que foi medido, comando a comando

Servidores subidos por mim e **derrubados por mim** ao terminar (PG `16/main`
PID 27348, `mysqld` PID 28541; `mysqld` com `--skip-log-bin` para não crescer
binlog com o disco em 2,3 GB). Nenhum processo de terceiro foi tocado. Os
bancos de teste `t426` foram removidos.

### 1.1 PostgreSQL 16.13 — MEDIDO

**PG-1 — transação com escrita em duas tabelas, `ALTER TABLE` concorrente na
segunda, depois `COMMIT`.**

Sessão A: `BEGIN; INSERT INTO mae …; INSERT INTO filha …;` e fica parada.
Sessão B: `ALTER TABLE filha ADD COLUMN extra int;`

Depois de 4 s, o estado do servidor, lido dele mesmo:

```
27770|Client|ClientRead|idle in transaction|SELECT 'A: escritas empilhadas' AS marco;
27784|Lock|relation|active|ALTER TABLE filha ADD COLUMN extra int;
-- pg_locks WHERE NOT granted:
relation|filha|AccessExclusiveLock|f
```

A manda `COMMIT;` → `COMMIT`. B então devolve `ALTER TABLE` com
`B_RC=0 B_FIM_EM=4.104330025s`. As duas linhas no disco.
**O DDL esperou 4,10 s. O `COMMIT` não sentiu nada.**

**PG-1b — o DDL espera até atrás de um LEITOR puro.** Uma transação
`REPEATABLE READ` que só fez `SELECT` segura `AccessShareLock`; o `ALTER`
ficou com `AccessExclusiveLock` **não concedida** por tempo indefinido
(`lock_timeout` de fábrica é `0`). Tive de matar a minha própria sessão para
destravar — e é o preço documentado do desenho deles.

**PG-2/3 — o DDL segura primeiro; a transação bate nele.** A com
`SET lock_timeout='3s'`:

```
INSERT INTO mae VALUES (1,'pai-1');        -> INSERT 0 1
INSERT INTO filha VALUES (10,1,'…');
ERROR:  55P03: canceling statement due to lock timeout
COMMIT;                                     -> ROLLBACK
SELECT count(*) FROM mae;                   -> 0
```

Três coisas num bloco só: a falha é da **instrução**; o `COMMIT` devolve o rótulo
**`ROLLBACK`** e **não é erro**; e o `INSERT` que já tinha dado certo **sumiu**.
Atômica.

**PG-4 — abraço mortal entre a transação e o DDL. Quem morre?**

```
INSERT INTO filha VALUES (10,1,'x');
ERROR:  40P01: deadlock detected
DETAIL:  Process 28033 waits for RowExclusiveLock on relation 57352 …; blocked by process 28034.
         Process 28034 waits for AccessExclusiveLock on relation 57345 …; blocked by process 28033.
COMMIT;   -> ROLLBACK
SELECT count(*) FROM mae;  -> 0
```

**O DDL ganhou**: a sessão B commitou os dois `ALTER TABLE`; a vítima foi a
transação DML, e ela saiu com **zero** aplicado.

**PG-5 — plano em cache e catálogo.** Uma transação `REPEATABLE READ` cujo
instantâneo é **anterior** ao DDL vê a coluna **nova** num `SELECT *` — o
catálogo do PG não obedece ao instantâneo da transação. Mas o plano genérico
guardado por `PREPARE` quebra:

```
EXECUTE p;
ERROR:  0A000: cached plan must not change result type
INSERT INTO mae VALUES (2,'pai-2');
ERROR:  25P02: current transaction is aborted, commands ignored until end of transaction block
COMMIT;  -> ROLLBACK
```

O **25P02** é a garantia estrutural do PG contra a metade: depois do primeiro
erro ele **recusa fazer mais qualquer coisa**. Não há como chegar ao `COMMIT`
com meio trabalho feito.

DDL sem concorrente: **0,049 s**.

### 1.2 MySQL 8.0.46 — MEDIDO

Fábrica, lida do próprio servidor: `transaction_isolation=REPEATABLE-READ`,
`lock_wait_timeout=31536000` (**um ano**), `innodb_lock_wait_timeout=50`.

**MY-1 — o mesmo PG-1.** `performance_schema.metadata_locks` durante a espera:

```
filha | SHARED_WRITE      | GRANTED | 52   <- a transação
filha | SHARED_UPGRADABLE | GRANTED | 53   <- o ALTER
filha | EXCLUSIVE         | PENDING | 53   <- o ALTER esperando
-- processlist: "Waiting for table metadata lock"
```

`B_RC=0 B_LEVOU=4.031251946s`. **O DDL esperou 4,03 s.** `COMMIT` OK, as duas
linhas gravadas. Converge com o PG.

*Nota colhida de graça e que vale para nós:* o `ALTER TABLE filha` tomou
`SHARED_UPGRADABLE` **também em `mae`**, por causa da chave estrangeira. O
congelamento da filha alcança a mãe lá.

**MY-2 — a divergência, e é a medição que mais importa deste documento.**
Tabela `t2` congelada por outra sessão (`LOCK TABLES t2 WRITE`, que é o
equivalente exato da nossa FASE A). A com `lock_wait_timeout=3`:

```
INSERT INTO t1 VALUES (1,'primeira');      -> OK
INSERT INTO t2 VALUES (10,'segunda');
ERROR 1205 (HY000) at line 5: Lock wait timeout exceeded; try restarting transaction
-- visto de FORA, information_schema.innodb_trx:
142696 | RUNNING | 25 | trx_rows_modified=1 | trx_tables_locked=1
SELECT COUNT(*) FROM t1;                   -> 1   (lê a própria escrita)
COMMIT;                                    -> OK
-- no disco: t1 com a linha, t2 SEM.
```

**O MySQL sai pela metade.** E o ponto é *quem* escolheu: repetido o cenário
trocando só a última palavra —

```
ROLLBACK;
SELECT COUNT(*) FROM t1;  -> 0     (e t2 também 0)
```

O motor **nunca** relatou «o `COMMIT` falhou» tendo aplicado parte. Ele parou
na **instrução**, disse o que houve, deixou a transação **viva** e devolveu ao
cliente as duas saídas inteiras.

**MY-3 — DDL dentro de transação não existe.** Medido: A deu
`START TRANSACTION; INSERT INTO t1 …; ALTER TABLE t2 …;` e a linha de `t1`
ficou **gravada** — o `ALTER` commitou a transação aberta antes de rodar, e o
MDL foi solto. Logo, **em MySQL o DDL nunca participa de uma transação**, e o
abraço mortal DDL×transação daquela forma não tem como existir.
(Fonte oficial: «The statements listed in this section … implicitly end any
transaction active in the current session, as if you had done a `COMMIT`
before executing the statement», com `ALTER TABLE` na lista —
<https://dev.mysql.com/doc/refman/8.0/en/implicit-commit.html>.)

**MY-4 — o erro DEDICADO a este caso existe, e eu o reproduzi.** Transação em
`REPEATABLE READ` cuja *read view* nasceu antes do DDL, tocando a tabela
alterada pela primeira vez:

```
SELECT * FROM t2;
ERROR 1412 (HY000) at line 5: Table definition has changed, please retry transaction
```

**MY-5 — o contraste que separa «instrução» de «transação».** Abraço mortal de
linha no InnoDB:

```
ERROR 1213 (40001) at line 4: Deadlock found when trying to get lock; try restarting transaction
```

A vítima teve a transação **inteira** desfeita — o `UPDATE t2 SET v='B2'` que
já tinha dado certo sumiu (o valor final é `A2`, do outro). Ou seja, o MySQL
tem dois níveis, e **ele diz qual é qual**: 1205 desfaz a instrução, 1213
desfaz a transação.

**O catálogo, impresso pelo próprio servidor (`perror`):**

```
MySQL error code MY-001205 (ER_LOCK_WAIT_TIMEOUT): Lock wait timeout exceeded; try restarting transaction
MySQL error code MY-001213 (ER_LOCK_DEADLOCK): Deadlock found when trying to get lock; try restarting transaction
MySQL error code MY-001412 (ER_TABLE_DEF_CHANGED): Table definition has changed, please retry transaction
```

### 1.3 SQLite 3.45.1 — MEDIDO

**SQ-1 — transação de escrita aberta, `ALTER TABLE` concorrente.**

```
-- com .timeout 0
Error: stepping, database is locked (5)      B_RC=5 em .005355059s
-- com .timeout 3000
Error: stepping, database is locked (5)      B_RC=5 em 3.011046789s
```

**O DDL é RECUSADO, não enfileirado** — esperou os 3,011 s que mandei esperar e
**ainda assim falhou**. A transação seguiu e commitou as duas linhas.
É aqui que o SQLite **diverge dos três maduros**.

**SQ-2 — invertido: o DDL segura, a transação tenta.** Nem abrir ela abriu:

```
Runtime error near line 2: database is locked (5)      <- BEGIN IMMEDIATE
Runtime error near line 3: database is locked (5)      <- INSERT
Runtime error near line 6: cannot commit - no transaction is active
-- no disco: nada aplicado
```

**A recusa acontece na porta.** Sem porta aberta, não há metade.

**SQ-3 — `SQLITE_BUSY_SNAPSHOT`.** Transação `BEGIN DEFERRED` que leu primeiro;
DDL externo comita; A tenta escrever:

```
PRAGMA schema_version;  -> 3     (antes do DDL)
PRAGMA schema_version;  -> 3     (DEPOIS do DDL: A continua no instantâneo velho)
SELECT * FROM t1;       -> 1|primeira        (duas colunas; no disco já há três)
INSERT INTO t1 …;       -> database is locked (5)
COMMIT;                 -> OK     (era só leitura; zero aplicado)
```

**Documentação oficial, citada porque diz o que o CLI não mostra:**
<https://www.sqlite.org/rescode.html> — «An SQLITE_BUSY error can occur at any
point in a transaction: when the transaction is first started, during any write
or update operations, **or when the transaction commits**.» E
<https://www.sqlite.org/lang_transaction.html> — «When COMMIT fails in this way,
**the transaction remains active** and the COMMIT can be retried later after the
reader has had a chance to clear.»

**É o único dos quatro em que o `COMMIT` pode falhar — e mesmo aí não há
metade: a transação fica de pé, inteira, para repetir o `COMMIT`.**

### 1.4 MariaDB — LIDO, NÃO MEDIDO

**Não está instalado nesta máquina.** Tudo abaixo é leitura de fonte primária.

- KB oficial, *Metadata Locking* (<https://mariadb.com/kb/en/metadata-locking/>):
  «When a connection tries to use a DDL statement (like an ALTER TABLE) which
  modifies a table that is locked, that connection is **queued**, and has to wait
  until it's unlocked.» `lock_wait_timeout` de fábrica **31536000** (um ano); ao
  estourar, `ERROR 1205 (HY000): Lock wait timeout exceeded; try restarting
  transaction`.
- **Fonte**, `sql/share/errmsg-utf8.txt` do branch `10.11`, baixado e lido aqui:
  - `:4778` `ER_LOCK_WAIT_TIMEOUT` (sem linha de sqlstate → HY000) — `eng "Lock wait timeout exceeded; try restarting transaction"`
  - `:4913` `ER_LOCK_DEADLOCK 40001` — `eng "Deadlock found when trying to get lock; try restarting transaction"`
  - `:6535` `ER_TABLE_DEF_CHANGED` — `eng "Table definition has changed, please retry transaction"`
  **Mesmo catálogo do MySQL, palavra por palavra.**
- Divergência própria dela, e vale para o 426: MariaDB tem
  **`ALTER TABLE … NOWAIT` e `… WAIT n`**
  (<https://mariadb.com/kb/en/wait-and-nowait/>), também em `DROP TABLE`,
  `CREATE INDEX`, `LOCK TABLE`, `RENAME TABLE`, `TRUNCATE TABLE`. **É o DDL
  que declara quanto espera** — nunca a transação que paga.

---

## 2. A matriz dos quatro

Legenda: **[M]** medido aqui hoje · **[L]** lido em fonte primária.

| pergunta | PostgreSQL 16.13 (peso 4) | MariaDB (peso 3) | MySQL 8.0.46 (peso 2) | SQLite 3.45.1 (peso 1) |
|---|---|---|---|---|
| **(1) o `COMMIT` pode falhar por DDL concorrente?** | **[M] não.** O `COMMIT` devolve o rótulo `ROLLBACK`, que não é erro; a falha já aconteceu na instrução | **[L] não** (mesmo MDL, mesmo catálogo do MySQL) | **[M] não.** Ou passa, ou foi precedido de erro de instrução | **[M/L] sim** — `SQLITE_BUSY` pode sair no `COMMIT`; mas a transação **fica ativa** e se repete o `COMMIT` |
| **erro/SQLSTATE do choque** | **[M]** `55P03 lock_not_available` (classe 55); `0A000 feature_not_supported` (classe 0A); `40P01 deadlock_detected` (classe 40) só no abraço mortal real | **[L]** 1205 HY000; 1412 HY000; 1213 **40001** | **[M]** 1205 HY000; **1412 HY000 `ER_TABLE_DEF_CHANGED`**; 1213 40001 | **[M]** `SQLITE_BUSY` (5) / `SQLITE_BUSY_SNAPSHOT` (517); `SQLITE_SCHEMA` (17) |
| **(2) atômica ou pela metade?** | **[M] atômica, sempre.** `25P02` recusa toda instrução depois do primeiro erro; `COMMIT`→`ROLLBACK`; 0 linhas | **[L]** como o MySQL | **[M] o cliente decide**, e as duas saídas foram medidas: `COMMIT`→metade, `ROLLBACK`→zero. O motor **nunca** relata `COMMIT` falho com parte aplicada | **[M] atômica.** Recusa na porta (`BEGIN IMMEDIATE` falha); 0 aplicado |
| **(3) diz «repita»? há classe dedicada?** | **[M]** classe **40 — Transaction Rollback** é a repetível, mas o choque com DDL **não cai nela** (cai em 55/0A) | **[L]** três erros dizendo `try restarting` / `please retry transaction`; classe SQLSTATE é **HY000** em dois dos três | **[M]** idem, e **1412 é o erro DEDICADO a este caso exato** | **[M/L]** `SQLITE_BUSY` é o código repetível; a doc diz que o `COMMIT` «can be retried later» |
| **(4) o DDL espera, ou a transação é abortada?** | **[M] o DDL ESPERA** — 4,10 s medidos; e indefinidamente atrás de um leitor puro (`lock_timeout=0` de fábrica). A transação só morre se **ela** pediu `lock_timeout`, ou no abraço mortal — e no abraço mortal medido **o DDL ganhou** | **[L] o DDL ESPERA** — «is queued», um ano de fábrica; e tem `NOWAIT` para o DDL desistir | **[M] o DDL ESPERA** — 4,03 s medidos, um ano de fábrica | **[M] o DDL é RECUSADO** em 0,005 s. Diverge dos três |
| **em todos os quatro, quem paga?** | **o DDL** | **o DDL** | **o DDL** | **o DDL** (desiste) |

---

## 3. A régua, com o número

### 3.1 Entra por aceite automático (convergência dos três maduros, nada nosso se opõe)

- **D1 — o `COMMIT` não falha por causa de DDL concorrente, porque o DDL
  espera a transação.** PG **[M]** 4,10 s; MySQL **[M]** 4,03 s; MariaDB
  **[L]** enfileira, um ano de fábrica. Nenhuma pétrea nossa se opõe.
  **Entra sem pergunta.**
- **D2 — nenhum motor devolve «o `COMMIT` falhou» tendo aplicado parte do
  conjunto.** Convergem os **quatro**, cada um por um caminho: PG pelo `25P02`
  **[M]**; MySQL entregando a escolha ao cliente na instrução **[M]**; SQLite
  recusando na porta **[M]**; MariaDB pelo mecanismo do MySQL **[L]**.
  → **o 426 é defeito por esta régua, não escolha de desenho.**
- **D3 — «repita» só se diz sobre transação que aplicou ZERO, e sempre na
  INSTRUÇÃO.** Os quatro. O erro dedicado existe e tem nome: **1412
  `ER_TABLE_DEF_CHANGED`** — «Table definition has changed, please retry
  transaction» — e ele é dito **antes** de qualquer `COMMIT`.

### 3.2 Divergem — decide a média ponderada, e o número vai escrito

- **D4 — erro de instrução MANTÉM a transação viva e devolve a escolha ao
  cliente.** «Mantém viva»: MariaDB **3** + MySQL **2** + SQLite **1** = **6**.
  «Aborta a transação inteira» (PG, pelo `25P02`): **4**. **6 × 4 → mantém
  viva.** É exatamente o que o `ClasseDoErro` desta casa já faz
  (`crates/phxsql-server/src/transacao.rs:171`, `Instrucao` × `Transacao`): a
  escolha do PhxSql já estava do lado vencedor, e agora está com o número.
- **D5 — o DDL ESPERA; não é recusado.** «Espera»: PG **4** + MariaDB **3** +
  MySQL **2** = **9**. «Recusa o DDL» (SQLite): **1**. **9 × 1 → espera.**
  E note o que não se decide por voto porque é unânime: **quem paga é o DDL,
  nunca a transação.**

### 3.3 Hipótese morta, com o número — o resultado que mais poupa tempo

**H2 morreu medida.** Não existe classe SQLSTATE dedicada ao choque «transação
aberta × DDL» em nenhum dos quatro. Contado: dos **cinco** erros que os motores
usam para este caso, **quatro** ficam fora da classe 40 — `55P03` (classe 55),
`0A000` (classe 0A), `1205` (HY000) e `1412` (HY000); só o `1213`/`40P01`, que
é abraço mortal de verdade, é classe 40. **O sinal de «repita» mora no CÓDIGO
e no TEXTO do erro, não na classe.** Quem for desenhar isto no PhxSql **não
deve** criar faixa nova de código nem procurar uma classe: é o par
(código, momento) que decide.

**H3 morreu pela metade, e a metade que ficou é o veredito sobre nós:** o
MySQL **consegue** sair pela metade — mas só porque o cliente mandou `COMMIT`
depois de ver o erro e de ler a própria escrita. Medido o mesmo cenário com
`ROLLBACK`: **zero**. **O PhxSql é o único dos cinco em que a metade acontece
sem ninguém ter pedido, e é anunciada ao cliente como falha.**

---

## 4. Choque com pétrea — aparece, não se aceita nem se ignora calado

**Não há choque nenhum em adotar D1–D5.** Mas há **duas divergências nossas**,
e a lei manda nomear a restrição que as causa.

**(a) O MEIO de conseguir atomicidade não se copia — e a restrição é «a ordem
de digitação é sagrada».** PG, MySQL e MariaDB conseguem o tudo-ou-nada
**desfazendo**: undo, rollback segment, reversão de página. Aqui o `.reg`
**nunca reaproveita slot excluído e nunca apaga digitação**, então desfazer
está proibido por pétrea. A nossa atomicidade só pode ser **para a frente** — a
marca sincronizada como ponto de compromisso e o `transacao::recuperar`
completando —, que é exatamente o que a tabela de energia no cabeçalho do
`op_commit` já promete (`crates/phxsql-server/src/servidor.rs:15497-15514`).
**Mesma garantia, outro mecanismo, restrição nomeada: isto é inspiração, não
cópia.**

**(b) O choque VIVO, e ele é do 426, não desta pesquisa:** o `repetir: true`
depois de meia aplicação colide com **«só existe filho se o pai existir
primeiro»**. O cliente repete; a ordem de digitação proíbe reaproveitar o slot,
então o pai reinserido nasce com **rowid novo**; e a filha da primeira tentativa
continua apontando para o pai da primeira. Uma repetição «inofensiva» vira dois
pais para uma linhagem, e o invariante do dono «o filho não pode ter a mesma
data do pai» passa a ter dois candidatos a pai. **A pesquisa não revoga pétrea
nenhuma: ela confirma que o comportamento de hoje já a fere.**

**Nada sobe ao dono.** Não há empate na matriz, não há troca de SLA, prazo ou
promessa de produto, e o único choque com pétrea é a favor da pétrea — ela
manda consertar, não manda perguntar.

---

## 5. O que isto manda o 426 fazer

1. **O `COMMIT` não pode ser o lugar onde o congelamento aparece.** D1, aceite automático: o portão decide **antes**, com a trava global na mão — nunca fora dela.
2. **Havendo transação viva cujo conjunto de escrita toque a tabela, quem espera ou desiste é a MIGRAÇÃO** (D5, 9 × 1). Nossos prazos de transação limitam a espera — vantagem sobre o «um ano» deles e sobre o `lock_timeout=0` do PG.
3. **`EmMigracao` (4006) no `COMMIT` não pode carregar `repetir: true`** (D3): «repita» só se diz sobre transação que aplicou **zero**.
4. **O carimbo tem de saber ONDE o erro aconteceu, não só qual erro é.** Hoje `adianta_repetir()` é propriedade do tipo (`phxsql-core/src/error.rs:296`), cega ao momento; é a raiz da camada (b).
5. **Quebrando a passada DEPOIS da marca, a resposta certa não é «falhou, repita» — é «confirmada, completando»**, porque a marca já é o ponto de compromisso e o próprio código diz isso.
6. **O `drop(trava)` tem de sair dos DOIS braços**: hoje só existe no `Ok` (`servidor.rs:15599`), e por isso o `travar_dados()` do `Err` (`:15611`) devolve `Err` e o `recuperar` da `:15612` nunca roda.
7. **Não copie o desfazer** (§4a): a ordem de digitação o proíbe; o nosso tudo-ou-nada é para a frente.
8. **Se nascer erro novo, o irmão medido é o `1412 ER_TABLE_DEF_CHANGED`** — dito na instrução, com «retry» no texto e **não** numa classe SQLSTATE nova (H2 morta).
9. **A prova real do 426 já está certa; falta uma assertiva:** nenhuma resposta de `COMMIT` pode trazer `repetir: true` junto com `gravadas > 0`.
10. **Nada sobe ao dono.** D1–D3 entram por aceite automático; D4 e D5 por 6 × 4 e 9 × 1; e a única pétrea tocada manda consertar.

---

### Anexo — limites declarados desta pesquisa

- **MariaDB não foi medido.** Não está instalado; as quatro respostas dela são
  KB oficial e `errmsg-utf8.txt` do fonte, citados com linha. Se alguém a
  instalar, o cenário a repetir é o **MY-2** — é o único em que MySQL e
  MariaDB poderiam divergir, e ele muda o peso de um lado inteiro do D4.
- **O `SQLITE_BUSY` no `COMMIT` não foi reproduzido**, só lido nas duas
  páginas oficiais: o CLI imprime o código primário (5) e não o estendido
  (517), e o cenário do rollback-journal com leitor pendurado pede um programa
  em C. **Raciocinado, não medido**; o que decidiria na bancada é um
  `sqlite3_step` num `COMMIT` com um leitor de outra conexão segurando.
- **Não houve fan-out.** Não há ferramenta de subagente nesta sessão, então
  `pesquisa-motor` e `pesquisa-bancada` não foram convocados; medi os três
  motores eu mesmo. Registrado como dispensa por impedimento, não por decisão.
- **Nenhum número acima foi citado de outro documento.** Os tempos (4,10 s /
  4,03 s / 3,011 s / 0,005 s / 0,049 s) saíram do relógio desta máquina hoje.
