# Pesquisa do papel J — rodada de 24/09/2026 (pedidos 533, 542 e 365)

Papel J, 24/09/2026. Só leitura no repositório (HEAD `7f96430`, extraído por `git archive` para o
rascunho). Provas e fontes baixadas em `scratchpad/j-rodada/` (apêndice). Motores medidos nesta
máquina: **PostgreSQL 16.13**, **MySQL 8.0.46**, **MariaDB 10.11.14**, **SQLite 3.45.1**.

| pedido | decisão | número que decide | sobe ao dono? |
|---|---|---|---|
| **533** | **(a)**: `fdatasync` da subida do byte 52, uma vez por tabela por janela, antes da primeira escrita. **(b) morre** | emulação «nenhuma página do `.ndx` chegou»: **18/18 calados**, sem página nova no disco para (b) conferir; catraca `alcancam-fsync-2` **23 → 23** e `TETO_FSYNC_POR_FECHO_V2` **8 → 8** com a subida sincronizada; custo **+1 `fdatasync` por tabela por janela** (8 → 9 por janela, +12,5%), **~102 µs** aqui, sem vencedor no ciclo do servidor | **não** |
| **542** | diretórios **0700**, todos os arquivos do banco **0600**, cópia de backup e restauração **0700/0600**, independente do `umask`. Base existente: alerta, sem recusar | «outros não leem»: **3 de 3 maduros, medido** (aceite automático). Grupo sem acesso: **7 × 3** | **não** |
| **365** | **a decisão fica** (arquivo do Profiler: todo literal vira `?`; texto que não se analisa vira o tamanho). **O motivo muda** | registro de estatística normaliza (3/3). O **registro por evento**, que é o equivalente do Profiler, guarda o texto **cru** nos 3 (medido). Quem decide é a pétrea, não a convergência | choque do **motivo 1** (pétrea), **já resolvido pela pétrea**: aparece na mesa como registro, sem pedir decisão nova |

**Papel que não cumpriu:** o *fan-out* para `pesquisa-motor` e `pesquisa-bancada` não aconteceu,
porque esta sessão não tinha ferramenta para abrir subagente. Fiz os dois domínios sozinho.
**Premissa velha no pedido:** «83,5% da inserção está no `.ndx`» é número de antes do cache.
O próprio `docs/DESEMPENHO.md` (l. 29–37, remedido em 08/09/2026) dá **34,6%** para o `.ndx` e
**60,5%** para `.reg` + `.log`, sobre 7,4 µs por linha. O `CLAUDE.md` ainda cita os 83,5% (tarefa
do papel H).

---

## 1. Pedido 533 — a subida do byte 52 sem `fsync`

### 1.1 Hipóteses, escritas antes de medir

- **H1a:** os maduros garantem que a marca (ou o registro) fica durável **antes** das páginas por
  meio de um `fsync`. Então a direção (a) é o comportamento deles.
- **H1b:** uma geração/LSN por página, conferida na leitura, detecta o furo sem `fsync`. É a
  direção (b), «como o LSN de página do PostgreSQL e do InnoDB».
- **H1c:** o modelo do `pg_control`. A marca sobe durável **uma vez por vida do processo** e só
  desce na parada limpa.
- **H1d:** o cabeçalho limpo do `.ndx` guarda o tamanho do `.reg`, e a abertura compara os dois.
  É uma conferência barata, sem `fsync`.
- **H1e (premissa do pedido):** o `fsync` da subida cria seção nova sob `travar_dados`, e por isso
  a `alcancam-fsync-2` o proíbe.

### 1.2 Matriz motor × comportamento

