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
   **Feito** em `crates/phxsql-sql/`, menos a ligação com o servidor: não há
   operação `sql` no protocolo ainda, e sem ela o DBeaver não lista nada.
2. **`INSERT`/`UPDATE`/`DELETE`** por chave primária. Fecha o CRUD.
3. **`BULKINSERT`** e o catálogo (`information_schema`). Fecha a carga e a
   introspecção.
4. **`JOIN`**, mapeando para o `juntar` que já existe.
5. **Expressão e planejador** — o trabalho de verdade, e o único que não é
   tradução.

Os três primeiros são tradução de coisa medida e testada. É por ali.
