# Pedido 299 — a atomicidade de um commit atravessando a réplica: o que já fecha, o que sobra, e o desenho do resto

10/10/2026 · papel J (pesquisador), com o papel C (DBA) consultado pela leitura dos pareceres dele
(`recuperacao-e-replica-desenho-unico.md`, 676–722) · só leitura, sem código · HEAD `24558294`.
Nível: modelo forte (garantia de dado, formato em disco, recuperação).

Decisão do dono que motiva este documento (09/10/2026, «alta segurança e fidelidade dos dados»):
**a atomicidade de um commit entre tabelas tem de atravessar a réplica; deixa de ser preço
declarado.**

---

## 0. Resumo — a premissa do 299 estava velha

A linha do 299 (17/09/2026) descreve um sistema que **não existe mais**. Medido e lido hoje:

| Frase do 299 (17/09) | Hoje (HEAD `24558294`) | Quem fechou |
|---|---|---|
| «o `.log` não tem id de transação» | `.log` v4, cabeçalho 44 → **52 B**, `tx` por tomada da trava (`log.rs:594` `proximo_tx`; `FORMATO.md` §4) | 676, 684 |
| «um commit chega como N eventos soltos» | o `Juntador` (`replica.rs:850`) funde as tabelas pela ordem dos `tx`; o grupo vai ao disco sob **uma** tomada | 676 |
| «lote de 500 corta o commit» | a transação só é aplicada **inteira na mão**; o teto é 64 MiB (`log.rs:689`) e a origem **recusa** no `COMMIT`/carga acima dele | 676, 685, 686 |
| «a réplica que cai no meio fica com meio commit» | o grupo grava a marca `.tx` antes do 1.º evento; o arranque **completa para a frente** antes de a porta abrir | 682, 698, 699, 701 |
| «e não há como saber» | marca em voo + relatório do arranque; `transacoes_em_pedacos`, `commits_mistos`, `origem_sem_id_de_transacao` | 676, 684 |
| erro de dado no meio do grupo | completa o resto na hora, com a mesma trava; o que não fecha fica marcado no disco | 713, 722 |
| quórum e bidirecional tabela a tabela | o mesmo `Juntador` | 681, 698, 722 |

**Medido agora** (`cargo test -p phxsql-server --test venda-inteira-na-replica --test
venda-inteira-na-queda-da-replica --test venda-inteira-pelo-quorum --test
venda-inteira-na-queda-do-bidi`, 10/10/2026, árvore do HEAD): **23 de 23 verdes** — fio derrubado
no 2.º e no 4.º `replicar`, `SIGKILL` no meio do grupo, `SIGKILL` entre o `.reg` e o diário, erro
de dado injetado no meio, quórum com leitor em laço, bidi com conflito no meio.

Então o 299 **não pede mais o desenho grande** (id de transação, diário de transação, fronteira no
fio, marca na réplica) — os quatro já estão no código. O que ele pede é **fechar quatro restos que
a leitura achou** (§3), três deles com o mesmo defeito de forma: **a parada é por TABELA onde
deveria ser por TRANSAÇÃO.**

---

## 1. As hipóteses, escritas antes de medir

A ordem do pedido mandava propor o meio. As quatro hipóteses do enunciado foram escritas antes da
leitura, e a leitura as resolveu assim:

| | Hipótese | Resultado |
|---|---|---|
| H1 | id de transação no evento do `.log` + a réplica aplica só grupos completos | **já existe** (676). A recusa do papel C de 17/09 («tira o cabeçalho de 44 bytes») foi superada pela decisão do dono de 07/10: o cabeçalho cresceu para 52 B antes do selo |
| H2 | diário de transação separado, replicado | **morta.** Seria um segundo motor para a pergunta «o que é um commit?» que o `tx` do evento já responde — lei «função e comando não se duplicam». E exigiria posição global, que o `posicao` já dá como fronteira (`REPLICACAO.md` §8.2 item 2) |
| H3 | marca de fronteira no fio | **já existe na forma barata**: o `posicao` é a fronteira (conta os eventos com a trava de escrita da origem na mão); o fim da transação se deduz pela mudança de `tx` na cabeça de cada fila. Um evento-marcador de commit (o *commit record* do PostgreSQL, o *XID event* do binlog) não compra nada além disso aqui |
| H4 | a réplica aplica sob uma marca `.tx` própria e completa no arranque, como o ACID-C local | **já existe** (682/698/699/701/713/722), e é por ela que a queda do processo não deixa meia venda |
| **H5** (nova, da leitura) | **o resto é a parada por tabela**: onde algo PARA (continuidade rompida, divergência determinística, corte do PITR), a parada cai no meio de uma transação | **sustentada** — §3, R1 a R3 |
| H6 (nova) | a réplica precisa de *undo* para desfazer a metade | **morta pela pétrea.** Desfazer devolve slot; o `.reg` não reaproveita slot. Os maduros desfazem (rollback); nós **ensaiamos antes** e **andamos para a frente** — a divergência registrada na §4 |

