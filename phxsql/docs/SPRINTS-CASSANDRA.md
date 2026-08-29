# Sprints inspirados no Cassandra(R) — proposta para aprovação

> **Este documento continua sendo a fonte, mas não é mais a lista.** As
> cinco propostas daqui foram para a lista única de `docs/SPRINTS.md`,
> junto com as outras 26 dos manuais do Redis(R), MariaDB(R) e
> Teradata(R) — lá elas estão ordenadas por valor ÷ custo, com as
> duplicatas fundidas e as contradições apontadas. Duas coisas mudaram na
> travessia: o **Sprint 1 foi reescrito** para entrar pedido e não
> imposto (`SPRINTS.md` §2.1 — do jeito que está aqui, ele afrouxaria a
> garantia da exclusão no padrão, sem ninguém pedir), e o **Sprint 2 saiu
> da lista de sprints** por leitura do próprio código (§5.1).

> **Nada deste documento foi executado, e nada será sem o seu sim.** Ele é uma
> lista de trabalho *proposta*, sprint a sprint, para você aprovar, recusar ou
> reordenar. Onde há código novo neste commit, é **um exemplo de medição** —
> `--example custo-do-excluir` — que existe para que a premissa do Sprint 1
> possa ser conferida por qualquer pessoa. Nenhuma funcionalidade entrou.

Documento de **leitura de manual com proposta**, no contrato da casa:

- toda afirmação sobre o Cassandra(R) traz **a fonte** — URL e seção. Afirmação
  sem fonte não entra;
- todo número da casa sai de `bancada/resultados.json`, do `docs/DESEMPENHO.md`
  ou de **medição minha reproduzível**, com o comando escrito;
- todo sprint traz a **premissa a medir primeiro** — a medição que pode
  **matar** o sprint. Matar é resultado válido, e é o que este projeto faz
  melhor;
- o que eu **descartei** está no §5, com o motivo e a fonte. Ele importa tanto
  quanto o que eu propus.

---

## 1. O que já foi lido, e por que este documento não repete o `CASSANDRA.md`

O `docs/CASSANDRA.md` leu o **código-fonte** do Cassandra(R) 5.0 — o caminho de
escrita e o quórum, com `arquivo:linha`. Ele respondeu duas perguntas e as
respondeu bem: o commit log não é o segredo, e o `QUORUM` no modo padrão não
significa disco.

Este documento lê **o manual**, que é outra coisa e serve a outro fim. O código
diz o que o motor *faz*; o manual diz o que os operadores *sofreram* — e é no
manual que estão o `gc_grace_seconds`, o dado que ressuscita, o aviso de que
LWT custa Paxos, o aviso de que *collection* é anti-padrão acima de certo
tamanho e o aviso contra exclusão em coluna fora de uma *materialized view*.
Nada disso aparece lendo o `StorageProxy.java`. **Foi daí que saíram três dos
cinco sprints.**

### Páginas lidas

| Assunto | Página |
|---|---|
| DML: INSERT/UPDATE/DELETE, TTL, BATCH, LWT, LIMIT | `developing/cql/dml.html` |
| DDL: opções de tabela, `gc_grace_seconds`, `default_time_to_live` | `developing/cql/ddl.html` |
| Tipos: counters, collections, UDT, vector | `developing/cql/types.html` |
| Materialized views | `developing/cql/mvs.html` |
| Índices: 2i e SAI | `developing/cql/indexing/indexing-concepts.html` |
| SAI por dentro | `developing/cql/indexing/sai/sai-concepts.html` |
| Busca vetorial | `getting-started/vector-search-quickstart.html` |
| Motor de armazenamento: commit log, memtable, SSTable | `architecture/storage-engine.html` |
| Dynamo: replicação, consistência, gossip, tokens | `architecture/dynamo.html` |
| Tombstones, zumbis e o período de graça | `managing/operating/compaction/tombstones.html` |
| `cassandra.yaml` | `managing/configuration/cass_yaml_file.html` |

Todas sob `https://cassandra.apache.org/doc/latest/cassandra/`, exceto a de
busca vetorial (`https://cassandra.apache.org/doc/latest/`).

### Do lado da casa

`CLAUDE.md`, `docs/CASSANDRA.md`, `docs/CONCORRENTES.md`, `docs/DESEMPENHO.md`,
`docs/PENDENCIAS.md`, `docs/REPLICACAO.md` (§12, o bidirecional),
`docs/CLUSTER.md`, `docs/TRIGGERS.md`, `docs/JOBS.md`, `docs/ODBC.md`,
`docs/MENSAGENS.md`, `docs/ASSISTENTE-REPLICACAO.md`, `bancada/LEIA-ME.md`,
`bancada/resultados.json`, e do código `table.rs` (o `excluir_de_vez`),
`lixeira.rs`, `crates/phxsql-sql/src/sintaxe.rs` e `traduzir.rs`.

---

## 2. As frentes que já entraram — e onde este documento para

Dez frentes entraram na branch enquanto esta leitura acontecia. **Nenhum sprint
abaixo refaz nenhuma delas.** A tabela existe para provar isso, item por item,
porque o pior desperdício possível aqui seria propor o que já está pronto.

