# As cinco decisoes de formato passadas pela regua dos motores maduros

**Papel J (pesquisador), 17/09/2026.** So leitura. Fonte primaria em cada
afirmacao: manual oficial da versao corrente ou o **fonte** dos motores, com URL
ou arquivo:linha. Onde nao mediu, diz «nao medi».

**Por que este documento existe.** Ordem do dono, 17/09/2026: *«Suas
recomendacoes devem levar em conta o que o Mariadb e o PostgreSQL faz.»* O
integrador recomendou as cinco decisoes contra o nosso fonte e contra as nossas
petreas, e **nao passou nenhuma pela regua** que esta casa tem para isto: tres
motores maduros convergindo e aceite automatico; onde nao convergem, decide a
media ponderada (PostgreSQL 4, MariaDB 3, MySQL 2, SQLite 1); e convergencia nao
revoga petrea, mas **o choque aparece**.

O alcance que o integrador errou, e que e o aprendizado desta rodada: a regua
era lida como peneira para **receita que vem de fora**, e vale tambem para
**decisao nossa sobre o que o banco faz**. Semantica de carimbo de tempo, faixa
de sequencia, conflito de unicidade, cifra em repouso e criterio de eleicao sao
todos «o que o banco faz».

**Nota de metodo:** nao houve fan-out; o papel J rodou as tres frentes sozinho.
O SQLite **sai da conta** em 290, 292, 293 e 294 por nao ter replicacao nem
cluster — dito explicitamente em vez de contar peso 1 para uma opiniao que ele
nao tem.

---

## 289 — carimbo de data/hora por linha, com avanco forcado

| motor | carimbo de relogio por linha | onde mora a ORDEM de fato |
|---|---|---|
| PostgreSQL (4) | `CURRENT_TIMESTAMP`/`now()` = inicio da **transacao**, constante nela; nao ha coluna de sistema com hora | **`xmin`**, contador de 32 bits |
| MariaDB (3) | `row_start` `TIMESTAMP(6)`, do **inicio da instrucao** | alternativa na propria coluna: `BIGINT UNSIGNED` = id de transacao + `mysql.transaction_registry` |
| MySQL (2) | `NOW()` constante por **instrucao**; sem coluna de sistema | **`DB_TRX_ID`**, 6 bytes |
| SQLite (1) | nao medi | nao tem versionamento de sistema — sai da conta |

### (a) O empate do relogio e de proposito

PostgreSQL, Date/Time Functions, verbatim:

> «Since these functions return the start time of the current transaction, their
> values do not change during the transaction. **This is considered a feature**:
> the intent is to allow a single transaction to have a consistent notion of the
> "current" time, so that **multiple modifications within the same transaction
> bear the same time stamp**.»

MySQL: «NOW() returns a constant time that indicates the time at which the
statement began to execute»; `SYSDATE()` (relogio real) e a excecao.

### (b) O desempate dos maduros e um contador, nao uma resolucao mais fina

MariaDB, System-Versioned Tables, verbatim:

> «To use transaction-precise history, InnoDB needs to remember **not timestamps,
> but transaction identifier per row**. This is done by creating generated
> columns as `BIGINT UNSIGNED`, **not `TIMESTAMP(6)`**.»

E o motivo que eles dao **nao e granularidade**: e que hora de escrita e hora de
visibilidade sao grandezas diferentes.

> «A point in time when a row was inserted or deleted does not necessarily mean
> that a change became visible at the same moment. With transactional tables, a
> row might have been inserted in a long transaction, and became visible hours
> after it was inserted.»

**Medido no fonte do MariaDB 11.4, nao na prosa:** `sql/table.cc:9730`
(`TABLE::vers_update_fields()`) chama `vers_start_field()->set_time()`; e
`sql/field.cc:5751-5759`:

```c
int Field_timestamp_with_dec::set_time()
{
  THD *thd= get_thd();
  set_notnull();
  ulong msec= decimals() ? thd->query_start_sec_part() : 0;
  store_TIMESTAMP(Timestamp(thd->query_start(), msec).trunc(decimals()));
```

`thd->query_start()` e o **inicio da instrucao**: duas linhas do mesmo `INSERT`
saem com `row_start` identico, por construcao.

### (c) Terceira convergencia, de brinde: o destino honra o que veio

MySQL, Replication and System Functions: «For `NOW()`, the binary log includes
the timestamp. This means that the value **as returned by the call to this
function on the source** is replicated to the replica.» E o contraexemplo com
nome: «the `SYSDATE()` function is **not replication-safe**». E a doenca do
`rownum` (pedido 291) nomeada pelo manual deles.

