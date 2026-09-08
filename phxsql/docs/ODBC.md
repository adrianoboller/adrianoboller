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
| `CIFRA` | `Encrypt` | `1`/`true`/`sim` liga o aperto de mao (a cifra do fio) |
| `CHAVE_DO_FIO` | `Pino` | o pino: a chave publica X25519 do servidor, em hexa |

A cifra do fio tem uma secao propria (1.1); o resto do driver nao muda com ela.

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

## 1.1. A cifra do fio (o aperto de mao)

O desenho inteiro esta em `docs/CIFRA-DO-FIO.md`. O driver e um cliente comum
da porta de dados, e por isso ele fala o mesmo aperto de mao estilo Noise que a
`replica::Cliente` fala — reusando o `fio` do `phxsql-core`, sem uma segunda
copia de cripto aqui.

* **`CIFRA=1`** liga o tunel: antes de qualquer pedido, o driver manda
  `{"op":"cifrar",...}`, fecha o aperto e, da linha seguinte em diante, fala
  registros selados. O login e o token passam a viajar POR DENTRO do tunel.
* **`CHAVE_DO_FIO=<64 hexa>`** e o PINO — a chave publica que se ESPERA do
  servidor. Com pino, um servidor que apresente outra chave derruba a conexao
  (a defesa contra quem esta no meio). Sem pino, o tunel protege so da escuta
  PASSIVA. O pino sai do proprio servidor: `phxsqld --chave-do-fio` o imprime.
* Escrever o pino **liga a cifra sozinho**: cair para claro por ter esquecido
  `CIFRA=1` seria rebaixar em silencio o que a pessoa pediu. Para falar claro,
  nao escreva o pino.
