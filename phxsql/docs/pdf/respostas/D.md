# D) lista de comandos SQL que são do sqlite excenciais que não tem no Phxsql

## Resposta curta
Mandei **19 comandos do SQLite(R) ao motor vivo: 3 aceitos, 16 recusados** — um a
mais aceito do que a resposta anterior (07/09/2026, commit a56a165, que media
2): **o `UPSERT` (`INSERT … ON CONFLICT(id) DO NOTHING`) passou a funcionar**
na rodada de composição SQL (item 7 de `docs/propostas/comparativo-19.md`,
fechado em 08-09/2026), junto de `rowid` e `LIMIT … OFFSET`, que já passavam.
Dos 16 recusados, **6 faltam e importam** e **3 faltam e não importam aqui**; e
há **7 equivalências provadas** nesta corrida. A resposta honesta continua
começando antes da lista: o que é essencial no SQLite(R) **não é comando, é o
modelo** — e a metade que importa dele já existe aqui (`crates/phxsql-ffi`, o
motor como biblioteca com ABI de C). Continua havendo **um gap sem caminho
nenhum: não existe apagar coluna**, nem em SQL nem no protocolo.

## Exemplo exercitado

Corrida de **2026-09-22 23:43 UTC**, commit **f180e24**. **Fonte da lista:** o
índice oficial de sintaxe, https://www.sqlite.org/lang.html.

**Esta resposta substitui a de 07/09/2026** — a lei do `docs/pdf/LEIA-ME.md`:
*doc antigo é ponto de partida, nunca fonte da resposta*. A mudança de estado é
o `UPSERT`; o resto do vocabulário específico do SQLite(R) (`PRAGMA`, `ATTACH`,
`INSERT OR REPLACE/IGNORE`, funções escalares) não foi tocado pela rodada de
composição, que mirou o `SELECT`.

**Um detalhe que mudou de figura, e vale registrar:** `INSERT OR REPLACE` e
`INSERT OR IGNORE` continuam recusando, mas a razão **não é mais** "INSERT
ainda não existe" — é `esperava INTO, e veio "OR"`: o parser aceita `INSERT
INTO` (e o upsert por `ON CONFLICT`, que é a forma padrão), só não aceita a
extensão sintática `INSERT OR ...` que é exclusiva do SQLite(R).

**As três listas abaixo se cruzam, e é de propósito.** Um comando recusado
pode ao mesmo tempo ter equivalente pelo protocolo — somar as três não dá o
total de recusas, e uma soma que fechasse esconderia justamente o que
interessa: quantos gaps têm saída hoje.

### (a) Falta, e IMPORTA para um cadastro comum — 6

| comando | a recusa REAL, colada |
|---|---|
| `PRAGMA table_info(clientes)` | `SQL, coluna 1: PRAGMA nao e um comando desta camada` |
| `CREATE TABLE IF NOT EXISTS t2 (…)` | `SQL, coluna 8: CREATE nesta camada cria TRIGGER ou PROCEDURE. Tabela se cria pela operacao criar_tabela do protocolo` |
| `INSERT OR REPLACE INTO clientes (id,nome) VALUES (1,'X')` | `SQL, coluna 8: esperava INTO, e veio "OR"` |
| `INSERT OR IGNORE INTO clientes (id,nome) VALUES (1,'X')` | a mesma |
| `ALTER TABLE clientes DROP COLUMN cidade` | `SQL, coluna 1: esperava SET depois do escopo tabela, veio "DROP"` — **e não há operação equivalente** |
| `SELECT * FROM clientes ORDER BY cidade, nome` | `SQL, coluna 41: ORDER BY de mais de uma coluna precisa de um indice composto com essas colunas nessa ordem -- e quem escolhe o indice ainda e quem chama, porque nao ha planejador` |

**O `UPSERT` saiu desta lista nesta rodada** — `INSERT INTO clientes (id,nome)
VALUES (1,'X') ON CONFLICT(id) DO NOTHING` é hoje `ACEITO`:

```
[ACEITO] INSERT INTO clientes (id,nome) VALUES (1,'X') ON CONFLICT(id) DO NOTHING
  {'op': 'inserir', 'afetadas': 1, 'rowid': 1,
   'notas': ['ON CONFLICT ... DO NOTHING vira `inserir` com se_existir: "ignorar" -- a linha existente fica exatamente como estava']}