| motor (peso) | como a informação «incompleto» chega ao disco antes das páginas | fonte | como |
|---|---|---|---|
| **PostgreSQL (4)** | **Regra WAL:** antes de gravar a página suja, `XLogFlush(LSN da página)`. O LSN da página serve para dizer **até onde sincronizar o log**, não para detectar nada na leitura | `src/backend/storage/buffer/bufmgr.c:3477-3495` (REL_16) | lido |
| | A marca de «não fechou limpo» é o `pg_control`. Passa a `DB_IN_PRODUCTION` **uma vez**, no arranque, e é gravada **com `pg_fsync`**. Só volta a `DB_SHUTDOWNED` no checkpoint da parada. Estado diferente de `DB_SHUTDOWNED` põe o banco em recuperação | `xlog.c:5782`, `:4171-4173`; `common/controldata_utils.c:242-246`; `xlog.c:6847-6848`; `xlogrecovery.c:915` | lido |
| | **Estrutura sem log** (tabela *unlogged*): não é ordenada, é **zerada depois de qualquer queda**, e quem decide é o estado durável acima. O comentário do fonte diz que a regra WAL não vale para ela «which will be lost after a crash anyway» | `xlog.c:5362-5368` (`ResetUnloggedRelations`); `bufmgr.c:3481-3494` | lido |
| | **Índice sem log** (hash, até a 9.6): «might need to be rebuilt with REINDEX after a database crash if there were unwritten changes … use is presently discouraged». Nenhuma marca detectava o caso | docs 9.6, `indexes-types.html` | lido |
| **MariaDB (3)** | InnoDB: `if (lsn > log_sys.get_flushed_lsn()) log_write_up_to(lsn, true)` antes do I/O da página | `storage/innobase/buf/buf0flu.cc:853-856`, `:755-760` (10.11) | lido |
| | Aria/MyISAM: a marca `open_count`/`STATE_CHANGED` vai por `my_pwrite` **sem `fsync`**. O próprio comentário diz que as tabelas transacionais se consertam pelo log e que o contador só aponta as «duvidosas» | `storage/maria/ma_locking.c:429-489` (comentário `:447-456`) | lido |
| **MySQL (2)** | InnoDB: «Force the log to the disk before writing the modified block», `log_write_up_to(newest_lsn)` | `storage/innobase/buf/buf0flu.cc:1199-1211` (8.0) | lido |
| | MyISAM: `_mi_mark_file_changed` = `mysql_file_pwrite` de 3 bytes **sem `fsync`**, igual ao nosso byte 52 de hoje. O preço está documentado: queda de energia corrompe, e «Queries don't find rows in the table or return incomplete results» | `storage/myisam/mi_locking.cc:474-494`; `refman/8.0/en/corrupted-myisam-tables.html` | lido |
| **SQLite (1)** | O journal é sincronizado **antes** de qualquer página ir ao arquivo do banco, tanto no despejo quanto no commit | `src/pager.c:4225-4262` (`syncJournal`), `:4631-4643`, `:6584` (3.45.1) | lido |

**Soma.** «Registro ou marca durável antes da página, por `fsync`» soma PG 4 + MariaDB 3 (InnoDB) +
MySQL 2 (InnoDB) + SQLite 1 = **10**, contra **0**. Os três maduros convergem, e o comportamento
entra por **aceite automático**.

O contraexemplo não diverge da regra: ele é o preço dela. A marca sem `fsync` do MyISAM e do Aria
não transacional é **exatamente** o nosso byte 52 de hoje, e é o motor que os próprios donos
documentam como corrompível numa queda de energia, com o sintoma que o C4 mediu: a busca não acha
a linha.

**E a premissa de (b) cai no fonte.** No PostgreSQL e no InnoDB o LSN de página não substitui o
`fsync`. Ele **manda fazer** o `fsync` do log até ele, antes de a página sair. Nenhum dos quatro
usa uma geração conferida na leitura como mecanismo de correção.

### 1.3 O que medi

**Emulação C4′ — a queda mais provável.** Nenhuma página do `.ndx` chegou, e o `.reg` chegou
(`prova533`, sobre o HEAD). As duas situações são plausíveis sem `fsync`: o núcleo devolve cada
inode ao disco por conta própria, e o `.reg` sujou antes do `.ndx`.

| cenário | `.reg` | `precisa_reconstruir` | filhas do cliente 2 | `verificar` | `excluir` do cliente 2 |
|---|---|---|---|---|---|
| `nada` 30.000/5.000, 20.000/2.000, 40.000/8.000, 2.000/300 | cresce | falso | **0** | acusa («N chaves para M registros») | **Ok** — **12/12, calado** |
| `move` 30.000/5.000 e 2.000/300: filhas passam do cliente 1 ao 2 **no mesmo slot** | **mesmo tamanho** | falso | **0** | **ok** | **Ok** — **6/6, calado** |

