# O) leitura do manual do Mariadb fonte e help verificando gaps ainda existentes gerar lista de sprints que julgue importante

## Resposta curta
O estudo anterior propôs **13 sprints**; conferi os 13 contra o motor de hoje: **2
fecharam** (`ALTER TABLE ADD COLUMN` e o índice de texto `.fts`, os dois com o comando que
agora passa colado), **11 sobreviveram** — e a conferência **encolheu três**, porque o
substrato que faltava passou a existir (o `varrer` já faz `AND` de duas condições). A sonda
achou ainda **6 gaps novos**, dois deles defeito e não recurso. **Lista final: 17 sprints**,
por importância.

## Exemplo exercitado

Corrida de **2026-09-07 16:49 UTC**, commit **a56a165**. **Fonte:**
https://mariadb.com/kb/en/sql-statements/ e as páginas já citadas no estudo anterior,
lidas nesta rodada.

### Os 13 sprints antigos, conferidos um a um contra o motor

| # | sprint de `SPRINTS-MARIADB.md` | estado hoje | a prova, desta corrida |
|--:|---|---|---|
| 10 | `ALTER TABLE ADD COLUMN`, preservando o rowid | **FECHADO** | `{"op":"acrescentar_coluna","tabela":"fornecedores","nome":"uf","tipo":"Str(2)","padrao":"SC"}` → `{'database': 'loja', 'tabela': 'fornecedores', 'coluna': 'uf', 'posicao': 2, 'colunas': 5, 'slots_reescritos': 0, …` |
| 13 | Índice de texto completo (`.fts`) | **FECHADO** | `{"op":"procurar_texto","indice":"porCorpo","palavra":"fênix"}` → `{'encontrados': 1, 'linhas': [{'rowid': 1, 'id': 1, 'corpo': 'a fenix renasce das cinzas', 'softdeleted': False, …`. **Dobra acento** (achou «fenix» pedindo «fênix») e **não faz prefixo** (`"fen"` → `{'encontrados': 0, 'linhas': []}`), exatamente como `docs/FTS.md` promete |
| 1 | O `WHERE` que filtra de verdade | **SOBREVIVE, e encolheu** | o substrato **existe e faz AND**: `{"op":"varrer","onde":[{"coluna":"cidade","op":"=","valor":"Blumenau"},{"coluna":"nome","op":"=","valor":"Adriano"}]}` → `{'registros': 3, 'visiveis': 3, 'marcadas': 0, 'devolvidas': 1, 'examinadas': 3, 'modo': 'posicao', …`. É o `SELECT` que não alcança: `SQL, coluna 54: o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, e nao ha planejador` |
| 2 | `EXPLAIN` e `ANALYZE` | **SOBREVIVE, e encolheu** | a premissa dele («o EXPLAIN diz algo que as `notas` já não digam?») está meio respondida: as notas **já dizem** o índice e a ausência de planejador — `['indice porNome escolhido pelo WHERE -- e nao ha planejador: se houvesse dois candidatos, o primeiro declarado venceria']`. Falta o **verbo**: `ANALYZE nao e um comando desta camada` |
| 3 | O que falta ao agendador (event scheduler) | **SOBREVIVE** (a metade (b)) | `{"op":"jobs"}` devolve `{'arquivo': 'jobs.json', 'log': 'jobs.log', 'relogio_no_ar': False, 'aviso_email': {'ligado': False, 'email_ligado': False, 'avisar_jobs': False, 'para': [], 'repetir_horas': 6}, 'jobs': [{'nome': 'quebra', 'descricao': 'job que falha de proposito, …` — **não há campo de réplica**. Um job de escrita ligado nos dois lados grava em dobro |
| 4 | `CHECK` declarativo | **SOBREVIVE** | `ALTER TABLE clientes ADD CONSTRAINT c1 CHECK (saldo >= 0)` → `SQL, coluna 1: ALTER nao e um comando desta camada`. E o `criar_tabela` não lê campo `check` nenhum |
| 5 | Colunas geradas `PERSISTENT` | **SOBREVIVE** | `ALTER TABLE clientes ADD COLUMN dobro DECIMAL(12,2) AS (saldo*2) PERSISTENT` → a mesma recusa |
| 6 | `EXCEPT` e `INTERSECT` | **SOBREVIVE** | `SELECT nome FROM clientes EXCEPT SELECT …` → `SQL, coluna 34: sobrou "SELECT" depois do fim do comando`. E o `unir` continua com dois modos só: `{'modo': 'distinta', 'sql': 'UNION', …` e `{'modo': 'tudo', 'sql': 'UNION ALL', …` |
| 7 | `AS OF` de uma linha | **SOBREVIVE**, e a premissa continua de pé | `SELECT … FOR SYSTEM_TIME AS OF NOW()` → `sobrou "SYSTEM_TIME" depois do fim do comando`. A matéria-prima está lá — `{"op":"diario"}` → `{'total': 9, 'eventos': [{'quando': '2026-09-07 16:48:56,984', 'carimbo_ms': 1788799736984, 'operacao': 'inclusao', 'rowid': 5, 'versao': 1, 'usuario': 0}, …` — e a imagem da linha **continua nascendo desligada**: `config.replicacao.imagem_da_linha: False` |
| 8 | Poda de volumes na varredura | **SOBREVIVE**, e continua dependendo do 1 | não há poda no código; e a varredura filtrada de hoje diz `'examinadas': 3` de 3 — ela examina tudo |
| 9 | Papéis no modelo de direitos | **SOBREVIVE** | `CREATE ROLE gerente` → `CREATE nesta camada cria TRIGGER ou PROCEDURE`. E o `config` não tem bloco de papéis: só `usuarios`, com direito por base e por tabela |
| 11 | Sequência como objeto próprio | **SOBREVIVE pela metade** | existe **uma por tabela**: `{"op":"sequencias"}` → `{'database': 'loja', 'total': 3, 'sequencias': [{'tabela': 'chamados', 'coluna': None, 'proxima': 0, 'registros': 1, 'tem_sequencia': False}, {'tabela': 'clientes', 'coluna': 'id', 'proxima': 7, 'registros': 6, 'tem_sequencia': True}, …]}`, e `{"op":"ajustar_sequencia","proxima":5000}` → `{'database': 'loja', 'tabela': 'clientes', 'antes': 7, 'proxima': 5000, 'aviso': ''}`. Falta o objeto que **atravessa duas tabelas** |
| 12 | O degrau seguinte do interpretador | **SOBREVIVE como medição**, não como sprint (`SPRINTS.md` §5.2) — e agora está medido | ver abaixo |