| Frente que entrou | O que o Cassandra(R) faria pensar em propor | Por que NÃO proponho |
|---|---|---|
| Gatilhos e procedimentos (`docs/TRIGGERS.md`) | executor de regra por linha | existe, com `BEFORE/AFTER INSERT/UPDATE/DELETE FOR EACH ROW` e `CALL` |
| Quatro modos de replicação (`REPLICACAO.md` §9–§12) | replicação multi-mestre | existe — e o Sprint 2 é **por cima** dela, não no lugar dela |
| Cluster com eleição e promoção (`CLUSTER.md` §2) | *gossip*, detector de falha | existe, com árbitro e maioria |
| Blacklist de IP e mensagens multilíngues | — | nada a acrescentar daqui |
| Jobs com aviso por e-mail (`docs/JOBS.md`) | agendador para varredura de expirados | existe — o Sprint 5 **monta em cima** dele |
| Driver ODBC (`docs/ODBC.md`) | — | fora do assunto |
| Editor ER com arrastar | — | fora do assunto |
| Assistente de replicação na tela | — | fora do assunto |
| Telas de configuração revistas | — | fora do assunto |
| Camada SQL (`crates/phxsql-sql`) | planejador de consulta | existe **e recusa por escrito** o que o Sprint 4 destrava |

E uma nota de manutenção, achada de passagem e **não corrigida aqui** porque o
`PENDENCIAS.md` está fora do meu alcance nesta frente: o bloco gerado do §5 do
`PENDENCIAS.md` ainda diz que *«a inserção é o ponto fraco do motor […] 0,8×
mais devagar»* — com os números **do lado dele** dizendo o contrário (109.300
linhas/s contra 88.994 do MySQL(R)). O `resultados.json` atual mede
**91,491 s contra 112,367 s**: o insert **ganha**. O gerador não sobreviveu à
virada de sinal — ele calcula a razão certa e imprime a palavra errada. É a
mesma família do selo de capa parado em 0.11.0, e vale uma linha na próxima
rodada que puder tocar aquele arquivo.

---

## 3. O achado que reordenou a lista

O `DESEMPENHO.md` §6 registra, sobre a exclusão, uma frase que ficou solta:

> «É a única fase em que o PhxSql *espera disco*: 4,3 s de CPU para 8,16 s de
> relógio.»

E a bancada de hoje (`bancada/resultados.json`) mostra a exclusão como a
**única** fase em que o motor perde:

| fase (10.000.000 de linhas) | PhxSql | MySQL(R) | |
|---|---:|---:|---|
| inserir 10.000.000 | **91,49 s** | 112,37 s | ganha |
| buscar 20.000 | **0,20 s** | 2,64 s | ganha |
| varrer 1.250.000 | **1,41 s** | 15,70 s | ganha |
| atualizar 20.000 | **0,45 s** | 5,51 s | ganha |
| **excluir 20.000** | **6,27 s** | **4,73 s** | **perde** |

O manual do Cassandra(R) faz a pergunta certa sobre isso sem saber que a faz.
Lá, **exclusão é escrita**:

> *"Cassandra treats a deletion as an insertion, and inserts a time-stamped
> deletion marker called a tombstone."*
> — `managing/operating/compaction/tombstones.html`, §*What are tombstones*

E escrita, no modo padrão, **não espera disco**:

> *"In periodic mode, writes are immediately acknowledged, and the commit log is
> simply synced every 'commitlog_sync_period' milliseconds."*
> — `architecture/storage-engine.html`, §*Commit log*

A nossa exclusão física faz o contrário, e **de propósito**. O comentário de
`table.rs` acima de `excluir_de_vez` diz por quê:

> «Guardar depois de liberar teria uma janela em que a linha não existe em lugar
> nenhum — e uma queda dentro dela não tem conserto. […] Entre perder e
> duplicar, duplica.»

E o `fsync` mora dentro de `LixeiraFile::guardar` (`lixeira.rs`), com a razão
escrita ali: *«"está na lixeira" com a página ainda suja na memória não é uma
garantia»*.

**Então medi quanto custa essa garantia.**

### A medição

```bash
cargo build --release --examples -p phxsql-store   # binario velho mede o passado
cargo run --release --example custo-do-excluir -- 200000 20000
```

O exemplo entra neste commit. Ele insere 200.000 linhas com o esquema **da
bancada** (o mesmo `carga.rs`: dois índices, `Decimal(15,2)` e `Date`),
sincroniza, e cronometra **só** o laço de 20.000 exclusões físicas, com o mesmo
espalhamento `(k * 7_919) % total + 1` que a bancada usa.

| variante | corridas | mediana |
|---|---|---:|
| **como está hoje** (`fsync` por exclusão) | 6,388 s (binário do repositório) · 5,928 · 6,589 · 6,624 s (cópia) | **≈ 6,5 s** |
| **sem o `fsync` da lixeira** (uma linha editada numa cópia) | 0,827 · 0,891 · 0,831 s | **≈ 0,83 s** |

**7,8× — e o teto é esse, não a entrega.**

Para refazer a segunda linha é preciso **editar uma cópia** do repositório, como
o `CONCORRENTES.md` já fez com as três mudanças dele: em
`crates/phxsql-store/src/lixeira.rs`, dentro de `guardar`, envolver o
`self.volumes.sincronizar()?` numa condição de variável de ambiente. A variante
**não entra no motor** — ela é a medição do preço de uma garantia, não uma
proposta de tirá-la.

### Três leituras, e a terceira é a que reordena a lista

**1. O número bate com a bancada.** 20.000 exclusões numa tabela de 200.000
custam 6,4 s aqui; 20.000 exclusões numa tabela de **10 milhões** custam 6,27 s
na bancada. Cinquenta vezes mais linhas, **o mesmo tempo**. Isso é a assinatura
de um custo **por operação**, não de um custo de estrutura — e fecha a frase que
o `DESEMPENHO.md` §6 deixou aberta sobre a espera de disco.