### Veredito: **MUDA em parte**

- **Confirma-se**: existir a coluna de sistema por linha; viajar na imagem e a
  replica honrar o que veio (aceite automatico, com contraexemplo nomeado).
- **O que nao fecha e a conclusao**: a decisao reconhecia que os maduros ordenam
  por contador e mesmo assim mandava **o relogio fazer o papel do contador**, por
  avanco forcado. **Nenhum dos tres forca relogio a avancar.**
- **Choque com petrea? Nao — ao contrario, a regua REFORCA a ordem do dono de
  11/09.** «Impossivel o filho ter a mesma data do pai» nao tem precedente nos
  tres: eles garantem o **oposto** dentro da unidade de trabalho. A petrea ganha
  da convergencia, como manda a lei. O que a regua entrega nao e a meta, e o
  **meio**: para cumprir a ordem do dono sem brigar com o relogio, o campo que
  ordena tem de ser um contador.

**Alerta que a fonte deu de graca:** se o contador for atribuido no `inserir` e o
`COMMIT` vier depois, duas linhas podem ficar carimbadas A<B e visiveis B<A — e
literalmente o paragrafo do MariaDB citado acima. Nos tres isso nao acontece
porque o contador e **de transacao** e a visibilidade sai do estado dela.

**DECISAO DO DONO, 17/09/2026: duas colunas, 16 bytes.** Um contador puro por no,
que cumpre a ordem e nao depende de relogio nenhum, e uma coluna de data e hora
comum, que pode empatar sem mentir.

---

## 290 — faixa da `Sequence` por no

- **MySQL (2):** `auto_increment_increment` e `auto_increment_offset` sao
  **variaveis de sistema do servidor/sessao**, escopo Global+Session, dinamicas,
  padrao 1, faixa 1..65535. Verbatim: «`auto_increment_increment` and
  `auto_increment_offset` are **intended for use with circular (source-to-source)
  replication**»; a serie e `offset + N x increment`. **Nada disso mora no esquema
  da tabela.**
- **MariaDB (3):** as mesmas variaveis; em Galera,
  `wsrep_auto_increment_control` (padrao ON) «automatically adjusts» as duas
  «according to the size of the cluster». **Medido no fonte**,
  `sql/wsrep_server_service.cc:231-234`:

```c
if (wsrep_auto_increment_control && view.own_index() >= 0)
{
  global_system_variables.auto_increment_offset= view.own_index() + 1;
  global_system_variables.auto_increment_increment= view.members().size();
```

  **Deslocamento = indice do no na visao + 1. Passo = tamanho do cluster.**
- **PostgreSQL (4):** `INCREMENT BY` e `START` moram no objeto sequencia, que e
  catalogo. E a replicacao logica **nao replica sequencia**: «Sequence data is not
  replicated… **the sequence itself would still show the start value on the
  subscriber**.» PG nao tem multi-master no nucleo, entao **nao vota** na pergunta
  de onde vem o deslocamento em multi-master.
- **SQLite (1):** sai da conta.

### A conta

**O passo mora no esquema?** PG sim (catalogo), MySQL/MariaDB nao (variavel de
servidor). Divergem: 4 x 5. **Mas a margem e enganosa e J nao a usa**: o objeto
sequencia do PG **nao e replicado**, entao la o «esquema» ja e local de qualquer
jeito. **Em nenhum dos quatro o passo replicado vem no esquema replicado.**

**De onde vem o deslocamento de cada no em multi-master?** So MySQL e Galera tem
o recurso, e os dois convergem: **da identidade/configuracao do no, nunca do
esquema da tabela.** Nao e trio pela letra da lei (e 5 de 5 **entre quem tem o
recurso**), e a diferenca fica registrada em vez de virar aceite automatico.

### Veredito: **CONFIRMA**, com precedente no fonte

«Inicio na identidade do no» e **literalmente** o que o Galera faz. E a correcao
que o dono fez ao parecer — tirar o inicio do `PSCH` porque a tabela nascida por
replicacao herda o bloco byte a byte — **e o mesmo raciocinio que levou o Galera
a derivar o offset da visao do cluster em vez de grava-lo**: o que e igual nos
dois nos nao pode ser o que os distingue.

