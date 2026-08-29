# O Teradata(R) lido contra o PhxSql: os sprints que dariam para executar

> **Este documento continua sendo a fonte, mas não é mais a lista.** As
> nove propostas daqui foram para a lista única de `docs/SPRINTS.md`,
> junto com as outras 22. Duas notas da travessia: o **sprint 5
> (`QUALIFY`) é recusado como prematuro** pelo documento do MariaDB(R), e
> a lista resolve isso por dependência em vez de por mérito
> (`SPRINTS.md` §2.4); e os **sprints 3 e 8 caem na trava de formato** —
> cinco propostas independentes dizem «PSCH v7», e cinco rodadas
> separadas custariam cinco migrações (§2.2).

Documento de **pesquisa e proposta**. Nada aqui foi implementado, e nada será
implementado sem o **sim do Adriano, sprint a sprint** — a lista existe para
ser aprovada, recusada ou reordenada por ele, e a última seção repete isso.

O contrato é o mesmo do `CONCORRENTES.md` e do `CASSANDRA.md`:

- **toda afirmação sobre o Teradata(R) traz a fonte** — o endereço da página
  oficial em `docs.teradata.com` e o nome da seção. Sem fonte, não entra;
- **todo número da casa é medido**, e a frase diz quando foi medido e por qual
  programa. O que é aritmética do formato aparece como *derivado*, não como
  medido;
- **cada sprint traz a premissa a medir primeiro** — a medição que pode
  **matar** o sprint. Matar é resultado válido, e este projeto já derrubou sete
  diagnósticos plausíveis desse jeito.

O Teradata(R) é um banco MPP *shared-nothing* de data warehouse. É o mundo mais
distante da casa entre as leituras já feitas — InnoDB e Aria são motores de um
nó que **decidem** como nós; o Cassandra(R) é distribuído mas não decide; o
Teradata(R) é distribuído **e** decide. É por isso que ele vale a leitura: quase
tudo que ele faz para dividir trabalho entre AMPs não cabe aqui, e **o descarte
fundamentado é metade do valor deste documento**.

---

## 1. O que foi lido

Documentação oficial do Teradata(R) Vantage, em `docs.teradata.com`:

| Assunto | Página |
|---|---|
| Parsing Engine (parser, otimizador, dispatcher, sessão) | `Database-Introduction/…/Virtual-Processors/Parsing-Engine` |
| Primary index, distribuição por hash, UPI/NUPI, *skew* | `Database-Design/Indexes-and-Maps/Primary-Indexes-and-Primary-AMP-Indexes` |
| Tabelas FALLBACK | `Database-Introduction/Vantage-and-RASUI/Software-Fault-Tolerance/Fallback-Tables` |
| Down AMP Recovery | `Database-Introduction/Concurrency-Control-and-Transaction-Recovery/System-and-Media-Recovery/Down-AMP-Recovery` |
| Transient journal | `SQL-Data-Definition-Language-Detailed-Topics/…/Transient-Journaling` |
| Permanent journal (BEFORE/AFTER, single/dual) | `SQL-Data-Definition-Language-Detailed-Topics/…/Assigning-a-Permanent-Journal-to-a-Table` |
| SET × MULTISET | `SQL-Fundamentals/Database-Objects/Tables/Duplicate-Rows-in-Tables` |
| QUALIFY | `SQL-Data-Manipulation-Language/SELECT-Statements/QUALIFY-Clause` |
| Queue tables | `SQL-Fundamentals/Database-Objects/Tables/Queue-Tables` |
| USI e NUSI | `SQL-Fundamentals/Database-Objects/Secondary-Indexes/USI-and-NUSI-Properties` |
| Join indexes | `SQL-Fundamentals/Database-Objects/Join-Indexes` |
| COLLECT STATISTICS | `SQL-Fundamentals/Query-Processing/Collecting-Statistics` |
| Multivalue Compression (MVC) | `Database-Design/Using-Data-Compression/Multivalue-Compression` |
| Macros | `SQL-Fundamentals/Database-Objects/Macros` |
| Integridade referencial (standard, batch, *soft*) | `SQL-Fundamentals/Database-Objects/Referential-Integrity/Referential-Integrity-Enforcement` |
| Tipos PERIOD e temporal | `Temporal-Table-Support/Basic-Temporal-Concepts/Transaction-Time-and-Valid-Time` |
| FastLoad — restrições | `Teradata-FastLoad-Reference-20.00/…/Restrictions-and-Limitations` |
| Workload management (TASM/TIWM) | `Teradata-VantageTM-Workload-Management-User-Guide-17.20/Introduction-to-Workload-Management/…` |
| MERGE — regra do ON | `SQL-Data-Manipulation-Language/…/MERGE/MERGE-Examples/Example-ON-Clause-Conditions-Must-Be-ANDed-With-The-Primary-Index-and-Partitioning-Column-Equality-Constraints` |
| Identity columns | `SQL-Data-Definition-Language-Detailed-Topics/…/Identity-Columns` |
| Row-level security | `Security-Administration/Implementing-Row-Level-Security/Working-with-Security-Constraint-Columns` |

**Uma honestidade sobre as fontes.** Boa parte das páginas de *Detailed Topics*
do portal devolve apenas o índice de navegação quando buscada por programa; o
corpo é montado no navegador. Quando isso aconteceu, este documento usa o que
conseguiu ler literalmente e **diz que usou**. Dois casos específicos:

- **MERGE**: o corpo não abriu. O que se afirma aqui vem do **título** de uma
  página de exemplo da documentação oficial, que já é a regra inteira: *«ON
  Clause Conditions Must Be ANDed With The Primary Index and Partitioning
  Column Equality Constraints»*. Nenhuma afirmação sobre o corpo do MERGE é
  feita além disso.
- **Identity columns**: o corpo não abriu. Sabe-se pelos títulos das páginas
  oficiais que existem as duas formas (`GENERATED ALWAYS` e `GENERATED BY
  DEFAULT`) e que há uma seção *«Process for Generating Identity Column
  Numbers»*. Nada além disso é afirmado — e não precisa ser, porque o item é
  descartado por já existir aqui (§5).

### As medições da casa feitas para este documento

Todas nesta máquina, na versão 0.18.0, com os programas que já estão no
repositório, depois de `cargo build --release --examples -p phxsql-store` —
que é a receita do `CLAUDE.md`, escrita porque uma rodada inteira de ganhos já
ficou invisível por causa de um binário velho.

```bash
./target/release/examples/onde-doi 200000
./target/release/examples/quanto-ocupa 200000 5
```

