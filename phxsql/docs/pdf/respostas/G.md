# G) exemplo de create database, table, column e ER

> Corrida em 2026-09-07T16:25:26Z UTC · commit `a56a165` · `target/release/phxsqld`
> · reproduzido por `python3 bancada/sql-exemplos/exercitar.py`

## Resposta curta

`CREATE DATABASE`/`TABLE`/coluna **não existem como texto SQL** (item A) —
são operações nativas do protocolo: `criar_database`, `criar_tabela`,
`acrescentar_coluna` (o `ALTER TABLE ADD COLUMN`) e `declarar_fk` (a chave
estrangeira). O "ER" do PhxSql não é um desenho: é o que a operação
`esquema` devolve — colunas com seu papel (`primaria`, `estrangeira`,
`nas_chaves_estrangeiras`, `nos_indices`), índices e a lista de
`chaves_estrangeiras` com `ao_excluir`/`ao_alterar`/`verificar`. Exercitado
abaixo: um database, duas tabelas com oito tipos de coluna, uma coluna
acrescentada depois (com linha já gravada), uma chave estrangeira nas três
formas que a regra primordial permite, e a recusa de excluir o pai vivo.

## Exemplo exercitado

### 1. `CREATE DATABASE` (op nativa `criar_database`)

```json
{"database":"filial","caminho":"/tmp/phx-f1-5172/dados/filial","ok":true,"op":"criar_database","ms":0}
```

### 2. `CREATE TABLE` com colunas de vários tipos

```json
{"op":"criar_tabela","database":"filial","tabela":"produtos",
 "colunas":[
   {"nome":"id","tipo":"Int8","obrigatoria":true},
   {"nome":"nome","tipo":"Str(80)","obrigatoria":true},
   {"nome":"preco","tipo":"Decimal(15,2)"},
   {"nome":"criado_em","tipo":"Date"},
   {"nome":"atualizado_em","tipo":"DateTime"},
   {"nome":"ativo","tipo":"Bool"},
   {"nome":"ficha","tipo":"Memo"},
   {"nome":"uuid","tipo":"Uuid"}],
 "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}
```

```json
{"database":"filial","schema":null,"tabela":"produtos","colunas":10,"indices":1,"paginada":false,"ok":true,"op":"criar_tabela","ms":0}
```

(10 colunas = as 8 declaradas + `softdeleted` e `rownum`, que o motor
acrescenta sempre — ver item H.)

Segunda tabela, `pedidos`, com índice não único em `produto_id`:

```json
{"database":"filial","schema":null,"tabela":"pedidos","colunas":5,"indices":2,"paginada":false,"ok":true,"op":"criar_tabela","ms":0}
```

### 3. `ALTER TABLE ADD COLUMN` — `acrescentar_coluna`, numa tabela que já tem linha

```json
inserir produto 1 antes do ALTER -> {"rowid":1,"registros":1,"ok":true,"op":"inserir","ms":0}
```

```json
{"op":"acrescentar_coluna","database":"filial","tabela":"produtos",
 "coluna":{"nome":"categoria","tipo":"Str(30)"}}
```

```json
{"database":"filial","tabela":"produtos","coluna":"categoria","posicao":8,
 "colunas":11,"slots_reescritos":1,"registros":1,"ms":1,
 "indices_refeitos":false,"ok":true,"op":"acrescentar_coluna"}
```

A linha 1 (já gravada) sobreviveu à mudança de esquema: `slots_reescritos:1`
confere com `registros:1` — o `.reg` inteiro foi reescrito com o slot mais
largo, e nada se perdeu.

### 4. `FOREIGN KEY` — a regra primordial nascendo, nas três formas

**(1) Sem dizer `ao_excluir`/`ao_alterar` — nasce `restringir`/`cascata`/conferida:**

```json
{"op":"declarar_fk","database":"filial","tabela":"pedidos","nome":"fk_produto",
 "colunas":["produto_id"],"tabela_ref":"produtos","colunas_ref":["id"]}
```

```json
{"database":"filial","tabela":"pedidos","nome":"fk_produto",
 "chaves_estrangeiras":1,"imposta":false,"arquivos_reescritos":false,
 "ok":true,"op":"declarar_fk","ms":0}
```

**(2) A regra primordial em ação: `ao_excluir:"cascata"` é RECUSADO na
declaração** (não na gravação — cedo é barato, tarde é um banco modelado
errado):

```json
{"op":"declarar_fk", ... ,"ao_excluir":"cascata"}
```

```json
{"ok":false,"op":"declarar_fk",
 "erro":"[SP000018] esquema invalido: \"ao_excluir\": \"cascata\" nao existe no PhxSql -- ao excluir e sempre \"restringir\". Nunca se mata o registro pai que tem filhos em outra tabela, e por isso o par cascata/cascata tambem nao existe. Para apagar o pai, apague as filhas antes",
 "codigo":2001,"nome":"ESQUEMA_INVALIDO","classe":"esquema","sprint":"SP000018","repetir":false,"ms":0}
```

**(3) `ao_alterar:"cascata"` dito por extenso — aceito, é o padrão mesmo
explícito:**

```json
{"database":"filial","tabela":"pedidos","nome":"fk_produto",
 "chaves_estrangeiras":1,"imposta":false,"arquivos_reescritos":false,
 "ok":true,"op":"declarar_fk","ms":0}
```

**(4) `verificar:false` — a escolha ESCRITA de declarar sem conferir:**

```json
{"op":"declarar_fk", ... ,"verificar":false}
```

```json
{"database":"filial","tabela":"pedidos","nome":"fk_produto",
 "chaves_estrangeiras":1,"imposta":false,"arquivos_reescritos":false,
 "ok":true,"op":"declarar_fk","ms":0}
```