- **H1b morre (18/18).** No `nada` e no `move`, o `.ndx` no disco é, byte a byte, o do último
  fecho limpo. Não existe página «mais nova que o cabeçalho» para uma geração conferir. As formas
  de árvore que o C4 do DBA viu **ruidosas** (20.000/2.000 e 40.000/8.000) saem **caladas** aqui.
- **H1d morre (6/6).** O `atualizar` regrava o slot no lugar (`reg.rs:2157-2179`), e o tamanho do
  `.reg` não muda. Até o `verificar` completo passa.
- **H1e morre, medida.** Com o `fdatasync` posto em `levantar_marca` numa cópia do fonte, o
  `mapa-da-trava.py --catraca` mede **`alcancam-fsync-2` 23 → 23**. O medidor enxerga o caminho
  novo em 4 seções (`op_inserir`, `op_excluir`, `op_reindexar`, `semear_mensagens`), e as quatro
  **já estavam** nas 23. O exemplo `fsync-por-fecho` mede **8 → 8**, porque conta só o fecho.
  Consequência para a G: **nenhuma catraca de hoje enxerga o `fsync` da subida**.
- **H1a, custo contra o nosso caminho:**

| medida | antes | com (a) | fonte |
|---|---|---|---|
| `fsync`/`fdatasync` em 2.000 pedidos + 1 lote de 2.000 (10 janelas) | 60 | 60 + **13** `fdatasync` (média de 83 µs) | `strace -c` sobre o `custo-do-byte-52` |
| escrita de 4 KiB + `fdatasync`, 500×3, neste disco (ext4/virtio) | — | mediana **101,5–102,0 µs**, p90 128–140 µs, máx. até 4 ms | laço Python no rascunho |
| ciclo do servidor, µs por pedido (abrir+inserir+Drop), 3 rodadas intercaladas × 5 corridas, carga 6–8 | 113,1 / 112,9 / 103,3 | 113,2 / 97,9 / 129,2 | `custo-do-byte-52` do HEAD × cópia com (a) |

  No ciclo do servidor as faixas se cruzam nas 3 rodadas: **não há vencedor**, e é isso que se
  afirma. A conta que não depende do disco é outra: o fecho já paga **8** `fsync` por tabela por
  janela (`TETO_FSYNC_POR_FECHO_V2`), e (a) acrescenta **1**, ou seja, **+12,5%** da conta de
  durabilidade que já existe.

  Em disco com FLUSH de verdade o número por pedido é **raciocinado, não medido**: com `fsync` de
  1 ms seriam ~5 µs por pedido (1 ms dividido pela janela de 200); com 10 ms, ~50 µs. Quem decide
  é o mesmo `custo-do-byte-52` rodado num SSD ou HDD sem cache de hospedeiro.
- **H1c: recusada, raciocinada.** O `phxsqld` só para por sinal (`FORMATO.md`, «não tem outro
  jeito de parar»). Então **todo** reinício reconstruiria toda tabela escrita desde o arranque, a
  **1,65–1,88 s por milhão de linhas** com 2 índices (C3 do DBA). Contra isso, (a) custa ~0,1 ms
  por tabela por janela.

### 1.4 Decisão

**(a).** O `fdatasync` entra em `NdxFile::levantar_marca`, logo depois de `gravar_cabecalho`,
**só na passagem de 0 para 1** (uma vez por tabela por janela). Esse é o lugar único das duas
portas que sobem a marca, e o `.fts` passa por ele também.

Três regras de execução:

- O `fdatasync` passa pelo motor `sincronia`, não pelo `File` cru. Assim o gancho
  `falha_de_teste::Onde::Fsync` e a trava do 509 («`fsync` recusado não desce mais») valem para
  ele também.
- Basta `fdatasync`: a página 0 já existe e o tamanho não muda.
- Se ele falhar, a marca em RAM volta a 0 e a escrita recusa **antes** de tocar o `.reg`.

