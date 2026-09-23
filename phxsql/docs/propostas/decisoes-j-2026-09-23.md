# Duas decisões que estavam na mesa do dono, decididas pelo papel J

**Data:** 23/09/2026 · **Commit lido:** `252e572` · **Papel:** J (pesquisador), só leitura.
**Pétrea que autoriza:** *«perguntar ao dono é ÚLTIMO recurso»* (23/09/2026) — o J vai ao help e
ao fonte dos quatro e decide pelas réguas que já existem.

**As duas decisões:**

| # | pergunta | decisão | régua | sobe ao dono? |
|---|---|---|---|---|
| 1 | a faixa `mod N` do `rowstamp` entra? | **NÃO — confirmo o parecer do DBA** | 0 de 4 particionam contador de ordem | **não** |
| 2 | o `.log` ganha id de transação? | **SIM — A e B juntos, não A ou B** | id: 9×1 · fronteira: 10×0 | **não** |

Nenhuma das duas sobe: não há choque com pétrea nossa, não há empate, e nenhuma é de
preço/prazo/SLA. O que sobra são **duas lacunas de desenho** (§4) e **dois documentos que
envelheceram** (§5) — nenhum deles é decisão do dono.

---

## 1. Hipóteses, escritas antes de medir

**Decisão 1**

- **H1** — faixa por servidor (`offset`/`increment`) é a receita dos maduros para identidade de
  linha em multi-origem; logo, estendê-la ao `rowstamp` é coerente e o DBA erra.
- **H2** — os maduros particionam só a **chave surrogada visível ao usuário**; o contador interno
  de ordem/versão nunca é particionado, e a identidade entre nós sai de um **par** (nó, contador);
  logo o DBA acerta.
- **H3** — o `mod N` compraria ordem entre nós de graça, que é o que falta hoje.

**Decisão 2**

- **H4** — os maduros carregam **id** de transação no fluxo de replicação → opção **A**.
- **H5** — os maduros carregam só **fronteira** → opção **B**.
- **H6** — o cabeçalho de 44 bytes tem reserva, e há um caminho mais barato que A e que B.

---

## 2. DECISÃO 1 — a faixa `mod N` do `rowstamp`: **NÃO**

### 2.1 A matriz, com a pergunta partida em duas

A pergunta central — *«faixa por servidor é a receita dos maduros, ou eles resolvem por outro
caminho?»* — só tem resposta depois de separar **dois objetos** que a pergunta juntava: a chave
surrogada que o usuário vê, e o contador interno de ordem. Os quatro tratam os dois de modo
oposto.

**(a) Chave surrogada visível (`AUTO_INCREMENT` / `SEQUENCE`) — a faixa É a receita**

| motor | peso | faixa por servidor? | fonte |
|---|---:|---|---|
| PostgreSQL | 4 | **SIM, mas à mão** — `CREATE SEQUENCE … INCREMENT BY / START WITH`; e o objeto sequência **não é replicado**: *«Sequence data is not replicated.»* | <https://www.postgresql.org/docs/current/logical-replication-restrictions.html> |
| MariaDB | 3 | **SIM** — `auto_increment_increment` / `auto_increment_offset`, e o Galera os **deriva da visão do cluster**: `auto_increment_offset = view.own_index() + 1` e `auto_increment_increment = view.members().size()`, em `Wsrep_server_service::log_view` | `sql/wsrep_server_service.cc` (MariaDB/server, `main`) |
| MySQL | 2 | **SIM** — *«intended for use with circular (source-to-source) replication»*; **variáveis de servidor**, nunca campo de esquema; 1..65.535; série = `offset + N × increment` | <https://dev.mysql.com/doc/refman/8.4/en/replication-options-source.html> |
| SQLite | 1 | **NÃO TEM** — só `sqlite_sequence(name,seq)`, contador monotônico por tabela, sem conceito de nó | <https://sqlite.org/fileformat2.html> |

**9 × 1** a favor da faixa — **para a `Sequence`**. Confirma o pedido 290, que já está decidido, e
confirma a correção do dono de 17/09 (o `início` sai da identidade do nó, não do esquema): é
literalmente o que o Galera faz, derivar o offset da visão do cluster em vez de gravá-lo.

