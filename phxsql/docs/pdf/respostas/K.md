# K) exemplo de connection com acesso nativo, odbc e oledb

> Corrida em 2026-09-07T16:33:40Z UTC · commit `a56a165` · binários de
> `target/release/` (`phxsqld`, `phxsqlcmd`, `libphxsql_odbc.so`,
> `libphxsql_ffi.{so,a}`) · reproduzido por `python3 bancada/conexoes/nativo.py`
> e `python3 bancada/odbc/provar.py --porta 6402`

## Resposta curta

**Nativo** existe e são três formas de login, da mais forte para a mais
fraca — desafio-resposta (a senha nunca sai da máquina do cliente), Base64
(esconde do olho, não da rede) e texto puro — exercitadas abaixo com um
soquete cru em Python, sem nenhuma biblioteca do PhxSql. **ODBC** existe de
verdade: um driver de ABI C (`crates/phxsql-odbc/`) carregado por `dlopen`,
provado nesta rodada com **73 conferências, zero falhas**. **OLEDB não
existe** e é decisão documentada (`docs/ODBC.md` §6): o caminho no Windows é
o ODBC acima envolvido pelo `MSDASQL` da própria Microsoft. Como "connection"
pode ser qualquer forma de acesso para o dono, o REST (porta HTTP) e o
embutido/FFI (biblioteca ligada no processo) também estão exercitados, mais
curtos.

## Exemplo exercitado

### 1. Conexão NATIVA — soquete cru + JSON Lines, as três formas de login

Servidor próprio na porta 6400. Login por **desafio-resposta**, com PBKDF2 e
HMAC calculados em Python puro (`hashlib`/`hmac` da biblioteca padrão, sem
nada do PhxSql), exatamente a conta do `docs/SEGURANCA.md` §2:

```
-> {"op": "desafio", "usuario": "adriano", "token": "token-da-porta"}
<- {"ok":true,"op":"desafio","resultado":{"sal":"a88ae7db8e6191bdb3e77616c336d9f1","iteracoes":210000,"nonce":"524846585a034cf6988e877b4629cbc0","validade_ms":60000},"ms":0}
-> {"op": "login", "usuario": "adriano", "nonce_cliente": "edba5f8006366f8700a4681c458a17f8", "prova": "6e5e192701b549c8bf17eec5a5cf741ad08ebcaaefcabe5a85b3a3eb0c65bb49", "token": "token-da-porta"}
<- {"ok":true,"op":"login","resultado":{"id":1,"nome":"root","login":"adriano", ...,"supervisor":true,"ativo":true},"ms":0}
```

A prova em ambos os sentidos: repetir o MESMO `nonce_cliente`/`prova` na
segunda vez falharia (o `nonce` do servidor vale uma vez só) — não
exercitado aqui porque já é o que `docs/SEGURANCA.md` mede; o que esta
rodada mede é a conta em si, feita do zero, sem olhar código do PhxSql.

Login por **Base64** e por **texto puro**, mesma sessão, mesmo servidor:

```
-> {"op": "login", "usuario_b64": "YWRyaWFubw==", "senha_b64": "Y29ycmV0YS1ob3JzZS1iYXR0ZXJ5", "token": "token-da-porta"}
<- {"ok":true,"op":"login", ... ,"ms":266}

-> {"op": "login", "usuario": "adriano", "senha": "correta-horse-battery", "token": "token-da-porta"}
<- {"ok":true,"op":"login", ... ,"ms":265}
```

CRUD completo pela mesma conexão, após login por desafio-resposta:

```
-> {"op": "criar_database", "database": "loja", "token": "token-da-porta"}
<- {"ok":true,"op":"criar_database","resultado":{"database":"loja","caminho":"dados/loja"},"ms":0}
-> {"op": "inserir", "database": "loja", "tabela": "clientes", "valores": {"id": 1, "nome": "Adriano Boller", "cidade": "Blumenau"}, "token": "token-da-porta"}
<- {"ok":true,"op":"inserir","resultado":{"rowid":1,"registros":1},"ms":4}
-> {"op": "ler", "database": "loja", "tabela": "clientes", "rowid": 1, "token": "token-da-porta"}
<- {"ok":true,"op":"ler","resultado":{"id":1,"nome":"Adriano Boller","cidade":"Blumenau","softdeleted":false,"rownum":1},"ms":0}
```

