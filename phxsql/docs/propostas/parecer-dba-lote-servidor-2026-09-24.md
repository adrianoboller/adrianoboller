# Parecer do DBA (papel C): lote «o servidor fica de pé» (502, 452, 466, 504, 510)

24/09/2026. Revisão só de leitura do worktree `agent-a44c6a8b5120f8072` (base `e5dd077`, não
comitado). As provas estão em `scratchpad/dba-lote-servidor/` (`m1`…`m6`, com os scripts).
Os testes rodaram pelo `cargo-da-frente.sh`: 11 da lib e 4 de integração, **15/15 verdes**.

## Veredito

| pedido | veredito | por quê |
|---|---|---|
| **502** | **LIBERA COM CONDIÇÃO** (C1, C2, C3) | A filha segura a trava e o reparo do 451 vale nela (M1). Há três defeitos novos, cada um com conserto de poucas linhas. |
| 452 | LIBERA | Guarda no `Drop`, recuo 1→60 s. O recuo só cala a direção que este nó inicia. |
| 466 | LIBERA | Um escritor só (`gravar_estas`/`gravar`), com a porta na frente. Arquivo intacto medido pelo soquete. |
| 504 | LIBERA | O filtro é a 1ª linha do `main`, antes de `args` e do `Config::ler`. Medido nos dois sentidos (M6). |
| 510 | LIBERA | O backup avisa pelo carteiro. O **irmão** que ficou de fora (o job) é a C2 do 502. |

## Condições do 502 (entram antes da integração)

| # | defeito | medida | conserto |
|---|---|---|---|
| **C1** | Lápide com `quando_ms` no **futuro** (relógio que voltou depois da queda: RTC, snapshot de VM, passo do NTP) conta como última corrida **sem teto**. Antes do lote, `ultimos` e `ultimo` zeravam a cada arranque, e a hora errada não atravessava o reinício. | **M3**, lápide +1 ano: job «a cada 1 min» com `proxima` = 2027-09-24; backup «a cada 24 h» com **0 cópias** na partida (controle: 1); `parado: false`, então o vigia não avisa. | `min(quando, agora)` em `fechar_interrompidas` e em `lapide_do_backup_no_arranque`. |
| **C2** | A corrida de **job** fechada no arranque vira FALHOU só no histórico e no `stderr`, e **não passa pelo `avisar_sobre_a_corrida`**. A do backup passa pelo carteiro. Isso vale para **qualquer** queda no meio de um job (`systemctl restart`, OOM, `SIGKILL`), e não só para a H5. Agora o job não roda de novo até a próxima hora dele: é o que pg_cron e MySQL/MariaDB fazem, e está certo, mas é calado. | **M4**, rele SMTP falso: interrompida **0** e-mail; falha comum (controle) **1**. | Mandar as fechadas pelo mesmo vigia do job, depois que o `Arc` existe (no `subir_jobs`). |
| **C3** | O teto escrito em `mapa-das-threads.py` («nunca há mais filhas vivas do que chamadores… relógio (1), backup (1), tela») **não vale** para um job cujo `pedido` é `job_rodar` dele mesmo ou um ciclo A→B→A: cada nível é uma filha nova. | **M2**: **≥45 filhas simultâneas**, e só parou no teto de endereçamento que eu impus (`ulimit -v` 1,5 GB). O processo então caiu por falha de alocação, efeito do limite. Antes do lote: a mesma recursão na pilha do relógio (não medido). | Mínimo: a frase do catálogo diz a verdade. O conserto (recusar `job_rodar` em thread de família `corrida`) vira o pedido P1. |

Fora do domínio C, uma linha para o B e o H: o comentário de doc do `trava_de_dados_sem_reparo`
(451, o «por que não é `unreachable!`») ficou colado no `struct RelogioNoAr`
(`servidor.rs`, logo acima do `struct`), e a função ficou sem ele. Volta para o lugar.

## As seis perguntas