**(b) Contador interno de ordem/versão — a faixa NÃO é a receita, e não é de ninguém**

| motor | peso | contador de ordem | particionado por nó? | identidade entre nós | fonte |
|---|---:|---|---|---|---|
| PostgreSQL | 4 | `xmin`/`xmax` (`TransactionId`, `uint32`); `xl_xid` em todo registro do WAL | **não** | par `(roident de `pg_replication_origin`, LSN)` | `src/include/access/xlogrecord.h`, struct `XLogRecord` |
| MariaDB | 3 | `seq_no` `uint64`, monotônico **por servidor** | **não** | trio `(domain_id u32, server_id u32, seq_no u64)` — *«The combination of server ID and sequence number ensures global uniqueness»* | <https://mariadb.com/docs/server/ha-and-performance/standard-replication/gtid> |
| MySQL | 2 | `DB_TRX_ID` 6 B, `DB_ROW_ID` 6 B, `DB_ROLL_PTR` 7 B | **não** | par `(SID uuid 16 B, GNO int64 8 B)` | <https://dev.mysql.com/doc/refman/8.4/en/innodb-multi-versioning.html> · `libs/mysql/binlog/event/control_events.h`, `class Gtid_event` |
| SQLite | 1 | nenhum id de transação em lugar nenhum | n/a | não tem nó | <https://sqlite.org/fileformat2.html> |

**0 de 4 particionam o contador de ordem. 3 de 3 maduros resolvem por PAR.** Unanimidade, nos
dois sentidos.

### 2.2 A decisão

**CONFIRMO o NÃO do DBA** — e por um motivo mais forte que o dele.

O parecer recusou por *«é irreversível e nenhum arranjo em uso hoje precisa dela»*. É verdade e é
pouco: ambos os argumentos envelhecem no dia em que aparecer um arranjo que precise. O motivo
que não envelhece é o medido acima: **nenhum motor particiona um contador de ordem, e os três
maduros com multi-origem resolvem a identidade entre nós por par — e o par já existe aqui**:
`origem: u16` no cabeçalho do evento (`crates/phxsql-store/src/log.rs:143-150`, hash `u16` do
`id_servidor`) mais o contador local do `no.rs`. Ligar o `mod N` seria inventar um mecanismo que
nenhuma das quatro fontes usa, e pagá-lo em irreversibilidade.

### 2.3 O argumento novo, que o parecer não tinha: o `mod N` piora a leitura

Hoje os dois nós numeram `1, 2, 3…` e a incomparabilidade é **óbvia** — `FORMATO.md` já a escreve:
*«O alcance da garantia é POR NÓ… Ordem global entre nós não se reivindica.»*

Com `mod N`, o nó A numeraria `1, 21, 41…` e o B `2, 22, 42…`. Os números passariam a **parecer
comparáveis** e continuariam não sendo, porque os dois contadores avançam a taxas diferentes: um
caixa que vendeu 300 vezes e outro que vendeu 3 produzem carimbos cuja ordem não diz nada sobre o
tempo. **Faixa disjunta convida à comparação errada** — e os maduros confirmam que ordem entre
nós não sai de contador: MySQL e MariaDB carregam um **relógio lógico** separado
(`last_committed` / `sequence_number`, 8 bytes cada, no `Gtid_event`) só para poder aplicar em
paralelo, e isso é grafo de dependência, não ordem total.

### 2.4 A hipótese que morreu

**H1 morreu, e morreu com a premissa certa.** A faixa por servidor **é** a receita dos maduros —
9 × 1 — mas do **objeto errado**. Ela vale para a chave surrogada, e lá nós já a adotamos (pedido
290). Nenhum dos quatro a estende ao contador de ordem. A receita estava certa; o alvo estava
trocado.

**H3 morreu medida:** o `mod N` não compra ordem entre nós. Mata o empate e não cria a ordem — o
DBA já dizia isso, e as quatro fontes confirmam por que: ordem entre nós, nos três maduros, vem
de um id de transação carimbado na origem e **honrado** no destino, nunca de um contador local
particionado.

**H2 sobreviveu inteira.**

### 2.5 O que isto NÃO decide

