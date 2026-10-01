# PhxMindSetSQL Web/PWA v0.7.0

Visualizador SQL multi-dialeto com núcleo arquitetural isolado e quatro projeções de alta qualidade sobre um único modelo neutro:

- **MindSet** — mapa semântico/hierárquico;
- **Relacional/DER** — tabelas, PK/FK, constraints e relações;
- **Obsidian** — knowledge graph;
- **Hybrid** — visão estrutural + contexto de objetos SQL.

## v0.7 — schema avançado + diff Arquivo × Banco Vivo

A v0.7 amplia o `UnifiedSqlModel` para **1.1** e adiciona, sem colocar parsing nos renderers:

- CHECK constraints;
- generated/computed columns com expressão completa;
- sequences;
- materialized views;
- partitioning;
- synonyms do SQL Server;
- events do MySQL/MariaDB;
- extensions do PostgreSQL;
- cardinalidade estimada e tamanho de tabela no banco ao vivo;
- diff entre schema normalizado de um arquivo SQL e schema normalizado de um banco conectado.

```text
Arquivo SQL -> ParserAdapter -----------+
                                        |
Banco vivo -> IntrospectorAdapter ------+-> UnifiedSqlModel 1.1
                                                  |
                                        +---------+----------+
                                        |                    |
                                  GraphProjection        SchemaDiff
                                        |                    |
                      +---------+-------+-------+            |
                      |         |       |       |            |
                   MindSet     DER   Obsidian Hybrid     Diff UI
```

**Regra arquitetural:** parsers e introspectores conhecem dialeto/catalogo; renderers conhecem somente o modelo/projeções normalizados. O serviço de diff compara dois `UnifiedSqlModel` e não interpreta SQL.

## Baseline PhxClaw v0.60

O arquivo incluído permanece byte a byte igual ao original e, na v0.7, o parser detecta:

- **294 tabelas**;
- **2.837 campos**;
- **254 foreign keys**;
- **669 CHECK constraints**;
- **2 generated columns** com expressão;
- **55 migrations**;
- **46 funções**;
- **58 triggers**;
- **76 índices**;
- **68 policies**;
- **1 extensão PostgreSQL** (`pgcrypto`).

SHA-256 do SQL: `0de68d3710d70ceb531d573c5b3e1306a527c26e9d384231d67a0f7bb27fee29`.

## Dialetos

| Recurso | PostgreSQL | MySQL/MariaDB | SQLite | SQL Server |
|---|---:|---:|---:|---:|
| DDL base / PK / FK | ✅ | ✅ | ✅ | ✅ |
| CHECK | ✅ | ✅ | ✅ | ✅ |
| generated/computed | ✅ | ✅ | ✅ | ✅ |
| sequence | ✅ | — | — | ✅ |
| materialized view | ✅ | — | — | — |
| partitioning | ✅ | ✅ | — | ✅ catálogo |
| synonym | — | — | — | ✅ |
| event | — | ✅ | — | — |
| extension | ✅ | — | — | — |
| introspecção viva | ✅ | ✅ | ✅ | ✅ |
| cardinalidade/tamanho | ✅ | ✅ | n/a por tabela | ✅ |

## Schema Diff

O botão **Schema Diff** é habilitado quando existem dois modelos:

1. um modelo vindo de arquivo SQL;
2. um modelo vindo de banco conectado.

O diff compara:

- tabelas adicionadas/removidas;
- colunas adicionadas/removidas/alteradas;
- generated/computed expression;
- CHECK constraints;
- partitioning;
- objetos avançados;
- relacionamentos FK.

**Cardinalidade e tamanho são ignorados pelo diff**, porque são dados operacionais e mudam sem alterar o schema.

## Banco ao vivo

O PWA não abre conexão TCP de banco diretamente. O caminho é:

```text
Browser/PWA -> HTTP/HTTPS -> phx-sql-gateway -> driver -> banco
                                      |
                                      +-> UnifiedSqlModel 1.1
```

A tela **Conectar DB** possui opções para incluir objetos avançados e cardinalidade/tamanho. O modo recomendado continua sendo profile com senha em variável de ambiente.

## Executar somente com arquivos SQL

```bash
python3 -m http.server 8080
```

Abra `http://localhost:8080`.

## Executar com Gateway / banco vivo

Linux/macOS:

```bash
cp gateway/profiles.example.json gateway/profiles.json
export PHX_DB_PASSWORD='sua-senha'
./start_gateway.sh
```

Windows PowerShell:

```powershell
Copy-Item gateway\profiles.example.json gateway\profiles.json
$env:PHX_DB_PASSWORD = "sua-senha"
.\start_gateway_windows.ps1
```

Abra `http://127.0.0.1:8787`.

## API do Gateway

```text
GET  /api/v1/health
GET  /api/v1/profiles
POST /api/v1/introspect
```

Opções relevantes v0.7:

```json
{
  "include_advanced_objects": true,
  "include_statistics": true
}
```

## Workspace Rust

```text
phx-sql-model
phx-sql-parser-api
phx-parser-common
phx-parser-postgresql
phx-parser-mysql
phx-parser-sqlite
phx-parser-sqlserver
phx-sql-contract
phx-sql-introspection-api
phx-introspector-postgresql
phx-introspector-mysql
phx-introspector-sqlite
phx-introspector-sqlserver
phx-sql-projection
phx-sql-core
phx-sql-gateway
```

## Testes

```bash
npm test
```

A suíte executável valida:

- arquitetura Parser/Introspector -> Model -> Projection -> Renderer;
- baseline PhxClaw e contagens avançadas;
- quatro dialect adapters;
- paridade de projeção entre dialetos;
- CHECK/generated/sequence/matview/partition/synonym/event/extension;
- diff Arquivo × Banco Vivo;
- contrato Live DB;
- estrutura estática do workspace Rust.

Há também testes Rust de recursos avançados em `crates/phx-sql-core/tests/advanced_features.rs`.

## Limitação da build desta sessão

O ambiente utilizado para montar esta release não possui `cargo`, `rustc` ou `wasm-pack`, nem instâncias reais dos quatro bancos. A camada Web/JS foi executada e passou integralmente em `npm test`. O código Rust, Gateway e introspectores passaram por validação estrutural/arquitetural, mas os testes Rust e os testes contra bancos reais ainda precisam ser executados em uma máquina com toolchain Rust e os SGBDs disponíveis.

Detalhes: `docs/ARCHITECTURE_V07.md`, `docs/ADVANCED_SCHEMA_V07.md`, `docs/SCHEMA_DIFF_V07.md` e `docs/VALIDATION_V07.md`.