A chave que fica para o resto da prova é a **(1)**, sem nenhum campo
opcional — a que nasce conferida por padrão.

### 5. A chave NASCE conferida: filha apontando para pai inexistente recusa na hora

```json
pedido produto_id=999 (nao existe) -> {"ok":false,"op":"inserir",
 "erro":"[SP000008] integridade referencial: fk_produto: nao existe produtos(id) com esse valor",
 "codigo":3006,"nome":"INTEGRIDADE","classe":"dado","sprint":"SP000008","repetir":false,"ms":0}

pedido produto_id=1 (existe) -> {"rowid":1,"registros":1,"ok":true,"op":"inserir","ms":0}
```

Ninguém mandou `"verificar":true` — a conferência aconteceu porque a chave
**nasceu** conferida.

### 6. A regra primordial na GRAVAÇÃO: excluir o pai vivo é recusado

```json
excluir produtos rowid=1 (tem pedido filho) -> {"ok":false,"op":"excluir",
 "erro":"[SP000008] integridade referencial: produtos: esta linha tem filhas em pedidos pela chave \"fk_produto\". Nunca se apaga o registro pai que tem filhos -- apague as filhas antes",
 "codigo":3006,"nome":"INTEGRIDADE","classe":"dado","sprint":"SP000008","repetir":false,"ms":0}
```

### 7. O `esquema` devolvido — o ER em forma de dado

`esquema` de `pedidos` (a tabela filha), resumido aos campos que importam
para o ER:

```json
{"tabela":"pedidos",
 "colunas":[
   {"nome":"id","tipo":"Int8","primaria":true,"nos_indices":["porId"]},
   {"nome":"produto_id","tipo":"Int8","nullable":true,"estrangeira":true,
    "nas_chaves_estrangeiras":["fk_produto"],"nos_indices":["porProduto"]},
   {"nome":"quantidade","tipo":"Int4"},
   {"nome":"softdeleted","tipo":"Bool","sistema":true},
   {"nome":"rownum","tipo":"UInt8","sistema":true}],
 "indices":[
   {"nome":"porId","unico":true,"primario":true,"colunas":[{"coluna":"id"}]},
   {"nome":"porProduto","unico":false,"colunas":[{"coluna":"produto_id"}]}],
 "chaves_estrangeiras":[
   {"nome":"fk_produto","colunas":["produto_id"],"tabela_ref":"produtos",
    "colunas_ref":["id"],"ao_excluir":"Restringir","ao_alterar":"Cascata",
    "verificar":true}],
 "ok":true,"op":"esquema","ms":0}
```

`esquema` de `produtos` (a tabela mãe), mostrando os oito tipos + as duas
colunas de sistema + a coluna acrescentada pelo `ALTER`:

```json
{"tabela":"produtos",
 "colunas":[
   {"nome":"id","tipo":"Int8","primaria":true},
   {"nome":"nome","tipo":"Str(80)"},
   {"nome":"preco","tipo":"Decimal { precisao: 15, escala: 2 }"},
   {"nome":"criado_em","tipo":"Date"},
   {"nome":"atualizado_em","tipo":"DateTime"},
   {"nome":"ativo","tipo":"Bool"},
   {"nome":"ficha","tipo":"Memo"},
   {"nome":"uuid","tipo":"Uuid"},
   {"nome":"categoria","tipo":"Str(30)"},
   {"nome":"softdeleted","tipo":"Bool","sistema":true},
   {"nome":"rownum","tipo":"UInt8","sistema":true}],
 "chaves_estrangeiras":[],
 "ok":true,"op":"esquema","ms":0}
```

**O ER da dupla `produtos`↔`pedidos`**, lido só destes dois `esquema`:
`pedidos.produto_id` é **1 para muitos** com `produtos.id` — `ao_excluir:
Restringir`, `ao_alterar:Cascata`, `verificar:true` — e o índice existe **dos
dois lados** (`porId` em `produtos`, `porProduto` em `pedidos`), que é a
exigência que a regra primordial impõe para a conferência funcionar nos dois
sentidos.

## O que NÃO existe, e é dispensa registrada

- **`CREATE DATABASE`/`TABLE`/`ALTER TABLE`/`FOREIGN KEY` por texto SQL** —
  já coberto no item A: são operações nativas, e a recusa do `CREATE` nomeia
  o caminho certo (`criar_tabela`).
- **Diagrama ER visual** — fora do escopo desta frente: o pedido do dono para
  este item é a chave declarada + o que `esquema` devolve; o diagrama da tela
  é outro item do PDF.
- **`Cascade/Cascade`** — não existe **por consequência**, não por uma
  segunda regra: como `ao_excluir` só aceita `restringir`, não há como montar
  o par cascata/cascata. Exercitado no passo (2) acima.
- **Índice automático do lado da mãe** (o que MySQL/MariaDB fazem, segundo
  `docs/INTEGRIDADE.md` §7.5) — o PhxSql **recusa na gravação** dizendo qual
  índice falta, em vez de criar um sozinho; não exercitado aqui porque exige
  uma tabela sem índice em `id`, cenário fora do que este exemplo monta.
- **`copiar_tabela_para` e restauração de backup não conferindo FK** — são
  decisões documentadas em `docs/INTEGRIDADE.md` §4.3/§4.4, não tocadas nesta
  bateria porque o pedido pede `CREATE`, não cópia nem backup.

## Como se refaz

```bash
python3 bancada/sql-exemplos/exercitar.py
```

A função `item_g` cria o database `filial`, as tabelas `produtos`/`pedidos`,
acrescenta a coluna `categoria`, declara e reexcluir a chave três vezes (para
mostrar as três formas) e imprime os dois `esquema` no fim.
