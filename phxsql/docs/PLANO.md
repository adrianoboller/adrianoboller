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
| `Cargo.toml` → `description` | "Ergonomic wrapper for SQLite" (citacao literal do repositorio) |

**Conclusão: rusqlite não é um motor de banco em Rust.** É uma casca
ergonômica sobre a biblioteca C do SQLite(R). Parser SQL, planejador de consulta,
B-tree, pager, WAL, journal — nada disso está em Rust; está nas 270 mil linhas
de `sqlite3.c` que o `libsqlite3-sys` compila junto.

Isso tem uma consequência direta: **não existe "transformar o rusqlite em
PhxSql"**. Se removermos o `libsqlite3-sys`, sobra um wrapper sem motor
embaixo. Não há armazenamento para reformular — o `.reg`/`.ndx`/`.bin`/`.memo`
teria de ser escrito do zero de qualquer maneira (que é o que já está feito no
diretório `phxsql/`).

### O que o rusqlite serve — e serve muito

`src/vtab/` expõe a API de **tabelas virtuais** do SQLite(R):

```rust
pub unsafe trait VTab<'vtab>        // best_index: onde entrego os índices ao planejador
pub trait CreateVTab<'vtab>
pub trait UpdateVTab<'vtab>         // delete / insert / update
pub trait TransactionVTab<'vtab>    // begin / sync / commit / rollback
pub unsafe trait VTabCursor         // filter / next / eof / column / rowid
```

Implementando esses traits sobre o PhxSql, o SQLite(R) passa a fazer parsing,
planejamento, JOIN, GROUP BY e agregações — e **o armazenamento é o PhxSql**.
O método `best_index` é exatamente o ponto onde o `.ndx` é oferecido ao
planejador, que então usa os nossos índices em vez de varrer a tabela.

O próprio repositório traz exemplos prontos: `src/vtab/csvtab.rs` (leitura),
`src/vtab/vtablog.rs` e `src/vtab/series.rs`.

**Estratégia recomendada, em duas fases:**

- **Fase A — SQL emprestado.** PhxSql como módulo de tabela virtual do SQLite(R),
  via rusqlite. Ganhamos SQL completo em dias, não em meses, sem escrever
  parser nem planejador. Custo: uma dependência em C.
- **Fase B — SQL próprio.** Parser e executor em Rust puro sobre o mesmo
  armazenamento, mantendo a Fase A como oráculo de teste diferencial: a mesma
  consulta nos dois caminhos tem de dar o mesmo resultado.

A Fase A não é desperdício — ela vira a suíte de testes da Fase B.

---

## 2. FraseSQL: o contrato de integração

Lido o pacote `FraseSQL_5.zip` (v1.2, 3.032 linhas de Rust, 35 arquivos).

> **Nota sobre o anexo.** O pacote foi enviado duas vezes, a segunda anunciada
> como "versão 2.0 evolução". Os dois arquivos são byte a byte idênticos
> (MD5 `ebd0a95c6a5f780c688986cd52e5a089`), e ambos declaram `version = "1.2.0"`
> no `Cargo.toml` e "v1.2" no `MANUAL.txt`. A 2.0 não chegou. Tudo abaixo se
> refere à 1.2.

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
dicionário do Clarion(R) chama de RELATION, com CASCADE/RESTRICT).

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
`odbc`, e usa esse caminho para Oracle(R), DB2(R), AS400(R), Informix(R), Sybase(R), Teradata(R)
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
| 6 | `Tabela.log` — data e hora de toda inclusão, alteração e exclusão | **pronto** |
| 7 | Reindex: recriar o `.ndx` do zero | **pronto** |
| 8 | Linha de comando | **pronto** |
| 9 | Pastas para separar tabelas | **pronto** |
| 10 | Separar bancos de dados | **pronto** |
| 11 | Hierarquia database → schema → tabela | **pronto** |
| 12 | Paginação de tabelas grandes (`_001`, `_002`, ...) | **pronto** |
| 13 | Quantidade de registros e de arquivos definida no `CREATE TABLE` | **pronto** |
| 14 | Chave estrangeira no esquema (exigida pelo catálogo do FraseSQL) | **pronto** |

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

