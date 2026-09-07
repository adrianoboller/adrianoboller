# Q) leitura do manual do Cassandra fonte e help verificando gaps ainda existentes gerar lista de sprints que julgue importante

## Resposta curta
O estudo anterior propôs **5 sprints**: **1 fechou**, **4 sobreviveram**, dois deles
encolhidos. Mandei **18 comandos do CQL: 18 recusados** — e é o esperado, porque CQL não é
SQL. O que o item pede de verdade é a outra metade: das **12 famílias** de recurso do
Cassandra(R) que não há aqui, **9 estão recusadas com motivo** (rowid é endereço, o slot é
fixo, a unicidade se confere na gravação), **2 viram sprint** e **1 é a inspiração da
leitura** — o `ALLOW FILTERING`, que só cabe **divergindo** do original. **Lista final: 5
sprints.**

## Exemplo exercitado

Corrida de **2026-09-07 16:49 UTC**, commit **a56a165**. **Fonte:**
https://cassandra.apache.org/doc/latest/cassandra/developing/cql/index.html, lida nesta
rodada, mais o `docs/CASSANDRA.md` desta casa (lido no fonte da 5.0.10, commit `7b5ab44`).

### Os 5 sprints antigos, conferidos contra o motor

| # | sprint de `SPRINTS-CASSANDRA.md` | estado hoje | a prova, desta corrida |
|--:|---|---|---|
| 1 | O `fsync` da lixeira entra na janela de durabilidade | **FECHADO** | o interruptor existe e nasce **desligado**, como manda «guarda nova entra pedida»: `{"op":"config"}` → `… 'recursos': {'durabilidade': 'por_lote', 'exclusao_na_janela': False, 'lote_operacoes': 200, 'lote_milissegundos': 200, 'cache_paginas': 2048, 'diario_volume_mib': 0, …` |
| 4 | Duas condições no `WHERE`: interseção de rowids | **SOBREVIVE, e encolheu** | o substrato **já faz AND**: `{"op":"varrer","onde":[{cidade=Blumenau},{nome=Adriano}]}` → `{'registros': 3, 'visiveis': 3, 'marcadas': 0, 'devolvidas': 1, 'examinadas': 3, 'modo': 'posicao', …`. Falta a tradução e a decisão de quando intersecar: `SQL, coluna 54: o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, e nao ha planejador que decida por qual indice comecar` |
| 3 | A posição confirmada de cada réplica, no source | **SOBREVIVE, e encolheu** | o argumento de correção do sprint era *«o cluster com eleição decide sem essa conferência»* — e **hoje ele decide POR ela**: o pulso carrega `posicao` e a eleição escolhe a maior (`cluster.rs`, «entre os elegíveis vence a maior posição do diário»). O que continua faltando é a posição **confirmada por réplica guardada no source**: `{"op":"replicacao_estado"}` → `{'papel': 'isolado', 'papel_configurado': 'isolado', 'id_servidor': '', 'somente_leitura': False, 'origens': {}}` |
| 2 | A retenção do diário no multi-master (o `gc_grace` da casa) | **SOBREVIVE** | o giro de volume existe como opção (`recursos.diario_volume_mib: 0`, desligado), mas **retenção é outra coisa** — não há regra de quanto tempo um evento precisa sobreviver para uma origem parada não ressuscitar linha. Depende do sprint 3 para a versão completa |
| 5 | TTL por linha, como coluna e job | **SOBREVIVE**, e a premissa morta continua morta | a premissa do estudo irmão dizia que *«um job com `UPDATE … SET SOFTDELETED = 1 WHERE prazo < NOW()` já faz isso hoje»*. **Não faz**, e agora está medido pelos dois lados: `UPDATE ainda nao existe nesta camada -- so SELECT` e, no corpo de rotina, `UPDATE dentro de rotina ainda nao existe: o motor atualiza e exclui por rowid, e traduzir o WHERE pede o planejador` |