**2. O ruído foi grande, e a condição de medição precisa estar escrita.** Uma
das corridas deu **48,782 s** — sete vezes a mediana. A máquina esteve
**disputada por outros agentes rodando bancadas ao mesmo tempo** durante esta
sessão, e o `DESEMPENHO.md` §6 já registrava que essa fase «varia demais entre
corridas». Descartei a corrida de 48,8 s como vizinhança, e o que sustenta a
conclusão **não é a mediana**: é que a **pior** corrida sem `fsync` (0,891 s)
ainda é **6,7× melhor que a melhor** corrida com ele (5,928 s). As duas
distribuições não se tocam. Mesmo assim, o Sprint 1 começa remedindo em máquina
quieta — está escrito lá.

**3. O que sobra do motor é rápido.** Sem o `fsync`, 20.000 exclusões custam
0,83 s contra os 4,73 s do MySQL(R) na mesma quantidade de trabalho. **A única
fase em que perdemos é a única em que esperamos o disco por operação.**

---

## 4. Os sprints propostos

Ordenados por **valor medível**: primeiro o desempenho do excluir, depois a
integridade do que já está em produção, depois a operação, depois a consulta,
depois a funcionalidade nova.

---

### Sprint 1 — O `fsync` da lixeira entra na janela de durabilidade

**Tamanho: P.**

#### O que entra

O `fsync` que `LixeiraFile::guardar` faz **por exclusão** passa a respeitar o
mesmo `recursos.durabilidade` que o caminho de gravação já respeita:

- `por_operacao` — **exatamente como hoje**: o disco confirma antes de o slot
  ser liberado. Nada muda para quem depende disso;
- `por_lote` (o padrão) — a exclusão entra na janela que já existe (200
  operações ou 200 ms, `config.rs`), e o `fsync` fecha a janela como já fecha
  para a inserção;
- durante um `BULKINSERT` reservado, a exclusão acompanha a carga, que já é um
  `fsync` só.

E a ordem **não muda**: guardar no `.trash` continua vindo antes de liberar o
slot. O que muda é **quem espera o disco**, não **em que ordem as coisas
acontecem**.

#### Por que agora

O Cassandra(R) **nunca** faz a thread do cliente executar o `fsync` — é uma
thread própria que sincroniza, e a escrita no máximo *espera*
(`architecture/storage-engine.html`, §*Commit log*: modo `batch` = *"won't
acknowledge writes until the commit log has been fsynced"*; modo `periodic`
padrão = *"writes are immediately acknowledged"*). E lá **exclusão é escrita**
(`tombstones.html`, §*What are tombstones*), então ela herda essa política em
vez de ter uma própria.

Aqui a exclusão tem uma política própria, mais rígida que a da inserção, e
**ninguém escolheu isso** — o `fsync` está dentro da lixeira porque a garantia
daquele arquivo depende dele, o que é certo, e acabou ficando fora da janela que
governa todo o resto.

O que a casa ganha, medido (§3): **≈ 6,5 s → ≈ 0,83 s** em 20.000 exclusões,
**7,8×** como teto. Na bancada, isso é a diferença entre **perder** para o
MySQL(R) (6,27 contra 4,73 s) e ganhar dele com folga — a última fase em que o
motor perde.

E o `DESEMPENHO.md` §4.9 já mediu o efeito irmão no caminho da inserção:
sincronizar a cada 200 **dobra** o custo por linha (16,13 contra 7,99 µs a 1
milhão), e boa parte disso é o *write-back* sendo neutralizado. A exclusão
sincroniza a cada **uma**.

#### Premissa a medir primeiro

**Duas medições, e a primeira pode matar o sprint.**

1. **Refazer a medição do §3 em máquina quieta**, três corridas de cada
   variante, sem outro agente rodando bancada. O critério combinado **antes**:
   se o ganho cair abaixo de **2×**, o sprint morre ali — o resto do trabalho
   não paga uma discussão de garantia. *(Hoje ele mede 7,8×, com o ruído
   descrito; e a corrida do binário do repositório, 6,388 s, é reproduzível com
   o comando acima.)*
2. **O que exatamente se perde numa queda dentro da janela nova.** Não é
   medição, é leitura, e tem de estar escrita antes de uma linha de código: com
   `por_lote`, uma queda **da máquina** entre o `guardar` e o `reg.excluir`
   pode deixar a linha *só* no `.reg` (nada se perde — a exclusão não
   aconteceu) ou *só* no `.trash` (a linha aparece duas vezes, que é o lado
   que o comentário do `table.rs` já escolheu: «entre perder e duplicar,
   duplica»). **Se existir um terceiro caso em que a linha some dos dois lados,
   o sprint morre inteiro** — essa é a garantia que a lixeira existe para dar.

#### Dependências

Nenhuma. A janela de durabilidade e o `BULKINSERT` já existem.

#### O que NÃO entra

- **Não** se remove o `fsync`. Ele passa a ser agrupado, e `por_operacao`
  continua com o comportamento de hoje, byte por byte.
- **Não** muda a ordem (guardar antes de liberar).
- **Não** entra o `.reason`, que tem o mesmo desenho: uma coisa de cada vez, e
  o `.trash` é o que está no caminho medido.
- **Não** vale para a exclusão **suave**, que já é uma alteração comum e já
  anda na janela.

---

### Sprint 2 — A retenção do diário no multi-master: o `gc_grace` da casa

**Tamanho: M.**

