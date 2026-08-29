# O caminho de escrita e o quórum do Apache Cassandra, lidos no fonte

Documento de **leitura de código**, no mesmo contrato do `CONCORRENTES.md`:
toda afirmação sobre o código deles traz `arquivo:linha`; toda afirmação de
custo diz se é **medida** ou **inferida** — e, quando é inferida, a frase diz
isso; toda proposta vem com **a medição que a confirmaria**, com o número
esperado.

São duas perguntas:

1. **O que o caminho de escrita do Cassandra faz para ser tão rápido**, e
   quanto disso cabe num motor de arquivos separados com ordem de digitação
   sagrada.
2. **Como funciona o quórum de escrita** — quem conta, quem espera, o que
   acontece com quem não respondeu, e o que o cliente pode concluir do OK.

> **A resposta curta da primeira.** O commit log **não é** o segredo, e a
> memtable **não é** o segredo sozinha. O segredo é o que a escrita **deixa de
> fazer**: ela não lê nada antes de gravar, não decide se aceita ou recusa, não
> toca no arquivo de dados, não mantém árvore nenhuma em disco e não sela
> página nenhuma. O que ela faz é **serializar a linha uma vez**, copiá-la para
> dentro de um `mmap` na posição que um `compareAndSet` reservou, e enfiar um
> ponteiro numa *skip list* em RAM. O commit log existe para que essa escrita
> em RAM possa ser confirmada ao cliente; a memtable existe para que ela seja
> barata. Uma sem a outra não funciona.
>
> E o preço está escrito no mesmo fonte: **o INSERT do Cassandra não sabe
> recusar uma chave repetida**, porque ele não lê antes de gravar
> (`ModificationStatement.java:408-417`). O conflito é resolvido depois, na
> leitura, por carimbo de hora (`Cells.java:79-84`). A conferência de unicidade
> que o nosso `table.rs:763-770` faz **antes de qualquer gravação** não tem
> equivalente lá — e não tem porque lá ela não é necessária, não porque eles
> descobriram um jeito rápido de fazê-la.

> **A resposta curta da segunda.** O `QUORUM` do Cassandra **não** quer dizer
> «o dado está em N discos». Com a configuração padrão
> (`commitlog_sync: periodic`, `conf/cassandra.yaml:634-636`) ele quer dizer
> «N processos copiaram os bytes para dentro de um `mmap` e puseram a linha na
> memtable». O `fsync` acontece **a cada 10 segundos**, numa thread de fundo, e
> nenhuma escrita espera por ele
> (`PeriodicCommitLogService.java:36-45`). Só o modo `batch`
> (`BatchCommitLogService.java:36-43`) faz o OK significar disco.

---

## 1. O que foi lido

| | Versão | Commit | Onde |
|---|---|---|---|
| Apache Cassandra | 5.0.10 (`build.xml:36`) | `7b5ab44` (2026-08-28), branch `cassandra-5.0` | `src/java/org/apache/cassandra/` |
| PhxSql | 0.17.0 | este repositório | `crates/phxsql-store/`, `crates/phxsql-server/` |

Do lado deles, lidos por inteiro ou quase:
`db/Keyspace.java`, `db/Mutation.java`, `db/MutationVerbHandler.java`,
`db/CassandraKeyspaceWriteHandler.java`, `db/CassandraTableWriteHandler.java`,
`db/ColumnFamilyStore.java` (o `apply`), `db/ConsistencyLevel.java`,
`db/memtable/SkipListMemtable.java`, `db/memtable/Flushing.java`,
`db/commitlog/CommitLog.java`, `db/commitlog/CommitLogSegment.java`,
`db/commitlog/AbstractCommitLogService.java`,
`db/commitlog/BatchCommitLogService.java`,
`db/commitlog/PeriodicCommitLogService.java`,
`db/commitlog/GroupCommitLogService.java`,
`db/commitlog/MemoryMappedSegment.java`,
`db/commitlog/AbstractCommitLogSegmentManager.java`,
`service/StorageProxy.java`, `service/AbstractWriteResponseHandler.java`,
`service/WriteResponseHandler.java`,
`service/DatacenterWriteResponseHandler.java`,
`locator/ReplicaPlan.java`, `locator/ReplicaPlans.java`,
`io/sstable/format/SortedTableWriter.java`,
`io/sstable/format/big/BigTableReader.java`,
`index/internal/CassandraIndex.java`,
`cql3/statements/ModificationStatement.java`, `db/rows/Cells.java`,
`utils/memory/MemtableAllocator.java`, `schema/MemtableParams.java`,
`schema/TableParams.java`, `hints/HintsDispatchTrigger.java`,
`src/antlr/Parser.g` e `conf/cassandra.yaml`.

Do nosso lado, e nesta ordem: `docs/DESEMPENHO.md` **inteiro**,
`docs/CONCORRENTES.md`, `docs/REPLICACAO.md`, `docs/CLUSTER.md`,
`crates/phxsql-store/src/table.rs`, `reg.rs`, `ndx.rs`, `log.rs`,
`crates/phxsql-server/src/servidor.rs` (a `Janela` e o fecho de gravação) e
`config.rs`.

**Este documento não mediu nada de novo.** Todos os números do PhxSql que ele
usa saem de `docs/DESEMPENHO.md` e de `bancada/`, e estão citados como tais.
Nenhum número do Cassandra foi medido aqui — não há cluster montado —, e por
isso **nenhuma frase deste documento afirma um custo em microssegundos do lado
deles**. O que se afirma do lado deles é o que o código faz e o que ele não
faz, com a linha.

Os números nossos usados como base, todos medidos e registrados:

| | |
|---|---|
| Inserção local, 2 índices, esquema simples | **7,50 µs/linha** (`DESEMPENHO.md` §4.8) |
| Inserção local, 2 índices, esquema da bancada (`+Decimal(15,2)`, `+Date`) | **7,92 µs/linha** (§4.8) — o esquema custa ~5% a mais que o simples, não 2,2× |
| Bancada de 10 milhões | PhxSql **261,8 s** contra MySQL(R) **111,7 s** |
| Perfil da bancada | **95% CPU**, **2,4 GiB** escritos contra **32,0 GiB** do MySQL(R) |
| Replicação | master **34.048 linhas/s**, réplica **17.450 eventos/s**, três em paralelo (§4.5) |
| Durabilidade, os três modos | 1.289 → 18.264 → 24.858 → 26.301 linhas/s, **20,4×** (§3, item 4) |
| `.log` por evento | **0,67 µs** sem imagem, **1,61 µs** com (§2.2) |

> **Nota de manutenção, achada de passagem.** `docs/REPLICACAO.md` §10 e
> `docs/CLUSTER.md` §2.2 ainda dizem «4.357 eventos/s contra 28.914 linhas/s».
> O `DESEMPENHO.md` §4.5 refez essa medição e os números são **17.450 contra
> 34.048** — 4,08×. Dois documentos ficaram para trás. É exatamente o defeito
> que o `CLAUDE.md` registra («número digitado à mão envelhece calado»), e ele
> não se conserta aqui: fica anotado para a rodada que atualizar os dois.

---

## 2. O caminho de uma escrita, passo a passo

### 2.1 Do cliente até o coordenador

O cliente manda o CQL para **qualquer** nó. Esse nó vira o *coordenador* da
escrita; ele não precisa ser dono do dado.

`StorageProxy.mutate` (`service/StorageProxy.java:878`) é a porta. Para cada
mutação ele chama `performWrite` (`:894`, definido em `:1371`), que faz três
coisas antes de mandar qualquer byte:

```java
ReplicaPlan.ForWrite replicaPlan = ReplicaPlans.forWrite(keyspace, consistencyLevel, tk, ReplicaPlans.writeNormal);
...
AbstractWriteResponseHandler<IMutation> responseHandler = rs.getWriteResponseHandler(replicaPlan, callback, writeType, mutation.hintOnFailure(), requestTime);
performer.apply(mutation, replicaPlan, responseHandler, localDataCenter, requestTime);
```
`service/StorageProxy.java:1383-1392`

1. **Descobre quem são as réplicas** daquele token e monta o plano.
2. **Confere disponibilidade antes de enviar** —
   `assureSufficientLiveReplicasForWrite` (`locator/ReplicaPlans.java:134-136`,
   `:138`). Se não houver réplicas vivas suficientes para o nível pedido, ele
   lança `UnavailableException` **sem mandar nada a ninguém**. «Indisponível» e
   «falhou depois de tentar» são erros diferentes, e o cliente consegue
   distingui-los.
3. **Cria o contador de respostas** — o objeto que vai decidir quando o OK sai.

Depois, `sendToHintedReplicas` (`:1475`) despacha. O detalhe que muda tudo está
no seletor `writeNormal` (`locator/ReplicaPlans.java:513-521`):

```java
if (!any(liveAndDown.all(), Replica::isTransient))
    return liveAndDown.all();
```

**O coordenador manda para TODAS as réplicas, vivas e mortas, sempre** — não
para o número que o nível de consistência exige. O nível de consistência decide
só **para quantas respostas ele espera**. Essa distinção é a metade da história
do quórum, e volta no §4.

