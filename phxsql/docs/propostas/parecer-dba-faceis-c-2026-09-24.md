# Parecer do DBA (papel C) — lote «fáceis C», itens 458 e 499

*24/09/2026. Só leitura, sem commit. Worktree `agent-a466115b9d5f16228`,
base **8e5a545** (a correção do integrador: `git diff HEAD`, e não
`diff a494f33`). O `transacao.rs`/`servidor.rs` do HEAD vivo (`bb3b158`, com
o lote de integridade `a494f33`) foram lidos junto, porque o 458 entra por
cima dele. Nada compilado: as medidas usaram o `target/debug/phxsqld` que a
frente já tinha (19:03, depois do último `mtime` do `servidor.rs`, 18:56) e o
`target/release/phxsqld` do repositório principal (08:02, `esvaziar_lixeira`
ainda no `OPS_ESCRITA`) como comportamento velho.*

| item | veredito |
|---|---|
| **458** — `transacoes`/`travas` em `TravaDaGuarda` com saneamento | **COM CONDIÇÃO** (C1–C3) |
| **499** — `esvaziar_lixeira` fora do `OPS_ESCRITA` | **BLOQUEIA na forma atual**: sai do portão da réplica **e** do portão da transação. Medido |

---

## 458 — a trava suja das transações

### (1) O saneamento pode pôr ABORT_ONLY numa transação que já passou da marca?

**Não.** `Transacoes::abortar_abertas` (`transacao.rs:679`) mexe só em
`Estado::Ativa`. No `op_commit`, a troca para `Confirmando` e o
`std::mem::take` das escritas acontecem **no mesmo trecho com `transacoes` na
mão**, antes de `marca_em_voo` nascer. Então toda transação que pode ter marca
no disco já está em `Confirmando`, e a lista dela já está fora do registro: o
saneamento não alcança nem o estado nem a lista.

**A relação com o 451 é de partição, e ela fecha:**

- **dados/451** decide o COMMIT que chegou à marca: completa a marca em voo,
  ou não há marca e não há dado;
- **transacoes/saneamento** decide as que não chegaram: ATIVA vira ABORT_ONLY;
- a transação da própria conexão que entrou em pânico sai pelo `AoSair`
  durante o desenrolar (`descartar_transacao`). A `Tomada` dela nasce com
  `ja_em_panico` e não suja de novo.

O `devolver_a_lista` (antes da marca, ou marca removida com zero aplicado)
devolve ATIVA uma lista que estava na thread do COMMIT, inteira, e fora do
alcance do pânico alheio. Correto.

**O irmão, porém, faz exatamente o que a pergunta teme.** Isso é anterior à
frente e está no HEAD vivo. Lido, **não medido** (ver P5):
`Transacoes::vencidas` não filtra por estado, e o `abortar_soltando` do prazo
põe ABORT_ONLY e **solta as travas** de uma transação em `Confirmando`. Se esse
COMMIT recusar antes da marca (`EmMigracao`, ou o `EmTransacao` do 516 sem
ceder, que é justamente o que se repete), `devolver_a_lista`
(`servidor.rs:17303` vivo) a devolve **ATIVA e sem travas**. O COMMIT seguinte
passa pelo portão de controle (`OPS_DE_TRANSACAO`, antes da conferência de
prazo), e o `op_commit` não confere `expira_ms`. Resultado: confirma sem
travas e fora do prazo, e abre uma janela de *lost update*.

### (2) A ordem das travas, e o pânico que envenenava as duas

**Mudou, e para melhor.** O §24.5 dizia: pânico com `transacoes` na mão dentro
do COMMIT envenena as duas, e só a de dados se cura. Agora as duas se curam.
Esse pânico só acontece **antes** da marca, porque o bloco de `transacoes` do
`op_commit` fecha antes de `marca_em_voo`. Então o 451 acha `marca_em_voo`
vazio e não completa nada, e o saneamento aborta as ATIVAS. Nenhum estado fica
sem dono.

A frente **não** mudou a ordem. O saneamento roda dentro do `travar()` e não
toma trava nenhuma. A ordem parcial que existe, lida:

- **dados ≺ transacoes**, no `op_commit`;
- **travas ≺ transacoes**, aninhada no `barrado_por_travas`: `travas.travar()`
  e, dentro dele, `transacoes.travar()`, na linha 16240 do worktree.

