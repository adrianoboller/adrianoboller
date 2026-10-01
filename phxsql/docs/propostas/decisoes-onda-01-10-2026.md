# Decisões da onda de 01/10/2026 — dez pedidos parados por falta de decisão

**Papel J, 01/10/2026.** Arquivo de apoio (pétrea de 12/09). Regra aplicada: cláusula «o pesquisador
decide; o dono é o impasse» — convergência dos três maduros = aceite; divergência = média ponderada
(PG 4, MariaDB 3, MySQL 2, SQLite 1); só choque com pétrea, empate real ou produto sobem.
Não edita `PENDENCIAS.md`, código nem dossiê; não comita.

**Medido nesta rodada** (máquina do contêiner, 4 núcleos, release, `commit 93fd1604`):

| o quê | número | como |
|---|---|---|
| absorver o diário local no mapa de toques (330) | **2,12–3,10 µs/evento**, 5 corridas, linear de 0,3 M a 3 M eventos | crate de rascunho fora do repo, mesma receita de `servidor.rs:5705-5750` (lotes de 500, teto 16 MiB, `valores_da_imagem`, `HashMap<String,Toque>`) |
| RAM do mapa de toques (330) | **88–114 B por chave distinta** (RSS) | idem, 0,3 M / 1 M / 3 M chaves |
| ler o diário em sequência, tabela aberta | **1,2 µs/evento** (1 M eventos) | `--example custo-do-desde 1000000` |
| peso do `.ndx` numa tabela (513) | **32,77%** do total (`.reg` 48,45%, `.log` 16,10%, `.trash`+`.reason` 2,68%) | `--example quanto-ocupa 200000 5` |
| colisão do `hash_id` u16 (329) | 2 ids **0,00153%**; 21 ids **0,31996%** (209,7×); 50 ids **1,85%**; com u32, 21 ids **4,9×10⁻⁸** | aritmética de aniversário, `python3` |

Números citados, não remedidos agora: backup **91,6–139,2 MB/s** em árvore e **26,0–33,6 MB/s** em zip
(papel C, 24/09, `parecer-dba-496-catastrofes-2026-09-24.md` §2.4); quórum 2-de-3 **0,661 ms**
em `127.0.0.1` (`bancada/quorum/`, 07/09).

---

## 207 — Quórum de escrita: o que ainda faltava decidir

Já decidido pelo dono: construir (07/09), «ok» = **aplicou e gravou em disco** (17/09), saída (i)
«aceita e avisa» (DBA, 10/09, `quorum-de-escrita.md` §5.2). Faltavam três: visibilidade durante a
espera, prazo padrão, e o que acontece com os commits SEGUINTES quando o quórum some.

**Hipóteses (antes de medir):** (V1) a linha fica invisível até o «ok» — segura a trava durante a
espera; (V2) visível já, espera fora da trava. (P1) prazo infinito; (P2) prazo finito. (D1) cada
commit espera o prazo de novo; (D2) depois do estouro o servidor degrada para assíncrono, dito em
cada resposta, até o quórum voltar.