**Onde divergimos da origem, e qual restrição causou a divergência.** Os maduros ordenam por
**log** (WAL/journal) e recuperam **refazendo**. Nós ordenamos **um bit por tabela por janela** e
recuperamos **reconstruindo**. A restrição que causa isso: o `.ndx` é derivado de um `.reg` que é a
fonte da verdade e só cresce pela ordem de digitação. Então não é preciso refazer nada; basta não
confiar. O equivalente direto é o caminho **sem log** do PostgreSQL (*unlogged*/hash): reconstruir
depois da queda, decidido por um estado que **ele** grava com `fsync`.

**Pétreas.**

- **Formato: não muda.** O byte 52 continua querendo dizer a mesma coisa, e não há migração. Muda
  o texto da regra 1 do `FORMATO.md`: «vai a 1 no arquivo, sem `fsync`» passa a «vai a 1 **no
  disco**, antes da primeira escrita».
- **Zero dependências** e **ordem de digitação** não são tocadas.
- **Catraca:** as duas seguram, medido. A regra «catraca nunca sobe» é decisão do dono, e por isso
  eu não a trataria como regra só de QA. Aqui ela não é acionada.

**Nada sobe ao dono.**

**O que (a) NÃO fecha:** o C5 (o 0 fica durável antes do `fsync` do `.reg`) continua sendo o ⏸
próprio dele.

### 1.5 Para os outros papéis

- **F:** prova nos dois sentidos pelo gancho `Onde::Fsync`. Com o `fsync` da subida recusado, o
  `inserir` erra e o `.reg` não cresce. Com o defeito reposto (sem o `fsync`), o gancho não
  dispara e o `.reg` cresce, então o teste reprova. Contra o SO: o cenário 522 do
  `bancada/catastrofes/prova.sh`, ou `dm-log-writes`. Os modos `nada` e `move` entram no
  `examples/disco-que-recusa.rs`.
- **G:** catraca nova para o `fsync` da subida (1 por tabela por janela), nascendo no número medido
  do dia. As duas de hoje são cegas a ele, medido.
- **⏸ histerese** (não baixar o 0 de tabela quente, para subir menos): só se o `custo-do-byte-52`
  num disco com FLUSH real mostrar que (a) pesa. Sem esse número, é palpite.

---

## 2. Pedido 542 — permissão dos arquivos de dado

### 2.1 Hipóteses, escritas antes

- **H2a:** os três maduros convergem em «outros não leem» para o diretório, os arquivos e a cópia
  física.
- **H2b:** divergem no **grupo**, e a régua decide.
- **H2c (a derrubar):** eles só herdam o `umask` do processo, como o SQLite.

### 2.2 Matriz

Medido com `umask 022` no shell que chamou (o padrão de instalação).

| motor (peso) | diretório | arquivos de dado | cópia física | recusa arrancar se estiver largo? | fonte | como |
|---|---|---|---|---|---|---|
| **PostgreSQL (4)** | **700** | **600** (298/298 em `base/1`; `pg_control` 600) | `pg_basebackup`: **700/600**, copia o modo da origem | **sim**: com 0755, «FATAL: data directory … has invalid permissions … should be u=rwx (0700) or u=rwx,g=rx (0750)» | `common/file_perm.h:24-41`, `file_perm.c:18-48`, `miscinit.c:384-398`, `pg_basebackup.c:2714-2721`, `streamutil.c` (`RetrieveDataDirCreatePerm`) | **medido** |
| | grupo sob pedido: `initdb --allow-group-access` dá 750/640 | | | | | medido |
| **MariaDB (3)** | **700** | **660** (198), mas o diretório 700 impede o grupo de chegar a eles; `mysql_upgrade_info` 644; logs 660 | `mariabackup`: **700/640** | não | `mysys/my_init.c:152-162` (0660/0700), `sql/mysqld.cc:4054` (`umask(~my_umask & 0666)`) | **medido** |
| **MySQL (2)** | **750** | **640** (157); chaves 600; certificados públicos 644; logs 640 | `CLONE LOCAL`: **750/640**. O `mysqldump` por redirecionamento sai **644** (`umask` de quem chama) | não | `mysys/my_init.cc:144-152` (0640/0750; `UMASK`/`UMASK_DIR` com `\|0600`/`\|0700`), `sql/mysqld.cc:7581` | **medido** |
| **SQLite (1)** | — | **644** (`umask`); `-wal`, `-shm` e `-journal` **herdam** o modo do banco (600 → 600) | `.backup` e `VACUUM INTO` nascem **644**, mesmo com a origem em 600 | não | `src/os_unix.c:166-167`, `:6150-6163` | **medido** |