---

## 2. Como os quatro resolvem — fonte primária, e a régua

| Comportamento | PostgreSQL (4) | MariaDB (3) | MySQL (2) | SQLite (1) |
|---|---|---|---|---|
| **a réplica aplica a transação da origem como unidade** | sim: `apply_handle_begin` abre, `apply_handle_commit_internal` comete (`src/backend/replication/logical/worker.c`, REL_17, l. 1010–1048, 2260–2300); o físico só torna visível no *commit record* (MVCC) | sim: *«An event group is a collection of events that are always applied as a unit»* (mariadb.com/kb/en/gtid) | sim: transação do binlog com GTID; *«committed together with the transactions»* (dev.mysql.com/doc/refman/8.4/en/replication-solutions-unexpected-replica-halt.html) | sim: *«All changes made by these functions are enclosed in a savepoint transaction»* (sqlite.org/session/sqlite3changeset_apply.html) |
| **a posição avança junto com o dado (crash-safe)** | `replorigin_session_origin_lsn = commit_data->end_lsn` **antes** do `CommitTransactionCommand()` (worker.c l. 2286–2289) | *«Updates to the state are done in the same transaction as the updates to the data»* (`mysql.gtid_slave_pos`) | *«the replica's progress information … is always consistent with what has been applied»* | n/a (sem posição) |
| **erro no aplicador: nada da transação fica, e o FLUXO para** (não só a tabela) | `AbortOutOfAnyTransaction(); … PG_RE_THROW()` (worker.c l. 4497–4505); *«A conflict that produces an error will stop the replication»* (docs, logical-replication-conflicts) | o SQL thread para; a transação volta | o SQL thread para; a transação volta | *«the savepoint transaction is rolled back, restoring the target database to its original state»* |
| **pular a transação é ato manual, nomeado pelo id** | `ALTER SUBSCRIPTION … SKIP (lsn = …)` | `gtid_slave_pos` / `sql_slave_skip_counter` | transação vazia com o GTID | — |
| **PITR para em fronteira de transação** | *«we only consider stopping before COMMIT or ABORT records»* (`xlogrecovery.c` REL_17 l. 2634); `recovery_target_inclusive` fala de *commit time* | grupo do binlog sem `COMMIT`/`XID` no fim não comete (raciocinado da forma do binlog, não lido no fonte) | idem | — |
| **assinatura parcial (subconjunto de tabelas) entrega o subconjunto da transação** | sim, a publicação é por tabela | sim, `replicate-do-table` | sim | — |

**Régua.** As cinco primeiras linhas: **PG, MariaDB e MySQL convergem** (9 × 0; com o SQLite onde
se aplica, 10 × 0) → **aceite automático**, sem pergunta. A linha do PITR: PG lido no fonte (4);
MySQL/MariaDB raciocinados; mesmo só com o PG, 4 × 0 → entra. A última linha também converge, e no
sentido contrário ao que se esperaria: **replicar um subconjunto de tabelas entrega meia transação
nos três** — isso entra como limite declarado, não como defeito (R5).

**O meio diverge, e a restrição nossa que causou a divergência tem nome.** Os quatro *desfazem*
(rollback/savepoint) a metade aplicada. Aqui desfazer devolveria slot, e **a ordem de digitação é
sagrada** (o `.reg` nunca reaproveita slot). Daí as duas peças nossas que não existem lá:
o **ensaio antes do primeiro evento** (o que pararia, para antes de algo entrar) e a **completação
só para a frente** (o que já entrou se completa, nunca se desfaz). A segunda já está no código
(713/722); a primeira só no bidirecional (722, `ensaiar_o_grupo_bidi`,
`servico_bidirecional_01.rs:1051`/`1126`).

---

## 3. Os restos — lidos no código; o RED de cada um é a fatia F0

