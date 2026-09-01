# O driver ODBC: PhxSql para programas de terceiros

O pedido #7 pede o caminho para Excel, Access, Crystal Reports e todo
programa que fala ODBC ou OLE DB enxergarem o PhxSql. A resposta desta
rodada e um **driver ODBC 3.x de verdade** — e a decisao documentada de
**nao** escrever um provider OLE DB nativo (secao 6, com o motivo e a ponte
que o substitui).

O driver mora em `crates/phxsql-odbc/`: uma `cdylib` de ABI C que o
gerenciador de driver carrega por `dlopen`/`LoadLibrary`. Por dentro ela e
um cliente comum da porta de dados — TCP, uma linha JSON por pedido, as ops
`login`, `sql` e `esquema` — escrito so com a `std` e o `phxsql-core` do
proprio workspace, como tudo aqui. Compila para Linux e para Windows
(`x86_64-pc-windows-gnu`) sem nada alem do `rustup target add`, e o
`empacotar.sh` ja poe `libphxsql_odbc.so` e `phxsql_odbc.dll` nos pacotes.

## 1. A connection string

DSN-less, chaves sem distincao de maiusculas:

```
Driver=PhxSql;Server=10.0.0.7;Port=5000;Token=o-token;UID=maria;PWD=a-senha;Database=loja
```

| chave | apelidos aceitos | o que e |
|---|---|---|
| `Server` | `Host`, `Servidor` | endereco do phxsqld |
| `Port` | `Porta` | porta de DADOS (a 5000 do config, nao a web) |
| `Token` | — | o `token` do config.json |
| `UID` | `User`, `Usuario` | login do usuario |
| `PWD` | `Password`, `Senha` | senha (aceita `{chaves}` para `;` dentro) |
| `Database` | `Db` | banco padrao dos comandos |

Duas decisoes que valem saber:

* **A string que o `SQLDriverConnect` devolve sai com `PWD=***` e
  `Token=***`.** O aplicativo costuma guardar essa string em arquivo de
  configuracao proprio, e o driver nao decide onde ela vai parar. O preco: a
  string devolvida nao serve para reconectar sozinha.
* **So DSN-less.** Um DSN de arquivo (`odbc.ini`, chaves de registro) exige
  que o DRIVER leia a configuracao via `SQLGetPrivateProfileString`, que
  mora na biblioteca do instalador (`libodbcinst`) — uma dependencia
  externa, que este projeto nao aceita. Registrar o driver (secao 3) usa o
  arquivo do sistema normalmente; so os PARAMETROS da conexao e que viajam
  na string. `SQLConnect` existe e aceita `host:porta/database` (ou uma
  connection string inteira) no lugar do nome do DSN.

## 2. O que o driver cobre — e o que ficou de fora, com o motivo

Exporta 21 funcoes, o nucleo que um consumidor de LEITURA usa:

```
SQLAllocHandle   SQLFreeHandle    SQLFreeStmt     SQLSetEnvAttr
SQLDriverConnect SQLConnect       SQLDisconnect
SQLExecDirect    SQLPrepare       SQLExecute
SQLNumResultCols SQLDescribeCol   SQLColAttribute SQLRowCount
SQLBindCol       SQLFetch         SQLGetData
SQLGetDiagRec    SQLGetInfo       SQLSetConnectAttr SQLSetStmtAttr
```

* O SQL aceito e o do servidor — o subconjunto de `SELECT` da op `sql`
  (`docs/SQL.md`). O texto vai INTEIRO para la: o parser mora no servidor, e
  o erro dele volta pelo `SQLGetDiagRec` com a coluna do problema. Sintaxe
  sai como SQLSTATE `42000`, tabela inexistente como `42S02`, e o campo
  `codigo` do servidor vira o "native error".
* `SQLPrepare` + `SQLExecute` existem porque o `isql` e outros clientes so
  falam por eles — mas preparar aqui e guardar o texto: **nao ha
  parametros** (`SQLBindParameter` ficou de fora; sem eles, preparar de
  verdade nao compraria nada).
* O conjunto de resultados chega INTEIRO na resposta (o servidor corta em
  `max_linhas`, 1000 por padrao). Consulta grande pede `LIMIT`/`OFFSET`.
* O fetch entrega texto (`SQL_C_CHAR`), inteiros (`SQL_C_SLONG` e parentes,
  com conferencia de faixa — estourar da `22003`) e ponto flutuante
  (`SQL_C_DOUBLE`/`FLOAT`). Buffer curto trunca AVISANDO (`01004`,
  `SQL_SUCCESS_WITH_INFO`) e a proxima chamada continua de onde parou — ha
  teste com o defeito reposto para isso (secao 7).