| | medido hoje |
|---|---:|
| inserção, 2 índices, esquema de 5 colunas | **10,3 µs/linha** |
| ↳ `.reg` + `.log` | 6,8 µs — **66,0%** |
| ↳ primeiro índice | 1,7 µs — 16,6% |
| ↳ conferir a chave única | 1,4 µs — 13,6% |
| ↳ segundo índice | 0,4 µs — 3,8% |
| páginas do `.ndx` gravadas por linha (depois do *write-back*) | **0,02** |
| CRC-32 de uma página de 4 KiB | 2,50 µs |
| `.reg`, em disco, de uma tabela de 200.000 linhas | 23,27 MiB — **45,27%** |
| `.ndx` | 17,94 MiB — 34,89% |
| `.log` | 8,81 MiB — 17,14% |
| razão do DEFLATE sobre o `.reg` | **4,76×** |
| inflar um volume de diário para servir 500 eventos | 39,1 ms contra 4,3 ms de leitura crua — **9×** |

> **Nota de manutenção, achada ao escrever este documento — e ela é sobre um
> número, que é o defeito que a casa mais persegue.** Eu ia apoiar dois sprints
> na frase «o `Decimal` e o `Date` levam a inserção de 7,50 para 16,61 µs».
> Ela **está derrubada**: o `DESEMPENHO.md` §4.8 registra que aquela medição
> saiu de um binário anterior ao *write-back* e que, recompilado, **o esquema
> da bancada custa ~0,4 µs a mais que o simples — 5%, e não 2,2×**. Só que o
> `docs/CASSANDRA.md` ainda carrega o número velho como verdade em **quatro
> lugares** (linhas 95, 313, 961 e 1252), inclusive na tabela «os números
> nossos usados como base». É o mesmo defeito que aquele documento anotou no
> `REPLICACAO.md`, e o conserto não é aqui: fica anotado para a rodada que
> tocar o arquivo. **Número digitado à mão envelhece calado — inclusive dentro
> do documento que inventou a frase.**

Duas dessas linhas mudam o filtro deste documento inteiro, e por isso vêm
primeiro:

1. **O `.ndx` deixou de ser o dono do tempo.** Depois do *write-back*, o custo
   dos dois índices é 3,5 µs de 10,3 — 34% —, e o `.reg`+`.log` é 66%. Toda
   proposta que ataque o índice está atacando um terço do problema; toda
   proposta que ataque o tamanho da linha está atacando dois terços.
2. **O `.reg` é 45,27% do disco e comprime 4,76×.** Há redundância medida
   dentro da linha, e ela nunca foi atacada — a compactação foi discutida duas
   vezes, e as duas vezes sobre o **diário**, que é 17,14%.

---

## 2. O que a casa já tem, e que este documento NÃO propõe

O briefing pede que estes apareçam como **ponto de partida**, e não como
novidade. Cada um é um lugar em que o PhxSql já chegou, sozinho, ao mesmo
desenho do Teradata(R).

**A integridade referencial declarada e não imposta já é a nossa.** O
Teradata(R) chama a terceira forma de *soft RI*: *«it provides a declarative
definition for a referential relationship, but it does not enforce that
relationship»*, e *«Enforcement of the declared referential relationship is
left to the user by any appropriate method»* (`Referential-Integrity-Enforcement`).
É exatamente o estado da FK aqui — declarada pelo protocolo no `criar_tabela`,
gravada no cabeçalho do `.reg`, preservada pelo `duplicar_tabela`, e **não
aplicada** —, com uma vantagem nossa: existe um teste que trava que *declarar
não é aplicar*, e ele falha no dia em que isso mudar em silêncio. O que o
Teradata(R) acrescenta é o **motivo** de a soft RI ser desejável e não uma
preguiça: ela existe para o otimizador. Isso só vira valor aqui quando houver
planejador — e é por isso que aparece no sprint 4, não como sprint próprio.

**O `BULKINSERT` já é o nosso FastLoad, e é melhor no que importa.** As
restrições do FastLoad são duras: a documentação diz que ele *«does not support
target tables defined with secondary indexes»*, *«does not support tables with
referential integrity»* nem tabelas com *«defined triggers»*, não mantém join
index, e a receita oficial para o caso comum é *«drop the secondary indexes.
Then load the table and recreate the secondary indexes»*
(`FastLoad-Reference-20.00/…/Restrictions-and-Limitations`). O `BULKINSERT`
daqui aceita tabela **com dados, com índices e com gatilhos** e mesmo assim
comprou 1,53× (43.500 → 66.500 linhas/s, medido), porque reservada a tabela a
janela de durabilidade não fecha e a carga vira um `fsync` só. E a receita do
FastLoad — largar o índice e refazer — **já foi medida aqui e recusada**: 1,22×
no melhor caso e prejuízo abaixo de M ≈ N/3 (`DESEMPENHO.md` §4.4). O que
falta para chegar ao FastLoad é outra coisa, e está no sprint 7.

**A partição alfanumérica `.pag` já existe, e não é a distribuição por hash.**
São 37 volumes fixos pela primeira letra de uma coluna (`FORMATO.md` §7). O
primary index do Teradata(R) distribui por **hash**: *«For each table row, the
values of these index columns are combined and hashed. The resulting numeric
hash value determines which AMP stores and manages the data in that row»*
(`Primary-Indexes-and-Primary-AMP-Indexes`). São coisas diferentes com um
problema em comum, e ele está tratado como **premissa a medir** em §5, não como
recomendação.

**A compactação já foi medida e recusada duas vezes**, e as duas sobre o
diário (`DESEMPENHO.md` §4.7 e §4.7.3). O sprint 3 propõe compressão, e é de
**coluna**, não de diário — a seção explica com números medidos hoje por que a
conta é outra, porque propor compressão nesta casa sem essa explicação seria
reabrir uma discussão já encerrada.

**Gatilhos e procedimentos entraram** (`docs/TRIGGERS.md`), com um
interpretador só para os dois (`crates/phxsql-sql/src/rotina.rs`), `CREATE
PROCEDURE`/`CALL` com parâmetros `IN`/`OUT`/`INOUT`, e o portão sendo o mesmo:
cada pedido que o corpo produz sai pelo `executar_derivado`, com a sessão de
quem chamou. **As macros do Teradata(R) são o degrau POR CIMA disso**, e o
sprint 6 registra a dependência.

**A replicação em quatro modos e o cluster com eleição entraram.** Nada neste
documento propõe replicação, quórum, eleição ou promoção — e o §5 explica por
que o FALLBACK, que é a resposta do Teradata(R) ao mesmo problema, continua
fora mesmo assim.

---

## 3. Os sprints

Ordenados por **valor medível ÷ custo**. Cada um tem escopo do tamanho de uma
rodada, e cada um começa por uma medição que pode matá-lo.

---

### Sprint 1 — A tabela-fila que se consome em ordem (P)