**Onde divergimos, e a restricao nossa:** o Galera deriva o **passo** do tamanho
do cluster **em tempo de execucao**, reajustando a cada mudanca de visao. Nos
gravamos o passo no `PSCH`. A restricao e a **ordem de digitacao sagrada** mais o
`.reg` nao reaproveitar slot: numero de `Sequence` ja gravado e imutavel aqui,
entao um passo que mudasse quando um no entra reabriria faixas ja usadas. O
Galera pode reajustar porque o auto_increment do InnoDB nao carrega promessa de
ordem; o nosso carrega. **Passo no esquema e mais rigido que o precedente, de
proposito.**

**Sem precedente, e J diz:** «o inicio usado fica gravado na primeira escrita e
trocar recebe recusa» nao tem equivalente nos quatro. E invencao nossa, coerente
com a ordem de digitacao; entra como decisao, nao como aceite.

**Nao medi:** como pglogical/BDR aloca faixa por no (galloc sequences), nem o
`CREATE SEQUENCE` do MariaDB sob Galera.

---

## 292 — unicidade em indice secundario trava o par bidirecional

### MariaDB/Galera (3): certifica SIM pelas chaves unicas secundarias

Medido no fonte. `storage/innobase/handler/ha_innodb.cc`,
`ha_innobase::wsrep_append_keys()` (a partir de 10134) percorre **todas** as
chaves e anexa chave de certificacao por indice unico:

```c
for (i=0; i<table->s->keys; ++i) {
    KEY* key_info = table->key_info + i;
    if (!hasPK || key_info->flags & HA_NOSAME || ...) {
        /* This key has chaged. If it is unique, this is an exclusive
           operation -> upgrade key type */
        if (key_info->flags & HA_NOSAME) {
            key_type = WSREP_SERVICE_KEY_EXCLUSIVE;
        }
```

`HA_NOSAME` e o sinalizador de indice unico (`include/my_base.h:282`). O conflito
vira **falha de certificacao**, contada em `wsrep_local_cert_failures` e
devolvida ao cliente como **`ER_LOCK_DEADLOCK`** (`sql/wsrep_mysqld.cc:2963`). E a
lista oficial de limitacoes do Galera **nao recusa tabela por indice unico
secundario**: a unica exigencia de chave e «All tables should have a primary key».

### PostgreSQL (4): nao certifica nada — aplica e quebra, visivelmente

Logical Replication Conflicts: «If incoming data violates any constraints the
replication will stop.» `insert_exists` e um dos sete tipos nomeados, e a
mensagem traz relacao, indice, **chave, linha local e linha remota**:

```
ERROR:  conflict detected on relation "public.test": conflict=insert_exists
DETAIL:  Key already exists in unique index "t_pkey", ...
Key (c)=(1); existing local row (1, 'local'); remote row (1, 'remote').
```

Saidas: `disable_on_error`, `ALTER SUBSCRIPTION … SKIP (lsn)`,
`pg_replication_origin_advance()`. O PG 18 acrescentou registro e contadores em
`pg_stat_subscription_stats`, e **nenhuma resolucao automatica**.

**E sim, o PostgreSQL fica em laco — por decisao escrita no fonte.**
`src/backend/replication/logical/worker.c:4537-4559`:

```c
PG_CATCH();
{
    /* Reset the origin state to prevent the advancement of origin
     * progress if we fail to apply. Otherwise, this will result in
     * transaction loss as that transaction won't be sent again by the server. */
    replorigin_reset(0, (Datum) 0);
    if (MySubscription->disableonerr) DisableSubscriptionAndExit();
    else { ... PG_RE_THROW(); }
```

E `launcher.c:1191-1197`, verbatim: «Each subscription's apply worker can only be
restarted **once per `wal_retrieve_retry_interval`, so that errors do not cause us
to repeatedly restart the worker as fast as possible**.»

**Isto e, palavra por palavra, o nosso defeito do 292.** A diferenca e que o
PostgreSQL **escolheu** isso — e o comentario diz por que: nao avancar a origem e
o que impede perder a transacao — e pagou tres coisas em cima: **estrangulou** o
laco (5 s por padrao), **gritou** (log com a chave e as duas linhas, contador) e
**deu a saida** (`disable_on_error`/`SKIP`).

### MySQL Group Replication (2)

Certificacao pela primaria ou equivalente. **Nao medi** se anexa chaves de indices
unicos secundarios. Quando o aplicador erra, o membro **sai do grupo**
(`group_replication_exit_state_action`: `READ_ONLY`, `OFFLINE_MODE`,
`ABORT_SERVER`). Nao e laco: e o no se declarar fora. E o GR **tem** precedente de
recusar por causa do modo multi — **mas no commit, nao na declaracao** (FK em
cascata com `enforce_update_everywhere_checks`, e SERIALIZABLE em multi-primary).

