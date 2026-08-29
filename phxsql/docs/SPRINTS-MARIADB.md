# Sprints a partir do manual do MariaDB(R)

> **Este documento continua sendo a fonte, mas não é mais a lista.** As
> treze propostas daqui foram para a lista única de `docs/SPRINTS.md`,
> junto com as outras 18. Três notas da travessia: o **sprint 3 perdeu
> metade**, porque um job já chama procedimento hoje (`SPRINTS.md` §4.1);
> o **sprint 12 saiu da lista** e virou uma contagem no Profiler, como ele
> mesmo pedia (§5.2); e as **duas sobreposições que a §6 daqui afirma**
> com a análise do Cassandra(R) — papéis e índice invertido — **não
> existem naquele documento** (§2.6).

**Documento de proposta. Nada aqui foi executado, e nada será executado sem o
seu sim — sprint a sprint.** É uma lista de trabalho possível, cada item com o
escopo fechado, a fonte que o justifica e, principalmente, **a premissa que
precisa ser medida antes de o sprint começar**. Premissa que cai mata o sprint,
e matar é resultado válido: foi assim que o pedido 113 descobriu que o alvo era
outro, e que o 114 ficou fora com o número na mesa.

A régua deste documento é a da casa: **toda afirmação sobre o MariaDB(R) carrega
a URL da página que a sustenta**, e o que não tem fonte não entra. Duas coisas
que eu queria afirmar ficaram de fora por isso, e estão ditas na seção 7.

Nenhum número de desempenho daqui é novo: os da casa saem de `docs/DESEMPENHO.md`,
`docs/CONCORRENTES.md`, `docs/TRIGGERS.md`, `docs/FORMATO.md` e
`bancada/resultados.json`, com a origem escrita ao lado. Onde eu inferi, a frase
diz que inferi.

---

## 1. O que foi lido

### Do lado do MariaDB(R) — a Knowledge Base oficial

| Assunto | Página |
|---|---|
| CHECK constraints | https://mariadb.com/kb/en/constraint/ |
| Colunas geradas (virtual/persistent) | https://mariadb.com/kb/en/generated-columns/ |
| Colunas invisíveis | https://mariadb.com/kb/en/invisible-columns/ |
| `CREATE SEQUENCE` | https://mariadb.com/kb/en/create-sequence/ |
| Sequências, visão geral | https://mariadb.com/kb/en/sequence-overview/ |
| Tabelas com versionamento de sistema | https://mariadb.com/docs/server/reference/sql-structure/temporal-tables/system-versioned-tables |
| Window functions | https://mariadb.com/kb/en/window-functions-overview/ |
| CTEs (`WITH` / `RECURSIVE`) | https://mariadb.com/kb/en/with/ |
| `ALTER TABLE` instantâneo (InnoDB) | https://mariadb.com/kb/en/instant-add-column-for-innodb/ |
| `INSERT … RETURNING` | https://mariadb.com/kb/en/insertreturning/ |
| Papéis (roles) | https://mariadb.com/kb/en/roles_overview/ e https://mariadb.com/kb/en/create-role/ |
| `ANALYZE` (o EXPLAIN que executa) | https://mariadb.com/kb/en/analyze-statement/ |
| Índice de texto completo | https://mariadb.com/kb/en/full-text-indexes/ |
| Poda de partição | https://mariadb.com/kb/en/partition-pruning-and-selection/ |
| Funções JSON | https://mariadb.com/kb/en/json-functions/ |
| Event scheduler | https://mariadb.com/kb/en/events/ e https://mariadb.com/kb/en/create-event/ |
| `EXCEPT` / `INTERSECT` | https://mariadb.com/kb/en/except/ |
| Conjuntos de caracteres e colações | https://mariadb.com/kb/en/character-sets-and-collations/ |
| MariaDB(R) × MySQL(R): o diferencial | https://mariadb.com/kb/en/mariadb-vs-mysql-features/ |

### Do lado da casa

`docs/PENDENCIAS.md`, `docs/SQL.md`, `docs/CONCORRENTES.md`, `docs/DESEMPENHO.md`,
`docs/FORMATO.md`, `docs/COMPARACAO.md`, `docs/HFSQL.md`, `docs/USUARIOS.md`,
`docs/TRIGGERS.md`, `docs/JOBS.md`, `bancada/resultados.json`,
`bancada/LEIA-ME.md`, e o código de `crates/phxsql-sql/` e
`crates/phxsql-server/`.

**O que já foi feito e este documento NÃO repete:** a leitura do caminho de
inserção do InnoDB e do Aria está inteira em `docs/CONCORRENTES.md`, com as
citações `arquivo:linha` e o empilhamento medido de 2,25×. Nenhuma proposta
daqui volta a esse assunto.

---

## 2. O diferencial do MariaDB(R) sobre o MySQL(R)

