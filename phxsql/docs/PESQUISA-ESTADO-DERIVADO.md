# Pesquisa: como os outros impedem que o estado derivado do catálogo fique velho

Documento do papel **J (pesquisador)**, e ele obedece à regra que dá sentido ao
papel: *receita de fora se mede contra o nosso gargalo antes de virar plano*.
Cada mecanismo entra com **URL e citação da documentação oficial** (ou do
comentário do fonte, com arquivo e função), e sai com **o número que o mataria
ou o confirmaria aqui**. Onde o manual não diz o custo, o documento **diz que
não diz**.

A pergunta é uma só: **o que MySQL(R)/InnoDB, MariaDB(R) e PostgreSQL(R) fazem
para que uma cópia derivada da definição da tabela não continue valendo depois
que a definição mudou — e o que disso sobrevive ao nosso tamanho.**

---

## 0. O defeito que gerou a frente, e o que ele é como classe

Hoje, 03/09/2026, `Table::redeclarar_chaves_estrangeiras`
(`crates/phxsql-store/src/table.rs:562`) trocava `self.esquema` e **não
refazia** `fks_conferidas` — uma lista de **posições** para dentro de
`esquema.chaves_estrangeiras()`. Redeclaração com menos chaves deixava índice
apontando para fora e o `inserir` seguinte entrava em **pânico**
(`index out of bounds`) na indexação de `conferir_fks` — hoje `table.rs:822`,
`table.rs:808` na numeração de antes do conserto. O irmão `acrescentar_coluna`, cem
linhas abaixo, já refazia. Consertado no commit `86617f3`, com prova real nos
dois sentidos.

A instância é pequena. A **classe** não é, e é dela que trata este documento:

> Um `struct Table` guarda uma **cópia** do esquema (`self.esquema`) e **listas
> derivadas dessa cópia** (`colunas_marcadas`, `fks_conferidas`), montadas uma
> vez na abertura para que o laço quente pague só um `is_empty()`. Quando o
> esquema muda, alguém tem de lembrar de refazer as listas.

Medido no código, e não estimado:

| o que | quanto | como se conta |
|---|---:|---|
| campos do `struct Table` | **20** | `awk '/^pub struct Table \{/,/^\}/' table.rs \| grep -cE '^\s+[a-z_]+:\s'` |
| desses, derivados do esquema | **2** | `colunas_marcadas`, `fks_conferidas` |
| lugares que escrevem `self.esquema` | **3** | `grep -n 'self\.esquema = ' table.rs` → `:580`, `:699`, `:4096` |
| lugares que esqueceram de refazer | **1 de 3** | o `:580`, até hoje |

Para comparação, medida do mesmo jeito no fonte do PostgreSQL(R)
(`src/include/utils/rel.h`, ramo `master`, baixado em 03/09/2026):

| o que | quanto |
|---|---:|
| campos `rd_*` da `RelationData` | **65** |
| desses, com **bandeira de validade própria** | **6** (`rd_isvalid`, `rd_indexvalid`, `rd_statvalid`, `rd_fkeyvalid`, `rd_partcheckvalid`, `rd_attrsvalid`) |

**Esta razão — 2 contra 65 — é a que decide o documento inteiro**, e vale
guardá-la antes de ler qualquer receita: os mecanismos abaixo foram construídos
para catálogos com dezenas de campos derivados, centenas de lugares que mudam o
catálogo e dezenas de processos com cache próprio de longa vida. Nós temos dois
campos, três lugares e um processo.

---

## 1. O que ESTA rodada mediu

Três medições próprias, todas em máquina ociosa (`loadavg` 0,25–0,72 em 4
núcleos, contra 4,39 da rodada anterior do `custo-de-abrir`).

### 1.1 Reconstruir a `Table` inteira: 46,1–47,9 µs

```bash
flock /tmp/phx-cargo.lock cargo build --release --example custo-de-abrir -p phxsql-store
./target/release/examples/custo-de-abrir 5000
```

| amostra | abrir a tabela (7 arquivos) e fechar | inserir com a tabela já aberta |
|---|---:|---:|
| 1 | **47,72 µs** | 11,36 µs |
| 2 | **47,92 µs** | 11,08 µs |
| 3 | **46,08 µs** | 13,83 µs |

Isto **fecha uma pendência do `PESQUISA-TRAVA-E-MVCC.md` §1.1**: lá o número
saiu 47,07/48,76 µs com a máquina em `loadavg` 4,39, e ficou registrado como
**teto**, com a ressalva de que o valor real seria menor. Não é: com a máquina
ociosa dá o mesmo. **Abrir não era limitado por concorrência de CPU** — é
trabalho de abrir sete arquivos, e custa o que custa.

### 1.2 O portão derivado vale entre 0,3 e 26,6 nanossegundos

Medidor próprio, escrito **fora do repositório** (não versionado; ver §7), com
`std::hint::black_box` e 20 milhões de voltas por medida. Ele compara o portão
**guardado** (a lista montada na abertura) com o portão **calculado direto do
esquema** — o que não tem como envelhecer porque não existe:

| esquema | portão FK: lista | portão FK: direto | portão LGPD: lista | portão LGPD: direto |
|---|---:|---:|---:|---:|
| 6 colunas, 0 marcada, 0 chave | 0,656 ns | 0,935 ns | 0,973 ns | 5,523 ns |
| 42 colunas, 0 marcada, 0 chave | 0,654 ns | 0,922 ns | 0,921 ns | **27,493 ns** |
| 42 colunas, 3 marcadas, 3 chaves (1 confere) | 0,784 ns | 1,373 ns | 0,928 ns | 4,353 ns |

Os valores absolutos incluem o custo do próprio `black_box`; **o sinal é a
diferença**, e ela é:

* **`fks_conferidas` compra 0,28–0,86 ns por linha gravada.** Contra uma
  inserção de **4,8 µs** (só `.reg`) a **15,9 µs** (dois índices, a forma da
  bancada — `DESEMPENHO.md` §2), isso é **0,005% a 0,018%**.
* **`colunas_marcadas` compra 4,6–26,6 ns por linha.** Contra os mesmos
  4,8–15,9 µs, **0,03% a 0,55%** — o pior caso é a tabela larga **sem** coluna
  marcada, em que o `any` percorre as 42 colunas para dizer «não».

O motivo da diferença é estrutural e vale escrever: a lista de chaves
estrangeiras **já é curta** (zero na maioria das tabelas), então percorrê-la
custa quase o mesmo que perguntar se a lista derivada está vazia. A lista de
**colunas** não é curta, e é lá que a lista guardada paga.

### 1.3 Refazer as duas listas custa 12,6–81,4 ns; clonar o `Schema` custa 3,0–3,8 µs

| esquema | refazer AS DUAS listas | conferir um selo `u64` | clonar o `Schema` inteiro |
|---|---:|---:|---:|
| 6 colunas, 0 chave | 12,6 ns | 0,913 ns | 435,5 ns |
| 42 colunas, 0 chave | 37,7 ns | 0,931 ns | **2.984,2 ns** |
| 42 colunas, 3 chaves | 81,4 ns | 0,941 ns | **3.816,9 ns** |

Estes três números matam ou confirmam quase tudo o que vem abaixo:

* **o conserto de hoje — refazer tudo o que deriva do esquema — custa 12,6 a
  81,4 ns**, e ele acontece dentro de uma operação que já reescreve o cabeçalho
  do `.reg` e regrava o `.pag`. É **livre**;
* **a cópia `self.esquema` NÃO é o erro**: clonar o `Schema` custa 3,0–3,8 µs
  numa tabela de 42 colunas, que é **19% a 24% de uma inserção de 15,9 µs**.
  Quem propuser «tire a segunda cópia e leia sempre do `.reg`» está propondo um
  quarto do custo de gravação — e a proposta morre aqui, medida;
* **um selo de versão custa 0,91–0,94 ns**, o mesmo preço do portão que ele
  protegeria.

---

## 2. Os mecanismos, um a um

### 2.1 PostgreSQL(R) — reconstruir a entrada inteira e TROCAR, em vez de remendar

