# FK auto-referenciada: exclusão do laço que aponta para si mesmo — 24/09/2026

Papel J. Medido ao vivo nos quatro motores (MariaDB e MySQL instalados neste
contêiner via `apt-get install`, depois removidos/revertidos para não deixar
efeito colateral — ver nota de limpeza no fim). Tabela `t(id PK, chefe_id FK
REFERENCES t(id) ON DELETE {RESTRICT|NO ACTION})`.

## Tabela motor × caso

| Motor (peso) | `id=1,chefe_id=1` **RESTRICT** | `id=1,chefe_id=1` **NO ACTION** | com filho `id=2,chefe_id=1` (controle) |
|---|---|---|---|
| PostgreSQL 16 (4) | **ACEITA** — medido | **ACEITA** — medido | **RECUSA** — medido, `ERROR: ... viola FK constraint` |
| MariaDB 10.11.14 (3) | **RECUSA** — medido, `ERROR 1451` | **RECUSA** — medido, `ERROR 1451` | **RECUSA** — medido, `ERROR 1451` |
| MySQL 8.0.46 (2) | **RECUSA** — medido, `ERROR 1451` | **RECUSA** — medido, `ERROR 1451` | **RECUSA** — medido, `ERROR 1451` |
| SQLite 3.45.1 (1) | **ACEITA** — medido | **ACEITA** — medido | **RECUSA** — medido, `FOREIGN KEY constraint failed (19)` |

Todos os quatro motores concordam no caso de controle (filho de verdade
recusa) — confirma que a FK está de fato ativa em cada um, não é motor
"desligado" dando falso aceite no auto-laço.

## RESTRICT × NO ACTION no PostgreSQL — muda o auto-laço?

Não, medido: os dois aceitam igual. A doc oficial diz a diferença entre as
cláusulas: *"The essential difference between these two choices is that
`NO ACTION` allows the check to be deferred until later in the transaction,
whereas `RESTRICT` does not."* (PostgreSQL 16 docs, §5.4.5 Foreign Keys,
https://www.postgresql.org/docs/16/ddl-constraints.html). Sem `DEFERRABLE`
explícito (não usado aqui), a diferença é só sobre *poder* adiar dentro de uma
transação com vários comandos — não sobre um único `DELETE` de uma linha só.
Como a linha `id=1` já foi fisicamente removida antes de a checagem rodar
(imediata ou "fim da instrução", que dentro de um único `DELETE` de uma linha
é o mesmo instante), a auto-referência já não existe mais para nenhuma das
duas cláusulas — daí aceitar igual.

## MariaDB — fonte do código

MariaDB usa a mesma linhagem de engine InnoDB do MySQL (fork histórico) e o
mesmo caminho de checagem: `storage/innobase/row/row0ins.cc`,
`row_ins_foreign_check_on_constraint` (~L1550) recusa a operação quando a
constraint não é `CASCADE`/`SET_NULL`, sem distinguir `RESTRICT` de
`NO ACTION` — ambas caem no mesmo `if`. Confirmado também pela KB oficial:
*"NO ACTION: Synonym for RESTRICT."* (MariaDB Knowledge Base, "Foreign Keys",
https://mariadb.com/kb/en/foreign-keys/). Fonte:
https://github.com/MariaDB/server/blob/11.4/storage/innobase/row/row0ins.cc

## Soma pela régua da casa (PG 4, MariaDB 3, MySQL 2, SQLite 1)

- **Auto-laço RESTRICT:** aceita = PG(4)+SQLite(1) = **5** × recusa =
  MariaDB(3)+MySQL(2) = **5** → **EMPATE REAL, 5×5**. A matriz não decide.
- **Auto-laço NO ACTION:** mesma divisão de motores → **EMPATE REAL, 5×5**.
- **Com filho (controle):** recusa unânime = 4+3+2+1 = **10** × aceita = 0 →
  sem dúvida, os quatro recusam quando há filho de verdade.

## Nota de limpeza (efeito colateral medido e revertido)

Instalar `mariadb-server` fez o `apt` **remover** `mysql-server-8.0` por
conflito de arquivo em `/usr/sbin/mysqld` (o processo já rodando, PID vivo
desde 23/09, não foi afetado — mantinha o binário aberto). Reinstalado
`mysql-server-8.0`/`mysql-server` na sequência; `/usr/sbin/mysqld` voltou a
ser o binário real (confirmado por `file` e por `SELECT VERSION()` = 8.0.46).
MariaDB removido depois de medir. Bases de teste (`teste_autolaco*`)
descartadas nos três motores com serviço persistente (PG, MySQL); SQLite
usou arquivo temporário no scratchpad, apagado.

## Para a mesa do dono

Isto é o caso nomeado pela pétrea local: **empate real, a matriz não decide**
(PhxSql `CLAUDE.md`, cláusula do papel J). Não decidi o comportamento do
PhxSql aqui — só entrego o medido. Quem pesa contra as pétreas (ordem de
digitação sagrada, regra primordial 1-para-muitos) é o integrador/dono.
