# Pesquisa: como os outros separam leitor de leitor, medido contra o NOSSO gargalo

Este documento é do papel **J (pesquisador)**, e ele obedece à regra que dá
sentido ao papel: *receita de fora se mede contra o nosso gargalo antes de virar
plano*. Nada aqui é recomendação por prestígio de motor. Cada receita entra com
**URL e citação da documentação oficial**, e sai com **o número que a mataria ou
a confirmaria aqui** — e quando eu não sei como medi-la contra o nosso caso, o
documento diz isso em vez de recomendá-la.

Ele **não** repete a `docs/CONCORRENCIA.md` (que perguntou «o que pôr no
lugar?») nem a §14 do `docs/DESEMPENHO.md` (que provou que a trava custa). Ele
pergunta uma coisa só: **o que o SQLite, o InnoDB e o PostgreSQL(R) fazem no par
leitor-com-leitor, e o que disso sobrevive ao nosso formato.**

---

## 0. O gargalo, relido e não remedido

Cinco fatos desta casa carregam o documento inteiro. Nenhum deles foi medido
aqui: todos vêm de bateria já registrada, e estão citados para que a conta de
cada receita se confira sem sair deste arquivo.

| fato | número | onde foi medido |
|---|---|---|
| a trava de dados é **ponto único** mas **global** | 76 seções críticas, uma só porta | `CONCORRENCIA.md` §1.1 |
| ela come paralelismo já com 2 clientes e metade da máquina ociosa | controle `ping` **1,99×**, `varrer` **1,51–1,59×**, `inserir` **1,45–1,49×** | `DESEMPENHO.md` §14 |
| o gap medido é **leitor-com-leitor** — a bateria rodou N leitores e **nenhum** escritor | idem | `CONCORRENCIA.md` §5(1) |
| a leitura segura a trava **23× mais** que a gravação, no padrão | `varrer(50)` 3.122–3.187 µs contra `inserir` 121–137 µs | `CONCORRENCIA.md` §7.1 |
| não há um segundo gargalo embaixo da trava | tabelas separadas escalam **igual** à mesma tabela (1,70 contra 1,67) | `DESEMPENHO.md` §14 |

E o fato que decide o desenho, e que não é um número:

> **A trava não protege a `Instancia`.** Ela tem **um** campo (`PathBuf`) e
> **todos** os métodos são `&self`. O `Mutex` é uma **ficha de exclusão** para o
> caminho de dados; o estado protegido está **no disco**, alcançado por um
> `Table` que é **aberto e fechado a cada operação**. Consequência:
> `RwLock<Instancia>` **compila de primeira e está errado**.
> — `CONCORRENCIA.md` §2

Isso já basta para uma triagem que economiza metade da pesquisa: **a nossa trava
protege o catálogo de tabelas abertas, e não páginas.** Toda receita que fala de
página — buffer pool, WAL-index, trava de bloco — só serve aqui se a ideia
sobreviver à troca da unidade protegida.

---

## 1. O que ESTA rodada mediu, e por que estes dois números

Duas medições próprias, as duas escolhidas porque uma receita de fora depende
delas para virar plano ou morrer.

### 1.1 Abrir a tabela custa 47 µs, e isso é 1,5% do que a trava fica presa

```bash
flock /tmp/phx-cargo.lock cargo build --release --example custo-de-abrir -p phxsql-store
./target/release/examples/custo-de-abrir 5000
```

| amostra | abrir a tabela (7 arquivos) e fechar | inserir com a tabela já aberta |
|---|---:|---:|
| 1 | **47,07 µs** | 10,72 µs |
| 2 | **48,76 µs** | 10,71 µs |

A máquina estava **ocupada** — `loadavg` 4,39 e 4,09 em 4 núcleos, com outras
frentes compilando. Isso não invalida o número **nesta direção**: máquina
ocupada deixa o `abrir` mais lento, então 47 µs é **teto**, e todo argumento
abaixo fica mais forte com o número real, não mais fraco.

**Por que este número existe:** no `servidor.rs` o `abrir` acontece **dentro** da
seção crítica — o padrão é `let dados = self.travar_dados()?;` e só então
`dados.abrir_qualificada(...)`. Então a seção crítica de uma leitura tem duas
partes de naturezas diferentes:

* **a consulta ao catálogo** — achar e abrir os 7 arquivos: **47 µs**;
* **o trabalho** — descer o índice, ler as linhas, montar a resposta: o resto.

Contra os 3.122 µs que a trava fica presa num `varrer(50)` (§7.1 do
`CONCORRENCIA.md`), a consulta ao catálogo é **~1,5%** do tempo de posse.

