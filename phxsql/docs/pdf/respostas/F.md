# F) exemplo de trigger em phxsql

> Corrida em 2026-09-07T16:25:26Z UTC · commit `a56a165` · `target/release/phxsqld`
> · reproduzido por `python3 bancada/sql-exemplos/exercitar.py`

## Resposta curta

Trigger existe, pela mesma op `sql` (`CREATE TRIGGER nome BEFORE|AFTER
INSERT|UPDATE|DELETE ON tabela FOR EACH ROW <corpo>`). `BEFORE` roda **com a
trava de dados na mão**, pode alterar `NEW` e cancelar com `SIGNAL`; `AFTER`
roda **depois de soltar a trava**, pode gravar em outra tabela (auditoria),
mas não tem `SIGNAL` — não há transação para desfazer. Os três exercitados
abaixo, com `INSERT` de verdade disparando cada um.

## Exemplo exercitado

Tabela `clientes` (a mesma do item A, já com 5 linhas) e uma tabela nova
`auditoria` (`evento Str(120)`).

### 1. `BEFORE INSERT` normaliza a cidade

```sql
CREATE TRIGGER normaliza_cidade BEFORE INSERT ON clientes FOR EACH ROW
  SET NEW.cidade = UPPER(TRIM(NEW.cidade))
```

```json
{"gatilho":"normaliza_cidade","tabela":"clientes","quando":"BEFORE","evento":"INSERT","criado":true,"ok":true,"op":"sql","ms":0}
```

Inserindo com cidade `"  gaspar "` (espaços e minúscula de propósito):

```
inserir -> {"rowid":6,"registros":6,"ok":true,"op":"inserir","ms":0}
ler     -> {"id":100,"nome":"Fabio","cidade":"GASPAR","softdeleted":false,"rownum":6,"ok":true,"op":"ler","ms":0}
```

A linha gravada tem `cidade:"GASPAR"` — o `BEFORE` normalizou antes de gravar,
sem a tela ou o cliente terem feito nada.

### 2. `BEFORE INSERT` recusa por `SIGNAL` quando o nome vem vazio

```sql
CREATE TRIGGER exige_nome BEFORE INSERT ON clientes FOR EACH ROW
  IF NEW.nome IS NULL OR NEW.nome = '' THEN
    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cliente sem nome nao entra';
  END IF
```

```json
{"gatilho":"exige_nome","tabela":"clientes","quando":"BEFORE","evento":"INSERT","criado":true,"ok":true,"op":"sql","ms":0}
```

Inserindo com `nome:""`:

```json
{"ok":false,"op":"inserir",
 "erro":"[SP000021] cliente sem nome nao entra (SIGNAL SQLSTATE 45000)",
 "codigo":3005,"nome":"SINAL","classe":"dado","sprint":"SP000021","repetir":false,"ms":0}
```

A linha **não entra**, o código é `3005`/`SINAL`, e a `MESSAGE_TEXT` que o
dono do gatilho escreveu chega intacta ao cliente — sem prefixo nenhum da
casa por cima.

### 3. `AFTER INSERT` audita em outra tabela, vendo a linha como ela FICOU

```sql
CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW
  INSERT INTO auditoria (evento)
  VALUES (CONCAT('entrou ', NEW.nome, ' de ', NEW.cidade))
```

```json
{"gatilho":"audita","tabela":"clientes","quando":"AFTER","evento":"INSERT","criado":true,"ok":true,"op":"sql","ms":0}
```

Inserindo `{"id":102,"nome":"Gilda","cidade":" navegantes"}`:

```
inserir  -> {"rowid":7,"registros":7,"ok":true,"op":"inserir","ms":0}
```

Tabela `auditoria` depois:

```json
{"registros":1,"visiveis":1,"devolvidas":1,"examinadas":1,
 "linhas":[{"rowid":1,"evento":"entrou Gilda de NAVEGANTES","softdeleted":false,"rownum":1}],
 "ok":true,"op":"varrer","ms":0}
```

O evento gravado é **"entrou Gilda de NAVEGANTES"** — com a cidade já em
maiúscula: o `AFTER` lê o `NEW` como a linha **ficou** depois do `BEFORE`
(`normaliza_cidade` já tinha normalizado essa mesma inserção), não como o
cliente mandou.

### `SHOW TRIGGERS` — os três, com o corpo guardado verbatim

```json
{"total":3,"gatilhos":[
  {"nome":"normaliza_cidade","tabela":"clientes","quando":"BEFORE","evento":"INSERT",
   "corpo":"SET NEW.cidade = UPPER(TRIM(NEW.cidade))", ...},
  {"nome":"exige_nome","tabela":"clientes","quando":"BEFORE","evento":"INSERT",
   "corpo":"IF NEW.nome IS NULL OR NEW.nome = '' THEN   SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'cliente sem nome nao entra'; END IF", ...},
  {"nome":"audita","tabela":"clientes","quando":"AFTER","evento":"INSERT",
   "corpo":"INSERT INTO auditoria (evento) VALUES (CONCAT('entrou ', NEW.nome, ' de ', NEW.cidade))", ...}
], "ok":true,"op":"sql","ms":0}
```

### Limpeza

```
DROP TRIGGER normaliza_cidade -> {"excluido":true,...}
DROP TRIGGER exige_nome       -> {"excluido":true,...}
DROP TRIGGER audita           -> {"excluido":true,...}
```

## O que NÃO existe, e é dispensa registrada

- **`UPDATE`/`DELETE` no corpo** — mesma razão do item E: sem planejador, o
  motor grava por `rowid`.
- **`AFTER` com `SIGNAL`** — decisão documentada: a escrita já aconteceu e
  não há transação para desfazer; oferecer `SIGNAL` ali prometeria um
  cancelamento que não existe.
- **Cadeia de `AFTER` sem fundo** — existe teto de **8 níveis**
  (`docs/TRIGGERS.md` §9.1); um `AFTER` que grava na própria tabela já
  chegou a **derrubar o processo inteiro** por estouro de pilha antes do
  conserto, e não foi reexercitado aqui porque já tem teste dedicado
  (`a_cadeia_de_gatilhos_para_no_teto_e_avisa`).
- **`CASE`, `LOOP`, `REPEAT`, cursor, `HANDLER`, `CREATE FUNCTION`,
  `DEFINER`, `FOLLOWS`/`PRECEDES`, variável de sessão** — mesma lista do
  item E; `docs/TRIGGERS.md` §8 documenta cada recusa com o motivo.
- **Falha de `AFTER` desfazendo a escrita** — não existe transação, então
  falha de `AFTER` vira **aviso** (`gatilhos_avisos`) numa resposta `ok`, não
  um erro que sugeriria repetir e duplicar. Não reexercitado nesta rodada
  (`docs/TRIGGERS.md` já traz a prova por soquete).

## Como se refaz

```bash
python3 bancada/sql-exemplos/exercitar.py
```

A função `item_f` cria a tabela `auditoria`, registra os três gatilhos,
insere as três linhas de teste e imprime a resposta crua de cada passo.