**O que faz.** Quando chega a notícia de que uma relação aberta mudou, o
PostgreSQL não corrige os campos afetados: ele constrói uma entrada nova do
zero e troca o conteúdo com a antiga, preservando o endereço para que o ponteiro
de quem segura a relação continue válido.
`src/backend/utils/cache/relcache.c`, `RelationRebuildRelation`
(<https://github.com/postgres/postgres/blob/master/src/backend/utils/cache/relcache.c>):

> «Reset and rebuild a relation cache entry from scratch (that is, from catalog
> entries). This is used when we are notified of a change to an open relation
> (one with refcount > 0). The entry is reconstructed without moving the
> physical RelationData record, so that the refcount holder's pointer is still
> valid.»

E a linha que interessa mais que o resto, porque é a **inversão do padrão**:

> «Since the vast majority of fields should be swapped, our method is to swap
> the whole structures and then re-swap those few fields we didn't want
> swapped.»

Isto é o oposto do que fazíamos: aqui o padrão era *guardar tudo e refazer o
que eu lembrar*; lá o padrão é *trocar tudo e devolver o que eu listar*. Quem
esquece um campo, no desenho deles, esquece de **preservar** — e o campo
esquecido volta ao valor novo, que é o certo. Quem esquece um campo, no nosso
desenho antigo, esquece de **refazer** — e o campo esquecido fica no valor
velho, que é o errado. **O mesmo esquecimento erra para lados opostos.**

**O que custa.** O manual não fala disso — é comentário de fonte, e ele não dá
número. O que ele dá é a **lista de exceções**, e ela é longa: `rd_smgr`,
`rd_refcnt`, quatro `SubTransactionId`, `rd_rel`, `rd_att` (o `TupleDesc`),
`rd_rules` + `rd_rulescxt`, `rd_rsdesc`, `rd_toastoid`, `pgstat_info`,
`pgstat_enabled`, `rd_partkey` + `rd_partkeycxt` — e o próprio comentário chama
a preservação do `TupleDesc` de *hack*. **O risco não some: muda de lugar**,
de «que campo derivado esqueci de refazer» para «que campo de sessão esqueci de
preservar».

**O que compraria aqui.** No nosso `Table` a operação equivalente é
`*self = Table::abrir(&self.diretorio, &self.nome)?` seguida de repor o que a
sessão pôs: `definir_usuario`, `definir_origem`, `ligar_imagem_no_diario`,
`espelhar`, `sobrepor`, `como_replica`. **Custo medido: 46,1–47,9 µs (§1.1)**,
contra **12,6–81,4 ns** de refazer as duas listas (§1.3) — **580× a 3.800× mais
caro pelo mesmo resultado**.

**Morto hoje. E vivo no dia em que o pedido 175 entrar** — este é o achado que
justifica a frente. O parecer do pedido 175 propõe **criar o índice na
declaração da chave**. Um índice novo não é um campo derivado do esquema em
memória: é uma **árvore nova no `.ndx`**, e o `self.ndx` do handle aberto não
saberia dela. Esse estado **não se recalcula do esquema** — só se recupera
reabrindo o arquivo. **O número que confirma a reconstrução é justamente o do
§1.1: 47 µs numa operação de modelagem** que já escreve em disco, contra a
alternativa, que é uma árvore que não existe para quem acabou de criá-la.

### 2.2 PostgreSQL(R) — lista derivada com bandeira de validade própria

**O que faz.** As listas caras derivadas do catálogo não são montadas na
abertura: são montadas **no primeiro pedido**, e cada uma tem a sua bandeira.
`relcache.c`, `RelationGetIndexList`:

> «The index list is created only if someone requests it. We scan pg_index to
> find relevant indexes, and add the list to the relcache entry so that we
> won't have to compute it again. Note that shared cache inval of a relcache
> entry will delete the old list and set rd_indexvalid to false, so that we must
> recompute the index list on next request. This handles creation or deletion of
> an index.»

E o irmão exato do nosso caso — a lista de **chaves estrangeiras**,
`RelationGetFKeyList`, com o mesmo `if (relation->rd_fkeyvalid) return ...` e um
aviso que também é nosso:

> «CAUTION: the returned list is part of the relcache's data, and could vanish
> in a relcache entry reset. Callers must inspect or copy it before doing
> anything that might trigger a cache flush.»

**O que custa.** O manual não diz. O fonte mostra o preço escondido:
`RelationGetIndexList` **devolve uma cópia** (`list_copy`) justamente porque
quem chama vai fazer buscas no catálogo, e uma busca no catálogo pode processar
mensagens de invalidação que apagam a lista debaixo do chamador. **A bandeira
preguiçosa compra segurança na escrita e cobra cópia na leitura.**

**O que compraria aqui.** Trocar `Vec<usize>` por `Option<Vec<usize>>` e
invalidar em vez de refazer. **Não compra nada**, e o número é o §1.3: refazer
custa 12,6–81,4 ns e acontece três vezes por vida de tabela; invalidar custaria
o mesmo esforço de disciplina — uma linha por lugar que mexe no esquema. **É o
mesmo esquecimento com outro nome.** O que fecha o buraco não é a preguiça, é a
**porta única** (§4.1).

### 2.3 PostgreSQL(R) — o barramento de invalidação compartilhado (sinval)

**O que faz.** Cada processo tem o seu cache; a mudança de catálogo de um vira
mensagem para todos. `src/backend/utils/cache/inval.c`:

> «If we successfully complete the transaction, we have to broadcast all these
> invalidation events to other backends (via the SI message queue) so that they
> can flush obsolete entries from their caches.»

E a frase que qualquer projeto com cache deveria ter na parede, porque descreve
o defeito que nem o barramento resolve:

> «While building a higher-level cache entry, a backend may receive a callback
> for the being-built entry or one of its dependencies. This implies the new
> higher-level entry would be born stale, and it might remain stale for the life
> of the backend. **Many caches do not prevent that.**»

**O que custa.** O manual não diz; o fonte diz, e é caro.
`src/backend/storage/ipc/sinvaladt.c`: buffer circular de **`MAXNUMMESSAGES`
4096** entradas, dois LWLocks (`SInvalReadLock`, `SInvalWriteLock`), um
spinlock, interrupções de *catchup* para quem fica para trás, e um caminho de
desistência:

> «If the buffer does overflow, we recover by setting the "reset" flag for each
> backend that has fallen too far behind. … When it does finally attempt to
> receive inval messages, **it must discard all its invalidatable state**, since
> it won't know what it missed.»

**O que compraria aqui: nada, e o número é zero.** O barramento existe porque há
**N processos** com cache de longa vida sobre o mesmo catálogo. Aqui há **um**
processo, uma trava global de dados, e `Table` aberta e fechada **por pedido**
— conferido por varredura, não por memória: não existe `HashMap<_, Table>`,
`Vec<Table>` nem `Mutex<Table>` no `phxsql-server`, e as `Cargas` do
`BULKINSERT` reservam a tabela **pelo nome** (`carga.rs:86`, chave
`database/tabela`), não guardando handle nenhum.

**O número que o traria à vida:** mais de um processo com `Table` aberta sobre
o mesmo diretório. Não existe hoje.

### 2.4 MySQL(R) e PostgreSQL(R) — proibir a definição de mudar enquanto alguém usa

**O que faz.** MySQL fecha a janela por trava de metadado.
<https://dev.mysql.com/doc/refman/8.4/en/metadata-locking.html>:

> «To ensure transaction serializability, the server must not permit one session
> to perform a data definition language (DDL) statement on a table that is used
> in an uncompleted explicitly or implicitly started transaction in another
> session.»
>
> «The server achieves this by acquiring metadata locks on tables used within a
> transaction and deferring release of those locks until the transaction ends.
> **A metadata lock on a table prevents changes to the table's structure.**»

PostgreSQL faz o mesmo com o modo de trava mais forte.
<https://www.postgresql.org/docs/current/explicit-locking.html>:

> `ACCESS EXCLUSIVE` … «This mode guarantees that the holder is the only
> transaction accessing the table in any way.» … «Many forms of `ALTER INDEX`
> and `ALTER TABLE` also acquire a lock at this level.»

**O que custa.** Serialização: nenhum DDL enquanto alguém lê, nenhuma leitura
enquanto o DDL roda. O manual não põe número nisso.

**O que compraria aqui: já está feito, e por outro motivo.** O
`abrir_travada` (`servidor.rs:6278`) roda **dentro** de uma trava que quem
chamou já tomou, e o comentário dela explica que a trava tem de cobrir abrir E
gravar como um bloco só. Somado a «uma `Table` por pedido», a janela em que uma
definição muda debaixo de um handle **tem a duração de um pedido**, e dentro
dele ninguém mais escreve.

**O número que o tornaria obrigatório está medido, e ele é grande:** a sessão de
carga que mantém a tabela aberta entre pedidos vale **1.353 → 11 µs por linha**
(§1.1, terceira linha do medidor). No dia em que ela entrar, o handle passa a
atravessar pedidos, a janela deixa de ser de um pedido, e **este mecanismo sai
de «já feito» para «projeto»**.

### 2.5 MySQL(R)/MariaDB(R) — descartar, nunca remendar

**O que faz.** A definição em cache (`TABLE_SHARE`) nunca é atualizada no
lugar: ela é **marcada** e as instâncias que a usam são **fechadas e
reabertas**. MariaDB(R), `sql/table_cache.cc`
(<https://github.com/MariaDB/server/blob/main/sql/table_cache.cc>), invariante
escrita no cabeçalho do arquivo:

> «Table cache invariants: … `TABLE_SHARE::free_tables` shall not receive new
> objects if `TABLE_SHARE::tdc.flushed` is true»

`TDC_element::flush_unused` só faz `flushed = true` e remove as instâncias não
usadas; `tc_release_table` confere `table->needs_reopen() || table->s->tdc->flushed`
e, se qualquer um for verdade, **destrói a instância em vez de devolvê-la ao
cache**. O `flush` completo exige trava exclusiva de metadado e **espera** as
referências dos outros caírem (`wait_for_refs`).

No lado do usuário isso é o `FLUSH TABLES`.
<https://dev.mysql.com/doc/refman/8.4/en/flush.html>:

> «Closes all open tables, forces all tables in use to be closed, and flushes the
> prepared statement cache.»

**O que custa.** O manual não dá o tempo. Dá o **efeito colateral**, e ele é
medível na instalação de quem mede: um `FLUSH TABLES` obriga a **reanalisar**
todo *prepared statement* que tocava aquelas tabelas (§2.6).

**O que compraria aqui.** Na nossa escala «descartar» é o §2.1 — reabrir a
`Table`, **46,1–47,9 µs**. Mesmo veredito: morto hoje pelo preço, vivo se o
`self.ndx` virar estado que só a abertura sabe montar.

### 2.6 MySQL(R) — selo de versão conferido NO USO, e o erro que sobra quando não dá para reparar

**Este é o mecanismo que teria PEGO o defeito de hoje**, e é o único da lista
com essa propriedade. Os outros o evitariam por disciplina; este o **denuncia**.

**O que faz.** A definição em cache carrega uma versão. Quem guardou algo
derivado dela guardou também a versão. No uso seguinte, compara.
`sql/sql_base.cc`, `check_and_update_table_version`
(<https://github.com/mysql/mysql-server/blob/trunk/sql/sql_base.cc>):

> «Compare metadata versions of an element obtained from the table definition
> cache and its corresponding node in the parse tree. … At prepared statement
> execute, an observer may be installed. If there is a version mismatch, we push
> an error and return true.»

O que acontece depois do desencontro tem **dois andares**, e a distinção entre
eles é a parte que se aproveita:

**Andar 1 — dá para refazer: refaz calado.**
<https://dev.mysql.com/doc/refman/8.4/en/statement-caching.html>:

> «To avoid problems caused by metadata changes to tables or views referred to by
> the prepared statement, the server detects these changes and automatically
> reprepares the statement when it is next executed. That is, the server
> reparses the statement and rebuilds the internal structure.»
>
> «Reparsing also occurs after referenced tables or views are flushed from the
> table definition cache, either implicitly to make room for new entries in the
> cache, or explicitly due to `FLUSH TABLES`.»

O sinal interno é o `ER_NEED_REPREPARE` (`sql/sql_prepare.cc`), e o worklog
oficial diz por que ele não chega ao usuário —
<https://dev.mysql.com/worklog/task/?id=4166>:

> «However, in cases when the parsed tree is invalid, but SQL statement text
> itself is still meaningful in the new schema, the server should not return the
> error to the user. This is necessary to preserve backward compatibility with
> old 5.0 applications, which do not expect the new error, and also to provide
> continuous operation of the server in 24x7 environments.»

**Andar 2 — não dá para refazer: recusa alto.** É a mensagem clássica.
<https://dev.mysql.com/doc/mysql-errors/8.4/en/server-error-reference.html#error_er_table_def_changed>:

> Error number: `1412`; Symbol: `ER_TABLE_DEF_CHANGED`; SQLSTATE: `HY000`
>
> Message: **Table definition has changed, please retry transaction**

Ela **não** nasce no servidor: nasce no motor de armazenamento e é traduzida
por `sql/handler.cc`, que mapeia `HA_ERR_TABLE_DEF_CHANGED → ER_TABLE_DEF_CHANGED`
(<https://github.com/mysql/mysql-server/blob/trunk/sql/handler.cc>). Quem a
produz é o InnoDB quando a transação corrente é **velha demais** para o objeto
de dicionário reconstruído — o comentário do próprio teste do InnoDB é claro,
em `mysql-test/suite/innodb/t/innodb-index.test`
(<https://github.com/mysql/mysql-server/blob/trunk/mysql-test/suite/innodb/t/innodb-index.test>):

> «# t2i and t2c are too new for this transaction, because they were rebuilt»

**O que custa — e aqui o manual DIZ, que é raro.** Três coisas:

* o teto: «The server attempts reparsing up to three times. An error occurs if
  all attempts fail.» (o `MAX_REPREPARE_ATTEMPTS` do WL#4166);
* o medidor: «For prepared statements, the `Com_stmt_reprepare` status variable
  tracks the number of repreparations.» — ou seja, **o custo é observável na
  instalação**, o manual não o cita em microssegundos;
* o aviso: «Reparsing is automatic, but to the extent that it occurs, diminishes
  prepared statement and stored program performance.»

**O que compraria aqui, e é o item mais forte do documento.** Um `u64` de versão
no `Schema`, incrementado a cada escrita; cada lista derivada guarda a versão de
que nasceu; o portão confere antes de indexar. **Custo medido: 0,91–0,94 ns por
gravação (§1.3)** — o mesmo preço do portão que ele protege (0,65–0,98 ns), e
**0,006% a 0,02%** de uma inserção de 4,8–15,9 µs. Não muda formato em disco:
o selo é de memória, e o `PSCH` continua o que é.

O que ele compra pelo 1 ns: o pânico de `conferir_fks` viraria **erro nomeado**,
e — mais importante — o caso que **não** dá pânico. Diz a cognição do dia:

> «Se a lista tivesse sido **reordenada** em vez de encolhida, não haveria
> pânico: haveria a conferência da chave **errada**, calada.»

Chave errada conferida em silêncio é exatamente o que um `index out of bounds`
**não** protege, e é exatamente o que um selo de versão pega.

### 2.7 MySQL(R) 8.0 — tirar a segunda cópia da definição

**O que faz.** Antes da 8.0 a definição vivia em arquivo (`.frm`), em tabelas
não-transacionais e no dicionário do motor, ao mesmo tempo. A 8.0 apagou o
arquivo. <https://dev.mysql.com/doc/refman/8.4/en/data-dictionary.html>:

> «In previous MySQL releases, dictionary data was stored in metadata files,
> nontransactional tables, and storage engine-specific data dictionaries.»

<https://dev.mysql.com/doc/refman/8.4/en/data-dictionary-file-removal.html>:

> «Issues with file-based metadata storage included expensive file scans,
> susceptibility to file system-related bugs, complex code for handling of
> replication and crash recovery failure states, and a lack of extensibility that
> made it difficult to add metadata for new features and relational objects.»

**O que custa.** O manual **não diz** o preço da migração em número; lista os
problemas que ela resolveu.

**O que compraria aqui: nada, e a medição RECUSA.** A tradução direta seria
apagar `self.esquema` e ler sempre de `self.reg.esquema()`. O empecilho não é de
gosto: `RegFile::esquema()` devolve `&Schema`, e o caminho de gravação já segura
`&mut self.reg` — a cópia existe para que os dois empréstimos não briguem. E
**clonar sob demanda custa 3,0–3,8 µs numa tabela de 42 colunas (§1.3), 19% a
24% de uma inserção de 15,9 µs.**

**A cópia fica.** Recusa medida, e ela também explica por que a lição da 8.0
**não** é «não tenha cópia»: é «não tenha **duas verdades**». A nossa cópia é
uma só e vem de uma só origem; o que faltava era a **porta** por onde ela se
troca.

### 2.8 Os caches por tamanho — e o número que diz por que eles existem

**O que fazem.** MySQL e MariaDB mantêm dois caches distintos: o das
**instâncias abertas** e o das **definições**.
<https://dev.mysql.com/doc/refman/8.4/en/table-cache.html>:

> «The cache of open tables is kept at a level of `table_open_cache` entries.»

MariaDB(R),
<https://mariadb.com/docs/server/ha-and-performance/optimization-and-tuning/system-variables/optimizing-table_open_cache>:

> «`table_open_cache` indicates the maximum number of tables the server can keep
> open in any one table cache instance.»
>
> «When the server needs to open a table, it evicts the least recently used
> closed table from the cache, and adds the new table.»

E na MySQL 8.0+ há ainda um terceiro, do dicionário de dados.
<https://dev.mysql.com/doc/refman/8.4/en/data-dictionary-object-cache.html>:

> «The dictionary object cache is a shared global cache that stores previously
> accessed data dictionary objects in memory to enable object reuse and minimize
> disk I/O.»
>
> «The table definition cache partition exists in parallel with the table
> definition cache that is configured using the `table_definition_cache`
> configuration option. Both caches store table definitions but serve different
> parts of the MySQL server. Objects in one cache have no dependence on the
> existence of objects in the other.»

**O que custa.** O manual dá o custo em **descritores de arquivo**, não em
tempo: «If `table_open_cache` is set too high, MySQL may run out of file
descriptors and exhibit symptoms such as refusing connections or failing to
perform queries.» Não achei, na documentação oficial de nenhum dos três, o
custo em microssegundos de abrir uma tabela — é justamente o número que cada
casa mede na sua.

**O que compraria aqui.** O nosso está medido no §1.1: **abrir custa
46,1–47,9 µs; inserir com a tabela aberta custa 11,1–13,8 µs.** Abrir é
**3,3× a 4,3× uma inserção**, e um cache de handles pouparia isso por pedido.

**É a troca central deste documento, e é bom vê-la escrita:** *todo* o
maquinário dos §2.1 a §2.6 existe **porque** esses motores decidiram guardar a
tabela aberta entre pedidos. Quem não guarda, não precisa de nada disso. Quem
guarda 47 µs por pedido compra, junto, a obrigação de saber quando a definição
mudou.

---

## 3. As três colunas

| mecanismo | o que custaria aqui | o número que o mataria ou o confirmaria |
|---|---|---|
| **Selo de versão conferido no uso** (MySQL `check_and_update_table_version`, §2.6) | um `u64` no `Schema`, um `u64` em cada lista derivada, uma comparação antes de indexar | **CONFIRMADO: 0,91–0,94 ns medidos**, contra 4,8–15,9 µs de uma inserção (0,006%–0,02%). É o único da lista que **pega** o defeito em vez de evitá-lo por disciplina — inclusive o caso mudo, a chave **reordenada**, que o pânico não pega |
| **Apagar `fks_conferidas`** (a lição da §2.7 aplicada onde ela cabe) | trocar o portão guardado por `chaves_estrangeiras().iter().any(\|f\| f.verificar)`, e o laço por um `.filter(...)` sobre a mesma fatia | **CONFIRMADO: a lista compra 0,28–0,86 ns por linha** e custou um pânico. O portão sem estado derivado custa 0,92–1,37 ns e **não tem como envelhecer** |
| **Manter `colunas_marcadas`** | nada: já existe | **CONFIRMADO manter: compra 4,6–26,6 ns por linha** (o pior caso é a tabela larga sem coluna marcada). É 30× o que a lista de chaves compra, e é a diferença entre uma lista de 42 itens e uma de 0 a 3 |
| **Porta única para trocar o esquema** (a inversão do padrão da §2.1, sem o preço dela) | um `fn trocar_esquema(&mut self, novo: Schema)` privado; os 3 lugares passam a chamá-lo | **CONFIRMADO: refazer tudo custa 12,6–81,4 ns** numa operação que já escreve dois arquivos. O que mede o ganho é a **superfície**: hoje 3 lugares × N derivados; depois, 1 lugar. Erro observado antes do conserto: **1 em 3** |
| **Reconstruir a `Table` inteira** (PostgreSQL `RelationRebuildRelation`, §2.1 / MariaDB `flushed`, §2.5) | `Table::abrir` + repor os campos de sessão (o PostgreSQL repõe 12, e chama a lista de *hack*) | **MORTO hoje: 46,1–47,9 µs medidos, 580×–3.800× o custo de refazer as listas.** **VIVO no dia do pedido 175:** o índice criado na declaração deixa o `self.ndx` velho, e esse estado **não se recalcula do esquema** — só reabrindo. Aí os 47 µs viram o preço certo |
| **Bandeira de validade preguiçosa** (PostgreSQL `rd_fkeyvalid`, §2.2) | `Option<Vec<usize>>` no lugar de `Vec<usize>` | **MORTO: não resolve o problema.** Invalidar é a mesma linha por lugar que refazer, e o custo de refazer já é 12,6–81,4 ns. O que fecha o buraco é a porta única, não a preguiça. O número que o traria de volta: um derivado cujo cálculo custasse mais que uma abertura (47 µs) — não temos nenhum |
| **Barramento de invalidação** (PostgreSQL sinval, §2.3) | buffer circular de 4096 mensagens, dois LWLocks, *catchup*, caminho de *reset* | **MORTO: o número é zero.** Um processo, uma trava global, `Table` aberta e fechada por pedido — conferido por varredura: não há `HashMap<_, Table>` no servidor, e as `Cargas` reservam **pelo nome**. O que o traria à vida: mais de um processo com `Table` aberta no mesmo diretório |
| **Trancar a definição** (MySQL MDL / PostgreSQL `ACCESS EXCLUSIVE`, §2.4) | nada: o `abrir_travada` já roda dentro da trava de quem chamou | **JÁ FEITO, e vira PROJETO no dia da sessão de carga:** o handle que atravessa pedidos vale **1.353 → 11 µs por linha** (medido no §1.1) — e é ele que reabre a janela que hoje dura um pedido |
| **Tirar a segunda cópia do esquema** (MySQL 8.0, remoção dos `.frm`, §2.7) | ler sempre de `self.reg.esquema()`, clonando quando o empréstimo brigar | **MORTO, medido: 3,0–3,8 µs por clone** numa tabela de 42 colunas — **19% a 24% de uma inserção de 15,9 µs**. A cópia fica; o que muda é a porta |
| **Cache de `Table` entre pedidos** (`table_open_cache`, §2.8) | pouparia 46,1–47,9 µs por pedido, e **traria junto** toda a obrigação dos §2.1–§2.6 | **NÃO VIRA PLANO POR ESTE DOCUMENTO.** O ganho está medido (abrir custa 3,3×–4,3× uma inserção); o que falta é o outro lado: **quantos pedidos por segundo repetem a mesma tabela**. Sem esse número não dá para dizer se 47 µs por pedido valem o maquinário — e ele não existe. O medidor que o daria é o mesmo cronômetro dentro da trava que o `PESQUISA-TRAVA-E-MVCC.md` §1.1 já pediu |

---

## 4. O veredito

### 4.1 O que fizemos hoje já é o certo para o nosso tamanho — e o número diz isso

**Sim, e está medido.** O conserto do `86617f3` — *refazer TUDO o que deriva do
esquema, e não o que a operação parece tocar* — custa **12,6 a 81,4
nanossegundos**, numa operação que já reescreve o cabeçalho do `.reg` e regrava
o `.pag`. Não há mecanismo neste documento que compre mais barato.

Os outros três motores fazem coisa muito mais cara, e **fazem porque têm
problema muito maior**: 65 campos derivados contra 2, centenas de lugares que
mudam o catálogo contra 3, e dezenas de processos com cache de longa vida
contra um processo que abre e fecha a tabela por pedido. **Trazer o maquinário
deles seria pagar o preço de um problema que não temos.**

O que muda de mecanismo para **desenho**, e é onde a pesquisa paga:

> **A inversão da §2.1 é de graça, e é a lição que sobrevive à diferença de
> tamanho.** O PostgreSQL troca a entrada inteira e depois devolve as poucas
> exceções, porque assim **o esquecimento erra para o lado seguro**. Nós
> refazemos o que lembramos, e o esquecimento erra para o lado errado. A versão
> barata da mesma ideia é **uma porta só**: `self.esquema` deixa de ser
> atribuído em três lugares e passa a ser trocado por um método que refaz tudo
> o que deriva dele. Custo: os mesmos 12,6–81,4 ns. Superfície de erro: de três
> lugares para um.

### 4.2 As três coisas que valem mexer, em ordem de retorno por nanossegundo

1. **Apagar `fks_conferidas`.** Ela compra **0,28–0,86 ns por linha** e cobrou um
   pânico. O portão calculado direto do esquema custa **0,92–1,37 ns** e não tem
   como envelhecer, porque não existe. Sai um campo derivado de dois; sai
   metade da classe do defeito. *(Papel B e C; a decisão de formato é nula —
   nada em disco muda.)*
2. **A porta única para trocar o esquema.** 12,6–81,4 ns, três chamadas, e o
   próximo campo derivado nasce coberto. *(Papel B.)*
3. **O selo de versão.** 0,91–0,94 ns, e é o único que **denuncia** em vez de
   evitar — inclusive o caso mudo, a lista reordenada, que o `index out of
   bounds` não pega. *(Papel G: é guarda, e guarda nova entra com o defeito
   que a motivou escrito ao lado.)*

Nenhuma delas é grande, e é esse o resultado: **o problema é de tamanho de dois
campos, e a resposta tem de ter o tamanho do problema.**

### 4.3 A precedência que este documento confirma

O parecer do pedido 175 escreveu que o conserto do portão vem **antes** de
criar o índice na declaração. Esta pesquisa **confirma pelo outro lado**: o
índice criado na declaração acrescenta um estado derivado de natureza
diferente — o `self.ndx`, que **não se recalcula do esquema em memória**. Para
ele, o único mecanismo correto da lista é o mais caro, a **reconstrução do
handle inteiro, 46,1–47,9 µs**. Ordem certa: fechar o barato primeiro; abrir o
caro só quando o pedido 175 exigir.

---

## 5. O que eu concluí primeiro, e estava errado

Abri esta frente convencido de que o achado seria **a lista preguiçosa com
bandeira de validade** (`rd_fkeyvalid`, §2.2). O raciocínio parecia sólido:
invalidar é uma linha curta, refazer é uma linha longa, e o PostgreSQL — que
sabe do assunto — escolheu invalidar.

Está errado por dois motivos, e o segundo eu só vi medindo.

**O primeiro:** invalidar e refazer têm **a mesma superfície de esquecimento**.
Quem esqueceu de escrever `self.fks_conferidas = ...` teria esquecido de
escrever `self.fks_conferidas = None` — é uma linha por lugar nos dois casos. Eu
tinha confundido *«mais barato de executar»* com *«mais difícil de esquecer»*, e
elas não são a mesma propriedade.

**O segundo, que a medição deu:** a lista preguiçosa é uma otimização de
**cálculo caro**, e o nosso cálculo custa **12,6–81,4 ns**. O PostgreSQL faz
preguiça porque `RelationGetIndexList` **varre a `pg_index`** — é I/O de
catálogo, não um `filter` sobre um `Vec` que já está na memória. **Trouxe a
receita sem trazer a premissa dela**, que é exatamente o erro que o papel J
existe para não cometer.

O achado real veio de uma medida que eu nem tinha planejado fazer: quando medi
o clone do `Schema` (3,0–3,8 µs) para escrever a §2.7, o contraste com o custo
das listas (0,28–26,6 ns) deixou visível que **as duas listas derivadas não são
o mesmo animal** — uma paga o que custa, a outra não paga nada. Foi essa
medição, e não a leitura de nenhum dos três fontes, que produziu a única
recomendação de apagar código deste documento.

---

## 6. A fronteira de licença

Cumprida, e sem nada a decidir pelo dono.

* **MySQL(R) e MariaDB(R) são GPLv2.** Li `sql/sql_base.cc`, `sql/table_cache.cc`,
  `sql/handler.cc` e um teste do InnoDB **para entender**, e cito arquivo,
  função e comentário. **Nenhuma linha foi copiada para `crates/`**, e este
  documento não propõe copiar nenhuma.
* **PostgreSQL(R) tem licença permissiva** (estilo BSD). Li `relcache.c`,
  `inval.c`, `sinvaladt.c` e `rel.h`, e cito o mesmo jeito. **Também não
  proponho copiar**: o que atravessa é *ideia de desenho* — inverter o padrão da
  troca, conferir um selo no uso —, e ideia de desenho não é código.
* **Zero dependências externas continua valendo.** Nada aqui pede crate.

**Higiene de disco, porque havia ~4,3 GiB livres e outras frentes trabalhando:**
não clonei nenhum dos três repositórios. Baixei **8 arquivos avulsos por HTTP,
1.283.111 bytes medidos** (`du -sb`), no *scratchpad* da sessão, fora do
repositório. **Não medi** quanto custaria um clone raso dos três — não medi
porque não quis gastar o disco para descobrir, e digo isso em vez de citar um
número que não conferi.

---

## 7. De onde saiu cada coisa

### Documentação oficial consultada

| assunto | URL |
|---|---|
| MySQL(R), travas de metadado (MDL) | <https://dev.mysql.com/doc/refman/8.4/en/metadata-locking.html> |
| MySQL(R), cache de tabelas abertas | <https://dev.mysql.com/doc/refman/8.4/en/table-cache.html> |
| MySQL(R), cache de objetos do dicionário | <https://dev.mysql.com/doc/refman/8.4/en/data-dictionary-object-cache.html> |
| MySQL(R), dicionário de dados | <https://dev.mysql.com/doc/refman/8.4/en/data-dictionary.html> |
| MySQL(R), remoção do armazenamento em arquivo (`.frm`) | <https://dev.mysql.com/doc/refman/8.4/en/data-dictionary-file-removal.html> |
| MySQL(R), cache de *prepared statements* e reanálise automática | <https://dev.mysql.com/doc/refman/8.4/en/statement-caching.html> |
| MySQL(R), `ER_TABLE_DEF_CHANGED` (1412) | <https://dev.mysql.com/doc/mysql-errors/8.4/en/server-error-reference.html#error_er_table_def_changed> |
| MySQL(R), `FLUSH TABLES` | <https://dev.mysql.com/doc/refman/8.4/en/flush.html> |
| MySQL(R), WL#4166 — *Prepared statements: automatic re-prepare* | <https://dev.mysql.com/worklog/task/?id=4166> |
| MariaDB(R), `table_open_cache` | <https://mariadb.com/docs/server/ha-and-performance/optimization-and-tuning/system-variables/optimizing-table_open_cache> |
| PostgreSQL(R), modos de trava explícita (`ACCESS EXCLUSIVE`) | <https://www.postgresql.org/docs/current/explicit-locking.html> |

### Fontes lidas (para entender; nada copiado)

| arquivo | o que se leu | URL |
|---|---|---|
| `src/backend/utils/cache/relcache.c` | `RelationClearRelation`, `RelationRebuildRelation` e o `SWAPFIELD`; `RelationGetFKeyList`; `RelationGetIndexList` | <https://github.com/postgres/postgres/blob/master/src/backend/utils/cache/relcache.c> |
| `src/backend/utils/cache/inval.c` | o comentário de cabeçalho inteiro | <https://github.com/postgres/postgres/blob/master/src/backend/utils/cache/inval.c> |
| `src/backend/storage/ipc/sinvaladt.c` | o comentário de cabeçalho e as constantes da fila | <https://github.com/postgres/postgres/blob/master/src/backend/storage/ipc/sinvaladt.c> |
| `src/include/utils/rel.h` | contagem dos campos `rd_*` e das bandeiras de validade | <https://github.com/postgres/postgres/blob/master/src/include/utils/rel.h> |
| `sql/table_cache.cc` (MariaDB) | invariantes do TDC, `tc_release_table`, `TDC_element::flush`/`flush_unused` | <https://github.com/MariaDB/server/blob/main/sql/table_cache.cc> |
| `sql/sql_base.cc` (MySQL) | `check_and_update_table_version`, `ask_to_reprepare` | <https://github.com/mysql/mysql-server/blob/trunk/sql/sql_base.cc> |
| `sql/handler.cc` (MySQL) | mapa `HA_ERR_TABLE_DEF_CHANGED → ER_TABLE_DEF_CHANGED` | <https://github.com/mysql/mysql-server/blob/trunk/sql/handler.cc> |
| `mysql-test/suite/innodb/t/innodb-index.test` | o caso em que o InnoDB devolve o erro | <https://github.com/mysql/mysql-server/blob/trunk/mysql-test/suite/innodb/t/innodb-index.test> |

Os três foram lidos nos ramos `master` (PostgreSQL), `main` (MariaDB) e `trunk`
(MySQL), baixados em **03/09/2026**. **Não fixei o commit** — tentei pela API do
GitHub e ela não respondeu com o SHA nesta sessão. É limitação registrada: quem
reconferir pode ver texto diferente se o arquivo tiver mudado.

### Números desta casa, e de onde saíram

| número | de onde |
|---|---|
| 46,08 / 47,72 / 47,92 µs para abrir a tabela; 11,08 / 11,36 / 13,83 µs para inserir com ela aberta; 1.241 / 1.328 / 1.353 µs abrindo por linha | `./target/release/examples/custo-de-abrir 5000`, três amostras, 03/09/2026, `loadavg` 0,72 |
| 0,28–0,86 ns (portão FK), 4,6–26,6 ns (portão LGPD), 12,6–81,4 ns (refazer as duas listas), 0,91–0,94 ns (selo `u64`), 435,5–3.816,9 ns (clone do `Schema`) | medidor próprio, `std::hint::black_box`, 20 milhões de voltas por medida (2 milhões nas duas últimas), duas passadas, 03/09/2026, `loadavg` 0,25–0,31. **Escrito fora do repositório**, no *scratchpad* da sessão — ver a nota abaixo |
| 20 campos no `struct Table`, 2 derivados do esquema, 3 lugares que escrevem `self.esquema`, 1 deles esquecia | varredura do `crates/phxsql-store/src/table.rs`, comandos na tabela do §0 |
| 65 campos `rd_*` e 6 bandeiras de validade na `RelationData` | varredura do `src/include/utils/rel.h` baixado |
| 4,8 µs (só `.reg`) e 15,9 µs (dois índices) por inserção | `DESEMPENHO.md` §2 — **medição anterior, não refeita aqui** |
| 47,07 / 48,76 µs para abrir, com `loadavg` 4,39 | `PESQUISA-TRAVA-E-MVCC.md` §1.1 — **medição anterior**, reconferida no §1.1 deste documento |
| não há cache de `Table` no servidor; as `Cargas` reservam pelo nome | varredura do `crates/phxsql-server/src/`, e `carga.rs:86` |
| o pânico em `conferir_fks` (`table.rs:808` antes do conserto) e o alcance latente | `docs/cognicao/cognicao_o-portao-fica-velho-quando-o-esquema-muda_20260903_1705.md` e commit `86617f3` |

**Nota sobre o medidor do §1.2 e §1.3, e ela é uma dívida:** ele mora fora do
repositório, então **morre com a sessão** — o que contraria a regra de que
script que resolveu algo não pode morrer com a sessão. Ficou fora de propósito
nesta rodada, porque acrescentar um `example` ao `phxsql-store` é mexer no
`crates/` e esta frente é de pesquisa. **Se qualquer uma das três
recomendações do §4.2 virar trabalho, o medidor entra junto** como
`--example custo-do-portao`, e aí os números acima passam a se refazer em vez de
envelhecer. Enquanto isso, o que está aqui é a receita completa: o portão
guardado contra o portão calculado, `black_box` nos dois lados, e os três
esquemas do §1.2.
