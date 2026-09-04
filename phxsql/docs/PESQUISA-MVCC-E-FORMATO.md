# Pesquisa: onde os outros põem a versão velha, e o que o NOSSO formato aceita

> ## Este arquivo e o irmão dele
>
> Há um `docs/PESQUISA-TRAVA-E-MVCC.md` neste repositório. Ele **não** é versão
> antiga deste: é outro documento, vivo, e os dois se dividem assim:
>
> | | `PESQUISA-TRAVA-E-MVCC.md` (o irmão) | **este arquivo** |
> |---|---|---|
> | pergunta | como os outros separam **leitor de leitor** | **onde mora a versão velha**, e quanto isso custa no nosso disco |
> | entrega | os dois níveis de trava (mapeamento × conteúdo) | os **bytes**, a taxonomia, e as perguntas fechadas para o DBA |
>
> **Este arquivo nasceu chamado `PESQUISA-MVCC-E-TRAVA.md`** — o mesmo nome do
> irmão com as duas palavras trocadas de lugar. A frente que o escreveu nomeou
> a armadilha e pediu a troca, mas não a fez, porque o nome tinha vindo do
> orquestrador; o orquestrador renomeou na integração. É a mesma lei do dossiê
> («só existe um por vez, para que ninguém atualize o errado»), e a correção é
> a mesma: **o nome distingue pelo ASSUNTO — formato — e não por reordenar as
> palavras do irmão.** Nome que se distingue por ordem de palavra não distingue
> nada: quem procura de memória erra os dois.

Este documento é do papel **J (pesquisador)** e obedece à lei que dá sentido ao
papel: *receita de fora se mede contra o nosso gargalo antes de virar plano*.
Nenhuma receita entra por prestígio de motor. Cada uma sai com **o número nosso
que a confirma ou a mata**, e quando eu não sei medi-la, o documento diz isso em
vez de recomendá-la.

**Sobre licença, e a regra vale para o documento inteiro:** *a técnica não é o
código.* Ideia, algoritmo e desenho de formato não se protegem por copyright —
só a expressão em código. Então **nenhuma técnica saiu daqui por causa da
licença de quem a implementou**, inclusive as do InnoDB. O que não entra é
**código GPLv2 colado no repositório**, porque o `Cargo.toml` deste projeto
declara `license = "MIT OR Apache-2.0"` e colar GPL tornaria essa linha falsa —
que é o mesmo defeito de uma afirmação que o código desmente, com o agravante de
não se consertar reescrevendo a frase. Tudo aqui está **descrito com minhas
palavras a partir de documentação, wiki e papers**, com a licença anotada ao
lado como **informação para o dono, não como veto**.

---

> ## ⚠ A premissa de DESEMPENHO deste documento morreu medida (04/09, 06:03/06:13)
>
> Quem chegar aqui para justificar a Sombra tem de ler isto **antes** da §5.
>
> Duas baterias limpas de `escolher-o-desenho.py`, com o `quieta.Vigia`
> aprovando as duas, mediram os tetos que faltavam. O teto que o relatório
> imprime para o MVCC (leitor com um escritor ao lado contra o leitor sozinho)
> deu 1,19×–1,38× — mas **parte desse custo é de haver qualquer segundo
> cliente, e o `RwLock` já o recupera**. O que **só** o MVCC compra é a
> diferença entre um escritor ao lado e outro leitor ao lado, e ela deu
> **1,00× · 0,91× · 1,13× · 1,02×** — uma das corridas com o escritor **mais
> barato** que o leitor. **Indistinguível do ruído.**
>
> Na mesma bancada o `RwLock` deu **2,48×–2,99×** de vazão de leitura, com
> quatro medições da espera dentro de 7% umas das outras: o instrumento não é
> cego a diferenças reais.
>
> **Então a Sombra não se justifica por velocidade.** O que sobra, e é real, é
> a **leitura repetível** (§4.3 do `CONCORRENCIA.md`): uma varredura longa
> enxerga hoje linhas gravadas no meio dela, e nenhum `RwLock` conserta isso —
> ele torna os leitores simultâneos, não consistentes. É defeito de
> **resultado**, não de tempo, e nenhum p99 o mostraria.
>
> Tudo o que este documento diz sobre **onde a versão velha mora** continua
> valendo palavra por palavra: é trabalho de formato, e o formato não mudou.
> O que mudou é **por que** se faria. `docs/CONCORRENCIA.md` §11.

## 0. O que este documento acrescenta

O irmão já cobriu, e não se refaz aqui: as travas do WAL do SQLite, o buffer
pool particionado do InnoDB, o `innodb_thread_concurrency`, os
`NUM_BUFFER_PARTITIONS` do PostgreSQL(R) e a receita dos dois níveis de trava.

O que falta lá e este traz:

1. **A taxonomia**, e ela é a pergunta de formato com outro nome: os três lugares
   onde um motor pode pôr a versão velha, e **qual deles a nossa pétrea mata**.
2. **Os bytes.** Quanto o InnoDB gasta por linha, e quanto cabe no nosso slot —
   contado no fonte, porque o número que circulava aqui está errado.
3. **A área de undo já existe aqui e chama-se `.trash`** — e ela já pagou o
   problema mais difícil que uma área de undo tem.
4. **A undo daqui não desfaz nada**, porque a transação não grava antes do
   `COMMIT`. É o achado que muda a conta inteira.
5. **MVCC não entrega serializável.** Entrega *snapshot isolation*, que admite
   *write skew* — e a SP000016 se chama «MVCC e níveis de isolamento».
6. **O que já foi tentado e não fechou**: zheap e o `BEGIN CONCURRENT` do
   SQLite, os dois relevantes porque tentaram exatamente o que queremos.
7. **O que só apareceu abrindo o CÓDIGO** dos outros, e não o manual: que o
   ponteiro de undo de 7 bytes é um endereço **estruturado**; que o registro de
   undo guarda **delta** e não cópia; que o InnoDB guarda **prefixo + referência**
   da coluna externa em vez do conteúdo — o que aqui esbarra no `.bin`/`.memo`,
   que **reaproveita bloco liberado**; e que o `VACUUM` do PostgreSQL(R) reusa
   espaço de **três** maneiras, todas contra a nossa pétrea.
8. **A terceira resposta, a do Cassandra(R)** — não guardar versão velha e
   resolver na leitura —, conferida no fonte, recusada pelo motivo que o nosso
   `docs/CASSANDRA.md` já registrou, e **com a parte aproveitável nomeada**.
9. **E o desenho NOSSO**, a **Sombra** (§7), com dez divergências justificadas e a
   prova real nos dois sentidos. *Levantamento sem proposta é meio trabalho.*

---

## 1. Os nossos números, conferidos na fonte — e os que NÃO batem

*Número citado é número que não se mede.* Fui à fonte de cada um.

### 1.1 Os que batem

| fato | número | fonte conferida |
|---|---:|---|
| o `lock` sem disputa custa quase nada | **13,2 ns**, contra 3.456 µs do parse do lote — 262.000× | `docs/DESEMPENHO.md` §14 |
| a trava global come paralelismo já com 2 clientes | `ping` **1,99×** contra `varrer` **1,51–1,59×** e `inserir` **1,45–1,49×**, com a máquina 52% ociosa | `docs/DESEMPENHO.md` §14 |
| não há segundo gargalo embaixo da trava | tabelas separadas escalam igual: 1,70 contra 1,67 | `docs/DESEMPENHO.md` §14 |
| uma leitura segura a trava **23× mais** que uma gravação | `varrer(50)` **3.122–3.187 µs** contra `inserir` **121–137 µs**, em `por_lote` | `docs/CONCORRENCIA.md` §7.1 |
| o `fsync` sob a trava custa | **1.267–1.371 µs**, **10,3×–12,3×** | `docs/CONCORRENCIA.md` §7.1 |
| o gargalo da gravação é o índice | **83,5%** de uma inserção está no `.ndx`; o `.reg` custa **16,5%** | `docs/DESEMPENHO.md`, `--example onde-doi` |
| a trava não protege a `Instancia` | `struct Instancia { base: PathBuf }`, todos os métodos `&self` | `crates/phxsql-store/src/catalogo.rs` |
| o mapa da trava | **76** seções críticas | `bancada/concorrencia/mapa-da-trava.py` |

### 1.2 O que NÃO bate: o cabeçalho do slot **não** está cheio

O briefing desta frente e a §4.2 do `docs/CONCORRENCIA.md` dizem **«24 de 24
bytes usados»**. **Medido no fonte, está errado.**

```text
[status u8][flags u8][res u16][crc32 u32][versao u64][tempero u64]
    0          1        2..4     4..8       8..16       16..24
```
— `crates/phxsql-store/src/reg.rs` l. 34–35, `SLOT_CAB = 24`.

Varrido o `reg.rs` inteiro atrás de quem **escreve** e de quem **lê** cada faixa:

| faixa | quem escreve | quem lê | livre? |
|---|---|---|---|
| `status` 0 | l. 1573, e o excluir | l. 151, 1142, 1757, 1767, 1844 | não |
| **`flags` 1** | **ninguém** | **ninguém** | **SIM — 1 byte** |
| **`res` 2..4** | **ninguém** | **ninguém** | **SIM — 2 bytes** |
| `crc32` 4..8 | l. 1619 | l. 153, 1768 | não |
| `versao` 8..16 | l. 1574 | l. 1143, 1641, 1849 | não |
| **`tempero` 16..24** | l. 1586, **só dentro de `if material.cifrado()`** | l. 1642, **depois de `if !material.cifrado() { return }`** | **SIM na v4; não na v5** |

**O número certo: 3 bytes livres num `.reg` v5 (cifrado) e 11 num `.reg` v4 (em
claro).**

Isso não derruba a conclusão da §4.2 — troca o argumento dela por um melhor. «Está
cheio» é uma parede que encerra a conversa; **«tem 3 e o InnoDB gasta 13» é uma
conta**, e conta se discute. A correção está pedida ao dono da §4.2; este
documento não edita `CONCORRENCIA.md`.

### 1.3 O que ENVELHECEU entre o briefing e a fonte

**«O gap medido é leitor-com-leitor — a bateria rodou N leitores e nenhum
escritor.»** Valia até 03/09. Em **04/09** a §10.5 do `docs/CONCORRENCIA.md`
publicou a primeira bateria limpa com escritores, aprovada pelo `quieta.Vigia`:

| modo | 1 cliente | 2 clientes | 4 clientes |
|---|---:|---:|---:|
| `sem-trava` (controle) | 7.821 op/s | 14.529 (**1,86×**) | 28.774 (3,68×) |
| `ler` | 446 op/s | 703 (1,58×) | 719 (1,61×) |
| **`gravar`** | 900 op/s | 458 (**0,51×**) | 706 (0,79×) |
| `gravar-tabelas-separadas` | 985 op/s | 402 (**0,41×**) | 174 (**0,18×**) |

**O escritor não escala mal: ele REGRIDE.** Dois entregam metade do que um
entrega; quatro em tabelas separadas entregam 18%.

**E isso importa para a escolha de desenho porque nenhuma das três candidatas
explica uma regressão.** Trava por tabela, `RwLock` e MVCC **repartem** disputa;
nenhuma faz dois clientes renderem menos que um. Curva que **cai** com
concorrência tem outra causa — comboio na fila da trava, `fsync` amplificado
pelo intercalamento, ou o ruído de uma bateria de 1 s.

**Não medido, e é o que eu mediria antes de escolher desenho:** se a regressão
sobrevive a uma bateria longa. A própria §10.5 se protege dizendo que a bateria
é curta e **não substitui** a medição formal. Mas enquanto ela não for refeita,
**«o gap é leitor-com-leitor» não pode mais ser usada como se fosse a foto
inteira** — o número do escritor existe agora, e é pior.

### 1.4 Duas correções menores ao briefing

* **`phxsql/CLAUDE.md` não existe.** A lei do projeto é o `CLAUDE.md` da raiz.
* **A §10 do `CONCORRENCIA.md` não é só «o ruído do controle»** — a §10.5 carrega
  a tabela acima, que é o número novo. Quem procurar a §10 pela ementa passa por
  cima dele.

---

## 2. A taxonomia: os três lugares onde a versão velha pode morar