E o **mesmo servidor**, agora pelo console oficial `phxsqlcmd` (a prova de
que console e "nativo à mão" são o mesmo protocolo):

```
$ PHXSQL_SENHA=correta-horse-battery phxsqlcmd --host 127.0.0.1 --porta 6400 \
    --token token-da-porta --usuario adriano --database loja \
    --comando 'SELECT nome, cidade FROM clientes'

sql: SELECT nome, cidade FROM clientes   op: varrer   registros: 1 ...
colunas (2):
  nome
  cidade
linhas (1):
nome            cidade
--------------  --------
Adriano Boller  Blumenau
```

### 2. ODBC — driver de ABI C, carregado por `dlopen`

`bancada/odbc/provar.py --porta 6402` sobe o servidor, monta os dados
conhecidos e roda `bancada/odbc/prova-abi.py` contra `libphxsql_odbc.so`
pelo `ctypes` (a mesma forma que o gerenciador de driver do sistema
carregaria):

```
· phxsqld pid 5192 na porta 6402
== 1. ciclo de handles: ENV -> versao ODBC 3 -> DBC -> conexao ==
  ok  SQLDriverConnect: 0
== 3. SELECT * descreve as colunas com tipo honesto ==
  ok  col 1: (0, 'id', 4, 10, 0, 0)
  ok  col 2: (0, 'nome', 12, 40, 0, 1)
  ok  col 3: (0, 'limite', 3, 12, 2, 1)
  ok  col 4: (0, 'desde', 91, 10, 0, 1)
== 7. COUNT(*) vira uma celula SQL_BIGINT ==
  ok  a contagem: (0, '3')
== 8. erro proposital: SQL_ERROR com diagnostico que nao vaza senha ==
  ok  tabela que nao existe e 42S02: '42S02'
  ok  a senha nao esta na mensagem: False
== 9. buffer curto trunca AVISANDO e continua ==
  ok  truncou com SUCCESS_WITH_INFO: 1
  ok  o pedaco que coube: 'Adriano'
  ok  a continuacao vem inteira: 0
  ok  o resto do nome: ' Boller'

PROVA COMPLETA: a .so carregada por dlopen respondeu a sequencia ODBC
inteira com os valores inseridos, os tipos do esquema e o truncamento
avisado -- cada afirmacao acima foi conferida, nao so impressa.
```

**73 conferências, 0 falhas** (`grep -c '^  ok' saida` = 73; nenhuma linha
`ERRO`/`FALHA`).

A string de conexão (DSN-less, chaves sem distinção de maiúsculas):

```
Driver=PhxSql;Server=127.0.0.1;Port=6402;Token=prova-odbc;UID=root;PWD=prova123;Database=base
```

Um DSN de arquivo (`/etc/odbcinst.ini`, Linux) para o mesmo driver:

```ini
[PhxSql]
Description = Driver ODBC do PhxSql
Driver = /caminho/para/libphxsql_odbc.so
Threading = 2
```

com `isql -v -k "Driver=PhxSql;Server=...;Port=...;Token=...;UID=...;PWD=..."`
testando sem DSN nenhum — já provado no `docs/ODBC.md` §7 com unixODBC
2.3.12 real (`Connected!`, grade com cabeçalho); não repetido nesta rodada
porque a prova de ABI acima já cobre o mesmo driver, e refazer só o `isql`
não mediria nada novo.

### 3. REST — outra forma de "connection", pela porta HTTP

Servidor próprio, dados na 6417 e REST na 6418, com desafio-resposta feito
do navegador para dentro (a mesma conta do item 1, agora sobre HTTP):

```
POST /v1/desafio  {"usuario": "root"}
  200 {"ok":true, ...,"resultado":{"sal":"...","iteracoes":210000,"nonce":"..."},"sessao":"c84c7c18..."}

POST /v1/login  {"usuario": "root", "prova": "...", "nonce_cliente": "..."}
  200 {"ok":true,"op":"login","resultado":{"id":1,"nome":"root",...},"sessao":"c84c7c18..."}

POST /v1/ler  {"tabela": "clientes", "rowid": 1}
  200 {"ok":true,"op":"ler","resultado":{"id":1,"nome":"Ana Maria",...},"sessao":"c84c7c18..."}
```

E o `Bearer` errado recusa com o código certo, sem vazar nada:

```
POST /v1/ler  {"tabela": "clientes", "rowid": 1}   (Authorization: Bearer errado)
  401 {"ok":false,"op":"ler","erro":"[SP000025] acesso negado: token invalido", ...}
```

