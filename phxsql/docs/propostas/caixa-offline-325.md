# Pedido 325 — 20 caixas e 1 servidor: parecer do papel J

**Data:** 07/10/2026 · **Papel:** J (pesquisador), só leitura · **Commit lido:** `5c1d2d52`
**Base da casa:** `parecer-dba-vinte-caixas-um-servidor-2026-09-17.md` e `-2026-09-23.md` (papel C),
`docs/REPLICACAO.md` §8/§8.1/§13, `PENDENCIAS.md` 229, 289, 290, 292, 299, 329, 330, 331, 406, 615.

**Pergunta do dono:** o servidor cai, o operador não percebe, o caixa grava local e, ao voltar, sobe sozinho.

---

## 1. Hipóteses, escritas antes de medir

| | desenho | quem escreve onde | como converge |
|---|---|---|---|
| **H1 — espelho** | cada caixa é um `phxsqld` local, dono do database `caixaNN`; o central é `replica` com 20 origens, uma por caixa; preços descem pela via inversa (o caixa é réplica do database `precos` do central) | cada database tem **um** escritor | pull do central, retomada por posição; visão da loja por leitura (`unir` com `partes`) |
| **H2 — consolidado** | 21 nós em `Papel::Multi`, todos escrevendo no mesmo `vendas` | 21 escritores na mesma tabela | por chave, «mais recente vence», `Sequence` com faixa |
| **H3 — fila no cliente** | o PDV grava no central; se cair, grava num embutido local e reenvia pelo protocolo quando a rede volta | o central é o único dono, e o caixa guarda pendências | reenvio comum; o central confere FK e unicidade de novo |

## 2. O que os quatro (e o Cassandra) fazem — fonte primária

