# A camada SQL: o que ela precisa saber

**O passo 1 existe agora**, em `crates/phxsql-sql/`: analisador léxico,
analisador sintático de um `SELECT` simples e o tradutor dele para as operações
do protocolo. Não há executor e não há planejador — e a seção 4 continua sendo
o roteiro do que falta.

Este documento é o desenho de antes de escrever, e existe porque **três
pendências esperam a mesma coisa**: o driver ODBC/OLE DB (#7), o DBeaver (#122)
e o protocolo de fio do PostgreSQL(R). Uma camada, três destravadas.

## O que o crate já faz

```
SELECT ( * | COUNT(*) | coluna [AS apelido] {, ...} )
FROM   [database.] [schema.] tabela [[AS] apelido]
[WHERE coluna ( = | <> | < | <= | > | >= ) literal]
[ORDER BY coluna [ASC|DESC]]
[LIMIT n [OFFSET m]]
```

```bash
cargo run -p phxsql-sql --example traduzir -- "SELECT * FROM matriz.estoque"
```

`traduzir(&Selecao, &[IndiceInfo], database)` devolve um `Plano`: a operação
(`varrer` ou `buscar`), o pedido pronto em JSON, o que o cliente ainda tem de
fazer com a resposta (a projeção, que é do cliente porque o protocolo sempre
devolve a linha inteira) e as **notas** — o que o tradutor decidiu e por quê.

O `FROM matriz.estoque` fecha o lado SQL do pedido #83: o endereçamento já
funcionava em toda operação, e faltava alguém escrever isso e chegar lá.

**O que não tem substrato recusa dizendo o nome da cláusula.** Um `WHERE cidade
= 'X'` sem índice em `cidade` **não** vira uma varredura com o filtro esquecido
no caminho: o `varrer` não filtra, e aceitar calado devolveria a tabela inteira
como se fosse a resposta. O mesmo para `ORDER BY` sem índice, `AND`, `LIKE`,
`IN`, `BETWEEN`, `IS NULL`, `DISTINCT`, `GROUP BY`, `JOIN`, os agregados que
não são `COUNT(*)`, e `BEGIN`/`COMMIT`/`ROLLBACK`.

**O que ainda NÃO está ligado:** o servidor não tem operação `sql`. O crate
traduz texto em pedido; ligar isso ao despachar e à tela de consulta é a
próxima rodada, e é pequena — mas não está feita, e dizer o contrário seria
inventar.

---

Ele também existe por um motivo mais imediato: o `BULKINSERT` entrou no
protocolo, e o motor SQL vai ter de conhecê-lo **como comando, e não como
nome de tabela**. Escrever isso agora é mais barato do que descobrir depois.

---

## 1. O que já está pronto embaixo

A camada SQL não precisa inventar mecanismo nenhum. Tudo que um `SELECT`,
`INSERT`, `UPDATE` ou `DELETE` precisa **já é uma operação do protocolo**, e
está medida:

| SQL | operação de hoje | o que ela já faz |
|---|---|---|
| `SELECT … WHERE chave = ?` | `buscar` | desce o índice, devolve rowids |
| `SELECT … LIMIT n OFFSET m` | `varrer` com `pular` | bissecta pelo rownum quando dá |
| `SELECT … ORDER BY col` | `varrer` com `indice` | a ordem sai do `.ndx`, sem ordenar nada |
| `SELECT count(*)` | `varrer` conta em O(1) | dois campos do cabeçalho |
| `INSERT` | `inserir` | 15,9 µs por linha, dois índices — **uma linha por `INSERT`, ver §6** |
| `INSERT` de muitas | `inserir_lote` | 16,3× a linha a linha |
| `UPDATE … WHERE chave = ?` | `buscar` → `ler` → `atualizar` | **três passos, ver §6**; com `versao`, recusa o conflito |
| `DELETE … WHERE chave = ?` | `buscar` → `ler` → `excluir` | suave por padrão, física a pedido — **ver §6** |
| `JOIN` | `juntar` | sete formas, com as três armadilhas documentadas |
| `UNION` | `unir` | distinta e todas |
| `GROUP BY` cruzado | `pivotar` | a tabulação cruzada |
| `CREATE TABLE` | `criar_tabela` | colunas, índices, partição |
| `information_schema` | `sistabelas` / `siscolunas` | o catálogo |

**O trabalho é de tradução, não de motor.** É por isso que ele cabe: o parser
vira chamadas ao que já existe e já tem teste.

---

## 2. `BULKINSERT`, e por que ele é diferente

```sql
BULKINSERT(true);
  INSERT INTO Clientes …    -- muitas, em laço ou em lote
BULKINSERT(false);
```

Hoje é `{"op":"bulkinsert","database":…,"tabela":…,"ligado":true}`, pela porta
de dados. Ver a seção correspondente no `MANUAL.txt`.

Três coisas que o parser **não pode** tratar como açúcar sintático:

1. **É palavra reservada.** Não pode existir tabela, coluna ou apelido chamado
   `BULKINSERT`. A reserva ficou **no parser**, em `RESERVADAS_DO_MOTOR`, e não
   no `validar_nome` do `catalogo.rs`: reservar palavra no motor quebraria banco
   de quem já tem a tabela. Assim ela só custa a quem escreve SQL — e `"BULKINSERT"`
   entre aspas duplas volta a ser um nome, para quem já tem essa tabela.

2. **É de sessão, não de instrução.** O estado vive na conexão, entre
   comandos — como uma transação. Um driver que multiplexa várias sessões
   lógicas no mesmo soquete **quebraria a exclusividade**, porque a reserva
   morre amarrada à *conexão*. O driver tem de garantir uma conexão por sessão
   enquanto houver reserva aberta, ou recusar o comando.

3. **Ele fala com o cliente sobre erro passageiro.** Quem esbarrar numa tabela
   reservada recebe `EM_CARGA` (4002) com `repetir: true`. No `SQLSTATE` do
   ODBC isso mapeia para a família de *serialization failure* / *lock not
   available* — **não** para «acesso negado». Errar esse mapeamento faz o
   cliente desistir de algo que ia funcionar em dez segundos.

### O vocabulário que o motor precisa reservar

Palavras que já têm significado no PhxSql e não podem virar identificador
quando o parser existir:

```
BULKINSERT      reserva a tabela para carga (exclusivo, de sessão)
ROWNUM          coluna de sistema: a ordem de digitação
SOFTDELETED     coluna de sistema: a marca de excluído
```

As três já são nomes tomados **hoje**, no motor. As duas últimas o esquema já
protege, e continuam colunas legítimas num `SELECT` — quem as reserva é o
esquema, não a linguagem. A primeira é a única reservada pelo parser.

---

## 2b. `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`

Também são comandos de **sessão**, e pelo mesmo motivo do `BULKINSERT`: a
transação pertence à **conexão**, não ao texto do comando. Um driver que
multiplexa conexões quebra a exclusividade sem avisar — e o servidor recusa
quando a ligação é zero (a porta web, a ponte MCP, o job agendado).

```sql
BEGIN;                       -- e também BEGIN TRANSACTION, BEGIN WORK,
START TRANSACTION;           --   START TRANSACTION
COMMIT;                      -- COMMIT WORK também
ROLLBACK;

SAVEPOINT antes_do_lote;
ROLLBACK TO SAVEPOINT antes_do_lote;   -- a palavra SAVEPOINT é facultativa
RELEASE SAVEPOINT antes_do_lote;
```

E a abertura declarada, que é o que paga pela trava de linha:

```sql
BEGIN TRANSACTION
  SCOPE (clientes, pedidos, pediditens, estoque)
  SCOPE MODE STRICT          -- DYNAMIC é o padrão
  TIMEOUT 5s
  LOCK TIMEOUT 500ms
  STATEMENT TIMEOUT 2s
  LOCK MODE AUTO;            -- AUTO, ROW, TABLE ou EXCLUSIVE
```

**As cláusulas não têm ordem.** Ordem obrigatória é uma regra que existe para
facilitar o analisador, e o preço dela é pago por quem digita.

### Três coisas que a integração ensinou

**1. O detector de transação vem ANTES do de rotina, e isso é medido.** O
detector de rotina analisa o texto inteiro pelo léxico comum, e o léxico recusa
`500ms` — número colado em identificador. Ele erra com `?` antes de o detector
de transação ser consultado, e um `LOCK TIMEOUT 500ms` nunca chegaria lá.
Inverter é seguro e não por sorte: o único `BEGIN` que **não** abre transação é
o do corpo de um `CREATE PROCEDURE p() BEGIN … END`, e esse texto começa por
`CREATE`.

**2. `500ms` não pode virar `500s`.** A unidade é separada do número só dentro
de um comando de transação, e só para os quatro sufixos de tempo (`ms`, `s`,
`m`, `h`). `SELECT 5x` continua sendo o erro que sempre foi, e o resto da
linguagem não muda um caractere. Há teste: quem lê `500ms` como 500 segundos
erra por mil vezes, e erra calado.

**3. Sobra depois do comando é erro.** `COMMIT AND CHAIN` não existe aqui, e
aceitar calado devolveria um `COMMIT` simples a quem pediu encadeamento.

### O que o `sintaxe.rs` faz com eles

Nada — e é de propósito. Eles não são consulta: não têm `FROM`, não produzem
linha e não dependem de esquema nenhum. Quem cai no analisador de `SELECT` com
um `BEGIN` na mão escreveu uma forma que nenhum dos dois entende, e a recusa
**lista as formas que existem** em vez de dizer que a transação não existe.

O desenho inteiro, o nível de isolamento pelo nome certo e o que foi recusado
estão em [TRANSACOES.md](TRANSACOES.md).

---

## 2c. `SHOW … SETTINGS` e `ALTER … SET` — as diretivas

```
SHOW ( SERVER | DATABASE <banco> | TABLE <tabela> | CONNECTION ) SETTINGS

ALTER ( SERVER | DATABASE <banco> | TABLE <tabela> | CONNECTION )
  SET <campo> = <valor>
  [ MOTIVO '<texto>' ]
```

```sql
SHOW SERVER SETTINGS;
SHOW DATABASE erp SETTINGS;
SHOW TABLE clientes SETTINGS;
SHOW CONNECTION SETTINGS;

ALTER SERVER   SET max_linhas = 500 MOTIVO 'pico de exportacao';
ALTER SERVER   SET recursos.cache_paginas = 4096;
ALTER SERVER   SET somente_leitura = TRUE;
ALTER DATABASE erp SET comandos_proibidos = (reindexar, excluir_tabela);
```

`<valor>` aceita `TRUE`/`FALSE` (com `ON`/`OFF`, `YES`/`NO`, `SIM`/`NAO` como
sinônimos), número (inclusive negativo), texto entre aspas, palavra solta (é o
`recursos.durabilidade = por_lote`) e lista entre parênteses. O campo pode ter
seção — `recursos.cache_paginas` —, que é exatamente a forma que o
`config_gravar` já aceita. O escopo também se escreve em português: `SERVIDOR`,
`BANCO`, `TABELA`, `CONEXAO`. O `MOTIVO '…'` (ou `COMMENT '…'`) é opcional e
alimenta o **diário administrativo** (`diretivas.log`, `FORMATO.md` §18).

Como o `BULKINSERT` e as transações, **não passam pelo `sintaxe.rs`**: não são
consulta, não têm `FROM`, não produzem linha. São comandos de administração, e
viram os pedidos `diretivas` e `diretiva_gravar` do protocolo — os dois exigem
`administrar`.

**`ALTER SERVER SET` não tem caminho próprio de gravação:** ele monta
`{"op":"config_gravar","campos":{…}}` e chama a mesma função — mesmo portão,
mesma conferência de tipo, mesma gravação atômica do `config.json`, mesma
aplicação a quente. Só se grava o que está em `CAMPOS_EDITAVEIS`; `token`,
`usuarios`, `cifra` e `replicacao` continuam sendo edição do arquivo.

**`ALTER TABLE … SET` e `ALTER CONNECTION SET` recusam, e a recusa nomeia o
caminho que funciona** — `duplicate_check` é o `unico` do índice, declarado no
`criar_tabela`; `referential_integrity` é o `verificar` da chave, e ela **nasce
conferida**. O motivo de cada dispensa está em [DIRETIVAS.md](DIRETIVAS.md),
que traz o mapa de cada diretiva do HFSQL contra o motor de hoje.

**O que este bloco NÃO rouba:** `SHOW TRIGGERS`, `SHOW PROCEDURES` e `SHOW
PROCEDURE STATUS` continuam do detector de rotinas. O de diretivas só reclama a
frase quando a palavra depois do `SHOW` é `SERVER`, `DATABASE`, `TABLE` ou
`CONNECTION`.

---

## 3. O que a camada SQL vai ter de resolver, e não tem embaixo

Honestidade sobre o tamanho do trabalho. A rodada de 08–09/2026 (itens 1–9,
`docs/propostas/comparativo-19.md`) fechou expressão no `WHERE`, `GROUP BY`,
subconsulta, CTE e junção — a §7 conta a gramática e o JSON de cada um. O que
**continua** sem substrato:

- **Planejador de índice.** Escolher *qual* índice usar quando há dois
  candidatos de igualdade continua sendo de quem chama — o primeiro declarado
  vence, e o plano diz isso na nota.
- **Janela além de `ROW_NUMBER`.** `RANK`, `DENSE_RANK`, `SUM() OVER (...)` e
  companhia recusam nomeando — só `ROW_NUMBER() OVER (...)` tem substrato.
- **`RIGHT`/`FULL`/`CROSS JOIN`.** Recusam nomeando; a direita se escreve
  trocando os lados.
- **Correlação e `EXISTS`.** Uma subconsulta (`IN` ou escalar) que cita coluna
  de fora, ou um `EXISTS (...)`, exigiriam rodar a subconsulta por LINHA da
  consulta de fora — não existe.
- **`WITH RECURSIVE` e mais de uma CTE.** Só uma CTE, não recursiva.
- **`UNION`.** Não há.
- **`COUNT(coluna)`** sem `DISTINCT` e sem `*`. Contar só os não-nulos de uma
  coluna é outro acumulador, que esta camada não construiu — `COUNT(*)` e
  `COUNT(DISTINCT coluna)` têm substrato, `COUNT(coluna)` sozinho recusa.
- **`COUNT(*)`/`GROUP BY` sobre visão.** `FROM v_c` vira `consultar` (filtra e
  projeta); agregar sobre o resultado de uma visão ainda não compõe — quem
  precisa disso escreve `SELECT COUNT(*) FROM (SELECT * FROM v_c) AS x`.
- **Nível de isolamento.** A transação **existe** desde o pedido 162 — esta
  linha dizia «não há» e contradizia a §2 deste mesmo documento, que descreve o
  detector dela. O que não existe é o que fica **acima** do `READ COMMITTED`:
  medido em `docs/ACID.md`, leitura não repetível, fantasma e *write skew*
  acontecem, e a leitura suja **não**. Quem escrever `SET TRANSACTION ISOLATION
  LEVEL SERIALIZABLE` precisa receber uma recusa que diz o nível real, e não um
  `Ok` que promete o que o motor não faz. `docs/TRANSACOES.md`, `docs/ACID.md`
  §5, `docs/SOMBRA.md`.

**O `BULKINSERT` não é transação, e o documento tem de dizer isso alto.** Ele dá
*exclusividade* e *uma sincronização no fim*. Ele **não** desfaz: se a carga
parar no meio, o que entrou está gravado. Quem lê `BULKINSERT(true)` esperando
`BEGIN` vai se surpreender no pior dia.

---

## 4. Por onde começar, quando começar

Na ordem em que cada passo destrava alguém:

1. ~~**`SELECT` de uma tabela**, com `WHERE` de igualdade e `LIMIT/OFFSET`.~~
   ~~**Feito** em `crates/phxsql-sql/`, menos a ligação com o servidor.~~
   **Ligado**: existe `{"op":"sql"}` no protocolo, e a seção 5 conta o que a
   ligação encontrou.
2. ~~**`INSERT`/`UPDATE`/`DELETE`** por chave primária. Fecha o CRUD.~~
   **Feito** em 08/09/2026 — `crates/phxsql-sql/src/dml.rs`, ligado à op `sql`;
   a seção 6 conta o que a ligação ensinou, e não era pouco.
3. **`BULKINSERT`** e o catálogo (`information_schema`). Fecha a carga e a
   introspecção.
4. ~~**`JOIN`**, mapeando para o `juntar` que já existe.~~ **Feito** em
   08–09/2026 (item 8, §7) — `INNER`/`LEFT [OUTER] JOIN`; `RIGHT`/`FULL`/
   `CROSS` recusam nomeando.
5. ~~**Expressão e planejador** — o trabalho de verdade, e o único que não é
   tradução.~~ **A expressão saiu** (item 2, §7): o `WHERE`/`HAVING`/`ON` que
   não é a forma simples vira texto normalizado, e quem avalia é
   `phxsql_core::expressao`, do lado do motor. **O planejador de índice
   continua não existindo** — ver §3.

Os quatro primeiros já traduziam coisa medida e testada quando entraram. A
tradução da composição (itens 4–9, §7) seguiu o mesmo molde: recusa nomeada
para o que não tem substrato, prova real nos dois sentidos por item.

---

## 5. A op `sql`, e o que ligar a crate ao servidor ensinou

O passo 1 do roteiro acima está fechado: existe `{"op":"sql"}` no protocolo.

```json
{"op":"sql","database":"loja","texto":"SELECT nome AS quem FROM clientes LIMIT 10"}
```

A resposta é a da operação traduzida, com três campos a mais na frente:

```json
{"sql":"SELECT nome AS quem …","op":"varrer",
 "notas":["sem ORDER BY a ordem e a de DIGITACAO …"],
 "colunas":["quem"],
 "registros":3,"devolvidas":3,"linhas":[{"quem":"Adriano"}]}
```

`COUNT(*)` devolve `contagem` em vez de linhas. O campo `sql` é aceito como
sinônimo de `texto`, porque é o nome que um driver escreveria.

### O portão continua sendo UM, e é por isso que custa um `esquema` a mais

A op `sql` **não abre tabela nenhuma**. Ela faz duas coisas, e as duas passam
pelo `executar_derivado`, que é o mesmo portão do pedido que chega pela rede:

1. pede o `esquema` da tabela do `FROM` — e é dali que saem os índices que o
   tradutor precisa para escolher entre `buscar` e `varrer`;
2. executa o `varrer` ou o `buscar` que a tradução produziu.

Abrir a tabela aqui dentro seria mais rápido, e seria o **segundo caminho até o
dado** — o que sempre esquece uma conferência. O projeto já pagou esse preço
uma vez: `juntar` e `unir` foram a porta dos fundos porque as tabelas delas não
passavam pelo campo `tabela` que o portão olha.

A tradução resolve isso **pelo outro lado**: ela *produz* o campo que o portão
já sabe olhar, em vez de pedir um portão novo. `SELECT * FROM folha` de quem
não pode ler a folha para no passo 1, com exatamente o mesmo erro de um
`{"op":"varrer","tabela":"folha"}`. O teste que trava isso é
`o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` — e com a chamada trocada
por um `executar` direto ele devolve a linha da folha, que é a prova de que ele
mede o que promete.

Vale para a **política** também: um servidor que proíbe `varrer` não pode ser
varrido escrevendo SELECT. O `politica_do_pedido` roda contra a operação
**traduzida**, e não contra a palavra `sql`.

### A permissão de fora é `ler`, e ela só aperta

`Atividade::da_operacao("sql")` é `Ler`. Não dá para ser `None`: a exigência de
login do `despachar` é justamente *«esta operação pede alguma atividade»*, e uma
op sem atividade seria chamável sem login num servidor com cadastro.

O preço, escrito para não surpreender ninguém: **quem só tem direito por TABELA,
e nenhum na base, para no portão de fora.** O portão de fora lê o campo
`database` do envelope e não tem como saber a tabela, que está dentro do texto
do SQL. Consertar isso exigiria um portão que interpreta linguagem — e portão
que interpreta linguagem é portão que erra. O de dentro continua conferindo a
tabela de verdade, então o de fora só aperta, nunca afrouxa.

### O que a ligação encontrou, e que ler o código não mostraria

**`WHERE id = 2` não funcionava contra uma coluna `Int4`.** Os testes da
crate passavam, e o teste de tradução também: o plano saía certinho, com
`"chave":["2"]`. O motor é que recusava, com `esperado inteiro, recebido
Texto("2")`.

Nenhum dos dois lados estava errado sozinho. O tradutor guarda todo literal
numérico como **texto** de propósito — é a mesma razão que faz o `Decimal` do
protocolo *exigir* texto: `f64` não representa `1500.00` exatamente, e
converter para número aqui desfaria dentro do tradutor a garantia que ele
existe para preservar. E o `json_para_valor` exigia número para coluna inteira
porque nunca ninguém tinha mandado texto.

A correção é **alargar**, e do lado do motor: coluna inteira passa a aceitar
inteiro escrito como texto. É o que o driver ODBC e o protocolo do
PostgreSQL(R) vão precisar de qualquer jeito, porque neles **todo** parâmetro
chega como texto. Quem manda número continua exatamente como antes — e o teste
que mais importa é esse, `numero_continua_valendo_exatamente_como_antes`.
Texto que não é número continua recusado com o mesmo erro, e o `Decimal`
continua recusando número: alargar não pode virar engolir.

**A lição:** *o tradutor testado contra o tradutor não prova a ligação.* Os 44
testes comparavam o plano com o plano esperado; o que faltava era alguém
executar o plano contra o motor. É a mesma lição do soquete, num degrau acima.

### Endereço de três partes, e o que ele não faz

`FROM banco.schema.tabela` escolhe o banco; `FROM schema.tabela` **não** — duas
partes são schema e tabela, e o banco continua sendo o do envelope. Isso já era
assim na crate, e o teste `o_banco_do_from_e_o_banco_da_permissao` trava a
consequência que importa: quando o SELECT escolhe o banco, é contra **esse**
banco que a permissão é conferida, e não contra o do envelope. Sem isso o campo
`database` do pedido seria enfeite.

### O que a op `sql` ainda não faz

Os três verbos de escrita por chave já entram (§6). O que continua fora é
tudo o que a seção 3 lista, e pela mesma razão: não há substrato. O que muda é
que agora a recusa chega ao cliente pela rede, com o nome da cláusula e a
coluna do texto — `SQL, coluna 10: …`. Um `WHERE cidade = 'Blumenau'` sem
índice em `cidade` recusa dizendo **quais colunas têm índice**, em vez de virar
uma varredura com o filtro esquecido no caminho.

### E um defeito que só a tela mostrou: a contagem arrastava uma linha

`SELECT COUNT(*)` devolvia `contagem: 3` **e** um `linhas` com um registro
dentro. A tradução pede `max: 1` para ler o campo `registros` do cabeçalho em
O(1) — a linha que vem junto é efeito colateral do caminho, não a resposta.

No JSON o campo extra passa despercebido, e o teste que existia olhava só a
contagem. No console ele vira uma tabela inteira embaixo do número, e quem olha
não tem como saber se aquela linha significa alguma coisa. A resposta de uma
contagem passa a carregar `contagem` e `registros`, e mais nada — nem os campos
que descrevem uma página que ninguém pediu.

**A lição:** *o formato só erra na tela.* O campo estava certo no JSON, e era
por isso que ninguém via.

---

## 6. `INSERT`, `UPDATE` e `DELETE` por chave — e por que `UPDATE` não é tradução direta

Entraram em 08/09/2026, em `crates/phxsql-sql/src/dml.rs`, ligados à op `sql`.
A gramática:

```text
INSERT INTO [database.] [schema.] tabela (coluna {, coluna}) VALUES (literal {, literal})
UPDATE      [database.] [schema.] tabela SET coluna = literal {, coluna = literal}
            WHERE coluna = literal
DELETE FROM [database.] [schema.] tabela WHERE coluna = literal
```

### A tabela da §1 estava certa e enganava

Ela mapeia `UPDATE` para `atualizar`, e a primeira leitura sugere trocar o
verbo pelo nome da operação. **Não basta**, e o motivo está em
`crates/phxsql-server/src/valores.rs`, no `json_para_linha`: o `atualizar`
recebe a linha **inteira**, e *coluna ausente entra como NULL*. Um
`UPDATE t SET nome = 'x' WHERE id = 5` traduzido direto mandaria só `nome` —
e zeraria `cidade`, `telefone` e o resto, sem erro nenhum. Isso só apareceu
lendo o caminho inteiro antes de escrever; o teste de tradução sozinho
passaria, porque o plano estaria «certo».

Por isso `UPDATE` e `DELETE` por chave são **três passos**, e o plano
(`PlanoDml`) diz isso em vez de esconder:

1. `buscar` no índice único acha o `rowid` da chave;
2. `ler` com `com_versao` traz a linha inteira e a `versao` dela;
3. `atualizar` grava a linha **mesclada** — a lida, com o `SET` por cima — ou
   `excluir` marca a linha; os dois levam a `versao` lida.

Quem executa os passos é o servidor (`executar_dml`), e cada um sai pelo
**mesmo** `executar_derivado` do `SELECT`: o portão continua um, lendo o
campo `tabela` do pedido traduzido. Não há caminho de escrita próprio, e é
assim que a porta dos fundos não nasce.

### A `versao` vai junto por decisão

Os três passos abrem uma janela entre ler e gravar, e a janela de conflito de
escrita (pedido 123) existe para isso. *Guarda nova entra pedida, não imposta*
— e a camada SQL é cliente **novo**, então ela pede: quem gravar a linha entre
o passo 2 e o 3 faz o motor recusar em vez de ser sobrescrito em silêncio. O
`buscar` não expõe `versao`; é por isso que o `ler` entra no meio.

### Só chave única

O `WHERE` tem de cair num índice **único** (ou primário) de uma coluna. Um
índice comum acha a linha — e é recusado por isso mesmo: um `UPDATE` que
alcança N linhas sem dizer quantas é a resposta errada com cara de certa, a
mesma recusa que o `SELECT` faz com a varredura pela metade. Sem `WHERE`,
recusa dizendo que mudaria a tabela inteira; com faixa (`>`, `<>`…), recusa
dizendo que o passo por chave desce até uma chave igual.

### Uma linha por `INSERT`

`VALUES (…), (…)` viraria `inserir_lote`, e `inserir_lote` **não empilha** numa
transação aberta (`OPS_EMPILHAVEIS`). Um `BEGIN; INSERT … VALUES (a), (b);
ROLLBACK` gravaria as duas por fora da transação, com cara de ter desfeito. A
recusa nomeia o caminho de carga: `BULKINSERT` e a operação `inserir_lote`,
fora de transação. O `INSERT` de uma linha vira `inserir`, que empilha — e há
teste de que ele empilha.

### O que recusa pelo nome

`INSERT` sem lista de colunas; `INSERT … SELECT`; `DEFAULT`; expressão no
`SET` ou no `VALUES`; coluna repetida; `SET` em coluna de sistema
(`softdeleted`, `rownum`); `UPDATE`/`DELETE` sem `WHERE`, por faixa, com
`AND`, ou sobre coluna sem índice único. Cada um com a frase que diz o que
faltou — os casos estão em `dml.rs`, teste `o_que_falta_recusa_pelo_nome`.
E `SET`, `VALUES` e `INTO` entraram nas cláusulas da gramática por um motivo
concreto: o endereço aceita apelido sem `AS`, e `UPDATE t SET` leria `SET`
como apelido da tabela.

### A resposta

`{"sql", "op", "notas", "afetadas"}`, mais o que a operação devolveu e vale
repetir: `rowid` e `registros` no `inserir`; `rowid` e a `versao` nova no
`atualizar`; `rowid`, `modo`, `reversivel` e `na_lixeira` no `excluir`.
Chave que não existe devolve `afetadas: 0` — não é erro. O console da tela
mostra `afetadas` quando não há `devolvidas`.

### As provas, nos dois sentidos

- `update_por_chave_muda_so_a_coluna_do_set` — a prova real da mescla: tire-a
  do `pedido_de_atualizar` e `cidade` volta NULL.
- `os_pedidos_dos_passos_levam_a_linha_mesclada_e_a_versao` — os pedidos dos
  passos são funções puras; tire a linha que põe a `versao` e ele falha.
- `o_dml_pelo_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada` — quem não
  pode gravar na folha não grava nela escrevendo SQL.
- `o_insert_pelo_sql_empilha_na_transacao` — o `INSERT` fica na conexão até o
  `COMMIT`, e o `ROLLBACK` não queima slot.
- `delete_por_chave_e_suave_e_some_da_lista` — some do `varrer`, fica no
  arquivo marcada, e o índice continua a achá-la: é o que `restaurar` precisa.
  A primeira versão deste teste conferia o `buscar`, e falhou: eu tinha
  medido o observável errado, não o comportamento.

O status «ok / planejado» das três atividades pela camada SQL sai da bancada
de gestão (`docs/GESTAO.md`, gerado), e não deste texto.

### O que continua fora, e o que não foi medido

Fora por desenho: `UPDATE`/`DELETE` por índice não único ou por faixa, várias
linhas por `INSERT`, expressão, `DEFAULT`, `INSERT … SELECT`. **Não medido
nesta rodada**: `UPDATE`/`DELETE` de uma linha nascida na *mesma* transação
aberta — o `buscar` do passo 1 desce o índice, e a linha empilhada ainda não
está nele; o desfecho esperado é `afetadas: 0`, e isso precisa de teste antes
de virar promessa.

**`ON CONFLICT`/`ON DUPLICATE KEY UPDATE` entraram em 08–09/2026** (item 7,
§7) — `INSERT` continua sendo uma linha só, mas ganhou o que fazer quando ela
já existe. O SET do upsert é a MESMA gramática do `UPDATE` (só literal;
`excluded.coluna` e expressão recusam nomeando).

---

## 7. Os itens 1–9: parâmetro, expressão, agregado, composição, junção e visão

Rodada de 08–09/2026, contrato em `docs/propostas/comparativo-19.md` (os
itens 8 e 9 — junção e subconsulta escalar — foram um acréscimo do dono a
meio da rodada, e entram no mesmo `consultar`). Cada item abaixo diz a
gramática que `crates/phxsql-sql/` aceita e o JSON que ela produz — não a
contagem de teste, que envelhece; rode `cargo test -p phxsql-sql` para o
número do dia.

### 1. Parâmetro `?`

```text
analisar_comando_com(texto, parametros: &[Json]) -> Result<Comando>
```

Cada `?` é contado da esquerda (zero-based) e resolvido para o literal de
`parametros[n]` **no léxico**, antes da sintaxe rodar — nunca por
substituição de texto, que reabriria a porta de injeção que o `?` existe
para fechar. Número em `Json` vira `Numero` com o texto que `Json::escrever`
já produz para ele; texto vira `Texto`; nulo e booleano viram a PALAVRA
(`NULL`/`TRUE`/`FALSE`). Contagem diferente recusa nomeando os dois números:
*"vieram N parametros e o comando tem M `?`"*. `analisar_comando(texto)` é
`analisar_comando_com(texto, &[])`.

### 2. Expressão no `WHERE`

`coluna op literal` sozinho continua a forma **Simples** — a única que desce
índice (`buscar`). Qualquer outra coisa (`AND`/`OR`, aritmética, função,
`IN` de lista, `BETWEEN`, `LIKE`, `IS [NOT] NULL`, parênteses, coluna contra
coluna) vira **Expressão**: o texto normalizado dos tokens do `WHERE`, que o
`varrer` recebe no campo `"expressao"` — quem avalia é
`phxsql_core::expressao`, nunca esta camada.

```json
{"op": "varrer", "database": "b", "tabela": "c",
 "expressao": "preco * 1.1 > 100 AND cidade = 'Blumenau'"}
```

Nome qualificado (`c.uf`) normaliza **sem espaço** ao redor do ponto — é a
única exceção à regra "um espaço entre tokens", porque o avaliador do core só
lê `p.id` como um token quando ele vem grudado.

### 3. `GROUP BY` e agregados

```text
SELECT coluna {, coluna} , FUNCAO(coluna) [AS apelido] {, ...}
FROM tabela [WHERE ...] [GROUP BY coluna {, coluna}] [HAVING expr]
[ORDER BY coluna [DESC] {, coluna [DESC]}] [LIMIT n]
```

`FUNCAO` ∈ `COUNT(*)`, `COUNT(DISTINCT coluna)`, `SUM`, `AVG`, `MIN`, `MAX`.
Vira `agrupar`:

```json
{"op": "agrupar", "database": "b", "tabela": "c",
 "por": ["cidade"],
 "agregados": [{"funcao": "contagem", "apelido": "contagem"},
               {"funcao": "soma", "coluna": "preco", "apelido": "total"}],
 "onde": [], "expressao": "", "tendo": "total > 1",
 "ordem": [{"coluna": "total", "desc": true}], "max": 1000}
```

`onde` (filtro simples) e `expressao` (item 2) são alternativos — o campo que
não vale fica vazio, os dois sempre presentes. `por` vazio = um grupo só
(`SELECT SUM(x) FROM t` sem `GROUP BY`). Apelido padrão: `contagem` sozinho,
os outros `funcao_coluna` (`soma_preco`). Coluna projetada fora do
`GROUP BY` e que não é agregado recusa nomeando, **na declaração**. `COUNT(*)`
sozinho sem `GROUP BY`/`HAVING` continua no caminho rápido de sempre
(`registros` do cabeçalho) — a única exceção é `COUNT(*)` com `WHERE` em
forma de expressão, que precisa varrer para contar e por isso também vira
`agrupar`. `ORDER BY` de várias colunas só vale aqui — o caminho simples
continua limitado a uma (exige índice). `OFFSET` com `GROUP BY` recusa: o
contrato de `agrupar` não tem `"pular"`.

### 4. `WITH`, subconsulta no `FROM`, `IN (SELECT …)`

```text
[WITH nome AS (SELECT ...)]
SELECT ( * | coluna [AS apelido] {, ...} )
FROM ( tabela [[AS] apelido] | nome_da_cte | (SELECT ...) AS apelido )
[WHERE ...] [ORDER BY ...] [LIMIT n [OFFSET m]]
```

Uma CTE só, não recursiva — duas ou `WITH RECURSIVE` recusam nomeando.
`WHERE coluna IN (SELECT campo FROM ...)`, ocupando o conjunto INTEIRO de um
`AND` de nível superior, vira `em`. Tudo vira `consultar`:

```json
{"op": "consultar", "database": "b",
 "de": {"op": "varrer", "tabela": "c"},
 "em": [{"coluna": "id", "de": {"op": "varrer", "tabela": "c"}, "campo": "id"}],
 "expressao": "preco > 10",
 "colunas": ["id", "nome"],
 "ordem": [{"coluna": "id", "desc": false}], "pular": 0, "max": 1000}
```

`de` (e cada `em`) é traduzido por quem chama, via um **resolvedor**
(`Resolvedor<'a> = dyn FnMut(&Selecao, &str) -> Result<Plano>`) — cada pedaço
roda pelo MESMO portão de permissão de qualquer pedido; é isto que faz da
composição uma composição, e não uma porta dos fundos (há prova com tabela
negada dentro do `IN`). Subconsulta correlacionada (cita coluna que não é a
dela mesma) e `EXISTS` recusam nomeando. `(SELECT` em qualquer lugar do
comando desvia para esta gramática — mesmo quando a forma ainda não existe
(escalar de um lado errado, `EXISTS`), porque cair aqui dá recusa nomeada em
vez de virar texto de expressão que o motor não lê.

### 5. `ROW_NUMBER() OVER (...)`

```text
ROW_NUMBER() OVER ([PARTITION BY coluna {, coluna}] [ORDER BY coluna [DESC] {, ...}]) [AS apelido]
```

Só essa função — `RANK`, `DENSE_RANK`, `SUM() OVER` etc. recusam nomeando. O
apelido (dado, ou o padrão `row_number`) entra em `consultar.janela` **e**
como uma coluna comum em `colunas`, na posição em que a chamada apareceu:

```json
{"janela": [{"funcao": "row_number", "particao": ["cidade"],
             "ordem": [{"coluna": "id", "desc": false}], "apelido": "n"}],
 "colunas": ["id", "nome", "n"]}
```

### 6. `CREATE VIEW` / `DROP VIEW`

```text
CREATE VIEW nome AS SELECT ...
DROP VIEW nome
```

`sql` é o texto ORIGINAL do `SELECT` (por posição de caractere, nunca
reconstruído dos tokens) e é analisado uma vez na hora do `CREATE` — para
recusar cedo o que esta camada não vai entender no primeiro uso. Vira
`criar_visao`/`excluir_visao`:

```json
{"op": "criar_visao", "database": "b", "nome": "v_c", "sql": "SELECT * FROM c"}
{"op": "excluir_visao", "database": "b", "nome": "v_c"}
```

`planejar_sobre(selecao, plano_de_dentro)` é o gancho para o servidor
resolver `FROM v_c`: ele reanalisa e traduz o SQL da visão (com os índices da
tabela QUE ELA USA) e passa o pedido pronto; esta função só aplica por cima o
resto do `SELECT` de fora (`WHERE`/colunas/`ORDER BY`/`LIMIT`/`OFFSET`).
`COUNT(*)`/`GROUP BY` sobre visão recusam nomeando — ver §3.

### 7. `INSERT ... ON CONFLICT` / `ON DUPLICATE KEY UPDATE`

```text
INSERT INTO t (...) VALUES (...)
  ON CONFLICT [(coluna)] DO NOTHING | DO UPDATE SET coluna = literal {, ...}
INSERT INTO t (...) VALUES (...) ON DUPLICATE KEY UPDATE coluna = literal {, ...}
```

```json
{"op": "inserir", "database": "b", "tabela": "c", "valores": {"id": "1"},
 "se_existir": "atualizar", "indice": "porCpf",
 "atualizar": {"nome": "B"}}
```

A coluna do `ON CONFLICT` vira NOME DE ÍNDICE (o protocolo pede índice, o SQL
nomeia coluna) pelo mesmo critério do `UPDATE`/`DELETE` por chave — só índice
ÚNICO de uma coluna serve, ambíguo recusa nomeando os candidatos.
`ON DUPLICATE KEY UPDATE` nunca nomeia índice: o servidor escolhe a primária.
`excluded.coluna` e expressão no `SET` recusam nomeando.

### 8. `[INNER|LEFT] JOIN ... ON`

```text
FROM t1 [[AS] a1]
  ([INNER] | LEFT [OUTER]) JOIN ( t2 [[AS] a2] | (SELECT ...) AS a2 )
  ON coluna[.coluna] = coluna[.coluna] {AND ...}
  {JOIN ...}*
```

```json
{"de": {"op": "varrer", "tabela": "pedidos"}, "apelido": "p",
 "juntar": [{"de": {"op": "varrer", "tabela": "clientes"}, "apelido": "c",
             "tipo": "interno",
             "em": [{"esquerda": "p.cliente_id", "direita": "c.id"}]}]}
```

`apelido` (do `de` e de cada junção) só entra no pedido quando há `juntar` —
sem junção nenhuma coluna precisa de prefixo, e o padrão é o nome da tabela
quando não há `AS`. O `ON` se divide pelos `AND` de nível superior; cada
pedaço `coluna = coluna` vira um par em `em` (junção por espalhamento em
memória), e qualquer outra condição vira fragmento de texto, ANDado na
`expressao` de fora junto com o `WHERE` — o contrato não tem campo de
expressão por junção. `RIGHT`, `FULL` e `CROSS` recusam nomeando; `ON` sem
nenhuma igualdade também.

### 9. Subconsulta ESCALAR no `WHERE`

```text
coluna[.coluna] ( = | <> | < | <= | > | >= ) (SELECT ...)
```

Não correlacionada, devolvendo exatamente uma linha e uma coluna — o `campo`
é o nome dela (o apelido, dado ou padrão, quando é um agregado). Numerada
`sub_1`, `sub_2`... na ordem em que apareceu; a comparação é reescrita na
`expressao` de fora:

```json
{"escalar": [{"nome": "sub_1",
              "de": {"op": "agrupar", "tabela": "c",
                     "agregados": [{"funcao": "media", "coluna": "preco"}]},
              "campo": "media_preco"}],
 "expressao": "preco > sub_1"}
```

Correlação e `EXISTS` recusam nomeando, como no item 4.

## 8. Quem EXECUTA: as ops que o servidor ganhou, e a costura entre as duas

Os itens 1–9 acima são a metade que **traduz**. Esta seção é a outra: o que o
servidor passou a **executar**, e o que só apareceu quando as duas metades se
encontraram.

### O que o servidor executa agora

| a tradução produz | a op que executa | onde ela mora |
|---|---|---|
| `GROUP BY` / agregados | `agrupar` | `crate::agrupar` (o acumulador é o do `pivot.rs`, e não uma cópia) |
| `WITH`, subconsulta, `IN (SELECT …)`, `JOIN`, `ROW_NUMBER`, escalar | `consultar` | `crate::consultar` |
| `CREATE VIEW` / `DROP VIEW` | `criar_visao` / `excluir_visao` | `crate::visoes` (`visoes.json` por banco) |
| `INSERT … ON CONFLICT` | `inserir` com `se_existir` | `crate::upsert` (o mesmo do DbLink) |
| `WHERE` por expressão | `varrer`/`SelectMemory` com `expressao` | `phxsql_store::memoria::passa` |
| `?` | `sql` com `parametros` | `analisar_comando_com`, no léxico |

O contrato entre as duas frentes é **o pedido JSON**, e não a assinatura de
uma função: foi isso que permitiu o servidor montar o `consultar` de uma visão
à mão enquanto `planejar_sobre` não existia, e trocar por ela depois sem mexer
em mais nada.

### O `consultar` não é a porta dos fundos, e é a FORMA que garante

`crate::consultar` não abre arquivo, não conhece `Table` e não sabe o que é
uma tabela: ele recebe linhas já lidas, em JSON. Quem lê é
`linhas_do_sub_pedido`, e ele só sabe chamar `executar_derivado` — política,
portão de permissão e direito por coluna, o mesmo caminho de um pedido que
chega pela rede. Ler uma tabela por ali exigiria acrescentar um caminho de
leitura a um módulo que não tem nenhum, e isso aparece na revisão; espalhar o
portão por dentro da composição é que seria invisível.

São **quatro** os caminhos até uma tabela dentro de um `consultar` — o `de`, o
`de` de cada junção, o de cada `escalar` e o de cada `em` — e os quatro passam
pela mesma função. Há teste com tabela negada em cada um.

E a permissão entra **duas vezes** num `SELECT` composto, porque o
planejamento também lê: `planejar_consulta` pede o `esquema` de cada tabela
citada pelo `executar_derivado`, para pegar os índices que o tradutor precisa.
A primeira recusa antes de montar o plano; a segunda recusaria de qualquer
jeito.

### O `Decimal` do `consultar` compara como TEXTO — e a recusa diz isso

A linha chega ao `consultar` como o sub-pedido a devolveu, sem esquema. O
resolvedor decide pelo formato do JSON: número sem parte fracionária é
inteiro, com parte fracionária é real, texto é texto. Um `Decimal` viaja como
**texto** no protocolo inteiro (para não passar por um `f64`), então
`preco = '10.01'` funciona e `preco > 10` **recusa dizendo** que o filtro deve
ir para a `expressao` do próprio sub-pedido, que lê o tipo do esquema e compara
no domínio inteiro escalado.

Converter todo texto que *parece* número seria pior: mudaria o significado de
uma coluna de texto — `codigo = '10'` deixaria de casar com o código `10`
gravado como texto.

### O defeito que só apareceu na costura: a projeção acontecia duas vezes

O `consultar` recebe `colunas` no próprio pedido e projeta **na fonte**, com o
apelido como chave da linha. O `resposta_do_sql` projetava de novo, procurando
`nome` numa linha que já se chamava `quem` — e devolvia uma coluna de **nulos**
com a cara de dado que não existe.

Nenhum teste de tradução acusava (lá o pedido está certo) e nenhum teste do
`consultar` acusava (lá a resposta está certa). Só apareceu exercitando o
caminho inteiro, num `SELECT nome AS quem FROM v_todos`. Hoje `resposta_do_sql`
não reprojeta o que a op já projetou, e o cabeçalho `colunas` continua saindo
de lá porque ele é o contrato da resposta.

### O que ainda recusa nomeando, e onde consertar

- **`ORDER BY p.id`** — nome qualificado na ordem: o analisador recusa com
  «sobrou "." depois do fim do comando». É da gramática, não da execução.
- **`COUNT(*)` sobre visão** — recusa dizendo que não há substrato nesta
  rodada; a contagem se faz com `agrupar` sobre a tabela de dentro.
- **Junção `RIGHT`, `FULL`, `CROSS`** — recusam nomeando nos dois lados (o
  tradutor e o `consultar`), e a do `RIGHT` ensina a trocar os lados.
- **Correlação e `EXISTS`** — recusam nomeando: exigiriam rodar a subconsulta
  por linha.
