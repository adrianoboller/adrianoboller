# Triagem das onze decisões paradas no dono, contra o help e o fonte dos quatro

**Papel J (pesquisador), 23/09/2026.** Só leitura. Fonte primária em cada
afirmação: manual oficial da versão corrente ou o **fonte** do motor, com URL ou
`arquivo:linha`. Onde não medi, digo «não medi».

**Por que este documento existe.** Ordem do dono, 23/09/2026: *«o agente
pesquisador e o agente que cuida das hipóteses são qualificados a buscar no help
do PostgreSQL, MySQL, MariaDB, SQLite as informações e só em último caso ver
comigo.»* A lei está no `CLAUDE.md` da raiz, na seção dos motores, com o caso que
a fundou: o pedido **340** chegou à mesa com a frase «entra por aceite
automático» escrita no próprio corpo, e o integrador perguntou assim mesmo.
Decisão do dono gasta à toa não volta.

**Nota de método:** não houve fan-out — o papel J rodou as onze frentes sozinho,
e isso está dito porque o briefing do papel prevê `pesquisa-motor` e
`pesquisa-bancada`, e eles não foram chamados. **Nada aqui foi medido em
bancada**; toda medição deste documento é de **fonte** — doc oficial, código dos
motores baixado e lido nesta sessão, e código nosso lido no repositório. Onde um
número decidiria a proposta, o número está nomeado como pendente de bancada.

**O SQLite sai da conta** em tudo que envolve replicação, cluster ou ligação de
saída: ele não as tem. Dito em vez de contar peso 1 para uma opinião que ele não
tem — `https://www.sqlite.org/whentouse.html`: *«If there are many client
programs sending SQL to the same database over a network, then use a
client/server database engine instead of SQLite.»*

---

## Placar — e é ele que diz se a regra nova valeu

| Classe | Quantos | Quais |
|---|---:|---|
| **A** — convergência, entra SEM pergunta | **4** | 251, 255, 300, 309 |
| **B** — voto ponderado, decidido sem pergunta | **0 inteiros, 1 meia** | a metade «reparar sozinho» do 255 (6 × 4) |
| **C** — é do dono mesmo | **4** | 325, 333, 337, 368 |
| **Fora da fila** — a premissa caducou ou ele já decidiu | **3** | 274 (entregue pelo 378), 293 e 294 (decididos por ele em 17/09 07:10 UTC) |

**A fila do dono cai de 11 para 4, e as 4 viram 5 perguntas de uma palavra.**
Três dos onze nunca deviam estar ali: um foi implementado e fechado em
22/09/2026, e dois ele já respondeu em 17/09/2026 — o texto da pendência traz a
decisão dele por extenso e o item continuou marcado como parado.

E o aprendizado que essa terceira linha ensina, que é a lei da casa apontada para
nós mesmos: **a lista do que falta também é palpite até alguém medir** — inclusive
a lista das decisões que esperam o dono.

---

## 251 — P2P: o AAD do selo por endereço contra a identidade sem domínio · **A**

**Premissa medida hoje.** `grep -rl "correio_" crates/` devolve **0**: as sete
tabelas do correio só existem no papel, e **nenhuma mensagem foi gravada**. O
próprio `docs/CORREIO-FORMATO.md:471-476` põe a pergunta e já nomeia a saída
(«migrar o AAD para `id` antes de gravar qualquer mensagem»). Trocar hoje é de
graça; depois é migração de dado cifrado.

**A pergunta de fato não é de comportamento de banco** — é de formato de correio
e de criptografia. Mas ela tem um precedente medido nos três, porque «qual é a
identidade de um nó» é exatamente o que a replicação deles resolve:

| motor | identidade do nó | endereço de rede entra? |
|---|---|---|
| PostgreSQL (4) | `system_identifier`, gerado no `initdb` de `gettimeofday()` + PID e guardado no `pg_control` — `src/backend/access/transam/xlog.c:5011-5013` e `:4193` (REL_17_STABLE) | não |
| MariaDB (3) | GTID `domínio-server_id-sequência`, ex. `0-1-10`; `gtid_slave_pos` «contains the GTID of the last transaction applied» ([doc](https://mariadb.com/docs/server/ha-and-performance/standard-replication/gtid)) | não |
| MySQL (2) | `server_uuid`: *«If `data_dir/auto.cnf` is not found, generate a new UUID and save it to this file»*, e *«do not attempt to write or modify this file»* ([doc 8.4](https://dev.mysql.com/doc/refman/8.4/en/replication-options.html)) | não |
| SQLite (1) | fora da conta | — |

**3 de 3: a identidade de um nó é um id gerado e estável, nunca o endereço de
rede.** Nenhuma pétrea se opõe — Ed25519 e X25519 já são desta casa, escritos
aqui, e a mudança não puxa crate nenhuma.

**E o dono já decidiu esta metade.** `docs/VISAO.md`, Pilar 2, ordem de 16/09:
*«identidade que não depende de um domínio»*. O AAD por endereço **contradiz uma
ordem que já existe**; perguntar de novo é gastar decisão para ouvir o que ele já
disse.

**Onde divergimos, e a restrição que causa.** Os três amarram a identidade ao
**canal** e pronto: não há relé no meio, então a autenticidade do canal cobre o
conteúdo. O nosso P2P prevê par-relé, e aí o canal deixa de cobrir — **a
assinatura Ed25519 por mensagem não tem precedente nos quatro porque eles não têm
o problema**. Sem precedente e sem pétrea contra, ela **não é decisão do dono**:
é do papel C (formato) com parecer do SEC.

**O que fazer:** AAD do selo passa a ser o `id` (não o endereço), antes de a
primeira mensagem existir; assinatura por mensagem entra como parecer de C+SEC,
não como pergunta. **Não medi** o custo da assinatura por mensagem — a bancada
que decidiria é o custo de um Ed25519 por mensagem contra o tamanho médio do
envelope.

---

## 255 — `.ndx` sujo depois de uma queda: varrer o byte 52 no arranque? · **A** (com um **B** dentro)

**Premissa medida hoje.** `crates/phxsql-store/src/ndx.rs:659-662` já lê o byte
52 **na abertura do arquivo** (`let sujo = cab[52] != 0;`). Ou seja: a detecção
que o item propõe pôr no arranque **já existe, no lugar certo**. O que falta é só
o aviso a quem opera.

| motor | varre tudo no arranque? | repara sozinho? | quem opera fica sabendo? |
|---|---|---|---|
| PostgreSQL (4) | **não** — recupera pelo WAL e diz no log: `errmsg("database system was not properly shut down; automatic recovery in progress")`, `src/backend/access/transam/xlogrecovery.c:954` | **não** para índice inválido: *«This index will be ignored for querying purposes… The recommended recovery method in such cases is to drop the index and try again»* ([CREATE INDEX](https://www.postgresql.org/docs/17/sql-createindex.html)) | **sim, pelo catálogo**: *«The psql `\d` command will report such an index as `INVALID`»* |
| MariaDB (3) | **não** — repara **ao ABRIR a tabela**: `ha_maria::open` põe `HA_OPEN_ABORT_IF_CRASHED` (`storage/maria/ha_maria.cc:1153-1156`) e `ha_maria::auto_repair` dispara em `HA_ERR_CRASHED_ON_USAGE` (`:2824-2830`) | **sim, de fábrica**: `MYSQL_SYSVAR_SET(recover_options, …, HA_RECOVER_BACKUP\|HA_RECOVER_QUICK, …)`, `storage/maria/ha_maria.cc:241-243` (11.4) | **sim, pelo log**: `sql_print_warning("Checking table:   '%s'")` e `sql_print_warning("Recovering table: '%s'")`, `:2438` e `:2445` |
| MySQL (2) | **não** — InnoDB recupera pelo redo; MyISAM repara ao abrir | **não de fábrica**: `MYSQL_SYSVAR_SET(recover_options, myisam_recover_options, …, nullptr, nullptr, 0, …)` — **default 0 = OFF**, `storage/myisam/ha_myisam.cc:108-112` (8.4.6) | **sim, pelo log** quando ligado: `LogErr(WARNING_LEVEL, ER_CHECKING_TABLE, …)` / `ER_RECOVERING_TABLE`, `:1462` e `:1465`; e a corrupção de índice do InnoDB é **persistida**: *«InnoDB writes a corruption flag to the redo log, which makes the corruption flag crash-safe»* ([doc](https://dev.mysql.com/doc/refman/8.4/en/innodb-recovery.html)) |
| SQLite (1) | **não** — rollback do hot journal **ao abrir o arquivo**: *«This recovery activity happens completely automatically and transparently to the user»* ([doc](https://www.sqlite.org/atomiccommit.html)) | sim (rollback, não reparo de índice) | transparente |

**A: 4 de 4 — ninguém varre no arranque. Todos agem na ABERTURA.** É isso que
dissolve a dúvida do item: «reconstruir sozinho no arranque muda o tempo de
subida» deixa de ser um dilema, porque **o arranque não é o lugar**. E nós já
estamos no lugar certo desde o `ndx.rs:662`.

**A: 3 de 3 — o estado fica visível a quem opera**, pelo log do servidor (PG,
MySQL, MariaDB) ou pelo catálogo (PG). É a metade que falta aqui: hoje o cliente
recebe a recusa e o operador não vê nada. **Entra sem pergunta**, pelo gancho que
já existe: a saúde do disco do pedido 249 tem fila e carteiro.

**B — reparar sozinho:** divergem. Contra reparar sozinho, PG **4** + MySQL **2**
= **6**; a favor, MariaDB **3** + SQLite **1** = **4**. **6 × 4 → não reparar
sozinho.** (E se o SQLite sair da conta, por rollback não ser reparo de índice,
fica 6 × 3 — o resultado não muda.)

**O que fazer:** manter a detecção na abertura; emitir o aviso para a fila de
saúde e para o painel; **não** varrer o diretório no arranque e **não**
reconstruir sozinho. Custo de implementação: **raciocinado, não medido** — o
`open`+`read` de 64 bytes por `.ndx` que o item propunha **deixa de ser
necessário**, então o custo cai para o de uma mensagem na fila.

---

## 274 — DbLink para PhxSql não tem como pedir o túnel · **FORA DA FILA — entregue**

**A premissa caducou, e a medição é de leitura direta do fonte.** O pedido **378**
está **☑️ FECHADO em 22/09/2026** e diz no corpo: *«Sucede o 371/274 com o alcance
medido»*. Conferido:

- `crates/phxsql-server/src/dblink/mod.rs:183` — `cifra: Option<bool>` **privado**
  (três estados), e `:194` — `pub chave_do_fio: String` (o pino).
- `dblink/mod.rs:493` — `self.cifra.unwrap_or(crate::config::CIFRA_DE_SAIDA_PADRAO)`:
  **ausente = ligado**.
- `dblink/phx.rs:35-52` — a frase «a `std` não traz TLS» **não existe mais**; o
  comentário hoje diz *«a frase que dizia o contrário nasceu falsa»* e explica
  por que o `cifrar` vem **antes** do `autenticar`.
- `docs/DBLINK.md:646` — a seção agora é «O fio do terceiro motor: o túnel, e
  **ligado de fábrica**», com os dois campos na tabela.

O pedido **371**, que carregava o resto do 274, está **☑️** também.

**Não se pergunta ao dono o que já está no disco.** O 274 sai da fila por
premissa caducada, e a lição é a da casa: *medir a premissa do item vem antes de
propor o item — inclusive quando o item é nosso*.

---

## 293 — replicar tabela com coluna EXTERNA marcada · **FORA DA FILA — ele decidiu em 17/09**

O próprio corpo do pedido traz, por extenso: **«DECIDIDO PELO DONO, 17/09/2026
07:10 UTC — recusar no motor agora»**, e no mesmo parágrafo a continuação:
*«O envelope da §11.5 continua sendo o desenho definitivo, e quando existir é
**ele** que levanta a recusa.»*

O que chegou **depois** da decisão dele não a reabre, e confirma o alvo:

- **A régua dos motores** (07:5x UTC, `docs/propostas/regua-dos-motores-decisoes-289-294-2026-09-17.md`):
  **3 de 3** — a cifra em repouso é local do servidor e não atravessa a
  replicação; o que atravessa é o valor decifrado, em canal cifrado, e o destino
  recifra com o material dele.
- **O veredito do SEC** (08:0x UTC, `docs/propostas/parecer-sec-uuid-como-sal-2026-09-17.md`):
  o UUID como sal **não entra** (A1–A4), e a recomendação é o desenho dos três
  **mais** o envelope §11.5.
- E o fecho do próprio item: *«A recusa no motor que o dono decidiu às 07:10
  continua de pé e continua sendo a única coisa que fecha o caso 3.»*

**Classificação: A** para o alvo (decifrar antes de enviar / envelope §11.5),
por convergência já medida e já autorizada pelo texto dele. **Nada a perguntar** —
o que falta é engenharia, não decisão.

---

## 294 — o critério de eleição do cluster · **FORA DA FILA — ele decidiu em 17/09**

Mesma situação, e também no próprio corpo: **«DECIDIDO PELO DONO, 17/09/2026
07:10 UTC — manter a soma no critério, e medir por tabela AO LADO.»**

A régua dos motores confirmou com precedente e nomeou cada grandeza: **LSN** no
PostgreSQL com o Patroni, **`seqno`** no Galera, **peso do membro + UUID** no
Group Replication — todas **escalares**, e o MySQL documenta o motivo exato
(*«so that all group members reach the same decision»*). Nenhuma é vetor por
tabela.

**Classificação: A** (confirmado). **Nada a perguntar.** O que sobrou virou o
pedido 313 (desqualificar o nó atrasado antes de comparar, como o `is_lagging`
do Patroni).

---

## 300 — garantias que não valem na réplica · **A**

Três achados. Os três têm precedente e nenhum bate em pétrea.

### (4) Escrita local na réplica pula um evento do source em silêncio — o mais grave

**Premissa medida hoje:** `servidor.rs:3807-3825` — `posicao_do_diario` **soma a
contagem local** sobre `db.todas_as_tabelas()`. A posição é um contador **nosso**,
não uma coordenada da origem.

| motor | de onde vem a posição da réplica | a réplica pode escrever? |
|---|---|---|
| PostgreSQL (4) | o LSN do WAL **da origem**; o standby só replica o que veio | **não**: *«All such connections are strictly read-only; not even temporary tables may be written»*, e a lista proibida inclui *«Sequence updates: `nextval()`, `setval()`»* ([hot standby](https://www.postgresql.org/docs/17/hot-standby.html)) |
| MariaDB (3) | `gtid_slave_pos` *«contains the GTID of the last transaction applied»* — e o GTID é `domínio-server_id-sequência`, cunhado pela **origem** | `read_only` (não medi o padrão) |
| MySQL (2) | `gtid_executed` / `mysql.gtid_executed`: *«used to preserve the assigned GTIDs of all the transactions applied»*, e o GTID é `source_id:transaction_id` com `source_id` = `server_uuid` **da origem** ([doc](https://dev.mysql.com/doc/refman/8.4/en/replication-gtids-concepts.html)) | `read_only`/`super_read_only` (não medi o texto) |
| SQLite (1) | fora da conta | — |

**3 de 3: a posição da réplica é a coordenada da ORIGEM, nunca uma contagem
local.** Por isso o defeito descrito no item **não existe lá**: uma escrita local
não pode mover uma posição que não é local.

**Onde divergimos, e a restrição nossa:** os três têm **um diário único** por
servidor; nós temos um `.log` por tabela, porque o modelo de arquivos separados
do HFSQL não tem diário único. Então a receita deles — «use a coordenada da
origem» — chega aqui como **«a posição é a do evento que veio, por tabela»**, e
não como «troque o contador». Isso é inspiração, não cópia: a divergência é
causada pelo nosso formato.

**Entra sem pergunta.** E o segundo meio da convergência também: a réplica
**recusa escrita local** em vez de avisar por `eprintln!` (`servidor.rs:2519-2522`)
— com a ressalva da casa, **guarda nova entra pedida, não imposta**: quem já
escreve numa réplica hoje não pode parar de um dia para o outro, então a recusa
nasce no modo `somente_leitura` que o próprio servidor já anuncia no arranque.

### (3) O comentário diverge do código

`cluster.rs:176` diz *«Posicao local do diario, somada sobre as tabelas
replicadas»*; `servidor.rs:3825` soma `db.todas_as_tabelas()`. Os três só contam
o que é **efetivamente replicado** (filtros explícitos: `replicate-do-db`,
publicações). Consertado pela (4), o (3) some junto — a coordenada da origem só
existe para tabela que a origem mandou.

### (§2.7) Não há contador de órfãs na réplica

**A réplica não julgar FK converge com o PostgreSQL, e ele diz isso por extenso:**
*«Since foreign keys are implemented as triggers, setting this parameter to
`replica` also disables all foreign key checks, which can leave data in an
inconsistent state if improperly used.»*
([session_replication_role](https://www.postgresql.org/docs/17/runtime-config-client.html))
É a mesma decisão do nosso `julga_integridade` (`table.rs:1504-1506`), e o nosso
motivo está medido («0 dos 2 eventos»). O MariaDB vai pelo outro lado — *«cascading
deletes and updates based on foreign key relations are not written to the binary
log»*, isto é, a réplica reexecuta e portanto julga. **Do MySQL 8.4 eu não medi**
o comportamento do aplicador em RBR. Vote se quiser: PG **4** (não julga) contra
MariaDB **3** (julga) → não julgar; mas o voto pende da célula que não medi, e
por isso **não é ele que decide**: a decisão já é nossa e já é medida.

**O que está aberto é só o contador**, e aí há convergência limpa:

| motor | contador de divergência do aplicador |
|---|---|
| PostgreSQL (4) | `pg_stat_subscription_stats.apply_error_count` e `sync_error_count` ([doc](https://www.postgresql.org/docs/17/monitoring-stats.html)) |
| MySQL (2) | `performance_schema.replication_applier_status_by_worker`: `LAST_ERROR_NUMBER`, `LAST_ERROR_MESSAGE`, `LAST_ERROR_TIMESTAMP` ([doc](https://dev.mysql.com/doc/refman/8.4/en/performance-schema-replication-applier-status-by-worker-table.html)) |
| MariaDB (3) | `Last_Errno`/`Last_Error` do `SHOW REPLICA STATUS` (não medi o texto verbatim) |

**3 de 3: divergência do aplicador é CONTADA e visível, nunca silenciosa.**
Contador de órfãs na réplica entra sem pergunta.

**Custo:** raciocinado, não medido. O que decidiria na bancada é o custo do
contador no laço do aplicador — e ele é da ordem de um incremento, contra os
17.450 eventos/s já medidos.

---

## 309 — a réplica honrar o `rownum` que vem na imagem · **A**

**Premissa medida hoje:** `table.rs:4508+` — o ramo `Insercao` de
`aplicar_evento_interno` chama `self.inserir(&valores)`, e o `inserir` chama
`numerar_linha` (`table.rs:3498`). **A réplica gera o `rownum` dela.** Confirmado.

| motor | a réplica regera o número, ou aplica o que veio? |
|---|---|
| PostgreSQL (4) | **aplica o que veio**: *«The data in serial or identity columns backed by sequences will of course be replicated as part of the table, but the sequence itself would still show the start value on the subscriber»* ([restrições](https://www.postgresql.org/docs/17/logical-replication-restrictions.html)); e no standby físico **não pode** regerar — `nextval()` é operação proibida durante a recuperação |
| MariaDB (3) | **aplica o que veio** em RBR: *«each insert, update, or delete performed by the statement for each row is logged to the binary log separately»* ([formatos](https://mariadb.com/kb/en/binary-log-formats/)) |
| MySQL (2) | **aplica o que veio**: *«Row-based replication… simply replicates the value returned by the function or stored program, so its effect on table rows and data is the same on both the source and replica»* ([SBR × RBR](https://dev.mysql.com/doc/refman/8.4/en/replication-sbr-rbr.html)) |
| SQLite (1) | fora da conta | — |

**3 de 3, e o PostgreSQL chega a proibir a réplica de gerar.** A via (b) é o
comportamento dos três; a via (a), que já fechou no pedido 291, é a nossa versão
do que eles fazem na **origem**.

**Conferência das pétreas, uma a uma:**

- **Ordem de digitação sagrada / slot nunca reaproveitado:** não é tocada.
  `rowid` (posição no `.reg`) e `rownum` (coluna contada) são grandezas
  diferentes: `aplicar_evento` já casa por **rowid** (`table.rs:4441-4455`), e a
  via (b) muda só o **valor da coluna**. Honrar o número da origem torna a
  réplica **mais** fiel, e não menos.
- **Pai antes do filho / data do filho:** não são tocadas.
- **Zero dependências:** não é tocada.

**Onde divergimos, e a restrição:** eles aplicam num diário **único e ordenado**;
nós aplicamos por tabela. Por isso a via (b) **não pode** ser ligada no
`Papel::Multi` (bidirecional), onde o `rownum` é local por desenho
(`servidor.rs:4874-4876`) — e aí o precedente maduro é outro e também é dos três:
faixas disjuntas (`auto_increment_increment`/`auto_increment_offset` na
replicação circular do MySQL/MariaDB), que é exatamente o `Sequence` **com faixa**
que o papel C recomendou no parecer das vinte caixas.

**O que fazer:** via (b) ligada **por modo** — honra a imagem em `Papel::Replica`,
mantém o número local em `Papel::Multi`. Entra sem pergunta.

---

## 325 — 20 caixas de supermercado e 1 servidor · **C — decisão de produto**

**Isto não é pergunta de comportamento de banco**, e digo de cara em vez de
forçar pesquisa que não cabe: é cenário de operação e decisão de abrir uma frente.
Mas há uma medição que **cabe e decide o enquadramento**, e é a de que a receita
não existe lá fora:

- **PostgreSQL, MySQL, MariaDB:** cliente-servidor. Não há gravação local no
  cliente com sincronização posterior no núcleo de nenhum dos três. O Galera e o
  Group Replication exigem **quórum online** — o oposto do cenário.
- **SQLite:** é embarcado e **não tem sincronização**; o próprio manual manda
  usar cliente-servidor quando os dados estão na rede
  ([whentouse](https://www.sqlite.org/whentouse.html)).

Ou seja: **offline-first com store-and-forward no caixa é território que os
quatro não pisam.** O inventário do item já mediu que três das seis pernas não
existem em linha de código nenhuma, e que nenhum dos 324 pedidos anteriores
nomeia replicação N-way. Pesquisa não resolve: é frente nova, com formato novo
(`Sequence` com faixa, `PSCH` v10) e prazo.

**Pergunta, em uma frase:**
> **Abro a frente das N pontas — cada caixa como `source` e o servidor central
> como `replica` multi-origem, com `Sequence` por faixa no `PSCH` v10 — sim ou
> não?**

---

## 333 — chat, robô de mensagens e agente de IA no PhxMail · **C — decisão de produto (e uma pétrea)**

**Também não é pergunta de comportamento de banco**, e as duas decisões abertas
do item são de produto — nenhum dos quatro motores tem chat, robô ou push de
celular. Pesquisa dos quatro aqui seria ruído, e ruído gasta a atenção dele igual.

O que a pesquisa **já** resolveu e não volta à mesa (está no
`docs/propostas/chat-e-robo-no-phxmail-2026-09-17.md`): as licenças (Boost 1.0 do
TDLib pode; GPLv3 e AGPL-3.0 **não ler**; as especificações do Signal são domínio
público), a premissa caída do WhatsApp, e o long-poll **recusado com número** —
`conexoes_web_max = 64`, uma thread por conexão, então 64 janelas de chat matam a
porta HTTP e a administração recebe 503.

**Duas perguntas, cada uma de uma palavra:**

> **(a) O chat é ponta-a-ponta (e então o servidor não busca no texto nem o robô
> lê) ou cifrado só em repouso (e então os dois funcionam) — «e2e» ou «repouso»?**

> **(b) Push de celular com o aplicativo fechado exige TLS, e TLS bate na pétrea
> de zero dependências: escrevo TLS aqui, como foi o SHA-256, ou o produto nasce
> sem push — «escrever» ou «sem push»?**

A (b) é o choque com pétrea no formato que a própria lei manda: o comportamento
(conexão cifrada) é convergência dos três; o **meio** (puxar uma crate) não passa
sem ele.

---

## 337 — o parecer externo corrige o raciocínio do nosso ACID · **C** (mas a metade (1) é **A**)

### (1) O argumento do ACID estava errado — **A, entra sem pergunta**

O parecer está certo, e os quatro manuais o provam em uma linha cada:

| motor | reivindica ACID? | nível de isolamento **padrão** |
|---|---|---|
| PostgreSQL (4) | *«has been ACID-compliant since 2001»* ([about](https://www.postgresql.org/about/)) | *«Read Committed is the default isolation level in PostgreSQL.»* ([13.2.1](https://www.postgresql.org/docs/17/transaction-iso.html)) |
| MariaDB (3) | InnoDB ACID | *«The default level is `REPEATABLE READ`»* ([SET TRANSACTION](https://mariadb.com/kb/en/set-transaction/)) |
| MySQL (2) | *«MySQL includes components such as the InnoDB storage engine that adhere closely to the ACID model»* ([15.1](https://dev.mysql.com/doc/refman/8.4/en/mysql-acid.html)) | *«The default isolation level for InnoDB is REPEATABLE READ.»* |
| SQLite (1) | *«SQLite implements serializable transactions that are atomic, consistent, isolated, and durable»* ([transactional](https://www.sqlite.org/transactional.html)) | serializável |

**4 de 4 reivindicam ACID, com TRÊS padrões diferentes.** Logo o padrão de fábrica
**não** é o que decide a reivindicação — e o `CLAUDE.md` diz hoje que o que nos
derruba é «só o I, e só por padrão», com o PostgreSQL peso-4 sendo o
contraexemplo vivo. **O raciocínio está errado e o conserto entra sem pergunta**,
porque a **conclusão não muda**: continua valendo não repetir *ACID compliant* em
documento técnico e não reivindicar `SERIALIZABLE` sem prova. Corrigir a razão de
uma lei cuja conclusão sobrevive é documentação, não revogação.

### (2) `cargo vendor` contra a justificativa da pétrea — **C, choque com pétrea**

O fato técnico do parecer é verdadeiro: `cargo vendor` + `--offline` compila sem
rede sem exigir reescrever nada. Isso **não revoga a pétrea** — receita de fora
não revoga lei nossa —, mas enfraquece a **razão escrita** dela («foi o que
permitiu `cargo build --offline`»). E trocar a razão de uma pétrea é dele: o texto
é dele.

**Pergunta, em uma frase:**
> **Troco a razão escrita da pétrea de zero dependências, de «compila offline»
> para «instalação simples», mantendo a pétrea intacta — sim ou não?**

---

## 368 — o `.lgpd` não tem expurgo, e o prazo é do dono · **C — prazo/retenção** (o mecanismo é **A**)

**Premissa medida hoje:** `grep -rn "expurg" crates/` só encontra o expurgo da
**lixeira** (`table.rs:5338`, `lixeira.rs:538`, `motivo.rs:114`) — **não há
expurgo de `.lgpd`**. Premissa viva.

### O mecanismo: convergência, entra sem pergunta

| motor | como se apaga histórico |
|---|---|
| MySQL (2) | por **arquivo inteiro**: *«After their expiration period ends, binary log **files** can be automatically removed»* ([binlog](https://dev.mysql.com/doc/refman/8.4/en/replication-options-binary-log.html)); e `PURGE BINARY LOGS` apaga arquivos |
| MariaDB (3) | idem, e o fonte diz onde: *«possible purges happen at startup and at binary log rotation»* (`sql/sys_vars.cc:1244-1252`, 11.4) |
| PostgreSQL (4) | por **segmento inteiro**: *«When old WAL segment files are no longer needed, they are removed or recycled»* ([WAL config](https://www.postgresql.org/docs/17/wal-configuration.html)) |

**3 de 3: apaga-se a JANELA inteira, nunca o registro.** É exatamente o que o
papel C propôs — expurgo por **volume**, pelo corte que a trilha já usa
(`trilha.rs:552`, `crate::diario::paginacao`), sem reescrever arquivo append-only
nenhum e sem mudar formato. Entra sem pergunta.

### O número: nem convergência nem voto resolvem

| motor | retenção padrão |
|---|---|
| MySQL (2) | **2.592.000 s = 30 dias** (`binlog_expire_logs_seconds`) |
| MariaDB (3) | **0 = nunca expira**: `expire_logs_days … DEFAULT(0)` (`sql/sys_vars.cc:1251`) e `binlog_expire_logs_seconds … DEFAULT(0)` (`:1264`), 11.4 |
| PostgreSQL (4) | não há prazo: o WAL sai quando não é mais necessário, e `wal_keep_size` é tamanho, não tempo |

**Os três dão três respostas diferentes, e uma delas é «nunca».** Não há
convergência, e o voto ponderado não se aplica: a grandeza nem é a mesma (segundos,
tamanho, nada). E o prazo de uma trilha de LGPD não é grandeza técnica — é
obrigação legal e política de retenção da empresa. **É dele.**

**Pergunta, em uma frase:**
> **Quantos dias o `.lgpd` guarda antes do expurgo por volume?**

(Âncora medida, para ele não decidir no vácuo: o MySQL entrega **30 dias** de
fábrica; o MariaDB entrega **nunca**.)

---

## O que eu NÃO medi

Dito por extenso, porque número citado é número que não se mede:

1. **Nada foi medido em bancada nesta frente.** Toda medição é de fonte.
2. **O custo do aviso de `.ndx` sujo** (255) na fila de saúde — raciocinado.
3. **O custo do contador de órfãs** (300) no laço do aplicador — raciocinado.
4. **O custo de uma assinatura Ed25519 por mensagem** (251) — raciocinado.
5. **O aplicador do MySQL 8.4 em RBR conferindo FK** (300 §2.7) — não achei a
   afirmação verbatim no manual; a célula fica vazia e o voto que dependia dela
   não foi usado para decidir.
6. **O texto verbatim de `read_only`/`super_read_only`** (MySQL/MariaDB) — a
   página de variáveis do servidor é grande demais e voltou truncada duas vezes;
   a convergência do (4) do pedido 300 foi fechada pelo PostgreSQL verbatim e
   pelos GTIDs dos outros dois, que bastam.
7. **`Slave_skipped_errors`/`Last_Error` do MariaDB** — citado de memória do
   `SHOW REPLICA STATUS`, **não conferido**; a convergência dos contadores está
   provada por PG e MySQL, e a terceira célula fica marcada.

## Um achado de brinde: as linhas citadas em `PENDENCIAS.md` envelheceram

Reconferi **cada** `arquivo:linha` que os onze pedidos citam, e **três dos que eu
ia repetir já apontavam para outro lugar** — o código andou desde 17/09:

| o que a pendência cita | onde está hoje |
|---|---|
| `julga_integridade` em `table.rs:1315-1317` (#300) | `table.rs:1504-1506` |
| o aviso da réplica sem `somente_leitura` em `servidor.rs:2315-2322` (#300) | `servidor.rs:2519-2522` |
| o `rownum` local do bidirecional em `servidor.rs:4222-4224` (#309) | `servidor.rs:4874-4876` |
| `posicao_do_diario` em `servidor.rs:3343-3376` e a soma em `:3361` (#294, #300) | `servidor.rs:3807` e `:3825` |
| o comentário «somada sobre as tabelas replicadas» em `cluster.rs:165-166` (#300) | `cluster.rs:176` |
| a paginação da trilha em `trilha.rs:525` (#368) | `trilha.rs:552` |
| `pub struct Definicao` em `dblink/mod.rs:115` (#371) | `dblink/mod.rs:131` |

Os três **conteúdos** continuam corretos: é só a coordenada que moveu. Mas é o
mesmo naipe do número digitado à mão — **referência de linha em documento também
envelhece calada**, e quem a seguir daqui a um mês vai ler outra coisa e achar
que a pendência mentiu. Não proponho gerador para isso (seria uma catraca nova
sem defeito medido que a motive); registro o número medido: **reconferi 10
referências e 7 estavam deslocadas** — as 3 que continuam certas são
`cluster.rs:134-147` (`vencedor`), `trilha.rs:41-52` e o conteúdo de todas as
outras, que moveu de linha sem mudar de sentido.

## As recusas medidas desta rodada

Porque recusa medida poupa mais tempo depois que aceite:

- **Varrer o byte 52 de todo `.ndx` no arranque (255): RECUSADO.** 4 de 4 motores
  agem na **abertura** do arquivo, não no arranque, e nós já agimos lá
  (`ndx.rs:662`). A varredura compraria nada e pagaria tempo de subida.
- **Reconstruir o índice sozinho no arranque (255): RECUSADO por voto, 6 × 4.**
- **Perguntar ao dono sobre 274, 293 e 294: RECUSADO.** Um está entregue no
  disco, dois trazem a decisão dele por extenso no próprio corpo.
- **Pesquisar os quatro motores para 325, 333 e 368: RECUSADO em parte.** Chat,
  robô, push e prazo de retenção não são comportamento de banco; o que os quatro
  têm a dizer sobre 368 é o **mecanismo** (e foi usado), não o número.

## Arquivos que este documento mediu

| origem | o que saiu daqui |
|---|---|
| `crates/phxsql-store/src/ndx.rs:659-662` | a detecção do `.ndx` sujo já é na abertura |
| `crates/phxsql-store/src/table.rs:4441-4455`, `:4508+`, `:3498` | a réplica regera o `rownum`; `aplicar_evento` casa por rowid |
| `crates/phxsql-server/src/servidor.rs:3807-3825` | a posição do cluster é contagem local sobre `todas_as_tabelas()` |
| `crates/phxsql-server/src/cluster.rs:176` | o comentário que diverge do código |
| `crates/phxsql-server/src/dblink/mod.rs:183,194,493` e `dblink/phx.rs:35-52` | o túnel do DbLink existe e é ligado de fábrica |
| `docs/VISAO.md` (Pilar 2) | o dono já mandou «identidade que não depende de um domínio» |
| `docs/CORREIO-FORMATO.md:471-476` | o AAD por endereço, e a saída já nomeada |
| `grep -rl "correio_" crates/` = **0** | nenhuma mensagem gravada: trocar o AAD é de graça hoje |
| `grep -rn "expurg" crates/` | só a lixeira; o `.lgpd` não tem expurgo |
