# Pedido 605 — o `fsync` da pasta protege um TERCEIRO? Hipóteses, fontes e medição segura

Papel J, 01/10/2026. Ciclo da pétrea (hipóteses → fonte → verificação → decisão).
Só leitura e pesquisa: nenhum `mount`, nenhum ioctl, nada compilado.

**Premissa do 605 (parecer C, raciocinado):** A cria a tabela; sob a trava ela já
fica visível; o `fsync` da pasta sai da trava (586/589/591/595). B insere nela,
faz o `fsync` dele e ouve «ok» antes do `fsync` da pasta de A. Uma queda leva a
tabela e a linha de B?

## 1. Hipóteses, escritas antes de medir

| # | Hipótese | Se verdadeira |
|---|---|---|
| H1 | ext4 **com diário**: o `fsync` de B comete a transação jbd2 que contém a entrada de A | premissa morre nesse FS |
| H2 | ext4 **sem diário**: o `fsync` de B grava o inode dele e só sobe à pasta em caso estreito | premissa vale |
| H3 | o `inserir` de B não faz `fsync` (o «ok» de B não promete disco) | 605 é inócuo |
| H4 | Windows: o `fsync` de pasta não existe no motor | 605 é subconjunto de um furo maior |

## 2. O que o fonte diz (Linux v6.18, o núcleo deste contêiner)

| Fonte | O que resolve |
|---|---|
| `fs/ext4/namei.c:2808` `ext4_create` → `ext4_add_nondir:2779` | inode e entrada de diretório nascem no **mesmo handle** jbd2 |
| `fs/ext4/ext4_jbd2.h` `ext4_update_inode_fsync_trans` | `i_sync_tid`/`i_datasync_tid` = tid do último handle que sujou o inode ⇒ ≥ tid da criação |
| `fs/ext4/fsync.c:97-115` `ext4_fsync_journal` | `fsync` espera `commit_tid` (`:102`) via `ext4_fc_commit` |
| `fs/jbd2/journal.c:787` `jbd2_complete_transaction` | espera o tid; jbd2 comete em ordem ⇒ tudo antes dele também está no disco |
| `fs/ext4/fast_commit.c:66-67` | com `fast_commit`, o commit leva **todas** as entradas de diretório e inodes da fila, não só o do chamador |
| `fs/ext4/fsync.c:82-95` `ext4_fsync_nojournal` | sem diário: grava os buffers e o inode **do arquivo**; sobe à pasta só por `ext4_sync_parent` (`:46`) |
| `fs/ext4/namei.c:2461` | `EXT4_STATE_NEWENTRY` só se liga quando a entrada exigiu **bloco novo** de diretório — e o 1º `fsync` (o de A) o limpa |
| `crates/phxsql-store/src/sincronia.rs:324-326` | fora de Unix, `sincronizar_diretorio` é `Ok(())` |
| `docs/propostas/parecer-dba-fsync-seletivo-2026-09.md` §4 | o `inserir` faz 8 `fsync` (2 do `.ndx`) |

**Medido agora, sem tocar em nada:** o `/` deste contêiner é ext4 **sem diário** —
`/proc/fs/jbd2/` vazio e `/sys/fs/ext4/vda/journal_task` = `<none>`. A bancada
daqui roda exatamente no regime de H2.

**Veredito das hipóteses (raciocinado do fonte, não medido em queda):**

- **H1 se sustenta** no ext4 com diário (padrão das distribuições), com ou sem
  `fast_commit`: o «ok» de B implica a entrada de A no disco. No XFS a mesma
  conclusão vem do `xfs_log_force_seq` em ordem de sequência do CIL —
  raciocinado, fonte não lido nesta rodada.
- **H2 se sustenta** no ext4 sem diário (e ext2): o bloco de diretório com a
  entrada de A fica sujo até o *writeback* (`commit=5`/expira 30 s); a queda
  deixa o inode de B gravado e órfão (`e2fsck` → `lost+found`). **Tabela e linha
  de B somem.**