Por isso **o saneamento nunca pode tomar `travas`**, e isso pesa na C1.

### (3) Algo com disco atrás foi posto na recuperação calada?

**Não.** Só `transacoes` e `travas` viraram `TravaDaGuarda`, e as duas são só
RAM (`Travas.tabelas` é fonte única, e `soltar_tudo` varre por id). As **54**
tomadas em **16** travas do `TomarTrava::tomar` (contei de novo: 54/16)
continuam **recusando**. As trocadas eram `map_err(|_| trava_envenenada())`
(45 delas) ou `Err(_) => return Err(...)`. Nenhuma passou de recusar para
calar. O único `into_inner` novo está no `TravaDaGuarda::travar`.

### O preço declarado estava com a razão errada — medido

A frente escreve que a transação saneada segura as travas até o ROLLBACK
«como todo bloco abortado (**o PostgreSQL também**)»
(`servidor.rs:59807-59808`, `SEGURANCA.md:5976`). **Medido no PostgreSQL
16.13 local**, com duas sessões:

1. A faz `BEGIN; UPDATE x SET v=2 WHERE id=1; LOCK TABLE x IN SHARE MODE;
   SELECT 1/0;`;
2. A fica em `idle in transaction (aborted)`, com **0** travas `relation`,
   `transactionid` ou `tuple` no `pg_locks`;
3. B roda `UPDATE x ... WHERE id=1` com `lock_timeout=1s` e **passa na
   hora**.

O PG solta as travas no *abort* (`AbortTransaction`), e não no ROLLBACK.
Segurar até o ROLLBACK é **escolha nossa**: é a mesma do ABORT_ONLY por erro
da casa (`transacao.rs:693` vivo). Ela é defensável, mas precisa estar escrita
como nossa.

E o HEAD vivo abriu uma porta única para o **gestor** encerrar uma transação,
`abortar_soltando` («segurar tabela alheia depois de a transação não poder mais
confirmar é exatamente o que cada uma existe para impedir»). O saneamento é o
terceiro motivo de encerramento pelo gestor e não passa por ela. A lei de
«função e comando não se duplicam» pede a pergunta: é a mesma decisão? Em
parte é. Só que ela **não se unifica ingenuamente**, porque soltar travas de
dentro do saneamento inverteria **travas ≺ transacoes**.

### Condições

- **C1.** Tirar «o PostgreSQL também» dos dois lugares, e escrever a
  divergência como nossa: a transação saneada segura as travas até o ROLLBACK,
  a queda ou o prazo (padrão `transacao_prazo_min` = 5). O PG solta no
  *abort*, medido. E escrever por que o saneamento não usa o
  `abortar_soltando`: a ordem das travas.
- **C2.** O §24.5 do `SEGURANCA.md` (linha 5345) ainda diz «o `Mutex` de
  `transacoes` (pedido 458) não muda … só a de dados se cura». Hoje ele
  contradiz o §29 do mesmo arquivo.
- **C3.** Prova do **segundo pânico**. É a razão de a `Tomada` existir, e
  nenhum teste reprova a volta do `veneno_dito` (um aviso por trava) no
  caminho da transação. O `trava_envenenada_nao_passa_calada` mede um pânico
  só. Roteiro:
  1. a 7 abre;
  2. pânico;
  3. a 7 faz ROLLBACK;
  4. a 8 abre e empilha;
  5. **pânico de novo**;
  6. o COMMIT da 8 tem de recusar com `TRANSACAO_ABORTADA`.

  Com a marca «já dito», a 8 confirma.

### Para a integração por cima do HEAD vivo

- `estourar_prazo` virou `abortar_soltando` no HEAD vivo, e o conflito é
  certo. **Mantenha o do HEAD vivo** e só troque as tomadas por `travar()`.
  O corpo em linha da frente perderia o `commit_barrado_por = None` e a porta
  do ciclo do 516.
- Nenhuma tomada nova do HEAD vivo passa calada. Contei entre 8e5a545 e
  bb3b158: +2 `transacoes.lock()`, +3 `travas.lock()` e +2
  `trava_envenenada()` sem argumento. A `TravaDaGuarda` não tem `.lock()`, e
  `trava_envenenada` passou a exigir o nome, então **o compilador obriga** a
  converter cada uma.
- Depois do saneamento, `commit_barrado_por` fica sujo na transação saneada.
  É inofensivo: a corrente do `commit_barrado` para em transação fora de
  `Ativa` (`transacao.rs:712` vivo).