**H2c morre.** Os três maduros impõem o modo: o PostgreSQL pela `pg_mode_mask`, e o MySQL e a
MariaDB reescrevendo o `umask` do processo. Só o SQLite herda o `umask`.

**Somas.**

- **Outros leem?** Não nos três maduros: **convergência, aceite automático**.
- **O grupo alcança o dado por padrão?** O acesso efetivo é o que o diretório **e** o arquivo
  deixam passar. Não: PG 4 + MariaDB 3 (o diretório 700 barra) = **7**. Sim: MySQL 2 + SQLite 1 =
  **3**. Só o dono.
- **A cópia física abre mais que a origem para outros?** Não nos três maduros: **convergência**. O
  `.backup` do SQLite abre, mas pesa 1 e perde para a convergência.
- **Recusar arrancar com permissão larga?** PG 4 recusa; MySQL 2 + MariaDB 3 + SQLite 1 = **6**
  não recusam. Não recusar, e a nossa lei «guarda nova entra pedida» empurra para o mesmo lado.

### 2.3 Decisão

| o quê | modo |
|---|---|
| diretório do banco e subdiretórios | **0700** |
| `.reg`, `.ndx`, `.memo`, `.bin`, `.trash`, `.reason`, `.log`, `.lgpd`, `.fts`, e **todo** arquivo que nasce no diretório de dados (`.psch`, `.tx`, `.pag`…) | **0600** |
| cópia de backup: diretório e arquivos | **0700/0600**, pelo mesmo motor, qualquer que seja o `umask` (hoje o `File::create` do `backup.rs`, perto de :70, sai 644) |
| restauração: palco e destino | **0700/0600**, pelo mesmo motor |
| `perfil.txt` e os logs do servidor (liga com o 365) | **0600**. O arquivo de texto do `pg_stat_statements` sai 600, medido |
| base que já existe em 0644/0755 | **não recusa e não aperta calado**: alerta com o comando. Apertar sozinho tiraria o acesso de quem hoje lê por grupo, e isso é guarda imposta |
| grupo sob pedido (como o `--allow-group-access` do PG) | ⏸, até alguém pedir |

**Um motor só.** Todas as criações passam por uma função, no molde do `create_new` +
`mode(0o600)` que o `config.rs` já tem, e não por um `set_permissions` depois de cada
`File::create`. É a lei «função e comando vêm do mesmo motor».

**Nada sobe ao dono.**

**Hipótese que morreu:** H2c.

---

## 3. Pedido 365 — o Profiler e os literais do `{"op":"sql"}`

### 3.1 Hipóteses, escritas antes

- **H3a (a sua):** os três normalizam os literais no registro de **estatística**
  (`pg_stat_statements` e o digest do `performance_schema`), e isso sustenta a normalização no
  Profiler.
- **H3b:** o registro que corresponde ao Profiler é o **registro por evento** (log geral, histórico
  de comandos), e nele os três guardam o texto **cru**.
- **H3c:** mesmo o registro de estatística guarda texto cru em algum caso.

### 3.2 Matriz

Mesmos comandos nos três. Um `INSERT` com CPF literal, um `SELECT` com literal, `CREATE` e `ALTER`
de usuário com senha, um `SET` com literal, e um `INSERT` com erro de sintaxe.