Todos **lidos, não medidos** (a árvore é só leitura nesta rodada e a prova pede um arquivo de
teste novo). A cadeia vai escrita para a F0 confirmar ou matar.

### R1 — a tabela rompida sai do grupo, e as irmãs da mesma transação entram (defeito ativo)

`aplicar_grupo_da_replica` (`servico_replicacao_01.rs:1574`) confere a continuidade no primeiro
grupo depois de (re)ligar; a tabela rompida vai para `rompidas` (`:1605`), sai da marca e do grupo,
o `Juntador` a larga (`replica.rs:940`) — e **as outras tabelas do mesmo grupo seguem**: o comentário
diz «a tabela rompida sai do grupo e da rodada — ela já parou de ser seguida, e as outras continuam,
como sempre foi». Cadeia: venda `T` toca `vendas`, `itens`, `pagamentos`; `pagamentos` da réplica
recebeu escrita local (sem `somente_leitura`, `espelho` desligado) → `T` entra em `vendas` e `itens`
e **não** em `pagamentos`, para sempre, legível e indistinguível. É o 299 inteiro, pela porta da
continuidade. **Na conta** (garantia que não vale).

### R2 — a divergência determinística no meio do grupo da réplica FIEL (defeito ativo, alcance menor)

O 713 completa para a frente o erro que só a rodada vê. O erro **determinístico** — rowid que não
bate, carimbo que não confere, exclusão que não acha a linha (`table.rs:8307` em diante) — bate de
novo na completação; a marca fica no disco; o arranque a trata com `NoArranque::Sim`
(`marca.rs:1849`): conta como impossível **e apaga** — a metade fica para sempre, dita uma vez no
relatório do arranque e em lugar nenhum que um cliente leia. O bidirecional já não tem isso: o 722
**ensaia o grupo inteiro antes do primeiro evento**. A réplica fiel não ensaia. Alcance: só uma
réplica que já divergiu (o 498 H3, slot sem evento, ⏸ pelo dono; escrita fora do servidor). **Na
conta**: a decisão do dono no 685 é «inteira ou não chega, sem exceção».

### R3 — o PITR corta evento a evento e tabela a tabela (defeito ativo)

`reaplicar_diario_ate` (`servico_backup_01.rs:1335`) é o segundo dono do `aplicar_evento` e **não
passa pelo `Juntador`**: corta por `e.carimbo > ate_ms` evento a evento (`:1422`), e o carimbo é o
relógio de cada evento (`log.rs:1363`, `agora_ms` por registro) — um commit de 600 eventos que
atravessa um milissegundo, com `ate` dentro dele, restaura meia venda. E a continuidade que falha
numa tabela vai para `parou_em` **por tabela**, enquanto as outras avançam. O ⏸ 717 (F10) nomeou só
o «fora de unidade»; o corte é outro furo, do mesmo motor. **Na conta**: restaurar meia venda é
dado errado.

### R4 — commit misto e origem anterior ao 676 (contados; fecham por virada, não por recusa)

`commits_mistos` (`servico_replicacao_02.rs:728–745`): uma tomada que grava em volume v2/v3 e em v4
chega partida. Fecha **forçando a virada do volume velho no arranque da origem**, antes da primeira
escrita — sem mudar formato (a virada já existe), custo de um volume novo por tabela velha, uma
vez. A origem anterior ao 676 (sem `tx`) continua «evento a evento, dito uma vez»: não há dado em
produção, origem e réplica sobem juntas, e recusar pararia uma réplica sem ganho de garantia.
**Fora da conta** depois da virada; o contador fica.

### R5 — assinatura parcial (limite declarado, não defeito)

Tabela fora do alcance do usuário da replicação, ou lote barrado pela cifra (342), entrega o
subconjunto da transação. Os três maduros fazem o mesmo (publicação por tabela, `replicate-do-table`)
→ aceite automático **como limite**, escrito no `REPLICACAO.md` §8.2 e contado (a réplica diz quantas
transações chegaram parciais por escopo).

### R6 — a réplica não diz em que transação está (lacuna de observabilidade)

`replicacao_estado` não expõe a última transação da origem aplicada inteira. Os três expõem
(`pg_replication_origin_status.remote_lsn`, `gtid_slave_pos`, `gtid_executed`) → aceite automático.
O meio aqui: em memória, por database, «desde o arranque» — **sem campo novo no disco**, porque a
posição de cada tabela já é o diário dela e, com R1/R2 fechados, toda posição depois da recuperação
está numa fronteira de transação.