> **Esta razão é uma ordem de grandeza, e não um número para publicar.** Os dois
> lados vêm de baterias diferentes: 47 µs é dentro do processo, numa tabela de
> 1.000 linhas; 3.122 µs é pelo soquete, num `varrer(50)` sobre 50.000 linhas
> com `Memo`. Dizer «1,5%» com casa decimal seria a mesma mentira que este
> projeto já cometeu ao comparar `varrer(50)` com `inserir(1)` sem dizer.
>
> **O número que fecharia isto**, e ele é barato: um segundo cronômetro dentro
> do escopo da trava, separando `abrir` do resto, lido pela telemetria que o
> `quanto-a-trava-fica-presa.py` já lê. É o mesmo instrumento, com uma marca a
> mais.

### 1.2 O mapa da trava, rodado hoje — e o `espelho` muda o teto do `RwLock`

```bash
python3 bancada/concorrencia/mapa-da-trava.py
```

| o que o mapa diz hoje (03/09) | número |
|---|---:|
| seções críticas | **76** |
| **não alcançam marcador de escrita** por caminho próprio — o teto do `RwLock` | **39/76** |
| idem, **e também sem a porta comum do espelho** (`recursos.espelho`) | **25/76** |
| alcançam `fsync` com a trava na mão — o que o `RwLock` **não** conserta | 22/76 |
| rodam código do dono do banco (gatilho `BEFORE`) | 5/76 |
| têm ponto de cancelamento próprio | 4/76 |
| soltam a trava cedo por `drop` | 8/76 |
| têm laço direto dentro da seção | 40/76 |

Classes: código-do-dono 5 (378 linhas), rede-ou-espera **0**, escrita-durável 18
(489), escrita 14 (293), leitura-com-varredura **29 (1.869)**, leitura-curta 10
(101). Tamanhos: menor 3, mediana 26, p90 89, maior 243, soma 3.130 linhas.

> Os números andaram de novo desde a redação da `CONCORRENCIA.md` (que registra
> 19/28/23/37) sem ninguém mexer neles — foram as frentes vizinhas mexendo nas
> cadeias que o mapa percorre. É por isso que o gerador existe, e é por isso que
> eu o rodei em vez de copiar a tabela de lá.

**O achado desta corrida, e ele importa para a receita do SQLite:** o teto do
`RwLock` **depende de configuração**. Com `recursos.espelho` **desligado** (que é
o padrão, `espelho: false` no `config.rs`) ele é **39/76**. Com o espelho
**ligado**, abrir tabela para **ler** pode sincronizar, e o teto cai para
**25/76** — uma queda de **36%** no que um `RwLock` teria para deixar passar. E
há um segundo caminho pelo qual abrir escreve, independente do espelho: o
`Table::abrir` **cria** o `.trash` e o `.reason` quando faltam
(`«abrir destes dois CRIA quando falta»`, no `table.rs`). Dois «leitores»
concorrentes na primeira abertura de uma tabela disputam a criação de arquivo.

*Leitor que escreve não é leitor*, e é isso que uma trava leitora-escritora não
tem como saber sozinha.

---

## 2. SQLite — o modo WAL, e a receita que responde a OUTRA pergunta

### O que a documentação oficial diz

Sobre a concorrência que o WAL compra:

> «Because writers do nothing that would interfere with the actions of readers,
> writers and readers can run at the same time.»
> — <https://www.sqlite.org/wal.html>

> «However, since there is only one WAL file, there can only be one writer at a
> time.»
> — <https://www.sqlite.org/wal.html>

Como o leitor fixa a versão que enxerga:

> «When a read operation begins on a WAL-mode database, it first remembers the
> location of the last valid commit record in the WAL. Call this point the "end
> mark". […] for any particular reader, the end mark is unchanged for the
> duration of the transaction, thus ensuring that a single read transaction only
> sees the database content as it existed at a single point in time.»
> — <https://www.sqlite.org/wal.html>

**A granularidade real**, que é a pergunta que interessa. O WAL não a muda: ela
continua sendo o **arquivo inteiro**, e o que muda é quem espera por quem.

> «SQLite uses reader/writer locks to control access to the database. […]
> Multiple processes can be doing a SELECT at the same time. But only one process
> can be making changes to the database at any moment in time, however.»
> — <https://www.sqlite.org/faq.html>

> «When any process wants to write, it must lock the entire database file for the
> duration of its update.»
> — <https://www.sqlite.org/faq.html>

As travas do WAL-index, que são o mecanismo por baixo (e o desmentido de quem
imagina trava por página):

> «An EXCLUSIVE WAL_WRITE_LOCK is held by any connection that is appending
> content to the end of the WAL. Hence, only a single process at a time can
> append content to the WAL.»
> — <https://www.sqlite.org/walformat.html>

> «There are five separate read locks, numbers 0 through 4. […] Connections
> obtain a shared lock on one of the read locks bytes while they are within a
> transaction.»
> — <https://www.sqlite.org/walformat.html>

### O que ele cobra

> «To avoid forcing every reader to scan the entire WAL […] a data structure
> called the "wal-index" is maintained in shared memory […] The wal-index greatly
> improves the performance of readers, but the use of shared memory means that
> all readers must exist on the same machine. This is why the write-ahead log
> implementation will not work on a network filesystem.»
> — <https://www.sqlite.org/wal.html>