### O CQL contra o motor: 18 comandos, 18 recusas

Nenhuma surpresa, e é por isso que a lista vale: ela separa *«recusou porque
falta»* de *«recusou porque não é a mesma linguagem»*.

```
[RECUSADO] CREATE KEYSPACE ks WITH replication = {'class':'SimpleStrategy'}
           SQL, coluna 39: caractere '{' nao faz parte da linguagem
[RECUSADO] CREATE TABLE t (pk text, ck text, v text, PRIMARY KEY ((pk), ck))
           CREATE nesta camada cria TRIGGER ou PROCEDURE
[RECUSADO] INSERT INTO clientes (id,nome) VALUES (9,'X') USING TTL 86400
           INSERT ainda nao existe nesta camada -- so SELECT
[RECUSADO] INSERT … IF NOT EXISTS                    (mesma recusa)
[RECUSADO] UPDATE clientes USING TIMESTAMP 1 SET …   (UPDATE ainda nao existe)
[RECUSADO] CONSISTENCY QUORUM
           CONSISTENCY nao e um comando desta camada
[RECUSADO] SELECT * FROM clientes WHERE cidade = 'Blumenau' ALLOW FILTERING
           SQL, coluna 50: sobrou "ALLOW" depois do fim do comando; um comando por vez
[RECUSADO] SELECT * FROM clientes WHERE token(id) > 0
           SQL, coluna 35: esperava um comparador (=, <>, <, <=, >, >=)
[RECUSADO] BEGIN BATCH … APPLY BATCH
           SQL, coluna 1: sobrou "BATCH" depois do comando de begin
[RECUSADO] CREATE MATERIALIZED VIEW mv AS …          (CREATE … TRIGGER ou PROCEDURE)
[RECUSADO] CREATE TYPE endereco (rua text, cidade text)             (idem)
[RECUSADO] CREATE TABLE c2 (id int PRIMARY KEY, tags set<text>)     (idem)
[RECUSADO] UPDATE clientes SET saldo = saldo + 1 WHERE id = 1       (counter)
[RECUSADO] CREATE CUSTOM INDEX ix ON clientes (cidade) USING 'StorageAttachedIndex'
[RECUSADO] TRUNCATE clientes
           TRUNCATE nao e um comando desta camada
[RECUSADO] DESCRIBE KEYSPACES
           DESCRIBE nao e um comando desta camada
[RECUSADO] SELECT * FROM clientes ORDER BY vetor ANN OF [1.0] LIMIT 5
           SQL, coluna 46: caractere '[' nao faz parte da linguagem
[RECUSADO] USE ks
           USE nao e um comando desta camada
```

### O que faz sentido aqui, e o que não faz — a pergunta do item

A régua é a lei da casa: *inspiração, não cópia* — e o teste dela é **«onde esta
lógica DIVERGE da de origem, e qual restrição nossa causou a divergência?»**.
Onde a resposta é «em lugar nenhum», não é inspiração: é cópia com outro nome.

**Faz sentido — 3**