#### O que entra

Uma **regra escrita, medida e travada por teste** sobre quanto tempo um evento
de exclusão precisa sobreviver no `.log` antes que seja seguro apagá-lo —
**antes** de existir qualquer expurgo de diário. Concretamente:

- a conta de quanto o diário pode encolher sem perder um evento que uma origem
  ainda não consumiu (a `replicacao-posicoes.json` já sabe onde cada origem
  está);
- um campo de configuração que **recusa** o expurgo abaixo dessa fronteira, em
  vez de aceitar e perder;
- e o teste que **repõe o defeito**: uma origem parada além da fronteira tem de
  fazer o expurgo **recusar**, não ressuscitar linha.

#### Por que agora

O modo multi-master entrou nesta rodada com «mais recente vence»
(`REPLICACAO.md` §12), e a seção da exclusão diz:

> «Alteração mais nova que a exclusão vence (a linha reaparece re-inserida);
> exclusão mais nova vence (a linha sai).»

Esse é **exatamente** o desenho do Cassandra(R) — e é exatamente onde o manual
deles registra o acidente que dez anos de operação ensinaram:

> *"If a node remains down or disconnected for longer than `gc_grace_seconds`,
> its deleted data will be repaired back to the other nodes and reappear in the
> cluster."*
> — `managing/operating/compaction/tombstones.html`, §*Zombies*

O `gc_grace_seconds` existe **só** para isso, e o padrão deles é generoso por
medo: **864.000 segundos, dez dias** (`developing/cql/ddl.html`, §*Other table
options*: *"Time to wait before garbage collecting tombstones (deletion
markers)"*).

**A boa notícia, e ela é medida por leitura de código: hoje a casa não tem esse
buraco.** Procurei expurgo de volume em `crates/phxsql-store/src/` e **não
existe** — o `.log` nunca apaga volume; ao chegar em `max_arquivos` ele **para
com erro**, que é o comportamento seguro. O evento de exclusão vive para
sempre, então nenhuma linha ressuscita hoje.

**A má notícia é que o buraco está a um sprint de distância — de outra frente.**
O `DESEMPENHO.md` §4.7.1 já tornou o corte do diário configurável
(`recursos.diario_volume_mib`) e §4.7.3 já mediu que compactar volume fechado
pouparia 14,70% da tabela. O passo natural seguinte de **qualquer** rodada de
espaço é apagar ou compactar volume velho de diário — e no dia em que isso
acontecer, sem esta regra, o multi-master ganha o dado-zumbi **em silêncio**,
que é o pior defeito que este projeto já teve três vezes.

**Escrever a fronteira custa uma rodada agora e evita um acidente depois.** É a
mesma lógica de «mudança de formato entra cedo».

#### Premissa a medir primeiro

**A pergunta que decide: hoje, com o diário intacto, uma origem parada por muito
tempo ressuscita linha?**

Na bancada que já existe (`bancada/replicacao/montar.py` sobe os servidores),
no par bidirecional: excluir uma linha em `alfa` com `beta` derrubado, deixar
`alfa` girar volumes de diário (com `diario_volume_mib` pequeno, que é
configuração e não código novo), subir `beta` e conferir se a linha volta.

- **Se a linha voltar**, o buraco é de hoje e o sprint sobe para o primeiro
  lugar da lista — vira correção, não prevenção.
- **Se a linha não voltar** (o que a leitura de código prevê, porque não há
  expurgo), o sprint fica onde está: escrever a fronteira **antes** do expurgo
  existir, e o entregável muda de «conserto» para «regra + teste que trava».
- **Se o par nem chegar a divergir** porque a recusa por falta de chave única
  (`REPLICACAO.md` §12) já barra a tabela do teste, a montagem do teste está
  errada, não o motor.

Em qualquer um dos três casos a medição é a entrega do primeiro dia.

#### Dependências

Do **Sprint 3** para a versão completa: só dá para calcular a fronteira com
segurança sabendo onde está a origem **mais atrasada**, e é isso que o Sprint 3
guarda. Sem ele, a regra tem de ser conservadora (tempo fixo, como o
`gc_grace_seconds` do Cassandra(R), que é conservador **porque** o Cassandra(R)
também não sabe onde cada réplica está).

#### O que NÃO entra

- **Não** entra o expurgo do diário. Este sprint escreve a regra que o expurgo
  terá de obedecer; quem escreve o expurgo é a rodada de espaço.
- **Não** entra *read repair* nem árvore de Merkle — o `CASSANDRA.md` §7.1 já
  os recusou, e a razão continua valendo.
- **Não** entra conserto de relógio. O `REPLICACAO.md` §12 já diz que NTP é
  pré-requisito, e este sprint não muda isso.

---

### Sprint 3 — A posição confirmada de cada réplica, guardada no source

**Tamanho: M.**

#### O que entra

O item que o próprio `CASSANDRA.md` §6.1 propôs e que **ainda não entrou** —
conferi no código: `marcas_do_diario` (`servidor.rs`) é a *dica de posição* que
comprou os 45× do §4.5, e `replicacao_estado` reporta as **origens** que este
servidor consome, não as réplicas que consomem **dele**.

O escopo fechado:

- guardar, por `(database, tabela, réplica)`, o maior `desde` já visto —
  no **servidor**, ao lado das marcas, porque a tabela é aberta e fechada a cada
  pedido;
- responder «o evento N já chegou a quantas réplicas?» numa operação nova ou num
  campo opcional de `posicao`;