---

## 499 — `esvaziar_lixeira` fora do `OPS_ESCRITA`

### O que a frente perguntou está certo

- **Mexe só no nó local, no dado replicado?** Sim. `Table::esvaziar_lixeira`
  grava o `.reason` e trunca o `.trash`, e não escreve no `.log`. O teste da
  frente mede o diário em 4 eventos antes e depois, nos dois nós.
- **`administrar` e `motivo` em todo caminho?** Sim.
  - O `motivo` é conferido na op **e** no motor (`conferir_motivo`), então
    vale para qualquer porta.
  - `administrar` vem de `Atividade::da_operacao`, e tanto o `despachar`
    (soquete e REST, que é `POST /api`) quanto o job (`portoes_do_pedido` sob
    o usuário do job, `servidor.rs:8350`) passam por esse portão.
  - O MCP não oferece a op (`ferramenta_mcp` falso, e há teste).

### O que ficou de fora: o `OPS_ESCRITA` responde SEIS perguntas, e a frente mudou as seis para acertar duas

| consumidor | pergunta | efeito de sair da lista |
|---|---|---|
| `servidor.rs:10982` / `:11032` | réplica redireciona; somente-leitura e cluster sem maioria recusam | **o pretendido** |
| `servidor.rs:14690` (`dentro_da_transacao`) | escrita que a transação não empilha **não entra** em transação | **passa: roda na hora e o ROLLBACK não a desfaz** |
| `servidor.rs:11175` (`barrado_por_travas`) | escrita comum esbarra em trava de transação (braço «estrutura, índice, **lixeira**» → Exclusiva) | **passa por cima da IX de outra transação** |
| `servidor.rs:9553/9968/10395` | telemetria conta escrita ou leitura | vira **leitura** |
| `catalogo.rs:103` → `rest.rs:354` | OpenAPI `x-phxsql-escreve` e catálogo `escreve` | diz **`false`** para quem apaga sem volta |
| `profiler.rs:193` (`ESCRITAS`, lista própria) | Profiler «muda dado» | continua **true**: agora as duas listas discordam |

**Medido pelo soquete** (`sonda499.py`, no scratchpad da sessão), no mesmo
roteiro contra os dois binários:

```text
                                       FRENTE (19:03)              VELHO (08:02)
BEGIN; esvaziar_lixeira                ok, apagadas 1              SP000018 «nao entra em transacao»
ROLLBACK -> lixeira                    0  (nao volta)              1
B BEGIN + inserir (IX); A esvaziar     ok, apagadas 1              SP000006 «tabela em transacao»
controle: reindexar nos dois casos     recusado                    recusado
```

**Vale também para o `expurgar_trilha`, e desde o 368.** Medi a mesma sonda,
e `BEGIN; expurgar_trilha` responde `ok`. A condição C2 que **este papel** deu
ao 368 nomeou só o portão da réplica e não viu as outras cinco perguntas da
lista. O erro de alcance é meu, e o 499 o repetiu por analogia.

A regra que está sendo quebrada está escrita logo acima da linha 14690:
«Escrita que a transação não sabe empilhar NÃO passa direto ao disco». O
PostgreSQL recusa o que não é transacional dentro do bloco («cannot run inside
a transaction block»). Aqui não se recusa **nem** se confirma: roda fora da
transação, calada, que é pior que as duas coisas.

### O `.trash` da réplica esvaziado muda o que se consegue devolver, e isso não está escrito

O `LGPD.md` diz que «o administrador de cada nó esvazia o `.trash` daquele
nó». Faltam quatro consequências:

- **(a)** Um expurgo por LGPD tem de ser mandado **a cada nó**, porque o do
  source não chega à réplica.
- **(b)** Depois de uma **promoção**, a lixeira do novo primário é a da
  réplica. A linha que o antigo source expurgou **reaparece** na `lixeira` do
  promovido, se ninguém esvaziou a réplica.
- **(c)** O backup de cada nó leva o `.trash` **dele**, então restaurar o
  backup da réplica devolve o que o source já tinha esvaziado.
- **(d)** Esvaziar a réplica apaga a **última cópia** de uma linha que o
  source já esvaziou. Essa é a intenção do LGPD, e também é o fim da
  recuperação por ali.