> «read performance deteriorates as the WAL file grows in size since each reader
> must check the WAL file for the content and the time needed to check the WAL
> file is proportional to the size of the WAL file.»
> — <https://www.sqlite.org/wal.html>

> «WAL might be very slightly slower (perhaps 1% or 2% slower) than the
> traditional rollback-journal approach in applications that do mostly reads and
> seldom write.»
> — <https://www.sqlite.org/wal.html>

> «By default, the checkpoint will be run automatically by the same thread that
> does the COMMIT that pushes the WAL over its size limit. This has the effect of
> causing most COMMIT operations to be very fast but an occasional COMMIT (those
> that trigger a checkpoint) to be much slower.»
> — <https://www.sqlite.org/wal.html>

E o que o checkpoint cobra do leitor longo, que é a mesma conta da *history
list* que o roteiro mediu no InnoDB:

> «A checkpoint can run concurrently with readers, however the checkpoint must
> stop when it reaches a page in the WAL that is past the end mark of any current
> reader.»
> — <https://www.sqlite.org/wal.html>

### O que isso compraria AQUI

**A receita responde uma pergunta que nós não fizemos.** O par que o WAL
conserta é **leitor com escritor**. O nosso gap medido é **leitor com leitor**
(§0). São pares diferentes, e o WAL não toca o nosso.

A parte do SQLite que responde ao **nosso** par não é o WAL — é a trava
leitora-escritora simples sobre o arquivo inteiro, que ele já tinha antes do
WAL: *«Multiple processes can be doing a SELECT at the same time»*. Ou seja: **o
SQLite já dá leitor-com-leitor sem WAL nenhum**, com exatamente o desenho que
aqui se chama «`RwLock`» na matriz da `CONCORRENCIA.md` §3.

Há uma assimetria a favor nossa e uma contra:

* **A favor:** o custo central do WAL no SQLite — o wal-index em memória
  compartilhada, o arquivo `-shm`, a proibição de sistema de arquivos em rede —
  é o preço de ser **serverless e multiprocesso**. O PhxSql é **um processo com
  threads**: o equivalente do wal-index aqui é um campo de `struct`, e não um
  arquivo. *A parte cara da receita não se aplica.*
* **Contra:** o que o SQLite chama de «readers don't block the writer» ele
  consegue porque o leitor **não escreve nada**. Aqui, o §1.2 mediu que abrir
  para ler pode sincronizar (espelho) e pode criar arquivo (`.trash`,
  `.reason`). Antes de qualquer trava leitora-escritora, **o caminho de leitura
  tem de deixar de escrever** — e isso é trabalho de código, não de desenho de
  trava.

**Um detalhe do WAL que vale ser roubado mesmo sem o WAL:** os **cinco** slots de
read-mark. O SQLite não dá uma trava por leitor nem uma trava só: dá **cinco**, e
cada leitor pega uma compartilhada. É a mesma ideia da §4 (particionar em vez de
trocar por leitora-escritora), num motor de arquivo, com o número mais modesto
possível. Guarde: a receita do PostgreSQL(R) vai chegar ao mesmo lugar por outro
caminho, e o encontro das duas é o que dá confiança na ideia.

---

## 3. MySQL(R)/InnoDB — o que de fato exige trava global, e o que não

### Trava por linha: o que a documentação diz

> «`InnoDB` implements standard row-level locking where there are two types of
> locks, shared (`S`) locks and exclusive (`X`) locks.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-locking.html>

> «Record locks always lock index records, even if a table is defined with no
> indexes. For such cases, `InnoDB` creates a hidden clustered index and uses
> this index for record locking.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-locking.html>

**A linha de cima é a mais importante desta seção para nós**, e não a primeira:
*a trava por linha do InnoDB é, na verdade, trava por **registro de índice***. O
InnoDB só consegue trava fina porque **tem onde pendurá-la** — um índice
agrupado que contém a linha. Aqui a linha mora no `.reg`, endereçada por conta
(`offset = data_offset + (slot-1) * slot_size`), e o `.ndx` é um índice
**secundário** que aponta para o rowid. Não há registro de índice agrupado onde
pendurar trava de linha: pendurá-la no rowid é possível, mas é **inventar** a
estrutura, não copiar a receita.

### Buffer pool: a receita de particionar, e ela é a mesma do PostgreSQL(R)

> «For systems with buffer pools in the multi-gigabyte range, dividing the buffer
> pool into separate instances can improve concurrency, by reducing contention as
> different threads read and write to cached pages.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-multiple-buffer-pools.html>

> «Each page that is stored in or read from the buffer pool is assigned to one of
> the buffer pools randomly, using a hashing function.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-multiple-buffer-pools.html>

E o **limiar**, que é o número que mata a receita para o nosso tamanho:

> «This option takes effect only when you set `innodb_buffer_pool_size` to a size
> of 1GB or more.» […] «For best efficiency, specify a combination of
> `innodb_buffer_pool_instances` and `innodb_buffer_pool_size` so that each
> buffer pool instance is at least 1GB.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-multiple-buffer-pools.html>

O nosso cache de páginas do `.ndx` é `PAGINAS_PADRAO = 2048` de
`PAGINA_PADRAO = 4096` — **8 MiB por tabela aberta** (`ndx.rs`). O próprio
InnoDB diz que particionar buffer pool **não tem efeito** abaixo de 1 GiB. Somos
**128× menores** que o limiar em que o dono da receita a liga. *Receita boa para
o gargalo alheio não é receita para o nosso*: esta é literalmente a mesma frase,
com o número do fabricante ao lado.

### `innodb_thread_concurrency`

> «`innodb_thread_concurrency` limits the number of concurrently executing
> operating system threads (and thus the number of requests that are processed at
> any one time).» […] Default `0`: «by default there is no limit on the number of
> concurrently executing threads.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-performance-thread_concurrency.html>

> «additional threads sleep for a number of microseconds, set by the
> configuration parameter `innodb_thread_sleep_delay`, before being placed into
> the queue» […] «Threads waiting for locks are not counted in the number of
> concurrently executing threads.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-performance-thread_concurrency.html>

> «Before limiting the number of concurrently executing threads, review
> configuration options that may improve the performance of `InnoDB` on
> multi-core and multi-processor computers.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-performance-thread_concurrency.html>

**Isto não é uma receita de concorrência: é um freio.** O padrão do fabricante é
**desligado**, e o próprio manual manda olhar tudo o mais antes de ligá-lo. E o
que ele faria aqui é o oposto do que se procura: nós temos uma trava global que
já entrega **exatamente 1** de concorrência efetiva no caminho de dados, e
`conexoes_max = 64` já é o nosso teto de threads. Ligar um segundo freio sobre um
motor já freado é a definição de receita que não se aplica.

### O achado que dói: a trava do AUTO-INC é a nossa ordem de digitação

Esta é a parte da pesquisa em que o InnoDB responde uma pergunta que ninguém
tinha feito, e a resposta é um **não** para nós.

> «In this lock mode, all "INSERT-like" statements obtain a special table-level
> `AUTO-INC` lock […] to ensure that auto-increment values are assigned in a
> predictable and repeatable order for a given sequence of INSERT statements, and
> to ensure that auto-increment values assigned by any given statement are
> consecutive.» (modo 0, «traditional»)
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-auto-increment-handling.html>

> «In this lock mode, no "INSERT"-like statements use the table-level `AUTO-INC`
> lock, and multiple statements can execute at the same time. This is the fastest
> and most scalable lock mode, but it is *not safe* when using statement-based
> replication or recovery scenarios when SQL statements are replayed from the
> binary log.» (modo 2, «interleaved»)
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-auto-increment-handling.html>

> «If you are using statement-based replication, set `innodb_autoinc_lock_mode`
> to 0 or 1 […] Auto-increment values are not ensured to be the same on the
> replicas as on the source if you use `innodb_autoinc_lock_mode` = 2.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-auto-increment-handling.html>

**Traduzido para cá:** o nosso rowid é `slot_count + 1`, e a réplica é fiel
porque *«se a réplica aplicar **todos** os eventos, **na ordem**, e **mais
ninguém** escrever nela, os rowids saem exatamente iguais aos do Source»*
(`REPLICACAO.md` §5) — e o `aplicar_evento` **para** quando o rowid diverge.

Somos, para efeito desta receita, **exatamente o caso de statement-based
replication**: dependemos de que a **ordem de alocação** seja reproduzível. O
InnoDB nomeia o preço da concorrência aqui, e o preço é a réplica.

**Mas há uma boa notícia, e ela vem do nosso formato, não da receita:** o `.log`
— que carrega a imagem da linha replicada — é **por tabela** (é uma das
extensões da tabela, ao lado de `.reg` e `.ndx`). A ordem que a réplica precisa é
uma **ordem total por tabela**, e não uma ordem total do servidor. Portanto:

> **Trava por tabela (ou trava particionada por hash do nome da tabela) preserva
> exatamente o invariante que a replicação exige.** Uma trava mais fina que a
> tabela — por página, por linha, por partição do `.reg` — **não** preserva, e
> cai na mesma objeção que o InnoDB registra para o modo 2.

Isso não é dedução livre: é a mesma conta que a §4.1 da `CONCORRENCIA.md` já fez
para o MVCC (a versão velha tem de ficar **fora** do `.reg` ou a replicação
quebra), aplicada agora à granularidade da trava em vez do formato.

---

## 4. PostgreSQL(R) — LWLocks e travas de partição de buffer

Esta é a pista que o pedido mandou olhar com mais atenção, e ela merece: é a
única das três que muda o **desenho** e não o **mecanismo**.

### O que a documentação oficial diz

O que é um LWLock, e por quanto tempo ele é para ser segurado:

> «These locks are typically used to interlock access to datastructures in shared
> memory.» […] «LWLocks support both exclusive and shared lock modes (for
> read/write and read-only access to a shared object).» […] «There is no
> provision for deadlock detection, but the LWLock manager will automatically
> release held LWLocks during elog() recovery.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/lmgr/README>

A partição, que é a ideia inteira:

> «As of PG 8.2, the BufMappingLock has been split into NUM_BUFFER_PARTITIONS
> separate locks, each guarding a portion of the buffer tag space.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README>

> «The partition that a particular buffer tag belongs to is determined from the
> low-order bits of the tag's hash value.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README>

> «To look up whether a buffer exists for a tag, it is sufficient to obtain share
> lock on the BufMappingLock.» […] «To alter the page assignment of any buffer,
> one must hold exclusive lock on the BufMappingLock.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README>

E a regra que impede o abraço mortal, que é a única coisa que a partição
acrescenta de risco:

> «If it is necessary to lock more than one partition at a time, they must be
> locked in partition-number order to avoid risk of deadlock.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README>

Os números, do próprio cabeçalho:

> `#define NUM_BUFFER_PARTITIONS 128` — «Number of partitions of the shared
> buffer mapping hashtable»
> `#define LOG2_NUM_LOCK_PARTITIONS 4` → **16** — «Number of partitions the
> shared lock tables are divided into»
> — <https://github.com/postgres/postgres/blob/master/src/include/storage/lwlock.h>

A implementação, que cabe em três linhas e é o retrato de quão barata a ideia é:

```c
static inline uint32
BufTableHashPartition(uint32 hashcode)
{
	return hashcode % NUM_BUFFER_PARTITIONS;
}
```
> — <https://github.com/postgres/postgres/blob/master/src/include/storage/buf_internals.h>

E a separação em **dois níveis**, que é a metade da receita que ninguém cita:

> «Each buffer header also contains an LWLock, the "buffer content lock", that
> \*does\* represent the right to access the data in the buffer.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README>

Os dois níveis inclusive aparecem separados na telemetria do produto:

> **BufferMapping** — «Waiting to associate a data block with a buffer in the
> buffer pool.»
> **BufferContent** — «Waiting to access a data page in memory.»
> — <https://www.postgresql.org/docs/current/monitoring-stats.html>

### O que isso compraria AQUI — e a resposta tem duas metades opostas

**Metade que NÃO serve: particionar por hash, com 128 partições.**

A partição existe para um espaço de chaves **enorme**: um `shared_buffers`
típico tem centenas de milhares a milhões de buffers, e uma trava por buffer
custaria mais memória de trava que de dado. Cento e vinte e oito é o número que
divide *isso*.

O nosso espaço de chaves é o **nome qualificado da tabela**. Uma base desta casa
tem dezenas de tabelas, não milhões. Um mapa `"database/tabela" -> trava` com
dezenas de entradas é pequeno o bastante para não precisar de partição nenhuma —
e particionar por hash, com dezenas de chaves em 128 baldes, **introduz** um
defeito que o mapa direto não tem: duas tabelas que caem no mesmo balde passam a
se serializar sem motivo.

A aritmética é simples e não precisa de medição: com `T` tabelas ativas e `P`
partições, o número esperado de pares que colidem é `C(T,2)/P`. Com 10 tabelas e
128 partições, **0,35 pares** — pouco. Com 40 tabelas, **6,1 pares** — que é
serialização falsa que ninguém vai achar por leitura, porque ela não está em
lugar nenhum do código: está no `%`.

**Então a recomendação é o oposto da pista, e é honesto dizer isso:** o
PostgreSQL(R) particiona porque **não pode** ter uma trava por chave. Nós
**podemos**. Copiar a partição seria copiar a **concessão** dele, não a ideia
dele.

**Metade que SERVE, e é a mais valiosa de toda esta pesquisa: os dois níveis.**

Repare no que o `BufMappingLock` **não** cobre: ele é segurado enquanto se
**consulta ou muda o mapeamento** — «to look up whether a buffer exists for a
tag» —, e a leitura do **conteúdo** é protegida por outra trava, no cabeçalho do
buffer. Os dois níveis têm durações de ordens de grandeza diferentes, e é isso
que faz a partição funcionar: a trava disputada é segurada por pouquíssimo
tempo.

A nossa trava faz **as duas coisas de uma vez**, e o §1.1 mediu a proporção:

| na seção crítica de uma leitura | o que é | ordem de grandeza |
|---|---|---:|
| achar e abrir a tabela — o **mapeamento** | consulta ao catálogo | **~47 µs** |
| descer o índice, ler as linhas, montar a resposta — o **conteúdo** | o trabalho | ~3.075 µs |

**A receita traduzida para o nosso caso**, e ela é uma frase:

> Uma trava curta e global (ou particionada, se algum dia houver milhares de
> tabelas) para **consultar o catálogo de tabelas abertas**, e uma trava por
> tabela — essa, sim, leitora-escritora — para **fazer o trabalho**.