| fonte | o que resolve | custo / limite |
|---|---|---|
| **MySQL** multi-source — <https://dev.mysql.com/doc/refman/8.4/en/replication-multi-source.html> | uma réplica, N fontes, um canal por fonte | *«does not implement any conflict detection or resolution … left to the application»* |
| **MariaDB** multi-source — <https://mariadb.com/kb/en/multi-source-replication/> | o mesmo, com conexão nomeada | *«the assumption was that there are no conflicts in data between the different primaries»* (CDR só no Enterprise 12.3) |
| **PostgreSQL** — <https://www.postgresql.org/docs/current/logical-replication-conflicts.html> | um assinante com N assinaturas | conflito *«will stop the replication; it must be resolved manually»* (`ALTER SUBSCRIPTION … SKIP`) — é a nossa decisão do 292 |
| PostgreSQL — <https://www.postgresql.org/docs/current/logical-replication-architecture.html> | a aplicação roda com `session_replication_role = replica`: *«triggers and rules will not fire on a subscriber»* | é o nosso `julga_integridade` (fidelidade, não reconferência) — convergência com o desenho medido (`INTEGRIDADE.md` §3) |
| PostgreSQL — <https://www.postgresql.org/docs/current/logical-replication.html> | *«applies the data in the same order as the publisher so that transactional consistency is guaranteed»* | — |
| MySQL GTID — <https://dev.mysql.com/doc/refman/8.4/en/replication-gtids-concepts.html> · MariaDB GTID — <https://mariadb.com/docs/server/ha-and-performance/standard-replication/gtid> | um id por transação; MariaDB: *«event group … always applied as a unit»*; `domain_id` = um fluxo por escritor | é o que o nosso `.log` não tem (pedido 299) |
| MySQL `auto_increment_increment/offset` — <https://dev.mysql.com/doc/refman/8.4/en/replication-options-source.html> (MariaDB idem); PG `CREATE SEQUENCE … START/INCREMENT` | faixa por nó para a identidade | PG: *«Sequence data is not replicated»* (<https://www.postgresql.org/docs/current/logical-replication-restrictions.html>) |
| MySQL Group Replication multi-primário — <https://dev.mysql.com/doc/refman/8.4/en/group-replication-network-partitioning.html> | escrita em vários mestres | a minoria *«is unable to progress and blocks»* — **recusa escrita offline** |
| MariaDB Galera — MDEV-14616 (<https://jira.mariadb.org/browse/MDEV-14616>) | idem, síncrono | nó fora do componente primário responde *«WSREP has not yet prepared node for application use»* — **recusa escrita offline** |
| **SQLite** Session — <https://www.sqlite.org/session/c_changeset_conflict.html> | o único dos quatro com escrita desconectada e reconciliação: changeset + manipulador de conflito (OMIT/REPLACE/ABORT) | FK conferida **uma vez, no fim do changeset** — filho antes do pai resolvido no commit |
| **Cassandra** 5.0.10 (`docs/CASSANDRA.md`, `StorageProxy.java:2442-2501`, `cassandra.yaml:80`) | *hint* guardada pelo coordenador e entregue quando o nó volta | prazo de 3 h (`max_hint_window`); «mais recente vence»; não recusa chave repetida |

**Leitura da matriz:**
- **Convergência dos três maduros → aceite automático:** (a) N fontes → 1 réplica é suportado, e o desenho correto é **fontes disjuntas** (nenhum resolve conflito entre fontes); (b) conflito de unicidade **para** a aplicação, com saída manual; (c) a identidade sem coordenação é a **faixa por nó** (já implementada: 290 + 615); (d) a réplica aplica **a transação da origem como unidade**.
- **Nenhum dos três maduros faz escrita multi-mestre desconectada:** os dois que têm multi-primário (GR, Galera) **recusam** escrita na minoria. A escrita offline com reconciliação só existe no SQLite (peso 1) e no Cassandra — e as duas receitas batem em pétrea nossa (abaixo).

## 3. Qual se sustenta contra as NOSSAS pétreas

| pétrea / exigência | H1 espelho | H2 consolidado | H3 fila no cliente |
|---|---|---|---|
| ordem de digitação | **preservada 20×**, byte a byte (`.reg` do central = a ordem do caixa) | relativizada ao nó; no central vale a ordem de chegada | o central tem a ordem do reenvio, não a da venda |
| regra primordial (pai/filho) | imposta na origem; uma órfã fica **confinada** ao `caixaNN` | a união de 21 bancos íntegros não é íntegra; `excluir_de_vez_replicado` mata o pai calado | o central reconfere, mas **o cupom já saiu**: não há como recusar uma venda que aconteceu |
| filho antes do pai vindo do caixa | não ocorre: pai e filha nascem no mesmo caixa, e o `rowstamp` é do processo | ocorre entre caixas; não há formato que o impeça offline (C, NÃO 5) | o reenvio teria de **remapear** as FKs locais para os ids do central (FK mutável) |
| conflito de chave | **não existe**: um escritor por database | chave de negócio com «mais recente vence»: 4 inserções → 2 linhas (229a, **citado** 07/09); estoque: 20 baixas de 100 → **99**, não 80 (**raciocinado** do desenho, bancada §7.5 do C nunca rodou) | erro de unicidade descoberto **depois** da venda |
| auto number entre 21 nós | dispensável: o database já diz a origem; a faixa (`inicio_da_sequencia`) é opcional para numerar a loja | obrigatório, e o N fica **para sempre** | o id muda no reenvio |
| «o operador não percebe» | **por construção**: o PDV nunca fala com o central para gravar | sim | não: a primeira falha paga o prazo de conexão |
| prova N>2 | multi-source existe (`REPLICACAO.md` §8), guarda de sobreposição (406); fan-in de 20 **nunca medido** | o par é a única forma provada | — |

**Vence H1.** É a única em que as duas pétreas (ordem de digitação, regra primordial) sobrevivem sem adaptação, e é exatamente o comportamento em que os três maduros convergem (fontes disjuntas → uma réplica).

**Onde H1 diverge da origem, e por quê (inspiração, não cópia):**
1. MySQL e MariaDB juntam N fontes **na mesma tabela** e deixam o conflito para a aplicação. Nós **não juntamos**: são 20 databases. A causa é a **ordem de digitação**: a réplica aplica por rowid, e duas fontes no mesmo `.reg` colidem no segundo evento.
2. O SQLite Session confere a FK no fim do changeset. Nós não adiamos: o **invariante «só existe filho se o pai existir primeiro»** está acima do voto. O pai vem antes porque nasce no mesmo caixa.
3. O Cassandra descarta a *hint* depois de 3 h. Nós guardamos sem prazo: o diário do caixa **é** a fila, e uma venda não pode envelhecer até sumir. O preço disso está na lacuna L3.

## 4. As hipóteses que morreram

- **H2 morreu** por três motivos. Nenhum dos três maduros faz escrita multi-mestre offline; os dois que têm multi-primário recusam escrita na minoria. A única receita offline com «mais recente vence» (Cassandra) não recusa chave repetida, e isso mata a conferência de unicidade. E o estoque como coluna sobrescrita perde decrementos, também por desenho. **Não volta** sem a bancada de 20 escritores no mesmo SKU (C §7.5) desmentir o 99.
- **H3 morreu** porque o reenvio pelo protocolo reconfere FK e unicidade **depois** que o cupom foi impresso. Uma venda real recusada não tem conserto, o remapeamento de ids torna a FK mutável, e o operador sente o prazo de conexão.

## 5. Recorte mínimo, em fatias (sem código neste parecer)

| # | fatia | formato? | decide |
|---|---|---|---|
| F0 | **Bancada do espelho**: 20 `phxsqld` (`caixa01..20`) e o central com 20 origens; o central cai por N min enquanto os caixas gravam venda+itens; sobe; medir o tempo até convergir, o SHA-256 por linha igual e zero *fail-stop*; e, na mesma bancada, a descida de `precos` (o central escreve em `precos` sendo réplica de `caixaNN`) | não | se «imediata» se sustenta; se a mão dupla por database funciona |
| F1 | Guarda **por base**: um nó que é réplica de X e aceita escrita local exige proibição de escrita em X (`blacklist.rs` `proibidos_por_base`); o aviso global de hoje passa a nomear a base | não | (C 23/09 §4a) |
| F2 | **Atomicidade venda+itens no fio** (299): id de transação no `.log` | **sim** (`.log` 44→52 B, versão nova; papel C assina) | o comportamento entra por convergência (PG/MySQL/MariaDB aplicam a transação como unidade) |
| F3 | Visão da loja: `unir` com `partes` sobre os 20 databases, medido com um dia de vendas contra `recursos.max_linhas` | não | o teto da consolidação |
| F4 | O contrato escrito do que se perde na janela (§6) | não | — |

**Ordem:** F0 → F1 → F2 → F3/F4. O F0 vem antes de tudo porque **as duas premissas de H1 ainda não estão medidas**.

## 6. Lacunas (o que não se mediu)

- **L1** — o fan-in de 20 origens nunca rodou: o `montar.py` mede 1→N, não N→1. Um item aberto em `REPLICACAO.md` §13 («buscar o lote FORA da trava», 30,7 s medidos numa réplica cortada) pode virar o gargalo das 20 threads sob a trava global. **Raciocinado, não medido**; quem decide é o F0.
- **L2** — a mão dupla por database (central réplica de `caixaNN` e origem de `precos`; o caixa ao contrário): `op_replicar` não filtra por papel (`servidor.rs:30684`), então pela leitura funciona. **Não medido.**
- **L3** — o diário do caixa não tem expurgo: a janela offline é ilimitada, e o disco também. A medida que decide é bytes de `.log` por dia de venda.
- **L4** — cadastro novo (cliente) feito em dois caixas durante a janela: a unicidade global não existe offline, em nenhum dos três maduros.
- **L5** — confirmar no fonte se MySQL/MariaDB conferem FK na aplicação de *row events* (só o PG está citado acima). Não muda a decisão: o nosso `julga_integridade` já é decisão medida.

## 7. O que sobe ao dono

- **PRODUTO (uma pergunta só):** o contrato de «transparente». Com H1, durante a queda:
  1. o caixa vende com o **último preço e o último cadastro** que recebeu;
  2. o central vê a loja **atrasada** até a reconexão;
  3. um cadastro novo feito em dois caixas **pode duplicar**;
  4. até o F2, a venda pode aparecer no central **sem parte dos itens** por uma rodada.

  Aceitar essa frase como promessa ao cliente é decisão de produto.
- **Choque com pétrea:** nenhum. H1 preserva as duas.
- **Empate:** nenhum. A convergência dos três decidiu.
- **Não sobe:** a identidade (faixa por nó, convergência, já feita), a parada da aplicação no conflito (convergência, já é o 292), a transação aplicada como unidade (convergência; o meio de formato é do papel C).
