# PhxSql — leitura do contexto e plano

Documento de trabalho. Registra o que foi lido, o que o material instrui a
fazer, e as decisões que ainda dependem do Adriano.

---

## 1. rusqlite: o que ele é de fato

Clonado de `github.com/rusqlite/rusqlite` (v0.40.1, edition 2024,
rust-version 1.88.0) e medido:

| Medida | Valor |
|---|---|
| Linhas de Rust em `src/` | 22.131 |
| Símbolos FFI distintos `ffi::sqlite3_*` chamados | 170 |
| `libsqlite3-sys/sqlite3/sqlite3.c` | **269.649 linhas de C** |
| `Cargo.toml` → `keywords` | `["sqlite", "database", "ffi"]` |
| `Cargo.toml` → `description` | "Ergonomic wrapper for SQLite" |

**Conclusão: rusqlite não é um motor de banco em Rust.** É uma casca
ergonômica sobre a biblioteca C do SQLite. Parser SQL, planejador de consulta,
B-tree, pager, WAL, journal — nada disso está em Rust; está nas 270 mil linhas
de `sqlite3.c` que o `libsqlite3-sys` compila junto.

Isso tem uma consequência direta: **não existe "transformar o rusqlite em
PhxSql"**. Se removermos o `libsqlite3-sys`, sobra um wrapper sem motor
embaixo. Não há armazenamento para reformular — o `.reg`/`.ndx`/`.bin`/`.memo`
teria de ser escrito do zero de qualquer maneira (que é o que já está feito no
diretório `phxsql/`).

### O que o rusqlite serve — e serve muito

`src/vtab/` expõe a API de **tabelas virtuais** do SQLite:

```rust
pub unsafe trait VTab<'vtab>        // best_index: onde entrego os índices ao planejador
pub trait CreateVTab<'vtab>
pub trait UpdateVTab<'vtab>         // delete / insert / update
pub trait TransactionVTab<'vtab>    // begin / sync / commit / rollback
pub unsafe trait VTabCursor         // filter / next / eof / column / rowid
```

Implementando esses traits sobre o PhxSql, o SQLite passa a fazer parsing,
planejamento, JOIN, GROUP BY e agregações — e **o armazenamento é o PhxSql**.
O método `best_index` é exatamente o ponto onde o `.ndx` é oferecido ao
planejador, que então usa os nossos índices em vez de varrer a tabela.

O próprio repositório traz exemplos prontos: `src/vtab/csvtab.rs` (leitura),
`src/vtab/vtablog.rs` e `src/vtab/series.rs`.

**Estratégia recomendada, em duas fases:**

- **Fase A — SQL emprestado.** PhxSql como módulo de tabela virtual do SQLite,
  via rusqlite. Ganhamos SQL completo em dias, não em meses, sem escrever
  parser nem planejador. Custo: uma dependência em C.
- **Fase B — SQL próprio.** Parser e executor em Rust puro sobre o mesmo
  armazenamento, mantendo a Fase A como oráculo de teste diferencial: a mesma
  consulta nos dois caminhos tem de dar o mesmo resultado.

A Fase A não é desperdício — ela vira a suíte de testes da Fase B.

---

## 2. FraseSQL: o contrato de integração

Lido o pacote `FraseSQL_5.zip` (v1.2, 3.032 linhas de Rust, 35 arquivos).

É um **gateway TCP que traduz frase em português para SQL** e executa em
qualquer um de 14 motores. Arquitetura: `token + frase` → plano (Grok, com
planejador local de reserva) → SQL no dialeto do banco → JSON.

### Como um banco novo entra

O ponto de extensão é claro e pequeno:

| Arquivo | O que acrescentar |
|---|---|
| `config.rs` | variante `Phxsql` em `enum Engine` e em `enum Family` |
| `config.rs` | `Engine::family()`, `label()`, `is_sql()`, `uses_odbc()` |
| `engine.rs` | `connect_phxsql(cfg)`, variante `Kind::Phxsql`, braço em `Live::execute` |
| `engine.rs` | montar o `Catalog` a partir do esquema PhxSql |
| `dialect.rs` | braço em `wrap_limit` e `quote_ident` |
| `catalog_sql.rs` | `dialect_name` para o prompt da IA |

A interface de execução é uma só:

```rust
pub async fn execute(&self, command: &str, max_rows: i64) -> Result<Vec<Value>>
```

E o catálogo que o FraseSQL espera:

```rust
Catalog { tables: Vec<Table> }
Table  { name, kind, comment, columns: Vec<Column>, foreign_keys: Vec<ForeignKey> }
Column { name, coltype, nullable, is_pk, is_fk, comment }
```

O `Schema` do PhxSql já tem nome, tipo, nulabilidade e índices. **Falta chave
estrangeira** — o `Schema` ainda não modela relacionamento. Para o FraseSQL
gerar JOIN corretamente, precisamos de FK no esquema (que é também o que o
dicionário do Clarion chama de RELATION, com CASCADE/RESTRICT).

### Regras que o FraseSQL impõe e o PhxSql precisa honrar

- **Comandos aceitos:** SELECT, INSERT, UPDATE e `WITH ... SELECT`.
  Recusados: DELETE, DROP, ALTER, TRUNCATE, CREATE, GRANT.
- **Protocolo:** JSON Lines — uma linha JSON por mensagem, UTF-8, terminada
  em `\n`. Ops: `ping`, `catalog`, `connections`, `explain`, `query`.
- **Segurança:** token comparado em tempo constante, blacklist de SQL, de
  frases, de regex e de tabelas; allowlist opcional; rate limit por IP+token;
  auditoria em JSONL; métricas Prometheus.
- **Portas:** FraseSQL escuta 9090, métricas em 9100. PhxSql em 5000 — sem
  colisão.
- **Configuração:** o FraseSQL usa `config.toml`. O PhxSql usará
  `config.json`, conforme pedido. São dois processos distintos; não conflita.

### Sobre ODBC no FraseSQL

O FraseSQL já resolve ODBC com a crate `odbc-api = "8"` atrás da feature
`odbc`, e usa esse caminho para Oracle, DB2, AS400, Informix, Sybase, Teradata
e Caché/IRIS. A documentação de instalação (`FraseSQL-ORACLE-ODBC.txt` e a
pasta `odbc/`) é o passo a passo de Instant Client, DSN, `odbc.ini`,
`odbcinst.ini` e `tnsnames.ora`. **Não precisamos inventar essa camada** —
precisamos reaproveitá-la.

---

## 3. Requisitos desta rodada

Consolidados do pedido:

| # | Requisito | Situação |
|---|---|---|
| 1 | Servidor MCP para o PhxSql | a fazer |
| 2 | Integração com outros bancos via ODBC | a fazer (reaproveitar `odbc-api`) |
| 3 | Integração via OLE DB | **ver questão aberta 1** |
| 4 | Porta 5000, configurável em `config.json` | a fazer |
| 5 | Tudo em Rust | a decidir na Fase A (ver questão aberta 2) |
| 6 | `Tabela.log` — data e hora de toda inclusão, alteração e exclusão | a fazer |
| 7 | Reindex: recriar o `.ndx` do zero | a fazer |
| 8 | Linha de comando | parcial — CLI existe, falta cobrir o resto |
| 9 | Pastas para separar tabelas | a fazer |
| 10 | Separar bancos de dados | a fazer |
| 11 | Hierarquia database → schema → tabela | a fazer |

### 3.1 Hierarquia pedida

```
Database Z/
├── <tabelas da raiz>
├── X/                  schema X
│   └── <tabelas do schema X>
└── Y/                  schema Y
    └── <tabelas do schema Y>
```

Mapeamento direto para o disco: um diretório por database, um subdiretório por
schema, e as quatro peças de cada tabela dentro do diretório correspondente.
Tabelas na raiz do database ficam sem schema (equivalente ao `public` do
Postgres ou ao `dbo` do SQL Server).

Nome qualificado: `database.schema.tabela`, com `database.tabela` para a raiz.
Isso casa com o `Catalog::has_table` do FraseSQL, que já trata nome curto e
nome qualificado por ponto.

### 3.2 `Tabela.log` — o quinto arquivo

A tabela passa de quatro para **cinco** arquivos:

```
cadastroClientes.reg + .ndx + .bin + .memo + .log = cadastroClientes
```

Registro de log proposto (largura fixa, mesmo espírito do `.reg`):