| do Cassandra(R) | por que cabe, e ONDE diverge |
|---|---|
| **`ALLOW FILTERING`, a cláusula que assume o custo** | Cabe porque o problema é o **mesmo**: `SELECT * FROM clientes WHERE cidade = 'Blumenau'` recusa aqui dizendo `exige um indice de uma coluna sobre cidade. Nao existe. O varrer filtra, mas dentro da pagina que ele EXAMINA -- e um SELECT que respondesse sobre a primeira pagina teria a cara de uma resposta completa`. Lá, a resposta a esse impasse é uma palavra que o autor escreve para assumir a varredura. **A divergência, e a restrição que a causa:** o `varrer` daqui **é paginado por desenho** (`'examinadas': 3`, `'ha_mais': False`, `cursor_inicio`/`cursor_fim`), então a cláusula não pode significar «varra tudo» como lá — ela tem de carregar o contrato de página, e a resposta tem de dizer **quantas examinou** e **se acabou**. Copiar a palavra sem isso devolveria meia resposta com cara de resposta inteira, que é exatamente o que a recusa de hoje evita |
| **TTL por linha** (sprint 5, sobrevivente) | Cabe **como coluna do motor expirando para o `SOFTDELETED`**, e não como apagar. **A divergência:** lá o TTL some com o dado; aqui a exclusão é suave por padrão, reversível, com motivo e lixeira — e a ordem de digitação proíbe reaproveitar o slot. Um TTL que apagasse de verdade quebraria as duas coisas. **E a premissa continua sendo do dono:** em ERP, dado que some sozinho costuma ser defeito |
| **`gc_grace`: a retenção do diário** (sprint 2, sobrevivente) | Cabe porque o problema é real no multi-master, que **existe aqui**. **A divergência:** lá o `gc_grace` protege *tombstones* contra ressurreição num anel sem ordem; aqui protege eventos do diário contra uma origem parada — a unidade é o evento do `.log`, não a lápide, e a decisão é «quanto tempo antes de girar o volume», não «quanto antes de compactar» |

**NÃO faz sentido — 9, com o motivo**

| do Cassandra(R) | por que não cabe |
|---|---|
| **`CONSISTENCY` ajustável (`ONE`/`QUORUM`/`ALL`)** | não há N réplicas da **mesma linha** decidindo por voto: há um source com diário e réplicas que aplicam. E o `docs/CASSANDRA.md` já mediu no fonte deles que o `QUORUM` **não** quer dizer «está em N discos» — quer dizer «N processos copiaram para um `mmap`», com `fsync` a cada 10 s numa thread de fundo. Trazer a palavra sem a semântica seria prometer durabilidade que o número não sustenta |
| **`token()`, partições por hash, `vnodes`, *snitch*, *gossip*** | o rowid aqui **é endereço** (`offset = data_offset + (rowid−1) × slot_size`), e a partição é por letra ou por volume (`.pag`). Um anel de tokens desfaria a ordem de digitação, que é pétrea |
| **`IF NOT EXISTS` (LWT, Paxos)** | **aqui a casa está à frente, e isso está medido no fonte deles**: o `INSERT` do Cassandra(R) não sabe recusar chave repetida, porque não lê antes de gravar — o conflito se resolve depois, por carimbo de hora. Aqui o índice único recusa na hora. Trazer LWT seria trazer a muleta de um problema que não temos |
| **`USING TIMESTAMP` / *last write wins* por carimbo** | mataria a conferência de unicidade e a regra primordial da integridade. Lá é coerente; aqui é destruição |
| **`BEGIN BATCH` / `APPLY BATCH`** | o `BULKINSERT` já dá exclusividade e uma sincronização no fim (`{'bulkinsert': True, 'database': 'loja', 'tabela': 'itens', 'reservada': True, 'expira_em_s': 1800, 'prazo_min': 30}`), e há transação de verdade desde o pedido 162. O batch *logged* deles é mais fraco que a transação e mais forte que o `BULKINSERT` — o degrau do meio não tem quem o queira |
| **`counter`** | recusado em `SPRINTS-CASSANDRA.md` §5.1; e `UPDATE … SET saldo = saldo + 1` pede o avaliador de expressão, que é o item 4 desta lista por outro caminho |
| **materialized views** | §5.4. Uma view materializada é uma segunda cópia do dado que o motor mantém em sincronia — segundo caminho até o dado, que é sempre o que esquece uma conferência |
| **collections (`set`/`list`/`map`) e UDT** | §5.6. O slot é **fixo**. Coleção de tamanho livre dentro do slot não existe; fora dele, é `.memo`, que já existe |
| **busca vetorial / `ANN OF`** | §5.5. E a recusa desta corrida é literal: `caractere '[' nao faz parte da linguagem` |
| ***compaction*** | §5.7. Compactar renumera rowid, e rowid é endereço |