- mostrar o atraso por réplica na tela de replicação, que hoje só se descobre
  rodando a bancada.

#### Por que agora

O `desde: N` que a réplica manda **já é uma confirmação verdadeira** — ela conta
do diário dela o que aplicou, não lembra de memória (`REPLICACAO.md` §9). O
source recebe isso a cada lote e **joga fora**.

Do Cassandra(R) vem a razão de guardar, e ela é a aritmética que sustenta toda
leitura em réplica:

> *"W + R > RF, where `W` is the write consistency level, `R` is the read
> consistency level, and `RF` is the replication factor"*
> — `architecture/dynamo.html`, §*Picking consistency levels*

Não dá para copiar o quórum síncrono — o `CASSANDRA.md` §7.1 já o recusou por
topologia (a replicação daqui é **puxada**, por causa do firewall), e nada nesta
leitura muda isso. Mas a **pergunta** que o quórum responde («a minha escrita já
alcançou K cópias?») é legítima aqui, e a resposta cabe **a posteriori**, sem
que o master alcance ninguém.

Três coisas concretas que isso destrava:

1. o `CLUSTER.md` §2.1 diz que promover uma réplica «é seguro **quando as
   réplicas estão na mesma posição**, e exige conferência quando não estão» —
   hoje essa conferência é manual, e o cluster com eleição que acabou de entrar
   decide **sem** ela;
2. a fronteira do Sprint 2;
3. o atraso por réplica, visível sem bancada.

#### Premissa a medir primeiro

`python3 bancada/replicacao/medir.py 100000`, com duas asserções, e a segunda
mata o sprint:

1. **Correção.** No fim da carga, a posição que o source guarda para cada
   réplica bate com a que a própria réplica informa, **dentro de um lote** (500
   eventos, o `max` do `replicar`). Diferença maior que 500 significa que a
   confirmação está mentindo — e uma confirmação que mente é pior que nenhuma.
2. **Custo.** A taxa do master **não pode se mexer**: 34.048 linhas/s
   (`DESEMPENHO.md` §4.5), dentro da variação que a bancada já tem. Queda acima
   de 1% significa que guardar um inteiro está custando mais do que deveria — e
   aí o lugar está errado (provavelmente dentro da trava global, e não do lado
   do pedido). **O sprint morre e volta com outro desenho.**

#### Dependências

Nenhuma para entrar. O *long-poll* com `Condvar` (`CASSANDRA.md` §6.2) o torna
**mais útil** — uma posição com 2 s de idade não serve de confirmação — mas não
é pré-requisito para guardá-la.

#### O que NÃO entra

- **Não** entra quórum de escrita síncrono, nem o master empurrando. As cinco
  razões estão no `CASSANDRA.md` §7.1 e continuam de pé.
- **Não** entra *hint* com prazo e despacho de fundo — é um subsistema, e a
  réplica daqui alcança sozinha de qualquer posição, que é justamente o que
  torna *hints* desnecessárias.
- **Não** entra o *long-poll*. É outro sprint, e já está escrito no
  `REPLICACAO.md` §9.

---

### Sprint 4 — Duas condições no WHERE: a interseção de rowids

**Tamanho: M.**

#### O que entra

O caso mais simples e mais pedido, e só ele: **duas igualdades ligadas por
`AND`**, cada uma com índice de coluna única. Busca as duas listas de rowids,
**interseca**, lê as linhas. Sem modelo de custo, sem planejador geral: começa
pela lista **menor**, que é uma decisão que se mede em vez de se estimar.

#### Por que agora

Hoje a camada SQL **recusa por escrito** — `crates/phxsql-sql/src/sintaxe.rs`:

> «o WHERE aceita UMA comparacao. Duas exigiriam interseccao de rowids, e nao ha
> planejador que decida por qual indice comecar»

Essa recusa é honesta (não finge que faz), e é exatamente a limitação que o
Cassandra(R) teve com o índice secundário legado — e que o SAI resolveu:

> *"unlike legacy secondary indexes, where at most one column index will be used
> per query, SAI implements a Query Plan that makes it possible to use all
> available column indexes in a single query"*
> — `developing/cql/indexing/sai/sai-concepts.html`, §*SAI*

Vale registrar que a **outra** metade do SAI **não** cabe aqui, e a fonte diz
por quê: ele *"attaches the indexing information to the SSTables"*
(`developing/cql/indexing/indexing-concepts.html`) e *"flushes Memtable index
contents directly to disk"* (`sai-concepts.html`) — ou seja, o SAI é índice
preso a arquivo imutável, e o `.ndx` é uma B+tree atualizada no lugar. **A
arquitetura não se copia; a capacidade da consulta, sim.**

O que a casa ganha: o ODBC acabou de entrar (`docs/ODBC.md`), e a primeira
consulta que qualquer ferramenta de terceiro manda tem dois filtros. Hoje ela
recebe um erro.

#### Premissa a medir primeiro

**A interseção ganha de ler-e-filtrar? E a partir de qual seletividade?**

Esta é a lição do pedido 113 aplicada antes da hora: a premissa do item é que
usar dois índices é melhor que usar o mais seletivo e filtrar o resto lendo.
**Isso pode ser falso**, e há caso em que certamente é: se um índice devolve 10
rowids e o outro 500.000, ler as 10 linhas e conferir a segunda coluna na mão é
mais barato que intersecar meio milhão de rowids.

Um exemplo novo, na forma do `--example adiar-vale-quando`, medindo o **ponto de
virada** por seletividade — como o `M ≈ N/3` daquele. O critério antes de medir:

- se a interseção **nunca** ganhar por mais de 1,2× na faixa realista, o sprint
  encolhe para «escolher o índice mais seletivo e filtrar o resto», que é muito
  menos código e entrega a mesma consulta ao usuário;
- se ganhar muito na faixa de duas colunas igualmente seletivas (o caso
  `cidade = X AND cadastro = Y`), entra como está proposto.

Em qualquer caso o usuário deixa de receber erro — **o que muda é o que roda por
baixo**, e é isso que a medição decide.

#### Dependências

Nenhuma. `Table::buscar` já devolve `Vec<RowId>` ordenado por chave, e a camada
SQL já tem onde recusar — é lá que a decisão passa a caber.

#### O que NÃO entra

- **Não** entra `OR` (união com desduplicação é outro problema).
- **Não** entra `BETWEEN`/faixa — a `sintaxe.rs` recusa por outro motivo («a
  faixa ainda não está exposta no protocolo»), e é outro sprint.
- **Não** entra planejador com estatística de coluna. Começar pela lista menor é
  uma regra, não um modelo de custo.
- **Não** entra `ALLOW FILTERING`. O Cassandra(R) o tem e avisa contra ele —
  *"the performance of the query can be unpredictable"* (`dml.html`,
  §*Allow filtering*) —, e a recusa honesta da casa é melhor que uma varredura
  que finge ser consulta.

---

### Sprint 5 — TTL por linha, como coluna do motor e job de varredura

**Tamanho: M.**

#### O que entra

Uma coluna de sistema opcional de expiração (na forma do `SOFTDELETED` e do
`rownum`, que já entraram assim), preenchida na inclusão a partir de um
`default_ttl` da tabela ou de um valor por linha, mais um **job** — os jobs já
existem, com agendador e aviso por e-mail (`docs/JOBS.md`) — que varre e aplica
a exclusão **suave** nas linhas vencidas.

#### Por que agora

O Cassandra(R) tem TTL nos dois níveis, e o manual é preciso sobre a semântica:

> *"specifies an optional Time To Live (in seconds) for the inserted values. If
> set, the inserted values are automatically removed from the database after the
> specified time"*
> — `developing/cql/dml.html`, §*Update parameters*

E o detalhe que a maioria das implementações erra:

> *"the TTL concerns the inserted values, not the columns themselves. This means
> that any subsequent update of the column will also reset the TTL"*
> — mesma seção

Mais o padrão por tabela, `default_time_to_live` (`developing/cql/ddl.html`,
§*Other table options*), e o que acontece no vencimento: *"Cassandra marks the
object with a tombstone, and handles it like other tombstoned objects"*
(`tombstones.html`, §*What are tombstones*).

Procurei TTL de linha no repositório e **não existe** — só expiração de sessão
(`http.rs`) e de reserva de carga. É funcionalidade genuinamente ausente, e a
casa tem **todas** as peças: coluna de sistema no fim do esquema (o padrão do
`SOFTDELETED`, esquema `PSCH` v4), exclusão suave com motivo, `.reason`,
agendador de jobs e a lixeira.

**E há uma decisão de projeto que o Cassandra(R) ajuda a tomar sem hesitar:** o
vencimento aplica exclusão **suave**, não física. Lá a linha vencida vira
tombstone e some na compactação; aqui a exclusão física passa pelo `.trash` e
sincroniza (§3), e uma varredura que exclui de vez 100.000 linhas vencidas de
madrugada seria a fase mais cara do motor rodando sozinha. Suave é reversível,
barata e já tem tela.

#### Premissa a medir primeiro

**Duas, e a primeira não é técnica.**

1. **Alguém pede isso?** TTL é a funcionalidade mais fácil de escrever e mais
   fácil de nunca ser usada num ERP — em log, sessão e telemetria ela é o
   assunto; em nota fiscal e cadastro, dado que some sozinho é **defeito**, não
   recurso. **Esta premissa é sua, não minha**, e é a que decide o sprint. Se a
   resposta for «não tenho esse caso», o sprint morre aqui e o documento fez o
   trabalho dele.
2. **Quanto custa a varredura**, se a resposta for sim. Uma varredura por
   coluna sem índice é O(N) por rodada; com índice sobre a coluna de expiração
   é uma faixa. Medir com o `--example custo-da-pagina` na forma da faixa, numa
   tabela de 1 milhão, antes de decidir se a coluna nasce indexada — porque
   índice a mais é custo em **toda** inserção, e o `DESEMPENHO.md` §2 já mede
   quanto (o segundo índice são 34% do tempo de uma inserção).

#### Dependências

Nenhuma técnica. Jobs, exclusão suave e coluna de sistema já existem.

#### O que NÃO entra

- **Não** entra TTL por **coluna** (só por linha). Lá o TTL é do *valor*, o que
  faz sentido num motor de células com carimbo; aqui a linha é um slot de
  largura fixa, e «coluna que vence» viraria nulo — que é outra coisa, e uma
  mentira sobre o dado.
- **Não** entra reset de TTL na alteração, que é a semântica deles. Aqui é uma
  decisão a tomar com você, e o padrão seguro é **não** resetar.
- **Não** entra exclusão física automática.

---

## 5. O que eu descartei, e por quê

Tão importante quanto o que propus. Cada linha traz a fonte e, quando existe, o
número da casa.

### 5.1 Counters

**Fora.** O manual lista as restrições que pagam a distribuição:

> *"A table that contains a counter can only contain counters"* · *"Counter
> updates are, by nature, not idemptotent. An important consequence is that if a
> counter update fails unexpectedly […] the client has no way to know if the
> update has been applied or not"* · *"The deletion of counters is supported, but
> is only guaranteed to work the first time you delete a counter"*
> — `developing/cql/types.html`, §*Counters*

Um contador que não sabe se foi aplicado e cuja exclusão só funciona uma vez é
**pior** que o que a casa já tem: `Sequence` no cabeçalho do `.reg` com
`ajustar_sequencia` (pedido 81), que é exato porque é um número só num lugar só.
Adotar counters seria comprar o preço da distribuição sem a distribuição.

### 5.2 Lightweight transactions (`IF NOT EXISTS`, Paxos)

**Fora, e a casa já ganha esta.** O próprio manual desaconselha:

> *"using IF NOT EXISTS will incur a non-negligible performance cost, because
> Paxos is used, so this should be used sparingly"*
> — `developing/cql/dml.html`, §*Insert statement* (e igual em *Update* e
> *Delete*)

A janela de conflito daqui (pedido 123) resolve o mesmo problema **melhor**: um
contador de versão **por registro**, sem relógio, que **recusa** com o erro 3004
`CONFLITO` e mostra as três colunas para quem decide, em vez de descartar em
silêncio. E a conferência de unicidade acontece **antes de qualquer gravação**
(`table.rs`), o que o Cassandra(R) não sabe fazer — o `CASSANDRA.md` §2.3 já
mostrou que lá o INSERT não sabe recusar chave repetida.

### 5.3 Batches (logged/unlogged) contra o nosso `BULKINSERT`

**Fora — já respondido, e a fonte confirma a resposta.** O manual:

> *"Batches are not a full analogue for SQL transactions."* · *"There is a
> performance penalty for batch atomicity when a batch spans multiple
> partitions. […] If the UNLOGGED option is used, a failed batch might leave the
> batch only partly applied."*
> — `developing/cql/dml.html`, §*Batch statement*

O `docs/SQL.md` já diz a mesma frase sobre o `BULKINSERT` («**não é transação**:
ele reserva a tabela, não desfaz nada»), e o `inserir_lote` com `parar_no_erro`
já documenta que deixa gravadas as linhas anteriores à falha. **Duas casas, a
mesma honestidade, nada a importar.** O ganho do lote já está medido: 1,53× com
a reserva (`DESEMPENHO.md` §6).

### 5.4 Materialized views

**Fora, e desta vez pelo aviso da própria fonte.**

> *"we advise against doing deletions on base columns not selected in views
> until this is fixed"* (CASSANDRA-13826)
> — `developing/cql/mvs.html`, §*Limitations*

Mais as restrições de criação: *"it must contain all the primary key columns of
the base table"* e *"it can only contain a single column that is not a primary
key column in the base table"*. Uma funcionalidade que o manual do próprio
projeto pede para usar com ressalva, num motor onde a alternativa (uma tabela
mantida por **gatilho**, que acabou de entrar) faz o mesmo com o custo à vista,
não entra.

### 5.5 Busca vetorial