| Campo | Bytes | Conteúdo |
|---|---|---|
| carimbo | 8 | milissegundos desde a época Unix |
| operação | 1 | 1=inclusão, 2=alteração, 3=exclusão |
| flags | 1 | reservado |
| reservado | 2 | |
| rowid | 8 | registro afetado |
| versão | 8 | versão do registro depois da operação |
| usuário | 4 | id do usuário/sessão (0 = não informado) |
| crc32 | 4 | do próprio registro de log |

36 bytes por evento, append-only, sem índice. Fica legível por `phxsql log` e
serve tanto de auditoria quanto de base para replicação futura.

**Questão de projeto:** o log guarda só o *evento* (barato, 36 bytes) ou também
o *conteúdo anterior* do registro, o que permitiria desfazer? A segunda opção
transforma o `.log` em journal de verdade e abre caminho para transações — mas
custa o tamanho do registro a cada alteração.

### 3.3 Reindex

Recriar o `.ndx` inteiro a partir do `.reg`: varre os registros ativos na
ordem de digitação, recodifica as chaves e reconstrói cada B+tree do zero.
Resolve três coisas de uma vez: `.ndx` corrompido ou apagado, árvore
subocupada depois de muitas exclusões (o `remover` atual não rebalanceia), e
acréscimo de um índice novo a uma tabela que já tem dados.

---

## 4. Questões abertas

### 1. OLE DB

OLE DB é uma API COM, só Windows. A Microsoft a declarou obsoleta em 2011;
voltou atrás apenas para o driver do SQL Server (MSOLEDBSQL). **Não existe
crate Rust de OLE DB**, e implementar um consumidor COM em `windows-rs` é
trabalho considerável para cobrir o que o ODBC já cobre.

Caminhos:

- **(a) Só ODBC.** Cobre Oracle, DB2, AS400, Informix, Sybase, Teradata,
  Caché, SQL Server, Access e mais. É o que o FraseSQL já faz.
- **(b) ODBC agora, OLE DB depois**, se aparecer uma fonte que só tenha
  provider OLE DB.
- **(c) ODBC + OLE DB desde já**, aceitando o custo e a restrição a Windows.

Recomendação: **(b)**. Pergunta: algum cliente seu tem alguma fonte que
**só** fala OLE DB, sem driver ODBC?

### 2. Direção da integração

"Integração com outros bancos via ODBC" pode significar duas coisas opostas, e
são trabalhos bem diferentes:

- **PhxSql como cliente:** o PhxSql lê Oracle, SQL Server, DB2 via ODBC.
  Permite consulta federada e migração de dados legados para o formato PhxSql.
- **PhxSql como servidor:** escrever um *driver ODBC do PhxSql*, para que
  Excel, Power BI, Crystal Reports e sistemas Clarion enxerguem o PhxSql como
  uma fonte de dados. Isso exige uma biblioteca C ABI que implemente a API ODBC
  (`SQLDriverConnect`, `SQLExecDirect`, `SQLFetch`...), registrada no
  `odbcinst.ini` / registro do Windows.

O segundo é o que faria o PhxSql substituir o TopSpeed nos aplicativos Clarion
existentes. Qual dos dois vem primeiro? Ou os dois?

### 3. Rust puro versus SQLite emprestado

O requisito diz "tudo em Rust". A Fase A (tabela virtual via rusqlite) traz
`sqlite3.c` junto — 270 mil linhas de C. Ou:

- **(a) Fase A com rusqlite**, SQL completo rápido, e Fase B substitui depois.
- **(b) Direto para o Rust puro**, sem C em momento algum, aceitando que SQL
  completo demora muito mais.

Recomendação: **(a)**, com a dependência atrás de uma *feature* do Cargo, para
que quem quiser possa compilar o PhxSql sem uma linha de C.

---

## 5. Ordem de trabalho sugerida

1. FK no `Schema` (o FraseSQL precisa para gerar JOIN, e o dicionário Clarion
   tem RELATION com CASCADE/RESTRICT)
2. Hierarquia database/schema/tabela em disco
3. `Tabela.log` e o comando `phxsql log`
4. Reindex e compactação
5. `config.json` e o servidor TCP na porta 5000
6. Servidor MCP
7. Camada SQL — Fase A ou B, conforme a decisão
8. ODBC — cliente ou driver, conforme a decisão
9. Integração no FraseSQL como `engine = "phxsql"`