A serialização da mutação para a rede é feita **uma vez** e guardada
(`:1501`, `Mutation.serializer.prepareSerializedBuffer`), com o comentário
dizendo por quê: sem isso, cada destino recalcularia o mesmo buffer.

### 2.2 Dentro de cada réplica

A réplica recebe `MUTATION_REQ`. `MutationVerbHandler.applyMutation`
(`db/MutationVerbHandler.java:73-76`):

```java
message.payload.applyFuture().addCallback(o -> respond(message, respondToAddress), wto -> failed());
```

**A confirmação só sai no callback de sucesso do `applyFuture`.** Então o que
está dentro do `applyFuture` é exatamente o que o OK do quórum garante — nem
mais, nem menos. Vale seguir até o fundo.

`Mutation.applyFuture` (`db/Mutation.java:241-245`) chama
`Keyspace.applyFuture`, que chama `applyInternal`
(`db/Keyspace.java:474`, `:523`). O corpo útil é este:

```java
try (WriteContext ctx = getWriteHandler().beginWrite(mutation, makeDurable))
{
    for (PartitionUpdate upd : mutation.getPartitionUpdates())
    {
        ...
        cfs.getWriteHandler().write(upd, ctx, updateIndexes);
    }
    if (future != null) {
        future.trySuccess(null);
    }
```
`db/Keyspace.java:625-661`

Duas etapas, e só duas.

**Etapa 1 — o commit log.** `beginWrite`
(`db/CassandraKeyspaceWriteHandler.java:42-65`) abre um grupo de ordenação e,
se a keyspace for durável (padrão `durable_writes = true`,
`schema/KeyspaceParams.java:33`), chama `CommitLog.instance.add(mutation)`
(`:99`).

`CommitLog.add` (`db/commitlog/CommitLog.java:300-341`) é curto e é o coração:

```java
Mutation.serializer.serialize(mutation, dob, MessagingService.current_version);
int size = dob.getLength();
int totalSize = size + ENTRY_OVERHEAD_SIZE;
Allocation alloc = segmentManager.allocate(mutation, totalSize);

CRC32 checksum = new CRC32();
final ByteBuffer buffer = alloc.getBuffer();
try (BufferedDataOutputStreamPlus dos = new DataOutputBufferFixed(buffer))
{
    dos.writeInt(size);                                     // 4 bytes
    updateChecksumInt(checksum, size);
    buffer.putInt((int) checksum.getValue());               // 4 bytes
    dos.write(dob.unsafeGetBufferAndFlip());                // a linha
    updateChecksum(checksum, buffer, buffer.position() - size, size);
    buffer.putInt((int) checksum.getValue());               // 4 bytes
}
...
executor.finishWriteFor(alloc);
```
`db/commitlog/CommitLog.java:308-334`

Quatro coisas, e nenhuma delas é uma chamada de sistema:

- **serializa a linha uma vez**, num buffer de rascunho por thread
  (`DataOutputBuffer.scratchBuffer`);
- **reserva espaço** com `segmentManager.allocate`, que chega em
  `CommitLogSegment.allocate` (`:201-223`) e daí no laço de
  `compareAndSet` (`:240-255`) — sem trava, sem chamada de sistema, com recuo
  por `LockSupport.parkNanos(1)`;
- **copia os bytes para dentro do buffer**, que é um `MappedByteBuffer`:
  `channel.map(FileChannel.MapMode.READ_WRITE, 0, DatabaseDescriptor.getCommitLogSegmentSize())`
  (`db/commitlog/MemoryMappedSegment.java:62`). O arquivo do segmento tem
  **32 MiB pré-alocados** (`conf/cassandra.yaml:660`), e escrever nele é um
  `memcpy` para a página do sistema — **não há `write()` por mutação**;
- **calcula CRC-32 duas vezes**: sobre os 4 bytes do tamanho e sobre o corpo da
  mutação. `ENTRY_OVERHEAD_SIZE = 4 + 4 + 4`
  (`db/commitlog/CommitLogSegment.java:93`). **Doze bytes de sobrecarga por
  entrada, e nenhum CRC de página.**

E os segmentos são criados **por uma thread de fundo, antes de fazerem falta**:
`AllocatorRunnable` (`db/commitlog/AbstractCommitLogSegmentManager.java:161-200`)
fica preparando o próximo `availableSegment` enquanto ninguém precisa dele.

**Etapa 2 — a memtable.** `cfs.getWriteHandler().write(...)`
(`db/CassandraTableWriteHandler.java:34-39`) chega em
`ColumnFamilyStore.apply` (`db/ColumnFamilyStore.java:1468-1503`):

```java
Memtable mt = data.getMemtableFor(opGroup, commitLogPosition);
UpdateTransaction indexer = newUpdateTransaction(update, context, updateIndexes, mt);
long timeDelta = mt.put(update, indexer, opGroup);
```
`db/ColumnFamilyStore.java:1476-1478`

A memtable padrão é a `SkipListMemtable`
(`schema/MemtableParams.java:99`), e ela é isto:

```java
private final ConcurrentNavigableMap<PartitionPosition, AtomicBTreePartition> partitions = new ConcurrentSkipListMap<>();
```
`db/memtable/SkipListMemtable.java:85`

`put` (`:107-138`) faz `partitions.get`, um `putIfAbsent` se a partição for
nova, e `previous.addAll(update, cloner, opGroup, indexer)` (`:130`) — tudo em
RAM, tudo em ponteiro. **Nenhum arquivo é aberto, lido ou escrito.**

O índice secundário anda junto, e anda pelo mesmo caminho:
`CassandraIndex.insert` (`index/internal/CassandraIndex.java:519-531`) monta
uma `PartitionUpdate` e chama
`indexCfs.getWriteHandler().write(upd, ctx, false)` (`:530`) — ou seja, **outra
memtable**. O comentário do `put` diz que a posição de commit log dela é nula
de propósito (`db/memtable/SkipListMemtable.java:101-104`): o índice não vai
para o commit log, porque ele se reconstrói do dado.

Terminadas as duas etapas, `future.trySuccess(null)`
(`db/Keyspace.java:660`), o callback dispara e a réplica responde.

**Onde a memtable finalmente vira arquivo.** `Flushing.writeSortedContents`
(`db/memtable/Flushing.java:151-174`) percorre a memtable **na ordem dela** e
anexa partição por partição no `SSTableWriter`. E o escritor **cobra** essa
ordem:

```java
if (lastWrittenKey != null && lastWrittenKey.compareTo(key) >= 0)
    throw new RuntimeException(String.format("Last written key %s >= current key %s, writing into %s", lastWrittenKey, key, getFilename()));
```
`io/sstable/format/SortedTableWriter.java:175-176`

Guarde essa linha: ela volta no §5, e é a prova de que **o primeiro arquivo já
sai reordenado**, antes de qualquer compactação.

### 2.3 O que a escrita deles NÃO faz que a nossa faz

A nossa inserção (`table.rs:734-803`) faz cinco coisas. Uma a uma, contra o
fonte deles:

| Nós fazemos | Onde, no nosso | Cassandra | Onde, no deles |
|---|---|---|---|
| **Conferir unicidade antes de gravar** — uma descida na árvore por índice único | `table.rs:763-770` | **Não faz.** O INSERT do CQL é um *upsert*; a leitura antes da escrita é reservada a cinco operações de nicho (elemento de lista por índice, concatenação em CAS…) | `cql3/statements/ModificationStatement.java:408-417` |
| **Codificar a linha** (`montar_payload`) | `table.rs:772` | **Faz**, uma vez, para o commit log — e outra para a rede, essa guardada | `db/commitlog/CommitLog.java:308`; `service/StorageProxy.java:1501` |
| **Codificar uma chave por índice** (`todas_as_chaves`) | `table.rs:756` | **Não faz.** A chave da partição já vem do cliente; o índice secundário é outra `PartitionUpdate` na memtable dele | `index/internal/CassandraIndex.java:519-531` |
| **Gravar um slot de largura fixa com CRC no arquivo de dados** | `reg.rs:895-901` | **Não faz.** O arquivo de dados (SSTable) não é tocado pela escrita. A entrada do commit log tem tamanho variável e vai para um `mmap` | `db/commitlog/CommitLog.java:311-334`; `MemoryMappedSegment.java:62` |
| **Manter uma B+tree em arquivo por índice** | `table.rs:788`, `ndx.rs` | **Não faz.** *Skip list* em RAM para o dado; memtable para cada índice secundário | `db/memtable/SkipListMemtable.java:85,130` |
| **Gravar o evento no diário, com a imagem da linha** | `table.rs:801`, `log.rs:372` | O commit log **é** o diário, mas é descartado quando a memtable descarrega, e **não alimenta réplica nenhuma** | `db/commitlog/CommitLog.java:353` (`discardCompletedSegments`) |
| **`fsync` por operação** (no modo `por_operacao`) | `servidor.rs:222-244` | Opcional. O padrão é **a cada 10 s**, numa thread de fundo | `PeriodicCommitLogService.java:36-45`; `conf/cassandra.yaml:634-636` |

Três leituras saem dessa tabela, e a terceira é a que importa.