O interpretador de rotinas, exercitado nesta corrida — o que passa e o que
recusa, colado:

```
[OK  ] CREATE PROCEDURE p3() BEGIN DECLARE x INT DEFAULT 0;
         WHILE x < 3 DO SET x = x + 1; END WHILE; END
  {'procedimento': 'p3', 'parametros': 0, 'criado': True}
[OK  ] CREATE PROCEDURE p6(IN v INT) BEGIN
         INSERT INTO clientes (nome) VALUES ('via proc'); END
  {'procedimento': 'p6', 'parametros': 1, 'criado': True}
[OK  ] CALL p6(1)
  {'procedimento': 'p6', 'saida': {}}
[ERRO] UPDATE dentro do corpo
  SQL, coluna 7: UPDATE dentro de rotina ainda nao existe: o motor atualiza e exclui por rowid, e traduzir o WHERE pede o planejador. O que ja da: INSERT e SELECT … INTO
[ERRO] CASE no corpo
  SQL, coluna 22: CASE nao existe no corpo; escreva com IF/ELSEIF/ELSE
[ERRO] DECLARE c CURSOR FOR SELECT …
  SQL, coluna 17: o tipo CURSOR nao existe em rotina; use INT, DECIMAL(p,s), VARCHAR(n), BOOL ou DATE (que viaja como texto)
[ERRO] START TRANSACTION dentro do corpo
  SQL, coluna 7: transacao e comando de SESSAO e nao cabe num corpo de rotina: ela pertence a CONEXAO, e o corpo roda dentro de UM pedido. Abra a transacao pela conexao e chame a rotina de dentro dela
```

**As quatro recusas nomeiam o motivo e a alternativa** — é o que faz este item
ser medição e não sprint: só o Profiler pode dizer em qual delas alguém esbarra
de verdade.

### Os 6 gaps novos que a conferência achou