É a ressalva que o pedido **297** já cobra («mesmo `.trash`/`.reason`: NÃO,
por desenho — sem ressalva escrita»). O 499 tornou a divergência ação do
operador, e ela deixou de ser só efeito da replicação.

### Para liberar

- **B1.** Separar a pergunta. Uma lista nova (por exemplo `OPS_DO_NO`) tira
  do `OPS_ESCRITA` só a pergunta «grava o dado replicado?», ou seja, o portão
  2b e o redirecionamento da réplica. `esvaziar_lixeira` e `expurgar_trilha`
  **continuam** escrita para a transação, o `barrado_por_travas`, a telemetria
  e o `escreve` do catálogo e do OpenAPI. Duas listas, porque são duas
  perguntas: a lei dos iguais vale para quem responde a **mesma** pergunta.
- **B2.** A sonda vira teste pelo soquete, e **tem de cair** com o defeito de
  hoje: `BEGIN` + `esvaziar_lixeira` recusa, e a lixeira segue com 1 depois do
  ROLLBACK. O irmão é o mesmo roteiro com o `expurgar_trilha`. O
  `a_replica_esvazia_a_propria_lixeira` tem de continuar verde.
- **B3.** Escrever (a)–(d) no `LGPD.md`, apontando para o 297.

---

## Pedidos propostos

| estado | pedido | origem |
|---|---|---|
| ☐ | **P1 — O `OPS_ESCRITA` responde seis perguntas, e tirar uma op dele muda as seis: `esvaziar_lixeira` (499) e `expurgar_trilha` (368) rodam dentro de BEGIN sem voltar no ROLLBACK e passam por cima da trava de outra transação.** Medido (sonda acima). Conserto: B1 + B2. Bloqueia o 499 | papel C, este parecer |
| ☐ | **P5 — O prazo varre transação em `COMMITTING`: solta as travas de um COMMIT em curso, e o `devolver_a_lista` a devolve ATIVA sem travas.** `vencidas()` sem filtro de estado, `abortar_soltando` sem conferir estado, `devolver_a_lista` põe `Ativa` sem condição, e o COMMIT não confere `expira_ms`. Janela de *lost update*. **Lido, não medido**: medir antes de consertar (na dúvida, fica na conta). É anterior ao 458 e está no HEAD vivo | papel C, este parecer |
| ⏸ | **P7 — O saneamento poderia limpar escritas, pontos, espera e `commit_barrado_por` como o `abortar_soltando`** (só RAM, sem tomar `travas`). Não é defeito: ABORT_ONLY só aceita ROLLBACK, e o `rollback_para` recusa | papel C, este parecer |

C1–C3 (458) e B3 (499) são condições dos próprios itens, e não pedidos novos.

**Conferência: 2 de 2 itens com veredito, 100%.** O que falta é da frente:
C1–C3 e B1–B3.

---

## Re-checagem — 24/09/2026

*Só leitura, sem compilar, no mesmo worktree (`git diff HEAD`, HEAD =
8e5a545). Os vermelhos que a frente declara (os testes novos caindo com o
defeito reposto) são palavra dela: não recompilei para repô-los.*

| item | veredito |
|---|---|
| **458** | **LIBERA** — C1, C2 e C3 fechadas |
| **499** | **LIBERA** — B1, B2 e B3 fechadas |

### 458

- **C1 fechada.** Não sobrou nenhum «o PostgreSQL também», nem no teste nem
  no `SEGURANCA.md`.
  - O §29 escreve o preço como escolha **nossa**: a transação saneada segura
    as travas até o ROLLBACK, a queda da conexão ou o prazo (5 por padrão),
    com a medição do PG 16.13 (0 travas, a outra sessão atualiza na hora).
  - O §29 também diz por que o saneamento não passa pelo `abortar_soltando`:
    tomar `travas` com `transacoes` na mão inverteria o
    `barrado_por_travas`.
  - O comentário do teste (`servidor.rs:59838`) diz o mesmo.
- **C2 fechada.** O §24.5 agora diz que as duas travas se curam, e descreve
  a divisão marca/ATIVA sem sobra.
- **C3 fechada.** Há teste do segundo pânico,
  `o_segundo_panico_com_as_transacoes_na_mao_tambem_saneia`:
  1. a 7 abre;
  2. pânico;
  3. o COMMIT da 7 recusa, e a 7 faz ROLLBACK;
  4. a 8 abre e empilha;
  5. segundo pânico;
  6. o COMMIT da 8 recusa com `TRANSACAO_ABORTADA`, e a tabela fica vazia.

  Ele mede o dado, e não só o veredito. A guarda
  `veneno-dito-uma-vez-por-trava` entrou no catálogo.