E é aqui que a receita fecha com o achado do §2 da `CONCORRENCIA.md`, em vez de
esbarrar nele. O problema registrado é que `RwLock<Instancia>` compila e está
errado *porque a `Instancia` não tem estado a proteger*. A receita do
PostgreSQL(R) diz **qual estado criar**: um **catálogo de tabelas abertas**,
`"database/tabela" -> Arc<RwLock<Table>>`, que é exatamente a estrutura que o
`BufMappingLock` protege lá. Com ele existindo:

* a `Instancia` passa a ter estado, e o `RwLock` passa a proteger alguma coisa;
* o marcador `PhantomData<Cell<()>>` que hoje **impede** o `RwLock` de compilar
  sai de propósito e por escrito, que é exatamente o que o comentário dele pede;
* a ordem canônica contra o abraço mortal é a mesma que o PostgreSQL(R) usa —
  *«they must be locked in partition-number order»* —, aqui a ordem lexicográfica
  do nome qualificado, que já existe escrita no `TRANSACOES.md` §11.3.

### E o custo que essa receita cobra aqui, que é de MEMÓRIA e ninguém contou

Manter tabela aberta muda uma conta que hoje ninguém paga. O cache de páginas do
`.ndx` **mora dentro do `NdxFile`** (`cache: CachePaginas`, criado em
`NdxFile::abrir`), e o `NdxFile` mora dentro do `Table`, que é **aberto e fechado
a cada operação**. Então:

| | páginas em RAM |
|---|---|
| **hoje**, com a trava global | a tabela da operação corrente: **8 MiB** (2.048 × 4 KiB) |
| **N leitores em paralelo, cada um abrindo o seu** | **N × 8 MiB** |
| com `conexoes_max = 64` (o padrão do `config.rs`) | **até 512 MiB** |

E `memoria_max_mb` nasce **0**, que é «sem teto» (`config.rs`).

**Este é o número que reordena a receita.** O que o buffer pool compartilhado do
PostgreSQL(R) e do InnoDB compra, no nosso caso, **não é concorrência: é o teto
de memória que a concorrência de leitores multiplicaria.** Um catálogo de
tabelas abertas com `Arc<Table>` compartilhado dá as duas coisas de uma vez — os
leitores param de se ver **e** param de duplicar 8 MiB cada um.

*O caminho contrário — soltar os leitores sem catálogo compartilhado — compra o
paralelismo e paga com 512 MiB de pico que hoje não existe, num servidor cujo
teto de memória nasce desligado.*

---

## 5. As três colunas

Uma receita que eu não sei como medir contra o nosso caso **não vira plano**, e a
terceira coluna é onde isso aparece.