**Fora agora, e registrado para não se procurar de novo.** Existe e é recente —
`vector<float, n>` (`developing/cql/types.html`, §*Vectors*: *"A fixed length
non-null, flattened array of float values"*), índice `USING 'sai' WITH OPTIONS =
{ 'similarity_function': … }` e consulta `ORDER BY … ANN OF … LIMIT`
(`getting-started/vector-search-quickstart.html`). É bem desenhado.

Não entra porque **não há caso de uso na casa**: o motor serve ERP, e um índice
ANN é um projeto próprio (grafo em disco, ou varredura com distância), não um
ajuste. Se um dia entrar, o degrau anterior é o Sprint 4 — sem consulta que
combine índices, um índice vetorial não teria como participar de um filtro.

### 5.6 Collections (list/set/map) e UDT

**Fora**, e o motivo é do manual deles:

> *"Collections are meant for storing/denormalizing relatively small amount of
> data"* · *"Individual collections are not indexed internally. Which means that
> even to access a single element of a collection, the whole collection has to
> be read"* · sobre listas: *"These operations […] incur an internal
> read-before-write"* e *"are not idempotent by nature"*
> — `developing/cql/types.html`, §*Collections*, §*Lists*

Ler a coleção inteira para tocar um elemento é o oposto do que o slot de largura
fixa compra. O `.memo` e o `.bin` já guardam o que não cabe inline, e a
modelagem de ERP tem tabela filha para isso. **A lição aproveitável é o aviso**,
não o tipo: quando alguém propuser uma coluna JSON aqui, este parágrafo é a
resposta.

### 5.7 Compaction, e o espaço em disco

**Fora, e já medido duas vezes.** A compactação existe lá para fundir SSTables e
recolher tombstone; aqui ela esbarra na regra que define o projeto — o `.reg`
nunca reaproveita slot, e a ordem de digitação é sagrada. O `CONCORRENTES.md`
§6.4 já fechou essa porta contra o InnoDB e o Aria.

E sobre o espaço, que é o que dói (2,43 GiB contra 0,88 GiB do MySQL(R) na
bancada): o `DESEMPENHO.md` §4.7.3 já mediu onde ele está, e **não é nos
diários** — o `.ndx` sozinho comprime 8,26× e pouparia 2,1× mais que os três
diários juntos. Nada a acrescentar por esta leitura.

### 5.8 Paginação por token

**Fora — a casa já ganha, e por muito.** O CQL não tem `OFFSET`: só `LIMIT` e
`PER PARTITION LIMIT` (`developing/cql/dml.html`, §*Limit clause*), e a
paginação é por cursor de chave. Aqui existem **as duas**: cursor (pedido 102) e
salto por posição, que sai de uma bissecção porque a ordem lógica é a física —
**164 µs contra 246 ms, 1.500×** numa página no meio de 800 mil linhas
(`DESEMPENHO.md` §6). O que o Cassandra(R) não pode ter, nós temos de graça.

### 5.9 LSM, memtable de escrita, commit log como fonte de replicação

**Fora**, e não repito o argumento: `DESEMPENHO.md` §5 e `CASSANDRA.md` §5 e
§7.2 já o fizeram com o fonte na mão. Registro só a conferência que esta leitura
do manual acrescenta, porque ela **confirma** o veredito por outro caminho: o
manual diz que *"SSTables are immutable, and never written to again after the
memtable is flushed"* (`architecture/storage-engine.html`, §*SSTables*) — e é
essa imutabilidade que obriga a compactação, que reordena, que quebra a ordem de
digitação. **A cadeia inteira sai de uma decisão que a casa não tomou.**

### 5.10 Snitch, gossip e vnodes

**Fora.** O detector deles é *"a variant of the Phi Accrual Failure Detector"* e
o gossip roda *"every second, every node"* (`architecture/dynamo.html`,
§*Gossip*, §*Ring membership and failure detection*); as réplicas são escolhidas
por rack via Snitch (§*Network topology strategy*). Isso serve a um anel de
iguais com *consistent hashing* — e o arranjo daqui é um master com réplicas de
leitura, ou um par bidirecional, com o cluster e a eleição que **acabaram de
entrar** resolvendo a promoção. Copiar gossip seria trazer um subsistema para
uma topologia que não o pede.

---

## 6. Tabela-resumo

| # | Sprint | Tam. | Premissa a medir primeiro | Depende de |
|---:|---|:---:|---|---|
| 1 | O `fsync` da lixeira entra na janela de durabilidade | **P** | refazer a medida em máquina quieta; **abaixo de 2× o sprint morre**. E provar que não existe caso em que a linha some dos dois lados | — |
| 2 | A retenção do diário no multi-master (o `gc_grace` da casa) | **M** | com o diário girando volumes, uma origem parada ressuscita linha? Se sim, vira correção e sobe; se não, é prevenção e fica | Sprint 3 (para a versão completa) |
| 3 | A posição confirmada de cada réplica, no source | **M** | a posição guardada bate com a informada dentro de um lote (500); e a taxa do master **não cai 1%** (34.048 linhas/s) | — |
| 4 | Duas condições no WHERE: interseção de rowids | **M** | o ponto de virada por seletividade; se a interseção nunca ganhar 1,2×, o sprint encolhe para «índice mais seletivo + filtro» | — |
| 5 | TTL por linha, como coluna e job | **M** | **você tem esse caso de uso?** Depois: o custo da varredura, e se a coluna nasce indexada | — |

**Ordem sugerida: 1, 3, 2, 4, 5.** O Sprint 1 é o de maior ganho medido por
menor custo e fecha a última fase em que o motor perde na bancada. O 3 vem antes
do 2 porque é dependência dele e entrega valor sozinho. O 4 é o que mais muda a
vida de quem liga uma ferramenta pelo ODBC. O 5 depende de uma resposta sua,
não de uma medição minha.

---

## 7. A execução aguarda aprovação

**Nenhum destes cinco sprints começa sem o seu sim, e o sim é sprint a sprint.**
Cada um tem uma premissa que pode matá-lo antes da primeira linha de código — e,
quando a premissa mata o sprint, **a medição é a entrega**: mais um diagnóstico
plausível derrubado com número, que é o que este projeto faz melhor. O
`DESEMPENHO.md` já tem sete deles.

---

## Nota sobre os nomes

Apache Cassandra(R) é marca da Apache Software Foundation. MySQL(R) e InnoDB são
marcas da Oracle Corporation. MariaDB(R) e Aria são marcas da MariaDB
Corporation Ab. PostgreSQL(R) é marca da PostgreSQL Community Association of
Canada. Este documento lê a **documentação pública** do Apache Cassandra(R) 5.0
para entender decisões de projeto; nenhum código foi copiado, e tudo o que se
propõe aqui é reimplementação de ideia documentada, escrita do zero e só com a
`std` do Rust — **nenhum sprint acima pede uma crate**.

Duas afirmações da folha de marca da casa continuam **falsas** e não aparecem
neste documento: *ACID compliant* e *built-in replication*. Não há transação, e
a replicação é uma funcionalidade construída, não uma propriedade herdada.

---

## Como refazer o que este documento mediu

```bash
# o binario velho mede o passado -- isto vem sempre antes
cargo build --release --examples -p phxsql-store

# o custo do excluir, como esta hoje (mediana ~6,5 s; a corrida do repositorio deu 6,388 s)
cargo run --release --example custo-do-excluir -- 200000 20000

# a bancada de onde saem os numeros de comparacao
python3 bancada/medir.py 10000000

# a bancada da replicacao, onde a premissa dos sprints 2 e 3 se mede
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```

Para a variante **sem** o `fsync` da lixeira (a segunda linha da tabela do §3),
copie o repositório e, em `crates/phxsql-store/src/lixeira.rs`, dentro de
`guardar`, condicione o `self.volumes.sincronizar()?` a uma variável de
ambiente. **A variante não entra no motor**: ela mede o preço de uma garantia,
não propõe tirá-la.