| # | o gap | a prova |
|--:|---|---|
| N1 | **o vocabulário de abertura do cliente** — `SHOW TABLES`, `SHOW DATABASES`, `SHOW CREATE TABLE`, `DESCRIBE`, `USE` | `SHOW nesta camada lista TRIGGERS ou PROCEDURES; tabelas e colunas saem por sistabelas/siscolunas` / `DESCRIBE nao e um comando desta camada` / `USE nao e um comando desta camada`. E as três primeiras **já têm resposta pronta**: `{"op":"tabelas"}` → `{'database': 'loja', 'schemas': [], 'tabelas': ['chamados', 'clientes', 'itens']}`, `{"op":"bancos"}` → `['loja']`, `{"op":"esquema"}` → o esquema inteiro |
| N2 | **`WHERE pk = n` recusa quando a chave é `Sequence`** | `SELECT * FROM clientes WHERE id = 2` → `tipo invalido: esperado numero da sequencia, recebido Texto("2")`; o irmão `Int8` passa: `SELECT * FROM itens WHERE id = 1` → `{'sql': 'SELECT * FROM itens WHERE id = 1', 'op': 'buscar', …` |
| N3 | **`FROM schema.tabela` inexistente vaza o erro cru do SO, e manda repetir** | `SELECT * FROM filial.clientes` → `[SP000010] erro de E/S: No such file or directory (os error 2)`, com `"repetir": true`. O caminho sem schema faz certo: `nenhum volume de naoexiste.reg em …`, com `"repetir": false` |
| N4 | **o catálogo documenta valores que o motor recusa** | `{"op":"unir","modo":"distinto"}` (o valor do catálogo) → `união desconhecida: "distinto" (use distinta ou tudo)`. E o **exemplo colável** do `pivotar` no catálogo → `agregador desconhecido: "somar" (use soma, media, contagem, minimo, maximo ou distintos)`; além disso o catálogo documenta `chave`/`coluna` onde o motor quer `linhas`/`colunas` |
| N5 | **`LIMIT 0, 2`** — a forma que todo código MySQL(R)/MariaDB(R) escreve | `SQL, coluna 31: sobrou "," depois do fim do comando; um comando por vez` |
| N6 | **`INSERT`/`UPDATE`/`DELETE` em SQL** (o passo 2 do roteiro de `docs/SQL.md` §4) | `INSERT ainda nao existe nesta camada -- so SELECT. A operacao equivalente ja funciona pelo protocolo` — as três operações existem e rodaram nesta corrida pelo protocolo |

### A lista de sprints que eu proponho — 17, por importância

**O critério da ordem:** primeiro o que um aplicativo de cadastro comum ou o
driver dele topa **hoje**; depois o que custa correção; depois recurso.

**Correção — cabem numa rodada e são defeito de hoje**

| ordem | sprint | tam. | por quê |
|--:|---|:--:|---|
| 1 | **A chave `Sequence` aceita inteiro escrito como texto** (N2) | **P** | é a chave primária de quase toda tabela nascida pela tela, e `SELECT * FROM t WHERE id = 1` — a consulta mais escrita do mundo — recusa. O alargamento já foi feito para `Int` (`docs/SQL.md` §5) e **não alcançou o irmão**. O teste que importa é o do comportamento velho: quem manda número continua igual |
| 2 | **A tabela que não existe recusa dizendo isso, com `repetir: false`** (N3) | **P** | um driver que lê `repetir: true` tenta de novo para sempre, e é justamente o caminho que ele percorre primeiro (`information_schema.tables`). E a recusa não nomeia nada |
| 3 | **O catálogo publica o que o motor aceita, não o que alguém digitou** (N4) | **P** | *todo número visível sai de um gerador* — aqui é a mesma lei para valor de parâmetro. Duas listas (`Uniao::de_texto`, `Agregador::de_texto`) já estão no código; o catálogo tem de sair delas |

**Destravamento — o que outros esperam**

| ordem | sprint | tam. | por quê |
|--:|---|:--:|---|
| 4 | **O `WHERE` que filtra, e a segunda condição** (antigo 1) | **M** | a conferência **encolheu o sprint**: o `varrer` já filtra e já faz AND — está medido acima. Falta a tradução e a decisão de quando intersecar rowids. Cinco itens o nomeiam como dependência |
| 5 | **`INSERT`/`UPDATE`/`DELETE` por chave primária, em SQL** (N6) | **M** | fecha o CRUD e é o passo 2 do roteiro que o próprio `docs/SQL.md` escreveu. Depende do 1 para o `WHERE` |
| 6 | **O vocabulário de abertura: `SHOW TABLES`/`DATABASES`, `DESCRIBE`, `USE`** (N1) | **P** | três dos quatro são tradução de poucas linhas para operações que já respondem; o `USE` é estado de conexão, e o desenho disso já está escrito (`docs/SQL.md` §2) |
| 7 | **`LIMIT n, m` e o *upsert* (`ON DUPLICATE KEY`, `REPLACE INTO`)** (N5) | **P** | dialeto puro; depende do 5 |