**O que é no Teradata(R).** Uma queue table é *«similar to ordinary base
tables, with the additional unique property of behaving like an asynchronous
first-in-first-out (FIFO) queue»*. Ela obriga uma coluna `TIMESTAMP` com
`DEFAULT CURRENT_TIMESTAMP`, e o verbo é o `SELECT AND CONSUME`: *«Data is
returned from the row with the oldest timestamp in the specified queue table.
The row is deleted from the queue table, guaranteeing that the row is processed
only once»*. Quando não há linha, a transação **espera** até que *«a row is
inserted into the queue table»* ou seja abortada. Um `SELECT` comum sobre a
mesma tabela é uma espiada que não consome. (`SQL-Fundamentals/…/Queue-Tables`)

**O que resolveria aqui.** A casa acumulou consumidores de fila e não tem
fila: os jobs, o laço da réplica, a sincronia de tabelas primas do DbLink, e
agora o aviso por e-mail. Hoje uma fila se escreve como `varrer` + `excluir`, e
duas sessões que varrem ao mesmo tempo pegam a mesma linha — o pop atômico não
existe.

**Por que ele é barato exatamente aqui, e caro em quase todo outro motor.** As
três peças de que uma fila precisa já são propriedades do formato, e nenhuma
foi feita para isso:

- **a ordem de digitação é a ordem FIFO.** O `.reg` devolve as linhas na ordem
  em que foram digitadas, e essa é a regra que o projeto mais defende. Numa
  LSM ou num InnoDB a ordem física é a da chave, e a fila precisa de um índice
  para existir. Aqui ela **é** o arquivo;
- **o `rownum` já é o carimbo sequencial**, preenchido pelo motor, nunca
  reaproveitado, e `rowid_do_rownum` acha por bissecção — 20 leituras num
  milhão, sem índice;
- **a exclusão suave já existe**, então «consumido» é uma marca, e a linha
  continua inteira para auditoria — o que o Teradata(R) não oferece, porque lá
  a linha é apagada.

Ou seja: a coluna `TIMESTAMP` obrigatória do Teradata(R) aqui **não precisa
existir**, porque o `rownum` e o `.log` já carimbam.

**A premissa a medir primeiro — e ela pode matar o sprint.** Duas perguntas, e
a segunda é a assassina:

1. *O pop concorrente é atômico de graça?* A trava global de dados serializa
   toda operação, então dois clientes não deveriam conseguir consumir a mesma
   linha. Isso **não se prova por teste unitário** — é a lição que o
   `BULKINSERT` cobrou caro (`CLAUDE.md`: teste unitário não prova queda de
   conexão, soquete prova). A prova é dois clientes por soquete, em
   `bancada/`, consumindo 10.000 linhas e conferindo que a união é exata e a
   interseção é vazia.
2. *Achar a primeira linha não consumida continua barato quando a fila tem um
   milhão de consumidas?* Se o consumo virar uma varredura linear a partir do
   rowid 1, a fila degrada com a idade e **o item morre ali**. O número que
   decide: uma fila com 1.000.000 de linhas consumidas e 100 pendentes tem de
   entregar a próxima em tempo comparável ao `pular` de hoje — **6 ms no fim
   de 200.000 linhas**, medido no pedido 107. Acima de 50 ms, o desenho tem de
   guardar a marca da cabeça da fila, e isso é outro sprint, não este.

**Dependências.** A espera («a transação espera até que uma linha seja
inserida») é a **mesma primitiva** que o `CASSANDRA.md` §6.2 já propôs para o
*long-poll* da replicação: uma `Condvar` da `std`, com o sinal vindo de quem
grava o evento e a espera **fora da trava global** — a armadilha registrada lá,
que congelaria o master, é a mesma que congelaria o servidor aqui. Se o
long-poll entrar pela replicação, a fila herda a peça pronta. Se este sprint
vier antes, ele entrega a peça para a replicação. **Anotar a sobreposição é
mais importante do que decidir a ordem.**

**O que NÃO entra.** O `SELECT AND CONSUME` do Teradata(R) é transacional: um
*rollback* devolve a linha para a fila. Aqui não há transação, então o contrato
é outro e tem de estar **escrito na resposta e no manual**: a linha é entregue
uma vez, e se o cliente morrer depois de recebê-la, ela já foi. Prometer
devolução sem mecanismo seria o defeito que o projeto recusa por escrito.
Também fica fora a espera com prazo configurável por consulta — a primeira
versão espera ou não espera.

---

### Sprint 2 — A imagem anterior no diário (M)

**O que é no Teradata(R).** São dois journals distintos, e a distinção importa.
O *transient journal* existe para desfazer: ele *«provides rollback capability
for a transaction»* (`…/Transient-Journaling`). O *permanent journal* é
declarado por tabela e guarda `BEFORE JOURNAL` e/ou `AFTER JOURNAL`, simples ou
duplo, e serve para recuperar a um ponto no tempo
(`…/Assigning-a-Permanent-Journal-to-a-Table`).

**O que resolveria aqui.** O `.log` v2 já guarda a imagem **depois** da linha —
é o *after journal*, e é o que faz a replicação funcionar. Falta a imagem de
**antes**. E isso não é uma ideia importada: é literalmente o que a pendência
11 (Transações) diz faltar — *«não há journal com a imagem anterior da linha»*.
O Teradata(R) só dá o nome e a prova de que a peça se separa do resto: **ele
tem before image sem que isso obrigue a ter transação**, porque o permanent
journal é uma opção de tabela, e o rollback mora no outro journal.

Com a imagem anterior passam a existir três coisas que hoje não existem:

- **desfazer uma operação pontual** — «volte a linha 4.173 ao que era às 14h»;
- **auditoria que mostra o que mudou**, e não só que mudou. Hoje o `.log` diz
  que houve alteração e como a linha ficou; quem quer o *diff* precisa do
  evento anterior daquela linha, que pode estar a milhões de eventos atrás;
- **o primeiro tijolo real da transação**, sem prometer o verbo.

**Por que agora.** *Mudança de formato entra cedo* — é regra da casa. O diário
já foi de v2 para v3 quando a cifra entrou, e o cabeçalho já tem versão. Uma v4
com a imagem anterior é barata hoje e vira migração depois.

**A premissa a medir primeiro.** *Quanto custa a imagem anterior por evento?*
O que já se sabe, medido (`DESEMPENHO.md` §2.2): o evento **sem** imagem custa
0,67 µs e **com** a imagem depois custa 1,61 µs — 2,4×. A pergunta aberta é se
a imagem de antes custa o mesmo, e há razão para achar que custa **muito
menos**: `atualizar` e `excluir` já leem o slot antes de gravar, porque a
versão do registro está lá e a janela de conflito a confere. Se o payload
anterior já está em RAM no momento da gravação, o custo é o `write` e mais
nada.

O número que decide, e ele é combinado **antes**: o `atualizar` da bancada mede
0,454 s para 20.000 linhas — **22,7 µs por linha**. Se a imagem anterior custar
acima de 10% disso (2,27 µs), ela entra **desligada por padrão**, como a cifra
entrou; abaixo, pode nascer ligada. E, em qualquer dos dois casos, vale a regra
que o projeto escreveu com todas as letras: **guarda nova entra pedida, não
imposta** — arquivo v3 continua abrindo, e cliente que não pede continua igual.