```

E as funções escalares, que no SQLite(R) são metade do trabalho de uma tela —
continuam recusando, e a razão continua sendo a mesma raiz:

```
[RECUSADO] SELECT replace(nome,'a','b') FROM clientes
           SQL, coluna 15: esperava FROM, e veio "("
[RECUSADO] SELECT ifnull(cidade,'?')    FROM clientes
           SQL, coluna 14: esperava FROM, e veio "("
[RECUSADO] SELECT datetime('now')
           SQL, coluna 16: esperava FROM, e veio "("
[RECUSADO] SELECT last_insert_rowid()
           SQL, coluna 25: esperava FROM, e veio "("
```

**As quatro são a mesma falta, e ainda é a mesma da resposta B:** não há
expressão nem função escalar na PROJEÇÃO do `SELECT` — só no `WHERE`/`HAVING`/
`ON` (item 2 da §7 de `docs/SQL.md`). A quarta tem consolo — o `inserir`
**devolve o rowid** na resposta (`{'rowid': 5, 'registros': 5}`), então quem
insere pelo protocolo já sabe o número sem perguntar. Quem escreve SQL, não.

**O `DROP COLUMN` continua sendo o único desta lista sem saída nenhuma.** As
operações do catálogo têm `acrescentar_coluna`; não têm a irmã. Numa tabela de
slot fixo apagar coluna é reescrever o `.reg` inteiro — o mesmo trabalho que o
`acrescentar_coluna` já mediu —, então o custo é conhecido; o que não existe é
a operação.

### (b) Falta, e NÃO importa aqui — 3, com o motivo

| comando | por que não é essencial neste motor |
|---|---|
| `ATTACH DATABASE '/tmp/o.db' AS o` | `ATTACH nao e um comando desta camada`. É a resposta do SQLite(R) para «um arquivo é um banco». Aqui um banco é um **diretório** e o endereçamento de três partes já atravessa bancos (`FROM banco.schema.tabela` funciona, e a permissão é conferida contra **esse** banco). O que o `ATTACH` compra, o `FROM` de três partes e o `copiar_tabela` já dão |
| `VACUUM` | `VACUUM nao e um comando desta camada`, e é **não**, não «ainda não»: compactar renumera rowid, e rowid é endereço (`offset = data_offset + (rowid−1) × slot_size`). A ordem de digitação é pétrea |
| `CREATE VIRTUAL TABLE f USING fts5(corpo)` | `CREATE nesta camada cria TRIGGER ou PROCEDURE`. **O recurso existe com outra forma** — ver (c). O que não existe é a tabela virtual como conceito, e ela não faz falta: aqui o índice de texto é um arquivo ao lado (`.fts`), declarado na criação da tabela |

### (c) Existe, com outro nome ou outra forma — 7

| SQLite(R) | o que faz a mesma coisa aqui | prova desta corrida |
|---|---|---|
| `rowid` | **passa igual** — é coluna de sistema aqui também | `[ACEITO] SELECT rowid FROM clientes` → `{'op': 'varrer', 'colunas': ['rowid'], 'devolvidas': 3, …}` |
| `LIMIT 1 OFFSET 1` | **passa igual** | `[ACEITO]` → `{'op': 'varrer', 'devolvidas': 1, …}` |
| `PRAGMA table_info(t)` | `{"op":"esquema"}` | as 6 colunas, com tipo, máscara e marca de dado pessoal |
| `SELECT name FROM sqlite_master` | `{"op":"sistabelas"}` / `{"op":"tabelas"}` — a tabela `sqlite_master` não existe aqui, a recusa nomeia isso | `nao encontrado: a tabela sqlite_master nao existe em loja`, e `{"op":"tabelas"}` devolve `{'tabelas': ['chamados', 'clientes', 'itens']}` |
| `fts5` (`MATCH`) | o `.fts`, declarado em `indices_texto` na criação, e `{"op":"procurar_texto"}` | `{'encontrados': 1, 'linhas': [{'rowid': 1, 'corpo': 'a fenix renasce das cinzas', …}]}` |
| `ALTER TABLE … RENAME TO` | `{"op":"renomear_tabela"}` (a forma SQL `ALTER TABLE … RENAME` ainda recusa: `esperava SET depois do escopo tabela, veio "RENAME"`) | `{'origem': 'fornecedores', 'destino': 'fornecedores2', 'arquivos': 8}` |
| `SAVEPOINT` / `RELEASE` / `ROLLBACK TO` | **existem, e passam pelo SQL** — a recusa desta corrida foi a **certa**, e vale colar | ver abaixo |

```
[RECUSADO] SAVEPOINT sp1              (sem transação aberta — de propósito)
  [SP000018] esquema invalido: esta conexao nao tem transacao aberta;
  comece com {"op":"begin"} (ou BEGIN / START TRANSACTION pelo SQL).
  Lembre que a transacao pertence a CONEXAO: reconectar desfaz a que estava aberta