Fica o que já estava dito: o P5 (o prazo varre transação em `COMMITTING`)
é anterior e segue proposto. As notas de integração do 458 continuam valendo:
fique com o `abortar_soltando` do HEAD vivo, e o compilador obriga a converter
as tomadas novas.

### 499 — a pergunta do integrador

**`grava_dado_replicado` é o ÚNICO leitor do `OPS_DO_NO`?** Sim. Fora dos
testes, o `OPS_DO_NO` aparece só em `servidor.rs:307`, dentro do
`grava_dado_replicado`. A função tem **dois** chamadores:

- `:11009` — `Papel::ReadReplica` redireciona;
- `:11063` — o portão 2b, que cobre o cluster (`recusa_de_escrita`) e, sem
  cluster, o `somente_leitura()` vivo.

Todas as outras decisões continuam lendo o `OPS_ESCRITA`, e é o certo para
cada uma:

- a transação: `:14721` («não entra em transação») e `:14672` (o gatilho
  AFTER no COMMIT);
- a trava de outra transação (`:11206`);
- a telemetria (`:9580`, `:9995` e `:10422`);
- o catálogo (`catalogo.rs:103`), que alimenta o `x-phxsql-escreve`
  (`rest.rs:354`) e o filtro da ponte MCP (`mcp.rs:133`).

O Profiler (`ESCRITAS`) já listava as duas, e agora as três listas
concordam: «escreve» é sim nas três, e «grava o dado replicado?» é não só
para as duas do nó.

**Algum caminho ainda decide «réplica pode?» pela lista velha?** Não.

| caminho | por onde passa | resultado para as ops do nó |
|---|---|---|
| soquete / `POST /api` | `despachar` → `portoes_do_pedido` | réplica e somente-leitura atendem |
| job | `executar_job` → `portoes_do_pedido` (`:8350`, sob o usuário do job) | idem |
| MCP | não oferece (`ferramenta_mcp` falso; `escreve()` verdadeiro, então a ponte somente-leitura também as esconde) | — |
| cluster, nó não-master | portão 2b → `grava_dado_replicado` falso → não pergunta à eleição | atende (arquivo do nó) |
| cluster promovido / spare promovido | o `papel_atual()` e o `somente_leitura` são **vivos** e lidos no mesmo portão | nada fica preso ao papel antigo |
| SQL / ODBC / HTTP da tela | nenhum decide somente-leitura por conta própria (lido); a tela só rotula «grava / só lê» pelo `x-phxsql-escreve`, que agora diz «grava» | — |

O único outro dono de «somente leitura» é o `dblink_consultar`
(`:24545`). Ele responde outra pergunta, a da instrução remota, e não toca
nas duas.

**B2 fechada.** São dois testes pelo soquete em `tests/lixeira-da-replica.rs`,
cada um cobrindo as **duas** ops:

- `as_ops_do_no_nao_entram_em_transacao`: `BEGIN` + op, recusa «não entra em
  transação», e a lixeira segue em 1 depois do ROLLBACK;
- `as_ops_do_no_esbarram_na_trava_de_outra_transacao`: recusa com
  `EM_TRANSACAO` enquanto a B segura a tabela. Depois do ROLLBACK da B, o
  esvaziar passa, o que prova que a recusa foi pela trava.

O `a_replica_esvazia_a_propria_lixeira` continua no arquivo, e o
`tudo_que_grava_esta_na_lista_de_escrita` voltou a exigir as duas no
`OPS_ESCRITA`.

**B3 fechada.** O `LGPD.md` traz as duas listas e o porquê, as
consequências (a)–(d) do `.trash` por nó, e aponta para o 297.

**Resíduo, que não bloqueia:** o **spare** não atende as duas
(`OPS_NO_SPARE` é lista de permissão, e elas não estão nela). O `.trash`
dele só se esvazia depois de promovido, e aí vale o (b). Isso já era assim
antes da frente.

**Re-checagem: 2 de 2 itens liberados, 100%.** Das propostas, seguem abertas
o P5 (☐, medir antes de consertar) e o P7 (⏸).