**Dependências.** Nenhuma frente em andamento. Toca o formato do `.log`, que é
a fonte da replicação — então o teste que mais importa **não é o da imagem
nova**: é o de que uma réplica que não conhece a v4 continua aplicando os
eventos como antes.

**O que NÃO entra.** `BEGIN`/`COMMIT`/`ROLLBACK`. Isso é a transação inteira —
identificador de transação na sessão, desfazer em cascata, isolamento —, e o
`SQL.md` já registra por que prometer o verbo sem o mecanismo é pior do que não
ter o verbo. Também fica fora o *journal duplo* do Teradata(R) (duas cópias do
diário): a resposta da casa para perda de arquivo é o `.bkp` e a replicação.

---

### Sprint 3 — Dicionário de coluna: a compressão que cabe num slot fixo (M)

**O que é no Teradata(R).** A *multivalue compression* guarda os valores
frequentes uma vez só, na estrutura da tabela: *«the database stores the value
only once in the table header, regardless of how many times it occurs as a
field value»*. Declara-se na criação — `jobtitle CHARACTER(30) COMPRESS
('cashier', 'manager', 'programmer')` —, o limite é de 255 valores por coluna,
e a documentação diz que ela tem *«the best cost/benefit ratio compared to
other methods»* (`Database-Design/Using-Data-Compression/Multivalue-Compression`).

**Por que a conta é diferente da compactação do diário, recusada duas vezes.**
Esta é a parte que precisa vir antes da proposta, e ela é sustentada por três
números medidos **hoje**, nesta máquina:

| | medido hoje | a recusa do diário |
|---|---:|---|
| massa que o alvo ocupa | `.reg` = **45,27%** da tabela | `.log`+`.trash`+`.reason` = 19,84% |
| redundância disponível ali | DEFLATE do `.reg` = **4,76×** | DEFLATE do `.log` = 4,16× |
| custo de LER o que foi comprimido | **nenhum**: o slot continua endereçado por conta | inflar o volume: **39,1 ms** contra 4,3 ms — **9×** |

O terceiro é o que decide. A compactação do diário morreu porque *«um volume
compactado não se lê por dentro»*: para servir 500 eventos é preciso inflá-lo
inteiro, e o servidor abre e fecha a tabela a cada pedido, então não há cache
que segure o volume inflado. **Um dicionário de coluna não tem esse custo**: o
valor está num vetor de até 255 entradas no cabeçalho, e ler a coluna é
indexar esse vetor. `offset = data_offset + (rowid−1) × slot_size` continua
valendo byte por byte.

**E há uma segunda diferença, que é o motivo de isto caber num slot fixo.** O
Teradata(R) pode deixar fora da lista os valores raros porque a linha dele tem
largura variável. Aqui o slot é fixo — então o desenho que cabe **não é** a MVC
tal como está lá: é um **domínio fechado**, um `ENUM`, em que a coluna passa a
ocupar 1 byte e o dicionário mora no bloco de esquema. A largura continua fixa;
ela só fica **menor, para todas as linhas**. É por isso que a ideia sobrevive à
regra que mata quase tudo neste motor.

**O tamanho do ganho, derivado e não medido.** Na tabela do próprio medidor
(`quanto-ocupa`), a coluna `cidade` é `Str(20)` e recebe **8 valores
distintos**. Um código de 1 byte no lugar de 20 poupa 19 bytes por linha; o
`.reg` mede 23,27 MiB para 200.000 linhas, ou ~122 bytes por slot. São **~15,6%
do `.reg` e ~7% da tabela inteira, de uma única coluna**. O número é
**aritmética do formato sobre uma largura medida** — não é uma medição do
dicionário, que não existe —, e a frase diz isso de propósito.

**A premissa a medir primeiro, e ela é sobre dado real.** *Quantas colunas de
uma base de verdade têm domínio de 255 valores ou menos?* Sem essa resposta o
ganho é hipotético, e a resposta não está nesta máquina: está na base do
Adriano. O medidor é pequeno e já tem peça pronta — o `SelectMemory` monta
mapas por coluna e é 87× mais rápido que o disco, medido. Uma varredura que
conta valores distintos por coluna responde em uma passada.

**O critério combinado antes:** se numa tabela real do Adriano as colunas de
domínio fechado somarem **menos de 10% da largura do slot**, o sprint morre —
porque 10% de 45,27% é 4,5% do disco, e não paga uma mudança de formato.

**A segunda premissa, que ninguém lembra de medir.** Espaço não é o número
inteiro. Trocar um `memcpy` de 20 bytes por uma busca em vetor **pode custar
CPU** no caminho mais quente que este motor tem — e a inserção hoje é 66%
`.reg`+`.log`. O `--example onde-doi` mede isso com uma coluna dicionarizada
contra a mesma tabela sem: se a inserção subir acima de 5%, o ganho de disco
está sendo pago com o tempo que a casa acabou de comprar (16,4 → 7,5 µs em uma
rodada).

**Dependências.** Mudança do bloco de esquema (PSCH v6 → v7), com o campo no
**fim** do bloco, que é a receita que a marca de dado pessoal já usou para que
quem lê a v6 pare antes. Nenhuma frente em andamento toca isso.

**O que NÃO entra.** Compressão de bloco no `.reg` (DEFLATE por volume) — é
exatamente a mesma conta que já foi recusada no diário, e pelo mesmo motivo: o
slot deixaria de se ler por dentro. E fica fora `ALTER` que dicionariza coluna
de tabela cheia: reescrever todos os slots é migração, não sprint.

---

### Sprint 4 — Cardinalidade por índice, para o tradutor escolher (P)

**O que é no Teradata(R).** `COLLECT STATISTICS` guarda a demografia das
colunas no dicionário de dados, e *«The Optimizer uses the synopsis data when
it generates its table access and join plans»*. Há amostragem que detecta
*skew* e aumenta a amostra sozinha, e a recoleta implícita mantém o mesmo modo
da coleta original (`SQL-Fundamentals/Query-Processing/Collecting-Statistics`).
O histograma padrão é de 250 intervalos
(`SQL-Request-and-Transaction-Processing/…/Interval-Histograms`).

**O que resolveria aqui.** O `SQL.md` é explícito sobre o buraco: *«Planejador.
Escolher qual índice usar quando há dois candidatos. Hoje quem chama escolhe,
dizendo o nome do índice»*. Com o driver ODBC entregue, quem escolhe deixou de
ser um programa da casa e passou a ser o Excel(R) — que não sabe o nome de índice
nenhum.

**Por que é o mais barato da lista.** A estatística mais útil de todas — quantas
chaves distintas cada índice tem — **já está gravada**: o cabeçalho do `.ndx`
guarda `qtd_chaves` por índice, e `verificar()` já sabe recalculá-la varrendo.
Não há coleta a escrever; há um número a ler e uma regra de desempate a
escrever no tradutor.