```

Esta é a diferença entre «não tem» e «tem, e você não abriu a transação». Uma
lista de gaps que não corre o comando confunde as duas — o `BEGIN`/`SAVEPOINT`/
`ROLLBACK TO`/`COMMIT` pelo SQL estão provados na resposta B, seção (c).

### A comparação que este item pede, e que não é de comando

O que faz o SQLite(R) ser o SQLite(R) não está no `lang.html` — **não retestada
nesta corrida**, mantida da rodada anterior porque nenhuma mudança do período
tocou o modelo de arquivo:

| característica do SQLite(R) | no PhxSql |
|---|---|
| banco = **um arquivo** | banco = **um diretório**, tabela = 8 arquivos (`.reg`, `.ndx`, `.bin`, `.memo`, `.log`, `.trash`, `.reason`, `.pag`) |
| roda **dentro do processo**, sem servidor | **existe**: `crates/phxsql-ffi`, o motor como biblioteca com ABI de C (`docs/EMBUTIDO.md`), pensado para o aparelho off-line que sincroniza depois |
| **tipagem dinâmica** (qualquer valor em qualquer coluna) | **não, e é decisão**: o slot é fixo e o tipo é conferido na gravação — `esperado numero da sequencia, recebido Texto("2")` é essa conferência falando |
| zero configuração | há `config.json`, porque há servidor, usuários, replicação e cifra |
| `ON CONFLICT` / UPSERT como cláusula | **existe pelo SQL desde 08-09/2026** (ver acima) |

**Onde os dois se encontram** é o modo embutido, e é ali que uma lista de gaps
de SQLite(R) vira plano de trabalho de verdade: um aplicativo que hoje usa
SQLite(R) no aparelho troca o arquivo pelo `phxsql-ffi` e ganha a sincronia com
o servidor central — mas escreve pelo **punho** da biblioteca, não por SQL.

## O que NÃO existe, e é dispensa registrada

- **Não existe apagar coluna** — nem `ALTER TABLE DROP COLUMN`, nem operação de
  protocolo. É o único item desta resposta sem saída nenhuma, e não achei
  decisão escrita recusando-o: é falta, não escolha.
- **Não existe `VACUUM`, e não vai existir** enquanto rowid for endereço.
  Dispensa com motivo, não pendência.
- **Não existe tipagem dinâmica**, e a conferência de tipo na gravação é o que
  a impede. Quem espera o comportamento do SQLite(R) («guardo texto numa coluna
  inteira e depois vejo») vai bater na conferência, e é o desenho.
- **`ON CONFLICT ... DO NOTHING` mudou de estado nesta rodada** e saiu da
  lista de faltas — é o único item cuja categorização não é herdada da
  resposta anterior.
- **Não exercitei o `phxsql-ffi`** nesta rodada. Ele existe e está documentado;
  toda esta resposta é sobre o texto SQL que chega pela op `sql`, e dizer que o
  modo embutido «funciona» sem tê-lo corrido seria número citado.
- **Não medi contra um SQLite(R) real.** A coluna dele desta resposta sai do
  manual citado; a coluna do PhxSql sai da saída colada.

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py sqlite
```
