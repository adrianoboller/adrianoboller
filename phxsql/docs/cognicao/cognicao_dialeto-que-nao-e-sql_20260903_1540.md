# Quando o «dialeto» do outro banco não é SQL

*Descoberto em 03/09/2026, 15:40, ao desenhar o motor `phxsql` do DbLink
(pedido 166).*

## 1. O que aconteceu

O DbLink foi construído em cima de uma pergunta só: `consultar(sql, teto)`. Os
dois motores de fora respondem o catálogo em SQL — `SHOW FULL COLUMNS`,
`pg_attribute` —, então acrescentar um motor sempre quis dizer «escrever o SQL
dele».

O terceiro motor é outro PhxSql, e ele **não tem catálogo em SQL**. Tem
`bancos`, `sistabelas`, `esquema` e `varrer`, que são as mesmas operações que a
tela dele já usa.

## 2. O que eu concluí primeiro, e estava errado

Que o caminho era ensinar o `dialeto` a falar PhxSql: `sql_tabelas` emitiria um
`SELECT` contra alguma tabela de catálogo, e nada mais mudaria. É o desenho que
a estrutura do módulo empurra, e ele tem duas falhas que só aparecem medindo o
outro lado:

- **não existe a tabela de catálogo.** O `phxsql-sql` traduz `SELECT` para
  `varrer`/`buscar` sobre tabela de dado; não há `information_schema`. O SQL
  que eu escreveria seria contra algo que não existe.
- **o `esquema` traz MAIS do que o `SHOW`**: chave primária, papel da coluna
  nos índices, dado pessoal, rótulo, máscara. Passar por um SQL inventado seria
  jogar fora o que o próprio protocolo já responde.

## 3. O que a medição disse

Sondando um `phxsqld` de verdade antes de escrever qualquer linha:

| pergunta | em SQL | pelo protocolo |
|---|---|---|
| tabelas de uma base | não há catálogo | `sistabelas`: nome, schema, registros, slots, colunas, índices, chave primária, chaves estrangeiras, bytes por linha |
| colunas de uma tabela | não há catálogo | `esquema`: 15 campos por coluna |
| conteúdo paginado | `SELECT … LIMIT n OFFSET m` **funciona** (o `phxsql-sql` aceita as duas palavras) | `varrer` com `pular`/`max`, e a contagem vem junta de graça |

E o que **atravessa bem** é só o `dblink_consultar`: a op `sql` do outro lado
existe e é a mesma que a tela dele usa, então a instrução vai inteira e quem a
executa é o motor de lá.

O desvio ficou na **primeira linha** de cada operação, antes de qualquer
`format!` — montar SQL para depois jogá-lo fora é o mesmo erro do observador
que trabalha antes de olhar o próprio interruptor (o Profiler, 7% da carga).

## 4. A regra

**Quando o motor novo não responde a pergunta na mesma língua, o dialeto não é
uma tradução: é um desvio.** E o que sobra do lado SQL **recusa dizendo qual
operação nativa responde**, em vez de devolver instrução inventada — um
`&'static str` obriga o ramo novo a inventar, e instrução inventada compila.

E o corolário do formato: **a RESPOSTA continua com os nomes de sempre.**
`dblink_estrutura` do PhxSql devolve `Field`, `Type`, `Null`, `Key`, `Default`,
`Comment` e `Key_name`, `Column_name`, `Non_unique`, `Seq_in_index`, com a
polaridade do nome (0 = único). Um terceiro vocabulário faria cada cliente
crescer um `if` por motor, e o motor esquecido é o que para de funcionar sem
ninguém ver. O que o `esquema` tem a mais vai em colunas **extra**, depois das
seis.

## 5. Como está guardado hoje

- `Motor::catalogo_em_sql()` é o sinal, num lugar só — a tela e as operações
  perguntam a ele em vez de adivinharem pelo nome do motor.
- `operacoes::desviar_phx!` é uma macro justamente para que a sétima operação
  não possa esquecer o desvio: quem escrever a próxima copia **uma** linha.
- `Motor::sem_catalogo_em_sql` monta a recusa que nomeia a op nativa, e o teste
  `o_phxsql_recusa_catalogo_em_sql_e_diz_a_op_nativa` trava os **dois** lados —
  o novo recusa e os dois de fora continuam montando.
- `dblink::phx` tem os seis testes de formato, inclusive o do booleano `1`/`0`
  fechado contra `dialeto::booleano_lido`.
- **Onde o buraco ficou:** `tipo_no_create` devolve, para o motor `phxsql`, o
  nome que o `criar_tabela` lê — e isso **não é SQL de `CREATE TABLE`**, porque
  o PhxSql não cria tabela por SQL. O único chamador (a sincronia) recusa este
  motor antes de chegar lá, mas o nome da função continua prometendo o que ela
  não faz. O que segura é o teste que fecha o ciclo contra
  `valores::tipo_de_texto`, e não o nome.