Não toca no pedido 290. O `passo` está no disco (`schema.rs`) e o **`início` continua sem via de
produção** — `definir_inicio_da_sequencia` tem zero chamadores fora de teste (medido pelo DBA em
23/09). A recomendação do DBA ali fica de pé e esta decisão não a atrasa nem a acelera: são
objetos diferentes, que é exatamente o achado da §2.1.

---

## 3. DECISÃO 2 — id de transação no `.log`: **A e B, juntos**

### 3.1 A matriz — e a pergunta era três perguntas

| pergunta | PostgreSQL (4) | MariaDB (3) | MySQL (2) | SQLite (1) | placar |
|---|---|---|---|---|---:|
| **(a)** carrega **id** de transação no fluxo? | **SIM** — `xl_xid` (`TransactionId`) em **todo** registro do WAL físico; no fluxo lógico, campo `Xid` no `Begin`, `Stream Start`, `Stream Commit`, `Begin Prepare` | **SIM** — `GTID_EVENT` (`0xa2`): `seq_no` u64 + `domain_id` u32 + `server_id` do cabeçalho comum | **SIM** — `Gtid_event`: `SID` 16 B + `GNO` 8 B; e `Xid_event` de 8 B no commit | **NÃO** — zero id em todo o formato do WAL | **9 × 1** |
| **(b)** marca a **fronteira**? | SIM — `Begin`/`Commit`, `Stream Start`/`Stream Commit` | SIM — `GTID_EVENT` abre (substitui o `BEGIN`), `XID_EVENT`/`COMMIT` fecha | SIM — `Gtid_event` abre, `Xid_event` fecha | **SIM** — cabeçalho do *frame*, offset 4: tamanho do banco em páginas após o commit; **zero** nos demais | **10 × 0** |
| **(c)** o id vai em **cada** evento de linha? | **NÃO** — `Insert`/`Update`/`Delete` só carregam `Xid` *«only present for streamed transactions»* (protocolo v2) | **NÃO** — um `GTID_EVENT` por **grupo de eventos**, nunca por linha | **NÃO** — cabeçalho comum de **19 bytes** (`timestamp` 4, `event_type` 1, `server_id` 4, `event_size` 4, `log_pos` 4, `flags` 2) **não tem campo de transação** | n/a | **9 × 0** para «uma vez, na fronteira» |

**Fontes:**
`src/include/access/xlogrecord.h` (struct `XLogRecord`: `uint32 xl_tot_len; TransactionId xl_xid; XLogRecPtr xl_prev; uint8 xl_info; RmgrId xl_rmid; pg_crc32c xl_crc;`) ·
<https://www.postgresql.org/docs/current/protocol-logicalrep-message-formats.html> ·
<https://mariadb.com/docs/server/reference/clientserver-protocol/replication-protocol/gtid_event> ·
`libs/mysql/binlog/event/control_events.h` (mysql-server `trunk`), `class Gtid_event` linhas 928-1053 e `class Xid_event` linhas 497-541 ·
<https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_replication_binlog_event.html> ·
<https://sqlite.org/fileformat2.html> (seção *Write-Ahead Log*, cabeçalho de frame de 24 bytes).

### 3.2 A decisão, e por que não é «A ou B»

A pergunta chegou como escolha entre A e B. **Medida, não é escolha: são as duas metades da mesma
receita, e todas as quatro fontes trazem as duas.**

- **A fronteira é unânime: 10 × 0.** Os quatro a marcam, inclusive o SQLite, que não tem mais
  nada. Ela **entra**.
- **O id é a convergência dos três maduros: 9 × 1.** Entra **sem pergunta**, pela régua 1 — e não
  há pétrea nossa se opondo; ao contrário, *«mudança de formato entra cedo»* é pétrea **a favor**.

**B sozinho é a resposta do SQLite** — o motor de peso 1, o único que não tem multi-origem no
núcleo e o único cujo WAL não carrega id nenhum. Escolher B por ser grátis é adotar a receita de
peso 1 contra a dos três maduros somando 9, e pagar o preço que a própria proposta já declarava:
*«não agrupa venda com itens»*.

**Decisão: entram os dois.** O id de 8 bytes **e** o bit de fronteira.

### 3.3 Onde divergimos dos três, e qual restrição nossa causa a divergência

Os três maduros põem o id **uma vez por transação, na fronteira** (9 × 0, §3.1c). Aqui ele vai
**em cada evento**, e a restrição que causa a divergência tem nome e endereço:

> **O fluxo deles é UM, ordenado, por servidor. O nosso é POR TABELA.**
> `docs/REPLICACAO.md` §4: *«A réplica guarda um número por tabela e pede o que falta.»*

No PostgreSQL, no MySQL e no MariaDB, «os eventos entre o marcador de início e o de fim pertencem
a esta transação» é verdade porque **existe um só arquivo e uma só ordem**. Aqui, um marcador no
`.log` de `venda` não diz uma palavra sobre o `.log` de `itens` — são dois arquivos, duas
posições, dois laços. O marcador continua útil **dentro** de cada tabela (é o que B entrega), e o
que atravessa tabelas só pode ser o **id carimbado no evento**.

Segunda divergência, menor: o id deles é um **par/trio** (`SID`+`GNO`, `domain`+`server`+`seq`).
Aqui metade do par **já está gravada**: `origem: u16` nos bytes 10..12 do cabeçalho do evento. O
campo novo é só a segunda metade — e isso é economia que cai da nossa própria história
(`bidirecional`), não da receita deles.

Terceira: **8 bytes, e não 4.** O `xl_xid` do PostgreSQL é `uint32` e custa à casa dele a máquina
inteira de wraparound e `VACUUM FREEZE`. MariaDB (`seq_no` u64) e MySQL (`GNO` int64) usam 8, e
os dois pesam 5 contra 4. **5 × 4 para 8 bytes** — e o desempate real não é o voto, é não herdar
wraparound num motor que não tem `VACUUM`.

### 3.4 O custo de A, re-medido — o parecer superestimou

O parecer do DBA escreve: *«cabeçalho 44→52, **versão nova do `.log`**; uma migração por tabela
replicada»*. Medido no fonte de hoje:

| afirmação | medido | onde |
|---|---|---|
| o cabeçalho de 44 bytes está cheio | **confirmado** — 0..8 carimbo, 8 operação, 9 flags, 10..12 origem, 12..20 rowid, 20..28 versão, 28..32 usuário, 32..36 tam_imagem, 36..40 CRC, 40..44 tempero. **Zero byte livre** | `log.rs`, `fn escrever` |
| o byte de flags tem espaço | **7 bits livres de 8** — só `FLAG_IMAGEM = 1` existe | `log.rs:92` |
| «uma migração por tabela» | **não** — o `.log` é *append-only* e o volume **rola**; o volume novo nasce com cabeçalho próprio. Precedente idêntico já pago: a cifra v2→v3, em que *«um `.log` que já existe em claro continua em claro, porque um arquivo append-only não se reescreve»* | `log.rs:482-503`, `log.rs:48-56` |
| o custo real de código | `EVENTO_CAB` deixa de poder ser constante: **24 ocorrências em 2 arquivos** (`crates/phxsql-store/src/log.rs` e `crates/phxsql-store/examples/quanto-ocupa.rs`) | medido por varredura |
| «source e réplica sobem juntos» | **continua verdade, e a quebra é ALTA e nomeada**: `PhxError::VersaoNaoSuportada { arquivo, encontrada, suportada }` | `cofre.rs:794-800`; teto hoje é 3, em `cofre.rs:924` |

**Custo em bytes, aritmética sobre o número do próprio módulo (não é bancada):** o `log.rs`
declara que *«um registro de 200 bytes gasta ~244 bytes de diário por alteração»* → 8/244 =
**+3,3%** de diário na linha com imagem. Numa **exclusão**, que não leva imagem, 8/44 = **+18,2%**.
O número que decidiria isto na bancada é o tamanho médio de imagem da carga real; não foi medido.

### 3.5 A variante barata que eu avaliei e RECUSEI, com o motivo medido

Antes de decidir, testei uma terceira via que não estava na mesa: **um `Operacao` novo (tag 4) de
marcador de transação, reusando os campos `rowid`/`versao` para o id** — custo **zero byte de
cabeçalho** e **zero mudança de layout**, e é a forma exata dos três maduros (evento de fronteira
próprio: `GTID_EVENT`, `Xid_event`, `Begin`/`Commit`).