| # | resposta | prova |
|---|---|---|
| 1a | O binário anterior **não quebra**. Ele ignora `em_curso`, sobe, e mostra cada abertura como uma falha fantasma sem detalhe, o que o FORMATO §22 já diz. | **M5**: binário pré-lote (mesmo `Corrida::de_json` da base) sobe e o log fica intacto. |
| 1b | Queda entre a lápide e o fim da corrida: o arranque fecha como FALHOU e conta como a última. **Lápide órfã não existe**, porque o arranque a consome: acrescenta a linha que fecha, ou apaga o arquivo. O segundo arranque não fecha de novo, e o job volta na cadência. A exceção é a C1. | O teste `corrida_aberta_sem_fecho…` e o M3 (controle). |
| 1c | **Sem `fsync`, e está certo.** O laço é o do processo morto com a máquina de pé, e a página segue no cache do núcleo. Na queda de energia a lápide se perde e a corrida roda **uma** vez a mais, o que não é laço: o próximo `abort` deixa lápide nova. A escrita vem **antes** da filha (`registrar_inicio` e depois `executar_job_na_filha`; `gravar_lapide_do_backup` e depois `rodar_em_filha`). | Leitura mais os testes (b)/(d) do 502, verdes. |
| 1d | Cluster: a lápide de **job é por nó** (o `.log` fica ao lado do `jobs.json` de cada config). A de **backup é por destino**. Num destino compartilhado entre nós com o mesmo caminho de base, o `base` não distingue os nós (P2, ⏸). | Leitura. Não medido. |
| 2 | **O ponto crítico se sustenta.** Quem toma a trava é a filha; o pai não segura nada no `join`. A família `corrida` ≠ `servico`, então o `Drop` da `TravaMedida` repara com o guard ainda na mão, `reparos == panicos` libera o veneno, e o reparo que falha **ainda aborta** (H5). Jobs rodam com `ligacao 0`, e `begin` e `BULKINSERT` recusam: não sobra trava de transação sem `AoSair`. | **M1** (filha em pânico com escrita pela metade, `InserirDepoisDoContador`): processo vivo, depois `inserir`/`varrer`/`COUNT` **ok**, e a trava serve. `rowid`/`rowstamp` seguem sem reuso. Backup (M1-b): idem. |
| 3 | **Não há regravação.** Só o `gravar_estas` escreve o `dblink.json` (e o `gravar`, o `jobs.json`); os dois chamam `exigir_legivel` primeiro. O `salvar` clona antes, e a memória não muda sem o disco. O `op_dblink_salvar` trancado cai no `achar` (Err), pula a herança e recusa no `salvar`. | Teste `cadastro-acessorio-trancado` (5 casos pelo soquete, bytes do arquivo iguais): 3/3 verdes. |
| 4 | O `tirar_a_memoria_do_core()` é a **primeira** linha do `main`, antes de `args`, do `--senha` e do `Config::ler` (cofre, `config.phz`, chave do fio). Resíduo aceito: o `core` ainda leva os registradores (notas); não medido. | **M6**: pré-lote `00000033`, core 11,9 MB, senha **3×**; worktree `00000000`, core 64 KB, senha **0×**. |
| 5 | **Não sobe.** `alcancam-fsync-2` **23/92** no worktree = 23/92 no `main`; `rede-ou-espera` 0. Nenhum `sync_*` novo no diff, e as lápides são escritas fora da trava global. | `mapa-da-trava.py` nas duas árvores. |
| 6 | **Diff limpo.** 20 arquivos e 5 novos. Toda linha acrescentada cita só 502/452/466/504/510 (e o 451 como contexto); as 13 guardas novas do catálogo são do lote. As assinaturas do lote (`rodar_em_filha`, `abrir_ou_trancar`, `GuardaDoPulso`, `LAPIDE_DO_BACKUP`, `RelogioNoAr`) não aparecem em **nenhum** outro worktree nem no `main`. Os `servidor.rs.bak`/`cluster.rs.bak`/`dblink.rs.bak` **não estão mais** na raiz do scratchpad, então não dá para saber quem restaurou deles; o efeito procurado nas árvores deu zero. | `grep` nas árvores; contagem das citações no diff. |

## Formato em disco

| peça | mudança | migração |
|---|---|---|
| `jobs.json.log` | Um campo **aditivo** (`em_curso`) e uma linha a mais por corrida. O log cresce 2× e a cauda de 64 KiB mostra metade das corridas. | Nenhuma: o binário anterior lê (M5). |
| `.phxsql-backup-agendado.em-curso` | Arquivo novo no **destino**, e não na base, porque a base iria dentro do próprio backup. | Nenhuma. |
| `dblink.json` / `jobs.json` | Não muda. Muda a leitura: ilegível **tranca** em vez de ler vazio. | Nenhuma. |

## Pedidos novos (texto pronto, número a atribuir)

- **P1 ☐**, fica na conta: é garantia que não vale. **Job que roda `job_rodar` de si mesmo (ou ciclo A→B→A) sobe uma thread filha por nível, sem teto.** Medido: ≥45 filhas simultâneas até um teto de endereçamento de 1,5 GB (`dba-lote-servidor/m2`). Antes do 502, a mesma recursão ia na pilha do relógio. Conserto: `op_job_rodar` recusa em thread de família `corrida` («job não dispara job»), com prova pelo soquete nos dois sentidos. Nenhum dos maduros deixa um evento/job disparar outro de forma síncrona.
- **P2 ⏸**, depois da versão. **Lápide do backup em destino compartilhado entre nós do cluster:** com o mesmo caminho de base, o nó B, ao subir, apaga a lápide do nó A em curso e pula o próprio backup por um ciclo (com aviso falso). Conserto: gravar e conferir também `id_servidor`/máquina. Pré-existente e mais grave: zips do mesmo minuto colidem pelo nome, e o `manter` de um nó apaga os do outro. Não medido.
- **P3 ⏸**, depois da versão. **`corrida_em_panico` diz mais do que sabe:** classe `SP000010` «arquivo corrompido» e texto «entrou em PANICO… a trava foi reparada» saem também quando a filha **nem nasceu** (M2: `nao consegui subir a thread … os error 11`). E o FALHOU não diz que o efeito pode ter ficado: em M1, o `inserir` do job em pânico ficou gravado (`rowid 1`) com o histórico dizendo FALHOU, e quem roda de novo à mão duplica.

Cognição candidata (PENDENTE, para o integrador): *marca persistida entre arranques herda o
relógio de quem a escreveu. Lápide sem `min(quando, agora)` para o backup por um ano, e a tela
diz «não parado»* (M3).

Revisão: 6/6 perguntas respondidas, 6 medições. A % do projeto **não foi medida aqui**: o
gerador grava páginas, e esta revisão é só de leitura.