| receita | o que ela custa aqui | o número que a mataria ou a confirmaria |
|---|---|---|
| **SQLite — WAL** (leitor não espera escritor) | **Responde outro par.** O nosso gap medido é leitor-com-leitor; o WAL conserta leitor-com-escritor. É o mesmo par que a SP000016 (MVCC) já cobre, com o mesmo custo de formato — e aqui ele viria com o custo extra do *checkpoint*, que o SQLite documenta como «an occasional COMMIT much slower» | **Confirma:** o p99 de um `varrer` com um escritor ao lado contra o p99 do mesmo `varrer` sozinho. Já é exatamente o que o `escolher-o-desenho.py` mede para o MVCC — falta só a máquina parada. **Mata:** se esse p99 for igual ao p99 sozinho, não há leitor esperando escritor para consertar |
| **SQLite — trava leitora-escritora sobre o arquivo inteiro** (o que ele já fazia antes do WAL, e o que responde ao NOSSO par) | Exige que o caminho de leitura **pare de escrever**. Medido hoje: com `recursos.espelho` ligado o teto do `RwLock` cai de **39/76** para **25/76** seções, e o `Table::abrir` **cria** `.trash` e `.reason` quando faltam | **Confirma:** a curva de controle (`ping`, que não toma a trava) contra a de leitura, com N leitores — o `escolher-o-desenho.py` já a separa assim, e a §14 já deu a distância (1,99× contra 1,51–1,59×). **Mata:** se com o espelho ligado o teto de 25/76 não cobrir as operações que a carga real usa. Isso o fonte não diz — pede contagem de chamadas por operação, que hoje ninguém coleta |
| **InnoDB — trava por linha** | **Não há onde pendurar.** A trava por linha do InnoDB é trava por **registro de índice agrupado**; aqui a linha mora no `.reg` por conta aritmética e o `.ndx` é secundário. Seria inventar a estrutura, não copiar a receita | **Não sei medir isto contra o nosso caso, e por isso não recomendo.** Não existe experimento barato que separe «trava por linha ajudaria» de «trava por tabela ajudaria» antes de uma das duas existir. Recomendar seria palpite com sotaque de manual |
| **InnoDB — buffer pool particionado** (`innodb_buffer_pool_instances`) | **O fabricante desliga a receita abaixo de 1 GiB.** O nosso cache é **8 MiB por tabela aberta** — 128× abaixo do limiar em que a receita do dono dela liga | **Já está morta pelo número do próprio fabricante**, e é uma recusa medida: enquanto `cache_paginas × PAGINA_PADRAO` não passar de 1 GiB, a proposta não volta. Reviveria se algum dia houver pool compartilhado ≥ 1 GiB |
| **InnoDB — `innodb_thread_concurrency`** | É **freio**, não motor. Nasce desligado no fabricante, e o manual manda olhar tudo o mais antes de ligá-lo. Aqui já há `conexoes_max = 64` e uma trava global que entrega concorrência efetiva 1 | **Já está morta pela construção**: pôr teto de threads num caminho que já serializa em 1 não pode aumentar vazão. Nenhuma medição é necessária, e é por isso que ela não deve ser feita |
| **InnoDB — `innodb_autoinc_lock_mode=2`** (soltar a alocação sequencial) | **Quebra a réplica**, e o fabricante escreve isso: «not safe when using statement-based replication». Somos esse caso — `REPLICACAO.md` §5, e `aplicar_evento` **para** quando o rowid diverge | **Confirmada como recusa**, com URL. E o corolário útil: como o `.log` é **por tabela**, uma trava **por tabela** preserva a ordem que a réplica exige; qualquer granularidade mais fina que a tabela não preserva. **O número que fecharia:** rodar a bancada de replicação (modo A) com dois escritores em tabelas diferentes e conferir que `aplicar_evento` não para |
| **PostgreSQL(R) — particionar a trava por hash (128 partições)** | **Copia a concessão, não a ideia.** Postgres particiona porque não pode ter trava por chave; nós podemos. Com dezenas de tabelas em 128 baldes, a colisão é serialização falsa que não aparece em lugar nenhum do código | **Mata:** a aritmética `C(T,2)/P` — com 40 tabelas ativas dá **6,1 pares** colidindo à toa. **Confirmaria** só se `T` passasse de alguns milhares, e aí o número a medir é quantas tabelas uma base real tem. Hoje não sabemos, e é barato saber: contar `.reg` por base |
| **PostgreSQL(R) — os DOIS NÍVEIS** (trava curta para o mapeamento, trava separada para o conteúdo) | **É a receita que serve, e a que resolve o §2 da `CONCORRENCIA.md`.** Custa criar o estado que hoje não existe: um catálogo de tabelas abertas `"database/tabela" -> Arc<RwLock<Table>>`. E custa **memória**: manter tabela aberta mantém o cache de 8 MiB por `.ndx` vivo — mas é o mesmo movimento que **evita** os N × 8 MiB que leitores paralelos sem catálogo custariam | **Confirma:** medido nesta rodada, o mapeamento (`abrir`, **47,07 / 48,76 µs**) é ~1,5% do que a trava fica presa numa leitura (3.122 µs, §7.1). Se a parte globalmente serializada cair para ~1,5%, o teto de Amdahl da leitura sai de 1× para ~**66×** — muito acima dos 4 núcleos desta máquina. **Mata:** se um cronômetro dentro do escopo da trava mostrar que `abrir` é a maior parte do tempo de posse, e não a menor. **Este é o número que falta**, e ele é barato: uma segunda marca na telemetria que o `quanto-a-trava-fica-presa.py` já lê |

---

## 6. O que eu recomendo NÃO perseguir, e por quê

**1. WAL, como resposta à SP000011.** Não porque seja ruim — porque responde
outro par. Se a casa quiser leitor-sem-esperar-escritor, o item já tem nome,
número e formato decidido: é a SP000016, com a versão velha **fora** do `.reg`
(`CONCORRENCIA.md` §4.2). Chamar isso de WAL só acrescentaria um nome.

**2. Trava por linha.** Não tenho como medi-la contra o nosso caso antes de ela
existir, e não há estrutura onde pendurá-la sem inventar formato. Recomendá-la
seria exatamente o que este papel existe para não fazer.

**3. Buffer pool particionado e `innodb_thread_concurrency`.** As duas morrem com
o número do próprio fabricante, e ficam registradas aqui **com o número** para
não voltarem: 1 GiB de limiar contra os nossos 8 MiB, e um freio que nasce
desligado sobre um motor que já serializa em 1.

**4. Particionar a trava por hash com 128 partições.** A pista era boa e a
conclusão é o contrário dela: com dezenas de tabelas, o mapa direto é melhor que
a partição, e a partição **acrescenta** serialização falsa. A ideia do
PostgreSQL(R) que serve é a outra metade — os dois níveis.

**5. `innodb_autoinc_lock_mode=2` (soltar a alocação sequencial de rowid).**
Recusa com URL do fabricante e com a nossa própria `REPLICACAO.md` §5. E ela deixa
o presente: **a tabela é a granularidade mais fina que a réplica aceita.**

### O que eu recomendo perseguir, e nesta ordem