* **So as funcoes ANSI.** As `...W` (UTF-16) ficaram de fora: o gerenciador
  de driver converte as chamadas wide do aplicativo para as ANSI sozinho, e
  o texto aqui e UTF-8 dos dois lados. O custo de dobrar a superficie nao
  comprava funcionalidade nesta rodada. Consequencia pratica: acento chega
  como UTF-8 — aplicativo Windows que exija UCS-2 no buffer vai mostrar
  acento errado ate a rodada das `W`.
* **Sem transacoes NO DRIVER** — e o motivo mudou, entao a frase mudou junto.
  `SQLGetInfo(SQL_TXN_CAPABLE)` responde `SQL_TC_NONE` e desligar o autocommit
  e recusado com `HYC00`, como antes; o que ja **nao** e verdade e a
  justificativa que estava escrita aqui, «porque o servidor nao tem».

  O servidor tem: `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT` entraram e nada vai a
  disco antes do `COMMIT`. Isso faz desta uma lacuna **do driver**, e nao um
  limite do motor — o que e uma divida maior, nao menor, porque o driver esta
  sub-relatando uma capacidade que existe. Uma auditoria externa achou esta
  contradicao, e ela estava certa.

  Continua valendo o principio: prometer `rollback` que o driver nao sabe
  entregar seria pior que recusar. O que falta e ligar o `SQLEndTran` as
  operacoes que ja existem no protocolo, e isso e uma rodada propria.
* **Sem `SQLTables`/`SQLColumns`** (o catalogo): o servidor ja tem
  `sistabelas`/`siscolunas`, e ligar uma na outra e uma rodada propria.
  Ferramenta que exige catalogo para listar tabelas vai listar vazio; a
  consulta digitada funciona.
* **Escrita (INSERT/UPDATE/DELETE) nao passa**, porque a op `sql` do
  servidor so traduz SELECT hoje. Quando o servidor aprender, o driver ja
  repassa — ele nao olha o verbo.

## 3. Instalar e registrar

### unixODBC (Linux)

No `odbcinst.ini` do sistema (`/etc/odbcinst.ini`, ou o do `ODBCSYSINI`):

```ini
[PhxSql]
Description = Driver ODBC do PhxSql
Driver = /caminho/para/libphxsql_odbc.so
Threading = 2
```

(ou `odbcinst -i -d -f esse-arquivo.ini`, se o utilitario estiver
instalado). Teste imediato, sem DSN:

```bash
isql -v -k "Driver=PhxSql;Server=127.0.0.1;Port=5000;Token=...;UID=...;PWD=...;Database=loja"
```

Provado nesta maquina com unixODBC 2.3.12: `Connected!`, grade com
cabecalho, projecao e `COUNT(*)` — a transcricao esta na secao 7.

### Windows

O registro de driver ODBC no Windows e um par de chaves de registro (e o
que o instalador oficial da Microsoft escreve; `odbcconf.exe` faz o mesmo):

```reg
Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\ODBC\ODBCINST.INI\PhxSql]
"Driver"="C:\\phxsql\\phxsql_odbc.dll"
"Setup"=""
"APILevel"="1"
"ConnectFunctions"="YYN"
"DriverODBCVer"="03.00"

[HKEY_LOCAL_MACHINE\SOFTWARE\ODBC\ODBCINST.INI\ODBC Drivers]
"PhxSql"="Installed"
```

Depois disso qualquer aplicativo conecta pela connection string da secao 1
(em Excel/Access: "outra origem de dados" -> connection string).

**O limite honesto desta rodada:** a DLL foi COMPILADA para
`x86_64-pc-windows-gnu` (o mesmo alvo dos .exe, que ja rodam la) e exporta
as 21 funcoes pelo nome — conferido com `objdump` no PE. Ela **nao foi
executada num Windows de verdade** nesta rodada; a prova funcional (secao 7)
rodou no Linux, pela mesma base de codigo. Se o primeiro uso no Windows
tropecar, o suspeito numero um e convencao de chamada ou largura de tipo —
e a assinatura de tudo esta em `crates/phxsql-odbc/src/tipos.rs`.

## 4. Os tipos, honestos

O tipo de cada coluna vem da op `esquema`, nao de adivinhar pelo valor — e
por isso `SELECT nome AS apelido` declara `SQL_VARCHAR` (o apelido nao esta
no esquema; texto e o que da para prometer). Quando o esquema inteiro nao
responde, o driver avisa (`01000`, `SQL_SUCCESS_WITH_INFO`) e declara tudo
texto.