### A conta

- **Algum deles RECUSA a tabela por indice unico secundario?** PG nao; Galera nao,
  e certifica por essa chave de proposito; GR nao para unico secundario.
  **Convergencia dos tres: nenhum recusa. Aceite automatico.**
- **O conflito aparece em vez de sumir?** PG erra e registra com a chave e as duas
  linhas; Galera devolve `ER_LOCK_DEADLOCK` e conta; GR tira o membro com acao
  configurada. **Convergencia dos tres. Aceite automatico.**
- **Laco infinito e defeito?** Sob a regua, **nao**: o PG faz isso por escolha
  documentada. O que e defeito sob qualquer regua e o laco **calado e sem
  estrangulamento** — e e esse que o nosso tinha.

### Veredito: a (2) CONFIRMA; a (1) CHOCA com a convergencia

Nenhum dos tres recusa a tabela. E nao e o *meio* que diverge, e o
**comportamento**: «esta tabela nao replica» contra «esta tabela replica e o
conflito aparece». Nenhuma petrea nossa se opoe ao segundo. **Sem petrea
contraria, a convergencia manda.**

**E a premissa da recusa nao se sustenta como estava escrita.** Ela se justificava
com «nao ha ninguem funcionando — o arranjo de hoje nao replica, so falha calado e
para sempre». Isso vale para o par que **colide**. E falso para o par que **nao
colide**: uma tabela com primaria `porId` e unica `porEmail` replica perfeitamente
enquanto os dois nos nao criarem o mesmo e-mail, e no **unidirecional** — o caso
comum — ela nunca colide, porque so um lado escreve. A recusa na declaracao tirava
essas do ar. **A petrea «guarda nova entra pedida, nao imposta» estava batendo de
frente, e foi afastada com um argumento que nao passa pela medicao.**

**Nao medi:** quantas tabelas do nosso acervo tem unico secundario e replicam hoje
sem colidir. E esse numero que dimensiona o estrago da recusa.

**DECISAO DO DONO, 17/09/2026: trocar a recusa na declaracao por PARADA VISIVEL DO
PAR.** A tabela continua podendo nascer e replicar; no conflito, a replicacao
daquele par para, marcada, contada e gritada com o valor da chave e as duas
linhas, e entra a saida manual de pular o evento pela posicao. A saida (b) — casar
por N chaves — ganha precedente forte e receita medida (uma chave de certificacao
por indice unico, tipo promovido a exclusivo quando o valor muda), e continua
pedido proprio.

**A recusa que continua certa e nao esta em questao** e a nossa de tabela **sem
chave unica** no multi: sem chave unica nao ha identidade da linha entre
servidores, e o nosso `.reg` nao reaproveita slot, entao o numero do slot nao
serve de identidade. O Galera so *recomenda* primaria; nos recusamos, e a
restricao que causa a divergencia e essa.

---

## 293 — replicar tabela com coluna cifrada marcada como externa

- **MariaDB (3).** Cifra em repouso por servidor, transparente. E o binlog
  responde por extenso, verbatim: «**The master decrypts encrypted binary log
  events as it reads them from disk, and before its binary log dump thread sends
  them to the replica, so the replica actually receives the unencrypted binary log
  events.**» E: «**when using encrypted binary logs with replication, you can have
  different encryption keys on the master and the replica.**»
- **MySQL (2).** O mesmo, por outras palavras, verbatim: «**Data in motion in the
  replication event stream… is decrypted for transmission, and should therefore be
  protected in transit by the use of connection encryption.**»
- **PostgreSQL (4).** **Nao tem cifra em repouso no nucleo.** Tres caminhos:
  `pgcrypto` por coluna (a chave vem do cliente a cada consulta), cifra de
  particao pelo sistema de arquivos (cada replica com a chave dela, WAL em claro
  sobre TLS), e cifra do lado do cliente.
- **SQLite (1):** sai da conta.

### A conta

MariaDB e MySQL convergem **explicitamente**, cada um com uma frase dedicada. O
PostgreSQL converge **no efeito pelos dois caminhos que tem**. **3 de 3 no
comportamento — aceite automatico:**

> **A cifra em repouso e local do servidor e nao atravessa a replicacao. O que
> atravessa e o valor decifrado, num canal cifrado, e o destino recifra com o
> material dele.**