### Documentação que mente hoje (H — vai junto da F5)

- `REPLICACAO.md` §8.2, «O que NÃO mudou»: diz que a queda do processo deixa o grupo pela metade —
  o 682 fechou isso.
- `REPLICACAO.md` §4: «quando as transações entrarem … o campo reservado já está guardado» — os
  bytes reservados foram gastos pela `origem`, e o `tx` entrou crescendo o cabeçalho.
- `CLAUDE.md` (marca) e `ACID.md` §2.4: «a atomicidade entre tabelas não atravessa o fio da réplica
  (pedido 299)» — vale hoje só para R1–R3.
- A própria linha do 299 no `PENDENCIAS.md`.

---

## 4. O custo, contra o nosso gargalo

O gargalo do insert é o `.ndx` (83,5%, `DESEMPENHO.md`). O que este desenho cobra:

| Item | Bytes por evento | `fsync` a mais | Leitura a mais | Formato |
|---|---|---|---|---|
| H1/H3/H4 (já pagos: 676/682) | +8 B (44 → 52; com imagem 223 → 231, +3,6%) | 1 por grupo (a marca), **fora** da trava | — | `.log` v4, já entrou |
| R1 parada por transação | 0 | 0 | 0 (decisão no `Juntador`, em memória) | não |
| R2 ensaio da réplica fiel | 0 | 0 | inclusão: **0** (rowid = `slot_count + 1`, conta); alteração/exclusão: **1 leitura de slot** do `.reg`; unicidade: **só** com `escritas_locais > 0` ou sem `somente_leitura` (portão antes do trabalho) | não |
| R3 PITR pelo `Juntador` | 0 | 0 | a memória da transação inteira na mão (≤ 64 MiB, o mesmo teto) | não |
| R4 virada forçada | 0 | 1 por tabela velha, uma vez | — | não (a virada existe) |
| R6 última transação | 0 | 0 | 0 (u64 em memória) | não |

**Nenhum byte novo por evento e nenhum `fsync` novo por evento.** A réplica antiga não muda nada:
nenhum formato muda, e o 676 já documentou réplica velha × origem nova.

**O número que falta.** A leitura de slot do ensaio (R2) tem um análogo medido no 405 — a mesma
leitura «feita de fora» custou **+14,7%** no laço da réplica (21,49 → 24,65 µs/evento, `debug`,
`REPLICACAO.md` §5.1) —, mas é **número citado, não medido agora**, e é de alteração; a venda é
inclusão (custo zero no ensaio). **Raciocinado, não medido.** Decide na bancada: um `--example
custo-do-ensaio -p phxsql-server` em `release`, A/B intercalado de cinco pares com o ensaio ligado e
desligado, 50.000 eventos em mistura 90% inclusão / 10% alteração, faixa min–max; e a
`bancada/replicacao/medir.py` (cujo `resultados.json` é de **17/09/2026, 0.18.0**, de antes do
676/682 — o custo do que já foi pago **também não está medido**: vazão 43.606 eventos/s é número
velho).

**O custo que não é byte — e o DBA o põe na mesa.** R1 troca disponibilidade por fidelidade: com a
parada por transação, uma tabela rompida **segura o database inteiro** a partir da primeira
transação que a toca (hoje, as irmãs seguem). É o que os três maduros fazem (o fluxo para, não a
tabela) e é o que o dono pediu em 09/10. Transação que **não** toca a rompida e vem **antes** dela
entra; a que vem depois espera — porque pode depender causalmente da parada.

---

## 5. O que o DBA (papel C) diz não

1. **Nenhum *undo* na réplica.** Desfazer a metade devolve slot; o `.reg` não reaproveita slot. Só
   ensaio antes e completação para a frente. A Sombra continua parada.
2. **Nenhum slot consumido pelo ensaio.** O ensaio só **lê**: prevê o rowid pela conta, lê o slot
   da alteração. Ensaio que reserva slot «para garantir» é o reuso pela porta dos fundos.
3. **A réplica nunca reexecuta a cascata.** A cascata é só do `ao_alterar`, executada **na origem**;
   os elos chegam como eventos dentro do mesmo `tx` e se aplicam como eventos. Reexecutar aplicaria
   duas vezes. O ensaio confere os elos como confere qualquer evento.