**A premissa a medir primeiro.** *Qual a diferença real entre escolher o índice
certo e o errado?* Se for pequena, o item morre e a medição é a entrega — mais
um diagnóstico plausível derrubado. A medição: a mesma consulta com dois
índices candidatos (um seletivo, um não), resolvida pelos dois caminhos.

**O número que decide, combinado antes:** abaixo de **2×** de diferença, não
compensa — porque o tradutor passaria a ter uma regra a mais para errar, e a
casa já pagou caro por segundos caminhos até o dado. Acima de 10×, é o item
mais rentável do documento e sobe na lista.

**Dependências.** A camada SQL e a op `sql` (entregues). Nada em andamento.

**O que NÃO entra.** Histograma por intervalo, amostragem, recoleta automática
e estatística de coluna **sem** índice — as três primeiras são o subsistema
inteiro do Teradata(R), e a última exige varrer a tabela, que é o custo que
esta proposta existe para evitar. Cardinalidade de índice, e nada mais.

---

### Sprint 5 — QUALIFY, com as duas funções de janela que ele exige (M)

**O que é no Teradata(R).** O `QUALIFY` filtra pelo resultado de uma função
analítica ordenada — o que o `WHERE` não consegue, porque no `WHERE` a função
de janela ainda não foi calculada. A avaliação vem **depois** de `WHERE`,
`GROUP BY` e `HAVING`, e a documentação tem uma seção só para essa ordem
(`SQL-Data-Manipulation-Language/SELECT-Statements/QUALIFY-Clause` e
`…/Usage-Notes/Evaluation-Order-of-WHERE-GROUP-BY-and-QUALIFY-Clauses`).

**O que resolveria aqui.** «Os 3 maiores pedidos de cada cliente» é a consulta
que todo relatório pede e que hoje não tem como escrever — nem com subconsulta,
que também não existe. Com o driver ODBC entregue, isso deixou de ser teórico:
é o que uma ferramenta de BI gera sozinha.

**Escopo fechado.** `ROW_NUMBER()` e `RANK()`, com `PARTITION BY` e `ORDER BY`,
e o `QUALIFY` sobre elas. Duas funções, uma cláusula.

**A premissa a medir primeiro.** *A ordenação sai de graça do `.ndx`?* Um
`ROW_NUMBER() OVER (ORDER BY col)` com índice em `col` é a varredura pelo
índice, que já existe e não ordena nada — a ordem sai do `.ndx`, como o
`SQL.md` já registra. A pergunta é o `PARTITION BY`: ele exige agrupar, e a
casa já tem `pivotar`, com *hash join* e seis resumos.

**O que decide o tamanho do sprint:** se o `pivotar` já resolver o
agrupamento, isto é tradução — e tradução de coisa medida e testada é o que o
`SQL.md` chama de trabalho que cabe. Se exigir ordenar em memória a tabela
inteira, o sprint dobra e vira G, e aí ele volta para a mesa antes de começar.

**Dependências.** Camada SQL (entregue), `varrer` por índice (existe),
`pivotar` (existe). O tradutor já recusa dizendo o nome da cláusula que não
suporta, e é essa disciplina que o `QUALIFY` herda: **o que não tiver substrato
recusa**, em vez de devolver a tabela inteira como se fosse a resposta.

**O que NÃO entra.** Molduras de janela (`ROWS BETWEEN`), agregados acumulados
sobre janela, `LAG`/`LEAD`, e as demais funções ordenadas. São o resto de um
capítulo grande do manual, e nenhuma delas é o que o relatório pede primeiro.

---

### Sprint 6 — Macro: a consulta parametrizada salva, por cima das procedures (P)

**O que é no Teradata(R).** *«A macro consists of one or more statements that
can be executed by performing a single statement»*, executada com `EXECUTE`, e
*«Performing a macro is similar to performing a multistatement request»*. Ela
pode conter um `EXECUTE` de outra macro
(`SQL-Fundamentals/Database-Objects/Macros`).

**O degrau, e por que ele é por cima e não ao lado.** Os procedimentos entraram
nesta rodada, com um interpretador próprio — `BEGIN…END`, `DECLARE`, `IF`,
`WHILE`, `SIGNAL` (`docs/TRIGGERS.md`). **Uma macro não precisa de nada disso**:
ela é texto SQL com parâmetros, e quem a executa é o tradutor que já existe.
É a peça mais leve das três (macro < procedure < gatilho), e cobre o caso mais
comum de todos — a consulta do dia a dia, salva com nome, que hoje cada
programa cliente guarda por conta própria.

**O valor que não é comodidade, e sim segurança.** O parâmetro tem de entrar
como **valor** no pedido traduzido, nunca por concatenação de texto. É a mesma
lição que o projeto já escreveu duas vezes: a rotina **produz** o pedido que o
portão já sabe conferir, em vez de ganhar uma porta própria — e o teste que
trava isso já tem nome na casa (`call_nao_e_a_porta_dos_fundos_para_a_tabela_negada`);
o da macro seria o irmão dele. Uma macro que montasse SQL por concatenação
seria a porta dos fundos que a casa fechou duas vezes.

**A premissa a medir primeiro — e ela pode matar o sprint em uma tarde.** *Uma
macro é mesmo mais barata que a procedure equivalente de uma instrução só?* Se
`CALL` de um procedimento de uma linha já custa o mesmo, a macro é açúcar
sintático e **não deve existir** — porque um objeto novo no catálogo, com
permissão própria e tela própria, é custo permanente. O medidor já está
escrito: `cargo run --release -p phxsql-server --example custo-do-portao`, na
forma intercalada que a rodada dos gatilhos usou (e que mostrou que o portão
não aparece acima do ruído). Se a diferença ficar dentro do espalhamento, o
item morre com o número na mesa.

**Dependências.** `docs/TRIGGERS.md`, entregue nesta rodada. **Este sprint não
existe sem aquele**, e é a dependência mais forte do documento.

**O que NÃO entra.** Macro que executa macro — o Teradata(R) permite, e aqui
abriria recursão sem limite dentro do despachar. E macro com DDL: guardar um
`DROP TABLE` parametrizado para rodar depois, com o poder de quem chamar, é a
escalada que a regra dos gatilhos já recusou ao exigir `administrar`.

---

### Sprint 7 — As recusadas que ficam, e a carga que retoma (P)

**O que é no Teradata(R).** O FastLoad separa o que não entrou em *error
tables*, e a documentação tem um procedimento próprio para corrigir a primeira
delas (`FastLoad-Reference-20.00/…/Handling-Teradata-FastLoad-Errors`). A carga
é retomável por *checkpoint*.