1. **O cronômetro que falta** — separar, dentro do escopo da trava, o tempo de
   `abrir` do tempo de trabalho. É uma marca a mais na telemetria que o
   `quanto-a-trava-fica-presa.py` já lê, e é o número que confirma ou mata a
   receita dos dois níveis. **Medir a premissa do item vem antes de implementar o
   item.**
2. **Tirar a escrita do caminho de leitura** — o espelho e a criação de `.trash`
   / `.reason` no `abrir`. Sem isso nenhuma trava leitora-escritora é honesta, e
   com isso o teto do `RwLock` sobe de 25/76 para 39/76 **sem escrever desenho
   nenhum**.
3. **O catálogo de tabelas abertas** — que é o estado que hoje não existe, e sem
   o qual o `RwLock` protege nada (`CONCORRENCIA.md` §2). Ele traz junto o teto de
   memória que a concorrência de leitores multiplicaria.
4. **Trava por tabela sobre esse catálogo**, com ordem canônica lexicográfica
   contra o abraço mortal — e é a granularidade mais fina que a replicação aceita.

Os itens 1 e 2 **não precisam de máquina parada**. O 3 e o 4 precisam da bateria
do `escolher-o-desenho.py` para fechar.

---

## 7. Fontes

Toda afirmação de manual acima tem URL. As que não achei, digo que não achei:

* **Não achei** documentação oficial do SQLite que dê o custo em microssegundos
  do WAL-index por leitor — o site dá a forma e a proibição de rede, não o
  tempo.
* **Não achei** número oficial do PostgreSQL(R) para o tempo de posse do
  `BufMappingLock`; o que existe é a categoria de espera (`BufferMapping`) na
  telemetria, que é onde o número aparece **na instalação de quem mede**, e não
  no manual.

### Documentação oficial consultada

| assunto | URL |
|---|---|
| SQLite, modo WAL | <https://www.sqlite.org/wal.html> |
| SQLite, travas do WAL-index | <https://www.sqlite.org/walformat.html> |
| SQLite, concorrência e granularidade | <https://www.sqlite.org/faq.html> |
| InnoDB, travas de linha, gap e next-key | <https://dev.mysql.com/doc/refman/8.4/en/innodb-locking.html> |
| InnoDB, buffer pool com múltiplas instâncias | <https://dev.mysql.com/doc/refman/8.4/en/innodb-multiple-buffer-pools.html> |
| InnoDB, `innodb_thread_concurrency` | <https://dev.mysql.com/doc/refman/8.4/en/innodb-performance-thread_concurrency.html> |
| InnoDB, AUTO_INCREMENT e os três modos de trava | <https://dev.mysql.com/doc/refman/8.4/en/innodb-auto-increment-handling.html> |
| PostgreSQL(R), LWLocks e partição do gestor de travas | <https://github.com/postgres/postgres/blob/master/src/backend/storage/lmgr/README> |
| PostgreSQL(R), partição da tabela de buffers | <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README> |
| PostgreSQL(R), `NUM_BUFFER_PARTITIONS` e `NUM_LOCK_PARTITIONS` | <https://github.com/postgres/postgres/blob/master/src/include/storage/lwlock.h> |
| PostgreSQL(R), `BufTableHashPartition` | <https://github.com/postgres/postgres/blob/master/src/include/storage/buf_internals.h> |
| PostgreSQL(R), eventos de espera `BufferMapping` e `BufferContent` | <https://www.postgresql.org/docs/current/monitoring-stats.html> |

### Números desta casa, e de onde saíram

| número | de onde |
|---|---|
| 47,07 / 48,76 µs para abrir a tabela | `./target/release/examples/custo-de-abrir 5000`, duas amostras, 03/09, `loadavg` 4,39 e 4,09 |
| 76 seções, 39/25/22/5/4/8/40 de 76, as seis classes | `python3 bancada/concorrencia/mapa-da-trava.py`, rodado em 03/09 |
| 3.122–3.187 µs de trava numa leitura; 121–137 µs numa gravação | `CONCORRENCIA.md` §7.1 — **medição anterior, não refeita aqui** |
| 1,99× / 1,51–1,59× / 1,45–1,49× | `DESEMPENHO.md` §14 — **medição anterior, não refeita aqui** |
| 8 MiB por tabela aberta | `PAGINAS_PADRAO = 2048` e `PAGINA_PADRAO = 4096`, em `crates/phxsql-store/src/ndx.rs` |
| `conexoes_max = 64`, `memoria_max_mb = 0`, `espelho = false` | `crates/phxsql-server/src/config.rs` |
| `Table::abrir` cria `.trash` e `.reason` | comentário e código de `Table::abrir`, em `crates/phxsql-store/src/table.rs` |
| o `.log` é por tabela | a lista `EXTENSOES` do `catalogo.rs`, e `REPLICACAO.md` §3 |
| o rowid é `slot_count + 1` e a réplica para na divergência | `REPLICACAO.md` §5, e `Table::aplicar_evento` |