É o que interessa, porque a casa já comparou com o MySQL(R) e já lhe ganha em
quatro das cinco fases da bancada. A página de comparação oficial
(https://mariadb.com/kb/en/mariadb-vs-mysql-features/) lista como exclusivo do
MariaDB(R): tabelas com versionamento de sistema, colunas `INVISIBLE`,
sequências, window functions, CTEs recursivas, CHECK constraints, papéis,
operações instantâneas de coluna, `EXCEPT`/`INTERSECT` com parênteses de
precedência, e um conjunto de motores próprios (Aria, ColumnStore, Spider,
CONNECT, MyRocks, OQGRAPH, FederatedX).

Duas dessas são exclusivas de verdade e valem para nós:

- **Sequências.** A página do `CREATE SEQUENCE`
  (https://mariadb.com/kb/en/create-sequence/) é explícita: *«MySQL does not
  implement sequences — this is MariaDB-specific functionality»*.
- **`RETURNING`.** `INSERT … RETURNING`, `DELETE … RETURNING` e
  `REPLACE … RETURNING` (https://mariadb.com/kb/en/insertreturning/).

E uma que a casa deve ler ao contrário: a página de comparação vende *thread
pool* e *parallel replication* como vantagem. Aqui não são candidatos — o
gargalo medido da escrita nunca foi concorrência (`docs/DESEMPENHO.md` §1: 95%
de CPU com 0,0 MiB lidos), e a trava única já serializa tudo.

---

## 3. O substrato que mudou esta análise

Três descobertas na leitura do código mudaram a ordem dos sprints, e valem mais
do que qualquer item da lista.

### 3.1 O avaliador de expressão já existe — e é exato

A frente de gatilhos e procedimentos entregou `crates/phxsql-sql/src/rotina.rs`,
2.991 linhas com uma árvore de expressão (`Expr`), um avaliador, e um `Numero`
de mantissa `i128` com escala — **sem `f64` em ponto nenhum**. `docs/TRIGGERS.md`
registra: `1.10 * 3` dá `3.30`, `0.1 + 0.2` dá `0.3`,
`ROUND(1500.00 * 1.1, 2)` dá `1650.00` exato.

Isso é exatamente o substrato de que **três** recursos do MariaDB(R) precisam —
`CHECK`, coluna gerada e `WHERE` com expressão — e ele está escrito, testado e
pago. Hoje `fn avaliar` e `fn expr` são privados; expô-los é uma mudança
pequena, e é a diferença entre três sprints caros e três sprints de tradução.

### 3.2 O ponto de disparo do `BEFORE` é o ponto exato do `CHECK`

`docs/TRIGGERS.md`: o `BEFORE` roda **com a trava de dados na mão, entre a
conversão da linha e a gravação**, pode alterar `NEW` e pode cancelar por
`SIGNAL`. É, literalmente, a descrição do que a página do CHECK do MariaDB(R)
promete (https://mariadb.com/kb/en/constraint/): *«Before a row is inserted or
updated, all constraints are evaluated in the order they are defined»*.

O lugar está construído. Falta o verbo declarativo por cima dele.

### 3.3 O filtro por comparação existe — mas só na memória

`op_selecionar_memoria` (`servidor.rs`) aceita `onde` como lista de
`{coluna, op, valor}`, com `Operador` e `Filtro` já escritos. O `varrer` em
disco **não filtra** — e é por isso que `docs/SQL.md` recusa um `WHERE` sem
índice pelo nome da cláusula, em vez de devolver a tabela inteira como se fosse
a resposta.

Ou seja: a peça existe, e está do lado errado. Isso barateia o sprint 1 e
**cria uma dependência**: sem filtro no disco não há o que podar, e o sprint da
poda de partição (8) depende do 1.

### 3.4 E uma quarta, que é um alerta

`replicacao.imagem_da_linha` nasce **`false`** num servidor isolado
(`config.rs`: o padrão segue o papel, e só `source`/`multi` exigem imagem). Sem
ela o `.log` diz *que* o rowid 42 mudou e não diz *para quê*
(`docs/FORMATO.md` §4). **Num servidor isolado, hoje, não está sendo guardado o
histórico que uma tabela versionada precisaria** — e histórico que não foi
gravado não se recupera depois. É a premissa que manda no sprint 7.

---

## 4. Os sprints

Ordem por valor medível dividido pelo custo. Cada um cabe numa rodada.

---

### Sprint 1 — O `WHERE` que filtra de verdade

**Escopo fechado.** Levar o `onde` que o `SelectMemory` já tem
(`{coluna, op, valor}`) para o `varrer` em disco, e ligá-lo ao tradutor de
`SELECT`: um `WHERE coluna = valor` sem índice deixa de ser recusado e passa a
ser uma varredura filtrada, com a nota dizendo que é varredura. Em seguida,
`AND`/`OR` sobre esses filtros.

**Por que agora.** É a chave de três portas. `docs/SQL.md` §3 lista o que a
camada SQL não tem e chama expressão e planejador de *«o trabalho de verdade»*;
e `docs/TRIGGERS.md` §8 recusa `CREATE FUNCTION` com o motivo exato *«devolveria
valor dentro de expressão SQL, e a camada SELECT não avalia expressão»*. A
mesma ausência aparece nas duas frentes. E metade do trabalho está escrito: o
`Filtro`/`Operador` do lado da memória, o `Expr`/`avaliar` do lado das rotinas.

**Premissa a medir primeiro.** *Uma varredura filtrada em disco é rápida o
bastante para ser oferecida, ou vai virar a operação lenta que o
`SelectMemory` recusa fazer calada?* O número de referência é da casa: a
varredura de faixa lê 1.250.000 linhas em 1,41 s (`bancada/resultados.json`),
**8× o MySQL(R)** — o formato de slot fixo é bom nisso. Se o filtro por linha
custar pouco sobre esses 1,41 s, o item vive. **Se custar caro, o sprint morre e
a recusa atual continua sendo a resposta certa.**

**Dependências.** Nenhuma frente em andamento. Depende de expor `avaliar`/`expr`
do `rotina.rs` (mudança pequena, mas é acoplamento entre crates e precisa de
desenho).

**O que NÃO entra.** Planejador (escolher entre dois índices candidatos),
subconsulta, `GROUP BY` geral, `LIKE`, `IN`, `BETWEEN`. E não entra prometer
`WHERE` sobre coluna sem índice em tabela grande sem dizer que é varredura: a
nota que o `sql` já devolve é obrigatória aqui.

---

### Sprint 2 — `EXPLAIN` e `ANALYZE` sobre o que o tradutor já decide

**Escopo fechado.** `EXPLAIN <select>` devolvendo o que a op `sql` já produz por
dentro — a operação escolhida (`varrer` ou `buscar`), o índice, as `notas` do
tradutor e a estimativa de linhas que sai do cabeçalho em O(1). E
`ANALYZE <select>`, que executa e devolve **linhas lidas de verdade** ao lado da
estimativa.

**Por que agora.** A distinção é a do MariaDB(R)
(https://mariadb.com/kb/en/analyze-statement/): o `ANALYZE` *«invoca o
otimizador, executa a instrução e produz a saída do EXPLAIN»*, com `r_rows` e
`r_filtered` — o real ao lado do estimado. E a casa já tem as duas metades:
`docs/COMPARACAO.md` deixou o `EXPLAIN` de fora dizendo *«faz sentido depois da
camada SQL»* — e a camada SQL **existe agora** (op `sql`, `docs/SQL.md` §5),
devolvendo `op`, `notas` e `colunas`. O Profiler e o `estatisticas` com
percentis já dão o lado do real.

**Premissa a medir primeiro.** Nenhuma de desempenho — é a de **valor**:
*o `EXPLAIN` daqui diz algo que as `notas` já não digam?* Se a resposta for
«mostra a mesma coisa com outro nome», o sprint vira só o verbo `EXPLAIN` como
sinônimo, que é meia hora e não uma rodada. **Essa é uma premissa que encolhe o
sprint em vez de matá-lo, e é para isso que ela existe.**

**Dependências.** Nenhuma. Ganha corpo depois do sprint 1 (aí há escolha real a
explicar).

**O que NÃO entra.** Estatísticas persistidas para o planejador — `ANALYZE
TABLE` no sentido do MariaDB(R). `docs/COMPARACAO.md` já recusou com o motivo
certo: sem planejador, estatística é arquivo para manter atualizado sem
ninguém ler.

---

### Sprint 3 — O que falta ao agendador para cobrir o event scheduler

**Escopo fechado.** Duas coisas, e só elas: (a) um job cujo corpo é
`CALL procedimento(...)`, em vez de apenas uma operação nomeada do protocolo;
(b) o equivalente ao `DISABLE ON SLAVE` — um job que **não roda no servidor que
é réplica**.

**Por que agora.** O terreno do event scheduler já está coberto pela frente de
jobs (`docs/JOBS.md`: estado por job, aviso por e-mail, silêncio por período,
prova por soquete). O que sobra é exatamente o que a página do `CREATE EVENT`
descreve e a casa não tem. E a segunda metade ficou **urgente** porque os quatro
modos de replicação entraram: a documentação do MariaDB(R) é literal —
*«DISABLE ON SLAVE indicates that an event was created on a master and has been
replicated to the slave, which is prevented from executing the event»*
(https://mariadb.com/kb/en/create-event/). Sem isso, um job de escrita ligado
nos dois lados de uma replicação roda duas vezes; num par multi-master com
«mais recente vence», roda dos dois lados e cada um sobrescreve o outro.

**Premissa a medir primeiro.** *Um job de escrita, hoje, roda mesmo nos dois
lados?* Sobe-se um par origem/réplica (portas 5364/5764 são as minhas nesta
análise) com o mesmo `jobs.json`, liga-se um job que insere, e conta-se. **Se a
réplica já recusa a escrita por outro caminho, metade do sprint some** — e é
melhor descobrir isso com dois servidores do que com um desenho.

**Dependências.** (a) depende do interpretador entregue pela frente de triggers
— e ele já está entregue, com `CALL` e `OUT`. (b) depende de o job saber o papel
do servidor, que o `config.json` já traz.

**O que NÃO entra.** `ON COMPLETION PRESERVE`, `STARTS`/`ENDS`, e a sintaxe
`CREATE EVENT` em SQL. O cadastro de jobs da casa já resolve o agendamento com
tela e histórico; trocar isso por SQL seria refazer o que funciona.

---

### Sprint 4 — `CHECK` declarativo, no ponto onde o `BEFORE` já roda

**Escopo fechado.** `CHECK (expressão)` na criação da tabela e em
`ALTER TABLE ADD CONSTRAINT`, guardado no esquema, avaliado antes de gravar,
recusando com o nome da restrição. Nome automático quando não vier escrito, e
`DROP CONSTRAINT` para tirar.

**Por que agora.** A fonte descreve o comportamento que já temos montado
(https://mariadb.com/kb/en/constraint/): avaliação antes de inserir ou
atualizar, na ordem em que foram definidas, com erro nomeado —
`ERROR 4022 (23000): CONSTRAINT 'nome' failed`. Aqui, o §3.2 acima mostra que o
lugar existe e o §3.1 que o avaliador existe. **É tradução, não motor** — que é
a mesma razão pela qual a camada SQL coube (`docs/SQL.md` §1).

E há um ganho que o gatilho não dá: uma regra declarativa **aparece na
estrutura**. Hoje, para saber que `preco > 0`, é preciso ler o corpo de um
gatilho; com `CHECK`, o Diagrama ER, o `siscolunas` e o driver ODBC leem a
regra como dado.

**Premissa a medir primeiro.** *Tabela sem `CHECK` continua custando
exatamente o que custa hoje?* É a regra da casa — instrumentação desligada
custa zero, e o portão vem antes do trabalho. O molde já existe e está medido:
o `AtomicBool` dos gatilhos, cujo custo `docs/TRIGGERS.md` §4 não conseguiu
separar do ruído (diferença de +0,99 µs contra espalhamento de 24,16 µs no
mesmo cenário). **A premissa aqui não é se dá para medir; é reproduzir aquele
método** — rodadas intercaladas, servidor limpo por cenário, e a régua do ruído
publicada ao lado.

Segunda premissa, de garantia: *o que acontece com as linhas que já estão
gravadas e violam a regra nova?* A resposta precisa ser escrita antes de o
código existir.

**Dependências.** O avaliador exposto (sprint 1 ou uma extração própria). Muda
o formato: esquema `PSCH` v7 — e **mudança de formato entra cedo**, enquanto
não há dado em produção.

**O que NÃO entra.** O `check_constraint_checks = OFF` do MariaDB(R), que
desliga a conferência globalmente para carregar dado. Aqui isso seria uma
chave que apaga em silêncio uma garantia que o dono da tabela pediu — e a casa
já tem o lugar certo para carga rápida, que é o `BULKINSERT`, e ele **não**
desliga garantia nenhuma. Também não entram subconsulta e função não
determinística, que a própria fonte proíbe.

---

### Sprint 5 — Colunas geradas, no caminho de escrita

**Escopo fechado.** Coluna `GENERATED ALWAYS AS (expr) PERSISTENT`: o motor
calcula na inclusão e na alteração, grava como coluna comum, e recusa a
gravação direta nela.

**Por que agora.** A fonte separa os dois modos com clareza
(https://mariadb.com/kb/en/generated-columns/): `VIRTUAL` calcula na leitura,
`PERSISTENT`/`STORED` calcula na escrita e **é o que aceita índice e chave
estrangeira**. Aqui o `PERSISTENT` cai como uma luva: o motor já preenche
colunas sozinho (`rownum` e `softdeleted` são exatamente isso,
`docs/FORMATO.md` §1), o slot é de largura fixa, e o avaliador está pronto.

**Premissa a medir primeiro.** *Quanto custa por linha avaliar uma expressão
simples no caminho de escrita?* A referência é dura: a inserção inteira custa
**7,5 µs por linha** hoje (`docs/DESEMPENHO.md` §4.8), e `.reg`+`.log` já são
60,8% disso. Uma expressão que custe 1 µs é 13% da inserção. **Se o avaliador
for caro por chamada, a coluna gerada persistente fica cara exatamente nas
tabelas grandes, que são as que a teriam.** Mede-se com o `onde-doi`, e **antes
de medir: `cargo build --release --examples -p phxsql-store`** — a bancada já
perdeu uma rodada inteira de ganhos por medir com binário velho
(`docs/DESEMPENHO.md` §4.8).

**Dependências.** As mesmas do sprint 4, e o mesmo `PSCH` v7 — os dois deveriam
entrar na mesma mudança de formato, não em duas.

**O que NÃO entra.** `VIRTUAL`. Calcular na leitura significa mexer em todo
caminho que devolve linha — e a casa já aprendeu três vezes que **coluna nova
no fim de uma lista quebra quem usa `find(...)` onde devia usar `filter(...)`**
(`CLAUDE.md`). Uma coluna que não existe no slot é a versão pior desse mesmo
problema. Também não entra coluna gerada como chave primária, que a fonte
proíbe.

---

### Sprint 6 — `EXCEPT` e `INTERSECT` sobre o `unir` que já existe

**Escopo fechado.** Duas operações novas no mesmo molde do `unir`, com as
variantes `DISTINCT` (padrão) e `ALL`, e o SQL correspondente no tradutor.

**Por que agora.** A semântica está na fonte
(https://mariadb.com/kb/en/except/): `EXCEPT` é a diferença de conjuntos,
`EXCEPT ALL` preserva duplicatas, `INTERSECT` tem precedência maior que
`UNION`/`EXCEPT`. E aqui a máquina está pronta: o pedido 91 entregou `UNION` e
`UNION ALL` com **chave composta e nulo que não casa com nulo, como no SQL**, e
as sete figuras de junção incluem `so_esquerda` e `so_dos_lados`, que são
diferença de conjuntos por outro nome.

**Premissa a medir primeiro.** *A comparação de linhas do `unir` serve como
está para diferença e interseção, ou ela depende de alguma decisão que só valia
para união?* Isso se responde lendo o código e escrevendo o teste do caso do
nulo — não precisa de bancada.

**Dependências.** Nenhuma.

**O que NÃO entra.** O `MINUS` do modo Oracle, e os parênteses de precedência
entre operações de conjunto — que a fonte cita como diferencial do MariaDB(R),
mas que só fazem sentido quando houver mais de duas na mesma consulta.

---

### Sprint 7 — `AS OF` de **uma linha**, sobre o diário que já existe

**Escopo fechado.** Deliberadamente pequeno: `SELECT … FOR SYSTEM_TIME AS OF
<instante> WHERE rowid = N` — a linha como ela estava, reconstruída do `.log`.
Mais a lista de versões daquela linha, com quem mexeu e quando.

**Por que agora.** A fonte
(https://mariadb.com/docs/server/reference/sql-structure/temporal-tables/system-versioned-tables)
descreve `AS OF`, `BETWEEN … AND`, `FROM … TO` e `ALL`, com `row_start`/`row_end`
e o aviso de que o crescimento *«pode afetar significativamente o tamanho da
tabela»*. Aqui, três quartos da matéria-prima já estão gravados:

- o `.log` guarda **a imagem da linha** — payload cru, sem reencodar, com o
  conteúdo dos anexos junto (`docs/FORMATO.md` §4), com carimbo em
  milissegundos, usuário e **a versão do registro depois da operação** — que é,
  em tudo menos no nome, um `row_start`;
- o `.trash` guarda a linha inteira **antes de sumir**, com o conteúdo dos
  externos, e é sincronizado antes de o slot ser liberado;
- o `.reason` guarda **por que** ela saiu, e sobrevive à linha.

E os volumes do diário **nunca são apagados** — não há `remove_file` nem rotação
em `log.rs`. O histórico, uma vez gravado, fica.

**Premissa a medir primeiro — e ela pode matar o sprint.** *O histórico está
sendo gravado?* Medido no código, não suposto: `replicacao.imagem_da_linha`
nasce **`false`** em servidor isolado (§3.4). Num servidor que não replica, o
`.log` de hoje **não tem** as imagens, e nenhum `AS OF` as inventa. Ligar a
chave tem preço já medido em `docs/FORMATO.md`: **21.740 → 19.531 linhas/s** e
**44 → 223 bytes por evento**. Então a pergunta que decide é: *o Adriano aceita
~10% da vazão de escrita e 5× o tamanho do diário para ter história?* Se a
resposta for não, o sprint morre aqui — e essa é a decisão dele, não minha.

Segunda premissa, se a primeira passar: *quanto custa reconstruir uma linha?*
O `.log` é sequencial e sem índice, então chegar ao evento N é caminhar pelos
anteriores (`docs/FORMATO.md`, «o que a largura variável custa»). Para **uma
linha** isso é caminhar o diário inteiro filtrando por rowid. Mede-se num
diário real antes de prometer o verbo.

**Dependências.** Nenhuma frente em andamento. A tabela inteira `AS OF` — não só
uma linha — é outro sprint, maior, e depende deste ter passado nas duas
premissas.

**O que NÃO entra.** `AS OF` de tabela inteira, `PARTITION BY SYSTEM_TIME`,
versionamento preciso por transação (**não há transação aqui**, e a fonte
descreve esse modo em cima de IDs de transação), `DELETE HISTORY`, e
`WITHOUT SYSTEM VERSIONING` por coluna.

---

### Sprint 8 — Poda de volumes na varredura

**Escopo fechado.** Uma varredura com filtro sobre a coluna que corta a
partição pula os volumes que não podem conter resposta, e diz na resposta
quantos pulou.

**Por que agora.** É a poda de partição do MariaDB(R)
(https://mariadb.com/kb/en/partition-pruning-and-selection/): o otimizador
descobre pelas condições do `WHERE` quais partições interessam e *«as outras não
serão lidas»*. Aqui o dado necessário **já está gravado**: na partição por
período, cada volume grava a própria fronteira no cabeçalho (pedido 76,
`docs/FORMATO.md` §7); na alfanumérica, o balde é a primeira letra. Ninguém
usa isso para pular arquivo.

E há um alerta que a mesma fonte entrega de graça, e que só agora nos atinge:
com gatilhos `BEFORE INSERT`/`BEFORE UPDATE`, o MariaDB(R) **desiste da poda**,
porque não sabe se o gatilho vai mudar a coluna que decide a partição. A frente
de gatilhos acabou de entrar. **É o mesmo buraco, e ele existe aqui a partir de
agora.**

**Premissa a medir primeiro.** *Quanto se ganha?* Depende inteiramente de
quantos volumes a pergunta típica dispensa — numa tabela de 12 volumes mensais,
uma consulta de um mês lê 1 em vez de 12, e isso é 12× no papel. **Inferido, não
medido**, e é exatamente o tipo de conta que a casa já viu dar errado: mede-se
com uma tabela particionada de verdade antes de escrever qualquer coisa.

**Dependências.** **Depende do sprint 1** — sem filtro no `varrer` de disco não
há condição pela qual podar. Esta dependência é a razão de o sprint 1 estar em
primeiro.

**O que NÃO entra.** A seleção explícita de partição (`SELECT … PARTITION (p3)`),
que é útil e é o passo seguinte natural, mas só depois de a poda automática
existir e estar medida.

---

### Sprint 9 — Papéis no modelo de direitos

**Escopo fechado.** `papeis` no `config.json`: um nome, um conjunto de direitos
por base e por tabela, e a lista de papéis de cada usuário. O direito efetivo
é o do usuário mais o dos papéis dele.

**Por que agora.** A fonte (https://mariadb.com/kb/en/roles_overview/) descreve
o que a casa não tem: papel concedido a papel, com os direitos aninhados
*«imediatamente disponíveis»*, e `CREATE ROLE … WITH ADMIN`
(https://mariadb.com/kb/en/create-role/). Hoje o direito é escrito usuário a
usuário (`docs/USUARIOS.md`) — e com direito por tabela (pedido 124), a
configuração de dez pessoas com o mesmo cargo é a mesma regra copiada dez
vezes. Copiar regra de permissão é como se esquece de tirar uma.

**Premissa a medir primeiro.** Não é de desempenho — é a premissa que a casa já
escreveu no `CLAUDE.md`: *o teste que mais importa numa regra de permissão nova
é o do comportamento **velho***. Antes de qualquer código, o teste
`sem_papel_nada_muda` — um `config.json` sem a seção de papéis produz
**exatamente** os mesmos direitos de hoje, usuário por usuário. Regra que muda
o significado da configuração que já existe tira o direito de alguém sem
ninguém ter pedido.

**Dependências.** Nenhuma. Mas há um cuidado documentado: o portão é **um só**,
e as operações que não têm o campo `"tabela"` (`juntar`, `unir`,
`dados_pessoais`) já precisaram de conferência própria. Papel não pode virar um
segundo caminho até a decisão de permissão.

**O que NÃO entra.** `SET ROLE` por sessão — ativar e desativar papel no meio da
conexão. É a parte da fonte que existe para separar poderes de um DBA humano num
console; aqui multiplicaria os estados de uma sessão sem resolver o problema que
motivou o sprint, que é a regra copiada dez vezes.

---

### Sprint 10 — `ALTER TABLE ADD COLUMN`, preservando o rowid

**Escopo fechado.** Acrescentar uma coluna a uma tabela que já tem dado,
reescrevendo o `.reg` volume a volume, com **o rowid de cada linha preservado**,
e os índices reconstruídos em lote.

**Por que agora.** É o que trava o editor de modelo do Diagrama ER (pedido 127),
e a tela hoje diz isso com honestidade em vez de fingir. A fonte do MariaDB(R)
(https://mariadb.com/kb/en/instant-add-column-for-innodb/) mostra o truque
deles — *«uma operação O(1) para inserir um registro oculto especial, e uma
atualização do dicionário de dados»* — e **ele não serve aqui**: o truque
funciona porque a linha do InnoDB é de largura variável e carrega a contagem de
colunas. O nosso slot é de largura fixa, calculada do esquema; uma coluna a
mais muda o `slot_size`, e o endereço de toda linha (`offset = data_offset +
(rowid−1) × slot_size`) muda junto. **Não existe `ALGORITHM=INSTANT` para este
formato, e é melhor escrever isso do que descobrir na metade.**

O que existe é a reescrita, e ela é barata pelo lado que costuma doer: a
reconstrução do índice em lote custa **0,31 s por milhão de chaves**
(`docs/DESEMPENHO.md` §4.3, 23× a 25× o `reindexar` antigo).

**Premissa a medir primeiro.** *Quanto custa reescrever o `.reg` de uma tabela
grande?* É leitura e escrita sequenciais, que é o que este formato faz de
melhor — a varredura lê 1.250.000 linhas em 1,41 s
(`bancada/resultados.json`). **Inferido, não medido:** uma reescrita de 10
milhões de linhas fica na casa dos minutos. Mede-se antes, porque é o número que
decide se a operação pode acontecer com o banco no ar ou se precisa de janela.

**A armadilha específica desta casa, e ela é séria.** As colunas de sistema
`softdeleted` e `rownum` entram **no fim da lista, para não deslocar as colunas
do usuário** (pedidos 97 e 103). Uma coluna nova do usuário tem de entrar
**antes** delas — e isso desloca o índice das colunas de sistema. A casa já
gravou três vezes a mesma lição: *coluna de sistema nova quebra quem filtra pela
primeira*, e o pior defeito daquela família quebrou **todo salvar e todo
incluir** pela tela. Este sprint mexe exatamente ali.

**Dependências.** Nenhuma frente em andamento; o editor visual do Diagrama ER
(entregue) é quem consome.

**O que NÃO entra.** Excluir coluna, trocar tipo, renomear, e reordenar. E não
entra fazer isso com a tabela aberta por outra sessão — o modo exclusivo não
existe ainda (`PENDENCIAS.md`, planejado 13).

---

### Sprint 11 — Sequência como objeto próprio

**Escopo fechado.** Uma sequência nomeada no banco, independente de tabela, com
`INCREMENT`, `MINVALUE`, `MAXVALUE`, `START` e `CYCLE`, e o valor obtido por
uma operação do protocolo — utilizável por **várias** tabelas.

**Por que agora.** A fonte (https://mariadb.com/kb/en/create-sequence/) mostra o
que o contador de tabela não dá: um número compartilhado entre tabelas, com
passo e limites próprios. Aqui, o pedido 81 entregou `sequencias` e
`ajustar_sequencia`, mas o contador vive **no cabeçalho de cada `.reg`**, e
`docs/FORMATO.md` §12 registra o limite: **uma sequência por tabela**. O caso
real que isso não atende é a numeração única de documento que atravessa duas
tabelas.

**Premissa a medir primeiro.** *Quanto custa um `NEXTVAL` durável?* O contador
de hoje é de graça: já está no cabeçalho que a inserção grava de qualquer jeito.
Uma sequência independente precisa do próprio arquivo e da própria
sincronização — e a fonte é explícita sobre como o MariaDB(R) escapa disso: um
`CACHE` de 1.000 valores por padrão, com o preço escrito, *«FLUSH TABLES,
desligar o servidor etc. descartam os valores em cache»*, deixando buracos na
numeração. **A premissa é a pergunta ao Adriano: buraco na numeração é
aceitável?** Para nota fiscal, tipicamente não — e aí a sequência custa um
`fsync` por número, que a medição precisa mostrar antes de alguém a usar em
laço.

**Dependências.** Nenhuma.

**O que NÃO entra.** `NEXT VALUE FOR` dentro de expressão SQL (depende do
sprint 1), e o modo Oracle `seq.nextval`.

---

### Sprint 12 — O degrau seguinte do interpretador

**Escopo fechado.** As recusas mais pedidas da lista que `docs/TRIGGERS.md` §8
já publica, em ordem de custo: `CASE`, `LOOP`/`REPEAT`, `HANDLER` para erro, e
`CREATE FUNCTION`.

**Por que agora.** Não é proposta nova — é a lista que a própria frente deixou
escrita, com **17 recusas nomeadas e testadas**: `CASE`, `LOOP`, cursor,
`HANDLER`, `UPDATE`/`DELETE` no corpo, `DEFINER`, `FOR EACH STATEMENT`,
`CREATE FUNCTION`, as características (`DETERMINISTIC`…), `CALL` aninhado,
`BEGIN`/`COMMIT`, `FOLLOWS`/`PRECEDES`, variável de sessão. Quem cola um corpo
do MySQL(R)/MariaDB(R) descobre o que trocar pelo nome, o que é a metade certa
do trabalho.

**Premissa a medir primeiro.** *Quais dessas faltam de verdade?* `CASE` e
`LOOP` foram recusados com um motivo honesto — *«`IF/ELSEIF` e `WHILE` cobrem;
menos superfície»* — e continuam cobertos. **Este sprint só existe se alguém
esbarrar nas recusas**, e o jeito de saber é contar: o Profiler já vê o que
chega pela porta antes de virar dado. Contar as recusas por nome durante um
tempo de uso real é mais barato que implementar as quatro.

**Dependências.** `CREATE FUNCTION` **depende do sprint 1** — a recusa dela é
literalmente «a camada SELECT não avalia expressão». `HANDLER` e cursor
dependem de decidir o que fazer sem transação: um `HANDLER` que engole erro no
meio de um corpo que já gravou deixa meia escrita, e isso precisa ser dito.

**O que NÃO entra.** `UPDATE`/`DELETE` no corpo. A frente já mediu e recusou
contra o motor: o motor escreve **por rowid**, e traduzir um `UPDATE … WHERE`
exigiria o planejador. Aceitar só `WHERE chave = valor` criaria um verbo que
funciona ou não conforme o índice — **e um verbo que às vezes funciona é pior
que um verbo que falta**. Fica fora até o planejador existir.

---

### Sprint 13 — Índice de texto completo (`.fts`)

**Escopo fechado.** Um arquivo novo, `.fts`, índice invertido por termo, com o
mesmo desenho de página do `.ndx`; busca por palavra e por conjunto de palavras
sobre colunas `Str` e `Memo`.

**Por que agora.** É o item que `docs/HFSQL.md` §3.2 já elegeu como o segundo
maior buraco, e a fonte do MariaDB(R)
(https://mariadb.com/kb/en/full-text-indexes/) descreve a superfície que se
espera: `MATCH … AGAINST` em modo natural, booleano e com expansão de consulta,
`ft_min_word_length`, e listas de palavras vazias.

**Premissa a medir primeiro.** *Quanto custa hoje procurar uma palavra num
`.memo`?* Hoje é varredura — mas varredura aqui é rápida (8× o MySQL(R) na faixa,
`docs/DESEMPENHO.md` §6). **O número que justifica ou mata este sprint é o
tempo de achar uma palavra em uma tabela de um milhão de linhas com memo
grande**, e ele não existe: nunca foi medido. Sem ele, «índice de texto
completo» é desejo, não plano. Este é o sprint cuja premissa tem mais chance de
mudar a decisão.

**Dependências.** Nenhuma. É o maior da lista e o mais isolado.

**O que NÃO entra.** Relevância por ranking, expansão de consulta, e ordenação
linguística (que é outro assunto — ver seção 7).

---

## 5. O que descartei de propósito

| Candidato | Por que fica fora |
|---|---|
| **Window functions e CTEs** | A fonte é boa (https://mariadb.com/kb/en/window-functions-overview/, https://mariadb.com/kb/en/with/) e o recurso é real. Mas a camada SQL daqui ainda recusa `AND`, `LIKE`, `IN`, `BETWEEN`, `IS NULL`, `DISTINCT`, `GROUP BY`, `JOIN` e todo agregado que não seja `COUNT(*)` (`docs/SQL.md`). Propor `ROW_NUMBER() OVER (PARTITION BY …)` antes de o `AND` funcionar é construir o telhado antes da parede. **Volta quando o sprint 1 estiver feito e o `GROUP BY` existir.** |
| **Replicação** | Saiu da lista: os quatro modos entraram, inclusive multi-master com «mais recente vence». |
| **Gatilhos e procedimentos** | Entregues. Este documento propõe só o degrau seguinte (sprint 12). |
| **Event scheduler como recurso** | Coberto pela frente de jobs. Sobrou o sprint 3, que é o recorte do que falta. |
| **`ALGORITHM=INSTANT` literal** | Descartado **com motivo técnico**, não por prioridade: o truque do InnoDB depende de linha de largura variável, e o nosso slot é fixo (sprint 10). Copiá-lo seria copiar o nome sem o mecanismo. |
| **Colunas `INVISIBLE`** | A fonte (https://mariadb.com/kb/en/invisible-columns/) descreve exatamente o remédio para o problema que a casa teve: `rownum` e `softdeleted` aparecendo onde não deviam. Mas torná-las invisíveis **agora** mudaria a resposta de quem já lê essas colunas hoje — e guarda nova entra pedida, não imposta. Cabe como opção na criação da tabela, não como mudança de comportamento. |
| **`JSON` como tipo de coluna** | O projeto tem parser JSON próprio, então é tentador. Mas as funções úteis (`JSON_VALUE`, `JSON_TABLE`; https://mariadb.com/kb/en/json-functions/) vivem **dentro de expressão SQL**, que não existe. Depois do sprint 1, e não antes. |
| **Versionamento preciso por transação** | A fonte o descreve sobre IDs de transação. **Não há transação no PhxSql** — prometer o verbo sem o mecanismo seria pior que não tê-lo. |
| **`check_constraint_checks = OFF`** | Chave global que apaga em silêncio uma garantia pedida pelo dono da tabela. O `BULKINSERT` já dá carga rápida sem desligar garantia. |
| **`OPTIMIZE TABLE` / compactação** | Já decidido em `docs/COMPARACAO.md`: compactar renumera rowid, e rowid é endereço. Depende de decisão do Adriano sobre a ordem de digitação, não de análise nova. |
| **Motores do MariaDB(R) (ColumnStore, Spider, CONNECT)** | Fora da regra de zero dependências, e o equivalente útil — o motor LSM ao lado do atual — já está analisado e recusado como ajuste em `docs/DESEMPENHO.md` §5: é projeto próprio, não sprint. |

---

## 6. Candidatos compartilhados com as análises irmãs

Anotados, não resolvidos — a consolidação é da integração.

| Candidato | Compartilhado com | Observação |
|---|---|---|
| **Expurgo por idade / TTL** (o `DELETE HISTORY … BEFORE SYSTEM_TIME` da fonte) | análise do Cassandra(R) e do Redis(R) | Os três chegam ao mesmo lugar por caminhos diferentes: TTL por linha, expiração de chave, e poda de histórico. Aqui tocaria `.log`, `.trash` e `.reason` — e hoje **nenhum volume desses é apagado**. Vale desenhar uma vez, não três. |
| **Papéis (sprint 9)** | análise do Cassandra(R) | Lá também há papéis concedidos a papéis. Se as duas análises propuserem, é um sprint só. |
| **Sequência / contador (sprint 11)** | análise do Redis(R) | O `INCR` do Redis(R) é o mesmo problema com outro nome, e a pergunta do cache com buraco na numeração é idêntica. |
| **Índice invertido (sprint 13)** | possivelmente a análise do Cassandra(R) | Índice secundário lá, texto completo aqui; o mecanismo de página é o mesmo `.ndx`. |

---

## 7. O que eu queria afirmar e não afirmei

A régua da casa é que sem fonte não entra, e três coisas esbarraram nela:

1. **Em que versão entraram os papéis e o `CREATE SEQUENCE`.** As páginas
   (https://mariadb.com/kb/en/create-role/, https://mariadb.com/kb/en/sequence-overview/)
   não trazem a nota de versão. Eu sei de cabeça, e é exatamente por isso que
   não escrevi: **número citado é número que não se mede.** O que está sustentado
   é o que a página diz — que o MySQL(R) não implementa sequências.
2. **Ordenação linguística.** A página de colações
   (https://mariadb.com/kb/en/character-sets-and-collations/) que consegui ler é
   um índice: confirma que colação define armazenamento e ordenação, e cita
   `utf8mb3`/`utf8mb4` e `NO PAD`, mas não as colações acentuais específicas.
   O buraco da casa é real e está descrito em `docs/HFSQL.md` §3.4 — «Álvaro»
   não cai junto de «Alvaro» —, e a tabela de dobra de acento da partição
   alfanumérica já é o começo do caminho. **Fica como candidato, não como
   sprint, até haver fonte à altura.**
3. **Se o job realmente roda nos dois lados de uma replicação.** É a premissa do
   sprint 3, e está escrita como premissa justamente porque eu não a medi.

---

## 8. Tabela-resumo

| # | Sprint | Tamanho | Premissa a medir primeiro | Dependência |
|---:|---|:---:|---|---|
| 1 | O `WHERE` que filtra de verdade | M | A varredura filtrada é rápida o bastante para ser oferecida | expor o avaliador do `rotina.rs` |
| 2 | `EXPLAIN` e `ANALYZE` | P | O `EXPLAIN` diz algo que as `notas` já não digam | — (rende mais depois do 1) |
| 3 | O que falta ao agendador | P | Um job de escrita roda mesmo nos dois lados da replicação | interpretador (entregue) |
| 4 | `CHECK` declarativo | M | Tabela sem `CHECK` custa exatamente o que custa hoje | avaliador; `PSCH` v7 |
| 5 | Colunas geradas `PERSISTENT` | M | Custo por linha de avaliar a expressão na escrita | avaliador; mesmo `PSCH` v7 do 4 |
| 6 | `EXCEPT` e `INTERSECT` | P | A comparação de linhas do `unir` serve como está | — |
| 7 | `AS OF` de uma linha | M | A imagem da linha nasce **desligada**: 10% da vazão e 5× o diário é preço aceitável? | — |
| 8 | Poda de volumes | M | Quantos volumes a pergunta típica dispensa | **sprint 1** |
| 9 | Papéis no modelo de direitos | M | `sem_papel_nada_muda`: config sem papéis produz os direitos de hoje | — |
| 10 | `ALTER TABLE ADD COLUMN` | G | Custo de reescrever o `.reg` de uma tabela grande | — |
| 11 | Sequência como objeto próprio | M | Buraco na numeração é aceitável? Senão, custo do `fsync` por número | — |
| 12 | O degrau seguinte do interpretador | M | Quais recusas alguém esbarra de verdade (contar pelo Profiler) | `CREATE FUNCTION` depende do 1 |
| 13 | Índice de texto completo | G | Quanto custa **hoje** achar uma palavra num `.memo` | — |

---

## 9. A execução aguarda aprovação

**Nada desta lista começa sem o seu sim, e o sim é sprint a sprint** — não em
bloco. Cada um foi escrito para caber numa rodada e para poder ser recusado
sozinho, sem derrubar os outros.

E há uma ordem que eu defenderia se você perguntasse: **1, 3 e 2 primeiro.** O 1
porque destrava quatro dos outros e apaga uma lista de recusas que a camada SQL
carrega desde que nasceu; o 3 porque é pequeno e porque a replicação em quatro
modos acabou de tornar real um problema que antes não existia; o 2 porque é
quase de graça e porque dá ao Adriano a ferramenta de ver o que o motor decidiu
— que é o que torna as decisões seguintes discutíveis com número em vez de
opinião.

Os sprints 4 e 5 deveriam entrar **juntos ou em sequência imediata**: os dois
mudam o esquema para `PSCH` v7, e mudança de formato entra cedo — enquanto não
há dado em produção, mudar é barato; depois vira migração.

---

## Nota sobre os nomes

MariaDB(R) e Aria são marcas da MariaDB Corporation Ab. MySQL(R) e InnoDB são
marcas da Oracle Corporation. HFSQL(R) é marca da PC SOFT. Cassandra(R) e
Redis(R) são marcas dos seus respectivos donos. As páginas da Knowledge Base do
MariaDB(R) foram lidas como documentação pública para entender decisões de
projeto; nenhum código foi copiado, e tudo o que este documento propõe seria
escrito do zero, só com a `std` do Rust.