**O que resolveria aqui.** O `inserir_lote` devolve as recusadas **na
resposta**, com o número da linha — e nada persiste. Numa carga de um milhão
pela rede, a resposta é grande demais para ser lida por gente, e uma conexão
cortada leva junto a única cópia da lista. E há o buraco irmão, que o
`CASSANDRA.md` §6.4 já nomeou: a nossa escrita **não é idempotente** — `inserir`
tira rowid novo a cada chamada —, então repetir uma carga interrompida duplica.

**Escopo fechado.** As linhas recusadas de uma carga vão para um arquivo ao
lado da tabela, no mesmo espírito do `.reason` (que já guarda motivo, autor e
carimbo de quem excluiu), e a resposta passa a dizer **onde elas estão** em vez
de carregá-las inteiras.

**A premissa a medir primeiro.** *A receita da chave externa já resolve a
retomada?* O `CASSANDRA.md` §6.4 propõe uma coluna do cliente sob índice único:
como a conferência de unicidade acontece **antes de qualquer gravação**
(`table.rs`), repetir o lote inteiro recusa as que já entraram e a carga vira
segura para repetir do começo. Se a prova passar — 1.000 linhas, interrupção na
700, repetição do lote inteiro, e a tabela terminando com **exatamente 1.000** —
então a retomada é **documentação e teste**, não código, e este sprint encolhe
para só a error table, virando P de verdade.

**Dependências.** `BULKINSERT` e `inserir_lote` (entregues). Nenhuma frente em
andamento.

**O que NÃO entra — e aqui o Teradata(R) é o exemplo do que não copiar.** As
restrições do FastLoad seriam uma **regressão**: exigir tabela vazia, sem
índice secundário, sem gatilho e sem RI é abrir mão do que o `BULKINSERT` já
faz com tabela cheia e indexada. E a receita de largar o índice e recriá-lo já
foi medida e recusada aqui (1,22× no melhor caso, prejuízo abaixo de M ≈ N/3).

---

### Sprint 8 — O tipo PERIOD e os dois predicados de vigência (M)

**O que é no Teradata(R).** Uma coluna de tipo `PERIOD` *«stores a pair of DATE
or TIMESTAMP values that define the beginning and end»* do período. Sobre ela
se constroem o *valid time* (quando o fato vale no mundo) e o *transaction
time* (quando foi registrado), e uma tabela com os dois é bitemporal
(`Temporal-Table-Support/Basic-Temporal-Concepts/Transaction-Time-and-Valid-Time`).

**O que resolveria aqui.** Vigência é problema de ERP, não de data warehouse:
preço que vale de tal a tal data, contrato, alocação, tabela de comissão. Hoje
isso se escreve com duas colunas `Date` e comparações à mão — e é sempre na
fronteira que erra (o dia do fim entra ou não entra?).

**Escopo fechado.** O **tipo** (par de `Date`/`DateTime`, largura fixa no slot,
com a fronteira definida no formato de uma vez por todas) e **dois predicados**:
sobreposição e contenção. Nada mais.

**A premissa a medir primeiro — e aqui a expectativa é o contrário do que eu
ia escrever.** *Quanto custa por linha, contra as duas colunas `Date` que ele
substitui?* A tentação era citar o `DESEMPENHO.md` para dizer que codificar
coluna é caro aqui — e esse número **é justamente um dos que a casa já
derrubou**: a frase «o `Decimal` e o `Date` levam a inserção de 7,50 para
16,61 µs» foi medida com um binário anterior ao *write-back*, e o §4.8 registra
a correção com todas as letras — recompilado, **o esquema da bancada custa
~0,4 µs a mais que o simples, 5%, e não 2,2×**.

Então a expectativa honesta é que um tipo composto novo custe **pouco**, e a
medição serve para confirmar isso e não para descobri-lo. `--example onde-doi`
com uma coluna `PERIOD` contra duas colunas `Date`.

**O que a medição decide, e é uma escolha entre dois sprints diferentes:** se o
`PERIOD` custar o mesmo que as duas colunas `Date` — o resultado provável —,
ele é **açúcar sintático com valor de correção**, e ainda vale, porque a
fronteira certa gravada uma vez no motor vale mais que a mesma regra repetida
em quarenta telas; mas aí é um sprint **P** de tipo derivado, e não M. Se
custar menos (uma codificação em vez de duas), é ganho de verdade. **O
documento não promete qual dos dois é**, e a diferença entre eles é o tamanho
do sprint.

**Dependências.** Mudança de formato no bloco de esquema, e nenhuma frente em
andamento. **Candidato compartilhado com a análise do MariaDB(R)** — as
*system-versioned tables* de lá atacam o mesmo assunto por outro lado, e a
consolidação é da integração.

**O que NÃO entra.** `VALIDTIME`/`TRANSACTIONTIME` automáticos, tabelas
bitemporais, e as consultas `CURRENT`/`SEQUENCED`/`NONSEQUENCED`. A razão é de
substrato, não de tamanho: a manutenção automática de período fecha o período
antigo e abre um novo **dentro do mesmo UPDATE**, e sem transação as duas
escritas podem ficar pela metade. É a mesma razão que mantém `COMMIT` fora do
sprint 2.

---

### Sprint 9 — Direito por linha (G)

**O que é no Teradata(R).** A *row-level security* põe colunas de restrição na
tabela e decide o acesso linha a linha por UDFs de política
(`Security-Administration/Implementing-Row-Level-Security/Working-with-Security-Constraint-Columns`;
`SQL-External-Routine-Programming/…/Security-Constraint-UDFs`).

**O que resolveria aqui.** A casa foi de direito por **base** (desde sempre) a
direito por **tabela** (pedido 124) e marca de dado pessoal por **coluna**
(pedido 125). Falta a linha: «o vendedor vê os clientes dele». Hoje isso se
faz na aplicação, que é onde ele não deveria estar.

**O desenho que caberia, e o que ele não copia.** A UDF de política do
Teradata(R) **não cabe**: seria código do usuário no caminho de toda leitura de
toda linha. O que cabe é uma coluna de sistema com o rótulo — a mesma receita
de `softdeleted` e `rownum`, que entram sozinhas na criação e vão **no fim da
lista** para não deslocar as colunas do usuário — e a regra no cadastro, ao
lado de `"tabelas"`.

**As duas armadilhas que a casa já pagou, e que este sprint pisaria em cheio.**
São o motivo de ele ser G e de estar no fim da lista:

1. **Coluna de sistema nova quebra quem filtra pela primeira.** O `rownum`
   quebrou *todo salvar e todo incluir* pela tela quando entrou, e o padrão dos
   três defeitos era o mesmo: `find(...)` onde devia ser `filter(...)`. Uma
   terceira coluna de sistema procura exatamente esses lugares de novo.
2. **O portão é UM só, e o campo que ele lê é o furo.** O direito por tabela
   custou conferência própria em `juntar` e `unir`, que não têm o campo
   `"tabela"`. Um direito por **linha** tem de valer no `varrer`, `buscar`,
   `juntar`, `unir`, `pivotar`, `sql` e no `CALL` de uma rotina — e o que
   alguém esquecer vira a porta dos fundos que ninguém acha por leitura.