### 3.3 Paginação de tabelas grandes

Definida no `CREATE TABLE`, com dois parâmetros:

| Parâmetro | Significado |
|---|---|
| `registros_por_arquivo` | quantos registros cabem em cada volume |
| `max_arquivos` | quantos volumes a tabela pode ter |
| `digitos` | largura do sufixo, padrão 3 (`_001`) |

Capacidade da tabela = `registros_por_arquivo x max_arquivos`.

```
cadastroClientes_001.reg
cadastroClientes_002.reg
cadastroClientes_003.reg
```

**O endereçamento continua sendo uma conta, não uma busca** — que é a
propriedade que faz o `.reg` valer a pena:

```
volume = (rowid - 1) / registros_por_arquivo + 1
slot   = (rowid - 1) % registros_por_arquivo + 1
offset = data_offset + (slot - 1) * slot_size
```

Três garantias sobrevivem intactas:

- **Ordem de digitação:** o volume N+1 vem sempre depois do N, e dentro de
  cada volume os slots continuam em ordem de inserção.
- **O rowid é global e nunca muda.** Ele não é "posição no volume", é posição
  na tabela; o volume sai dele por divisão.
- **O `.ndx` não muda em nada.** Ele já guarda rowid, e o rowid continua
  global. Nenhuma linha do código de índice precisa saber que existe volume.

#### O que pagina e o que não pagina

| Arquivo | Pagina? | Motivo |
|---|---|---|
| `.reg` | sim | cresce por quantidade de registros |
| `.bin` | sim | é o que mais cresce em bytes (fotos, anexos) |
| `.memo` | sim | idem |
| `.log` | sim | append-only, cresce para sempre |
| `.ndx` | **não** | é uma B+tree por índice sobre a tabela inteira; partir o arquivo partiria a árvore |

#### Consequência no ponteiro externo

O ponteiro gravado no `.reg` precisa passar a dizer em qual volume do `.bin` /
`.memo` o conteúdo está. Ele continua com 16 bytes, redistribuídos:

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 6 | offset dentro do volume (u48 — 256 TB por volume) |
| 6 | 2 | número do volume (u16 — 65.535 volumes) |
| 8 | 4 | tamanho do conteúdo |
| 12 | 4 | CRC-32 do conteúdo |

É mudança de formato. Como estamos na versão 1 e não há nada em produção,
entra agora, em vez de virar uma migração depois.

#### Abertura preguiçosa

Uma tabela de 999 volumes não pode abrir 999 descritores de arquivo. Abre-se o
volume `_001` (que traz o esquema) e os demais sob demanda, com um cache LRU de
descritores. É exatamente o *lazy open* do `FileManager` do Clarion(R).

Cada volume carrega o cabeçalho completo, com o seu número, o
`registros_por_arquivo` e o total de volumes — se o `_001` se perder, os outros
ainda sabem dizer o que são.

#### Tabela cheia

`rowid > registros_por_arquivo x max_arquivos` devolve erro explícito
"tabela cheia", em vez do estouro silencioso de 2 GB que o TopSpeed(R) dava.

### 3.4 Reindex

Recriar o `.ndx` inteiro a partir do `.reg`: varre os registros ativos na
ordem de digitação, recodifica as chaves e reconstrói cada B+tree do zero.
Resolve três coisas de uma vez: `.ndx` corrompido ou apagado, árvore
subocupada depois de muitas exclusões (o `remover` atual não rebalanceia), e
acréscimo de um índice novo a uma tabela que já tem dados.

---

## 4. Decisões tomadas

As três questões abaixo foram decididas pelo autor:

| Questão | Decisão |
|---|---|
| OLE DB | **ODBC + OLE DB desde já** — aceitando o custo e a restrição a Windows para o OLE DB |
| Direção do ODBC | **Os dois, driver primeiro** — primeiro o driver ODBC do PhxSql (saída), para Excel, Power BI, Crystal e os apps Clarion(R); depois o cliente (entrada) |
| Camada SQL | **rusqlite atrás de uma *feature* do Cargo** — SQL completo rápido pela tabela virtual, com a dependência de C opcional; depois vira oráculo de teste do parser próprio |

O texto original de cada questão fica abaixo, como registro do raciocínio.

## 4.1 Questões (já respondidas)

### 1. OLE DB

OLE DB é uma API COM, só Windows. A Microsoft a declarou obsoleta em 2011;
voltou atrás apenas para o driver do SQL Server (MSOLEDBSQL). **Não existe
crate Rust de OLE DB**, e implementar um consumidor COM em `windows-rs` é
trabalho considerável para cobrir o que o ODBC já cobre.

Caminhos:

- **(a) Só ODBC.** Cobre Oracle(R), DB2(R), AS400(R), Informix(R), Sybase(R), Teradata(R),
  Caché, SQL Server, Access(R) e mais. É o que o FraseSQL já faz.
- **(b) ODBC agora, OLE DB depois**, se aparecer uma fonte que só tenha
  provider OLE DB.
- **(c) ODBC + OLE DB desde já**, aceitando o custo e a restrição a Windows.

Recomendação: **(b)**. Pergunta: algum cliente seu tem alguma fonte que
**só** fala OLE DB, sem driver ODBC?

### 2. Direção da integração

"Integração com outros bancos via ODBC" pode significar duas coisas opostas, e
são trabalhos bem diferentes:

- **PhxSql como cliente:** o PhxSql lê Oracle(R), SQL Server, DB2(R) via ODBC.
  Permite consulta federada e migração de dados legados para o formato PhxSql.
- **PhxSql como servidor:** escrever um *driver ODBC do PhxSql*, para que
  Excel, Power BI, Crystal Reports e sistemas Clarion(R) enxerguem o PhxSql como
  uma fonte de dados. Isso exige uma biblioteca C ABI que implemente a API ODBC
  (`SQLDriverConnect`, `SQLExecDirect`, `SQLFetch`...), registrada no
  `odbcinst.ini` / registro do Windows.

O segundo é o que faria o PhxSql substituir o TopSpeed(R) nos aplicativos Clarion(R)
existentes. Qual dos dois vem primeiro? Ou os dois?

### 3. Rust puro versus SQLite(R) emprestado

O requisito diz "tudo em Rust". A Fase A (tabela virtual via rusqlite) traz
`sqlite3.c` junto — 270 mil linhas de C. Ou:

- **(a) Fase A com rusqlite**, SQL completo rápido, e Fase B substitui depois.
- **(b) Direto para o Rust puro**, sem C em momento algum, aceitando que SQL
  completo demora muito mais.

Recomendação: **(a)**, com a dependência atrás de uma *feature* do Cargo, para
que quem quiser possa compilar o PhxSql sem uma linha de C.

---

## 5. Ordem de trabalho sugerida

**Fundação — concluída:**

1. ~~Paginação do `.reg`/`.bin`/`.memo`/`.log` e o novo formato do ponteiro~~
2. ~~FK no `Schema`~~
3. ~~Hierarquia database/schema/tabela em disco~~
4. ~~`Tabela.log` e o comando `phxsql log`~~
5. ~~Reindex~~

**A fazer, nesta ordem:**

6. `config.json` e o servidor TCP na porta 5000
7. Servidor MCP
8. Camada SQL — tabela virtual do SQLite(R) via rusqlite, atrás de uma *feature*
9. Driver ODBC do PhxSql (saída), depois cliente ODBC e OLE DB (entrada)
10. Integração no FraseSQL como `engine = "phxsql"`
11. Compactação, transações, concorrência