O `pgcrypto` e a excecao que confirma: quando o texto cifrado **e** o valor da
coluna, ele replica sem problema — porque ai a cifra e da **aplicacao**, e o motor
nunca precisou da chave.

### Veredito: a decisao tratava o sintoma, e o choque aparece

A premissa da celula era exata: o sal e sorteado por arquivo (`Material::novo()`),
entao «compartilhar senha E sal» nunca se satisfaz. **O que a regua acrescenta e
que os tres nunca tentaram compartilhar sal nenhum.** A condicao que nunca se
satisfaz e a condicao de um desenho que eles abandonaram.

E o terceiro caso medido — «cifra desligada na replica grava 63 bytes de texto
cifrado como se fossem o conteudo, sem erro» — **nao e argumento a favor de
recusar**: e a prova de que hoje mandamos **texto cifrado no fio**, que e
precisamente o que os tres nao fazem.

**Sendo preciso sobre o choque:** «senha nunca em texto puro» **nao** e o que isto
quebra (nenhuma senha viaja aqui), e **nao ha choque com zero dependencias**: o
desenho maduro exige o dado decifrado atravessar o fio, e o nosso aperto de mao
estilo Noise ja cifra o fio, escrito aqui. **Este caso nao e o do TLS** — o meio
existe e nao pede crate nenhuma.

O que ficava em aberto para J era uma coisa so: **se o material de cifra da coluna
no destino pode ser estabelecido sem que a origem o conheca.**

### O fato que o DONO trouxe, e que fecha essa lacuna por outro caminho

Palavra dele, 17/09/2026: *«As chaves sao uuid v7, nao podem ser diferentes na
origem destino.»* **Medido pelo integrador no fonte, e esta certo nos dois
pedacos:**

1. O `id` de cada coluna e um `Uuid::v7()` — `crates/phxsql-core/src/schema.rs:259`
   (`pub id: Uuid`), nascido em `:295` e no ramo de ausencia da v3 em `:1392`.
2. Esse id chega **identico** na replica, porque a tabela nasce do mesmo bloco de
   esquema do source, byte a byte (`servidor.rs:2796-2818`, `replica.rs:302-306` e
   `:326-332`, `catalogo.rs:599-607`).
3. **E a cifra nao conhece UUID nenhum:** `Material::novo()`
   (`crates/phxsql-store/src/cofre.rs:347-363`) sorteia o sal com
   `bytes_aleatorios(SAL_LEN)`. A unica mencao a UUID no `cofre.rs` e `:735`, sobre
   o nome do `.trash`/`.reason`.

Ou seja: **existe ja no formato um identificador provadamente igual nos dois
lados, e a derivacao da chave nao o usa.** Papel SEC foi chamado para julgar
adversarialmente se derivar o material de cifra desse UUID e seguro, contra RFC
8018 e NIST SP 800-132, e para comparar com o desenho dos tres. **Enquanto o
veredito de SEC nao volta, nada disso e plano.**

---

## 294 — criterio de eleicao

| | grandeza | escalar ou vetor? |
|---|---|---|
| PostgreSQL + Patroni (4) | LSN do WAL | **escalar** — `patroni/ha.py:34` declara `('wal_position', int)`; comparacao `if my_wal_position < st.wal_position:` (`ha.py:1446`); atraso por subtracao contra `maximum_lag_on_failover` (`ha.py:1378`) |
| MariaDB/Galera (3) | `seqno` (`wsrep_last_committed`) | **escalar**, um por cluster; o GTID e `UUID:seqno` |
| MySQL GR (2) | versao do servidor -> `group_replication_member_weight` -> menor `server_uuid` | **escalares** |
| SQLite (1) | — | sai da conta |

E o MySQL escreve o **porque** do ultimo criterio, que e a propriedade que o dono
nomeou: «This factor acts as a guaranteed and predictable tie-breaker **so that
all group members reach the same decision** if it cannot be determined by any
important factors.»

### A conta: convergencia dos tres, e num ponto mais forte do que a pergunta

1. **Nenhuma das tres grandezas e vetor por tabela.** Nem por objeto, nem por
   relacao, nem por particao. Aceite automatico.
2. **Todas sao totalmente ordenadas, e o desempate e explicito e deterministico** —
   e o MySQL registra que a razao e que todos cheguem a mesma decisao. Aceite
   automatico.
3. O unico lugar dos maduros onde a grandeza **nao** e escalar e o conjunto de
   GTIDs da failover assincrona do MySQL, e ele e vetor **por UUID de origem**,
   nunca por tabela — e a ordem que define e **parcial**, que e exatamente o risco
   que o dono descreveu.