**Recurso — o que o manual do MariaDB(R) traz e aqui não há**

| ordem | sprint | tam. | por quê, e a premissa que pode matá-lo |
|--:|---|:--:|---|
| 8 | **`CHECK` declarativo** (antigo 4) | **M** | o lugar já existe: o `BEFORE` roda com a trava na mão, e o avaliador exato (`i128` com escala, sem `f64`) já está no `rotina.rs`. **Premissa:** tabela sem `CHECK` custa exatamente o que custa hoje — e o que fazer com as linhas já gravadas que violam a regra nova, escrito antes do código |
| 9 | **`EXCEPT` e `INTERSECT`** (antigo 6) | **P** | a máquina do `unir` já compara linhas com chave composta e nulo que não casa com nulo. **Premissa:** essa comparação serve como está |
| 10 | **Papéis no modelo de direitos** (antigo 9) | **M** | com direito por tabela, dez pessoas do mesmo cargo são a mesma regra copiada dez vezes. **Premissa:** `sem_papel_nada_muda` **antes** de qualquer código |
| 11 | **Sequência como objeto próprio** (antigo 11) | **M** | hoje é uma por tabela, medido acima. **Premissa é do dono:** buraco na numeração é aceitável? Se não, custa um `fsync` por número |
| 12 | **Colunas geradas `PERSISTENT`** (antigo 5) | **M** | mesma rodada de formato do 8 (`PSCH` v7). **Premissa:** custo por linha de avaliar a expressão na escrita — a inserção inteira custa 7,5 µs, e 1 µs de expressão são 13% |
| 13 | **`DISABLE ON SLAVE`: o job que não roda na réplica** (antigo 3(b)) | **P** | defeito de hoje com quatro modos de replicação no ar. **Premissa:** subir um par e contar — se a réplica já recusa a escrita por outro caminho, o sprint some quase inteiro |
| 14 | **`EXPLAIN` e `ANALYZE`** (antigo 2) | **P** | rende **depois** do 4. Se as `notas` já bastarem, encolhe para o verbo como sinônimo |
| 15 | **Poda de volumes na varredura** (antigo 8) | **M** | depende do 4. **Premissa:** quantos volumes a pergunta típica dispensa, medido numa tabela particionada de verdade. E o alerta que a fonte dá de graça: com gatilho `BEFORE`, o MariaDB(R) **desiste da poda** — e gatilho já entrou aqui |
| 16 | **`AS OF` de uma linha** (antigo 7) | **M** | a matéria-prima está gravada. **Premissa é do dono:** a imagem da linha nasce desligada; ligar custa ~10% da vazão e 5× o diário |
| 17 | **O degrau seguinte do interpretador** (antigo 12) | — | **não é sprint: é medição.** As quatro recusas estão coladas acima; qual delas incomoda de verdade, só o Profiler diz |

## O que NÃO existe, e é dispensa registrada

- **`ALTER TABLE` não existe em SQL**, em nenhuma forma — e é por isso que os
  sprints 8 e 12 desta lista recusam pela mesma mensagem. O que um cadastro faz
  com ele já existe como operação.
- **`CREATE FUNCTION` recusa nomeando o motivo**, e isso não é gap a fechar
  antes do sprint 4: função devolve valor dentro de expressão, e a camada
  `SELECT` não avalia expressão.
- **Não subi um par de replicação nem um cluster nesta rodada.** O sprint 13
  desta lista tem premissa a medir, e eu **não a medi** — dizer que o job grava
  em dobro sem ter contado seria diagnóstico plausível, que é exatamente o que
  esta casa não aceita. O que está medido é a ausência do campo.
- **Não medi desempenho de nada aqui.** Todos os números de custo desta
  resposta (7,5 µs, 10%, 5×, 0,553 µs/linha) vêm citados de `DESEMPENHO.md`,
  `FORMATO.md` e `SPRINTS.md`, com a origem dita ao lado. O que **eu** medi
  nesta corrida é aceito/recusado, e está colado.

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py mariadb   # os 27 comandos do manual
python3 bancada/gaps-sql/sondar.py           # com as equivalências e os achados
```
