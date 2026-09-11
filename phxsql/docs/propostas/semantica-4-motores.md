# Semântica de FK-em-transação e de consultas SQL — os quatro motores, medidos na doc

Pesquisa do papel J (11/09/2026), pedida pelo dono depois de achar dois
problemas concretos no PhxSql: (1) a transação que cria pai e filho juntos e
(2) consultas SQL. Cada afirmação abaixo saiu da **documentação oficial**, com
URL. A decisão de cada ponto segue a régua que o dono deu:

- **Convergência dos três maduros** (PostgreSQL + MariaDB + MySQL no mesmo
  comportamento) = **verdade absoluta, aceite automático** (lei do `CLAUDE.md`).
- **Onde divergem**, decide a **média ponderada**: PostgreSQL **4**, MariaDB
  **3**, MySQL **2**, SQLite **1**.
- **Dois invariantes do dono ficam ACIMA do voto** (pétrea): «só existe filho
  se o pai existir primeiro» e «impossível o filho ter a mesma data do pai».

## 1. O cenário central — `BEGIN` → INSERT pai → INSERT filho (FK) → `COMMIT`

| motor | sucede? | como confere a FK | `DEFERRABLE`? |
|---|---|---|---|
| **PostgreSQL** (peso 4) | **sim** | imediata, no fim de cada statement; enxerga o próprio insert não commitado | sim (padrão `NOT DEFERRABLE`/`INITIALLY IMMEDIATE`) |
| **MariaDB** (peso 3) | **sim** | imediata, linha a linha (erro 1452 se órfã) | não (InnoDB) |
| **MySQL/InnoDB** (peso 2) | **sim** | imediata, linha a linha | não (só o NDB adia) |
| **SQLite** (peso 1) | sim **se** FK ligada; imediata quando ligada | imediata por padrão; suporta `DEFERRABLE INITIALLY DEFERRED` | sim, mas FK vem **desligada** por padrão |

**Veredito de convergência:** PostgreSQL + MariaDB + MySQL **convergem** — pai
antes do filho na mesma transação **sucede**, e os três conferem a FK
**imediatamente** (por statement/linha), enxergando o pai recém-inserido e
ainda não commitado da própria transação. **Aceite automático.**