**Primeira: o CRC de página, nós já tiramos.** Desde o *write-back* (§4.8 do
`DESEMPENHO.md`), o CRC-32 da página do `.ndx` acontece no despejo e não por
linha (`ndx.rs:708`). Nesse ponto o desenho já é o deles.

**Segunda: a codificação da linha, eles pagam também.** É o achado mais recente
do `DESEMPENHO.md` (§4.8): depois do *write-back*, o custo dominante da nossa
inserção passou a ser `montar_payload` + `codificar_chave`. Duas colunas
(`Decimal(15,2)` e `Date`) custam ~0,4 µs a mais — 7,50 para 7,92 µs, e não os
16,61 que este documento citou antes de o §4.8 derrubar o número. O Cassandra
**não** evita esse trabalho: `Mutation.serializer.serialize`
(`CommitLog.java:308`) é exatamente o mesmo tipo de custo, e ele o paga uma vez
por mutação, mais uma vez para a rede. **Então o que hoje domina a nossa
inserção não é o que a arquitetura deles resolve.** Esse fato sozinho reordena
tudo o que se poderia querer copiar, e volta no §5 e no §6.

**Terceira, e é a linha que não se cruza: eles não decidem.** A nossa inserção
tem de responder «aceito ou recuso» **antes** de gravar, porque o `.reg` nunca
reaproveita slot — o comentário de `table.rs:758-762` diz isso com todas as
letras, e o `CONCORRENTES.md` §6.2 já o defendeu contra o MariaDB(R)/Aria. O
Cassandra nunca responde isso: ele grava e deixa o conflito para a leitura
resolver, por carimbo de hora:

```java
private static Cell<?> resolveRegular(Cell<?> left, Cell<?> right)
{
    long leftTimestamp = left.timestamp();
    long rightTimestamp = right.timestamp();
    if (leftTimestamp != rightTimestamp)
        return leftTimestamp > rightTimestamp ? left : right;
```
`db/rows/Cells.java:79-84`

E a exceção prova a regra: quando o cliente **precisa** de uma decisão, ele usa
`IF NOT EXISTS`, que cai no Paxos — e a documentação do próprio método diz que
«CAS is still intended to be used "when you really need it," not for all your
updates» (`service/StorageProxy.java:274-276`), com uma leitura obrigatória
entre as fases de *prepare* e *accept* (`:294-295`).

> **A conclusão honesta desta seção.** A escrita do Cassandra é rápida porque
> ela **é menos escrita**. Não há aqui um truque de engenharia que dê para
> importar sem importar junto a decisão de não recusar nada. E é por isso que a
> comparação certa para o PhxSql continua sendo o InnoDB e o Aria — que
> **decidem**, como nós —, e não o Cassandra. O que o Cassandra oferece de
> aproveitável não está no caminho de escrita: está no §4.

---

## 3. O commit log e a memtable: o que é o ganho de verdade

### 3.1 As três políticas de `fsync`

Quem decide quando os bytes saem da página do sistema para o disco é o
`AbstractCommitLogService`. Ele tem **uma thread**, e ela é a única que
sincroniza:

```java
SyncRunnable sync = new SyncRunnable(preciseTime);
executor = executorFactory().infiniteLoop(name, sync, SAFE, NON_DAEMON, SYNCHRONIZED);
```
`db/commitlog/AbstractCommitLogService.java:154-155`

O laço dela (`:172-216`) é o desenho inteiro em quinze linhas:

```java
long pollStarted = clock.now();
boolean flushToDisk = lastSyncedAt + syncIntervalNanos <= pollStarted || state != NORMAL || syncRequested;
synchronized (this)
{
    Thread.interrupted();
    if (flushToDisk)
    {
        syncRequested = false;
        commitLog.sync(true);
        lastSyncedAt = pollStarted;
        syncComplete.signalAll();
        syncCount++;
    }
    else
    {
        commitLog.sync(false);
    }
}
```
`db/commitlog/AbstractCommitLogService.java:177-197`

`commitLog.sync(true)` chega, para um segmento mapeado, em
`SyncUtil.force((MappedByteBuffer) buffer)`
(`db/commitlog/MemoryMappedSegment.java:93`) — um `msync` do mapeamento
inteiro, **uma vez, por todas as mutações que caíram nele**.

O que muda entre as três políticas é só o que a escrita faz enquanto isso:
`finishWriteFor` → `maybeWaitForSync` (`:281-287`).

| Política | `maybeWaitForSync` | O que o cliente perde | Onde |
|---|---|---|---|
| **`periodic`** (padrão) | Só espera se o último `fsync` for mais velho que **1,5 × o intervalo**; fora disso **não espera nada** | Até `commitlog_sync_period` de escritas confirmadas, numa queda **da máquina**. Padrão: **10 s** | `PeriodicCommitLogService.java:36-45`; `config/DatabaseDescriptor.java:3423-3429`; `conf/cassandra.yaml:634-636` |
| **`group`** | Espera o `fsync` da janela: `alloc.awaitDiskSync(...)` | A latência da janela (`commitlog_sync_group_window`, sugerido 1000 ms) — mas **não perde dado** | `GroupCommitLogService.java:34-41`; `conf/cassandra.yaml:629` |
| **`batch`** | Pede sincronização imediata (`requestExtraSync()`) e **espera** | Nada. Paga a latência do disco em cada escrita | `BatchCommitLogService.java:36-43` |

O `periodic` não espera **de verdade**: o teto de 1,5× é uma rede de segurança
contra disco entupido, não uma promessa de durabilidade.

```java
long expectedSyncTime = nanoTime() - blockWhenSyncLagsNanos;
if (lastSyncedAt < expectedSyncTime)
{
    pending.incrementAndGet();
    awaitSyncAt(expectedSyncTime, commitLog.metrics.waitingOnCommit.time());
    pending.decrementAndGet();
}
```
`db/commitlog/PeriodicCommitLogService.java:38-44`

Com o padrão de 10 s, `blockWhenSyncLagsNanos` é **15 s**
(`DatabaseDescriptor.java:3427`: `getCommitLogSyncPeriod() * 1.5`).

### 3.2 A nossa janela, lado a lado

A nossa `Janela` (`crates/phxsql-server/src/servidor.rs:194-256`) faz a mesma
pergunta e responde de forma **mais apertada**:

```rust
Durabilidade::PorLote => {
    let n = self.pendentes.fetch_add(1, Ordering::SeqCst) + 1;
    if n >= self.a_cada {
        self.fechar();
        return true;
    }
    ...
    if desde.elapsed().as_millis() as u64 >= self.ms {
```
`servidor.rs:226-241`

O mapa é este:

| PhxSql | Cassandra | A diferença que importa |
|---|---|---|
| `por_operacao` | `commitlog_sync: batch` | Iguais em promessa: durável quando a operação retorna |
| `por_lote` (padrão) | `group` **mais** um teto por contagem | O nosso fecha por **quantidade OU tempo**, o que vier primeiro (200 operações ou 200 ms, `config.rs:719-720`). O `periodic` deles fecha **só por tempo**: dez segundos a 100 mil escritas/s são um milhão de mutações sem `fsync` |
| `sistema` | **não existe** | O modo mais frouxo do Cassandra ainda sincroniza a cada 10 s. O nosso `sistema` nunca sincroniza (`servidor.rs:225`) e entrega o assunto ao sistema operacional |

**Nós já temos o truque do commit log, e ele já está medido**: 1.289 → 26.301
linhas/s, **20,4×** (`DESEMPENHO.md` §3, item 4). Não há nada a importar aqui —
a receita já foi aplicada, e por um caminho um pouco mais seguro que o deles.

Há **uma** diferença de desenho, e ela é pequena mas real. No Cassandra,
**nenhuma escrita de cliente executa o `fsync`**: a thread `SyncRunnable` o faz
sempre (`:154-155`, `:188`), e a escrita no máximo *espera*. No PhxSql, a
operação que fecha a janela por contagem **executa** o `fsync` ela mesma:

```rust
if self.tabela_reservada(p) || !self.janela.hora_de_gravar() {
    ...
    return Ok(());
}
// A janela fechou: esta vai agora, e as outras da janela junto.
t.sincronizar()?;
```
`servidor.rs:3530-3538`

Nós já temos a thread — `ligar_relogio_de_gravacao` (`servidor.rs:663-685`) —,
mas ela só fecha a janela **por tempo, quando ninguém grava**. Uma operação em
cada 200 paga o `fsync` inteiro de arquivos que crescem até 1,5 GiB. Isso vira
o item 3 do §6, e ele começa por **medir o `fsync`**, não por mexer no código.

### 3.3 O `BULKINSERT`, e por que ele é a peça certa

Durante uma reserva de carga a nossa janela **não fecha**: o comentário de
`servidor.rs:3527-3529` diz que o `BULKINSERT(false)` é quem sincroniza, uma
vez, no fim. Ou seja: **a carga inteira vira um `fsync` só**.

Esse é o `periodic` do Cassandra com um limite melhor. O `periodic` deles é
limitado pelo **relógio** — dez segundos, sejam eles mil escritas ou um milhão.
O nosso, durante uma carga, é limitado pela **carga**: a janela abre quando o
cliente reserva e fecha quando ele solta, e a soltura é garantida por dois
caminhos (a saída da conexão, `servidor.rs:3352-3366`, e a reserva vencida).
O cliente sabe exatamente qual é a janela dele, porque ele a abriu.