**E uma que fica no meio:** o **`CREATE CUSTOM INDEX` (SAI)**. O índice
secundário deles é o parente do índice de texto daqui, e o `.fts` **já fechou**
essa metade (sprint 13 do MariaDB(R), provado na resposta O). A outra metade é
um gap real e **sem caminho nenhum**: *não existe criar índice depois de a
tabela nascer* — e isso é decisão registrada
(`docs/PARECER-175-INDICE-NA-DECLARACAO.md`), não esquecimento.

### A lista de sprints que eu proponho — 5

| ordem | sprint | tam. | por quê, e a premissa que pode matá-lo |
|--:|---|:--:|---|
| 1 | **Duas condições no `WHERE`** (antigo 4; = O-4, = P-3) | **M** | é o mesmo item por três caminhos, e a conferência **encolheu-o**: o `varrer` já faz AND. **Premissa:** o ponto de virada por seletividade entre intersecar rowids e filtrar varrendo — se a interseção nunca ganhar 1,2×, sobra só a metade barata |
| 2 | **A cláusula que assume a varredura** (inspirada no `ALLOW FILTERING`) — nova | **P**, e depende do 1 | fecha a recusa mais comum da camada SQL sem mentir sobre a resposta. **Premissa, e ela pode matá-lo:** com o item 1 pronto, sobra caso em que a pessoa **ainda** quer a varredura assumida? Se não sobrar, o sprint morre com o número na mesa |
| 3 | **A posição confirmada de cada réplica, no source** (antigo 3) | **M** | encolheu — a eleição já compara posição —, mas o source continuar sem saber até onde cada réplica chegou é o que impede a retenção do 4. **Premissa:** a posição guardada bate com a informada dentro de um lote (500), e a taxa do master não cai 1% |
| 4 | **A retenção do diário no multi-master** (antigo 2) | **M** | depende do 3. **Premissa:** com o diário girando volumes, uma origem parada ressuscita linha? Se sim, **vira correção e sobe**; se não, é prevenção e fica onde está |
| 5 | **TTL por linha, expirando para o `SOFTDELETED`** (antigo 5) | **M** | **a premissa é sua, não minha:** você tem esse caso de uso? Depois: o custo da varredura do job, e se a coluna nasce indexada |

## O que NÃO existe, e é dispensa registrada

- **Não subi um Cassandra(R) nesta rodada, e não li o fonte dele de novo.** O
  `docs/CASSANDRA.md` foi lido no fonte da 5.0.10 (commit `7b5ab44`) e é a fonte
  desta resposta para tudo o que afirmo sobre eles; a documentação oficial
  confirmou a lista de assuntos do CQL. Onde eu afirmo o que o PhxSql faz, a
  fonte é a saída colada.
- **Não subi cluster nem par de replicação.** Os sprints 3 e 4 desta lista têm
  premissa a medir, e **eu não a medi**: o que está medido é que
  `replicacao_estado` devolve `origens: {}` num servidor isolado. Afirmar que a
  origem parada ressuscita linha sem ter contado seria diagnóstico plausível.
- **Não há CQL aqui, e não vai haver.** A op `sql` fala SQL; um dialeto de
  chave-partição sobre um motor cujo rowid é endereço não seria compatibilidade,
  seria fantasia. As 18 recusas coladas acima são a prova de que a decisão está
  no código e não só no documento.
- **Nenhuma linha de código do Cassandra(R) foi copiada, aqui ou antes.** A lei
  da casa pede a pergunta e não a promessa: das três coisas que eu digo caberem,
  **as três nomeiam a restrição nossa que causa a divergência** — a página do
  `varrer`, a exclusão suave com lixeira, e o evento do diário no lugar da
  lápide. Onde eu não soube nomear a divergência, o item foi para a coluna do
  «não cabe».

## Como se refaz

```bash
python3 bancada/gaps-sql/sondar.py cassandra
```