**RECUSADA, e o número que a recusa:** `Operacao::de_tag` devolve `Err` para tag desconhecida
(`log.rs:116-121`), e a cura de arranque faz `Err(_) => break` (`log.rs:568-571`) e em seguida
**grava o cabeçalho encurtado**. Um binário velho lendo um `.log` com tag 4 **não recusa: trunca
em silêncio** tudo o que vem depois do primeiro marcador, e declara sucesso.

O contraste decide sozinho, e a lei já estava escrita nesta casa (`FORMATO.md`, justificativa da
v8 e da v10): **«recusa alta é melhor que aceite errado.»**

- **A** (versão nova de volume) → `VersaoNaoSuportada`, nomeando arquivo, versão encontrada e
  versão suportada. **Alta.**
- **tag nova em volume v2/v3** → truncamento calado. **A pior falha possível neste repositório.**
- **B** (bits do byte de flags) → o leitor velho lê `dst[9] & FLAG_IMAGEM` e **ignora** os bits que
  não conhece. Degradação benigna: não vê a fronteira, não quebra nada. É por isso que B é mesmo
  grátis — e continua entregando a resposta de peso 1.

A versão nova de volume, então, **não é o preço de A: é o que torna a quebra alta.**

### 3.6 A pétrea «guarda nova entra pedida, não imposta»

Ela alcança esta decisão e **não a bloqueia** — indica a condição. O precedente está no mesmo
arquivo: a cifra nasce desligada, e o volume só sobe de versão quando o `config.json` pede
(`log.rs:48-56`, `cofre.rs`). O id de transação entra do mesmo jeito: **o volume só rola para a
versão nova quando o arranjo pede**, e o par 1↔1 que está de pé hoje continua subindo byte a
byte. O teste que mais importa é o do comportamento **velho**.

### 3.7 A hipótese que morreu

**H5 morreu: 1 contra 9.** «Só fronteira» é o que o SQLite faz, e funciona lá porque o WAL dele é
um arquivo único e ordenado. Transplantada para um diário **por tabela**, a fronteira deixa de
alcançar o que a pergunta pedia — e a própria proposta B já dizia isso; o que faltava era o
número que mostra que ela é a resposta do motor mais fraco da régua.

**H6 morreu no fonte**, e o achado vale mais que a hipótese: **não há reserva.** Ver §5.

**H4 sobreviveu, corrigida:** eles carregam o id **e** a fronteira, não um ou outro.

---

## 4. As duas lacunas que ficam — e nenhuma é do dono

### 4.1 Completude: o id diz «a qual transação», não «chegou tudo»

Com o id e a fronteira, a réplica sabe que o evento de `itens` pertence à transação X e que X
acabou **naquela tabela**. Continua sem saber **quantas tabelas** X tocou — e é isso que separa
«meio commit chegou» de «o commit era assim mesmo».

Os maduros ganham essa resposta **de graça** do fluxo único. MySQL a reforça com
`transaction_length` no `Gtid_event` — *«The packed transaction's length in bytes, including the
Gtid»* (`control_events.h:999-1002`). **Essa receita não se transplanta**: comprimento em bytes
só faz sentido num fluxo só.

O que fecharia aqui é a **lista (ou a contagem) das tabelas participantes** gravada na fronteira.
O conjunto de escrita já existe em RAM até o `COMMIT` e já é serializado na marca desde o ACID-C
(super-journal, marca v3 — `docs/FORMATO.md`, `docs/ACID.md` §2.4). **É desenho do papel C, não
decisão do dono, e eu não o invento aqui.**

### 4.2 O `.log` por tabela continua sendo a escolha certa? A premissa caducou

`docs/REPLICACAO.md` §4 justifica a posição por tabela assim: *«Porque o PhxSql ainda não tem
transações entre tabelas. Sem transação, não existe ordem global que precise ser preservada.»*

**A premissa caducou**: há transação desde o pedido 162, e desde o ACID-C (15/09) a cascata entra
inteira no conjunto de escrita. A conclusão pode continuar certa — replicar em paralelo por tabela
é ganho real —, mas ela hoje se apoia num fato que deixou de ser fato. **Medir a premissa do item
vem antes de implementar o item, inclusive quando o item é nosso.** Item para o papel C.

---

## 5. Dois documentos que envelheceram — achados no caminho, entregues porque decidem