| PhxSql | ODBC | observacao |
|---|---|---|
| `Int4` / `Int2` / `Int1` | `SQL_INTEGER` / `SQL_SMALLINT` / `SQL_TINYINT` | |
| `Int8`, `Sequence` | `SQL_BIGINT` | |
| `UInt4` / `UInt8` | `SQL_BIGINT` | sem sinal nao cabe no tipo assinado do mesmo tamanho |
| `Decimal(p,e)` | `SQL_DECIMAL(p,e)` | viaja como texto com as casas exatas |
| `Real4` / `Real8` | `SQL_REAL` / `SQL_DOUBLE` | |
| `Date` | `SQL_TYPE_DATE` | texto `AAAA-MM-DD` |
| `Time` | `SQL_TYPE_TIME` | texto `HH:MM:SS,cc` — centesimos, virgula |
| `DateTime` | `SQL_TYPE_TIMESTAMP` | texto `AAAA-MM-DD HH:MM:SS,mmm` |
| `Str(n)` | `SQL_VARCHAR(n)` | UTF-8 |
| `Memo` | `SQL_LONGVARCHAR` | le-se em pedacos pelo `SQLGetData` |
| `Bin` | `SQL_LONGVARCHAR` | o servidor manda HEXADECIMAL; prometer `VARBINARY` mentiria |
| `Uuid` / `Uuid256` | `SQL_CHAR(36)` / `SQL_CHAR(64)` | forma canonica minuscula |
| `Bool` | `SQL_BIT` | `1`/`0` |
| `COUNT(*)` | `SQL_BIGINT` | uma grade de uma celula |

`SELECT *` projeta pelas colunas do esquema, NA ORDEM DELE, e esconde as
colunas de sistema (softdeleted, rownum) — o mesmo que a tela faz. Quem
quiser uma coluna de sistema pede por nome.

## 5. Senha e token nao vazam — por construcao e por teste

O login leva a senha no corpo do pedido; por isso **nenhum caminho de erro
do transporte ecoa o pedido** — a mensagem de falha menciona so a operacao.
A connection string devolvida mascara `PWD` e `Token`. Ha teste unitario
para a mascara (`mascarada_nao_vaza_segredo`) e conferencia na prova de ABI
(a mensagem de diagnostico de um erro de verdade e vasculhada pela senha).

## 6. OLE DB: a decisao de NAO escrever um provider nativo

Um provider OLE DB e um objeto COM: class factory registrada por CLSID no
registro do Windows, `IDBInitialize`/`IDBCreateSession`/`IDBProperties`,
`ICommandText`, `IRowset` com acessores de campo (`IAccessor`), semantica de
apartamento de thread, e o instalador disso tudo. E um mundo proprio, so
Windows, impossivel de PROVAR aqui (COM nao roda nesta bancada) — e provador
e o criterio da casa: entregar um provider que nunca rodou seria exatamente
o "parece certo" que as regras proibem.

O caminho suportado e a **ponte oficial da Microsoft**: o provider
`MSDASQL` ("Microsoft OLE DB Provider for ODBC Drivers"), que vem no
Windows e transforma qualquer driver ODBC em origem OLE DB. Connection
string de consumidor OLE DB (ADO, por exemplo):

```
Provider=MSDASQL;Extended Properties="Driver=PhxSql;Server=10.0.0.7;Port=5000;Token=...;UID=...;PWD=...;Database=loja"
```

E a ponte canonica — durante decadas foi como o proprio SQL Server aparecia
em OLE DB — e cobre Excel, Access, ADO e companhia. Se um dia um consumidor
exigir OLE DB nativo (linked server do SQL Server com recursos finos, por
exemplo), o custo esta descrito no primeiro paragrafo e vira pedido proprio.

## 7. A prova — e como repeti-la

A prova nao passa pelo unixODBC de proposito: `bancada/odbc/prova-abi.py`
carrega a MESMA `.so` por `ctypes`/`dlopen` e chama as MESMAS funcoes que o
gerenciador de driver chamaria — prova de ABI literal, mais o `isql` por
cima como prova de integracao.

```bash
# 1. um phxsqld SEU (a prova usou 127.0.0.1:5305, token prova-odbc,
#    root/prova123 -- config minimo baseado no exemplos/Config_exemplo_01.json)
# 2. a tabela e as linhas conhecidas:
python3 bancada/odbc/montar-dados.py
# 3. a prova pela ABI:
cargo build --release -p phxsql-odbc
python3 bancada/odbc/prova-abi.py target/release/libphxsql_odbc.so
```