**Os quatro:**
- PG: espera **sem prazo** e invisível — `xact.c` (REL_17) linha 1541: *«at this stage we have
  marked clog, but still show as running in the procarray and continue to hold locks»*; o doc diz
  que o commit *«may never be completed»* sem standby (https://www.postgresql.org/docs/current/warm-standby.html).
  `remote_apply` = aplicado **e** em disco (runtime-config-wal) — é o «ok» que o dono escolheu.
- MySQL: `AFTER_SYNC` padrão, invisível até o ack; estouro de `rpl_semi_sync_source_timeout`
  (10 000 ms) **volta a assíncrono** e retoma quando a réplica alcança
  (https://dev.mysql.com/doc/refman/8.0/en/replication-semisync.html).
- MariaDB: padrão `AFTER_COMMIT` (**visível** antes do ack), timeout 10 000 ms, **degrada** para
  assíncrono (https://mariadb.com/kb/en/semisynchronous-replication/).
- SQLite: não replica — peso 0 aqui.

**Decisão:** V1, P2 = **10 000 ms**, D2 com aviso por resposta.
**Número:** visibilidade: invisível PG 4 + MySQL 2 = **6** × visível MariaDB 3 = 3. Prazo: finito
MySQL 2 + MariaDB 3 = **5** × infinito PG 4. Degradar: MySQL 2 + MariaDB 3 = **5** × esperar sempre
PG 4. Custo de V1 com a trava global: piso **3,16×** por commit (0,661 contra 0,209 ms, loopback).
D1 foi recusada por raciocínio: com a trava presa, réplica morta = **10 s de servidor parado por
escrita**, para sempre.
**Implementar:** `cluster.quorum_prazo_ms` (padrão 10000) ao lado de `quorum_minimo` em `config.rs`;
espera dentro da tomada de `travar_dados` do commit, depois do `fsync` local; a réplica só confirma
depois do `fsync` da tabela, que já vem antes da posição (pedido 535); o envio vai pelo canal do pulso
(`quorum-de-escrita.md` §1.2, `replicar_empurrar`). Estouro → resposta `ok` com
`"quorum":{"pedido":N,"confirmado":k,"alcancado":false}`, estado `degradado` em `replicacao_estado`
e contador; volta ao modo síncrono no primeiro ack completo. `quorum_imposto` vira `true` no mesmo commit.
**Prova:** (1) 3 nós, quórum 2: matar as duas réplicas → o commit volta em ≤ prazo+ε com
`alcancado:false`, e o seguinte volta em < 5 ms (degradado); se o defeito for reposto (D1), o segundo
commit leva ≥ 10 s e o teste falha. (2) Leitor concorrente durante a espera não vê a linha
(se a espera sair da trava, o teste falha).
**Sobe ao dono?** Não. Cabe nas decisões de 07, 10 e 17/09; o prazo saiu da régua (5 × 4).
D2 não esconde nada: cada resposta degradada diz que não alcançou.

---

## 309 — Réplica honrar o `rownum` da imagem (via b)

**Hipóteses:** (H1) só a via (a), e o buraco antigo fica como legado; (H2) via (b) em toda
aplicação replicada; (H3) via (b) só na réplica **fiel** e no PITR, e o bidirecional continua local.

**Os quatro:** os quatro aplicam na réplica o valor que veio, sem gerar outro. PG lógico aplica a
tupla enviada (o padrão da coluna não roda no subscriber); MySQL/MariaDB em RBR gravam a imagem
inteira (`binlog_row_image=full`, https://dev.mysql.com/doc/refman/8.0/en/replication-options-binary-log.html);
SQLite aplica a changeset com os valores gravados (https://www.sqlite.org/sessionintro.html).

**Decisão:** H3. **Número:** convergência dos três (e do SQLite) → aceite automático. Precedente
interno: `carimbar_linha` (`table.rs:3725-3745`) já **honra** o `rowstamp` sob `como_replica`, pelo
mesmo motivo («gerar um local faria o retrato SHA-256 divergir»). No bidirecional o `rownum` fica
local, pela pétrea da ordem de digitação **em cada servidor** (`REPLICACAO.md` §12) — por isso H2 morre.
Ganho a mais, lido e não medido: o PITR reaplica o diário pelo mesmo `aplicar_evento`
(`REPLICACAO.md` §6) e hoje **renumera**, então um buraco histórico do original some na cópia restaurada.
**Implementar:** em `table.rs`, `rownum_para` recebe o modo; um campo `honrar_rownum` (ligado só
por `aplicar_evento`, e não por `inserir_replicado`) faz a inclusão gravar o `rownum` da imagem quando
ele é > 0 e anda o contador do `.reg` para `max(atual, veio+1)`, no espelho de `anotar_carimbo`.
Imagem com 0 (fonte anterior à coluna) continua numerando localmente.
**Prova:** fonte com buraco fabricado (rownum 1,2,4) → réplica fiel e PITR devolvem 1,2,4 e o
retrato SHA-256 bate; com o defeito reposto, saem 1,2,3 e o teste falha. Irmão que trava o velho:
o bidirecional continua dando 1,2,3 na ordem de chegada daqui.
**Sobe ao dono?** Não. O pedido dizia «decisão do dono», mas a convergência resolve e nenhuma pétrea
se opõe (na réplica fiel, honrar é preservar a ordem do source).

---

## 329 — `hash_id` de 16 bits, conferido só no par

**Hipóteses:** (H1) conferir colisão contra o **conjunto** de ids conhecidos; (H2) alargar a
origem para u32 (formato do `.log`); (H3) número **atribuído**, não hash.

**Os quatro:** PG usa **16 bits, atribuídos** — `typedef uint16 RepOriginId`, e `origin.c` procura o
primeiro livre (`for (roident = InvalidOid + 1; roident < PG_UINT16_MAX; ...)`, REL_17); colisão é
impossível por construção. MySQL e MariaDB: `server_id` de 32 bits **atribuído pelo operador**, e o
servidor recusa a replicação quando os dois lados são iguais. SQLite: não se aplica.

**Decisão:** H3 + H1, H2 recusada. **Número:** três maduros convergem em «identidade atribuída, não
hash» → aceite. 21 nós hoje: 0,31996%; com número atribuído, **0** por construção; H2 compraria
4,9×10⁻⁸ e custaria um `.log` v4 — o PG prova que 16 bits bastam quando não são sorteados.
**Implementar:** `replicacao.numero_servidor` (1..65535) em `config.rs`; ausente → `hash_id(id)`
(comportamento de hoje, guarda pedida). O número viaja na resposta de `posicao` e no `para` do
`replicar` (`servidor.rs:28275-28283`). O nó guarda `numero → id` de todo par visto (as origens que ele
puxa e os que puxam dele), durável pela mesma troca do `replicacao-posicoes.json` (pedido 535), e
recusa o par cujo número já pertence a outro id, nomeando os dois. A conferência de hoje
(`servidor.rs:5424`) vira um caso dessa.
**Prova:** três servidores, dois caixas com ids cujo `hash_id` colide (achar o par por força bruta no
teste): o central recusa o segundo com os dois nomes; reposto o defeito (só o par), o central suprime
os eventos do caixa inocente e o teste, que conta as linhas no outro caixa, falha. Comportamento velho:
sem `numero_servidor`, um par sem colisão replica igual a hoje.
**Sobe ao dono?** Não.

---

## 330 — A primeira rodada do bidirecional depois do arranque

**Hipóteses:** (H1) o custo é pico de RAM — **morta**, já lê em lotes (`servidor.rs:5714-5721`);
(H2) o custo é **tempo com a trava global presa**; (H3) o custo é o mapa sem teto.

**Medido:** **2,12–3,10 µs por evento** e **88–114 B por chave**, linear. E o achado que muda o
pedido: a absorção roda dentro de `abrir_para_bidi_sob_a_trava`, com `travar_dados()` (exclusiva)
na mão (`servidor.rs:5474` → `5503`). Raciocinado, não medido no servidor: 10 M eventos ≈ **21–31 s
de servidor parado** (escrita **e** leitura) na primeira rodada de cada tabela; 10 M chaves ≈
**0,9–1,1 GiB** de RSS.

**Os quatro:** nenhum reconstrói um mapa de chaves lendo o próprio log no arranque. PG guarda o
progresso da origem em disco (`replorigin`, persistido no checkpoint) e a resolução de conflito lê o
carimbo **da própria linha** (`track_commit_timestamp`); MySQL NDB resolve por coluna de carimbo na
linha (`NDB$MAX`); MariaDB/Galera certifica por write-set, sem mapa. Não há convergência sobre o
**meio** — só sobre o comportamento: o arranque não para o servidor.

**Decisão:** em duas partes. **(a) agora:** absorver sob a trava de **leitura**, em lotes, e só a cauda
sob a exclusiva. **(b) depois, papel C:** tirar o mapa da RAM pondo o «último toque» na linha (colunas
de sistema, mudança de formato; a lápide pode vir da `.trash`). (b) não entra nesta onda.
**Número:** a parada é 100% da absorção hoje; com (a), raciocinado, cai para a cauda (eventos
escritos durante a pré-absorção × 2,5 µs).
**Implementar:** em `servidor.rs`, antes de `abrir_para_bidi`, um laço com `travar_dados_para_ler`
(o motor já existe, `servidor.rs:2289`) que abre a tabela de leitura e chama o mesmo corpo de
`absorver_diario_local` por até N lotes (~10 ms), solta a trava, repete até faltar menos de um lote.
O corpo fica **um só**: a função recebe a `Table` aberta, venha ela de qualquer uma das duas fichas.
Publicar `toques` (número de chaves) por tabela em `replicacao_estado`, para o RSS deixar de ser calado.
**Prova:** 200 k eventos no diário local, reinício, e um `inserir` concorrente na primeira rodada:
espera máxima pela trava (telemetria) < 50 ms; reposto o defeito (absorção sob a exclusiva), a espera
passa de ~0,5 s e o teste falha. Depois, bancada de 1 dia / 1 mês / 1 ano como o pedido manda.
**Sobe ao dono?** Não.

---

## 331 — Chave composta no bidirecional

**Hipóteses:** (H1) continuar recusando; (H2) aceitar a composta, casando pela **tupla inteira**;
(H3) casar por uma coluna da composta.

**Os quatro:** PG aceita PK composta e `REPLICA IDENTITY USING INDEX` de várias colunas
(https://www.postgresql.org/docs/current/sql-altertable.html); Galera: *«multi-column primary keys are
supported»* (https://mariadb.com/kb/en/mariadb-galera-cluster-known-limitations/); MySQL Group
Replication exige PK ou equivalente, sem limite de colunas
(https://dev.mysql.com/doc/refman/8.0/en/group-replication-requirements.html); SQLite session usa a
PK, composta inclusive. Nos quatro, a identidade é a tupla: uma coluna diferente = outra linha.

**Decisão:** H2. **Número:** convergência dos quatro → aceite. «Só uma das colunas diverge» não é
conflito: é outra linha, e o «mais recente vence» do 292 vale por tupla.
**Implementar:** `bidirecional::chave_unica` devolve `(indice, Vec<usize>)` e aceita índice único de N
colunas, todas obrigatórias (ver 517). A chave canônica composta mora **no mesmo motor**:
`dblink::sincronia::chave_canonica` ganha a forma de lista, com cada parte prefixada pelo tamanho
(`"3:abc|1:7"`), e a forma de uma coluna devolve **exatamente** o texto de hoje. `tabela.buscar(indice,
&valores_da_tupla)` já aceita composta (`.ndx` desde o pedido 2). `posicao` passa a mandar também
`"chave_colunas":[...]`, e `"chave"` continua com o nome quando é uma coluna só. Sem formato novo.
**Prova:** `itens(venda,item)` com A inserindo (1,1),(1,2) e B (1,3): os dois lados terminam com três
linhas, alterar (1,2) em A não toca (1,1), e excluir (1,3) em B não toca as outras. Reposto o defeito
(casar só por `venda`), (1,2) sobrescreve (1,1) e o teste falha. O `assert!(chave_unica(&composta)
.is_none())` (`bidirecional.rs:819`) muda de sentido; o irmão que trava o velho é o de uma coluna, que
casa exatamente como hoje.
**Sobe ao dono?** Não.

---

## 416 — DELETE replicado sem imagem: o mesmo defeito do 405

**Hipóteses:** (H1) o evento de exclusão leva só o `rowstamp` (8 bytes, campo novo, formato novo);
(H2) leva a **imagem inteira** quando `imagem_da_linha` está ligada, pelo caminho que o papel multi
já usa; (H3) não conferir e documentar.

**Achado ao ler:** a premissa «pede decisão de FORMATO» está **errada**. O `.log` v2 já grava exclusão
com imagem (`imagem_na_exclusao`, `table.rs:733-739` e `6193`), com a marca do 498 sabendo achá-la na
lixeira (`table.rs:6575-6590`). Falta só ligá-la fora do multi e conferir do lado da réplica.

**Os quatro:** DELETE leva a imagem de antes nos quatro — PG manda a chave antiga (identity
`DEFAULT`) ou a linha inteira (`FULL`); MySQL e MariaDB `binlog_row_image=full` por padrão
(*«Only the before image is logged»* para DELETE, todas as colunas); SQLite: *«The payload of a DELETE
change consists of the values for all fields of the deleted row»*.

**Decisão:** H2. **Número:** quatro convergem → aceite; e H2 custa **zero formato**, contra um campo
novo em H1. Custo: a exclusão passa de 44 B para 44 B + imagem (≈184 B por evento com imagem, papel C
24/09) — exclusão é a operação rara desta casa.
**Implementar:** onde o servidor liga `imagem_no_diario` pelo config, ligar também `imagem_na_exclusao`
(ver 564: isso tem de ser **um lugar só**). Em `aplicar_evento_interno`, braço `Exclusao`
(`table.rs:6755`): com imagem presente, ler o payload do slot e comparar `rowstamp` com o da imagem
— o mesmo `conferir_identidade_da_alteracao`, renomeado `conferir_identidade` e usado nos dois braços.
Imagem ausente (fonte antigo) ou carimbo 0 → sai de cena, como na alteração.
**Prova:** duas origens no mesmo database, exclusão da segunda origem cai num rowid que é de outra
linha → a réplica para nomeando os dois carimbos; reposto o defeito, a linha errada some com `Ok` e o
teste falha. Comportamento velho: réplica fiel de uma origem só, exclusão aplicada igual a hoje.
**Achado colateral, lido e não medido (provável defeito ativo):** no papel multi, a exclusão **pela
porta** não liga `imagem_na_exclusao` — `abrir_travada_com` (`servidor.rs:12327`) liga só
`imagem_no_diario`, e o único `ligar_imagem_na_exclusao(true)` do servidor é o do aplicador
(`servidor.rs:5529`). Tabela sem `softdeleted` exclui fisicamente por padrão (`servidor.rs:22931`), o
outro lado recebe exclusão sem imagem, `aplicar_por_chave` devolve `Err` (`servidor.rs:5766-5771`) e o `?`
de `aplicar_lote_bidi` (`:5546`) para a rodada — o laço que o 292 matou, por outro caminho. Merece
pedido próprio, com teste de dois servidores multi.
**Sobe ao dono?** Não.

---

## 424 — O ledger proibido cresce pela réplica

**Hipóteses:** (H1) guarda no caminho de leitura (`Schema::desserializar`); (H2) a réplica
**recusa** criar tabela de ledger com coluna marcada; (H3) a réplica cria, **grita e conta**, e o
censo roda em todo nó.

**Os quatro:** PG lógico não replica DDL — o subscriber declara a tabela, então as regras **dele**
valem; MariaDB reexecuta o DDL na réplica com as checagens dela; MySQL tem a escolha explícita
`REQUIRE_TABLE_PRIMARY_KEY_CHECK = STREAM` (padrão: honra o source) | `ON` (impõe a da réplica).
SQLite: não se aplica. (O mecanismo de ledger em si não existe em nenhum dos quatro — parecer C,
23/09, §4.)

**Decisão:** H3. **Número:** a régua dá «a réplica julga» PG 4 + MariaDB 3 = **7** × MySQL 2 — mas
choca com a pétrea «guarda nova entra pedida, não imposta»: recusar pararia a replicação de uma cadeia
que nasceu legítima. **A pétrea ganha, e o choque fica escrito aqui.** H1 morre pelo motivo do C
(quebra a réplica legítima). População hoje: **0** cadeias no repositório e **4** de demonstração no
disco, nenhuma com coluna candidata (parecer C, 23/09, §3) — o risco é de amanhã.
**Implementar:** um motor só, `phxsql_store::ledger::censo(dir) -> Vec<(tabela, e_ledger,
colunas_marcadas, blocos)>`, lendo o byte de `dado_pessoal` pelo `Schema` (não por texto); chamado
por (1) `--example censo-do-ledger`, (2) `garantir_tabela_da_replica` (`servidor.rs:29721`) quando o
esquema que chega é ledger com coluna marcada → linha no log e contador `ledger_marcado_recebido` em
`replicacao_estado`, sem recusar. Corrigir `ledger.rs:63-66` e `FORMATO.md:675` para «não desfaz, **e
acompanha a réplica**».
**Prova:** source com cadeia marcada gravada antes da guarda → a réplica cria, o contador vira 1 e o
censo da réplica a lista; reposto o defeito (sem chamar o censo na criação), o contador fica 0 e o
teste falha. Comportamento velho: `cadeia_marcada_gravada_antes_da_guarda_abre_le_e_grava` continua verde.
**Sobe ao dono?** Não para decidir: a pétrea já decide. Se o dono quiser o `ON` do MySQL (recusar na
réplica), é ele quem reabre a pétrea.

---

## 513 — Backup segura a trava global durante a cópia inteira

**Hipóteses:** (H1) ficar como está e só avisar o tempo; (H2) copiar sob a trava de **leitura**:
leitores andam, escritores esperam; (H3) **duas passadas**: cópia sem trava + acerto curto sob a
exclusiva, consistente no **fim** (o desenho do `mariabackup`/`BACKUP STAGE` e do rsync duplo);
(H4) cópia difusa + replay do diário na restauração (`pg_basebackup` puro).

**Os quatro:** os três maduros copiam **sem parar a escrita**: PG `pg_basebackup` (cópia difusa +
WAL); MariaDB `BACKUP STAGE` — o InnoDB se copia com DML andando e só `BLOCK_COMMIT` trava, curto
(https://mariadb.com/kb/en/backup-stage/); MySQL `LOCK INSTANCE FOR BACKUP` *«permits DML during an
online backup»* e bloqueia só arquivo criado/renomeado e manutenção
(https://dev.mysql.com/doc/refman/8.0/en/lock-instance-for-backup.html). SQLite: o `sqlite3_backup_step`
trava só durante cada passo, e quando a escrita vem **do mesmo processo e da mesma conexão** o destino
é atualizado junto (https://www.sqlite.org/backup.html) — é o nosso caso: um processo só.

**Decisão:** H2 já, H3 em seguida; H4 recusada. **Número:** hoje 100 GB = **12–18 min** em árvore e
**50–64 min** em zip, com tudo parado (C, 24/09). H2: a leitura deixa de parar, a escrita ainda espera
a cópia inteira. H3, raciocinado e não medido: a parada vira o acerto — caudas dos arquivos que só
crescem, slots tocados e o `.ndx` (**32,77%** da tabela, medido) só das tabelas escritas durante a
cópia. Se 10% das tabelas forem escritas: ~3,3 GB a 91–139 MB/s ≈ **24–36 s**, contra 50–64 min. O
zip sai da trava inteiro (comprime a árvore pronta). H4 recusada: exige `imagem_da_linha` ligada em
todo servidor (sem ela o evento não traz o dado) e o `aplicar_evento` recusa cópia adiantada
(guarda do rowid, `table.rs:6785`).
**Implementar:** (H2) `op_backup` (`servidor.rs:23669`) troca `travar_dados()` por
`travar_dados_para_ler()` — antes, provar que nada grava sob a trava de leitura (a dúvida do C:
cura de cabeçalho, `.pag`). (H3) em `phxsql_store::backup`: fase 0 sob a exclusiva (ms) — tamanho de
cada arquivo e `eventos()` de cada tabela, e o **congelamento** (`phxsql_store::congelamento`, o
motor do 421) passa a recusar manutenção (`reparar`, `reindexar`, `acrescentar_coluna`, cifrar, criar
e apagar tabela) com `4006, repetir:true`; fase 1 sem trava, copia tudo; fase 2 sob a exclusiva: para
cada tabela cujo `eventos()` andou, recopia o cabeçalho e os slots dos rowids dos eventos novos, as
caudas do `.reg`/`.log`/`.trash`/`.reason`/`.lgpd` e o `.ndx`, `.bin` e `.memo` inteiros; tabela parada
não paga nada. Zip = árvore em pasta temporária + compressão depois da trava (conferir espaço antes).
O `fsync` e o manifesto continuam fora da trava (524/552).
**Prova:** backup de 1 GB com uma thread gravando inserção, alteração e exclusão durante a fase 1;
restaurar e comparar com o retrato do servidor no instante da fase 2 (SHA-256 por tabela) e
`conferir` limpo; reposto o defeito (pular a fase 2), o retrato diverge e o teste falha. Telemetria:
espera máxima de um `inserir` durante o backup < duração da fase 2 + 10%; com H1, ≈ backup inteiro.
**Sobe ao dono?** Não. O momento do retrato passa do início para o fim da cópia, como no `mariabackup`.
Não é promessa de produto nova: o manifesto já diz quando foi feito.

---

## 517 — Índice único sobre coluna que aceita NULL como identidade

**Hipóteses:** (H1) `chave_unica` exige que **todas** as colunas da chave sejam obrigatórias; (H2)
aceita, e o evento com chave NULL para o par (`ParadaDaTabela`); (H3) ignora as linhas com chave
NULL (o SQLite).

**Os quatro:** PG: `USING INDEX` exige índice *«unique, not partial, not deferrable, and include only
columns marked NOT NULL»*; MySQL Group Replication: *«primary key equivalent where the equivalent is a
non-null unique key»*, e `binlog_row_image=minimal` só identifica por chave única *«all columns NOT
NULL»*; MariaDB/InnoDB escolhem como índice agrupado só o único sem NULL; SQLite session *«ignores all
such rows»* com NULL na PK.

**Decisão:** H1. **Número:** três convergem → aceite. H3 (peso 1) perderia linha calada.
**Implementar:** no predicado `serve` de `bidirecional::chave_unica` (`bidirecional.rs:163`), acrescentar
`!esquema.colunas()[c].nullable` para cada coluna; a recusa da tabela ganha o motivo «a chave X aceita
nulo: torne a coluna obrigatória». Como defesa, no braço de exclusão e no de alteração
(`servidor.rs:5861`, `:5883`): `achadas.len() > 1` → erro com nome, nunca `first()`.
**Prova:** índice único em coluna anulável → `chave_unica` = `None` e a tabela aparece em `recusas`;
dois nós com chave NULL em linhas diferentes, exclusão de uma → a outra fica. Reposto o defeito, a
exclusão apaga a linha do outro nó e o teste falha. Comportamento velho: PK obrigatória casa igual.
**Sobe ao dono?** Não. Um par que já replica uma tabela com chave anulável passa a recusá-la, **com o
motivo escrito**: é defeito ativo (dado errado), não guarda nova.

---

## 564 — A recuperação grava o diário sem imagem

**Hipóteses:** (H1) `completar` liga a imagem lendo o config, como o `abrir_travada`; (H2) a política
da imagem passa a morar **num lugar só**, que todo `abrir_qualificada` herda; (H3) gravar no `.log`
(cabeçalho) que aquele diário leva imagem.

**Os quatro:** a política de log de replicação é do **servidor**, não de quem abre a tabela: PG
`wal_level`; MySQL e MariaDB `binlog_format`/`binlog_row_image` globais, aplicados no ponto único de
escrita do log. E o que a recuperação completa é o que a réplica recebe, no mesmo formato (o MySQL
decide o commit recuperado **pelo** binlog).

**Decisão:** H2. **Número:** convergência dos três → aceite; e a lei «função e comando não se duplicam»
decide entre H1 e H2: hoje a decisão está escrita em **5** lugares (`servidor.rs:3655`, `5528-5529`,
`12327`, `24374`, `26573`) mais a herança da filha (`table.rs:3274-3275`), e o 564 é o **sexto** que
esqueceu. H1 seria o sétimo. H3 muda formato para guardar o que é configuração. «Lido, não medido» do
parecer continua: o teste abaixo é a medida.
**Implementar:** `phxsql_store::catalogo` — a `Instancia` recebe a política (`imagem_no_diario`,
`imagem_na_exclusao`) uma vez no arranque do servidor (multi → as duas ligadas; source → do config;
embutido → desligada), o `Database` a herda, e `Database::abrir_qualificada` a aplica. Os cinco
`ligar_imagem_*` do servidor somem. `Database::recuperar_marcas` (`marca.rs:1447`) passa a gravar com
imagem sem ninguém lembrar.
**Prova:** source com `imagem_da_linha`, réplica, pânico na passada do COMMIT, arranque que completa
a marca → a réplica aplica o evento completado; reposto o defeito, ela para com «veio sem imagem» e o
teste falha. Comportamento velho: servidor isolado continua gravando o evento sem imagem (44 B).
**Sobe ao dono?** Não.

---

## Matriz de evidência

| pedido | fonte primária | o que resolve | custo |
|---|---|---|---|
| 207 | PG `xact.c:1541` (REL_17), docs warm-standby e runtime-config-wal; MySQL semisync; MariaDB semisync | invisível até o ack; prazo 10 s; degradar dizendo | 3,16× por commit (piso, loopback) |
| 309 | PG lógico, MySQL `binlog_row_image`, SQLite sessionintro | réplica aplica o valor que veio | um campo de modo no `Table` |
| 329 | PG `origin.c:285`, `RepOriginId uint16`; `server_id` MySQL/MariaDB | identidade atribuída | campo de config + mapa durável; zero formato |
| 330 | medição própria (2,1–3,1 µs/ev, 88–114 B/chave) | absorção fora da trava exclusiva | reordenar uma fase |
| 331 | PG ALTER TABLE, Galera limitations, MySQL GR | tupla inteira é a identidade | assinatura de `chave_unica` |
| 416 | PG identity, MySQL `binlog_row_image`, SQLite sessionintro | DELETE leva imagem de antes | +≈140 B por exclusão; zero formato |
| 424 | PG (sem DDL lógico), MariaDB, MySQL `REQUIRE_TABLE_PRIMARY_KEY_CHECK` | censo em todo nó; pétrea ganha da régua 7×2 | um motor de censo |
| 513 | `pg_basebackup`, MariaDB `BACKUP STAGE`, MySQL `LOCK INSTANCE FOR BACKUP`, SQLite backup API | cópia sem parar a escrita | fase 2 ≈ `.ndx` das tabelas escritas |
| 517 | PG `USING INDEX`, MySQL GR, InnoDB, SQLite session | chave só NOT NULL | um predicado |
| 564 | PG `wal_level`, MySQL/MariaDB `binlog_*` globais | política num lugar só | tirar 5 cópias da decisão |

## Lacunas (o que não medi)

1. **513:** a fase 2 é raciocinada, não medida. Decide na bancada: backup de 1 GB com escritor
   concorrente e espera máxima de um `inserir`. E falta provar, antes da H2, que nada grava sob a
   trava de leitura.
2. **330:** o tempo do 1 dia / 1 mês / 1 ano depende da taxa de eventos real de um caixa, que não
   existe aqui. Medi o custo por evento e por chave; a taxa vem do cliente.
3. **207:** ida e volta com `fsync` na réplica fora do `127.0.0.1` — o 0,661 ms é o piso.
4. **416 colateral** (exclusão pela porta no multi sem imagem) e **331 colateral** (no bidirecional,
   a alteração que **muda a chave** vira inserção do outro lado, porque a imagem é o «depois»; o
   SQLite registra isso como DELETE+INSERT e o PG manda a chave antiga): os dois são lidos e não
   medidos, e não têm pedido.
5. As fontes de MySQL/MariaDB e da documentação do PG foram lidas pela página do manual; o PG teve o
   fonte lido (`xact.c`, `origin.c`, REL_17_STABLE). O MySQL/MariaDB não teve.