Esta é a nossa pergunta de formato, e ela tem nome na literatura. A classificação
canônica é a de Wu, Arulraj, Lin, Xian e Pavlo, *An Empirical Evaluation of
In-Memory Multi-Version Concurrency Control*, PVLDB 10(7), 2017
(<https://www.vldb.org/pvldb/vol10/p781-Wu.pdf>), repetida nas notas do curso
CMU 15-721 (<https://15721.courses.cs.cmu.edu/spring2020/notes/03-mvcc1.pdf>).
São três esquemas de **version storage**:

* **Append-only** — a versão nova é escrita **no mesmo espaço de armazenamento**
  da tabela, e as versões formam uma cadeia. É o PostgreSQL(R) e é o MySQL(R)
  na classificação do paper.
* **Time-travel** — as versões antigas vão para uma **estrutura separada**.
* **Delta** — guarda-se **só a diferença** entre versões, e não a cópia inteira.

E duas dimensões que decidem o custo:

* **ordenação da cadeia**: *oldest-to-newest* (O2N) ou *newest-to-oldest* (N2O).
  N2O acha depressa a versão corrente e cobra do leitor antigo; O2N faz o
  contrário. O paper chama o custo de **version chain length problem**.
* **ponteiro no índice**: **físico** (aponta para um lugar no disco, e então cada
  versão nova obriga a mexer no índice) ou **lógico** (aponta para um cabeçalho
  de versão, e o índice não muda).

> **Limitação medida, e digo qual:** o PDF do paper baixa (1,2 MB, HTTP 200), mas
> **este contêiner não tem `pdftotext` nem `poppler-utils`**, e o `WebFetch`
> recusou transcrever verbatim de *streams* comprimidos — a mensagem foi «I
> cannot reliably extract exact verbatim sentences from the compressed content
> streams without risking transcription errors». Então a taxonomia acima está
> **parafraseada do resumo do próprio fetch e das notas do CMU**, e não citada
> palavra a palavra. Não é «não consegui»: é «consegui o conteúdo, não consegui a
> citação literal, e o motivo é a falta de uma ferramenta local».

### 2.1 O mapa que interessa: cada motor contra a NOSSA pétrea

> **A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot excluído.
> **O `rowid` é ENDEREÇO** — `offset = data_offset + (rowid − 1) × slot_size` —,
> não identificador lógico.

| motor | onde põe a versão velha | esquema | **a nossa pétrea deixa?** |
|---|---|---|---|
| **PostgreSQL(R)** | tupla nova **no próprio heap**; a velha fica e o `VACUUM` limpa | append-only | **MATA.** Versão nova = slot novo = rowid novo. Quebra a ordem de digitação **e** a replicação por rowid |
| **Firebird / InterBase** | *back version* **na mesma página de dados**, guardada como **delta** | append-only + delta | **MATA.** Mesma página = mesmo arquivo de dados. O `.reg` é de slots de largura fixa: não há «resto da página» onde caiba uma back version |
| **MySQL(R) / InnoDB** | **undo tablespace separado** (`undo_001`, `undo_002`), só as colunas mudadas | delta, N2O | **DEIXA.** É o molde do nosso roteiro |
| **Oracle(R)** | **undo/rollback segments** separados; na linha ficam só o id de transação e o ponteiro de undo | delta / time-travel | **DEIXA.** E traz junto o `ORA-01555` |
| **SQL Server(R)** | *version store* no **`tempdb`** — um banco **recriado a cada arranque** (a partir da 2019 há também o PVS, persistente e opcional) | time-travel, **não durável** | **DEIXA**, e é o precedente da undo **em RAM** |
| **RocksDB** | na própria LSM, por *sequence number*; a *compaction* recolhe | append-only | **MATA.** Registro novo por versão, e ainda pede compactação — que é a SP000014 recusada pelo dono |
| **LMDB / Bolt** | páginas antigas do *copy-on-write*, devolvidas por uma **free list** | time-travel por página | **MATA duas vezes**: pede reuso de página (a pétrea) e muda o lugar da linha (o rowid-endereço) |
| **HANA / HyRISE** | delta em memória | delta | fora do nosso porte, mas é o precedente de delta puro |

**Leitura desta tabela, e é o resultado central da pesquisa:** das oito receitas,
**a nossa pétrea mata quatro**, e as quatro que sobrevivem são exatamente as que
põem a versão velha **fora do armazenamento principal**. Não é coincidência nem
sorte de escolha: *a família append-only é incompatível com «o `.reg` nunca
reaproveita slot» por construção*, porque append-only quer justamente escrever
versão onde a tabela mora, e a nossa tabela endereça por conta.

**O caminho anotado no `docs/ROTEIRO-1.0.md` — «cadeia de versões ancorada no
`rowid` + área de undo, no molde do InnoDB» — é, portanto, o único ramo da
taxonomia que sobrevive às nossas regras.** Isso é um resultado forte, e ele vale
mais que uma recomendação: quem quiser mudar de caminho tem de derrubar uma
pétrea primeiro, e a tabela diz qual.

### 2.2 O refinamento que os três de undo fazem, e nós não: **delta, não cópia**

Isto eu **li no fonte**, e não num artigo. O registro de undo de uma alteração,
em `storage/innobase/trx/trx0rec.cc` (MySQL(R) 8.x, **GPLv2 — lido, nada
copiado**), tem acima do laço este comentário do próprio autor:

> «Save to the undo log the old values of the columns **to be updated**.»

E o laço abaixo dele grava `n_updated` — a contagem das colunas mexidas — e
então, **por coluna mexida**, o número do campo e o valor **velho**. As colunas
que não mudaram não aparecem no registro.

**E o registro de undo de uma INSERÇÃO é ainda menor:** `trx_undo_page_report_insert`
grava só «the fields required to uniquely determine the record to be inserted»,
percorrendo `dict_index_get_n_unique(index)` — ou seja, **só a chave**. Faz
sentido e é elegante: para desfazer uma inserção basta saber qual linha apagar.

**O nosso `.trash` está do lado errado dessa frase.** Ele guarda **o payload
inteiro do slot, byte a byte, mais o conteúdo de cada coluna externa**
(`docs/FORMATO.md` §5). Para uma lixeira está certo — ela é uma foto para
restaurar. Para uma undo de MVCC é o **pior** formato possível: uma alteração que
mexe numa coluna de 4 bytes escreveria a linha inteira com os `Bin`/`Memo` junto.

**O que a forma delta compra:** uma alteração que não toca coluna externa **não
copia externo nenhum**, e o registro cai para `(coluna, valor velho)` das colunas
mexidas. Contra os **83,5% no `.ndx` e 16,5% no `.reg`** de uma escrita, o tamanho
do registro de undo decide se o MVCC entra pelo lado barato ou pelo caro.

### 2.3 E o que o InnoDB faz com as colunas externas — a resposta que eu não tinha

Esta é a parte que só apareceu abrindo o fonte, e ela **acrescenta uma terceira
opção** ao nosso problema do `Bin`/`Memo`.

`trx_undo_page_fetch_ext`, no mesmo arquivo, chama-se literalmente *«Fetch a
prefix of an externally stored column, for writing to the undo log»*. Ele **não
copia o BLOB**: copia um **prefixo** dele e, logo em seguida, **anexa os 20 bytes
da referência externa** (`BTR_EXTERN_FIELD_REF_SIZE`, que é `FIELD_REF_SIZE` em
`include/btr0types.h`, descrito ali como *«the size of a reference to data stored
on a different page»*). A função irmã chama-se *«Writes to the undo log a prefix
of an externally stored column»*.

**Ou seja: o InnoDB guarda um ponteiro para o BLOB velho, e conta com o BLOB
velho continuar existindo** — quem o libera é a purga, depois de ninguém mais
precisar dele (`trx0rec.cc`: *«free the old externally stored field»*).

**E é exatamente essa garantia que o nosso `.bin`/`.memo` NÃO dá hoje.** A nossa
própria especificação diz por que o `.trash` copia conteúdo em vez de ponteiro:

> «apontam para blocos que a própria exclusão acabou de liberar, e que a próxima
> inserção **pode reaproveitar**. A foto voltaria sendo a foto de outra linha.»
> — `docs/FORMATO.md`, «Por que não é um `.reg` paralelo»

> **O achado, e ele é uma pergunta de formato que ninguém tinha feito:** o
> `.bin`/`.memo` **reaproveita bloco liberado**. A pétrea «não reaproveita slot»
> vale para o `.reg`, **não** para os externos. Então a receita barata do InnoDB
> — ponteiro em vez de cópia — só funciona aqui se o `.bin`/`.memo` aprender a
> **não liberar** um bloco enquanto houver registro de undo apontando para ele.
> Isso é contagem de referência ou marca de fixação: **um terceiro item de
> formato**, além do `.reg` v6 e da marca do `.ndx`.


---

### 2.4 A terceira resposta, e ela é a do Cassandra(R): **não guardar versão velha — resolver na LEITURA**

> **Antes de escrever qualquer coisa nova sobre o Cassandra(R), li o que esta casa
> já tem:** `docs/CASSANDRA.md`, feito no fonte da **5.0.10**, commit `7b5ab44`,
> com caminho e linha em cada afirmação. Ele responde metade do que eu perguntaria
> e **já registra duas recusas** que atravessam esta pesquisa (§7.3 e §7.4 de lá).
> Esta seção **não o refaz** — cita, confere na fonte e **avança**.

A taxonomia da §2 tem três esquemas; o Cassandra(R) é o caso-limite de um deles.
**Toda escrita é uma versão, nenhuma versão é «velha», e ninguém guarda cadeia:**
a mutação vai para a memtable, depois para SSTables imutáveis, e **quem decide
qual valor é o certo é o caminho de LEITURA**, fundindo os candidatos por carimbo.

**Conferido no fonte hoje** (`cassandra-5.0`, `db/rows/Cells.java`, HTTP 200,
12.313 bytes, **Apache 2.0**): `resolveRegular` compara os dois carimbos e devolve
`leftTimestamp > rightTimestamp ? left : right`. **A afirmação do
`CASSANDRA.md` continua valendo, e é literal: o carimbo maior ganha e o menor
desaparece sem aviso.**

E a outra, também conferida (`db/commitlog/PeriodicCommitLogService.java`, HTTP
200, 1.876 bytes): o `maybeWaitForSync` **só** faz o escritor esperar quando a
sincronização já está atrasada (`lastSyncedAt < expectedSyncTime`); no caso normal
a escrita **não espera** `fsync` nenhum. **Continua valendo: no padrão, o OK deles
não quer dizer «está em N discos».**

#### O que isso nos custaria — e a recusa já está registrada, com o número

**Fora, e o `CASSANDRA.md` já disse por que, duas vezes:**

* **§7.3, «Escrever sem conferir unicidade»:** *«O `.reg` nunca reaproveita slot
  (`table.rs:758-762`), então aceitar primeiro e resolver depois deixa um buraco
  permanente por linha recusada.»* Aqui isso mataria a conferência de unicidade do
  `table.rs:763-770` e, com ela, **a regra primordial da integridade** — porque um
  `inserir` que não sabe recusar também não sabe recusar filha sem pai.
* **§7.4, «O carimbo do cliente decidindo o vencedor»:** *«Adotar o deles seria
  trocar uma recusa por uma perda silenciosa»* — o mesmo estrago que o `CLAUDE.md`
  registra sobre o merge de conflito, que marca **quem mexeu**, não quem chegou por
  último.

**Não repito a recusa: registro que ela já existe, e acrescento a consequência
para ESTA pergunta.** O Cassandra(R) não é um quarto lugar onde pôr a versão
velha; é a demonstração de que **existe um motor grande que não a põe em lugar
nenhum** — e o preço é exatamente aquilo que nós vendemos: *o `INSERT` deixa de
saber dizer não.*

#### E o que é APROVEITÁVEL, que é o que o dono pediu para eu dizer

Três coisas, e a primeira entra no nosso desenho (§7.8):

1. **A resolução no caminho de LEITURA é a forma certa, e é a forma que a Sombra
   já quer ter.** Um leitor que aterrissa num `rowid` e precisa decidir *qual
   versão eu vejo* está fazendo, em pequeno, o que o `Cells.resolve` faz entre
   memtable e SSTables. **A técnica se adota; o mecanismo diverge** — e a
   divergência está na §7.8.
2. **A sombra não se replica, e agora são TRÊS motores dizendo o mesmo.** O commit
   log deles é descartado quando a memtable descarrega e **não alimenta réplica
   nenhuma** (`CASSANDRA.md` §7.6); a undo do InnoDB exige que *«source and each
   replica must have its own undo tablespace file directory»*. A §4.1 do
   `CONCORRENCIA.md` chegou lá **por dedução**; hoje há duas confirmações
   independentes.
3. **O precedente sobre durabilidade, e ele é a favor da Sombra em RAM.** Se um
   motor grande aceita, **por padrão**, que o seu log de *recuperação* fique até
   10 s sem `fsync`, então a nossa sombra — que **não recupera nada**, só mostra
   versão a leitor vivo (§5.4) — aceitar **zero** durabilidade é um passo menor, e
   não maior.

#### E onde o Cassandra(R) nos quebraria, dito antes de alguém trazer

**A LSM com SSTable e compactação reescreve e reaproveita espaço.** Isso já morreu
aqui **medido**, duas vezes: no `CASSANDRA.md` §7.2 (*«a LSM não ataca o custo que
hoje domina, que é a codificação da linha»*) e na recusa da arquitetura de escrita
que chegou pronta, em que das dez propostas duas eram reais (`DESEMPENHO.md`).
**Qualquer receita que venha de lá tem de dizer como passa pela ordem de
digitação, ou entra aqui como recusa com motivo.** A da §2.4 entra como recusa.

---

## 3. Candidato 1 — trava por tabela

### Quem faz assim

**SQLite** trava o **arquivo inteiro** para escrever:

> «When any process wants to write, it must lock the entire database file for the
> duration of its update.» — <https://www.sqlite.org/faq.html> *(domínio público)*

No SQLite um arquivo é um banco; aqui um conjunto de dez arquivos é uma
**tabela**. **O análogo direto do SQLite no nosso desenho é a trava por tabela** —
não a global. *Hoje somos mais grossos que o SQLite, não mais finos.*

**InnoDB** vai a registro, e o manual diz por que pode:

> «Record locks always lock index records, even if a table is defined with no
> indexes. For such cases, `InnoDB` creates a hidden clustered index and uses this
> index for record locking.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-locking.html>
> *(documentação do produto; o código do InnoDB é GPLv2 — foi **lido para
> entender**, e **nada** dele está copiado aqui)*

A trava fina só existe porque há **registro de índice agrupado** onde pendurá-la.
Aqui a linha mora no `.reg` por conta aritmética e o `.ndx` é secundário: pendurar
no rowid é possível, mas é **inventar** estrutura, não copiar receita.

### Contra qual número NOSSO isso se mede

**Contra a replicação, e o número é do fabricante da receita:**

> «Auto-increment values are not ensured to be the same on the replicas as on the
> source if you use `innodb_autoinc_lock_mode` = 2.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-auto-increment-handling.html>

Aqui o `rowid` é `slot_count + 1`, o `aplicar_evento` **para** quando ele diverge,
e o `.log` — que carrega a imagem replicada — é **por tabela** (`EXTENSOES`,
`crates/phxsql-server/src/catalogo.rs` l. 352). A ordem que a réplica exige é
**total por tabela**, não total do servidor.

> **A tabela é a granularidade mais fina que a replicação desta casa aceita.**

**Confirma:** rodar `bancada/replicacao/` (modo A) com dois escritores em tabelas
diferentes e conferir que `aplicar_evento` não para. **Mata:** se parar.

**O que hoje NÃO a favorece:** nada. A §14 mediu tabelas separadas escalando
igual — **com a trava global**, o que é o previsto pela construção. *Trava por
tabela ainda não existe para ser medida.*

---

## 4. Candidato 2 — `RwLock`

### A armadilha do briefing: CONFIRMADA, e é pior do que está escrito

`crates/phxsql-store/src/catalogo.rs`, o tipo inteiro é `struct Instancia { base:
PathBuf }`, e `abrir_qualificada`, `abrir_tabela`, `abrir_database`, `databases`,
`todas_as_tabelas` — **e até `criar_database`** — são `&self`.

**`RwLock<Instancia>` compila de primeira, sem um erro, e está errado.** Nenhum
método pede `&mut`, então `.read()` em toda operação passa; dois escritores com
guarda de leitura abrem dois `Table` sobre os mesmos arquivos.

**O que eu acrescento: a armadilha não é o `RwLock`, é o tipo.** Um `RwLock`
protege o **valor**, e o valor aqui é um `PathBuf` imutável. O estado real está
nos **dez arquivos** da tabela, alcançados por um `Table` aberto e fechado a cada
operação. Trocar `Mutex` por `RwLock` sem antes **criar o estado** é trocar uma
ficha de exclusão por nenhuma.

O PostgreSQL(R) usa `LWLock` — que é exatamente um leitor-escritor — mas nunca
sobre um objeto sem estado, e ainda separa em dois níveis:

> «Each buffer header also contains an LWLock, the "buffer content lock", that
> *does* represent the right to access the data in the buffer.»
> — <https://github.com/postgres/postgres/blob/master/src/backend/storage/buffer/README>
> *(licença PostgreSQL, permissiva)*

### O primo optimista, e por que ele é o aviso mais útil desta seção

O SQLite tem um ramo — **`BEGIN CONCURRENT`** — que faz exatamente o que se
gostaria aqui: vários escritores em paralelo num motor de escritor único.

> «When a write-transaction is opened with "BEGIN CONCURRENT", actually locking
> the database is deferred until a COMMIT is executed.»
> «the system uses optimistic page-level-locking to prevent conflicting concurrent
> transactions from being committed.»
> «In order to serialize COMMIT processing, SQLite takes a lock on the database as
> part of each COMMIT command […] At most one writer may hold this lock at any one
> time.»
> — <https://www.sqlite.org/src/doc/begin-concurrent/doc/begin_concurrent.md>
> *(domínio público)*

**E o modo de falhar, que é o achado:**

> «writing two rows with adjacent values for "a" probably will cause a conflict
> (as the two keys are stored on the same page), but writing two rows with vastly
> different values for "a" will not» — e a receita para evitar isso é *«it is
> better to explicitly assign random values to INTEGER PRIMARY KEY fields»*,
> forçadas inserindo `9223372036854775807`.
> — mesma fonte

**Traduzido para cá, e é uma recusa dupla:**

1. **Conflito falso por vizinhança física.** Duas linhas que nada têm a ver
   colidem porque caíram na mesma página. É o mesmo defeito que o documento irmão
   recusou na partição por hash — *serialização que não está escrita em lugar
   nenhum do código, está no layout*.
2. **A receita do próprio SQLite para contornar é chave primária ALEATÓRIA**, e
   aqui o `rowid` é sequencial **por exigência da replicação** (§3). A saída que o
   dono da receita recomenda é proibida pela nossa pétrea.

E o ramo **nunca foi integrado ao SQLite principal**, o que é o dado mais honesto
sobre o tamanho do problema.

**E o escritor único do SQLite, conferido no fonte** (`src/wal.c` e `src/pager.c`,
**domínio público**): o `wal.c` define um `WAL_WRITE_LOCK` e o toma em modo
**exclusivo** (`walLockExclusive(pWal, WAL_WRITE_LOCK, 1)`) para anexar ao WAL; o
`pager.c` documenta no cabeçalho que *«An EXCLUSIVE lock is held on the database
file when writing»*. **Uma trava, exclusiva, uma por vez** — o mesmo desenho que
temos hoje, com a diferença de que lá ela é do arquivo e aqui é do servidor
inteiro.

### Contra qual número NOSSO isso se mede

**Contra os 23×.** Uma leitura segura a trava **3.122 µs**, uma gravação
**137 µs** (`CONCORRENCIA.md` §7.1). Separar leitor de leitor ataca ~96% do tempo
de posse — **se a carga for a do medidor**, e a §3.1 do `CONCORRENCIA.md` já avisa
que ela compara `varrer(50)` com `inserir(1)`.

**E agora há um segundo motivo para desconfiar:** o `gravar` regride (0,51×,
§1.3). Uma curva que regride não se explica por «96% do tempo é leitura».

**Mata:** se a contagem de chamadas por operação numa carga real mostrar que o
caminho quente não é o `varrer`. Isso o fonte não diz — pede telemetria por
operação, que hoje ninguém coleta.

---

## 5. Candidato 3 — MVCC, e a decisão de formato

### 5.1 O molde do InnoDB, com os bytes

> «`DB_TRX_ID`: 6 bytes […] `DB_ROLL_PTR`: 7 bytes […] The roll pointer points to
> an undo log record written to the rollback segment. If the row was updated, the
> undo log record contains the information necessary to rebuild the content of the
> row before it was updated. […] `DB_ROW_ID`: 6 bytes».
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-multi-versioning.html>

**Conferido no fonte, e não só no manual.** Em
`storage/innobase/include/data0type.h` (MySQL(R) 8.x, **GPLv2 — lido, nada
copiado**) os três tamanhos são constantes com asserção de compilação ao lado:
`DATA_ROW_ID_LEN = 6`, `DATA_TRX_ID_LEN = 6`, `DATA_ROLL_PTR_LEN = 7`, e logo
abaixo `static_assert(DATA_TRX_ID_LEN == 6)` e
`static_assert(DATA_ROLL_PTR_LEN == 7)`. *O fabricante trava esses números no
compilador — não são detalhe de implementação, são formato.*

**Preço do molde: 6 + 7 = 13 bytes por linha.** O `DB_ROW_ID` não conta para nós —
o nosso `rowid` já existe e já é estável, que é a boa notícia do roteiro.

E a decisão de visibilidade, descrita com minhas palavras a partir da
documentação e do que a comunidade publicou sobre o `ReadView`: o leitor guarda,
no instante em que abre, o **menor** e o **maior** id de transação ativos e a
**lista** dos ativos. Ao topar com uma linha, compara o `DB_TRX_ID` dela: mais
antigo que o menor → visível; igual ou maior que o maior, ou dentro da lista →
invisível, e então **desce a cadeia de undo** até achar a primeira versão visível.
*(Descrição da técnica; o código é GPLv2 e não foi copiado.)*

#### Como 7 bytes bastam: o ponteiro de undo é um ENDEREÇO ESTRUTURADO

Esta é a parte transferível, e ela não está no manual — está em
`include/trx0undo.ic`, na função que monta o ponteiro. Descrita com minhas
palavras: os 7 bytes **não** são um deslocamento cru de arquivo. São quatro
campos empacotados num inteiro de 56 bits — um **bit** dizendo se a undo é de
inserção, **7 bits** de identificador do segmento de undo, **32 bits** de número
de página e **16 bits** de deslocamento dentro da página (com uma asserção
`offset < 65536` logo acima, que é o que garante os 16 bits). A função irmã
desempacota os mesmos quatro.

> **A lição para nós, e ela muda a conta da §5.2:** um ponteiro de undo não
> precisa ser um `u64` de deslocamento. O nosso `.trash` já é **por volume**, e um
> par `(volume u16, deslocamento u32)` é **6 bytes** — menos que os 7 do InnoDB,
> porque não precisamos do bit de inserção (a nossa undo não desfaz nada, §5.4)
> nem de 128 segmentos.
>
> **Mesmo assim não cabe:** 6 bytes contra os **3** livres (§1.2). O déficit cai
> de 10 para **3 bytes** — e três bytes ainda são `slot_size` maior, ou seja
> `.reg` v6. *A conta muda; a conclusão não.*

#### E a decisão de visibilidade, lida no fonte

`include/read0types.h` traz o `ReadView` com os campos `m_low_limit_id`,
`m_up_limit_id`, `m_creator_trx_id` e a lista ordenada `m_ids`. O método
`changes_visible` decide em três degraus, e descrevo a lógica com minhas
palavras: id **abaixo** do limite inferior, ou o id do próprio criador da visão →
visível; id **igual ou acima** do limite superior → invisível; entre os dois →
faz busca binária na lista dos ativos, e é visível **se não estiver** nela. Não
achando visível, desce a cadeia de undo.

**O que isso custa a nós, e é uma linha de projeto que ninguém escreveu:** essa
decisão precisa de um **contador global de transações** e de uma **lista de
transações ativas**. Nenhum dos dois existe aqui — e o campo `versao u64` do
nosso slot **não serve**, porque é contador **por linha**, para a guarda de
conflito de escrita (`conferir_versao_pedida`), e não uma ordem global. A §4.2 do
`CONCORRENCIA.md` já avisava disso; o fonte do InnoDB mostra **exatamente** qual
estrutura falta.

E a purga é limitada pela visão mais antiga aberta: `trx0purge.cc` clona a visão
mais antiga (`clone_oldest_view`) e é ela que decide o que ainda não pode ser
recolhido — o mecanismo por trás da *history list* que o oráculo do roteiro viu
ir de 7 a 207.

Onde a undo mora, e a frase que confirma o nosso desenho:

> «The initial undo tablespace size is normally 16MiB.» […] **«In a replication
> environment, the source and each replica must have its own undo tablespace file
> directory.»**
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-undo-tablespaces.html>

A §4.1 do `docs/CONCORRENCIA.md` afirma **por dedução** que a área de undo é
estado local e não se replica. **A dedução está certa, e agora tem a palavra do
dono da receita atrás dela.** É a única frase deste documento que transforma uma
dedução nossa em fato citável.

### 5.2 Onde caberia o ponteiro de versão — a conta

| | bytes |
|---|---:|
| o que o InnoDB gasta por linha (`DB_TRX_ID` + `DB_ROLL_PTR`) | **13** |
| o que sobra no nosso slot, `.reg` **v5** (cifrado) | **3** |
| o que sobra no nosso slot, `.reg` **v4** (em claro) | **11** |

**(a) Não cabe — `.reg` v6, `slot_size` maior.** Faltam **10 bytes** se copiarmos
o molde do InnoDB inteiro (13 − 3), e **3 bytes** na versão enxuta que o fonte do
`trx0undo.ic` sugere: identificador de transação de 4 bytes mais ponteiro
estruturado `(volume u16, deslocamento u32)` de 6 = **10 − 3 livres = 3 de
déficit**. *Três bytes ou dez, é `slot_size` maior do mesmo jeito.*
O mecanismo está **provado**: a v4 virou v5 crescendo o `slot_size` em 16 bytes, e
`offset = data_offset + (rowid − 1) × slot_size` continuou valendo; o byte 8 do
cabeçalho decide quantos bytes ler, e um `.reg` v4 se lê byte a byte como antes
(`docs/FORMATO.md`, «O slot cifrado (versão 5)»). **Custa** reescrever toda tabela
existente **slot a slot, na mesma ordem** — o caminho do `acrescentar_coluna`
(`FORMATO.md` §1.1). A ordem de digitação sobrevive à migração; o tempo de parada,
não.

**(b) Cabe nos 11 bytes da v4 — e é armadilha. RECUSADA.** Um ponteiro de 8 bytes
cabe folgado no `tempero`, e o custo de formato seria zero. Mas o `tempero` nasce
no dia em que a tabela **liga cifra de coluna** (l. 1580–1586 do `reg.rs`), e essa
tabela teria de migrar **depois** — que é a migração cara, exatamente o que a
pétrea «mudança de formato entra cedo» existe para evitar.

**(c) Os 3 bytes dão um índice, não um endereço.** Chegam para «esta linha tem
versão anterior?» mais um índice de 16 bits num mapa em RAM. Não chegam para um
deslocamento de arquivo. É o que abre a §5.4.

### 5.3 A área de undo já existe aqui, e chama-se `.trash`

Este achado não veio de motor nenhum — veio da nossa própria especificação. O
`docs/FORMATO.md` §5 descreve o `.trash`: registro de **tamanho variável**
(cabeçalho de 56 bytes + **o payload do slot byte a byte** + o conteúdo de cada
coluna externa), ancorado no **rowid que a linha tinha**, **fora** do `.reg`, em
volumes próprios, sempre anexado, com **7 bytes reservados** no cabeçalho.

**Estruturalmente já é uma área de undo.** E ela já pagou o problema mais difícil
que uma área de undo tem, com o motivo escrito:

> «Copiar só o *payload* para um `.reg` paralelo guardaria os ponteiros — que
> apontam para blocos que a própria exclusão acabou de liberar, e que a próxima
> inserção pode reaproveitar. **A foto voltaria sendo a foto de outra linha.**»
> — `docs/FORMATO.md`, «Por que não é um `.reg` paralelo»

**Traduzido para MVCC:** uma versão velha **não pode** guardar o ponteiro de 16
bytes que o `.reg` guarda para uma coluna `Bin`/`Memo` — tem de guardar o
**conteúdo**, ou tem de garantir que o bloco velho sobreviva. O InnoDB tem o mesmo
problema e o resolve pelo segundo caminho (o BLOB antigo fica vivo até a purga).

> **Quem projetar a undo daqui e copiar só o payload de largura fixa vai produzir
> versões velhas que mostram o `Memo` de outra linha — e o CRC vai aprovar.**

**E agora há três saídas, não duas** (a terceira veio do fonte do InnoDB, §2.3):

| saída | custo | quem faz |
|---|---|---|
| **copiar o conteúdo** do externo para dentro do registro de undo | seguro, e caro: um `Memo` de megabytes viaja numa alteração que mexeu num inteiro | o nosso `.trash` hoje |
| **guardar prefixo + referência**, e **impedir** que o bloco velho seja liberado enquanto houver undo apontando | barato de escrever; **exige contagem de referência ou marca de fixação no `.bin`/`.memo`** — um terceiro item de formato | o InnoDB |
| **recusar MVCC em linha com externo alterado** | zero de formato; recusa cedo e por escrito, como o `ao_excluir` | ninguém — é saída nossa |

Este parágrafo custou zero de pesquisa externa. Estava escrito na nossa
especificação, num arquivo que ninguém abre quando pensa em MVCC.

### 5.4 O achado que muda a conta: a undo daqui NÃO desfaz nada

No InnoDB a undo faz **duas** coisas, e o manual as separa:

> «Insert undo logs are needed only in transaction rollback and can be discarded
> as soon as the transaction commits. Update undo logs are used also in consistent
> reads, and can be discarded only after there is no transaction present for which
> InnoDB has assigned a snapshot […]»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-multi-versioning.html>

**Aqui só a segunda metade existe:**

> «Serve de auditoria, e não de journal para desfazer — e não precisa ser: **a
> transação não desfaz escrita gravada, porque não grava nada antes do
> `COMMIT`**.» — `docs/FORMATO.md` §17

Confirmado por soquete na `docs/PENDENCIAS.md` (pedido 157): *«a escrita fica fora
da tabela até o commit e a leitura vai na tabela»*.

> **A área de undo do PhxSql serviria só para visibilidade de leitor vivo, nunca
> para desfazer. Logo não precisa sobreviver a uma queda** — um leitor que morre
> na queda não tem a quem mostrar a versão velha.

**E há prior art para isso, num motor grande:** o SQL Server(R) guarda o *version
store* no **`tempdb`**, que é **recriado a cada arranque**; só a partir da 2019, e
**por opção**, existe o *Persistent Version Store* dentro do banco do usuário
(<https://learn.microsoft.com/en-us/sql/relational-databases/databases/tempdb-database>).
*Não durável é uma escolha de projeto que um motor comercial faz há duas décadas,
e não um atalho.*

**Por que isso muda tanto:** no InnoDB a undo é durável de graça, porque as
mudanças ao rollback segment vão para o **redo log** — o WAL que ele tem. **Nós
não temos WAL** (`FORMATO.md` §17: «exigiria um journal de páginas, que é o WAL
que este desenho não tem»). Uma undo durável aqui pagaria o próprio `fsync`:

| | trava presa numa gravação |
|---|---:|
| hoje, `por_lote` (`fsync` a cada 200 operações) | **121–137 µs** |
| com barreira de durabilidade por operação, medido | **1.404–1.492 µs** |
| a barreira | **1.267–1.371 µs — 10,3× a 12,3×** |

— `docs/CONCORRENCIA.md` §7.1.

**Uma undo durável levaria a gravação de 23× mais barata que a leitura para ~2,2×
mais barata** (1.404 contra 3.122 µs) — e isso antes de contar que seriam **dois**
arquivos a sincronizar. **Uma undo não-durável custa zero disso.** É a diferença
entre `.reg` v6 obrigatório e um mapa em RAM.

### 5.5 A amplificação de escrita, medida contra os 83,5%

Robert Haas, do núcleo do PostgreSQL(R), nomeia o preço do caminho de undo:

> «performing an update means writing two tuples — the old one must be copied to
> the undo tablespace, and the new one must be written in its place»
> — <http://rhaas.blogspot.com/2011/02/mysql-vs-postgresql-part-2-vacuum-vs.html>
> *(artigo de terceiro, autor do projeto; comparação, não especificação)*

E o outro lado da mesma moeda:

> «Under InnoDB, most of the bloat […] is in the rollback tablespace», enquanto
> «in PostgreSQL it's mixed in with the actual table data». — mesma fonte

**Contra o nosso número:** uma inserção gasta **83,5% no `.ndx`** e **16,5% no
`.reg`**. O segundo *tuple* que a undo obriga a escrever é um **append** num
arquivo, sem índice — cai no lado barato. **Mas só se a undo for delta** (§2.2):
copiar payload inteiro mais externos põe um `Memo` de megabytes no caminho quente
de uma alteração que mexeu num inteiro.

**O número que fecharia:** medir quanto a trava fica presa num `excluir` — a única
operação que **já** escreve versão velha em arquivo separado e **já** sincroniza
(§7).

### 5.6 O `.ndx` continua sem noção de visibilidade

A entrada de folha do `.ndx` é `chave completa + rowid`, `ck_len = key_len + 8`
(`docs/FORMATO.md` §2). **Não há bit onde marcar visibilidade** sem mudar a
largura da entrada — **segundo** formato a versionar.

O InnoDB documenta como resolve, e o preço:

> «When a secondary index column is updated, old secondary index records are
> delete-marked, new records are inserted, and delete-marked records are
> eventually purged. […] However, delete-marked records are not used to cover the
> query using the index.»
> — <https://dev.mysql.com/doc/refman/8.4/en/innodb-multi-versioning.html>

Duas coisas caem daqui: **(1)** o índice precisa de marca de apagado; **(2)** toda
leitura que topa numa marca volta ao `.reg` — o índice deixa de responder sozinho.
Aqui isso é descer o `.ndx` **e** ler o slot, e o `.ndx` é onde estão **83,5%** do
custo de uma escrita.

**E é exatamente aqui que o zheap parou** (§6.2). *«Cadeia de versões ancorada no
rowid + área de undo» não menciona o `.ndx`, e o `.ndx` é a metade grande.*

### 5.7 MVCC não entrega serializável — e a SP000016 se chama «MVCC e níveis de isolamento»

Esta é a lacuna que a pesquisa achou no **nome** do item, e ela não é semântica.

> «The Repeatable Read isolation level is implemented using a technique known in
> academic database literature and in some other database products as *Snapshot
> Isolation*.»
> «The Repeatable Read mode provides a rigorous guarantee that each transaction
> sees a completely stable view of the database. **However, this view will not
> necessarily always be consistent with some serial (one at a time) execution** of
> concurrent transactions of the same level.»
> — <https://www.postgresql.org/docs/current/transaction-iso.html> *(permissiva)*

O exemplo trabalhado é o **write skew**: A soma `class = 1` e insere na `class 2`;
B soma `class = 2` e insere na `class 1`. As duas leem estado consistente, as duas
gravam, e **nenhuma ordem serial produz aquele resultado**. Sob *Repeatable Read*
as duas cometem; sob *Serializable* uma cai com «could not serialize access due to
read/write dependencies among transactions».

O nome formal da anomalia é **A5B — Write Skew**, de Berenson, Bernstein, Gray,
Melton e O'Neil, *A Critique of ANSI SQL Isolation Levels*, SIGMOD 1995
(<https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/tr-95-51.pdf>,
também em <https://arxiv.org/pdf/cs/0701157>) — o paper que **definiu** snapshot
isolation e, no mesmo texto, mostrou que ela admite anomalia.

E o preço de consertar está publicado: Ports e Grittner, *Serializable Snapshot
Isolation in PostgreSQL*, PVLDB 5(12), 2012 (<https://arxiv.org/abs/1208.4179>).
A SSI rastreia **rw-antidependências** procurando *dangerous structures* — um par
de dependências rw adjacentes entre três transações — e para isso os autores
precisaram de **um gestor de travas novo** e de **uma técnica para limitar a
memória**. E ela **aborta**: o PostgreSQL(R) avisa que *«applications using this
level must be prepared to retry transactions due to serialization failures»*.

**O que isto significa para o roteiro, e é uma pergunta, não uma objeção:**

> A SP000016 entrega **snapshot isolation**. Chamar o item de «MVCC e níveis de
> isolamento» sugere que o `SERIALIZABLE` vem junto. **Não vem** — vem depois, com
> um gestor de travas novo, um teto de memória e um cliente que sabe repetir a
> transação. Isto precisa estar escrito **antes** de a sprint começar, ou o
> `SERIALIZABLE` vira a metade que se descobre no fim.

### 5.8 O custo que ninguém contou: a décima primeira extensão

Uma tabela tem **dez** arquivos hoje —
`reg, ndx, bin, memo, log, bkp, trash, reason, pag, lgpd` — em **duas** listas com
respostas **diferentes** (`EXTENSOES`, o que uma cópia leva; `EXTENSOES_TODAS`, o
que um `excluir_tabela` apaga), `crates/phxsql-server/src/catalogo.rs` l. 352 e
377. O comentário acima delas é um catálogo de defeitos **repetidos**:

> «A lista de apagar tinha SEIS extensoes e a tabela ja tinha NOVE. […] O `.lgpd`
> entrou nesta lista DEPOIS de o dono olhar a tela e contar dez arquivos onde a
> frase dizia cinco. […] exatamente o mesmo defeito […] repetido pela mesma razão:
> extensao nova entra no motor e ninguem volta aqui.»

**Duas vezes seguidas, e na segunda o que ficava para trás era a trilha de dados
pessoais sob um nome que não existe mais.** Um `.undo` seria a **décima primeira**.
Não é motivo para não fazer — é um item de projeto que a frase «área de undo fora
do `.reg`» esconde.

---

## 6. Avaliado e RECUSADO, com o motivo

Recusa com motivo impede a mesma proposta de voltar.

### 6.1 PostgreSQL(R) — versão nova no próprio heap. **MORTA NA PÉTREA.**

> **Table 66.4. HeapTupleHeaderData** — `t_xmin` 4, `t_xmax` 4, `t_cid`/`t_xvac`
> 4, `t_ctid` 6 bytes; «There is a fixed-size header (occupying **23 bytes** on
> most machines)»; `t_ctid` é o «current TID of this or newer row version».
> — <https://www.postgresql.org/docs/current/storage-page-layout.html>

Uma atualização escreve **tupla nova no heap**. Traduzido: **slot novo no `.reg`
por versão**, que é o desenho que a §11.1 do `docs/TRANSACOES.md` recusa — e recusa
com razão: quebra a ordem de digitação **e** a replicação por rowid.

**Recusada. Custo real: infinito, porque o preço é uma pétrea.** O número ao lado é
23 — bytes de cabeçalho por versão, num lugar onde cada versão custa um endereço
que a réplica precisa reproduzir.

**E o que ela cobra de quem a adota: o `VACUUM` — e ele é RECUSA MEDIDA contra a
nossa ordem de digitação, agora lida no fonte.**

Em `src/include/access/htup_details.h` o comentário do `t_ctid` diz, com todas as
letras, que ao ser atualizada *«its t_ctid is changed to point to the replacement
version of the tuple»* — a versão nova é outra tupla, com outro endereço, e a
velha aponta para ela. É a nossa morte por rowid, escrita pelo projeto.

E o que vem depois, em `src/backend/access/heap/pruneheap.c` e
`src/backend/access/heap/vacuumlazy.c` (**licença PostgreSQL, permissiva**), são
**três formas de reusar espaço**, e as três batem na pétrea:

1. **Reuso de ponteiro de linha.** A poda classifica cada item em
   `LP_REDIRECT`, `LP_DEAD` ou **`LP_UNUSED`**, e o próprio `PruneState` carrega
   um campo chamado `mark_unused_now` — *«whether or not dead items can be set
   LP_UNUSED during pruning»*. A segunda passada do vacuum faz o mesmo:
   *«Setting LP_DEAD to LP_UNUSED in vacuum's second pass»*. **`LP_UNUSED` é
   exatamente o slot excluído voltando ao uso.**
2. **Mapa de espaço livre.** O vacuum chama `RecordPageWithFreeSpace` e
   `FreeSpaceMapVacuumRange`: o espaço liberado é publicado para a próxima
   inserção achar.
3. **Truncar a relação.** `lazy_truncate_heap`, ligada por `do_rel_truncate` —
   o arquivo encolhe.

> **As três são a SP000014**, que o dono **recusou**. Não é que o `VACUUM` seja
> caro para nós: é que ele **não pode existir aqui**, e sem ele o desenho do
> PostgreSQL(R) não fecha — versão velha entraria e nunca sairia. *Recusa medida,
> com o nome das três funções.*

O `HOT` (*heap-only tuples*) alivia só quando a atualização **não** toca coluna
indexada **e** a versão nova **cabe na mesma página**
(<https://wiki.postgresql.org/wiki/Heap_HOT_Selective_Index_Updates>) — e ele
depende do `LP_REDIRECT`, que é reuso de ponteiro outra vez. Nenhuma das
condições existe num heap de slots de largura fixa endereçados por conta.

### 6.2 zheap — a nossa própria receita, tentada por outros, **PARADA em cima do índice**

O zheap é a tentativa do PostgreSQL(R) de sair de (6.1) e ir **exatamente para
onde o nosso roteiro aponta**: só a versão corrente no armazenamento principal, as
velhas numa área de undo.

Do wiki do projeto (<https://wiki.postgresql.org/wiki/Zheap>): vantagens —
«inplace updates even when index columns are updated», evitar «the need for a
dedicated vacuum process to perform retail deletes». Pendências declaradas pelos
próprios autores — «**Delete marking in indexes**» e «More testing is needed for
recovery and rollbacks. We will not be surprised if we see some issues in that
area.»

Estado: o repositório da EnterpriseDB parou em 2019; a integração com a Pluggable
Storage API seguia incompleta em dezembro de 2021; a CYBERTEC assumiu o
financiamento depois (<https://www.cybertec-postgresql.com/en/postgresql-zheap-current-status/>,
<https://pgpedia.info/z/zheap.html> — *terceiros, citados como estado do projeto*).

**Isto NÃO recusa a nossa receita. Mede o tamanho dela.** Um time financiado,
dentro do motor que mais entende de armazenamento aberto, passou anos e não
fechou — e o que ficou pendente foi **exatamente a nossa §5.6**: *delete marking in
indexes*.

**O que isto recusa, concretamente:** recusa **tratar a SP000016 como uma sprint**.
Ela é um bloco, e a parte do índice é a metade grande.

### 6.3 Firebird / InterBase — *back version* na mesma página. **MORTA NA PÉTREA.**

Guarda a versão anterior como **delta**, mas **dentro do banco**, junto do
registro — e por isso «back versions […] must be removed one at a time, as they
are encountered»
(<https://firebirdsql.org/file/documentation/papers_presentations/Multi_version_concurrency_control.pdf>,
licença IDPL/documentação do projeto).

**O delta é a parte boa e nós a queremos (§2.2). O lugar é a parte que morre:** o
`.reg` é heap de slots de **largura fixa** — não existe «resto de página» onde uma
back version caiba, e pôr uma no slot seguinte é a morte de (6.1).

### 6.4 LMDB e Bolt — escritor único com COW. **RECUSADOS, por dois motivos nossos.**

São o motor mais próximo do nosso na disciplina, e por isso valia olhar.

> «Bolt allows only one read-write transaction at a time but allows as many
> read-only transactions as you want at a time.»
> — <https://github.com/boltdb/bolt> *(MIT — permissiva)*

O LMDB (OpenLDAP Public License — permissiva) tem a mesma disciplina, por
**copy-on-write** num B+tree, com uma **segunda árvore para as páginas livres**
(<https://en.wikipedia.org/wiki/Lightning_Memory-Mapped_Database>,
<https://dbdb.io/db/lmdb>).

**Motivo 1 — o COW depende de reuso, e nós não reusamos.** O Bolt escreve o preço
de não conseguir reciclar: páginas antigas não voltam enquanto transações antigas
estiverem abertas, o banco cresce, e «deleting large chunks of data will not allow
you to reclaim that space on disk». **A nossa pétrea é a versão permanente desse
defeito**, e a SP000014 (reuso/`VACUUM`/compactação) está **recusada pelo dono**.
Adotar COW seria adotar o custo do Bolt **sem** a free list que o compensa.

**Motivo 2 — o COW muda o endereço da linha.** Página nova + caminho até a raiz
reescrito = a linha muda de lugar. Aqui o `rowid` **é** o endereço.

**O que fica deles, e é de graça:** a disciplina «um escritor de cada vez, leitores
à vontade» **não precisa de COW**. É o que o candidato 2 entrega com o formato
intacto. **O LMDB confirma o alvo e recusa o mecanismo.**

### 6.5 RocksDB / LSM — versão por *sequence number*. **MORTA DUAS VEZES.**

Cada escrita ganha um número de sequência; o *snapshot* fixa um número; a
*compaction* recolhe as versões que nenhum snapshot vivo precisa
(<https://github.com/facebook/rocksdb/wiki/Snapshot>, licença Apache-2.0 / GPLv2 —
**permissiva no ramo Apache**).

**Morte 1:** cada versão é um **registro novo** — a família append-only da §2.1.
**Morte 2:** o mecanismo depende de **compaction**, que é reescrever e reordenar o
armazenamento — a SP000014 recusada.

**E há uma recusa anterior, já registrada nesta casa e que este item confirma:** a
arquitetura de escrita que chegou pronta (WAL, group commit, MemTable, LSM) foi
medida contra o nosso gargalo, e das dez propostas **duas eram reais**
(`docs/DESEMPENHO.md`). *A LSM não volta como resposta ao MVCC pelo mesmo motivo
por que não voltou como resposta à escrita.*

### 6.6 SQLite `BEGIN CONCURRENT` — otimista com detecção por página. **RECUSADO.**

Conflito falso por vizinhança física, e a receita do próprio SQLite para
contorná-lo é **chave primária aleatória** — proibida aqui pela replicação (§4).
Nunca integrado ao SQLite principal.

### 6.7 Ponteiro de versão no `tempero` (v4). **RECUSADO.** §5.2(b).

### 6.8 Undo durável com `fsync` próprio. **RECUSADO com número, salvo ordem do DBA.**

Custa **1.267–1.371 µs** por gravação e leva o tempo de posse da trava numa
escrita de **137 µs para 1.404+ µs**. Compraria durabilidade para um desfazer que
**esta casa não faz** (§5.4). **Reviveria se e somente se** a transação passar a
gravar antes do `COMMIT`.

### 6.9 Trava mais fina que a tabela. **RECUSADA, com a palavra do fabricante.**

Página, linha, partição do `.reg`: todas quebram a ordem total por tabela que a
réplica exige. O InnoDB nomeia o preço no manual dele
(`innodb_autoinc_lock_mode=2`). **A tabela é o piso.**

### 6.10 O que eu NÃO consigo medir, e por isso não recomendo

**Trava por linha.** Não há registro de índice agrupado onde pendurá-la, e não
existe experimento barato que separe «trava por linha ajudaria» de «trava por
tabela ajudaria» antes de uma das duas existir. *(Mesma conclusão do documento
irmão, por outro caminho — e a coincidência é o que dá confiança nela.)*

---

## 7. O NOSSO desenho: a **Sombra** — e as DEZ divergências que o tornam nosso

Levantamento sem proposta é meio trabalho. Esta seção é o método que esta casa já
usou no SHA-256, no HMAC, no PBKDF2 e no analisador de JSON: **ler a norma,
entender, reescrever aqui, e provar contra vetor.** Nada abaixo é «façamos como o
InnoDB». É o desenho do PhxSql, nos nossos termos, com cada divergência **nomeada
e justificada por uma restrição nossa**.

Chamo-o de **Sombra**: uma cadeia de versões velhas pendurada no `rowid`, do mais
novo para o mais velho, que **nenhum caminho de escrita paga quando não há leitor
com visão aberta**.

### 7.1 A premissa que nos separa dos dois oráculos, e ela é nossa desde a 0.19

O InnoDB guarda no `ReadView` um limite inferior, um superior **e a lista
ordenada dos ativos** (`m_ids`, com busca binária por linha lida). O
PostgreSQL(R) guarda, no `SnapshotData` do `snapmgr.c`, exatamente o mesmo trio:
`xmin`, `xmax` e o vetor `xip` dos que estão em curso — e o
`HeapTupleSatisfiesMVCC` do `heapam_visibility.c` chama `XidInMVCCSnapshot` para
consultá-lo.

**Os dois precisam da lista pelo mesmo motivo: nos dois motores existe, dentro da
tabela, linha escrita por transação que ainda não cometeu.** A lista é o que
permite ao leitor pular essas linhas.

**Aqui isso não acontece.** A transação do PhxSql **empilha em RAM e só aplica no
`COMMIT`** (`docs/FORMATO.md` §17; medido por soquete na `PENDENCIAS.md`, pedido
157), e o `COMMIT` aplica **sob a trava global**, que serializa.

> **Divergência 1, e é a que barateia todo o resto:** *toda linha que está no
> `.reg` já foi cometida.* Logo o leitor nunca precisa perguntar «esta linha é de
> alguém que não cometeu?». Precisa só de **um número**: «o que foi cometido até
> a marca *M*». **A lista de ativos, a busca binária por linha e os dois limites
> somem** — e somem por uma propriedade que já temos, não por um atalho.

O preço de manter a marca: um `u64` incrementado sob a trava que já é tomada. O
`lock` sem disputa custa **13,2 ns** (`DESEMPENHO.md` §14) e o incremento é uma
soma. *Custo de projeto: nenhum.*

### 7.2 Onde cabe o nosso equivalente do `DB_ROLL_PTR` — e cabe nos 3 bytes

O InnoDB gasta **13 bytes por linha** (`DATA_TRX_ID_LEN = 6` +
`DATA_ROLL_PTR_LEN = 7`, com `static_assert` no fonte). Nós temos **3 bytes
livres** — `flags` no byte 1 e `res` nos bytes 2..4 (§1.2, medido no `reg.rs`).

**Divergência 2 — não precisamos do `DB_TRX_ID` na linha.** Ele existe no InnoDB
para decidir a visibilidade **da própria linha**, e pela §7.1 essa pergunta não
existe aqui: a linha do `.reg` é sempre a corrente cometida. A marca que decide
visibilidade mora **no registro de sombra**, e não no slot.

**Divergência 3 — o ponteiro é ÍNDICE, não endereço.** O `trx0undo.ic` já ensina
que 7 bytes bastam porque o ponteiro é **estruturado** (bit de inserção,
segmento, página, deslocamento) e não um deslocamento cru. Nós vamos um passo
além, e podemos porque a nossa sombra é **por tabela** e o leitor já tem o
`Table` aberto na mão:

> **O ponteiro de sombra é um índice de 24 bits num diretório por tabela.** Zero
> significa «esta linha não tem sombra». Sobram **16.777.215** sombras vivas por
> tabela — muito acima de qualquer conjunto de trabalho de visão aberta.
>
> **Onde o InnoDB gasta 13 bytes, o PhxSql gasta 3 — e são exatamente os 3 que
> temos.** *O `.reg` v6 pode não ser necessário, e essa é a conclusão que mais
> muda o roteiro.*

**E a armadilha desses 3 bytes, medida e dita:** o CRC do slot cobre
`slot[SLOT_CAB..]` — **só o payload**, não o cabeçalho (`reg.rs` l. 153:
`crc32(&slot[SLOT_CAB..]) == Campos(slot).u32(4)`). Isso é bom, porque escrever o
ponteiro **não invalida o CRC**; e é ruim, porque o ponteiro fica **sem proteção
de integridade**. Um bit trocado ali apontaria para a sombra de outra linha.

**A defesa, e ela é o padrão desta casa:** o registro de sombra **carrega o
próprio `rowid`**, como já fazem o registro do `.trash` (offset 12) e o evento do
`.log` («o slot que esta operação vai escrever»). Sombra cujo `rowid` não bate com
o de quem a pediu é **recusada**, não usada. *Verificação no destino custa 8 bytes
por sombra e dispensa mudar o CRC.*

### 7.3 O registro de sombra: delta, e o externo só quando o externo mudou

**Divergência 4 — o formato do registro é delta, e não a foto do `.trash`.** O
`trx0rec.cc` grava, na alteração, `n_updated` e depois **só as colunas mexidas**;
e na inserção, **só a chave**. Adotamos a forma, com nomes nossos:

```text
sombra: [rowid u64][marca u64][anterior u24][n u16]
        por coluna mexida: [coluna u16][tam u32][valor velho]
        por coluna EXTERNA mexida: [coluna u16][tam u32][conteúdo]
```

**Divergência 5 — o externo vai por CONTEÚDO, e não por prefixo mais referência.**
O InnoDB guarda prefixo + os 20 bytes de referência
(`BTR_EXTERN_FIELD_REF_SIZE`) e **conta com o bloco velho continuar vivo até a
purga**. **Nós não podemos:** o `.bin`/`.memo` **reaproveita bloco liberado**, e a
nossa própria especificação diz o estrago —
*«a foto voltaria sendo a foto de outra linha»* (`FORMATO.md`, «Por que não é um
`.reg` paralelo»).

Então copiamos conteúdo, **como o `.trash` já faz** — mas só da coluna externa que
**mudou**. Alteração que não toca `Bin`/`Memo` não copia byte nenhum de externo,
que é o caso comum e é o que o `.trash` hoje não sabe fazer.

*(A alternativa — fixar o bloco no `.bin`/`.memo` por contagem de referência — é a
Pergunta 3b da §8. Ela é mais barata em bytes e mais cara em formato, e a decisão
é do DBA.)*

### 7.4 O `.ndx`: adiamento em vez de marca — a divergência de que mais me orgulho

O InnoDB **marca** a entrada velha do índice secundário como apagada e a purga
depois. Nós **não podemos marcar**: a folha do `.ndx` é `chave completa + rowid`,
`ck_len = key_len + 8`, e um bit de marca muda a largura da entrada — segundo
formato a versionar. **É exatamente onde o zheap parou** (§6.2).

**Divergência 6 — usamos uma propriedade que o nosso `.ndx` já tem e o do InnoDB
não precisa ter.** A `docs/FORMATO.md` §2 diz, sobre a remoção:

> «Remover tira a entrada da folha **sem rebalancear** a árvore. A busca continua
> correta (folhas vazias apenas não produzem resultado).»

Remoção que não rebalanceia é remoção **adiável**. Então:

> **Enquanto houver visão aberta sobre a tabela, a alteração de coluna indexada
> ACRESCENTA a entrada nova e NÃO REMOVE a velha.** A remoção fica pendurada na
> sombra e acontece na purga.
>
> O índice passa a **superinformar** — devolve rowids cuja chave corrente é outra
> —, e quem filtra é a verificação que o leitor já vai fazer de qualquer jeito:
> descer a sombra até a marca dele e conferir o valor da coluna.
>
> **É o *delete-marking* do InnoDB, obtido sem um bit no índice.** A marca vira
> adiamento, e o adiamento é de graça porque a nossa remoção não rebalanceia.

**E o que isso NÃO resolve, dito antes que alguém tromb com ele:** num índice
**ÚNICO**, a entrada velha ainda presente faz a inserção da nova bater na
unicidade. Para índice único há três saídas, e nenhuma é grátis: (a) recusar
alteração de coluna de índice único com visão aberta — o estilo desta casa, que
recusa cedo e por escrito; (b) conferir a unicidade contra a **versão corrente**
em vez de contra a presença da entrada, o que custa uma leitura do `.reg` por
inserção; (c) o bit de marca, e aí é formato do `.ndx`. **Isto é pergunta para o
DBA, e está na §8.**

### 7.5 O que a Sombra custa quando ninguém a usa: **zero, e o portão vem antes**

Pétrea desta casa, paga pelo Profiler: *instrumentação desligada tem de custar
zero, e o portão que decide isso vem **antes** do trabalho.*

Na Sombra o portão é um `bool` por tabela — «há visão aberta?» — lido **antes** de
montar registro nenhum. Sem visão aberta:

* a escrita **não** monta sombra, **não** aloca e **não** compara nada;
* a leitura **não** consulta o diretório;
* o slot continua com os 3 bytes em zero, que é o que toda tabela já gravada tem.

*O caminho quente de hoje não muda de forma nenhuma.* E isso não é promessa: é
mensurável, e a §7.7 diz como.

### 7.6 A purga, e por que ela NÃO é a SP000014

Versão velha morre quando a visão aberta mais antiga passa da marca dela — a
mesma regra do `trx0purge.cc`, que clona a visão mais antiga (`clone_oldest_view`)
para decidir o que ainda não pode sair.

**Divergência 7 — quando o teto estoura, quem é recusado é o LEITOR.** O
PostgreSQL(R) não tem teto e paga em *bloat*; o Oracle(R) e o SQL Server(R) têm
teto (e o Oracle(R) devolve `ORA-01555`). Nós teremos teto, e a recusa vai para o
leitor **por um motivo medido**: recusar o escritor deixaria um leitor longo
parar toda a gravação, e a §10.5 já mostra o que a gravação faz sob disputa —
**0,51× com dois clientes**. *Não se acrescenta comboio a um caminho que já
regride.*

O nome do erro entra pela fábrica de idiomas, como manda a pétrea:
`erro.versao_recolhida`.

**E a distinção que precisa estar escrita antes de alguém tropeçar nela:** purgar
sombra **não** é a SP000014. A SP000014 foi recusada porque **o `.reg` não
reaproveita slot**; a Sombra vive em outro lugar, e reciclar espaço lá não toca a
ordem de digitação de nada. *O `.reg` continua ganhando um slot por linha nova e
nenhum por versão* — que é a condição inteira sob a qual a §4.1 do
`CONCORRENCIA.md` autoriza o MVCC.

### 7.7 Como se PROVA — e a prova é nos dois sentidos

O equivalente ao vetor do FIPS aqui é a bancada. Cada item traz **o que faz
passar** e **o que tem de fazer FALHAR com o defeito reposto**, porque teste que
passa por engano é pior que teste que falta.

| o que se prova | passa quando | **falha com o defeito reposto quando** |
|---|---|---|
| **leitura repetível** | leitor abre visão e lê `V`; escritor comete `V'`; o leitor relê e ainda vê `V`; fecha, reabre e vê `V'` | desligada a consulta à sombra, a segunda leitura devolve `V'` — **o teste tem de ficar vermelho** |
| **custo zero desligado** | `varrer` e `inserir` sem visão aberta, antes e depois, dentro da tolerância de controle do `quieta.Vigia` (15%) | movido o portão para **depois** da montagem do registro, a curva de `inserir` cai — é a repetição controlada do defeito do Profiler |
| **a recusa é do LEITOR** | teto minúsculo, leitor longo aberto, escritor grava além do teto: o **leitor** recebe `erro.versao_recolhida` e o **escritor** mantém a vazão de 1 cliente | trocado o alvo da recusa, o escritor bloqueia e a vazão cai — o teste tem de ver a queda |
| **a replicação não sente** | bancada de replicação (modo A) com a Sombra ligada: os rowids do réplica batem com os do source | forçada a alocação de um slot por versão, `aplicar_evento` **para** — e é ele que dá o veredito |
| **o índice adiado** | com visão aberta, alterar coluna indexada: a visão acha a linha pela chave **velha** e não a acha pela **nova**; depois da purga, o inverso | removida a entrada velha na hora, a visão **perde** a linha — o teste tem de perdê-la |
| **a sombra não vira foto de outra linha** | sombra com `rowid` divergente do pedido é **recusada** | zerada a conferência do `rowid`, um ponteiro corrompido devolve a linha errada — e o teste tem de devolvê-la |

**E os dois números que MATAM o desenho, medidos antes de escrever código:**

1. **O custo de gravar a sombra.** Medido pelo `excluir`, que já escreve versão
   velha em arquivo separado (§9). Se custar como uma gravação durável
   (~1.400 µs contra 137 µs), a premissa «zero `fsync`» está errada e a Sombra em
   RAM não é o desenho — é a §8, opção B.
2. **O tamanho da sombra em memória.** Rodar o cenário do próprio oráculo do
   roteiro — a *history list* que foi de **7 a 207** com um leitor aberto — e
   medir os bytes por versão numa linha larga. Se 200 versões estouram um teto
   razoável, a opção A morre medida, e isso é resultado tão válido quanto o ganho.

### 7.8 O que a Sombra toma do Cassandra(R), e onde ela DIVERGE dele

O dono mandou tratar o Cassandra(R) como base permanente e buscar **inspiração,
não cópia**. Aqui está exatamente o que se aproveita e onde a nossa lógica é
outra — que é o que separa inspiração de cópia.

**Divergência 8 — resolvemos na leitura, como eles, mas por MARCA e não por
RELÓGIO.** O `Cells.resolveRegular` compara **carimbo de hora** e devolve o maior.
Isso paga o preço do relógio: duas máquinas com relógios diferentes elegem
vencedores diferentes, e por isso o Cassandra(R) precisa de carimbo do cliente e
de resolução determinística até no empate.

A Sombra compara uma **marca monotônica gerada sob a trava que já existe** — sem
relógio, sem carimbo do cliente, sem empate possível, porque o incremento é
serializado pela mesma trava global que serializa tudo (e custa **13,2 ns**). *A
propriedade que eles compram com um protocolo, nós já temos de graça pelo defeito
que estamos tentando remover.* É a segunda vez neste documento em que uma
limitação nossa vira simplificação: a primeira foi a §7.1.

**Divergência 9 — nada desaparece.** No `resolveRegular` o valor perdedor
**some sem aviso**, e o `CASSANDRA.md` §7.4 já recusou isso com o nome certo:
*«trocar uma recusa por uma perda silenciosa»*. Na Sombra o «perdedor» é a versão
**mais velha**, que é exatamente a que o leitor antigo **quer** ver — ninguém
perde nada, porque não há disputa de autoria: há duas épocas da mesma linha.

**Divergência 10 — a fusão é sobre DUAS fontes, não sobre N.** Eles fundem
memtable mais um número indeterminado de SSTables, e é por isso que a leitura
deles fica mais cara quanto mais compactação faltar. Nós temos **o slot do `.reg`
(a corrente) e a cadeia de sombra (as anteriores)** — e a cadeia é vazia no caso
comum, o que faz o custo cair a **um teste de ponteiro nulo** (§7.5). *A forma é
deles; o número de fontes é nosso, e é o que impede a leitura de degradar.*

**E o que se adota inteiro, sem divergir:** a ideia de que **o caminho de leitura
é o lugar certo para decidir**, e não o de escrita. É o mesmo motivo por que o
`.ndx` pode superinformar e ser filtrado depois (§7.4): quem decide é quem lê.

### 7.9 O que a Sombra NÃO entrega, e está dito antes de alguém prometer

* **Não é `SERIALIZABLE`.** Entrega *snapshot isolation*, que admite **write
  skew** (§5.7). O `SERIALIZABLE` é outro item, com gestor de travas próprio.
* **Não separa escritor de escritor.** A regressão de **0,51×** (§1.3) continua
  inteira: a trava global é tomada antes de qualquer noção de versão existir.
* **Não separa leitor de leitor.** Isso é a SP000011, e é o documento irmão.
* **Não substitui a decisão de formato — adia-a com motivo.** Se a medição de
  7.7(2) matar a opção em RAM, o `.reg` v6 volta, e a pétrea «mudança de formato
  entra cedo» manda decidir o significado dos 3 bytes **agora**, mesmo sem
  implementar: quem grava zero hoje continua compatível amanhã.

---

## 8. O que fica para o C (DBA) decidir — perguntas fechadas, com preço

Nenhuma é recomendação. São escolhas cujo preço eu consegui apurar.

### 8.0 — RESPONDIDAS pelo dono em 04/09/2026

As sete foram postas uma a uma. **Quatro caíram sozinhas** com a resposta da
primeira, porque eram todas «se for disco».

| # | pergunta | decisão |
|---|---|---|
| **1** | RAM ou disco? | **A — em RAM, e o teto NÃO é número novo: é o `transacao_prazo_min` que já existe.** Ver 8.0.1 abaixo. **A decisão de formato da SP000016 desaparece: nenhum `.reg` v6, nenhuma migração, nenhum arquivo novo** |
| 2 | se disco, no slot ou fora? | **caiu** — não há disco |
| 3 | linha inteira ou delta? | **caiu** |
| 3b | o `.bin`/`.memo` fixa bloco? | **caiu** |
| **4** | o `.ndx` ganha marca? | **NÃO. Recusar MVCC em alteração de coluna indexada** — cedo e por escrito, como o `ao_excluir`. É a única das três que preserva o zero-formato da resposta 1; a marca reintroduziria um segundo formato a versionar |
| 5 | a cópia leva a `.undo`? | **caiu** — não há `.undo` |
| **6** | *snapshot* ou `SERIALIZABLE`? | ***Snapshot isolation*, e o documento DIZ isso**, com o exemplo do *write skew* onde o cliente o leia. `SERIALIZABLE` (SSI) é outro item, e não se promete pelo nome da sprint |
| **7** | a ordem SP11×SP16 muda? | **A bateria longa do `gravar` roda antes de a SP000011 começar — e SÓ ela espera.** A SP000016 começa já: depois das respostas 1, 4 e 6 ela virou RAM + recusa, com teto herdado e zero formato, e não toca na trava. As duas **desacoplaram** |

**O que a Sombra virou depois destas respostas:** cadeia de versões **em RAM**,
sem `fsync`, **zero mudança de formato em disco** — nem no `.reg`, nem no
`.ndx`, nem extensão nova —, com duas recusas escritas (leitor longo cuja versão
foi recolhida; alteração de coluna indexada sob transação) e a limitação de
*snapshot isolation* documentada.

### 8.0.1 — A condição que o dono trouxe, e ela remove a objeção à opção A

O dono apontou que *«o timeout tem um parâmetro no `config.json`, e o tempo
máximo obedece essa config»*. **Medido, e o parâmetro é outro — melhor:**

| campo | o que limita | padrão | imposto? |
|---|---|---|---|
| `timeout_s` | a **espera por um pedido** numa conexão ociosa (`set_read_timeout` no soquete) | 30 s | sim, mas **não** limita transação que continua conversando |
| **`transacao_prazo_min`** | **a transação INTEIRA** | **5 min** | **sim** — `transacao.rs:620` filtra por `expira_ms`, e `bancada/transacoes/provar.py` exercita com prazo de 1 min |

O comentário do próprio campo já dizia o raciocínio, escrito antes desta
pesquisa existir: *«uma transação segura tabelas contra a escrita de todo
mundo, e ninguém digita por dez minutos com uma transação aberta. Zero não
desliga: cairia no padrão, porque transação sem prazo nenhum é exatamente a que
trava a tabela para sempre.»*

**O que isso muda:** a cadeia de versões da Sombra **não pode crescer sem fim**,
porque a transação que a segura morre em 5 minutos por um prazo que já existe e
já é imposto. A recusa estilo `ORA-01555` deixa de ser a primeira defesa e vira
a **segunda rede** — só dispara se a memória estourar *antes* do prazo.

É o mesmo desenho de duas redes do `carga_prazo_min`, e está escrito lá: *«a
primeira é a queda da conexão, que desfaz na hora; esta pega o soquete pendurado
vivo com o cliente morto do outro lado.»*

**E o teto do prazo não sobe.** O campo é curto de propósito, e trocar risco de
RAM por risco de tabela travada é uma troca que precisa de número, não de
palpite.

**O número da §9 continua valendo, e mudou de papel:** ele não decide mais entre
A e B — decide se a recusa do leitor longo é confortável ou apertada. Medir
continua barato e continua valendo.

---

### Pergunta 1 — A versão velha mora em RAM ou em disco?

Ela só existe por causa da §5.4: **a undo daqui não desfaz nada**, logo não precisa
sobreviver a uma queda. O SQL Server(R) faz assim há duas décadas (`tempdb`).

| opção | custa | compra | perde |
|---|---|---|---|
| **A — em RAM**, mapa `rowid -> versões`, no molde da `Sobreposicao` que já entrega *read-your-own-writes* | **zero de formato.** Nenhum `.reg` v6, nenhum arquivo novo, nenhuma migração, nenhum `fsync` | leitura repetível para leitor vivo — a **única** coisa que falta (`CONCORRENCIA.md` §4.3: o RYOW já está feito) | teto de memória. `memoria_max_mb` nasce **0** = sem teto; a *history list* do oráculo foi de **7 a 207** com um leitor aberto. Leitor longo estoura, e a saída é **recusá-lo** |
| **B — em disco**, `.reg` v6 + `.undo` | **10 bytes** por slot, migração slot a slot de toda tabela existente, **11ª extensão** em duas listas (§5.8), e o risco do ponteiro `Bin`/`Memo` (§5.3) | leitor longo sem teto de memória | o `fsync`, **se** for durável — e §6.8 diz que não precisa |
| **C — híbrido**, RAM com transbordo | o pior dos dois de projeto | o melhor dos dois de operação | só se escolhe **depois** de A ou B existir |

**A pergunta fechada:** *o PhxSql aceita recusar um leitor longo com «versão velha
já foi recolhida» — como o Oracle(R) faz com o `ORA-01555: snapshot too old`
(<https://docs.oracle.com/en/error-help/db/ora-01555/>) — em troca de MVCC sem
mudança de formato nenhuma?*

**Se sim, a opção A, e a decisão de formato da SP000016 desaparece.** Se não, a
opção B — e aí a pétrea «mudança de formato entra cedo» manda decidir **agora**.

> **A favor de A, e é um argumento de simetria:** o Oracle(R) e o SQL Server(R)
> **limitam** o crescimento das versões; o PostgreSQL(R) não tem como, e é dele a
> fama de *bloat* (EnterpriseDB, §2.2). Se vamos ter teto de qualquer jeito, tê-lo
> em RAM custa menos que tê-lo em disco.

### Pergunta 2 — Se for disco: os bytes de versão entram no slot ou fora dele?

| opção | preço |
|---|---|
| **no slot** (`.reg` v6, `slot_size` +8 ou +16, como da v4 para a v5) | migração de toda tabela existente, reescrita slot a slot na mesma ordem. **Mecanismo provado.** Ordem de digitação intacta |
| **fora do slot**, mapa `rowid -> ponteiro` no `.undo` | zero de mudança no `.reg`. Paga uma busca a mais por linha lida sob transação, e leitura do `.undo` no arranque para reconstruir o mapa |
| **nos 3 bytes livres**, como índice de 16 bits para um mapa em RAM | zero de formato **e** zero de arquivo — mas é a opção A com atalho, e só vale se a resposta da pergunta 1 for A |

### Pergunta 3 — A undo guarda a linha inteira ou só o delta?

O `.trash` guarda inteira; o InnoDB guarda **só as colunas mexidas**, e isso está
lido no fonte (§2.2) — inclusive o caso mais barato de todos, o undo de
**inserção**, que grava **só a chave**.

| opção | preço |
|---|---|
| **linha inteira** (formato do `.trash`) | reaproveita formato que já existe e já resolveu o `Bin`/`Memo`. **Custa** copiar `Memo` de megabytes numa alteração que mexeu num inteiro |
| **delta** `(coluna, valor velho)`, com o undo de inserção guardando só a chave | registro pequeno; alteração que não toca externo não copia externo. **Custa** formato novo, e reconstruir a versão velha passa a ser *aplicar deltas sobre a corrente* — mais código e mais lugares de errar |

### Pergunta 3b — O `.bin`/`.memo` aprende a NÃO liberar bloco fixado?

Esta pergunta **só existe depois de ler o fonte do InnoDB** (§2.3), e ela é de
formato tanto quanto as outras. Hoje o `.bin`/`.memo` **reaproveita bloco
liberado** — a pétrea «não reaproveita» vale para o `.reg`, não para os externos.

| opção | preço |
|---|---|
| **fixar o bloco** (contagem de referência ou marca) enquanto houver undo apontando | a undo fica pequena, como a do InnoDB. **Custa** um terceiro item de formato e um caminho de liberação novo — e liberar errado é a «foto de outra linha» que o `.trash` existe para evitar |
| **copiar o conteúdo** para dentro da undo | zero de formato novo nos externos. **Custa** o tamanho, sempre |
| **recusar MVCC quando a alteração toca coluna externa** | zero de tudo. **Custa** uma operação legítima recusada |

### Pergunta 4 — O `.ndx` ganha marca de apagado?

Sem ela não há MVCC correto quando a alteração toca coluna indexada (§5.6). **É
onde o zheap parou.**

| opção | preço |
|---|---|
| **marca de apagado no `.ndx`** | **segundo** formato a versionar; toda leitura que topa numa marca volta ao `.reg` |
| **recusar MVCC em alteração de coluna indexada** | o motor recusa operação legítima — mas recusa **cedo e por escrito**, que é o padrão desta casa em `ao_excluir` |
| **reindexar sob transação** | mata o ganho: a transação longa vira a operação mais cara do motor |

### Pergunta 5 — Uma cópia de tabela leva a `.undo` da origem?

`EXTENSOES` e `EXTENSOES_TODAS` são listas diferentes **de propósito** («a lixeira
da origem não é da cópia»). A `.undo` também não deveria ser — mas isso precisa
estar **escrito**, porque a lista já foi esquecida **duas vezes** (§5.8).

### Pergunta 6 — O que a SP000016 promete: *snapshot isolation* ou `SERIALIZABLE`?

São coisas diferentes, e a diferença é o *write skew* (§5.7).

| opção | preço |
|---|---|
| **snapshot isolation, e o documento diz isso** | é o que a undo entrega. Custa **escrever a limitação** onde o cliente a leia, com o exemplo do `write skew` |
| **`SERIALIZABLE` de verdade** | SSI: gestor de travas novo, teto de memória, e transações que **abortam** — o cliente tem de saber repetir. É outro item, e o paper do PostgreSQL(R) mostra que é grande |

### Pergunta 7 — A ordem SP000011 × SP000016 muda de novo com o número de 04/09?

O roteiro inverteu a ordem em 02/09 com o argumento *«o gap é leitor-com-leitor, e
o MVCC não conserta esse par»*. Em 04/09 apareceu o número do escritor: **0,51×
com 2 clientes**.

**Isso não devolve a ordem antiga** — MVCC também não conserta
escritor-com-escritor. Mas invalida a **premissa** da frase que justificou a
inversão, e premissa inválida não sustenta ordem, nem quando a ordem continua
certa por outro motivo. *A pergunta fechada:* **a bateria longa do `gravar` roda
antes de a SP000011 começar, ou a inversão fica de pé com a justificativa
remendada?**

---

## 9. O número que falta, e é barato

Um só, e ele decide a pergunta 1.

**Medir quanto a trava fica presa num `excluir`.** É a única operação que **já**
escreve uma versão velha em arquivo separado e **já** sincroniza esse arquivo
antes de liberar o slot do `.reg` (`FORMATO.md` §5, «A ordem é o recurso»). Ele é
o protótipo pronto do custo de gravar uma undo durável — **e ninguém mediu.**

O instrumento existe e não pede código novo no motor: o
`bancada/concorrencia/quanto-a-trava-fica-presa.py` já lê `trava_ms` da telemetria
em torno de um laço de `inserir` e de um de `varrer`. Falta um terceiro laço, de
`excluir`, na mesma bateria.

* **~137 µs** (como o `inserir` em `por_lote`) → undo durável é barata, opção B
  viável.
* **~1.400 µs ou mais** → a §6.8 está confirmada com número próprio, e a opção A
  (undo em RAM) deixa de ser alternativa e passa a ser a resposta.

*Medir a premissa do item vem antes de implementar o item — inclusive quando o
item é nosso.*

**Não medido, e nomeado:** a bateria longa do `gravar` (§1.3); a contagem de
chamadas por operação numa carga real, que decide se os 23× valem para a carga e
não só para o medidor (§4).

---

## 10. Fontes, licença de cada uma, e o que a rede não deu

**A anotação de licença é informação para o dono, não veto.** Nenhuma técnica saiu
deste documento por causa dela. **Nenhuma linha de código de nenhum motor foi
copiada para cá**; tudo é descrição a partir de documentação, wiki e paper.

| fonte | licença | o que se usou |
|---|---|---|
| MySQL(R) Reference Manual + **o fonte do InnoDB** (ver a tabela abaixo) | documentação do produto; o código é **GPLv2** — **lido para entender, nada copiado** | os 6+7 bytes, insert × update undo, o *delete-marking* no índice, a undo que não se replica, o `ReadView` (descrito com minhas palavras) |
| MariaDB(R) | **GPLv2** | não foi consultada por redundância, e **não** por licença: o InnoDB do `mysql-server` é o mesmo motor e foi lido no fonte |
| PostgreSQL(R) — `storage-page-layout`, `transaction-iso`, `mvcc-intro`, `buffer/README` | **licença PostgreSQL**, permissiva | os 23 bytes do `HeapTupleHeaderData`, o `t_ctid`, o exemplo de *write skew*, a trava de conteúdo do buffer |
| PostgreSQL(R) Wiki — `Zheap`, `Heap_HOT_Selective_Index_Updates` | wiki do projeto | vantagens e pendentes do zheap **pelos próprios autores**; as duas condições do HOT |
| SQLite — `faq.html`, `begin_concurrent.md` | **domínio público** | a granularidade de arquivo inteiro; o conflito falso por página e a receita de chave aleatória |
| Bolt (`boltdb/bolt`) | **MIT** | escritor único e os *Caveats* do COW — **do README** |
| LMDB | **OpenLDAP Public License** | escritor único e a árvore de páginas livres, por descrição de terceiros |
| RocksDB — wiki `Snapshot` | **Apache-2.0 / GPLv2 (duplo)** | *sequence numbers* e recolhimento por *compaction* |
| Firebird — paper de MVCC do projeto | **IDPL / documentação** | *back versions* como delta, dentro do banco |
| Oracle(R) — `ORA-01555` | documentação de erro, proprietária | **só a existência e o texto do erro**, como precedente de «recusar o leitor» |
| Microsoft(R) — `tempdb-database` | documentação, proprietária | o *version store* num banco recriado a cada arranque |
| Wu, Arulraj, Lin, Xian, Pavlo — PVLDB 10(7) 2017 | paper acadêmico | a taxonomia (append-only / time-travel / delta), O2N × N2O, ponteiro físico × lógico |
| CMU 15-721, notas `03-mvcc1` | material de curso | a mesma taxonomia, com os motores nomeados |
| **`docs/CASSANDRA.md` desta casa** (fonte da 5.0.10, commit `7b5ab44`) | nosso | as duas recusas já registradas (§7.3 e §7.4) e o commit log que não replica (§7.6) — **citado, não refeito** |
| Apache Cassandra(R) 5.0, fonte | **Apache 2.0** — compatível com o nosso | a resolução por carimbo no caminho de leitura, e o `fsync` periódico |
| Berenson, Bernstein, Gray, Melton, O'Neil — SIGMOD 1995 | paper acadêmico | a definição de *snapshot isolation* e o **A5B — Write Skew** |
| Ports, Grittner — PVLDB 5(12) 2012 | paper acadêmico | SSI, *dangerous structures*, e o que custou implementá-la |
| Haas (`rhaas.blogspot.com`), EnterpriseDB, CYBERTEC, pgPedia | artigos de terceiros | comparações e **estado de projeto**, citados como tal e nunca como especificação |

### O que foi lido NO FONTE, com o código HTTP e o tamanho

Tudo por `GET` em `raw.githubusercontent.com`. **Nada disto foi copiado para o
repositório** — o que entrou no documento é descrição minha, em português, do que
o código faz.

| arquivo | HTTP | bytes | o que se tirou dele |
|---|---:|---:|---|
| `mysql-server/storage/innobase/include/data0type.h` | 200 | 23.854 | `DATA_ROW_ID_LEN=6`, `DATA_TRX_ID_LEN=6`, `DATA_ROLL_PTR_LEN=7`, com `static_assert` nos dois últimos |
| `mysql-server/storage/innobase/trx/trx0rec.cc` | 200 | 91.033 | o undo de alteração grava **só as colunas mexidas**; o de inserção grava **só a chave**; e o prefixo + referência de 20 bytes para coluna externa |
| `mysql-server/storage/innobase/include/trx0undo.ic` | 200 | 9.983 | o ponteiro de undo de 7 bytes é **endereço estruturado** (bit de inserção, id de segmento, página, deslocamento) |
| `mysql-server/storage/innobase/include/read0types.h` | 200 | 8.291 | o `ReadView` e os três degraus de `changes_visible` |
| `mysql-server/storage/innobase/trx/trx0undo.cc` | 200 | 74.283 | os segmentos de undo |
| `mysql-server/storage/innobase/trx/trx0purge.cc` | 200 | 69.780 | a purga limitada pela visão aberta mais antiga (`clone_oldest_view`); a *history list* |
| `mysql-server/storage/innobase/include/btr0types.h` | 200 | — | `BTR_EXTERN_FIELD_REF_SIZE`, «a reference to data stored on a different page» |
| `postgres/src/include/access/htup_details.h` | 200 | 30.982 | o `t_ctid` que passa a apontar para a versão substituta |
| `postgres/src/backend/access/heap/pruneheap.c` | 200 | 90.468 | `LP_REDIRECT` / `LP_DEAD` / **`LP_UNUSED`**, e o `mark_unused_now` |
| `postgres/src/backend/access/heap/vacuumlazy.c` | 200 | 133.258 | `RecordPageWithFreeSpace`, `FreeSpaceMapVacuumRange`, `lazy_truncate_heap` |
| `cassandra/cassandra-5.0/.../db/rows/Cells.java` | 200 | 12.313 | `resolveRegular`: o carimbo maior ganha, o menor some |
| `cassandra/cassandra-5.0/.../commitlog/PeriodicCommitLogService.java` | 200 | 1.876 | `maybeWaitForSync` só espera quando a sincronização já atrasou |
| `sqlite/src/wal.c` | 200 | 183.813 | o `WAL_WRITE_LOCK` tomado em modo exclusivo |
| `sqlite/src/pager.c` | 200 | 305.013 | a trava EXCLUSIVE do arquivo para escrever |

**Licença do que foi lido, para o dono decidir e não para vetar leitura:**
`mysql-server` é **GPLv2** (o cabeçalho de cada arquivo do InnoDB diz isso);
`postgres` é **licença PostgreSQL**, permissiva; `sqlite` é **domínio público**;
`cassandra` é **Apache 2.0**, que é a mais limpa de todas para nós porque é
**compatível com o nosso `MIT OR Apache-2.0`** — ali não há armadilha jurídica
nenhuma, e a regra «inspiração, não cópia» vale por motivo puramente técnico.
O `Cargo.toml` deste projeto declara `license = "MIT OR Apache-2.0"`, e é essa
linha que **colar** GPLv2 tornaria falsa. **Ler não a torna falsa, e por isso
nenhuma técnica saiu deste documento por causa de licença.**

### O que a rede NÃO deu, com o comando e o erro

Só duas coisas, e nenhuma delas é motor:

* **`https://www.vldb.org/pvldb/vol10/p781-Wu.pdf`** — desce (HTTP 200, 1,2 MB),
  mas **este contêiner não tem `pdftotext` nem `poppler-utils`**
  (`/bin/bash: line 1: pdftotext: command not found`; e o leitor de PDF respondeu
  `pdftoppm is not installed`), e o `WebFetch` recusou transcrever verbatim de
  *streams* comprimidos. A taxonomia entrou **parafraseada**, conferida contra as
  notas do CMU 15-721. *Limitação medida, com o comando e a mensagem.*
* **`http://www.lmdb.tech/doc/`** — **HTTP 503 Service Unavailable**. O LMDB
  entrou por fontes de terceiros e pelo README do Bolt, que é reimplementação
  declarada dele. **Nenhuma citação de LMDB aqui é da documentação oficial**, e é
  por isso que ela está dita assim.

**E uma correção ao registro desta frente:** o briefing original desta pesquisa
tratava a fronteira de licença como se ela impedisse **ler** InnoDB e MariaDB(R).
Não impede, e a distinção certa é a que o orquestrador escreveu depois: **a
técnica não é o código.** As fontes desceram todas, com HTTP 200, por `GET` em
`raw.githubusercontent.com`. *Se algum 403 apareceu antes, era artefato de `HEAD`
contra o proxy, e não bloqueio* — **limitação suposta não vale; limitação medida,
sim.**

### Números desta casa, e de onde saíram

| número | de onde | refeito nesta rodada? |
|---|---|---|
| **3 bytes livres na v5, 11 na v4** | varredura de `crates/phxsql-store/src/reg.rs` atrás de quem escreve e lê cada faixa do `SLOT_CAB` | **sim** — e contradiz o «24 de 24» do briefing e da §4.2 do `CONCORRENCIA.md` |
| 10 extensões por tabela, em duas listas | `EXTENSOES` e `EXTENSOES_TODAS`, `catalogo.rs` l. 352 e 377 | sim, lido |
| entrada de folha do `.ndx` = `key_len + 8` | `docs/FORMATO.md` §2 | não, citado |
| 3.122–3.187 / 121–137 / 1.267–1.371 µs | `docs/CONCORRENCIA.md` §7.1 | não — conferido na fonte |
| 1,99× / 1,51–1,59× / 13,2 ns / 83,5% no `.ndx` | `docs/DESEMPENHO.md` §14 e `--example onde-doi` | não — conferido na fonte |
| `gravar` **0,51×** com 2 clientes | `docs/CONCORRENCIA.md` §10.5 (04/09) | não — e é o número que o briefing desta frente não tinha |
| a transação não grava antes do `COMMIT` | `docs/FORMATO.md` §17 e `docs/PENDENCIAS.md` pedido 157 | não, citado |