Medido: o `BULKINSERT` levou a carga em lote de 43.500 para 66.500 linhas/s,
**1,53×** (`DESEMPENHO.md` §6).

### 3.4 O que a memtable custa, e o fonte não esconde

A memtable é barata por linha e cara por sistema, e o preço aparece em
`MemtableAllocator.SubAllocator.allocate`
(`utils/memory/MemtableAllocator.java:169-196`):

```java
while (true)
{
    if (parent.tryAllocate(size))
    {
        acquired(size);
        return;
    }
    ...
    WaitQueue.Signal signal = parent.hasRoom().register(...);
    opGroup.notifyIfBlocking(signal);
    ...
    signal.awaitThrowUncheckedOnInterrupt();
}
```

Quando a memória da memtable acaba e a descarga não dá conta, **a escrita
bloqueia**. É a contrapressão do desenho: a rapidez por linha é comprada com um
teto de RAM e com uma fila que, cheia, para tudo. Um motor que grava no arquivo
a cada linha — como o nosso — não tem essa parede, e é honesto dizer que essa é
uma vantagem nossa, não só uma limitação.

---

## 4. O quórum de escrita

Este é o capítulo próprio, e ele responde quatro perguntas: **quem decide
quantos esperar**, **quem conta**, **o que acontece com quem não respondeu**, e
**o que o OK diz e o que ele não diz**.

### 4.1 Onde o coordenador decide quantas confirmações esperar

A conta mora em `ConsistencyLevel.blockFor`
(`db/ConsistencyLevel.java:133-170`):

```java
case QUORUM:
case SERIAL:
    return quorumFor(replicationStrategy);
case ALL:
    return replicationStrategy.getReplicationFactor().allReplicas;
case LOCAL_QUORUM:
case LOCAL_SERIAL:
    return localQuorumForOurDc(replicationStrategy);
```

e `quorumFor` é uma linha:

```java
public static int quorumFor(AbstractReplicationStrategy replicationStrategy)
{
    return (replicationStrategy.getReplicationFactor().allReplicas / 2) + 1;
}
```
`db/ConsistencyLevel.java:91-94`

Com RF = 3, `QUORUM` = 2. Com RF = 5, 3.

Para escrita há um ajuste que a leitura não tem —
`blockForWrite` (`:172-192`) soma as réplicas **pendentes**, as que estão
entrando no anel durante um *bootstrap*:

```java
case ONE: case TWO: case THREE:
case QUORUM: case EACH_QUORUM:
case SERIAL:
case ALL:
    blockFor += pending.size();
```
`db/ConsistencyLevel.java:185-189`

O comentário no `AbstractWriteResponseHandler.blockFor` explica por quê:
«During bootstrap, we have to include the pending endpoints or we may fail the
consistency level guarantees»
(`service/AbstractWriteResponseHandler.java:229-231`). O caminho é
`blockFor()` → `replicaPlan.writeQuorum()`
(`locator/ReplicaPlan.java:202`) → `blockForWrite`.

`LOCAL_QUORUM` conta só o datacenter local, e o filtro está num
`waitingFor` sobrescrito
(`service/DatacenterWriteResponseHandler.java:36,64-67`): resposta de outro
datacenter **não conta** para o limiar.

### 4.2 Quem envia, e para quantos

Já dito no §2.1, e vale repetir porque é a fonte de quase todo mal-entendido
sobre quórum: **o envio vai para todos**
(`locator/ReplicaPlans.java:517`, `writeNormal`), **a espera é pelo quórum**.

`sendToHintedReplicas` (`service/StorageProxy.java:1475-1583`) separa os
destinos em três:

- **eu mesmo** → `performLocally` (`:1569`, definido em `:1674-1707`), que
  executa a mutação no estágio `MUTATION` e chama `handler.onResponse(null)`
  (`:1683`) — a escrita local **conta como uma resposta**;
- **outro nó do meu datacenter** → `sendWriteWithCallback` (`:1575`), um por nó;
- **outro datacenter** → **uma** mensagem para **um** nó de lá, que a repassa
  aos vizinhos (`:1581`, `sendMessagesToNonlocalDC`). Uma travessia de WAN por
  datacenter, não uma por réplica.

E o destino **sabidamente morto** não recebe nada:

```java
else
{
    //Immediately mark the response as expired since the request will not be sent
    responseHandler.expired();
    if (shouldHint(destination))
    {
        if (endpointsToHint == null)
            endpointsToHint = new ArrayList<>();
        endpointsToHint.add(destination);
    }
}
```
`service/StorageProxy.java:1548-1559`

### 4.3 Quem conta, e quem espera

O contador é o mais simples possível
(`service/WriteResponseHandler.java:50,58-66`):

```java
responses = blockFor();
...
public void onResponse(Message<T> m)
{
    if (responsesUpdater.decrementAndGet(this) == 0)
        signal();
```

Começa em `blockFor()` e desce. Chegou a zero, `signal()` acorda quem espera
(`AbstractWriteResponseHandler.java:275-291`).

Quem espera é `mutate`, e ele espera **depois** de disparar todas as mutações:

```java
// wait for writes.  throws TimeoutException if necessary
for (AbstractWriteResponseHandler<IMutation> responseHandler : responseHandlers)
    responseHandler.get();
```
`service/StorageProxy.java:904-906`

`get()` (`AbstractWriteResponseHandler.java:112-138`) é um
`condition.await(timeoutNanos)` com o `write_request_timeout` —
**2000 ms** por padrão (`conf/cassandra.yaml:1332`). Não sinalizou:
`WriteTimeoutException`, com o número de confirmações que chegaram
(`:140-150`). Chegaram falhas demais para o quórum ainda ser possível:
`WriteFailureException` (`:136`).

E há um detalhe de honestidade no lançamento do timeout que vale citar
inteiro, porque ele diz muito sobre o que essas exceções significam:

```java
// It's pretty unlikely, but we can race between exiting await above and here, so
// that we could now have enough acks. In that case, we "lie" on the acks count to
// avoid sending confusing info to the user (see CASSANDRA-6491).
if (acks >= blockedFor)
    acks = blockedFor - 1;
```
`service/AbstractWriteResponseHandler.java:144-148`

**O timeout pode ser lançado sobre uma escrita que, no instante seguinte, tinha
quórum.** Eles preferem mentir no número a confundir o cliente.

Há ainda um acelerador: `maybeTryAdditionalReplicas`
(`AbstractWriteResponseHandler.java:341-369`), chamado em
`mutate` (`:901`). Se o quórum não fechou dentro de
`additionalWriteLatencyMicros`, ele contata as réplicas ainda não contatadas.
Com replicação normal `writeNormal` já contatou todo mundo, então esse caminho
só faz diferença com replicação transitória.

### 4.4 O que acontece com as réplicas que não responderam

Esta é a parte que mais se confunde, e o fonte é claro em três pontos.

**1. Elas receberam a mutação.** O coordenador não cancela nada quando o quórum
fecha. `writeNormal` mandou para todas, e todas vão aplicar quando chegarem
lá. «Não respondeu em 2 s» **não** é «não recebeu».

**2. A que estava morta ganha uma *hint*.** `submitHint`
(`service/StorageProxy.java:1564`, definido em `:2781-2820`) grava um arquivo
local no coordenador com a mutação inteira e o carimbo de criação:

```java
HintsService.instance.write(hostIds, Hint.create(mutation,  creationTime));
validTargets.forEach(HintsService.instance.metrics::incrCreatedHints);
// Notify the handler only for CL == ANY
if (responseHandler != null && responseHandler.replicaPlan.consistencyLevel() == ConsistencyLevel.ANY)
    responseHandler.onResponse(null);
```
`service/StorageProxy.java:2811-2815`

Três coisas nessas cinco linhas:

- a *hint* **não conta para o quórum**, exceto em `CL = ANY` — o único nível em
  que «escrevi um bilhete para mim mesmo» é resposta suficiente;
- ela é entregue depois por uma thread de fundo,
  `HintsDispatchTrigger.run` (`hints/HintsDispatchTrigger.java:52-71`), que
  varre os *stores* vivos e agenda o despacho;
- ela **tem prazo**. `shouldHint`
  (`service/StorageProxy.java:2442-2501`) recusa escrever a *hint* se o nó
  estiver fora há mais que `max_hint_window` — **3 horas** por padrão
  (`conf/cassandra.yaml:80`) — ou se as *hints* daquele nó já passarem de
  `max_hints_size_per_host` (`:2488-2497`).

**Passado o prazo, a divergência deixa de ter conserto automático rápido** e
depende do *read repair* ou de um `nodetool repair`. Esse é o motivo pelo qual
um nó fora por meio dia não volta consistente sozinho.

**3. A que falhou também ganha uma *hint*.** `onFailure`
(`AbstractWriteResponseHandler.java:293-311`):

```java
if (hintOnFailure != null && StorageProxy.shouldHint(replicaPlan.lookup(from)) && requestTime.shouldSendHints())
    StorageProxy.submitHint(hintOnFailure.get(), replicaPlan.lookup(from), null);
```

