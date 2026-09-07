# E) exemplo de stored procedure em phxsql

> Corrida em 2026-09-07T16:25:26Z UTC · commit `a56a165` · `target/release/phxsqld`
> · reproduzido por `python3 bancada/sql-exemplos/exercitar.py`

## Resposta curta

Stored procedure existe, pela mesma op `sql` do resto (`CREATE PROCEDURE
nome([IN|OUT|INOUT] p TIPO, …) <corpo>`), com um interpretador próprio
(`crates/phxsql-sql/src/rotina.rs`) que entende `DECLARE`, `SET`,
`IF/ELSEIF/ELSE`, `WHILE`, `SIGNAL` e `SELECT … INTO`. `CALL` executa o corpo
e devolve os parâmetros `OUT`/`INOUT` num objeto `saida`. Cada `INSERT`/
`SELECT` que o corpo produz sai pelo **mesmo portão de permissão** de um
pedido da rede (`executar_derivado`), então a procedure nunca dá poder que
quem chama já não tinha. Exercitado abaixo com duas procedures reais.

## Exemplo exercitado

### 1. `somar_ate` — `IN`, `OUT`, `DECLARE` e `WHILE`

```sql
CREATE PROCEDURE somar_ate(IN ate INT, OUT total INT) BEGIN
  DECLARE i INT DEFAULT 1;
  SET total = 0;
  WHILE i <= ate DO
    SET total = total + i;
    SET i = i + 1;
  END WHILE;
END
```

Resposta da criação:

```json
{"procedimento":"somar_ate","parametros":2,"criado":true,"ok":true,"op":"sql","ms":0}
```

`SHOW PROCEDURES` — o corpo fica guardado **verbatim**, o que o `CALL` compila
a cada carga:

```json
{"total":1,"procedimentos":[{"nome":"somar_ate",
  "parametros":[{"modo":"IN","nome":"ate","tipo":"INT"},
                {"modo":"OUT","nome":"total","tipo":"INT"}],
  "corpo":"BEGIN   DECLARE i INT DEFAULT 1;   SET total = 0;   WHILE i <= ate DO     SET total = total + i;     SET i = i + 1;   END WHILE; END",
  "criado_em":"2026-09-07 16:25:26,231","criado_por":""}],
 "ok":true,"op":"sql","ms":0}
```

Duas chamadas, dois resultados — a soma de Gauss confere as duas:

```
CALL somar_ate(100)  -> {"procedimento":"somar_ate","saida":{"total":5050},"ok":true,"op":"sql","ms":0}
CALL somar_ate(10)   -> {"procedimento":"somar_ate","saida":{"total":55},"ok":true,"op":"sql","ms":0}
```

### 2. `resumo_clientes` — a procedure lendo o motor com `SELECT … INTO`

```sql
CREATE PROCEDURE resumo_clientes(OUT quantos INT, OUT primeiro_nome VARCHAR(40))
BEGIN
  SELECT COUNT(*) INTO quantos FROM clientes;
  SELECT nome INTO primeiro_nome FROM clientes WHERE id = 1;
END
```

```json
{"procedimento":"resumo_clientes","parametros":2,"criado":true,"ok":true,"op":"sql","ms":0}
```

```
CALL resumo_clientes()
-> {"procedimento":"resumo_clientes","saida":{"quantos":5,"primeiro_nome":"Ana"},"ok":true,"op":"sql","ms":0}
```

Os cinco clientes da massa de teste (`Ana`, `Bruno`, `Carla`, `Duda`, `Elis`)
batem com `quantos:5`, e `id = 1` é `Ana` — os dois `SELECT … INTO` do corpo
leram o motor de verdade, pelo mesmo `executar_derivado` de um pedido comum.

### Limpeza

```
DROP PROCEDURE somar_ate       -> {"procedimento":"somar_ate","excluido":true,"ok":true,"op":"sql","ms":0}
DROP PROCEDURE resumo_clientes -> {"procedimento":"resumo_clientes","excluido":true,"ok":true,"op":"sql","ms":0}
```

## O que NÃO existe, e é dispensa registrada

- **`UPDATE`/`DELETE` dentro do corpo** — decisão documentada em
  `docs/TRIGGERS.md` §8: o motor grava por `rowid`, e traduzir um `UPDATE …
  WHERE` exigiria o planejador que não existe. `INSERT` e `SELECT … INTO`
  cobrem auditoria e leitura.
- **`CASE`, `LOOP`, `REPEAT`, cursor, `HANDLER`** — `IF/ELSEIF` e `WHILE`
  cobrem o que é preciso; menos superfície de erro.
- **`CREATE FUNCTION`** — recusada nomeando o motivo (*"devolveria valor
  dentro de expressão SQL, e a camada SELECT não avalia expressão"*): não
  testado nesta rodada porque já está coberto por teste unitário
  (`crates/phxsql-sql/src/rotina.rs`), e o item F/A já mostram o padrão de
  recusa nomeada.
- **`CALL` aninhado** — não existe nesta versão.
- **`DEFINER`** — a procedure roda com o poder de quem **chama**, nunca de
  quem a criou.
- **Variável de sessão (`@x`)** — não há; usa-se `DECLARE`.
- **Transação dentro do corpo** — não há `BEGIN`/`COMMIT` de dado (só o
  `BEGIN … END` de bloco): não há transação no PhxSql para o corpo abrir.

## Como se refaz

```bash
python3 bancada/sql-exemplos/exercitar.py
```

A função `item_e` cria as duas procedures, chama cada uma duas vezes e
imprime a resposta crua de cada passo.