* **Sem essas chaves, o driver fala CLARO, exatamente como sempre falou.** Um
  servidor com `cifra_fio.exigir: true` recusa o claro com erro nomeado ("este
  servidor exige a cifra do fio"), e a conexao falha no primeiro pedido — em vez
  de um silencio.

Dois limites, ditos sem enfeite:

* **Pino torto e ERRO, nao "siga sem pino".** Um `CHAVE_DO_FIO` que nao seja uma
  X25519 de 64 hexa recusa a conexao, em vez de virar "sem pino" — deixar um
  pino invalido rebaixar a garantia e o oposto do que ele existe para fazer.
* **O login do driver e a senha em claro DENTRO do tunel**, e nao o
  desafio-resposta. Ele nao amarra a credencial ao canal (`amarrar_canal`),
  entao um servidor com `cifra_fio.exigir_amarra: true` recusaria esse login.
  `exigir` (a decisao desta rodada) o driver atende; `exigir_amarra` fica para
  quando o driver aprender o desafio-resposta.

## 2. O que o driver cobre — e o que ficou de fora, com o motivo

Exporta 24 funcoes, o nucleo que um consumidor de LEITURA usa:

```
SQLAllocHandle   SQLFreeHandle    SQLFreeStmt      SQLSetEnvAttr
SQLDriverConnect SQLConnect       SQLDisconnect
SQLExecDirect    SQLPrepare       SQLExecute
SQLBindParameter SQLNumParams     SQLDescribeParam
SQLNumResultCols SQLDescribeCol   SQLColAttribute  SQLRowCount
SQLBindCol       SQLFetch         SQLGetData
SQLGetDiagRec    SQLGetInfo       SQLSetConnectAttr SQLSetStmtAttr
```

* O SQL aceito e o do servidor — o subconjunto de `SELECT` da op `sql`
  (`docs/SQL.md`). O texto vai INTEIRO para la: o parser mora no servidor, e
  o erro dele volta pelo `SQLGetDiagRec` com a coluna do problema. Sintaxe
  sai como SQLSTATE `42000`, tabela inexistente como `42S02`, e o campo
  `codigo` do servidor vira o "native error".
* `SQLPrepare` + `SQLExecute` existem porque o `isql` e outros clientes so
  falam por eles. Preparar aqui e guardar o TEXTO — o plano continua sendo do
  servidor —, e o texto guardado e a fonte da contagem de `?`. **Os parametros
  existem desde a rodada das dezoito** e tem secao propria (2.1).
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

## 2.1. Parametros de instrucao preparada (`?`)

O driver liga valor por POSICAO e manda `"parametros": [...]` junto do texto,
no contrato de `docs/propostas/comparativo-19.md`:

```json
{"op":"sql","database":"loja","texto":"SELECT * FROM clientes WHERE id = ?",
 "parametros":[3]}
```

**Sem `?` no texto, o campo nem aparece no pedido.** Quem nunca ligou
parametro manda byte a byte o que sempre mandou — guarda nova entra pedida,
nao imposta.

### O que o driver LIGA

| tipo C | vai no JSON como |
|---|---|
| `SQL_C_CHAR`, `SQL_C_DEFAULT` | texto |
| `SQL_C_SSHORT`, `SQL_C_SHORT`, `SQL_C_SLONG`, `SQL_C_LONG` | numero |
| `SQL_C_SBIGINT` | numero ate 2^53, **texto acima** |
| `SQL_C_DOUBLE`, `SQL_C_FLOAT` | **texto**, sempre |
| `SQL_C_BIT` | `true` / `false` |
| indicador `SQL_NULL_DATA` | `null`, em qualquer tipo C |

Quatro coisas dessa tabela sao decisao, e nao gosto:

* **O tamanho do texto sai do INDICADOR** (ou do NUL, com `SQL_NTS`), nunca do
  `BufferLength` — a especificacao manda ignora-lo em parametro de entrada de
  tipo caractere.
* **Inteiro acima de 2^53 vai como texto** porque o `Json` desta casa guarda
  numero num `f64`: um id de dezenove digitos voltaria ARREDONDADO, e a linha
  viria errada sem erro nenhum. O `json_para_valor` do servidor aceita inteiro
  em texto exatamente por causa deste caminho — o comentario esta la, e nomeia
  o ODBC.
* **Fracionario vai como texto SEMPRE**, que e a regra da casa inteira: o
  `literal_para_json` do tradutor manda todo literal numerico como texto, e o
  `json_para_valor` do servidor RECUSA decimal que chegue como numero («para
  nao perder centavo em f64»). Um fracionario em `Json::Numero` seria aceito
  numa coluna `Real` e recusado numa `Decimal` — duas respostas para a mesma
  ligacao. O `{}` do Rust escreve a forma mais curta que releia o MESMO `f64`:
  o driver nao inventa digito nem perde o que o aplicativo ja tinha.
* **`SQL_NULL_DATA` vem antes de tudo**: com ele o ponteiro de valor pode ser
  nulo de direito, e o driver nem olha o buffer.

### O que ele RECUSA, e onde

A recusa acontece na **ligacao** e nao na execucao, pela mesma decisao que o
`ao_excluir` desta casa ja tomou: uma ligacao se declara uma vez e se executa
muitas. Recusar cedo custa um erro lido enquanto se escreve o codigo; recusar
tarde custa um laco de mil execucoes que morre na primeira, longe de quem o
escreveu.

| o que | SQLSTATE | onde | motivo |
|---|---|---|---|
| posicao zero | `07009` | ligacao | o primeiro `?` e o 1 |
| `SQL_PARAM_OUTPUT` / `_INPUT_OUTPUT` | `HYC00` | ligacao | saida pediria valor por posicao de volta; a op `sql` devolve linhas |
| `SQL_C_WCHAR` | `HYC00` | ligacao | o driver e ANSI (secao 2); ligue `SQL_C_CHAR` em UTF-8 |
| outro tipo C | `HYC00` | ligacao | a mensagem NOMEIA o tipo e lista os que servem |
| valor nulo sem indicador | `HY009` | ligacao | `NULL` se manda pelo indicador |
| `SQL_DATA_AT_EXEC` | `HYC00` | execucao | `SQLPutData` nao existe aqui; ler o buffer pegaria lixo |
| NaN / infinito | `22003` | execucao | nao ha literal para eles nesta linguagem |
| faltou ligacao para um `?` | `07002` | execucao | **antes da rede**: nada sai, e a mensagem diz a posicao e os dois numeros |

### A contagem dos `?`

`SQLNumParams` conta pelo mesmo criterio do lexico do servidor
(`crates/phxsql-sql/src/lexico.rs`), que engole **quatro** trechos e nao so as
aspas simples: `'texto'` (com `''` valendo uma aspa dentro), `"identificador"`
(com `""`), o comentario `-- ate o fim da linha` e o `/* de bloco */`. Um `?`
dentro de qualquer um deles e dado ou comentario. Sem `SQLPrepare` antes a
resposta e `HY010` e nao zero — zero seria lido como «esta instrucao nao tem
parametros», e a ferramenta nem tentaria ligar.

**O preco da conta ser refeita aqui** (o driver nao depende do `phxsql-sql`, e
o lexico de hoje nem aceita `?`) esta escrito no proprio codigo: contexto novo
que o lexico aprender a engolir tem de chegar ao contador junto, ou o driver
passa a contar um `?` que o servidor nao ve.

### O que se le na EXECUCAO, e nao na ligacao

`SQLBindParameter` guarda o **endereco**, e isso e o contrato do ODBC: o valor
se le no `SQLExecute`. E o que permite ligar uma vez e executar mil trocando
so o conteudo do buffer — o laco de carga de qualquer ferramenta. A
contrapartida de seguranca: o driver **so desreferencia esses ponteiros dentro
de `SQLExecute`/`SQLExecDirect`**, a unica janela em que o contrato promete que
eles valem; nada mais no driver os toca.

`SQLDescribeParam` declara `SQL_VARCHAR` para todo `?`, com tamanho zero
(«nao sei») e nulavel. Nao e preguica: o driver nao planeja nada na
preparacao, entao nao sabe a que coluna cada `?` se compara — e um
`SQL_INTEGER` chutado seria a mesma mentira que a secao 8 ja recusou contar
sobre apelido de coluna. Nao ha risco de buffer no tamanho zero, porque o
driver nunca ESCREVE num buffer de parametro: saida e recusada na ligacao.

### O limite honesto de hoje

**A op `sql` do servidor ainda nao le `parametros`**, e o lexico dele recusa o
caractere `?` — medido em 08/09/2026 em
`crates/phxsql-server/src/servidor.rs:11795` (`op_sql` le so `texto`/`sql`).
Entao um `WHERE id = ?` volta com erro de sintaxe *do servidor*, e nao com a
linha. O lado do driver esta pronto e provado; o outro lado e a frente
F-CONSULTA do `docs/propostas/comparativo-19.md`.

O passo 7c da prova de ABI e uma **sonda viva**: ele confere sempre o que nao
depende do servidor e TENTA a volta, e enquanto ela nao vier a linha sai como
**NAO MEDIDA** com o motivo — nunca como «ok», nunca sumindo da lista. E o
teste `ponta_a_ponta_where_id_igual_pergunta` esta escrito e `#[ignore]`, com
o motivo no comentario.

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

* **86 conferencias, zero falhas e 1 NAO MEDIDA** (eram 73 antes do passo
  7c) — handles, conexao, `SELECT *` com os
  quatro tipos descritos certos (`SQL_INTEGER`, `SQL_VARCHAR(40)`,
  `SQL_DECIMAL(12,2)`, `SQL_TYPE_DATE`), valores identicos aos inseridos,
  decimal com as duas casas (`4200.50`), NULL pelo indicador, coluna
  amarrada, projecao com WHERE pela chave, `COUNT(*)`, prepare/execute,
  erros com SQLSTATE e native error, truncamento com continuacao, e o
  desmonte na ordem; e o passo 7c dos parametros (a contagem que nao conta o
  `?` das aspas, o `SQLDescribeParam`, o `07009` da posicao fora da faixa e o
  `07002` da ligacao que falta). A NAO MEDIDA e a volta de `WHERE id = ?`,
  pelo motivo da secao 2.1.
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

### A cifra do fio, de ponta a ponta

`bancada/odbc/prova-cifra.py` sobe um phxsqld PROPRIO com `cifra_fio.exigir:
true` — um servidor que recusa todo pedido em claro — e prova o driver nos dois
sentidos, contra ele:

```bash
cargo build --release && cargo build --release -p phxsql-odbc
python3 bancada/odbc/prova-cifra.py
```

* **com a cifra** (`CIFRA=1;CHAVE_DO_FIO=<pino>`): o `SQLDriverConnect` fecha o
  aperto, o login vai por dentro do tunel e o `SELECT COUNT(*)` responde `3`;
* **defeito reposto** (a mesma receita SEM a cifra): a conexao e recusada, e o
  diagnostico nomeia o motivo ("este servidor exige a cifra do fio") — o driver
  velho, que fala claro, esbarrando no `exigir`;
* **pino errado**: a cifra liga, mas a chave apresentada nao e a pinada, e o
  aperto cai no cliente sem vazar material de chave no diagnostico.

Os dados sao montados POR DENTRO do tunel com o cliente Noise independente da
`bancada/cifra-do-fio/prova.py` (Python puro), porque com `exigir: true` nem a
montagem pode falar claro. E o aperto tem prova em Rust tambem, sem gerenciador
de driver: `conexao::testes::aperto_pelo_canal_fecha_e_fala_por_dentro` sobe um
servidor de aperto em processo (so o `fio` do core) e confere que o `pedir`
viaja selado; `pino_errado_derruba_o_aperto_sem_vazar_chave` e o par do pino.

Registrado (2026-09-08, Linux x86_64, unixODBC 2.3.12): as tres conferencias
passam pelo `ctypes` e pelo `isql -k` de verdade — `SELECT COUNT(*)` devolve
`3` pelo tunel, e o claro cai com "Could not SQLDriverConnect".

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
* **`SQL_CLOSE` NAO desfaz a preparacao — e a prova nova quebrou nisso.** O
  passo 7c reusou o comando do passo 7b e leu `SQL_SUCCESS` onde esperava
  `HY010` de «SQLNumParams sem SQLPrepare»: o texto preparado em 7b continuava
  vivo, sem `?` nenhum. E o que a especificacao manda (fechar o cursor nao
  desprepara), o driver estava certo, e o defeito era DA PROVA — a mesma
  familia do `c_int` acima. O passo 7c aloca comando proprio desde entao.
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
  src/parametro.rs           contagem dos `?` e leitura das ligacoes
  src/registro.rs            handles como chaves de mapa (nunca ponteiro cru)
  src/texto.rs               truncamento e strings pela fronteira C
bancada/odbc/montar-dados.py o banco conhecido da prova
bancada/odbc/prova-abi.py    a prova pela ABI (dlopen + ctypes), 86 conferencias
bancada/odbc/prova-cifra.py  a cifra de ponta a ponta contra um servidor exigir:true
docs/ODBC.md                 este documento
```
