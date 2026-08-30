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
| `INSERT` | `inserir` | 15,9 µs por linha, dois índices |
| `INSERT` de muitas | `inserir_lote` | 16,3× a linha a linha |
| `UPDATE … WHERE rowid = ?` | `atualizar` | com `versao`, recusa o conflito |
| `DELETE` | `excluir` | suave por padrão, física a pedido |
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

## 3. O que a camada SQL vai ter de resolver, e não tem embaixo

Honestidade sobre o tamanho do trabalho — estas não existem no motor:

- **Expressão.** `WHERE preco * 1.1 > 100` não tem quem avalie. O `varrer` filtra
  por comparação simples, e só.
- **Planejador.** Escolher *qual* índice usar quando há dois candidatos. Hoje
  quem chama escolhe, dizendo o nome do índice.
- **`GROUP BY` geral.** O `pivotar` faz a tabulação cruzada, que é um caso.
- **Subconsulta e CTE.** Não há.
- **Transação.** Não há — e é a maior. `BEGIN`/`COMMIT`/`ROLLBACK` não têm o
  que chamar embaixo, e prometer o verbo sem o mecanismo seria pior do que
  não ter o verbo.

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
2. **`INSERT`/`UPDATE`/`DELETE`** por chave primária. Fecha o CRUD.
3. **`BULKINSERT`** e o catálogo (`information_schema`). Fecha a carga e a
   introspecção.
4. **`JOIN`**, mapeando para o `juntar` que já existe.
5. **Expressão e planejador** — o trabalho de verdade, e o único que não é
   tradução.

Os três primeiros são tradução de coisa medida e testada. É por ali.

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

**`WHERE id = 2` não funcionava contra uma coluna `Int4`.** Os 44 testes da
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

Tudo o que a seção 3 lista, e pela mesma razão: não há substrato. O que muda é
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