e para uma mutação comum `hintOnFailure` é sempre ela mesma
(`db/Mutation.java:158-161`).

**4. O que sobra é consertado na leitura, ou nunca.** O padrão de
`read_repair` é `BLOCKING` (`schema/TableParams.java:374`): uma leitura em
quórum que encontra divergência **conserta antes de responder**. E a varredura
completa é o `nodetool repair`, com árvore de Merkle
(`utils/MerkleTree.java:52-60`) — um subsistema inteiro que existe **porque**
não há como saber, sem comparar conteúdo, se duas réplicas têm o mesmo dado.
Guarde isso: volta no §5.

### 4.5 O que o OK garante

Concretamente, e nada além disso:

> **`blockFor` réplicas executaram `Keyspace.applyInternal` até o fim** — isto
> é, anexaram a mutação ao commit log delas
> (`CassandraKeyspaceWriteHandler.java:53`) e puseram a linha na memtable
> delas (`Keyspace.java:653`) — **e responderam**
> (`MutationVerbHandler.java:75`).

E disso sai uma garantia útil, que é pura aritmética do `blockFor`
(`ConsistencyLevel.java:91-94`) aplicado dos dois lados:

> **W + R > RF.** Com RF = 3, escrita em `QUORUM` (W = 2) e leitura em
> `QUORUM` (R = 2): 2 + 2 > 3, então os dois conjuntos **se cruzam em pelo
> menos uma réplica**. A leitura seguinte enxerga a escrita.

É isso. Tudo o mais que se costuma atribuir ao quórum não está lá.

### 4.6 O que o OK **não** diz

Sete coisas, em ordem de quanto surpreendem.

**1. Não diz que o dado está em disco.** Esta é a maior, e é a resposta direta
à pergunta do pedido. Com `commitlog_sync: periodic`
(`conf/cassandra.yaml:634`), a réplica responde depois de um `memcpy` para um
`MappedByteBuffer` (`MemoryMappedSegment.java:62`) e de uma inserção numa
*skip list* na heap. O `fsync` acontece na thread de fundo, a cada 10 s
(`AbstractCommitLogService.java:188`), e a escrita **não espera por ele**
(`PeriodicCommitLogService.java:38-44`).

Então:
- uma queda **do processo** (a JVM morre) **não perde nada**: os bytes estão na
  página do sistema, e o sistema os grava;
- uma queda **da máquina** (falta de energia) perde **até 10 s** de escritas
  confirmadas naquele nó;
- se as `blockFor` réplicas que confirmaram caírem dentro dessa janela — um
  rack, um circuito, um datacenter —, a escrita confirmada **some**.

Só `commitlog_sync: batch` (`BatchCommitLogService.java:36-43`) transforma o OK
em «está no disco de W máquinas», e ele cobra a latência do disco em cada
escrita.

> **Traduzindo para o nosso vocabulário:** o `QUORUM` padrão do Cassandra é o
> nosso `por_lote` com a janela em 10 s e sem teto por contagem — replicado em
> N máquinas. O que ele compra sobre o nosso não é durabilidade: é
> **independência de falha**. Dez segundos de janela em três máquinas
> diferentes é uma aposta melhor do que 200 ms numa só, mesmo sendo uma janela
> cinquenta vezes maior.

**2. Não diz que as outras N − W não receberam.** Quase certamente receberam:
`writeNormal` manda para todas (`ReplicaPlans.java:517`). O OK conta quantas
**confirmaram dentro do tempo**, e o que separa uma que confirmou de uma que
não confirmou costuma ser só a fila e a rede — não o dado.

**3. Não é atômico, e não há como desfazer.** Se o quórum não fecha,
`mutate` lança (`StorageProxy.java:906-931`) — e **o que já foi aplicado fica
aplicado**. Não existe caminho de desfazer em `Keyspace.applyInternal`. Uma
escrita que deu *timeout* pode ter chegado a zero, uma, duas ou todas as
réplicas.

**4. `WriteTimeoutException` significa «não sei», não «não gravou».** É o
corolário do item 3, e é a coisa mais importante que um cliente de Cassandra
precisa saber. A receita padrão é **repetir**, e repetir é seguro **só porque a
escrita é idempotente**: mesmo carimbo, mesma célula, mesmo resultado em
`resolveRegular` (`Cells.java:79-84`). Rodar duas vezes é igual a rodar uma.

**Isto não vale para nós, e a diferença é do formato.** A nossa `inserir`
(`table.rs:734`) tira um rowid novo a cada chamada (`reg.rs:904-906`), então
repetir depois de uma conexão cortada **cria uma linha duplicada**. E o
`inserir_lote` com `parar_no_erro` deixa gravadas as linhas anteriores à falha
— o comentário de `table.rs:841-850` diz isso e explica por quê. Vira item no
§6.

**5. Não é isolado.** Só a atualização de **uma partição** é atômica e isolada
localmente. Um lote que toca várias partições pode ser lido pela metade. O
`mutateAtomically` (`StorageProxy.java:1173`) compra atomicidade com um
*batchlog* gravado em outros nós **antes** da escrita e removido depois
(`:1163-1171`) — e mesmo ele garante «todas serão aplicadas eventualmente», não
«nenhuma será vista antes das outras».

**6. Não há ordem global, e o relógio decide.** O vencedor de um conflito é o
carimbo (`Cells.java:81-84`), e o carimbo vem do relógio do coordenador. Dois
coordenadores com relógios desencontrados fazem a escrita mais nova perder,
**em silêncio**.

> Vale registrar o contraste, porque a nossa resposta ao mesmo problema é
> melhor para um ERP: o `conferir_versao` (`table.rs:910`) usa um **contador
> por registro**, não um relógio, e ele **recusa** a segunda gravação em vez de
> descartá-la calado — mostrando os três valores para quem decide. É a lição
> que o `CLAUDE.md` registra sobre a janela de conflito de escrita, e ela vale
> contra o Cassandra tanto quanto valia contra o HFSQL(R).

**7. Não garante que a próxima leitura veja, em qualquer nível.** A garantia é
W + R > RF. Uma leitura em `ONE` depois de uma escrita em `QUORUM` pode cair
justamente na réplica que ficou de fora.

### 4.7 O custo: latência contra segurança

| Nível | Espera por | O que ganha | O que perde |
|---|---|---|---|
| `ANY` | 1 resposta **ou** uma *hint* gravada (`StorageProxy.java:2813-2815`) | Nunca falha enquanto o coordenador estiver vivo | Não garante que **nenhuma** réplica tenha o dado |
| `ONE` / `LOCAL_ONE` | 1 | A menor latência | Leitura em `ONE` pode não ver |
| `QUORUM` | ⌊RF/2⌋+1 | W + R > RF; sobrevive à morte de ⌊RF/2⌋ réplicas | Latência da **W-ésima mais rápida** |
| `LOCAL_QUORUM` | quórum do DC local | Não paga WAN | Não garante nada fora do DC |
| `ALL` | RF | Toda leitura, em qualquer nível, vê | Latência da **mais lenta**; uma réplica morta = `UnavailableException` |

Três leituras que o fonte sustenta:

1. **O quórum compra latência, não largura de banda.** A mensagem vai para
   todas as réplicas de qualquer jeito (`ReplicaPlans.java:517`). Entre
   `QUORUM` e `ALL` a rede é a mesma; o que muda é de quem você espera.
2. **A latência do quórum é a da W-ésima mais rápida**, e é por isso que ele é
   robusto: um nó com disco ruim ou GC longo simplesmente não aparece no
   número. Em `ALL`, ele **é** o número.
3. **A conferência de disponibilidade é anterior ao envio**
   (`ReplicaPlans.java:134-136`), então `ALL` com um nó morto falha **rápido e
   sem escrever nada** — o que é uma segurança, não um defeito.

---

## 5. A §5 do nosso `DESEMPENHO.md` continua de pé?

A §5 («Por que LSM não cabe dentro do motor atual») lista quatro
incompatibilidades. Conferi as quatro contra o código real do Cassandra.

### 5.1 Os quatro pontos, conferidos

**Ponto 1 — «A ordem de digitação. Compactação reordena.»
CONFIRMADO, e a §5 é branda demais.**

A reordenação **não espera pela compactação**. A memtable já é um mapa ordenado
por token (`SkipListMemtable.java:85`), o `Flushing.writeSortedContents`
(`Flushing.java:157-174`) percorre nessa ordem, e o escritor de SSTable
**recusa** receber fora de ordem:

```java
if (lastWrittenKey != null && lastWrittenKey.compareTo(key) >= 0)
    throw new RuntimeException(...);
```
`io/sstable/format/SortedTableWriter.java:175-176`

**O primeiro arquivo já sai reordenado.** A correção importa porque a §5, como
está escrita, deixa aberta a ideia de «uma LSM sem compactação preservaria a
ordem». Não preservaria: a ordem se perde na primeira descarga, e ela é
*imposta* pelo escritor.

**Ponto 2 — «O endereço por conta: `offset = data_offset + (rowid−1) ×
slot_size`.» CONFIRMADO, e o Cassandra mostra o preço de não ter.**