| motor (peso) | registro de estatística | registro por evento | texto que não se analisa | fonte | como |
|---|---|---|---|---|---|
| **PostgreSQL (4)** | `pg_stat_statements`: o DML sai normalizado (`VALUES ($1, $2)`, `cpf = $1`). **Os comandos de utilidade saem CRUS:** `CREATE ROLE r1 PASSWORD 'segredo-criar-123'` e `ALTER ROLE … 'segredo-alterar-456'` aparecem na visão **e** no `pgss_query_texts.stat` (2 ocorrências). O `SET` também sai com o literal | `log_statement`: **tudo cru**, senhas inclusive | fica fora do pgss; o log grava o texto cru, `STATEMENT:` (`log_min_error_statement`, padrão `error`) | doc `pgstatstatements.html` (16/17/atual: «When a constant's value has been ignored … replaced by $1»); `pg_stat_statements.c:1289-1297` (só normaliza com `jstate`), `:1188` (utilidade sem `jstate`) | **medido** (16.13) |
| **MariaDB (3)** | `DIGEST_TEXT` normalizado, `CREATE USER` inclusive. **Não há** coluna `QUERY_SAMPLE_TEXT` | `events_statements_history_long.SQL_TEXT`: **cru**. Log geral e log lento: **crus, com as senhas em claro** | **gravado cru no log geral** | doc «Performance Schema Digests»; doc de cifra em repouso: «not encrypted by the MariaDB server: … General Query Log, and Slow Query Log» | **medido** (10.11.14) |
| **MySQL (2)** | `DIGEST_TEXT` normalizado, `CREATE USER` inclusive. **Mas a mesma linha guarda `QUERY_SAMPLE_TEXT` CRU**, com o CPF `'999.888.777-66'`; só a senha vira `<secret>` | `events_statements_history.SQL_TEXT`: **cru**. Log geral e log lento: **crus** (CPF), com a senha trocada por `<secret>` (`log_raw=OFF`) | **não vai ao log geral**: «cannot be known to be password free» | `refman/8.0 performance-schema-statement-digests.html`; `password-logging.html`; `sql/sql_rewrite.cc` | **medido** (8.0.46) |
| **SQLite (1)** | não tem | não tem (só `sqlite3_trace` da aplicação) | — | — | lido |

**Somas.**

- **O registro de estatística normaliza o texto-chave?** Sim nos três (4 + 3 + 2 = 9):
  convergência. Com duas ressalvas medidas: o PG deixa o comando de utilidade cru, senha
  inclusive, e o MySQL guarda uma amostra crua na mesma linha. **H3c é verdadeira.**
- **O registro por evento guarda o texto cru?** Sim nos três (9 × 0): **convergência pelo texto
  cru.** Só o MySQL troca a senha e descarta o texto que não consegue analisar.

**Onde está o Profiler.** O `profiler.rs` se declara «o equivalente do Profiler do SQL Server»:
cada pedido **quando chega**, num anel e num arquivo. Isso é o **registro por evento**, e não o de
estatística. **H3a erra de registro:** a convergência citada existe, mas não alcança o Profiler.
**H3b se sustenta.**

### 3.3 Decisão: a sua decisão fica, com outro motivo

Seguida ao pé da letra, a convergência mandaria o arquivo guardar os literais. Isso **bate na
pétrea** «senha nunca em texto puro» e na decisão do 356 (o arquivo viaja com o disco e com o
backup; texto em claro ao lado do `.reg` cifrado anula o cofre). A pétrea ganha, e o choque fica
**registrado** aqui (motivo 1). Ele não pede decisão nova ao dono, porque a pétrea já decide na
direção que você escolheu.

**O que muda por eu estar corrigindo o motivo:**

1. O pedido, o commit e o documento não podem dizer «convergência do pgss/digest» como razão. A
   razão é a pétrea + o 356 + o precedente do MySQL (o que não se prova livre de segredo não vai
   ao log), estendido de «senha» para «dado cifrado».
2. **Normalizar todo tipo de frase, e não só DML.** O furo medido do PG (senha crua no pgss,
   porque utilidade não se normaliza) é o erro a não copiar. A normalização se faz no **léxico**:
   todo símbolo literal vira `?`, qualquer que seja o comando. É analisar, não recortar.
3. **Texto que o léxico recusa vira o tamanho em bytes**, como a pétrea já manda e como o MySQL faz
   (não grava). Medido: o `INSERT` com aspa aberta dá `Err`.
4. **O anel continua vendo cru** (simetria com o 356: quem vê o anel é administrador e tem o
   `config.json`). Só o arquivo cega.

**Recusada, com motivo: cegar só quando a frase nomeia tabela cifrada.** O `op_sql` aceita `CALL`
no nível de cima (teste `CALL somar(100)`, `servidor.rs:36751`). Um
`CALL grava('999.888.777-66')` não nomeia tabela nenhuma, e o literal acaba numa tabela cifrada
pela rotina ou pelo gatilho. É a família dos «7 de 116 que escondem a tabela». O vazamento foi
**lido no código, não medido**.

**Preço medido** (`lexico::analisar`, release, 5 corridas):

| texto | custo | peso |
|---|---|---|
| `INSERT` de 59 B | **0,626 µs** [0,624–0,630] | ~0,6% de um pedido de ~100 µs |
| `INSERT` de 5.000 linhas (129 KB) | **0,96 ms** [0,95–1,57] | ~7,4 ns/B |

Esse custo só se paga **com o Profiler ligado e gravando arquivo**, depois do interruptor.
Desligado, continua custando zero.

**Hipótese que morreu:** H3a, como **razão** (o comportamento que ela sustentava continua de pé).

---

## 4. Lacunas (o que esta pesquisa não alcançou)

| lacuna | o que decidiria |
|---|---|
| Queda de energia **real** para o 533. C4′ e o `move` são emulação no arquivo, e este disco virtual tem cache do hospedeiro | `dm-log-writes`/`dm-flakey` ou o cenário 522 do `bancada/catastrofes/prova.sh`, com (a) |
| Custo de (a) em disco com FLUSH real (aqui ~102 µs) | `custo-do-byte-52` e `pg_test_fsync` num SSD/HDD sem cache |
| Windows: os modos POSIX não se aplicam (ACL herdada do diretório) | pesquisa própria; compilação cruzada com `#[cfg(unix)]` |
| Modos da restauração do `mariabackup --copy-back` e do `pg_dump -f` | uma corrida de cada |
| Fan-out por domínio | não houve ferramenta de subagente nesta sessão |

## 5. Efeitos colaterais, desfeitos e registrados

- **Nenhum `apt install`.** A MariaDB veio por `apt-get download` + `dpkg -x` para o rascunho,
  **sem mudar o estado do dpkg**. Conferido depois: `mysql-server 8.0.46` continua `ii`, e o
  `mariadb-server` segue `rc` como já estava.
- O `mariabackup` rodou dentro de `unshare -m`, com um *bind mount* **privado** do `charsets` da
  MariaDB. Conferido: nenhuma montagem sobrou em `/usr/share/mysql/charsets`.
- O cluster PG de prova ficou em `/tmp/j-rodada-pg`, porque o usuário `postgres` não atravessa
  o diretório da sessão (0700). Foi parado e apagado.
- As instâncias MySQL/MariaDB do rascunho foram paradas pelo próprio soquete. Os diretórios de
  dados foram apagados **depois de provar**, por `cwd`, descritores e mapas, que nenhum processo
  os usava. Os logs ficaram em `evidencia/`.
- **Nenhum processo alheio foi tocado:** o `mysqld` do sistema (pid 2046), o `mariadbd` de outra
  frente (pid 12137) e o `postgres 16/main`.

## Apêndice — para refazer

Tudo em `scratchpad/j-rodada/`:

- `head/`: `git archive 7f96430`.
- `sub/`: o mesmo, com o `fdatasync` em `levantar_marca`.
- `prova533/`: os modos `nada`, `move` e `le`. A emulação é uma linha:
  `std::fs::write(ndx, ndx0)` (o `.ndx` volta inteiro ao último fecho; o `.reg` fica).
- `lex365/`: o custo do léxico.
- `catraca-{head,sub}/`: o `mapa-da-trava.py` sobre cada árvore.
- `ab-byte52.txt`, `emulacao-533b.txt`, `fontes/` (os arquivos citados, baixados das tags
  REL_16_STABLE, 8.0, 10.11 e version-3.45.1), `evidencia/` (logs dos três motores).

Comandos:

- `CARGO_TARGET_DIR=… cargo build --offline --release --example custo-do-byte-52 -p phxsql-store`
  (e `fsync-por-fecho`), em `head/phxsql` e `sub/phxsql`.
- `prova533 nada DIR 30000 5000 && prova533 le DIR`.
- `strace -f -c -e trace=fsync,fdatasync …/custo-do-byte-52 1 2000`.

Os modos `nada` e `move` pertencem ao `examples/disco-que-recusa.rs`: roteiro que resolveu algo não
morre com a sessão.
