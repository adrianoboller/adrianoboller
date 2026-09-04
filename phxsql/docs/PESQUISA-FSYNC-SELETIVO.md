# Pesquisa: o que os outros sincronizam no fecho, medido contra os nossos 7 `fsync`

Documento do papel **J (pesquisador)**, e ele obedece à regra que dá sentido ao
papel: *receita de fora se mede contra o nosso gargalo antes de virar plano*.
Cada mecanismo entra com **URL e citação do fonte** (arquivo e função, no
commit que eu de fato li), e sai com **o número que o mataria ou o
confirmaria aqui**. Onde eu não sei medir a receita contra o nosso caso, o
documento diz isso em vez de recomendá-la.

A pergunta é uma só, e é a do pedido: **como os outros decidem O QUE
sincronizar no commit ou no checkpoint — eles pagam `fsync` em componente que
não mudou, como sabem que não mudou, e o que a decisão custa e compra?**

---

## 0. O gargalo, relido e não remedido

Estes números **não foram medidos aqui**. Vêm de uma bateria já registrada
nesta mesma rodada (`git log`, commit `c050fb5`, "O comboio e fsync, e
medi-lo achou um buraco de durabilidade", 04/09/2026) e dos dois medidores que
esse commit deixou: `crates/phxsql-store/examples/o-comboio-por-dentro.rs`
(tempo, sem `strace`) e `crates/phxsql-store/examples/sonda-do-fecho.rs`
(identidade dos arquivos, com `strace`). Eu **rodei o segundo de novo** nesta
pesquisa, para conferir a lista antes de escrever qualquer proposta em cima
dela — isso está na §1.

| fato | número | onde |
|---|---|---|
| `fsync` por fecho de tabela (`Table::sincronizar`) | **7,0**, constante em três escalas (7,03 / 7,02 / 7,01) | commit `c050fb5` |
| fração do fecho que é `fsync` (o resto é `abrir`) | **95–96%** em K=1, **93%** em K=16 | idem, `o-comboio-por-dentro` |
| tempo que o fecho segura a trava global, K=16 | **19.602–20.093 µs** (≈ 20 ms) | idem |
| tempo que o fecho segura a trava global, K=1 | **1.197–1.479 µs** | idem |
| os sete arquivos do fecho normal (sem o `.reg`, que é bug já achado) | `.trash .bin .memo .log .reason .ndx .ndx` | `sonda-do-fecho.rs`, e confirmado de novo por mim na §1 |

E o fato que decide o desenho, e que não é um número:

> **`Volumes::sincronizar` (`crates/phxsql-store/src/volume.rs:336-347`)
> percorre os descritores ABERTOS e chama `sync_all()` em cada um. Não há
> distinção entre "foi aberto porque foi escrito" e "foi aberto porque foi
> lido" — e `Table::abrir` (`table.rs:714-726`) abre os sete de uma vez,
> sempre, mesmo quando a janela só vai inserir.**

Isso já entrega metade da resposta à pergunta do pedido antes de eu chegar a
qualquer fonte de fora: hoje o PhxSql sincroniza **o que está aberto**, não **o
que foi escrito**. A pesquisa que segue pergunta como os outros resolvem essa
mesma distinção — e a resposta, nos três motores que a resolvem, é a mesma
forma: **um sinalizador de sujeira por arquivo, ligado só no caminho de
escrita.**

---

## 1. O mecanismo no NOSSO código, verificado de novo antes de propor qualquer coisa

Antes de trazer fonte alheio, confirmei a premissa no nosso, porque *medir a
premissa do item vem antes de implementar o item* vale também para o item de
outra frente.

```bash
flock /tmp/phx-cargo.lock cargo build --release --example sonda-do-fecho -p phxsql-store
strace -f -y -T -e trace=fsync,openat target/release/examples/sonda-do-fecho
```

Reproduzi a corrida (mesmo commit, mesmo binário) e a fase 3 (o fecho, entre as
cercas 2 e 3) mostrou, na ordem:

```
openat(...clientes.reg,    O_RDONLY)              <- só LEITURA da schema
openat(...clientes.ndx,    O_RDWR)
openat(...clientes.bin,    O_RDWR)
openat(...clientes.memo,   O_RDWR)
openat(...clientes.log,    O_RDWR)
openat(...clientes.trash,  O_RDWR)
openat(...clientes.reason, O_RDWR)
openat(...clientes.reg,    O_RDWR)
fsync(.trash)  fsync(.bin)  fsync(.memo)  fsync(.log)
fsync(.reason) fsync(.ndx)  fsync(.ndx)
```

Sete `fsync`, nenhum no `.reg` — bate exatamente com o que o `c050fb5` já
tinha achado. **Não estou reusando os tempos desse `strace`**: uma corrida sob
`ptrace`, sem aquecimento e sem repetição, deu de 67 µs a 341 ms para o mesmo
`fsync(.trash)` em fases diferentes — ruído do instrumento, não do sistema
(o próprio `sonda-do-fecho.rs` já avisa disso no comentário: contar com
`strace` é para saber QUAIS, o `o-comboio-por-dentro.rs` sem `strace` é para
saber QUANTO). O que uso daqui é só a **identidade e a ordem** dos arquivos,
que os dois medidores concordam.

O motivo de todos os sete abrirem, mesmo numa janela que só insere, está no
código de cada componente. Os quatro que uma inserção pura nunca escreve
chamam só leitura de cabeçalho na abertura:

| arquivo | `abrir()` faz | escreve nele? |
|---|---|---|
| `.trash` | `LixeiraFile::abrir` → `l.cab(1)?; l.cab(volume_atual)?;` — só leitura (`lixeira.rs:340-360`) | só em `excluir` |
| `.bin` / `.memo` | `BlobFile::abrir` → `b.cab(1)?; b.cab(volume_atual)?;` — só leitura (`blob.rs:103-129`) | só se a linha tiver `Blob`/`Memo` |
| `.reason` | `MotivoFile::abrir` → `m.cab(1)?; m.cab(volume_atual)?;` — só leitura (`motivo.rs:277-297`) | só em `excluir` |

E `Volumes::arquivo` (`volume.rs:224-243`) não distingue: tanto a leitura de
cabeçalho quanto uma escrita real chamam a mesma função, que insere o
descritor em `self.abertos`. `sincronizar()` depois itera `abertos` inteiro.
**O sinalizador que falta não é "foi aberto", é "foi escrito depois da última
sincronização"** — e é exatamente esse sinalizador que os três motores a
seguir mantêm.

Achado de passagem, fora do escopo desta pesquisa mas na mesma vizinhança: o
`.pag` (`Table::gravar_pag`, `table.rs:2174-2183`, escrito por
`pag::escrever` com `std::fs::write` simples, `pag.rs:47-49`) é gravado a
**cada** `sincronizar()` e **nunca** aparece na lista de `fsync` — nem no
`strace` de hoje, nem no de 03/09. É o oposto do problema desta pesquisa: um
arquivo escrito sempre e sincronizado nunca. Fica registrado para o papel C
avaliar; não é a pergunta que este documento responde.

---

## 2. PostgreSQL(R) — um sinalizador por segmento de arquivo, ligado só na escrita

### O que o fonte faz

O `checkpointer` do PostgreSQL não varre os arquivos para descobrir quais
mudaram: ele mantém uma **tabela hash de pedidos pendentes**, e só entra nela
quem escreveu.

```c
/*
 * In some contexts (currently, standalone backends and the checkpointer)
 * we keep track of pending fsync operations: we need to remember all relation
 * segments that have been written since the last checkpoint, so that we can
 * fsync them down to disk before completing the next checkpoint.
 */
```
`src/backend/storage/sync/sync.c:34-40` (tag `REL_17_0`)

Quem alimenta essa tabela:

```c
static void
register_dirty_segment(SMgrRelation reln, ForkNumber forknum, MdfdVec *seg)
{
    FileTag tag;
    INIT_MD_FILETAG(tag, reln->smgr_rlocator.locator, forknum, seg->mdfd_segno);
    ...
    if (!RegisterSyncRequest(&tag, SYNC_REQUEST, false))
        ...
}
```
`src/backend/storage/smgr/md.c:1353-1362` (tag `REL_17_0`)

E `register_dirty_segment` só é chamado de seis pontos do `md.c`, e todos são
caminho de escrita: `mdcreate` (`md.c:242`), `mdextend` (`md.c:513`),
`mdzeroextend` (`md.c:617`), `mdwritev` (`md.c:1014`), `mdtruncate`
(`md.c:1196,1222`) e `mdregistersync` (`md.c:1266`, que o comentário do
próprio fonte descreve como "mark whole relation as needing fsync" — usado
fora do caminho de escrita normal, para forçar a marca inteira de uma vez).
**Nenhum** desses seis pontos é `mdread` ou `mdreadv`. Ler um
segmento não o marca; criar, estender, truncar ou escrever nele, sim. No
checkpoint, `ProcessSyncRequests` (`sync.c:286`) varre só a tabela hash — um
segmento nunca tocado por escrita desde o último checkpoint nunca entra nela,
e por isso nunca recebe `fsync`.

### O que compraria aqui, e o número

A granularidade deles é **segmento de relação** (até 1 GiB, um arquivo por
tabela ou índice, subdividido). A nossa já é **arquivo inteiro por
componente** — sete arquivos pequenos, não um arquivo grande fatiado. Isso
significa que a versão simplificada da ideia serve melhor aqui do que lá: em
vez de uma tabela hash com contador de ciclo (que existe no Postgres para
mesclar pedidos de **múltiplos processos concorrentes**, coisa que não temos —
uma única trava global já serializa tudo), bastaria um sinalizador por
componente dentro do próprio `Table`: `sujo: [bool; 7]` ou um `HashSet` pequeno,
ligado nos pontos de escrita de cada arquivo e consultado por
`Table::sincronizar` para pular o `sync_all()` — e o `flush()` junto, que em
`std::fs::File` já não faz nada, mas documenta a intenção — de quem não tem o
bit ligado.

**O número, medido no NOSSO esquema de bancada** (2 índices, sem `Blob`/`Memo`,
inserção pura — o mesmo da §4.8 do `DESEMPENHO.md`): dos sete arquivos do
fecho, só `.log` e os dois `.ndx` são de fato escritos numa inserção; `.reg`
seria o oitavo, uma vez corrigido o bug já achado. Isso é **4 de 8** arquivos
com escrita real — os outros quatro (`.trash`, `.bin`, `.memo`, `.reason`)
seriam pulados pelo sinalizador.

**O que isto NÃO é**: uma medição de tempo. É uma contagem estrutural, tirada
do código (§1). Quanto isso valeria em microssegundos **não sei dizer sem
medir** — os sete `fsync` podem não custar o mesmo entre si (arquivos maiores,
mais páginas sujas, custam mais), e a proporção 4/8 em contagem não é
necessariamente a proporção em tempo. O experimento que fecharia isto é barato
e já existe pela metade: uma variante do `o-comboio-por-dentro.rs` que, em vez
de inserir em todos os componentes, mede o fecho com o sinalizador simulado —
ou, mais simples, cronometrar cada um dos sete `fsync` individualmente dentro
do laço de `Volumes::sincronizar`, não só a soma. **Isto é implementação, e não
é meu papel fazer — é o item 1 da recomendação na §6.**

Para cargas com `excluir` (que escreve `.trash` e `.reason`) ou com colunas
`Blob`/`Memo`, o ganho é menor — o sinalizador só pula o que realmente não foi
tocado, e uma carga que também exclui toca mais dos sete. Isto não é um
detalhe: é o motivo de o número acima valer só para inserção pura, e o
documento não estica essa conta para outras cargas sem medir.

### Onde divergiria da origem, e a restrição nossa

**Divergência 1 — granularidade.** Eles marcam por segmento de até 1 GiB
dentro de um arquivo de relação; nós marcaríamos por arquivo inteiro, porque
cada componente **já é** um arquivo pequeno e dedicado — não há "segmento"
aqui, e inventar um seria complexidade sem uso.

**Divergência 2 — sem fila entre processos.** O deles existe porque
`RegisterSyncRequest` manda a marca para **outro processo** (o checkpointer),
por uma fila com limite e reintento (`sync.c:579-619`). Aqui não há segundo
processo: a trava global já serializa tudo dentro do mesmo, então o
sinalizador é um campo comum, sem fila, sem `hash_seq_search`, sem contador de
ciclo para diferenciar pedido velho de pedido novo — essa maquinaria inteira
existe para resolver concorrência entre processos que nós não temos, por
causa da nossa trava global (`CONCORRENCIA.md` §1.1, já medida em pesquisa
anterior desta casa).

**Divergência 3 — zero dependências.** O deles usa `dynahash` (tabela hash
própria do PostgreSQL) e uma fila de mensagens entre processos (shared memory).
Aqui a versão que sobra depois das duas divergências acima é um `bool` por
componente — não precisa de biblioteca nenhuma, e não é tentador precisar: é
exatamente o tipo de coisa que a regra de zero dependências deste projeto já
nos obriga a manter simples.

---

## 3. MySQL(R)/InnoDB — o mesmo desenho, na granularidade que já bate com a nossa

### O que o fonte faz

O InnoDB mantém uma lista de *tablespaces* (arquivos `.ibd`) com escrita
pendente de `fsync`:

```cpp
void Fil_shard::write_completed(fil_node_t *file) {
  ...
  if (fil_disable_space_flushing(file->space)) {
    ...
    file->set_flushed();
  } else {
    add_to_unflushed_list(file->space);
  }
}
```
`storage/innobase/fil/fil0fil.cc:7450-7461` (tag `mysql-8.0.40`)

```cpp
void Fil_shard::add_to_unflushed_list(fil_space_t *space) {
  if (!space->is_in_unflushed_spaces) {
    space->is_in_unflushed_spaces = true;
    UT_LIST_ADD_FIRST(m_unflushed_spaces, space);
  }
}
```
`fil0fil.cc:7437-7444`

`write_completed` só é chamado quando uma **escrita** termina, nunca numa
leitura. E o checkpoint só sincroniza quem está na lista:

```cpp
void Fil_shard::flush_file_spaces() {
  ...
  for (auto space : m_unflushed_spaces) {
    if ((to_int(space->purpose) & FIL_TYPE_TABLESPACE) && !space->stop_new_ops)
      space_ids.push_back(space->id);
  }
  ...
  for (auto space_id : space_ids) { ... space_flush(space_id); ... }
}
```
`fil0fil.cc:8156-8171`, chamado por `fil_flush_file_spaces()` (`fil0fil.cc:8187`)

E há uma segunda camada, mais fina ainda: mesmo um arquivo **na lista**, se já
foi sincronizado por outra chamada concorrente depois que entrou nela, é
pulado (`file.flush_counter >= old_mod_counter` → `skip_flush = true`,
`fil0fil.cc` dentro de `space_flush`) — colapsa chamadas duplicadas de
`fsync` no mesmo arquivo, coisa que nós também já pagamos hoje: se dois
`sincronizar()` seguidos acontecem sem escrita no meio, o segundo é
`fsync` desperdiçado, exatamente o caso desta pesquisa.

O redo log (commit do cliente) é sincronizado por uma **thread dedicada**,
separada da lista acima — o `fsync` que garante durabilidade do commit nunca
espera pelos arquivos de dado (`log0write.cc`, comentário nas linhas 113-133 e
278-280: "The log flusher thread is responsible for doing fsync() of the log
files"). Essa separação **já está estudada** nesta casa (§12.1 do
`DESEMPENHO.md`, o group commit, 2,63× medido) — não repito aqui.

### O que compraria aqui, e por que este é o modelo mais próximo do nosso

A granularidade do InnoDB é **arquivo** (`.ibd` por tabela, no modo
`innodb_file_per_table`) — a mesma unidade que nós já temos, sete vezes por
tabela em vez de uma. É o motivo de eu recomendar a forma do InnoDB, não a
forma bruta do PostgreSQL, como molde: um sinalizador por **arquivo inteiro**
(`is_in_unflushed_spaces` vira, aqui, um campo em cada `Volumes` ou um bit no
`Table`), sem a divisão em segmentos que o Postgres precisa porque os arquivos
dele são grandes e os nossos não são.

O número é o mesmo da §2 — 4 de 8 arquivos com escrita real numa inserção
pura — porque é a mesma pergunta respondida por um fonte diferente. O valor
de ler os dois não é duplicar o número: é confirmar que dois motores de
linhagem completamente diferente (processo único multi-thread com buffer pool
compartilhado, contra processos independentes com checkpointer via fila)
chegaram no **mesmo mecanismo**: marca na escrita, nunca na leitura, arquivo
fora da lista não paga syscall nenhum.

### Onde divergiria da origem, e a restrição nossa

**Divergência 1 — sem mutex de lista compartilhada.** `m_unflushed_spaces` é
uma lista global, protegida por um mutex do `Fil_shard`, porque dezenas de
threads (leitura, escrita, o *page cleaner*) mexem nela ao mesmo tempo. Aqui
o candidato mais simples é um campo **dentro do próprio `Table`**, sem mutex
próprio — porque `Table` já só existe sob a trava global de dados
(`CONCORRENCIA.md` §2: a trava não protege página, protege o catálogo de
tabelas abertas). Não precisamos do mutex do InnoDB porque já pagamos uma
trava mais grossa no mesmo lugar.

**Divergência 2 — nem todo `fsync` some.** O `space_flush` do InnoDB tem uma
saída de segurança (`skip_flush`) que ainda assim confere estado antes de
decidir pular. A nossa versão pode ser mais simples porque não há concorrência
de escrita **dentro** do mesmo componente entre o momento de marcar sujo e o
de sincronizar — de novo, a trava global.

**Divergência 3 — o redo log já está fora desta proposta.** A separação
log-contra-dado que o InnoDB faz é o item que a §12.1 do `DESEMPENHO.md` já
estudou e mediu (2,63×). Esta pesquisa não reabre esse item; olha só para os
outros seis/sete arquivos que **não são** o diário de durabilidade.

---

## 4. Apache Cassandra — paga os mesmos N arquivos por tabela, mas raramente: é amortização, não seletividade

### O que o fonte faz

Ao contrário dos dois anteriores, o Cassandra **não pula** `fsync` de
componente nenhum do SSTable. Cada escritor de componente é um
`SequentialWriter`, e a classe documenta a política no próprio comentário:

```java
/**
 * Adds buffering, mark, and fsyncing to OutputStream. We always fsync on
 * close; we may also fsync incrementally if Config.trickle_fsync is enabled.
 */
public class SequentialWriter extends BufferedDataOutputStreamPlus ...
```
`src/java/org/apache/cassandra/io/util/SequentialWriter.java:33-36` (commit
`7b5ab44`, o mesmo já pinado em `docs/CASSANDRA.md`)

E o `fsync` de fechamento é automático, via o framework `Transactional` deles,
não uma chamada isolada esquecível:

```java
protected class TransactionalProxy extends AbstractTransactional {
    ...
    protected void doPrepare() { syncInternal(); }
```
`SequentialWriter.java:88-91`

No flush de uma SSTable, o `Data.db` e o `Index.db` são sincronizados
explicitamente:

```java
public SSTableReader openFinalEarly() {
    // we must ensure the data is completely flushed to disk
    dataWriter.sync();
    indexWriter.writer.sync();
```
`io/sstable/format/big/BigTableWriter.java:217-221`

E o filtro de Bloom (`Filter.db`) também:

```java
public static void save(IFilter filter, Descriptor descriptor, boolean deleteOnFailure) throws IOException
{
    File filterFile = descriptor.fileFor(Components.FILTER);
    try (FileOutputStreamPlus stream = filterFile.newOutputStream(File.WriteMode.OVERWRITE))
    {
        filter.serialize(stream, descriptor.version.hasOldBfFormat());
        stream.flush();
        stream.sync();
```
`io/sstable/format/FilterComponent.java:73-81`

**Nenhum dos componentes de um SSTable escapa do `fsync`.** O Cassandra não
tem — e não precisa de — um sinalizador de sujeira por arquivo, porque cada
`SequentialWriter` só existe durante a vida de UM flush: se ele foi aberto, foi
para escrever aquele componente, e sempre escreve.

### O que compraria aqui — e a resposta é "nada de novo", com o número que mostra por quê

A razão de eles poderem pagar `fsync` incondicional em cada componente, sempre,
é que o **flush inteiro é raro**: dispara por tamanho da memtable
(centenas de MiB, tipicamente dezenas de milhares a milhões de mutações) ou por
tempo, não por operação. A durabilidade que o **cliente** vê não vem daí — vem
do commit log, sincronizado à parte, com a própria política de `periodic`
(10 s, sem esperar nenhuma escrita) ou `batch`/`group`
(`docs/CASSANDRA.md` §3.1, já medido e citado lá com `PeriodicCommitLogService.java`
e `GroupCommitLogService.java`). O flush do SSTable é o **checkpoint**
deles, não o commit.

**E isto já é o que o `por_lote` faz aqui.** O nosso `sincronizar()` (o fecho
de janela) É o equivalente ao flush de memtable: acontece por contagem OU
tempo (200 operações ou 200 ms, `config.rs:1655-1656`), não por escrita
individual. A diferença entre nós e o Cassandra não está em "sincronizar tudo
sempre é caro" — está em **quantas escritas cabem dentro de uma janela**: a
memtable deles acumula ordens de grandeza mais mutações por flush do que os
nossos 200 por janela.

Por isso esta receita **não entra na §6 como proposta nova**: ela confirma que
a forma já adotada (`por_lote`, group commit, §12.1 do `DESEMPENHO.md`) é a
resposta correta ao problema que o Cassandra resolve — e que aumentar a janela
(200 → mais) é a alavanca de amortização, não uma alavanca de seletividade
como a das §2 e §3. As duas alavancas são independentes e **as duas** cabem no
mesmo motor: maior janela reduz quantas vezes os 7-8 `fsync` acontecem;
sinalizador de sujeira reduz quantos dos 7-8 acontecem em cada vez. Uma não
substitui a outra.

### Onde divergiria — e por que aqui a resposta é "não diverge, porque não se propõe nada"

Não há item novo para nomear divergência: a leitura do Cassandra não produz
uma proposta de código, produz uma confirmação de que o desenho existente
(`por_lote`) já ocupa o lugar que essa receita ocuparia. Registrar isto é o
que evita a mesma pesquisa voltar pedindo "adaptar o batching do Cassandra" —
já está adaptado.

---

## 5. SQLite e LMDB — a pergunta não se coloca lá, porque N=1

### O que o fonte faz

O SQLite guarda toda tabela e todo índice de um banco dentro de **um único**
arquivo — não há "vários componentes" para escolher entre sincronizar ou não.
O commit no modo padrão (*rollback journal*) faz **no máximo dois** `fsync`:
um no diário, um no banco:

```c
int sqlite3PagerSync(Pager *pPager, const char *zSuper){
    ...
    rc = sqlite3OsSync(pPager->fd, pPager->syncFlags);
```
`src/pager.c:6373-6380` (tag `version-3.47.0`)

No modo WAL, o commit sincroniza **só o WAL** (um arquivo, à parte do banco):

```c
static int walFrames(...) {
    ...
    rc = sqlite3OsSync(pWal->pWalFd, CKPT_SYNC_FLAGS(sync_flags));
```
`src/wal.c:4076` (tag `version-3.47.0`)

E o banco principal só é sincronizado no **checkpoint**, separado e raro:

```c
static int walCheckpoint(...) {
    ...
    rc = sqlite3OsSync(pWal->pDbFd, CKPT_SYNC_FLAGS(sync_flags));
```
`src/wal.c:2308`

O LMDB é ainda mais extremo: um único arquivo `mmap`eado guarda **todos** os
bancos nomeados de um ambiente, e o commit faz **um** `msync`/`fdatasync` que
cobre tudo que mudou:

```c
int mdb_env_sync(MDB_env *env, int force) { ... }
```
`libraries/liblmdb/mdb.c:2536` (tag `LMDB_0.9.31`)

E há um truque específico para a página de metadados, que é escrita a cada
commit: em vez de `write()` bufferizado seguido de `fsync()`, o LMDB abre um
**segundo descritor** para o mesmo arquivo com `O_DSYNC`, e escreve a
metadados por ele — o próprio `write()` já é síncrono, então não paga um
`fsync()` a mais:

```c
/* Write to the SYNC fd unless MDB_NOSYNC/MDB_NOMETASYNC.
 * (me_mfd goes to the same file as me_fd, but writing to it
 * also syncs to disk. Avoids a separate fdatasync() call.)
 */
mfd = (flags & (MDB_NOSYNC|MDB_NOMETASYNC)) ? env->me_fd : env->me_mfd;
```
`mdb.c:3906-3909`, dentro de `mdb_env_write_meta` (`mdb.c:3837`)

### O que compraria aqui

**Nada, na pergunta "quais dos sete arquivos".** SQLite e LMDB não respondem
essa pergunta porque não a têm — eles têm 1 arquivo (2, com o diário/WAL). A
nossa arquitetura de **arquivos separados** (a marca do projeto, "modelo de
arquivos separados do HFSQL", primeira linha do `CLAUDE.md` do projeto) é
exatamente a escolha que cria o problema que o SQLite e o LMDB não têm — e não
é uma escolha à toa: é o que permite `.trash`/`.reason`/`.bin`/`.memo` existir
como arquivos de tamanho e ciclo de vida próprios, sem inchar o `.reg`
principal. Comparar com um motor de arquivo único aqui seria medir a receita
alheia contra o gargalo alheio, não o nosso — e por isso não vira proposta.

O truque do `O_DSYNC` no descritor de metadados é a única ideia isolável desta
seção, e ela não tem alvo claro hoje: o candidato mais parecido em forma —
pequeno, reescrito a cada `sincronizar()` — é o `.pag` (achado de passagem na
§1), que hoje **não é sincronizado nem pelo jeito antigo** (`write` +
`fsync`) nem pelo jeito do LMDB (`O_DSYNC`). Aplicar o truque resolveria um
problema que o `.pag` não tem hoje (ele não paga `fsync` nenhum, então não há
`fsync` para evitar) — ele criaria uma garantia nova, não economizaria uma que
já existe. Por isso fica registrado como ideia madura sem aplicação imediata,
não como proposta.

### Onde divergiria da origem, e a restrição nossa

Não há divergência a nomear, porque não há receita adotável — a diferença
estrutural (1 arquivo contra 7-8) não é uma decisão de código que se copie ou
se adapte, é a decisão de **formato** já tomada e documentada
(`docs/FORMATO.md`) e coberta pela cláusula de mudança de formato cedo, não
agora.

---

## 6. As três colunas

| receita | o que ela custa/compra aqui | o número que a confirmaria ou mataria |
|---|---|---|
| **PostgreSQL — sinalizador por segmento, ligado só na escrita** (`sync.c`, `md.c`) | Adaptável, mas na forma simplificada: sem fila entre processos, sem tabela hash com contador de ciclo — um campo por componente já que a trava global resolve a concorrência que o deles resolve com fila | **Contagem, não tempo:** 4 de 8 arquivos sem escrita real numa inserção pura do esquema da bancada. **Mede o tempo:** cronometrar cada um dos 7-8 `fsync` de per si dentro do laço de `sincronizar`, não só a soma — extensão barata do `o-comboio-por-dentro.rs` |
| **InnoDB — mesma ideia, granularidade de ARQUIVO** (`fil0fil.cc`) | É o molde mais próximo do nosso formato: a unidade dele (tablespace = arquivo) já é a nossa unidade (componente = arquivo). Sem mutex de lista compartilhada — a trava global já serializa | Mesmo número da linha acima; a leitura de dois fontes independentes que convergem no mesmo mecanismo é o que dá confiança de propor |
| **Cassandra — sync sempre, mas raro** (`SequentialWriter.java`, `BigTableWriter.java`) | **Já adotado.** É o que `por_lote` (200 op / 200 ms, `config.rs:1655-1656`) faz; a alavanca de amortização já está puxada, e é independente da de seletividade | Nenhum — não é proposta nova. Registrado para a mesma ideia não voltar pedindo "adaptar o batching deles" |
| **SQLite/LMDB — 1 arquivo, então a pergunta não existe** | Não se aplica — mudar para arquivo único contradiz a marca do projeto ("arquivos separados", `CLAUDE.md`) e é decisão de formato, não de código | Nenhum experimento barato a propor; recusa por incompatibilidade de identidade, não por número |
| **LMDB — `O_DSYNC` no descritor da metadata, em vez de write+fsync** | Sem alvo hoje: o candidato (`.pag`) não paga `fsync` nenhum atualmente, então o truque criaria uma garantia nova em vez de economizar uma existente | Fica registrado; não há número a medir porque não há custo a cortar hoje |

---

## 7. O que eu avaliei e recuso, com o número

**1. Copiar a tabela hash do PostgreSQL como está, com fila e contador de
ciclo.** Recusada por excesso: essa maquinaria existe para mesclar pedidos de
**múltiplos processos** concorrentes escrevendo no mesmo checkpointer. Aqui há
uma trava global e um processo — a versão simplificada (um `bool` por
componente) entrega o mesmo resultado sem a fila. Custo do que se recusa:
uma dependência de sincronização entre processos que este motor não tem e a
regra de zero dependências externas não pediria de qualquer forma.

**2. Adotar batching maior (Cassandra) como a resposta a este pedido.**
Recusada não porque seja ruim, mas porque **já está feita**: `por_lote` é essa
mesma resposta, medida em §12.1 do `DESEMPENHO.md` (2,63×). Aumentar a janela
ainda mais é uma alavanca válida, mas é OUTRO item — de amortização, não de
seletividade — e não deve se disfarçar de "achado desta pesquisa".

**3. Migrar para arquivo único (SQLite/LMDB) para eliminar a pergunta.**
Recusada por identidade, sem número: contradiz a primeira linha do
`CLAUDE.md` do projeto ("modelo de arquivos separados do HFSQL") e é decisão
de formato — CLAUDE.md do projeto manda essa decisão entrar cedo, e "agora,
para resolver `fsync`" não é cedo, é tarde e é o motivo errado.

**4. `O_DSYNC` no `.pag` (LMDB).** Não recusada por ruim — recusada por não
ter alvo: o `.pag` hoje não é sincronizado, então a receita resolveria um
problema inexistente (economizar um `fsync` que já não é pago) e criaria uma
garantia nova sem que ninguém tivesse pedido. Fica anotada, não implementada;
quem decidir que o `.pag` PRECISA de garantia de disco decide isso por outro
motivo, e aí a receita do LMDB volta a fazer sentido.

---

## 8. O que eu recomendo perseguir, e nesta ordem

1. **O sinalizador de sujeira por componente**, molde InnoDB (granularidade de
   arquivo, sem fila) — ligado só nos pontos de escrita real de cada um dos
   sete/oito arquivos, consultado por `Table::sincronizar` para pular
   `sync_all()` de quem só foi lido nesta janela. Contagem já confirmada por
   código e por `strace`: **4 de 8** numa inserção pura do esquema da bancada.
   **Isto é trabalho do papel B/C, não meu — eu não mudo código.**

2. **O cronômetro por arquivo**, antes do item 1 virar código: estender
   `o-comboio-por-dentro.rs` (ou um novo medidor, mesmo molde, sem `strace`)
   para separar os 7-8 `fsync` de per si, não só a soma. Sem isso, o número
   "4 de 8" fica sendo contagem, e a §0 desta casa já pagou caro por número
   citado sem medir (o mesmo documento que originou esta pesquisa, com o
   pedido 113, media o alvo certo com a causa errada).

3. **Não empilhar o item 1 com um aumento de `por_lote`** na mesma medição —
   são duas alavancas independentes (§4), e misturar as duas no mesmo
   experimento reproduziria o erro que a bancada deste projeto já cometeu duas
   vezes: comparar coisas que não são o mesmo trabalho.

---

## 9. Fontes

### Fontes de código consultados, com commit/tag fixado

| assunto | fonte | commit/tag |
|---|---|---|
| PostgreSQL, fila de pedidos de sincronização | `src/backend/storage/sync/sync.c` | `REL_17_0` |
| PostgreSQL, marca de segmento sujo na escrita | `src/backend/storage/smgr/md.c` | `REL_17_0` |
| InnoDB, lista de tablespaces não sincronizados | `storage/innobase/fil/fil0fil.cc` | `mysql-8.0.40` |
| InnoDB, thread de `fsync` do redo log | `storage/innobase/log/log0write.cc` | `mysql-8.0.40` |
| Cassandra, `fsync` sempre no fechamento do escritor | `src/java/org/apache/cassandra/io/util/SequentialWriter.java` | `7b5ab44` (o mesmo commit já pinado em `docs/CASSANDRA.md`) |
| Cassandra, `sync()` do `Data.db`/`Index.db` | `.../io/sstable/format/big/BigTableWriter.java` | `7b5ab44` |
| Cassandra, `sync()` do filtro de Bloom | `.../io/sstable/format/FilterComponent.java` | `7b5ab44` |
| SQLite, `fsync` do banco no commit (rollback journal) | `src/pager.c` | `version-3.47.0` |
| SQLite, `fsync` do WAL por commit e do banco no checkpoint | `src/wal.c` | `version-3.47.0` |
| LMDB, `fsync`/`msync` único do ambiente inteiro | `libraries/liblmdb/mdb.c` | `LMDB_0.9.31` |

Todos lidos via `raw.githubusercontent.com` nesta sessão (04/09/2026), URL
completa = `https://raw.githubusercontent.com/<owner>/<repo>/<tag>/<caminho>`.
Não achei razão para preferir uma tag diferente destas: são releases
estáveis recentes de cada projeto, e o Cassandra já estava fixado pelo estudo
anterior desta casa.

### O que eu NÃO consegui alcançar

Nada. Todas as cinco fontes (PostgreSQL, InnoDB, Cassandra, SQLite, LMDB)
estavam acessíveis por `raw.githubusercontent.com` nesta sessão — testado
arquivo por arquivo antes de citar (`curl -o /dev/null -w '%{http_code}'`,
todos 200). O `api.github.com` não estava disponível para este uso (mensagem:
"GitHub access to this repository is not enabled for this session"), mas não
precisei dele — o conteúdo bruto por `raw.githubusercontent.com` bastou para
toda citação.

### Números desta casa, e de onde saíram

| número | de onde |
|---|---|
| 7,0 `fsync` por fecho (7,03/7,02/7,01), fração 93-96%, 20 ms em K=16 | commit `c050fb5`, `o-comboio-por-dentro.rs` — **medição anterior, não refeita aqui** |
| a lista dos sete arquivos, sem o `.reg` | `sonda-do-fecho.rs`, rodado de novo nesta pesquisa com `strace -f -y -T -e trace=fsync,openat` — identidade e ordem confirmadas; tempos descartados por ruído do `strace` |
| `.trash`/`.bin`/`.memo`/`.reason` só leem cabeçalho na abertura | `lixeira.rs:340-360`, `blob.rs:103-129`, `motivo.rs:277-297` — lidos nesta pesquisa |
| `Volumes::sincronizar` fsyncs tudo que está aberto, sem distinguir leitura de escrita | `volume.rs:224-243` (`arquivo`) e `volume.rs:336-347` (`sincronizar`) — lidos nesta pesquisa |
| `Table::abrir` abre os sete componentes sempre | `table.rs:714-726` — lido nesta pesquisa |
| `.pag` nunca é sincronizado, apesar de escrito toda vez | `table.rs:2174-2183`, `pag.rs:47-49` — lido nesta pesquisa; achado de passagem |
| group commit já medido, 2,63× | `DESEMPENHO.md` §12.1 — **medição anterior, não refeita aqui** |
| `por_lote`: 200 operações ou 200 ms | `crates/phxsql-server/src/config.rs:1655-1656` (o padrão de `Recursos::default()`) — lido nesta pesquisa |

---

## Nota sobre os nomes

PostgreSQL(R) é marca registrada da PostgreSQL Community Association of
Canada, sob a licença permissiva PostgreSQL License. MySQL(R) e InnoDB são
marcas da Oracle Corporation, sob licença dupla GPLv2/comercial. Apache
Cassandra é marca da Apache Software Foundation, sob licença Apache 2.0.
SQLite(R) é marca registrada da Hipp, Wyrick & Company, Inc.; o código-fonte
em si é de domínio público. LMDB é obra da Symas Corporation, sob a licença
OpenLDAP. Este documento lê os fontes públicos de cada um para entender
decisões de projeto; **nenhum código foi copiado para o PhxSql** — as
divergências nomeadas em cada seção são o registro de que cada ideia passou
pela nossa cabeça e pelas nossas restrições antes de virar proposta, não pelos
nossos dedos.