- **H3 morre:** o inserir faz `fsync` (8 por linha).
- **H4 vale:** no Windows nem A tem a garantia da pasta; NTFS não lido — lacuna.

Conclusão: **o 605 é real**, dependente do sistema de arquivos — e o regime em
que ele vale é o da própria bancada. Fica na conta (garantia que não vale).

## 3. O que os quatro fazem ao criar arquivo de tabela

| Motor | Fonte | Meio | Comportamento para o terceiro |
|---|---|---|---|
| PostgreSQL 17 (4) | `smgr/md.c:190` `mdcreate` sem `fsync` de pasta; `catalog/storage.c:186` `log_smgrcreate`; `access/transam/xact.c:1491` `XLogFlush` antes de `:2349` `ProcArrayEndTransaction` | WAL sequencial; o *redo* recria o arquivo | tabela **invisível** até o commit estar no disco |
| MariaDB 11.4 (3) | `fil/fil0fil.cc:2044-2059` `FILE_CREATE` + `log_write_up_to(..., true)` **antes** do `os_file_create` | redo durável antes de o arquivo existir | o terceiro **espera** no MDL exclusivo |
| MySQL 8.x (2) | `os/os0file.cc:3160` `os_parent_dir_fsync_posix` logo após `O_CREAT`; MDL exclusivo do DDL (refman 8.4 «Metadata Locking») | `fsync` da pasta na criação | o terceiro **espera** no MDL |
| SQLite (1) | `os_unix.c:6773` `UNIXFILE_DIRSYNC` só para *journal* novo; `:3941-3953` uma vez, no 1º `xSync` | `fsync` da pasta preguiçoso, do criador | trava única de escrita — não há terceiro na janela |

**Convergência dos três maduros, no COMPORTAMENTO: a criação é durável antes de
ser utilizável por outra sessão.** Os meios divergem (log antes, `fsync` na
criação, flush antes da visibilidade) — o aceite é do comportamento, não do
meio. **Aceite automático.** Nada nosso se opõe.

**Divergência na janela** (o que o terceiro vê enquanto a criação não é
durável): esperar = MariaDB 3 + MySQL 2 = **5**; não enxergar = PostgreSQL **4**.
Régua: **esperar**.

## 4. O meio (nosso), com as recusas

| Meio | Custo | Decisão |
|---|---|---|
| (a) `fsync` da pasta de volta para baixo da trava | catraca `alcancam-fsync-2` 23 → **29** (medido no 589); catraca não sobe | **recusado** |
| (b) toda escrita consulta um conjunto de pastas pendentes (proposta do parecer C) | uma consulta a mais no laço quente, e um portão novo em cada operação que toca tabela (inserir, alterar, FK da filha, réplica…) — a que alguém esquecer é a porta | **recusado** (raciocinado; lei do motor único) |
| (c) **nasce reservada**: sob a trava a tabela entra no catálogo como «nascendo» (nome reservado, inutilizável); `fsync` fora da trava; publica numa trava curta; só então «ok». Quem acha «nascendo» espera fora da trava global | zero no laço quente (o estado sai da mesma busca de catálogo); a espera só existe na janela | **recomendado** |

Por que (c) diverge das três origens, e qual restrição causou: o PG e o MariaDB
compram «durável antes de visível» com um log sequencial que nós não temos
(o `.reg` é a verdade, não um diário); o MySQL compra com `fsync` sob o MDL, que
aqui é a trava global e a catraca proíbe. Ficou o estado intermediário, que é
o MDL deles sem segurar a trava global durante o `fsync`.

Excluir: o simétrico («morrendo») **não** se recomenda sem caso — o terceiro
que recria o mesmo nome faz `fsync` da mesma pasta e leva o `unlink` junto;
outro terceiro dependente da ausência não foi achado. Lacuna, para o M3.

## 5. Medição segura — o que cada meio prova e o que não prova

Disponibilidade medida agora: **sem** `/dev/mapper` (sem `dm-flakey`/`dm-log-writes`),
**sem** `/dev/kvm` e sem `qemu` (sem VM descartável), `tracefs` suportado mas não
montado, `strace` 6.8 presente, módulo `loop` presente.