**A premissa a medir primeiro.** *Quanto custa por linha lida?* Um filtro por
rótulo entra no caminho da leitura, e a leitura é onde este motor ganha hoje —
varrer é 11,1× o MySQL(R) na bancada de dez milhões (1,41 s contra 15,70 s).
**O número que decide, combinado antes: se a varredura da bancada passar de
1,55 s (10%), o desenho está errado** e o rótulo tem de virar índice, não
filtro.

**O teste que mais importa não é o do recurso novo.** É
`sem_regra_de_linha_nada_muda` — o irmão do
`sem_regra_de_tabela_nada_muda` que já existe. Regra que muda o significado da
configuração que já está lá tira o direito de alguém sem ninguém ter pedido.

**O que NÃO entra.** Níveis e categorias hierárquicos, e a UDF de política.

---

## 4. O que foi descartado, e por quê

Num motor de arquivos separados de um nó, a maior parte de um MPP não cabe.
Esta seção existe para que ninguém precise reabrir a discussão, e cada linha
diz **o que reabriria** o item.

### 4.1 AMPs, BYNET e o Parsing Engine distribuído

O PE *«is the vproc that communicates with the client system on one side and
with the AMPs (by the BYNET) on the other side»*, e ele decompõe o SQL em
passos possivelmente paralelos (`…/Virtual-Processors/Parsing-Engine`). É a
arquitetura inteira do Teradata(R), e ela pressupõe N processadores com discos
próprios. Aqui há um processo, um disco e **uma trava global de dados**.

**Fora.** O que a casa tem no lugar — réplicas, os quatro modos de replicação e
o cluster com eleição — resolve disponibilidade e escala de **leitura**, que é
outro problema.

### 4.2 A distribuição por hash do PRIMARY INDEX — fora, com a medição que a reabriria

O hash do primary index decide **em qual AMP a linha mora**. Sem AMPs, ele não
tem para onde distribuir: aqui ele só decidiria **em qual arquivo** a linha
cai. E aí ele bate de frente com a regra que define o projeto — o rowid é
endereço (`offset = data_offset + (rowid−1) × slot_size`), e a partição
alfanumérica só sobrevive porque a conta dela é a inversa exata da de sempre.

**Mas há um problema real que o Teradata(R) nomeia e que a casa tem.** É o
*skew*: *«one or more AMPs have significantly more or less data to process than
others, the database is said to have a skewed data distribution, and
performance suffers»*. A partição `.pag` alfanumérica corta por **primeira
letra** em 37 volumes fixos — e nomes próprios não se distribuem por letra. Um
volume `_S` gigante e um `_X` vazio é o mesmo fenômeno com outro nome.

**O que este documento faz com isso: escreve a premissa, não a proposta.** A
medição que reabriria o assunto é sobre a base do Adriano, não sobre esta
máquina — contar linhas por volume numa tabela real particionada por letra. Se
o maior volume passar de **3× a média**, existe um problema a resolver, e aí a
conversa é sobre um terceiro modo de partição. Se não passar, não há item.
**Recomendar hash aqui sem esse número seria exatamente a receita de fora
virando plano sem medir o nosso gargalo** — que é o erro que o `CLAUDE.md`
proíbe pelo nome.

### 4.3 FALLBACK

*«Each row in a fallback table is stored on an AMP different from the one to
which the primary row hashes»*, e o preço é declarado: *«it doubles the storage
space and the I/O (on INSERT, UPDATE, and DELETE statements)»*
(`…/Software-Fault-Tolerance/Fallback-Tables`).

**Fora, e o número é nosso.** A bancada de dez milhões mede o PhxSql ocupando
**2,43 GiB contra 0,88 GiB do MySQL(R)** — já é 2,8×, e é o preço conhecido do
slot fixo. Dobrar isso levaria a 4,86 GiB para comprar redundância que a casa
**já tem por dois outros caminhos**: o `.bkp` espelho, escrito no mesmo instante
que o `.reg`, e a replicação. E o `.reg` já tem CRC por slot com segunda
chance no espelho, que é o mesmo serviço do *«if there is a data read error,
Vantage can repair the primary copy of the data using the fallback copy»*.

### 4.4 Join indexes e NUSI com covering columns

Um join index é *«a file structure designed to permit queries … to be resolved
by accessing the index instead of having to access and join their underlying
base tables»* (`SQL-Fundamentals/Database-Objects/Join-Indexes`). Um NUSI pode
*«include covering columns»* (`…/USI-and-NUSI-Properties`).

**Fora, e não é pelo custo do índice.** Medido hoje, o segundo índice custa
0,4 µs de 10,3 (3,8%) — depois do *write-back*, índice ficou barato. É pior que
isso: um join index é o **resultado de uma junção** mantido na escrita, então
toda gravação em **qualquer** das tabelas envolvidas teria de reavaliar a
junção. Isso põe no caminho crítico da escrita um trabalho que hoje é da
leitura — e a escrita é onde este motor ainda perde uma fase (excluir, 6,27 s
contra 4,73 s do MySQL(R)).

**O que reabriria:** um caso real em que a mesma junção é feita muitas vezes
sobre dados que quase não mudam. Aí o nome certo não é join index, é **vista
materializada com recomposição agendada** — e a casa já tem jobs para agendar.

### 4.5 Workload management (TASM)

*«A workload is a class of database requests with common traits whose access to
the database can be managed with a set of rules»*, com *throttles* por sessão,
por pedido e por sistema, e três camadas de prioridade
(`Workload-Management-User-Guide-17.20/Introduction-to-Workload-Management`).

**Fora, por uma razão estrutural.** Escalonar pressupõe trabalho concorrente a
escalonar, e aqui **uma trava única serializa todo acesso a dados** — está na
pendência 12 como o que falta. Prioridade sobre uma fila de um só não é
prioridade.

**O que reabriria:** a trava por tabela (pendência 12). Depois dela, o item
mais simples da família — um teto de sessões por usuário — passa a fazer
sentido; a família inteira, não.

### 4.6 Tabela SET (proibir linha duplicada inteira)

Uma tabela SET *«is defined not to permit duplicate rows … because its
properties are based on set theory»*; a MULTISET permite
(`SQL-Fundamentals/…/Duplicate-Rows-in-Tables`). O Teradata(R) tem uma seção só
para o custo disso — *«Duplicate Row Checks for SET Tables with NUPIs»*.

**Fora, e a razão daqui é mais forte que a de lá.** Conferir linha inteira a
cada inserção é uma leitura por linha inserida. E o `.reg` **nunca reaproveita
slot**: recusar depois de gravar deixaria buraco permanente, e recusar antes
custaria a comparação do payload inteiro. O índice único já entrega o que
importa, e a conferência dele custa 1,4 µs medidos — sobre a **chave**, não
sobre a linha.