Não há rowid em lugar nenhum do Cassandra. O acesso é sempre por chave:

```java
public final RowIndexEntry getRowIndexEntry(PartitionPosition key, Operator op)
```
`io/sstable/format/big/BigTableReader.java:202`

Para chegar a uma linha: chave da partição → token → réplicas → em cada SSTable
candidata, filtro de Bloom → índice de partição → índice de linha → dado. É a
troca inteira: eles compram distribuição e pagam com uma cadeia de índices onde
nós temos uma multiplicação.

**Ponto 3 — «A paginação por cursor e o salto por bissecção.» CONFIRMADO, e
há prova direta na gramática.**

O `SELECT` do CQL tem `LIMIT` e `PER PARTITION LIMIT`, e **não tem `OFFSET`**:

```
( K_PER K_PARTITION K_LIMIT rows=intValue { perPartitionLimit = rows; } )?
( K_LIMIT rows=intValue { limit = rows; } )?
```
`src/antlr/Parser.g:283-284`

A paginação é por **cursor de chave** (o *paging state* carrega a última chave
vista), nunca por posição. «Me dê a página 4.000» não existe em CQL — é
exatamente a capacidade que a §5 diz que perderíamos, e o Cassandra não a tem
mesmo. Nossa medida do que está em jogo: **164 µs contra 246 ms, 1.500×**,
para uma página no meio de 800 mil linhas (`DESEMPENHO.md` §6).

**Ponto 4 — «A garantia da replicação.» CONFIRMADO, e o Cassandra mostra o
tamanho do subsistema que substitui.**

Sem identidade derivável, a única forma de saber se duas cópias são iguais é
**comparar conteúdo**. É o que eles fazem, em dois níveis: *read repair*
bloqueante por padrão (`schema/TableParams.java:374`) e reparo por árvore de
Merkle (`utils/MerkleTree.java:52-60`). Nós conferimos com uma comparação de
inteiros: a réplica confere se o rowid que ela **gerou** bate com o do evento, e
para na hora se não bater (`REPLICACAO.md` §5).

**Uma correção de redação, e ela é do nosso lado.** A §5 diz que a réplica
«chega aos mesmos rowids **sem que ninguém os transmita**». Não é bem isso: o
rowid **viaja** no cabeçalho do evento (offset 12, `REPLICACAO.md` §3). O que é
verdade — e é mais forte — é que a réplica **deriva o mesmo rowid sozinha**, e
o campo transmitido serve de **conferência**. Sem a derivação independente, o
campo não provaria nada. Vale corrigir a frase quando a §5 for tocada.

### 5.2 O que a §5 deixou passar

Três coisas, e a primeira é a maior.

**(a) A §5 é sobre o `.reg`, e ela não diz isso — então parece fechar uma porta
que continua aberta no `.ndx`.**

Releia as quatro incompatibilidades com o `.ndx` em mente:

| Restrição da §5 | Vale para o `.ndx`? |
|---|---|
| A ordem de digitação | **Não.** O `.ndx` é ordenado por chave por definição |
| O endereço por conta | **Não.** O `.ndx` *guarda* rowids; ele não é endereçado por eles |
| Paginação por cursor e bissecção | **Não.** Elas saem da ordem física do `.reg` |
| A garantia da replicação | **Não.** A réplica reconstrói o índice dela; o `.log` não carrega chave |

**Nenhuma das quatro alcança o `.ndx`.** E o que o Cassandra faz com a memtable
— acumular ordenado em RAM e escrever a série ordenada de uma vez — é
exatamente a forma que o §4.4 do `DESEMPENHO.md` já identificou como a única
que faria o adiamento valer:

> «O que o faria valer é outra coisa, e maior: reconstruir só sobre as linhas
> novas e **fundir** a série ordenada na árvore existente, em vez de refazê-la.
> Aí o custo passaria a depender de M, e não de N+M.»

E metade da peça já existe: `NdxFile::construir_em_lote` (§4.3) monta um milhão
de chaves em 0,31 s contra 7,72 s uma a uma, **23× a 25×**. Falta a fusão.

**Além disso, o custo de formato que o §4.4 temia já foi pago.** O §4.4 dizia
que adiar «exigiria marcar índice suspenso no formato do `.ndx`». Essa marca
**existe desde o write-back**: o campo `sujo` vai ao cabeçalho **antes** da
primeira página suja, e quem abre com ela levantada recusa responder e manda
reconstruir (`ndx.rs:290-308`, e a recusa em `:848-861`). O §4.4 foi
escrito antes do §4.8; o argumento de custo dele **envelheceu**.

**(b) O argumento mais forte contra a LSM hoje não está na §5 — está na §4.8.**

A §5 argumenta que a LSM **quebra** quatro coisas. Correto, e continua
correto. Mas há um argumento anterior, que dispensa a discussão inteira:

> **A LSM não ataca o custo que hoje domina a nossa inserção.**

O §4.8 mediu: depois do *write-back*, `.reg` + `.log` viraram **60,8%** do
tempo e os dois índices **29,4%**; o mesmo código com o esquema da bancada sai
de 7,50 para **7,92 µs** — as duas colunas custam ~5%, não 2,2×. (Este
documento chegou a citar 16,61 µs em quatro lugares; era um **binário velho**,
e o §4.8 conta a derrubada.) O custo dominante é a **codificação da linha**.

E o Cassandra **paga esse custo igual**: `Mutation.serializer.serialize`
(`CommitLog.java:308`), mais a serialização para a rede (`StorageProxy.java:1501`).
Uma LSM muda **onde os bytes caem**, não **como eles são produzidos**.

Então a §5 ganha um parágrafo novo, e é o mais curto e o mais forte dela: *a
razão de não fazer LSM hoje não é só que ela quebra quatro coisas — é que ela
não toca no que custa.*

**(c) A saída «dois motores» é mais normal do que a §5 dá a entender.**

A §5 diz que um `PHX-LSM` ao lado do motor atual «é um projeto próprio, não um
ajuste». Verdade. Mas vale registrar que **o Cassandra faz exatamente isso, e
por tabela**: a memtable é um parâmetro de esquema
(`schema/MemtableParams.java:99,104`), com implementações trocáveis
(`SkipListMemtable`, `ShardedSkipListMemtable`, `TrieMemtable`), e uma tabela
pode até **dispensar o commit log**
(`CassandraKeyspaceWriteHandler.java:74`, `writesShouldSkipCommitLog()`).
O desenho «motor escolhido por tabela» não é exótico; é como um banco de dados
de produção resolve esse mesmo impasse. Isso não muda o veredito da §5 — muda a
confiança com que ela pode ser escrita.

### 5.3 Veredito

**A §5 continua de pé, nos quatro pontos, e com evidência mais forte do que a
que ela tinha.** Ela precisa de três emendas:

1. o ponto 1 é mais duro do que ela diz — a reordenação é na **descarga**, não
   na compactação (`SortedTableWriter.java:175-176`);
2. o ponto 4 tem uma frase imprecisa — o rowid **é** transmitido; o que é
   derivado é a **conferência**;
3. falta o argumento decisivo, que é o da §4.8: **a LSM não atacaria o custo
   que hoje domina**.

E ela precisa de um limite explícito: **ela fala do `.reg`.** O `.ndx` não está
coberto por nenhuma das quatro restrições, e é lá que a única ideia
aproveitável do Cassandra pode caber — se a medição do §6, item 1, mostrar que
sobrou o que ganhar.

---

## 6. O que cabe aqui, em ordem de valor ÷ custo

Cinco itens. Cada um traz **a prova que o confirmaria**: qual medição, em que
exemplo, com que número esperado — e, quando cabe, o número que o **derruba**.

### 6.1 Registrar no source a posição confirmada de cada réplica

**Custo: baixo. Valor: alto. É a única peça do quórum que cabe no nosso
desenho, e ela já está no protocolo — só ninguém a guarda.**

Um quórum de escrita exige empurrar e esperar. A nossa replicação é **puxada**,
e tem de continuar sendo (§7). Mas o *ack* que o quórum do Cassandra colhe das
respostas, nós já recebemos de graça na pergunta: a réplica manda
`{"op":"replicar", ..., "desde":N}` (`REPLICACAO.md` §6), e **`N` é contado do
`.log` da própria réplica** — ela não lembra, ela conta o que aplicou
(`REPLICACAO.md` §9, «A posição é o diário da própria réplica»).

> **Então `desde: N` é uma confirmação verdadeira de que aquela réplica aplicou
> N eventos.** O source vê essa confirmação a cada lote e a joga fora.

O que fazer: guardar, por `(database, tabela, réplica)`, o maior `desde` visto —
no **servidor**, ao lado das marcas de posição que a §4.5 já pôs ali
(`marcas_do_diario`, `servidor.rs:337`, com teto de oito por tabela em
`MARCAS_POR_TABELA`, `:263`) e pela mesma razão que aquelas: a tabela é aberta e
fechada a cada pedido, então o estado sobre *quem pergunta* não pode morar
nela. E responder a uma pergunta nova: «o evento N já chegou a K réplicas?»

O que isso compra:

- a conferência que o `CLUSTER.md` §2.1 item 3 diz faltar para promover uma
  réplica («é seguro **quando as réplicas estão na mesma posição**, e exige
  conferência quando não estão») deixa de ser manual e vira uma consulta;
- o atraso por réplica, que hoje só se mede rodando a bancada;
- um **quórum a posteriori**: quem gravou pode perguntar depois se o evento
  alcançou K réplicas. Não é o quórum do Cassandra — não bloqueia, não garante
  nada no instante do OK —, mas responde a mesma pergunta com um atraso
  conhecido, e sem o master alcançar ninguém.

Entra **pedida, não imposta**: campo opcional em `posicao`, ou operação nova.
Cliente que não pergunta continua exatamente como está — é a regra do
`CLAUDE.md` sobre guarda nova, e o teste que a trava é o do comportamento
velho.

**A prova que confirmaria.** `python3 bancada/replicacao/medir.py 100000`, com
duas asserções novas:

1. **Correção.** No fim da carga, a posição confirmada que o source guarda para
   cada réplica tem de bater com a `posicao` que a própria réplica informa,
   **dentro de um lote** — 500 eventos, que é o `max` do `replicar`. Diferença
   maior que 500 significa que a confirmação está mentindo, e o item morre.
2. **Custo.** A taxa do master não pode se mexer: **34.048 linhas/s**, com a
   variação entre corridas que a bancada já tem. Queda acima de 1% significa
   que guardar um inteiro está custando mais do que deveria — e aí o lugar está
   errado (provavelmente dentro da trava global, e não do lado do pedido).

### 6.2 *Long-poll* no source, com `Condvar`

**Custo: médio. Valor: alto — e ele é o que faz o item 6.1 valer alguma
coisa.**

Já está na lista do `REPLICACAO.md` §9 (☐). O que a leitura do Cassandra
acrescenta é o **desenho da primitiva**, e ele é pequeno:
`AbstractCommitLogService` mantém uma `WaitQueue syncComplete` (`:69`); quem
precisa esperar por uma posição chama `awaitSyncAt` (`:317-328`), que registra
um sinal, **confere a condição de novo** e só então dorme; quem avança a
posição chama `syncComplete.signalAll()` (`:190`). O equivalente em `std` é
`std::sync::Condvar` — sem crate nenhuma.

Por que ele é pré-requisito do 6.1: uma posição confirmada com 2 s de idade não
serve de *ack*. Hoje o atraso de 1,3 a 2,1 s (`REPLICACAO.md` §10) é dominado
pelo `reconectar_em` de 2 s, **não** pela taxa de aplicação — a réplica aplica
17.450 eventos/s (§4.5) e o master escreve 34.048 linhas/s, então as três
juntas dão conta.

**A armadilha, e é a que mataria o item:** a espera **não pode** acontecer com
a trava global de dados na mão. O source segura essa trava em toda operação; um
`replicar` que dorme dentro dela congela o master. O sinal tem de vir de quem
grava o evento (`table.rs:801` → `log.rs:372`), e a espera tem de ficar fora da
trava.

**A prova que confirmaria.** `python3 bancada/replicacao/medir.py 100000`:

1. **O que tem de melhorar.** «Atraso de uma escrita até as três»: hoje
   **1,3 a 2,1 s**. Esperado com *long-poll* num master parado: **abaixo de
   300 ms** — o atraso passa a ser a rede mais a aplicação, e não o relógio.
2. **O controle que NÃO pode se mexer**, e é o que denuncia a armadilha: master
   **34.048 linhas/s** e `pendentes_de_gravacao` estável. Se a taxa do master
   cair com três réplicas penduradas, a espera está segurando a trava e o item
   está errado como escrito.

### 6.3 Medir o `fsync` antes de tirá-lo do caminho da operação

**Custo: baixo (a medição). Valor: desconhecido — e é por isso que o item
começa medindo.**

O Cassandra **nunca** faz uma escrita de cliente executar o `fsync`: a thread
`SyncRunnable` o faz sempre (`AbstractCommitLogService.java:154-155,188`), e a
escrita no máximo espera, e só se o sincronizador estiver 1,5 intervalos atrás
(`PeriodicCommitLogService.java:38-44`).

Nós fazemos diferente em um ponto: a operação que fecha a janela **por
contagem** executa o `t.sincronizar()` ela mesma (`servidor.rs:3538`). Uma
operação em cada 200 (`config.rs:719`) paga o `fsync` inteiro de arquivos que
crescem até 1,5 GiB. A thread para fazer isso **já existe**
(`ligar_relogio_de_gravacao`, `servidor.rs:663-685`) — hoje ela só fecha a
janela por tempo, quando ninguém grava.

E há uma pista solta esperando por isso: o §4.6 do `DESEMPENHO.md` deixou
**~283 ms por lote de 50.000** sem explicação na bancada, nomeando o
`sincronizar()` como o principal suspeito e dizendo que ele não pode ser a
diferença toda.

**A prova, e ela vem ANTES de qualquer conserto.** Um exemplo novo,
`--example custo-do-fsync`, na forma do `abrir-cresce` do §4.6: abre uma tabela
com 1, 3 e 6 milhões de linhas e cronometra **só** `Table::sincronizar`, três
corridas cada.

- **Esperado:** o custo cresce com o tamanho do arquivo, como o `abrir`
  crescia.
- **O número que decide:** custo medido ÷ 200 operações, contra os **23,0
  µs/linha** que a bancada mede (§4.6). **Abaixo de 2% (0,46 µs), o item morre
  ali** e a medição é a entrega — mais um diagnóstico plausível derrubado, que
  é o que este projeto faz melhor.
- **Se passar de 2%:** mover o `fsync` da contagem para a thread. A promessa
  documentada de `por_lote` **não muda** — ela já é «durável dentro da janela»;
  o que se perde é um reforço acidental que uma operação em 200 ganhava por
  sorte.

**A metade que NÃO se move, e é importante dizer:** o `fsync` do
`soltar_cargas_da_ligacao` (`servidor.rs:3352-3366`) é uma **promessa** a quem
chamou `BULKINSERT(false)`. Empurrá-lo para uma thread faria a carga responder
«gravei» antes de o disco saber. Isso é a piora silenciosa que o `CLAUDE.md`
recusa, e ela fica de fora explicitamente.

### 6.4 Documentar a retomada segura de uma carga interrompida

**Custo: um parágrafo e um teste. Valor: alto para quem importa dado.**

O §4.6 acima e o §4.4 do quórum encostam no mesmo buraco: o cliente do
Cassandra repete uma escrita que deu *timeout* porque a escrita é **idempotente**
(`Cells.java:79-84`). A nossa não é: `inserir` tira rowid novo
(`reg.rs:904-906`), e `inserir_lote` com `parar_no_erro` deixa gravadas as
linhas anteriores à falha (`table.rs:841-850`). Uma conexão cortada no meio de
uma carga de um milhão deixa o cliente sem saber onde parar.

A solução do Cassandra (carimbo do cliente decidindo o vencedor) **não cabe** —
ela troca uma recusa por uma perda silenciosa, e o §4.6 já argumentou contra.
A que cabe já está construída: **uma coluna de chave do cliente sob índice
único**. A conferência que fazemos antes de qualquer gravação
(`table.rs:763-770`) recusa a duplicata, e a carga vira segura para repetir do
começo.

Isto não é código novo — é **receita**, e ela precisa estar escrita, porque
ninguém a descobre lendo `inserir_lote`.

**A prova que confirmaria.** Um teste de integração: inserir um lote de 1.000
linhas com uma coluna `id_externo` sob índice único; interromper na linha 700
(erro provocado); repetir o **lote inteiro** do começo. Asserções:

- a tabela tem **exatamente 1.000** linhas ao fim (se tiver 1.700, a receita
  está errada);
- os rowids 1..699 mantêm os valores originais — a repetição não regravou o que
  já estava lá;
- e as 300 recusadas da segunda passada aparecem em `Lote::recusadas` com o
  número da linha, que é o contrato que o `inserir_lote` já promete.

### 6.5 Escrever, no MANUAL, o que cada modo de durabilidade promete

**Custo: documentação. Valor: o do `cache_paginas` — evitar um campo que
promete o que não entrega.**

Os números já existem (`DESEMPENHO.md` §3, item 4: 1.289 → 18.264 → 24.858 →
26.301 linhas/s, 20,4×). O que falta é a frase que diz o que cada um compra, e
agora ela tem uma referência externa para se apoiar:

| `recursos.durabilidade` | Equivale a | O que o cliente perde numa queda **da máquina** |
|---|---|---|
| `por_operacao` | `commitlog_sync: batch` (`BatchCommitLogService.java:36-43`) | Nada. Paga a latência do disco em cada operação |
| `por_lote` (padrão) | `group` (`GroupCommitLogService.java:34-41`) **mais** um teto por contagem que o Cassandra não tem | Até `lote_operacoes` operações **ou** `lote_milissegundos`, o que vier primeiro (200 e 200, `config.rs:719-720`) |
| `sistema` | **nada no Cassandra** — o modo mais frouxo deles ainda sincroniza a cada 10 s (`conf/cassandra.yaml:634-636`) | O que o sistema operacional ainda não tiver descarregado. Sem teto nosso |