### Veredito: **CONFIRMA**

Manter a soma escalar no `vencedor` (`crates/phxsql-server/src/cluster.rs:134-147`)
e por o vetor por tabela no pulso e no painel **como medida, nunca como voto**, e o
desenho dos tres.

**Onde divergimos, e a restricao nossa:** os tres comparam a posicao de um **diario
unico**; nos comparamos uma **soma de contagens por tabela** (`posicao_do_diario`,
`crates/phxsql-server/src/servidor.rs:3716-3748`). E por isso que a assimetria do
294 existe aqui e nao la: no LSN deles, 1.000 bytes de WAL da tabela grande e 0 da
pequena **nao** somam igual a 0 e 1.000, porque e a mesma posicao fisica e ela e
uma so. A restricao e o modelo de **arquivos separados do HFSQL**.

Isso **reforca** a decisao: o vetor por tabela e sintoma da nossa arquitetura, nao
falha do criterio, e o remedio que os tres usariam nao e vetorizar o voto, e ter
**uma** posicao.

**Peca do precedente que a nossa decisao nao tem, e que J sugere por na mesa:** o
Patroni nao compara so quem esta mais a frente — ele **desqualifica** quem esta
atrasado alem de um teto, `maximum_lag_on_failover`, **antes** de qualquer
comparacao (`ha.py:1371-1379`, `is_lagging`). E barato, nao mexe no consenso
(predicado local, igual em todo no, sobre um escalar que ja existe) e cobre um
buraco real: hoje o nosso `vencedor` promove o menos atrasado mesmo que ele esteja
arbitrariamente atras. **Vira pedido proprio.**

---

## Tabela dos vereditos

| # | decisao do dono, 17/09 07:10 | veredito da regua | decisao do dono depois da regua |
|---|---|---|---|
| 289 | carimbo `u64` de nanos com avanco forcado | **MUDA em parte** — o meio nao tem precedente; a meta e petrea e ganha | **duas colunas, 16 bytes** |
| 290 | passo no esquema, inicio na identidade do no | **CONFIRMA** | mantida |
| 292 | recusar a tabela no multi **e** parar o laco | **(2) CONFIRMA; (1) CHOCA** | **parada visivel do par** no lugar da recusa |
| 293 | recusar no motor agora | **trata o sintoma; o choque aparece** | o dono trouxe o UUID v7 que replica identico; **SEC julga** |
| 294 | manter a soma, medir por tabela ao lado | **CONFIRMA** | mantida |

## O que J NAO conseguiu medir

1. SQLite: constancia de `CURRENT_TIMESTAMP` por instrucao ou por transacao (289).
   Nao muda veredito: peso 1, e ele sai da conta nas outras quatro.
2. MySQL GR: se a certificacao anexa chaves de indices **unicos secundarios** (292).
3. O custo, em bytes e em taxa de insercao, de duas colunas de sistema contra uma
   (289). E bancada nossa (`--example onde-doi` / `carga`, com
   `cargo build --release --examples` antes, senao mede o passado).
4. Quantas tabelas do nosso acervo tem indice unico secundario e replicam hoje sem
   colidir (292). E o numero que faltava para o argumento «nao ha ninguem
   funcionando» se sustentar.
5. Se o material de cifra da coluna no destino pode ser estabelecido sem que a
   origem o conheca (293). **Papel SEC esta medindo, com o fato do UUID v7.**
6. Como as ferramentas de failover assincrona do MySQL quebram empate entre
   conjuntos de GTID incomparaveis (294).
7. Como pglogical/BDR aloca faixa de sequencia por no, e o `CREATE SEQUENCE` do
   MariaDB sob Galera (290).

## Fonte alheio lido

MariaDB 11.4: `sql/wsrep_server_service.cc`, `sql/wsrep_mysqld.cc`,
`storage/innobase/handler/ha_innodb.cc`, `sql/table.cc`, `sql/field.cc`,
`include/my_base.h`. PostgreSQL REL_18_STABLE:
`src/backend/replication/logical/worker.c`, `.../launcher.c`. Patroni:
`patroni/ha.py`. Mais os manuais oficiais de PostgreSQL 18, MySQL 8.4 e MariaDB
(funcoes de data/hora, replicacao logica e conflitos, opcoes de replicacao, Group
Replication, cifra de binlog, tabelas versionadas, variaveis do Galera).