### 4.7 Transient journal e rollback

*«Provides rollback capability for a transaction»* (`…/Transient-Journaling`).
Sem transação não há o que reverter, e o sprint 2 propõe deliberadamente a
metade que **não** depende dela — a imagem anterior, que no Teradata(R) mora no
*outro* journal. Copiar o transient journal seria começar pela ponta que exige
tudo o mais.

### 4.8 Tabelas bitemporais completas

Descartado dentro do sprint 8, e pelo motivo de substrato ali explicado.

### 4.9 Identity columns

Descartado **porque já existe**: `Sequence`, `Uuid` v7 e `Uuid256` conferidos
contra o vetor do RFC 9562, mais a tabela `sequences` na raiz do banco com o
contador ajustável pelo admin (pedido 81). Do Teradata(R) sobrou o vocabulário
(`GENERATED ALWAYS` × `GENERATED BY DEFAULT`), que é o que um dialeto SQL
precisaria aceitar um dia — e isso é assunto do parser, não do motor.

### 4.10 MERGE

**Fora deste documento, e não por descarte técnico.** O `MERGE` é útil e cabe;
o que impede é a fonte: o corpo das páginas não abriu (§1), e a única coisa que
se pode afirmar com citação é a regra do `ON`. **Propor um sprint sobre um
comando que não foi lido seria inventar** — e a casa tem regra contra número
citado; vale igual para semântica citada. Fica registrado como o primeiro item
a levantar numa segunda rodada de leitura.

Vale notar que a operação irmã **já existe** por baixo: o empurrão reentrável
da sincronia do DbLink (`ON DUPLICATE KEY UPDATE`) é a metade `WHEN MATCHED` /
`WHEN NOT MATCHED` resolvida no caso que a casa precisava.

---

## 5. Candidatos compartilhados com as análises irmãs

Anotados, não resolvidos — a consolidação é da integração.

| Candidato | Aqui | Irmã |
|---|---|---|
| **Fila com pop bloqueante** (sprint 1) | `SELECT AND CONSUME` do Teradata(R) | **Redis(R)** — `BLPOP` é o mesmo verbo com outro nome. Se as duas análises propuserem fila, é **uma** fila |
| **Espera com `Condvar`** (sprint 1) | a espera da queue table | **Cassandra(R)** §6.2, já escrito na casa: *long-poll* no source. Mesma primitiva, dois usos |
| **Vigência / temporal** (sprint 8) | tipo `PERIOD` | **MariaDB(R)** — *system-versioned tables* |
| **Identity / sequence** (§4.9) | já existe aqui | **MariaDB(R)** — `SEQUENCE`. Provável sobreposição de vocabulário |
| **Estatística para o otimizador** (sprint 4) | cardinalidade de índice | **MariaDB(R)** — histogramas do otimizador |
| **Imagem anterior** (sprint 2) | `BEFORE JOURNAL` | **MariaDB(R)/InnoDB** — *undo log*. Lá é meio de transação; aqui é fim em si |

---

## 6. Resumo

| # | Sprint | Tam. | Premissa a medir primeiro | Dependência |
|---:|---|:--:|---|---|
| 1 | Tabela-fila que se consome em ordem | **P** | achar a próxima pendente com 1M consumidas custa como o `pular` (6 ms)? Acima de 50 ms, o desenho muda | `Condvar` compartilhada com o *long-poll* (Cassandra(R) §6.2) |
| 2 | Imagem anterior no diário | **M** | custo por evento; acima de 10% do `atualizar` (2,27 µs), nasce desligada | formato do `.log` v3 → v4; réplica antiga tem de continuar |
| 3 | Dicionário de coluna (MVC que cabe) | **M** | quantas colunas reais têm domínio ≤ 255? Abaixo de 10% da largura do slot, morre. E o custo de CPU na inserção | PSCH v6 → v7 |
| 4 | Cardinalidade por índice | **P** | diferença entre o índice certo e o errado; abaixo de 2×, morre | camada SQL (entregue) |
| 5 | QUALIFY + `ROW_NUMBER`/`RANK` | **M** | o `PARTITION BY` cabe no `pivotar` que já existe? Se não, vira G | camada SQL, `varrer` por índice, `pivotar` |
| 6 | Macro parametrizada | **P** | é mais barata que a procedure de uma instrução? Se não, morre | **`docs/TRIGGERS.md`** (entregue) |
| 7 | Error table + retomada da carga | **P** | a chave externa sob índice único já torna a carga repetível? | `BULKINSERT`, `inserir_lote` (entregues) |
| 8 | Tipo `PERIOD` + dois predicados | **M** | custa menos que as duas colunas `Date`, ou o mesmo? | formato; compartilhado com MariaDB(R) |
| 9 | Direito por linha | **G** | varredura da bancada acima de 1,55 s (10%) reprova o desenho | portão único; a lição do `rownum` e a do `juntar`/`unir` |

**Sugestão de ordem, se a lista for aprovada inteira:** 4 → 6 → 1 → 7 → 2 → 3 →
5 → 8 → 9. Os dois primeiros são pequenos e podem morrer numa tarde de
medição, que é o melhor jeito de começar; o 9 é o único G e o único que mexe em
todos os caminhos de leitura.

---

## 7. A execução aguarda aprovação

**Nada desta lista começa sem o sim do Adriano, sprint a sprint.** Este
documento é uma proposta de pesquisa: ele lê a documentação oficial do
Teradata(R), mede a casa onde consegue medir, e escreve a premissa quando a
medição depende de dado que não está nesta máquina.

Três coisas que a aprovação deveria decidir junto:

1. **A ordem.** A sugerida acima é por valor ÷ custo, e não conhece a urgência
   do negócio.
2. **Os critérios de morte.** Cada sprint traz um número combinado **antes** da
   medição. Eles estão escritos para poderem ser discutidos agora — depois da
   medição, mudar o critério é escolher o resultado.
3. **O que vira só medição.** Os sprints 4, 6 e 7 podem terminar como um
   parágrafo no documento da área em vez de código, e isso é entrega completa:
   *a recusa com o número é resultado tão válido quanto o ganho*.

---

## Nota sobre os nomes

Teradata(R) e Teradata Vantage são marcas da Teradata Corporation. MySQL(R) e
InnoDB são marcas da Oracle Corporation. MariaDB(R) e Aria são marcas da
MariaDB Corporation Ab. Apache Cassandra(R) é marca da Apache Software
Foundation. Redis(R) é marca da Redis Ltd. HFSQL(R) é marca da PC SOFT.
Excel(R) é marca da Microsoft Corporation. Este
documento lê a documentação pública do Teradata(R) para entender decisões de
projeto; **nenhum código foi copiado**, e as propostas da §3 são
reimplementações de ideias documentadas, a serem escritas do zero e só com a
`std` do Rust.