4. **A mãe antes da filha dentro do grupo** (`ordem_das_maes`) continua; o ensaio segue a mesma vez.
5. **Nada de pular sozinho.** Pular uma transação parada é ato do operador, nomeado pelo `tx`
   (o `SKIP (lsn)` do PostgreSQL) — e aqui pular **diverge a numeração**, então nem isso entra nesta
   rodada: a saída é reconstruir a tabela (semeadura), como hoje.
6. **Nenhuma posição global, nenhum GTID inventado.** A posição continua por tabela; o `tx` e o
   `posicao` já dão a fronteira (`REPLICACAO.md` §4 continua certo no princípio).
7. **Nenhuma mudança de formato** neste resto. Se alguma fatia pedir uma, ela volta ao papel C antes
   do código.

---

## 6. As fatias, com aceite e prova

Prova real nos dois sentidos em toda fatia: **falha com o defeito reposto, passa com o conserto**,
pelo soquete, contra processo `phxsqld` de verdade. Aceite comum: a tupla `(vendas, itens,
pagamentos)` do central **nunca** é diferente de `(0,0,0)` ou `(1,N,1)`.

| Fatia | Papel | O que entra | Aceite / RED esperado no HEAD de hoje |
|---|---|---|---|
| **F0** | F | Três provas RED, sem conserto: (a) R1 — `pagamentos` da réplica com escrita local, venda na origem, rodada; (b) R2 — divergência que bate **também** na completação (gancho no `aplicar_evento_da_marca`, só `debug`), `SIGKILL`, arranque; (c) R3 — venda de 600 itens com carimbos em ≥ 2 ms (gancho de relógio), PITR com `ate` no meio | RED esperado: (a) `(1,N,0)`; (b) `(1,k,0)` depois do arranque, marca apagada; (c) `(1,k,0)` no restaurado. **Se alguma der verde no HEAD, o resto morre como hipótese** e sai deste documento com o número |
| **F1** | B | R2: **um** ensaio para a réplica fiel e o bidi — o `ensaiar_o_grupo_bidi` vira o ensaio do motor, com a identidade (rowid × chave) como parâmetro (desenho único §3.4). Sob a trava, antes do 1.º evento; recusa → nada entra, a marca sai | (b) verde; com o ensaio tirado, vermelho. O bidi continua verde (`venda-inteira-na-queda-do-bidi`, 12) |
| **F2** | B | R1: o `Juntador` aprende «parar na transação»: a fila rompida segura toda transação a partir da primeira que a toca; as anteriores que não a tocam entram. `replicacao_estado.paradas` diz o `tx`, as tabelas da transação, e o motivo; o alarme `ContinuidadeRompida` (769) já existe e é o mesmo | (a) dá `(0,0,0)` e a parada nomeia o `tx`; com o `largar` por tabela reposto, `(1,N,0)` |
| **F3** | B | R3: o PITR passa pelo `Juntador` (o mesmo motor da réplica — lei «do mesmo motor»): transação entra se o **maior** carimbo dela ≤ `ate` (o *commit time* do PG); continuidade rompida numa tabela vira parada global antes da primeira transação que a toca, dita na resposta. Junto, o «fora de unidade» do ⏸ 717 (F10), porque é a mesma função | (c) dá `(0,0,0)` ou `(1,600,1)`; com o corte por evento reposto, `(1,k,0)` |
| **F4** | B | R4: virada forçada do volume v2/v3 no arranque da origem; R6: `ultima_tx_da_origem` por database no `replicacao_estado`; R5: contador de transações parciais por escopo | `commits_mistos` fica 0 numa origem aberta sobre volume v3 e escrita em seguida; o RED é o mesmo cenário sem a virada (contador 1) |
| **F5** | H | Docs que mentem (§3, fim); a linha do 299 reescrita; a frase do `CLAUDE.md` passa a dizer o que vale depois de F1–F3 | conferência pelo integrador; nenhum número digitado |

Ordem: F0 → F1 → F2 → F3 → F4 → F5. F1 e F2 tocam o mesmo `aplicar_grupo_da_replica` — **uma
frente só**, senão o encontro delas é onde o defeito aparece (lei do orquestrador).

**Fecho do 299:** F0–F3 verdes pelo `./portoes.sh`, guardas novas no catálogo (uma por RED), e a
pergunta do enunciado respondida pela prova: *réplica derrubada no meio do grupo → no arranque, ou o
grupo inteiro está, ou nenhum; e a réplica sabe e diz* — o «sabe» pela marca e pelo ensaio, o «diz»
pela parada com o `tx` e pela `ultima_tx_da_origem`.