A bancada completa (57 passos, incluindo ARM64) é `bancada/rest/provar.py` —
não usada aqui por rodar em portas fora da faixa 6400–6419 desta frente; o
que está acima é um exercício próprio, curto, nesta faixa.

### 4. Embutido/FFI — o motor como biblioteca, sem porta nenhuma

`bancada/embutido/provar.sh x86` compila `crates/phxsql-ffi/c/prova.c`
contra o `.a` (ligação estática) e contra o `.so` (formato do Android) e
roda os dois:

```
staticlib 10M   cdylib 1.4M
simbolos phx_ exportados no .so: 44
1. abrir, criar, gravar e ler
  ok    phx_base_abrir com PHX_CRIAR
  ok    phx_ler do rowid 1
        linha 1: id=1 nome=Adriano Boller cidade=Blumenau
4. os caminhos de erro
  ok    chave duplicada devolve 3002
6. o panico que nao atravessa
  ok    contagem absurda vira erro, e nao aborta
  ok    o punho fica envenenado depois
40 passos, 0 falhas
e contra a cdylib (o formato do Android): 40 passos, 0 falhas
```

**40 passos, 0 falhas**, contra os dois formatos (`.a` e `.so`) — sem
soquete, sem `config.json`, sem thread de aceitação: é literalmente "ligar o
motor dentro do processo do aplicativo", que é a única forma de "connection"
que funciona em iOS e Android (`docs/EMBUTIDO.md` §1).

## O que NÃO existe, e é dispensa registrada

- **OLE DB nativo.** Não existe, e não é esquecimento: `docs/ODBC.md` §6
  documenta a decisão — um provider OLE DB é um objeto COM (class factory
  por CLSID, `IDBInitialize`, `IRowset`, apartamento de thread), só Windows,
  impossível de provar nesta bancada (COM não roda aqui), e entregar um
  provider nunca executado seria o "parece certo" que as regras da casa
  proíbem. O caminho suportado é a ponte oficial da Microsoft, o
  **`MSDASQL`** ("Microsoft OLE DB Provider for ODBC Drivers"), que envolve
  o driver ODBC provado acima:

  ```
  Provider=MSDASQL;Extended Properties="Driver=PhxSql;Server=10.0.0.7;Port=5000;Token=...;UID=...;PWD=...;Database=loja"
  ```

  Foi assim que o próprio SQL Server(R) apareceu em OLE DB durante décadas, e
  cobre Excel, Access, ADO e companhia. Um provedor OLE DB nativo exigiria
  escrever a fachada COM inteira (registro no sistema, `IDBCreateSession`,
  `ICommandText`, `IAccessor`) — trabalho de uma rodada própria, e só
  provável num Windows de verdade, que não está disponível aqui.
- **`SQLBindParameter` (parâmetros no ODBC)**, **`SQLTables`/`SQLColumns`
  (catálogo)** e **escrita (INSERT/UPDATE/DELETE) pelo driver ODBC** — as
  três já documentadas em `docs/ODBC.md` §2, com o motivo de cada uma
  (a op `sql` do servidor só traduz `SELECT` hoje; o catálogo do servidor
  existe mas não está ligado ao driver ainda). Não exercitadas de novo aqui
  porque a resposta já é a mesma da última medição, e nada mudou no driver
  nesta rodada.
- **O `.so`/DLL do ODBC executado num Windows de verdade** — compilado para
  `x86_64-pc-windows-gnu` e conferido por `objdump`, mas nunca rodado lá
  (`docs/ODBC.md` §3). A prova de ABI desta rodada, como a de referência,
  rodou em Linux.
- **A bancada REST completa (57 passos) e o programa em C do FFI sob ARM64**
  não foram refeitas nesta rodada: usam portas ou caminho de compilação
  cruzada fora do que esta frente precisava para responder "connection"; o
  exercício próprio acima já prova os dois caminhos com o motor vivo.

## Como se refaz

```bash
python3 bancada/conexoes/nativo.py            # nativo: 3 logins + CRUD + console
python3 bancada/odbc/provar.py --porta 6402   # ODBC: 73 conferencias pela ABI
bash bancada/embutido/provar.sh x86           # FFI/embutido: 40 passos, .a e .so
```

O exercício de REST desta resposta não tem script próprio salvo (é curto
demais para merecer um; a bancada de referência é `bancada/rest/provar.py`).