**(1) `crates/phxsql-store/src/log.rs`, linhas 13-16, contradiz o próprio código.** O desenho no
comentário do módulo ainda mostra:

```text
[carimbo i64 ms][operacao u8][flags u8][res u16]
[rowid u64][versao u64][usuario u32]
[tam_imagem u32][crc32 u32][res u32]
```

Os dois `res` **não existem mais**: os bytes 10..12 são `origem` (`fn escrever`) e os 40..44 são
`tempero` (`OFF_TEMPERO = 40`). `docs/FORMATO.md` está **certo** e atualizado; quem ler o fonte
lê errado. **Comentário que promete reserva que não existe é pior que comentário faltando** — foi
exatamente por ele que a hipótese H6 nasceu.

**(2) `docs/REPLICACAO.md` §4 promete uma reserva já gasta, duas vezes.** A frase é: *«Quando as
transações entrarem, entra junto um número de sequência do database inteiro; **o campo reservado
do cabeçalho do evento já está guardado para isso**.»* Medido: o `res u16` foi gasto pela
`origem` (bidirecional) e o `res u32` pelo `tempero` do nonce (cifra). **O plano escrito para esta
exata decisão contava com um campo que duas frentes posteriores consumiram, cada uma sem saber do
plano.** É a razão medida pela qual a decisão 2 não tem caminho grátis — e é por isso que ela
chegou à mesa como «A custa versão nova».

*(Correção de rota: nenhum dos dois é reafirmação de pétrea — o alcance novo é que
**reserva declarada num documento também envelhece**, e envelhece em silêncio, porque ninguém
recompila um comentário.)*

---

## 6. O que eu NÃO medi

- **Tamanho médio de imagem na carga real** — é o número que transforma os +3,3% / +18,2% da
  §3.4 em custo de disco. Bancada não rodou (disco em 92%; `cargo build --release` proibido nesta
  rodada). **Raciocinado, não medido.**
- **Custo de rolar volume ao subir a versão** — não medido; o que decidiria é o `fsync` do
  cabeçalho novo contra a taxa de virada de volume da carga.
- **O fonte do MariaDB para o `XID_EVENT`** — li o `GTID_EVENT` na documentação de referência e o
  `Xid_event` no fonte do **MySQL**; para o MariaDB a afirmação «`XID_EVENT`/`COMMIT` fecha o
  grupo» vem da documentação, não do `log_event.h` dele.
- **Se o PostgreSQL 18 replica sequência** — li a restrição na documentação *current*; não
  conferi se uma versão mais nova a removeu. Não muda a decisão 1: a conta (b) não depende disso.

## 7. As recusas medidas desta rodada

| recusado | número que recusa |
|---|---|
| `mod N` no `rowstamp` | **0 de 4** motores particionam contador de ordem; **3 de 3** maduros resolvem por par, e o par já existe (`origem u16`) |
| opção **B** sozinha | **1 contra 9** — é a resposta do SQLite, e ela só funciona lá porque o WAL é fluxo único |
| `Operacao` tag 4 em volume v2/v3 (o «A de graça») | o leitor velho **trunca em silêncio** (`de_tag` → `Err`, `curar` → `break`, depois grava o cabeçalho encurtado), em vez de recusar alto |
| id de transação de **4 bytes**, como o `xl_xid` do PG | **5 × 4** para 8 bytes, e herdaria wraparound num motor sem `VACUUM` |
| `transaction_length` do MySQL como medida de completude | comprimento em bytes só fecha num fluxo único; o nosso é por tabela |

## 8. Fonte alheio lido nesta rodada

- PostgreSQL — `src/include/access/xlogrecord.h` (struct `XLogRecord`) ·
  docs `protocol-logicalrep-message-formats` · docs `logical-replication-restrictions`
- MySQL — `libs/mysql/binlog/event/control_events.h` (`Gtid_event` 928-1053, `Xid_event` 497-541) ·
  docs `replication-options-source` (`auto_increment_*`) · docs `innodb-multi-versioning` ·
  docs `page_protocol_replication_binlog_event`
- MariaDB — `sql/wsrep_server_service.cc` (`Wsrep_server_service::log_view`) ·
  docs GTID · docs `GTID_EVENT`
- SQLite — `fileformat2.html`, seção *Write-Ahead Log* e `sqlite_sequence`