---

## 7. O que sobe ao dono

**Nada**, e pelos três nomes:

- **Choque com pétrea:** um, e com caminho dentro dela. O meio dos quatro maduros (rollback da
  metade) bate na ordem de digitação; o comportamento entra pelo ensaio e pela completação para a
  frente, como no 713/722. Pesquisa não revoga pétrea, e aqui nem precisou.
- **Empate real:** nenhum. Toda linha da §2 dá 9 × 0, 10 × 0 ou 4 × 0.
- **Produto:** a troca de R1 (uma tabela rompida segura o database inteiro) é disponibilidade contra
  fidelidade, e o dono já escolheu fidelidade em 09/10/2026 com estas palavras; os três maduros fazem
  igual. **Não se pergunta de novo.**

O que fica dito para não voltar: H2 (diário de transação separado) e H6 (*undo* na réplica) estão
**recusadas** — a primeira duplica a pergunta que o `tx` responde, a segunda devolve slot.

---

## 8. F0–F2, medido (10/10/2026, papel B, base `edbd6180`)

| | RED no HEAD | Prova | Depois do conserto |
|---|---|---|---|
| R1 | **vivo**: `(1, 4, 0)` | `venda-inteira-na-ruptura-da-replica::a_tabela_rompida_segura_a_transacao_inteira_e_as_seguintes` (duas escritas locais) | `(0, 0, 0)`; a venda anterior que não toca entra; `transacoes_paradas` nomeia o `tx` |
| R2 | **vivo**, com forma diferente da prevista: `((0, 4, 0), (0, 4, 0))` vivo e depois do arranque — a ordem do grupo é a do nome, `itens` antes | `…::a_divergencia_no_meio_do_grupo_nao_deixa_metade_no_arranque` (alteração de linha que saiu por fora do diário) | `(0, 0, 0)` nos dois; o erro diz «ensaio» |
| R2' (achado) | **vivo**: `(1, 4, 1)` com o pagamento no rowid 3 — o `aplicar_evento` conferia o rowid **depois** de gravar | `…::a_inclusao_que_diverge_nao_grava_no_rowid_errado` | `(0, 0, 0)` |
| R3 | **vivo**: `(1, 15, 0)` no restaurado | `servidor::testes_pitr::o_pitr_nao_restaura_meia_venda` (`#[ignore]`, aceite da F3) | — (F3, fora desta frente; **não** é o mesmo motor: o PITR não passa pelo `Juntador`) |

**Custo do ensaio** (`cargo run --release --example custo-do-ensaio -p phxsql-store`,
50.000 eventos, 90/10, grupos de 500, cinco pares intercalados): sem a unicidade
**+1,1%**, faixas se cruzam (5,83 [5,72–6,17] × 5,76 [5,73–5,87] µs/evento); com a
unicidade **+57,8%**, faixas separadas (9,40 × 5,96). Daí a divergência do §4: a
unicidade só entra na tabela com escrita local contada, e não «sem
`somente_leitura`» (o padrão de fábrica — toda réplica comum pagaria 58% sem
comprar nada, porque a escrita local pelo servidor já rompe a continuidade antes
de qualquer grupo).

**Três desvios do desenho, com motivo:** (1) a parada vai em
`replicacao_estado.origens.<o>.transacoes_paradas.<database>` e não em `paradas`,
que é o mapa por tabela que o `replicacao_pular` consome (§5.5: pular não entra);
(2) o ensaio da fiel e o do bidi continuam dois — perguntas diferentes (rowid ×
chave com «mais recente vence»), e a decisão comum (a identidade, o rowid
previsto) é UMA função do `Table` que o aplicador também chama; (3) a barreira
sai do fim do prefixo comum dos diários, e não do evento conferido — ver
`docs/cognicao/cognicao_a-conferencia-de-continuidade-ve-o-ultimo-evento-nao-o-primeiro-que-falta_20261010_0500.md`.

**Consequência que muda comportamento velho, de propósito:** a tabela de outra
história (601) agora segura o database a partir da primeira transação da origem
que a toca (`orfas-na-replica::a_mae_de_outra_historia_segura_as_filhas`). A
prova da órfã contada mudou de fábrica: a mãe que saiu da réplica por fora do
diário.