E a distinção que os três modos fazem é a queda da **máquina**, não a do
processo — e essa frase precisa de uma ressalva por arquivo, porque ela não é
uniforme desde o *write-back*:

- no `.reg` e no `.log`, uma queda **do processo** não perde nada em modo
  nenhum: o `write` já entregou os bytes ao núcleo, e é o `fsync` que os três
  modos disputam;
- no `.ndx`, **não**. Desde o *write-back* (§4.8) a página fica suja em RAM
  (`ndx.rs:708`), e uma queda do processo deixa a árvore atrás do `.reg`. O que
  fecha esse buraco é a marca `sujo` no cabeçalho, que vai ao arquivo **antes**
  da primeira página suja e faz quem reabrir recusar responder e mandar
  reconstruir (`ndx.rs:290-308`, `:848-861`).

Nenhuma das duas está escrita em lugar nenhum hoje, e são as primeiras coisas
que alguém pergunta.

> **Segunda nota de manutenção.** O comentário de `ndx.rs:148-155` («Por que a
> gravação continua atravessando») descreve o cache **de antes** do
> *write-back*: ele diz que segurar página suja em RAM «trocaria uma garantia
> por desempenho **sem avisar**» — e é exatamente o que o código passou a fazer,
> com o aviso, na marca `sujo`. O comentário está certo sobre o risco e errado
> sobre o presente. Mesmo defeito dos números velhos do `REPLICACAO.md`, e o
> mesmo conserto: quando a rodada tocar o arquivo, refazer a frase.

**A prova:** não precisa de uma nova — `cargo run --release --example
custo-do-sync` já refaz os três números, e o `CLAUDE.md` já exige que números
visíveis saiam de um gerador. O que este item pede é que a **frase** ao lado
dos números esteja certa.

---

## 7. O que não cabe, e por quê

### 7.1 O quórum de escrita síncrono

**Fora, e a razão é de topologia, não de gosto.**

O quórum exige o coordenador **empurrar** (`StorageProxy.java:1475`) e
**esperar** (`AbstractWriteResponseHandler.java:112`). A nossa replicação é
puxada, e é puxada **por causa do firewall**: o `REPLICACAO.md` §7 mostra o
desenho — o Source aceita entrada na 5000 só do IP da Réplica e **não alcança
ninguém**; o túnel Curitiba ↔ Bélgica existe justamente para que a porta 5000
nunca saia dele.

Para copiar o quórum precisaríamos, no mínimo:

1. **conexões de saída do master para cada réplica** — uma regra de firewall
   que hoje não existe e que o desenho recusa de propósito;
2. **um detector de falha** — o Cassandra usa o Gossiper
   (`StorageProxy.java:2462`, `Gossiper.instance.getEndpointDowntime`) para
   saber quem está vivo **antes** de enviar;
3. **uma loja de *hints*** com prazo, teto de tamanho e despacho de fundo
   (`hints/`, `StorageProxy.java:2442-2501`, `HintsDispatchTrigger.java`);
4. **uma conferência de disponibilidade** antes de escrever
   (`ReplicaPlans.java:134-136`), senão o quórum vira uma espera de dois
   segundos por réplica morta;
5. **e um conserto para a divergência que sobra** — que no Cassandra é o *read
   repair* bloqueante (`TableParams.java:374`) mais a árvore de Merkle
   (`MerkleTree.java`).

**Custo: um subsistema.** **Valor: uma garantia que o nosso arranjo não pede** —
as réplicas do PhxSql são réplicas de **leitura**, em `somente_leitura`
(`REPLICACAO.md` §5), servindo relatório. Elas não são um quórum de iguais, e
não há escrita para distribuir entre elas: um master só, e a escrita dele é o
teto (`CLUSTER.md` §2.2).

E há o número que fecha o argumento, com a honestidade que ele exige: a nossa
inserção local custa **7,50 µs** (esquema simples) ou **7,92 µs** (esquema da
bancada), medidos. Um quórum acrescentaria pelo menos uma ida e volta de rede
até a segunda réplica mais rápida. **Quanto isso é, aqui, não está medido — e
este documento não vai citar um número de rede que não mediu.** A ordem de
grandeza é evidente (rede em dezenas ou centenas de microssegundos contra
inserção em unidades), mas a decisão não depende dela: depende do item 1 da
lista acima, que é uma regra de firewall que o projeto recusou por escrito.

Se algum dia a pergunta voltar, a medição que a decidiria já tem bancada
montada: `bancada/replicacao/montar.py` sobe quatro servidores; cronometrar um
`aplicar` de **um** evento, ida e volta, do master até a réplica, dá o piso do
que um quórum custaria por linha. **Medir isso antes de discutir é a regra do
projeto**, e ela vale para este item como valeu para os seis diagnósticos que o
`DESEMPENHO.md` derrubou.

### 7.2 LSM, memtable de escrita e segmentos imutáveis para o `.reg`

**Fora**, pelos quatro motivos da §5 — **confirmados** aqui, com o ponto 1 mais
duro do que estava escrito (`SortedTableWriter.java:175-176`) — e pelo motivo
novo do §5.2(b): **a LSM não ataca o custo que hoje domina**, que é a
codificação da linha (§4.8), e que o próprio Cassandra paga
(`CommitLog.java:308`).

### 7.3 Escrever sem conferir unicidade

**Fora, e é a linha que não se cruza.** O `.reg` nunca reaproveita slot
(`table.rs:758-762`), então aceitar primeiro e resolver depois deixa um buraco
permanente por linha recusada. O Cassandra pode fazer isso porque não tem slot,
não tem rowid e não promete recusar nada — o conflito vira trabalho da leitura
(`Cells.java:79-84`) e da compactação.

O `CONCORRENTES.md` §6.2 já havia defendido a mesma linha contra o
MariaDB(R)/Aria. Três motores, o mesmo destino, e agora com o terceiro
mostrando qual é o preço de não pagar: **o INSERT deixa de saber dizer não.**

### 7.4 O carimbo do cliente decidindo o vencedor

**Fora.** É `Cells.resolveRegular` (`db/rows/Cells.java:79-84`): o carimbo
maior ganha, e o menor **desaparece sem aviso**. O nosso `conferir_versao`
(`table.rs:910`) resolve o mesmo problema com um contador por registro — sem
relógio, e **recusando** em vez de descartando, com os três valores na tela
para quem decide. Adotar o deles seria trocar uma recusa por uma perda
silenciosa, e é exatamente o estrago que o `CLAUDE.md` registra sobre o merge de
conflito.

### 7.5 `mmap` no `.log`

**Fora, por duas razões independentes.**

A primeira é de regra: `std` não tem `mmap`. Chegar lá exige `libc` ou
chamada de sistema crua, e **zero dependências externas** é o que fez a
compilação cruzada para Windows funcionar de primeira.

A segunda é de tamanho, e é medida: o `.log` custa **0,67 µs por evento** sem
imagem (`DESEMPENHO.md` §2.2), de **7,50 µs** por linha (§4.8) — **8,9%**, e o
`memcpy` continuaria acontecendo de qualquer jeito. O teto do item é menor que
os 8,9%, e o §3 já mediu que uma chamada de sistema aqui custa 0,10 µs.

### 7.6 O commit log como fonte da replicação

**Não é o que eles fazem, e vale registrar para ninguém procurar.** O commit
log do Cassandra é **descartado** quando a memtable descarrega
(`CommitLog.discardCompletedSegments`, `CommitLog.java:353`); ele não alimenta
réplica nenhuma. A replicação deles acontece **antes**, no coordenador, que
manda a mutação para todas as réplicas (`StorageProxy.java:1475`).

O nosso `.log` faz as duas coisas: é durabilidade **e** é a fonte da
replicação, com a imagem da linha dentro (`REPLICACAO.md` §3). Isso custa —
1,61 µs por evento com imagem contra 0,67 sem (§2.2) — e compra uma
propriedade que eles não têm: **a réplica pode alcançar sozinha, de qualquer
posição, sem que o master guarde estado sobre ela**. O §2.2 já registrou por
que o evento não pode ficar em RAM («índice perdido se reconstrói; evento
perdido não»), e a leitura do Cassandra não muda esse veredito — ela mostra o
outro desenho, que paga o mesmo preço num lugar diferente.

---

## Nota sobre os nomes

Apache Cassandra é marca da Apache Software Foundation. MySQL(R) e InnoDB são
marcas da Oracle Corporation. MariaDB(R) e Aria são marcas da MariaDB
Corporation Ab. Este documento lê os fontes públicos do Apache Cassandra 5.0
sob a licença Apache 2.0 para entender decisões de projeto; **nenhum código foi
copiado para o PhxSql**, e as propostas do §6 são reimplementações de ideias
documentadas, escritas do zero e só com a `std` do Rust.

---

## Como refazer o que este documento propõe medir

```bash
# a base, para comparar (ja existem)
cargo run --release --example onde-doi -- 200000
cargo run --release --example custo-do-sync
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000

# o exemplo novo do 6.3, que decide o item antes de ele virar codigo
cargo run --release --example custo-do-fsync -- 1000000 3000000 6000000
```