Fontes: PostgreSQL [SET CONSTRAINTS](https://www.postgresql.org/docs/current/sql-set-constraints.html),
[CREATE TABLE](https://www.postgresql.org/docs/current/sql-createtable.html);
MySQL [FOREIGN KEY](https://dev.mysql.com/doc/refman/8.0/en/create-table-foreign-keys.html);
MariaDB [Foreign Keys](https://mariadb.com/kb/en/foreign-keys/);
SQLite [Foreign Keys](https://www.sqlite.org/foreignkeys.html).

### O que isto decide para o PhxSql

1. **Pai→filho na mesma transação TEM de suceder** (convergência + invariante do
   dono). Hoje **falha**: `Table::conferir_fks` abre a mãe num segundo descritor
   e lê o `.ndx` do disco, sem enxergar o pai que a transação empilhou — o
   read-your-own-writes existe na leitura (pedido 162) e falta na **conferência
   de constraint**. É o defeito grave. Conserto: a pergunta «existe este pai?»
   consulta o conjunto de escrita pendente (a mesma `Sobreposicao` das leituras).
2. **`DEFERRABLE` / filho-antes-do-pai NÃO entra.** Diverge — só o PostgreSQL
   (peso 4) tem entre os maduros; MySQL/MariaDB não. Sem trio, e o invariante
   «só existe filho se o pai existir primeiro» proíbe. Voto e pétrea dizem o
   mesmo: **não**.
3. **Impossível o filho ter a mesma data do pai.** Invariante do dono
   (11/09/2026), e não vem dos motores — é causalidade que o PhxSql torna
   provável no dado: nasce uma **coluna de data/hora de sistema por linha**
   (mudança de formato, PSCH novo), e no commit o pai é carimbado com instante
   **estritamente anterior** ao do filho. Commitar pai e filho juntos nunca dá o
   mesmo instante aos dois.

## 2. Consultas SQL — onde os motores divergem, decidido pela média ponderada

| tópico | PG(4) | MariaDB(3) | MySQL(2) | SQLite(1) | soma | decisão do PhxSql |
|---|---|---|---|---|---|---|
| `LEFT JOIN` sem linha à direita → colunas NULL | NULL | NULL | NULL | NULL | convergem | **NULL** (padrão, aceite automático) |
| `GROUP BY` com coluna não agregada fora do `GROUP BY` | erra | aceita | erra (`only_full_group_by`) | aceita (linha arbitrária) | errar **6** × aceitar **4** | **ERRAR** |
| subconsulta escalar devolve > 1 linha | erra | erra | erra | usa a 1ª | errar **9** × 1 | **ERRAR** |
| `[NOT] EXISTS` correlacionado por apelido de fora | roda | roda | roda | roda | convergem | **RODAR** (é o pedido #240, hoje recusa — bug) |

Fontes: PostgreSQL [table expressions](https://www.postgresql.org/docs/current/queries-table-expressions.html),
[value expressions](https://www.postgresql.org/docs/current/sql-expressions.html);
MySQL [GROUP BY handling](https://dev.mysql.com/doc/refman/8.0/en/group-by-handling.html),
[subquery errors](https://dev.mysql.com/doc/refman/8.0/en/subquery-errors.html),
[EXISTS](https://dev.mysql.com/doc/refman/8.0/en/exists-and-not-exists-subqueries.html);
MariaDB [sql_mode](https://mariadb.com/kb/en/sql-mode/); SQLite
[quirks](https://www.sqlite.org/quirks.html), [expressions](https://www.sqlite.org/lang_expr.html).

Nota: MySQL e MariaDB, mesmo primos, **divergem no padrão de fábrica** do
`GROUP BY` (o MySQL liga `only_full_group_by`, o MariaDB não). É o caso que
mostra por que a régua ponderada existe: não há «verdade absoluta» ali, e o
voto (PG+MySQL contra MariaDB+SQLite) decide por **errar**.

## 3. Lacunas honestas da pesquisa (não achei frase única na doc)

- O read-your-own-writes explícito para a FK imediata do MySQL/MariaDB **não**
  tem frase dedicada; decorre de «checagem imediata contra o pai» + visibilidade
  das próprias escritas da sessão. Só o PostgreSQL ancora o *quando* («fim de
  cada statement»).
- `DEFERRABLE` no InnoDB: não achei suporte documentado — tratar como não tem.

## 4. O que ainda depende do dono

- Os **específicos do formato** da coluna de data/hora de sistema (tipo,
  codificação zero-dependência, posição entre as colunas de sistema, como o
  commit garante pai < filho) — é decisão de DBA e de formato, e formato se
  decide com o dono antes de gravar.
- O **caso concreto** da consulta SQL que o dono viu falhar: se é um dos quatro
  acima (em especial o `#240`), ou um novo, para a caça mirar o bug certo.

## 5. Medição de 11/09/2026 — três dos quatro já estão certos

Ao voltar a esta lista, medi o código em vez de supor. Três das quatro
decisões da §2 **já estão implementadas e corretas** — a hipótese de que eram
bugs morreu medida, que é resultado tão válido quanto ganho:

- **`GROUP BY` com coluna não agregada → ERRAR:** já recusa, nomeando, em
  `crates/phxsql-sql/src/sintaxe.rs:209` (`finalizar_projecao`), case-insensitive
  pelo `igual_sem_caso`; e `SELECT *` com `GROUP BY` também recusa (linha 159).
- **subconsulta escalar > 1 linha → ERRAR:** já recusa em
  `crates/phxsql-server/src/servidor.rs:10845` (`if dentro.len() != 1`), com a
  razão certa escrita ao lado («escolher a primeira faria a resposta depender da
  ordem»).
- **`= NULL` não diverge por caminho** (a suspeita que trouxe a caça aqui): o
  filtro simples guarda `_ if v.e_null() || f.valor.e_null() => false` em
  `crates/phxsql-store/src/memoria.rs:651` (nenhuma comparação casa com nulo, nem
  a `Igual`), e o avaliador de expressão usa lógica de três valores (`NULL`
  exclui no filtro). Os dois caminhos **concordam** — não há bug.

O `LEFT JOIN` NULL já convergia (aceite automático). Sobra **um** item SQL
decidido e não construído: o **`#240` — `[NOT] EXISTS` correlacionado**, que a
régua manda RODAR e que hoje **recusa nomeando** — é recurso faltando, não
resposta errada. Construí-lo roda a subconsulta por linha da consulta de fora
(N passagens pelo portão de permissão), e por isso é decisão de escopo do dono,
não conserto automático. **A caça segue precisando da consulta concreta que o
dono viu falhar** — se for o `#240`, o alvo está nomeado; se for outra, ela
ainda não está nesta lista.