Resultado registrado (2026-08-29, Linux x86_64, unixODBC 2.3.12):

* **73 conferencias, zero falhas** — handles, conexao, `SELECT *` com os
  quatro tipos descritos certos (`SQL_INTEGER`, `SQL_VARCHAR(40)`,
  `SQL_DECIMAL(12,2)`, `SQL_TYPE_DATE`), valores identicos aos inseridos,
  decimal com as duas casas (`4200.50`), NULL pelo indicador, coluna
  amarrada, projecao com WHERE pela chave, `COUNT(*)`, prepare/execute,
  erros com SQLSTATE e native error, truncamento com continuacao, e o
  desmonte na ordem.
* **`isql` de verdade:** `Connected!`, grade com cabecalho e os tres
  valores de `limite` certos, projecao e contagem — via
  `isql -v -k "Driver=PhxSql;..."` com o driver registrado num
  `odbcinst.ini` proprio (`ODBCSYSINI`).
* **O teste do defeito reposto** (a regra da casa: prova real nos dois
  sentidos): recolocado o truncamento CALADO no `SQLGetData` — devolver
  `SQL_SUCCESS` e dar a celula por entregue quando o buffer nao coube — o
  teste unitario `entregar_trunca_avisa_e_continua` falha e a prova de ABI
  falha em 4 conferencias (`SUCCESS_WITH_INFO`, `01004`, a continuacao
  ' Boller', o `SQL_NO_DATA` do fim), com o aplicativo recebendo `Adriano`
  como se fosse o nome inteiro. Com o conserto, 73/73. E o defeito
  classico de driver ODBC, e agora esta preso por teste dos dois lados.

## 8. Aprendizados da prova (frutiferos e infrutiferos)

* **`SQLRETURN` tem 16 bits, e a ABI so promete os 16 de baixo.** A
  primeira rodada da prova falhou 6 conferencias com valores como
  `1990525028` — cujos 16 bits baixos eram exatamente o `100`/`-1`/`-2`
  esperado. O defeito era DA PROVA: o `ctypes` le `c_int` por padrao, e o
  lixo nos bits altos e legitimo. O gerenciador de driver declara `short` e
  nunca ve isso. Mesma familia da licao do `socket.makefile()`: o teste
  errado acusa o servidor certo.
* **"Connected!" nao e prova de driver.** O `isql` conecta por
  `SQLDriverConnect` e consulta por `SQLPrepare`+`SQLExecute` — que nao
  estavam no recorte original, e sem os quais a primeira consulta morre em
  `IM001`. E depois o cabecalho da grade sai VAZIO se `SQLColAttribute` nao
  souber `SQL_DESC_LABEL` (18), que nenhuma lista "minima" menciona.
  Interface so se prova exercitando — a mesma licao da tela, agora na ABI.
* **Heuristica de texto morreu no primeiro contato; o campo estruturado
  ficou.** O plano era mapear sintaxe para `42000` por prefixo
  («SQL, coluna...»), e a mensagem real chega com prefixo proprio
  («esquema invalido: SQL, coluna...») — a heuristica nunca casava. O erro
  do servidor ja traz `nome` e `codigo` estruturados; o SQLSTATE agora sai
  do `nome` (`NAO_ENCONTRADO` -> `42S02`) e o `codigo` vira o native error
  do diagnostico. Analisar, nao recortar — a regra do Profiler, aqui.
* **Infrutifera, registrada para nao voltar:** tentar dar tipo honesto a
  apelido de coluna. A resposta da op `sql` so traz o ROTULO da projecao;
  ligar apelido a coluna de origem exigiria repetir o parser do servidor no
  driver — duas implementacoes da mesma gramatica, divergindo em silencio.
  Apelido declara `SQL_VARCHAR` e o valor continua integro; quem quiser o
  tipo pede a coluna pelo nome.

## 9. Os arquivos

```
crates/phxsql-odbc/          o driver (cdylib de ABI C)
  src/lib.rs                 as 21 funcoes exportadas
  src/tipos.rs               constantes e larguras da especificacao
  src/conexao.rs             connection string, TCP, login, erros com SQLSTATE
  src/resultado.rs           esquema -> tipos ODBC, montagem do resultado
  src/registro.rs            handles como chaves de mapa (nunca ponteiro cru)
  src/texto.rs               truncamento e strings pela fronteira C
bancada/odbc/montar-dados.py o banco conhecido da prova
bancada/odbc/prova-abi.py    a prova pela ABI (dlopen + ctypes), 73 conferencias
docs/ODBC.md                 este documento
```