| Meio | Derruba FS? | Prova | Não prova |
|---|---|---|---|
| **M1** fonte do núcleo (feito, §2) | não | o mecanismo de H1/H2 | que o 6.18 deste host se comporta igual sob queda |
| **M2** `strace -f -tt -y` de A e B concorrentes, alargando a janela com `-e inject=fsync:delay_enter=…` restrito ao `fsync` da pasta de A | não | que o «ok» de B sai **antes** do `fsync(pasta)` de A no nosso código; com (c), que não sai mais. É a **prova nos dois sentidos** da guarda, e roda sem root | perda em disco |
| **M3** estados de queda reconstruídos (modelo ALICE, OSDI'14 / CrashMonkey, OSDI'18): do traço do M2, montar numa pasta de rascunho a cópia que cada modelo permite — (i) sem diário: entrada só persiste com `fsync` da pasta; (ii) com diário: prefixo de transações — e abrir com o motor | não | o que a recuperação do motor faz em cada estado permitido; quantifica «tabela some / linha de B some» por modelo | que o núcleo obedece ao modelo |
| **M4** queda real em ext4 sobre `loop` (duas imagens: com e sem diário), `FS_IOC_SHUTDOWN` + `NOLOGFLUSH`, N=30, k/N | **sim, o da imagem** | perda real neste núcleo | queda de energia com cache volátil do disco (o `loop` sobre arquivo guarda tudo o que o ext4 já emitiu — modelo otimista) |

**Condições do M4, todas obrigatórias, e nenhuma é «conferir o caminho»:**

1. `set -euo pipefail`; imagem e ponto de montagem **dentro** da pasta de rascunho.
2. `losetup` devolve `/dev/loopN`; `findmnt -no SOURCE PONTO` tem de ser **esse**
   `/dev/loopN`, e `mountpoint -q PONTO` verdadeiro.
3. A conferência decisiva é **no mesmo descritor que recebe o ioctl**: abrir o
   ponto, `fstat` no descritor, `st_dev` == `rdev` do `/dev/loopN` **e** ≠
   `st_dev` de `/`; senão sai sem ioctl. Conferir pelo caminho e depois abrir de
   novo reabre a corrida que derrubou o contêiner.
4. Nenhuma outra frente no ar (o orquestrador agenda); `salvar-frentes.sh` antes.
5. Sem VM descartável aqui, o M4 **não roda neste contêiner** até existir uma.

**Decisão sobre a medição:** M2 + M3 bastam para decidir e para provar o
conserto; o M4 só mudaria o rótulo «ativo no ext4 com diário», que o fonte já
responde (não é). O M4 fica como lacuna, não como bloqueio.

## 6. Decisão

1. **605 fica na conta**, como defeito real em ext4 sem diário (o regime medido
   desta bancada) e no Windows; inócuo no ext4 com diário (fonte, não queda).
2. **Comportamento:** criação durável antes de utilizável por terceiro — aceite
   automático (PG + MariaDB + MySQL). Na janela, o terceiro **espera** (5 × 4).
3. **Meio:** (c) «nasce reservada». (a) recusado pela catraca (23 → 29); (b)
   recusado pelo laço quente e pelo portão espalhado.
4. **Prova:** guarda pelo M2 (vermelha com o código de hoje, verde com (c));
   M3 para os dois modelos de FS. M4 só em VM descartável.

## 7. Lacunas

- XFS e btrfs: raciocinado (XFS cobre por ordem do CIL; btrfs registra por
  inode — **pode não** cobrir os arquivos que B não sincroniza). Não lidos.
- NTFS/Windows: sem `fsync` de pasta no motor; comportamento do `$LogFile` não lido.
- Se `strace -e inject` respeita o filtro `-P` (só a pasta de A): conferir na bancada.
- Excluir: terceiro dependente da ausência — nenhum caso achado.
- A ordem da criação no `.log` da réplica com (c): o evento tem de sair só
  depois da publicação — não conferido.

## Sobe ao dono

Nada. Não há choque com pétrea, nem empate (5 × 4), nem produto.
