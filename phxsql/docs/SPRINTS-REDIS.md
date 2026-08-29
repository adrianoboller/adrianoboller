# O manual do Redis(R), lido contra a casa — sprints para aprovação

> **Este documento continua sendo a fonte, mas não é mais a lista.** As
> quatro propostas daqui foram para a lista única de `docs/SPRINTS.md`,
> junto com as outras 27. Duas notas da travessia: o **Sprint 2 teve a
> premissa derrubada por leitura de código** — o job com
> `UPDATE … SET SOFTDELETED` que ele usa como alternativa barata não
> existe, porque a camada SQL recusa `UPDATE` pelo nome
> (`sintaxe.rs:269`) e o corpo de rotina também (`SPRINTS.md` §6, item
> 18); e o **Sprint 3 aparece lá com o rótulo certo**, que é contrato e
> cliente, não capacidade nova (§4.2).

Documento de **proposta**, no contrato do `CONCORRENTES.md` e do
`CASSANDRA.md`: toda afirmação sobre o Redis(R) traz a fonte (URL e seção);
todo número da casa sai da bancada, do `DESEMPENHO.md` ou de uma medição
feita nesta análise, com a condição de medição dita e o roteiro para refazer.
Toda proposta vem com **a premissa a medir primeiro** — a medição que pode
matar o sprint, e matar é resultado tão válido quanto aprovar.

> **Nada daqui executa sem o sim do Adriano, sprint a sprint.** Este documento
> é a lista de candidatos com os números na mesa; a decisão é dele.

Uma honestidade de partida: **o Redis(R) não tem SQL.** O «manual do Redis
SQL» pedido é a documentação oficial de comandos, tipos e arquitetura
(redis.io/docs e redis.io/commands) — e é ela que foi lida. A própria página
de tipos abre dizendo o que ele é: *«Redis is a data structure server»*
([redis.io/docs/latest/develop/data-types/](https://redis.io/docs/latest/develop/data-types/),
introdução). Um servidor de estruturas de dados chave-valor, sem esquema, sem
junção, sem transação com rollback. A análise abaixo o trata como o que ele é.

---

## 1. O que foi lido

| Página | URL | Seções usadas |
|---|---|---|
| Persistência (RDB e AOF) | redis.io/docs/latest/operate/oss_and_stack/management/persistence/ | *RDB advantages/disadvantages*, *AOF advantages/disadvantages*, *Snapshotting → How it works*, *Log rewriting*, *How durable is the append only file?* |
| Pipelining | redis.io/docs/latest/develop/using-commands/pipelining/ | *Request/Response protocols and RTT*, *Redis Pipelining*, *It's not just a matter of RTT*, *A real world code example*, *Pipelining vs Scripting*, *Appendix: Why are busy loops slow…* |
| Transações | redis.io/docs/latest/develop/using-commands/transactions/ | *Usage*, *Errors inside a transaction*, *What about rollbacks?*, *Optimistic locking using check-and-set* |
| Expiração | redis.io/docs/latest/commands/expire/ | *Appendix: Redis expires* (*Expires and persistence*, *How Redis expires keys*, *How expires are handled in the replication link and AOF file*) |
| Notificações de keyspace | redis.io/docs/latest/develop/pubsub/keyspace-notifications/ | introdução (*fire and forget*), *Type of events*, *Configuration*, *Timing of expired events* |
| Eviction | redis.io/docs/latest/develop/reference/eviction/ | *Using the maxmemory configuration directive*, *Eviction policies*, *Approximated LRU algorithm*, *LFU eviction* |
| Replicação | redis.io/docs/latest/operate/oss_and_stack/management/replication/ | *How Redis replication works*, *Replication ID explained*, *Allow writes only with N attached replicas*, *How Redis replication deals with expires on keys* |
| Cluster (especificação) | redis.io/docs/latest/operate/oss_and_stack/reference/cluster-spec/ | *Write safety*, *Key distribution model*, *MOVED Redirection*, *Live reconfiguration*, *ASK redirection* |
| Tipos de dados | redis.io/docs/latest/develop/data-types/ | *Data types* (visão geral) |

Do nosso lado, e nesta ordem: `CLAUDE.md` inteiro, `docs/PENDENCIAS.md`,
`docs/DESEMPENHO.md` inteiro, `docs/CONCORRENTES.md`, `docs/CASSANDRA.md`,
`bancada/LEIA-ME.md`, `bancada/resultados.json`, e — já **depois** de as dez
frentes entrarem — `docs/TRIGGERS.md`, `docs/REPLICACAO.md`, `docs/CLUSTER.md`
e os trechos de `crates/phxsql-server/src/servidor.rs` (a `Janela` de
durabilidade, o laço JSON Lines, `op_memoria_carregar`), `config.rs`
(`Recursos`) e `crates/phxsql-store/src/table.rs` (o caminho do `SOFTDELETED`)
que cada candidato toca.

Os números da casa usados como base, todos medidos e registrados:

| | |
|---|---|
| Bancada de 10 milhões (`bancada/resultados.json`) | inserir **91,5 s contra 112,4 s** do MySQL(R) — o insert hoje ganha; buscar 0,20 contra 2,64; varrer 1,41 contra 15,70; atualizar 0,45 contra 5,51; **excluir ainda perde: 6,27 contra 4,73 s** |
| `sincronizar` a cada 200 no caminho da operação | **16,13 → 7,99 µs/linha sem ele** (`DESEMPENHO.md` §4.9, `--example custo-do-fsync`) |
| Modos de durabilidade | `por_operacao` → `por_lote` → `sistema`: 1.289 → 24.858 → 26.301 linhas/s, **20,4×** (§3, item 4) |
| Carga pela rede, linha a linha contra lote | 2.659 → 43.302 linhas/s, **16,3×** (§6) |
| Cache de páginas do `.ndx` | **2,40×**, teto de 2.048 páginas no joelho medido (§2.1) |
| Compactação dos diários | medida e **recusada duas vezes**: 14,7% no melhor corte contra 2,1× que o `.ndx` daria (§4.7.3) |
| `TabelaMemoria`/`SelectMemory` | **87×** o disco (pedido 31) |

---

## 2. O que as dez frentes já cobriram — conferido contra o código

Esta análise começou antes das dez frentes entrarem na branch principal e foi
fechada depois. Cada candidato foi **reconferido contra o código de lá**, e não
contra a lembrança — que é a regra que já tirou a chave estrangeira de
«pronto» para «parcial».

| Candidato desta análise | Estado na branch principal | O que sobra |
|---|---|---|
| Teto de memória (`maxmemory`) | **COBERTO** (`fe9cb30`): `recursos.memoria_max_mb` ganhou leitor de verdade em `op_memoria_carregar`, com recusa nomeada, zero = sem teto, e a conta de bytes (`m.bytes()`) que era a minha premissa | só o degrau seguinte, e ele depende do TTL — §3.4 |
| Gatilhos e procedimentos (o análogo do scripting Lua) | **COBERTO** (`367d6ca`, `df04628`): linguagem decidida — a do MySQL(R)/MariaDB(R), **um interpretador só** (`phxsql-sql/src/rotina.rs`) | nada a propor; muda o escopo do pipelining — §3.3 |
| Quatro modos de replicação, streaming e agendado | **COBERTO** (`9a24412`, `378c0f7`, `1a366e4`) | insumo entregue à frente — §3.12 |
| Cluster com eleição e promoção | **COBERTO** (`adace51`): inclusive o `REDIRECIONA` (erro 4003), que é o análogo do `MOVED` | muda a nota dos slots — §3.10 |
| Jobs com aviso por e-mail | **COBERTO** (`80f0265`) | vira a alternativa a bater no TTL — §3.2 |
| Firewall/blacklist e mensagens multilíngue | **COBERTO** (`1a4b180`, `d1f97af`) | fora do alcance desta análise |
| **fsync fora do caminho da operação** | **não coberto** | Sprint 1 |
| **TTL por linha** | **não coberto** (nenhuma ocorrência de expiração por linha no código) | Sprint 2 |
| **Pipelining** | **não coberto** (e já funciona sem mudar o servidor — §3.3) | Sprint 3 |
| **Pub/sub para a tela** | **não coberto** (o único `assinar` do servidor é assinatura Ed25519 de desafio) | Sprint 4 |

Uma correção que essa releitura obrigou, e que vale mais que a tabela: **eu
tinha escrito que faltava o fsync por relógio, e o relógio já existe.** A
`Janela` fecha por **quantidade ou por tempo, o que vier primeiro**
(`lote_operacoes` = 200, `lote_milissegundos` = 200 ms), e o comentário no
código explica por quê: só por quantidade, um servidor parado deixaria a
última gravação pendurada. O que falta é outra coisa, e o §3.1 foi reescrito
para dizer exatamente qual.

### A medição feita nesta análise, e em que condição

Pipelining pelo soquete, `phxsqld` recém-compilado
(`cargo build --release -p phxsql-server`) na porta de medição **5362**, base
em `/tmp`, sem tocar no demo (5199/5599) e **sem `pkill`** — o processo é
encerrado pelo PID que o próprio script subiu, porque a máquina tem `phxsqld`
de outros agentes rodando. Mesmo trabalho nas duas formas: 2.000 buscas
pontuais por índice único (`buscar`), duas corridas por forma, a primeira de
cada forma descartada como aquecimento. A forma A escreve um pedido e espera a
resposta; a B escreve o lote e então lê as respostas.

**Condição: máquina disputada** — `load average` 4,00 na primeira rodada e
4,17 na segunda, com três `phxsqld` de outras bancadas no ar. As duas rodadas
estão publicadas justamente por isso:

| forma | rodada 1 (load 4,00) | rodada 2 (load 4,17) |
|---|---:|---:|
| linha a linha | 174 · 178 µs | **193 · 202 µs** |
| empilhado, lotes de 100 | 120 · 122 µs | 125 · 123 µs |
| empilhado, lotes de 500 | 124 · 126 µs | 125 · 124 µs |
| empilhado, lote único de 2.000 | 139 · 146 µs | 126 · 119 µs |

Três conclusões, e a primeira vale mais que o número:

1. **O protocolo JSON Lines já atende pedidos empilhados hoje, sem mudar uma
   linha do servidor.** As 2.000 respostas voltaram corretas e na ordem, nas
   quatro formas, nas duas rodadas. Isso é um fato sobre o código, não uma
   medida — e não depende de a máquina estar quieta.
2. **O lado empilhado é estável (119–126 µs nas duas rodadas) e o linha a
   linha não é (174–202 µs).** Faz sentido, e o próprio manual do Redis(R)
   explica: o laço síncrono paga uma ida ao escalonador por operação
   (*Appendix: Why are busy loops slow even on the loopback interface?*), e é
   exatamente isso que uma máquina ocupada encarece. O ganho medido fica entre
   **1,45× e 1,58×** — e a dispersão está **do lado que favorece a proposta**,
   o que é motivo para desconfiar dela, não para comemorar.
3. **O número publicável exige máquina quieta**, e isso é resultado honesto,
   não desculpa: o que se pode afirmar hoje é «entre 1,45× e 1,58× no
   loopback, máquina com load ~4». E o loopback é onde o pipelining rende
   **menos** — o RTT é quase zero (*Request/Response protocols and RTT*). O
   ganho com RTT real é **inferência**, não medida.

---

## 3. Candidato a candidato

### 3.1 O `everysec` do AOF: quem executa o fsync — **SPRINT 1**

**O que é.** O AOF tem três políticas de `fsync`
([persistence](https://redis.io/docs/latest/operate/oss_and_stack/management/persistence/),
*How durable is the append only file?*): `always` («very very slow, very
safe»), `everysec` (**o padrão**) e `no`. A frase que interessa é a do
`everysec`, e ela não é sobre o relógio — é sobre **quem trabalha**:
*«fsync is performed using a background thread and the main thread will try
hard to perform writes when no fsync is in progress, so you can only lose one
second worth of writes»* (*AOF advantages*).

**O que resolveria aqui — e o que já existe.** Dois dos três modos já existem
com outros nomes: `always` é o `por_operacao`, `no` é o `sistema`. **E o
gatilho por relógio também já existe**: a `Janela` do servidor fecha por
quantidade *ou* por tempo, o que vier primeiro (`lote_operacoes` = 200,
`lote_milissegundos` = 200 ms). O que **não** existe é a outra metade da frase
do manual: aqui o `fsync` roda **dentro da operação que fechou a janela** — a
200ª gravação paga o `fsync` das 200. É a diferença entre «sincronizar a cada
N ms» e «nunca sincronizar no caminho de quem escreve».

O número da casa já está medido e é grande: `DESEMPENHO.md` §4.9 mostrou que o
`sincronizar` a cada 200 **dobra o custo por linha** (16,13 → 7,99 µs a 1M;
16,92 → 8,05 a 3M), e boa parte disso nem é o `fsync` em si (~4 µs por linha
amortizado): é o write-back sendo neutralizado, porque descarregar as páginas
sujas antes de a folha encher faz o CRC ser pago por poucas chaves em vez de
por centenas. O §4.9 termina dizendo que tirar o `fsync` do caminho «é decisão
de garantia, não de código» — o `everysec` é a forma madura dessa decisão, com
o contrato escrito. O `CASSANDRA.md` §6.3 chegou ao mesmo item pelo modo
`periodic` do Cassandra(R) — **candidato compartilhado com a análise do
Cassandra(R)**.

**Premissa a medir primeiro.** Três, e qualquer uma pode matar:

1. **O solavanco.** A thread de fundo precisa da trava global para sincronizar,
   e um `fsync` custa ~0,8 ms. Medir o **p99** da latência de operação com a
   thread ligada contra o de hoje: se a média melhorar 2× e o p99 dobrar, o
   sprint morre — cliente não sente média.
2. **Quanto sobrevive de ponta a ponta.** Rodar a bancada com o modo ligado; o
   §4.6 registra 6,6 µs por linha ainda sem explicação nesse caminho.
3. **Em máquina quieta.** As duas acima não valem nada com `load` 4 — e essa é
   uma condição de execução do sprint, não um detalhe.

Critério de morte, combinado antes: ganho de ponta a ponta abaixo de 2%, ou
p99 piorando mais do que o ganho médio compra.

**Custo/risco.** Uma thread, um modo novo de `Durabilidade`, o conjunto de
tabelas sujas que o servidor já mantém, e o MANUAL dizendo a promessa de cada
modo (o que o `CASSANDRA.md` §6.5 já propunha). O risco é de contrato, não de
código — e por isso o modo entra **pedido, não imposto**: o padrão continua
`por_lote`; quem quiser a janela escreve o modo novo no `config.json`. Guarda
nova entra pedida — e afrouxar guarda, com mais razão ainda.

### 3.2 TTL por linha (`EXPIRE`) → o `SOFTDELETED` — **SPRINT 2**

**O que é.** `EXPIRE chave segundos` põe prazo numa chave. Três decisões de
desenho interessam ([expire](https://redis.io/docs/latest/commands/expire/),
*Appendix: Redis expires*): o prazo é guardado como **timestamp absoluto**,
não duração (*Expires and persistence* — «the time is flowing even when the
Redis instance is not active»); a expiração é **preguiçosa mais ativa** (*How
Redis expires keys*: quem acessa uma chave vencida a expira, e um ciclo de
fundo pega as que ninguém acessa); e na replicação **só o master expira**
(*How expires are handled…*: «when a key expires, a `DEL` operation is
synthesized in both the AOF file and gains all the attached replicas»),
porque relógio de réplica divergiria.

**O que resolveria aqui.** Sessões, tokens, filas de trabalho, staging de
importação e cache de integração. E a casa tem **o lugar exato onde a
expiração pousa sem quebrar nada**: o `SOFTDELETED` (pedido 97). Expirar **não
é excluir** — é o motor marcar a linha sozinho quando `agora > prazo`. A linha
some das listas, fica inteira no `.reg` (a ordem de digitação não se toca, o
slot não se reaproveita), `restaurar` desfaz, e o expurgo físico continua
sendo decisão humana com `.trash` e `.reason`. As três lições traduzem direto:
coluna `DateTime` absoluta; leitura que **filtra** linha vencida sem gravar
nada (gravar na leitura poria escrita em todo `ler` sob a trava global) mais
um varredor de fundo que marca — e ele varre por `rownum`, incremental,
porque aqui a ordem física é a lógica e não precisa da amostragem aleatória
que o Redis(R) usa; e **a expiração acontece só na origem**, viajando às
réplicas como evento comum de marcação no `.log`.

**Premissa a medir primeiro — e ela mudou com as dez frentes.** A premissa
original (custo zero para quem não usa TTL) continua válida e se mede no
`varrer` da bancada. Mas entrou uma premissa **maior**, e ela pode matar o
sprint inteiro: **jobs (`80f0265`) e procedimentos (`367d6ca`) agora
existem** — dá para expirar linha hoje, sem uma linha de motor, com um job
horário rodando um `UPDATE … SET SOFTDELETED = 1 WHERE prazo < NOW()`. Então
a pergunta deixou de ser «quanto custa implementar» e virou **«o que o motor
faz que o job não faz?»**. Três respostas candidatas, e cada uma é medível: a
granularidade (o job acorda de minuto em minuto; a leitura filtrada esconde a
linha vencida **no mesmo segundo**), o custo (o job varre a tabela inteira a
cada rodada; o filtro na leitura custa uma comparação por linha já lida) e a
correção sob replicação (o job roda em quem? se rodar nos dois lados, dois
relógios decidem — exatamente o que o Redis(R) evita centralizando no master).
**Medir a alternativa barata antes de construir a cara** é a regra do pedido
113 aplicada a um item meu. Se o job der conta com custo parecido, o sprint
morre e vira uma página de documentação — resultado válido.

**Custo/risco.** Formato (`PSCH` v7, com a disciplina do v6: quem lê versão
velha para antes), o varredor no relógio de 30 s dos jobs, a tela de Nova
tabela. Risco maior: interação com a janela de conflito (linha marcada pelo
varredor entre o `ler` e o `atualizar` de alguém vira o erro 3004 — que é o
comportamento certo, e o teste que trava isso é o do comportamento velho).
**Candidato compartilhado com a análise do Cassandra(R)** (TTL por célula lá);
a consolidação é da integração.

### 3.3 Pipelining → o protocolo JSON Lines — **SPRINT 3**

**O que é.** Mandar N comandos sem esperar as respostas, e lê-las de uma vez
([pipelining](https://redis.io/docs/latest/develop/using-commands/pipelining/),
*Redis Pipelining*). O manual é explícito que o ganho não é só de RTT: sem
pipelining cada comando paga `read()`/`write()` e troca de contexto (*It's not
just a matter of RTT* — «the context switch is a huge speed penalty»). O
exemplo deles no loopback: 10.000 PINGs, 1,185 → 0,251 s (*A real world code
example*). E há a nota de tamanho: mandar em lotes «razoáveis», da ordem de
10k comandos, porque **o servidor enfileira as respostas em memória**.

**O que resolveria aqui.** A medição do §2 já respondeu metade: **o protocolo
aceita pedidos empilhados hoje**, e o cliente ganha **1,45×–1,58× no
loopback** — justamente onde o ganho é menor. A casa já colheu esse fruto para
escrita com o `inserir_lote` (16,3× pela rede); o pipelining cobre o que o
lote não cobre: **leituras pontuais em rajada**, sem operação nova no
servidor. O que falta não é código do motor: é o contrato escrito (a resposta
sai na ordem do pedido, FIFO **por conexão** — hoje é verdade por construção e
nenhum documento promete), o `phxsql-cmd` sabendo empilhar, e um teste por
soquete que trave a ordem.

**E o escopo encolheu com as dez frentes, de propósito.** O próprio manual do
Redis(R) diz que metade dos casos de pipelining se resolve melhor com script
no servidor, porque *«pipelining can't help in this scenario since the client
needs the reply of the read command before it can call the write command»*
(*Pipelining vs Scripting*). Esse caminho **agora existe aqui**: procedimentos
armazenados (`367d6ca`). Então o pipelining fica com o que é dele —
**operações independentes em rajada** — e não deve ser vendido como solução de
ler-calcular-gravar, que é da rotina no servidor.

**Premissa a medir primeiro.** Já medida no loopback, com a dispersão dita
(§2). Falta **em máquina quieta**, para publicar um número, e **entre máquinas
com RTT real**, que decide se o número publicado é o piso ou algo bem maior.
Se nem com RTT real passar de uns poucos por cento, o sprint encolhe para uma
seção de documentação — que é um resultado válido e barato.

**Custo/risco.** O menor da lista. Risco: prometer FIFO por escrito engessa uma
futura paralelização por conexão — dizer no mesmo parágrafo que a ordem é por
conexão, não do servidor inteiro. E documentar o teto de lote, com o motivo
(o buffer de respostas), como o manual deles faz.

### 3.4 `maxmemory` e eviction → **COBERTO; sobra um degrau, e ele espera o TTL**

**O que é.** `maxmemory` limita a RAM; ao esbarrar no teto, a
`maxmemory-policy` decide
([eviction](https://redis.io/docs/latest/develop/reference/eviction/),
*Eviction policies*): `noeviction` (**recusa a escrita com erro** e segue
servindo leitura), `allkeys-lru`, `volatile-ttl`, `allkeys-lfu`… O LRU deles é
**aproximado por amostragem** (*Approximated LRU algorithm*), porque o exato
«costs more memory».

**O que a casa fez enquanto isto era escrito.** Este sprint era o meu número
4, e **ele está feito** (`fe9cb30`) — e feito na forma que eu proporia:
`recursos.memoria_max_mb` ganhou leitor em `op_memoria_carregar`, com a
semântica do `noeviction` (recusa nomeada, dizendo quanto passaria do teto e
mandando liberar uma tabela ou subir o teto), `0` = sem teto como padrão, e a
conta de ocupação (`m.bytes()`) que era exatamente a minha premissa a medir.
De quebra, provar o teto achou um defeito muito pior: `memoria_carregar`
tomava a trava global **duas vezes**, e `Mutex` da `std` não é reentrante — a
operação travava o servidor inteiro. Fica registrado como o argumento mais
forte a favor da regra da casa: **campo de configuração sem leitor é pior que
campo ausente**, e o `cache_paginas` e o `memoria_max_mb` já provaram isso
duas vezes.

**O degrau que sobra**, e ele é pequeno: hoje o teto **recusa**; a alternativa
é **liberar sozinho** a tabela residente menos usada, que é o `allkeys-lru`
deles. Não proponho isso agora, por dois motivos com fonte: para um banco,
`noeviction` é a política honesta (o manual recomenda LRU para *cache*, e diz
que as políticas `volatile-*` «behave like `noeviction` if no keys have an
associated expiration»); e `volatile-ttl` — a única que faria sentido aqui,
porque libera o que já ia vencer — **depende do TTL existir**, ou seja, do
Sprint 2. Se o Sprint 2 for aprovado e entregue, este degrau vira meia rodada;
antes disso, não tem o que decidir.

### 3.5 Keyspace notifications → a grade que recarrega às cegas — **SPRINT 4**

**O que é.** Clientes assinam canais e recebem eventos de mudança
(`__keyspace@0__:chave` / `__keyevent@0__:del`) — com dois avisos do próprio
manual
([keyspace-notifications](https://redis.io/docs/latest/develop/pubsub/keyspace-notifications/)):
vem **desligado por padrão** porque «the feature uses some CPU power»
(*Configuration*), e Pub/Sub é ***fire and forget*** — «if your Pub/Sub client
disconnects, and reconnects later, all the events delivered during the time
the client was disconnected are lost» (introdução).

**O que resolveria aqui.** A tela só se atualiza perguntando: a grade recarrega
ao navegar, e dois usuários na mesma tabela não veem o trabalho um do outro
até recarregar (a janela de conflito pega o estrago na hora de salvar, não
antes). Uma operação `assinar` na porta de dados — a conexão fica aberta e o
servidor empurra uma linha JSON `{tabela, op, rowid}` a cada escrita
confirmada — deixaria a grade viva.

**E a fronteira ficou mais nítida com as dez frentes, não menos.** A
replicação agora tem quatro modos e dois agendamentos, e o laço da réplica
continua **puxando por posição** (`streaming` = puxa, aplica, dorme, repete).
Isso é o desenho certo, e este sprint **não encosta nele**: fire-and-forget
serve para tela (quem reconecta recarrega a grade, e a perda custou um
refresh) e **não serve para réplica**, que não pode perder evento. Empurrar
evento para réplica — inclusive o *long-poll* que o `CASSANDRA.md` §6.2 propõe
para o `source` — é assunto da frente de replicação, e fica explicitamente
fora daqui. **Candidato compartilhado** na fronteira, não no escopo.

E a lição do Profiler é lei aqui: **desligado tem de custar zero** — o
Profiler desligado cobrava 7% da carga porque trabalhava antes de olhar o
próprio interruptor.

**Premissa a medir primeiro.** A carga em lote pela rede com o mecanismo
presente e **zero assinantes**, contra a de hoje: a diferença tem de ser zero
dentro do ruído — e, com a máquina disputada, «dentro do ruído» exige máquina
quieta para significar alguma coisa. Se o ponto de publicação custar algo com
ninguém ouvindo, a forma está errada (portão atômico antes de qualquer
trabalho — já se sabe como). Segunda premissa: quantas conexões penduradas o
laço aguenta — hoje é uma linha de execução por conexão; medir o custo de 50
assinantes parados.

**Custo/risco.** O maior dos quatro: conexão de vida longa na porta de dados,
interação com `encerrar_sessao`, com a parada do serviço pela web e com o
`BULKINSERT`. Por isso vem por último — e o escopo fecha na **tela**.

### 3.6 Rewrite do AOF → o corte do nosso diário — **descarte**

O `BGREWRITEAOF` reescreve o diário no menor conjunto de comandos que
reconstrói o estado atual: «if you are incrementing a counter 100 times …
you'll end up with … 100 entries in your AOF. 99 of those entries are not
needed» (*Log rewriting*). Funciona porque o AOF tem **um único propósito**:
repor o estado.

O `.log` da casa tem três, e dois morrem na reescrita: ele é **a história**
(pedido 5: toda inclusão, alteração e exclusão com data e hora — auditoria não
admite «99 entradas desnecessárias»), e ele é **a posição da replicação**
(réplicas retomam por offset, e agora em quatro modos; reescrever invalidaria
toda marca de posição — o Redis(R) não tem esse problema porque a réplica dele
sincroniza por snapshot+stream, não por posição no AOF). O equivalente honesto
do problema que o rewrite resolve — diário que só cresce — já foi tratado por
outro caminho: rotação por volume (`recursos.diario_volume_mib`, §4.7.1) e
compactação **medida e recusada duas vezes** com os números na mesa (14,7% no
melhor corte contra 2,1× que o mesmo esforço compraria no `.ndx`, §4.7.3).
Reabrir isso sem número novo seria exatamente o que a regra da casa proíbe.

### 3.7 MULTI/EXEC → **descarte**

A transação do Redis(R) garante não-intercalação e nada mais: comando que
falha dentro do EXEC **não desfaz os outros** («even when a command fails, all
the other commands in the queue are processed», *Errors inside a
transaction*), e a posição oficial é «Redis does not support rollbacks»
(*What about rollbacks?*). Copiar isso aqui entregaria uma peça com cara de
transação e sem a metade que define transação — num projeto cuja folha de
marca **já afirma falsamente** *ACID compliant*, e onde a pendência 11
(transações de verdade, com journal e imagem anterior) está aberta. Meia
transação hoje tornaria a inteira mais difícil de explicar amanhã. E as partes
boas já existem por outros nomes: a não-intercalação, a trava global dá para
toda operação unitária e o `inserir_lote`/`BULKINSERT` dão para carga; o
`WATCH` otimista (*Optimistic locking using check-and-set*) é exatamente a
janela de conflito por `"versao"` com o erro 3004 `CONFLITO` (pedido 123) — a
casa chegou lá primeiro.

### 3.8 RDB por fork + copy-on-write → **descarte**

O retrato do RDB é barato porque «the only work the Redis parent process needs
to do … is forking a child that will do all the rest», com o preço dito no
mesmo manual: «fork() can be time consuming … may result in Redis stopping
serving clients for some milliseconds or even for one second» (*RDB
disadvantages*). `fork()` não existe na `std` portável (o binário Windows
compila de primeira e é regra que continue), e o backup ao vivo com ZIP e
manifesto SHA-256 (pedido 43) já cobre o retrato consistente.

### 3.9 Eviction LRU/LFU no cache de páginas do `.ndx` → **descarte com número**

O cache da casa já tem política medida: segunda chance (CLOCK), escolhida
porque fila simples despejaria a raiz (`DESEMPENHO.md`, *O cache, em uma
tela*), e o teto de 2.048 páginas saiu de uma varredura — **dobrar o teto
compra 0,8 µs** (§2.1). Trocar CLOCK por LRU/LFU amostrado resolveria um
problema que ninguém mediu existir; o LFU deles existe para padrões de acesso
de cache de aplicação (*LFU eviction*), não para descida de B+tree, onde a
raiz e o caminho quente já ficam. Se um dia a política aparecer como suspeita
num perfil, o `--example ordem-da-chave` é o lugar de provar — antes de
qualquer troca.

### 3.10 Slots do cluster → nota para a frente do cluster — **descarte aqui**

O keyspace do Redis(R) é repartido em **16384 slots fixos**,
`HASH_SLOT = CRC16(key) mod 16384` (*Key distribution model*), e a migração
move **slots inteiros** com os estados `MIGRATING`/`IMPORTING` e os
redirecionamentos `MOVED`/`ASK` (*Live reconfiguration*, *ASK redirection*). A
partição alfanumérica da casa (pedido 104) tem a mesma alma: **37 baldes
fixos**, balde decidido pela chave, `rowid = (balde−1) × rpa + slot`.

E o paralelo ficou mais próximo do que eu supunha: a frente do cluster
(`adace51`) **já entregou o redirecionamento** — `REDIRECIONA host:porta`,
erro 4003, quando a escrita chega numa réplica. Ou seja, o mecanismo existe; o
que ele redireciona é por **papel** (réplica → master), não por **faixa de
chave**. Se um dia a casa repartir o keyspace entre nós, o vocabulário que
faltará é a distinção que o Redis(R) fez: `MOVED` (definitivo, o dono do slot
mudou) contra `ASK` (transitório, só enquanto o balde migra) — sem os dois, um
cliente não sabe se deve atualizar o mapa ou só desviar aquele pedido. Fica a
nota, com a fonte, para a frente do cluster levar; propor aqui seria trabalhar
em cima do trabalho dos outros. E o aviso de honestidade que a própria
especificação dá: mesmo lá, «there is always a window of time when it is
possible to lose writes during partitions» (*Write safety*) — slots não
compram consistência forte.

### 3.11 Scripting Lua → **descarte; a decisão foi tomada e é melhor**

O manual usa o scripting como resposta melhor a metade dos casos de pipelining
e de transação (*Pipelining vs Scripting*; *Redis scripting and transactions*:
«usually the script will be both simpler and faster»). A pergunta equivalente
da casa — em que linguagem se escreve o gatilho e o procedimento — estava
parada nos pedidos 49 e 50 **e foi respondida** enquanto isto era escrito
(`367d6ca`, `df04628`): a linguagem é a do **MySQL(R)/MariaDB(R)**, sintaxe
similar e não idêntica, com **um interpretador só** para gatilho e
procedimento. É melhor que Lua para este projeto pela razão que o
`docs/DBEAVER.md` e o `docs/SQL.md` já sustentavam: quem chega vem de ODBC e
de SQL, não de Lua. Nada a propor; o único efeito desta análise é encolher o
escopo do Sprint 3 (§3.3).

### 3.12 `WAIT` / `min-replicas-to-write` → **descarte aqui** (insumo entregue)

`WAIT` pede confirmação de N réplicas para uma escrita, e o manual é honesto
sobre o limite: «it does not turn a set of Redis instances into a CP system…
acknowledged writes can still be lost during a failover»
([replication](https://redis.io/docs/latest/operate/oss_and_stack/management/replication/),
introdução). O `min-replicas-to-write`/`min-replicas-max-lag` é a mesma ideia
por configuração: aceitar escrita só se houver N réplicas com atraso menor que
M segundos, «a best effort data safety mechanism, where consistency is not
ensured … but at least the time window for data loss is restricted» (*Allow
writes only with N attached replicas*). A frente de replicação entregou os
quatro modos (`9a24412`, `378c0f7`) e o assistente (`1a366e4`); a semântica
«espere N cópias» é insumo para ela, com a fonte e o limite ditos — não um
sprint desta análise.

### 3.13 Streams, tipos de dados e o resto → **descarte**

Streams são «an append-only log» com consumo por posição (*data-types*,
*Streams*) — a casa já tem um: o `.log`, que a replicação consome por posição
desde o pedido 111 e agora em quatro modos. Adotar hash/list/set/zset como
tipos de coluna trairia o modelo tabular do projeto; o pedido 31 (tabela em
memória «tipo Redis(R)») já foi atendido no espírito certo, como tabela. Os
tipos probabilísticos e vetoriais são outro produto.

---

## 4. Os sprints, na ordem proposta

A ordem é por **valor medível ÷ risco**, e cada um cabe numa rodada. São
quatro — o quinto virou item coberto pelas dez frentes (§3.4).

### Sprint 1 — O `fsync` fora do caminho de quem escreve — **M**

- **Escopo fechado:** modo novo de `Durabilidade` em que a `Janela`, ao fechar,
  **não** sincroniza dentro da operação: levanta a marca, e uma thread de
  fundo faz o `fsync` das tabelas sujas. O gatilho por relógio **já existe**
  (`lote_milissegundos`) e não muda. MANUAL ganha a tabela «o que cada modo
  promete» — no modo novo o OK significa *recebido*, com janela de perda de N
  ms (a frase do manual do Redis(R): «you can only lose one second worth of
  writes»). Testes: queda do processo dentro da janela perde no máximo a
  janela; os três modos velhos não mudam **nada** (o teste que importa é o do
  comportamento velho).
- **Por que agora:** §4.9 mediu 16,13 → 7,99 µs/linha (2×) e parou na frase
  «decisão de garantia»; o Redis(R) fornece o contrato pronto e rodado há
  vinte anos. Fonte: *AOF advantages* e *How durable is the append only file?*.
- **Premissa a medir primeiro:** p99 com o `fsync` de fundo sob a trava global;
  ganho de ponta a ponta na bancada; **em máquina quieta**. Morte: ganho < 2%,
  ou p99 piorando mais do que o ganho médio compra.
- **Dependências:** nenhuma. Compartilhado com a análise do Cassandra(R).
- **Não entra:** mudar o padrão (`por_lote` fica); tocar na ordem
  `.trash`-antes-de-soltar-o-slot da exclusão (é garantia de formato, não
  janela); WAL/group commit (recusados com número, `DESEMPENHO.md` §3).

### Sprint 2 — TTL por linha, expirando para o `SOFTDELETED` — **M**

- **Escopo fechado:** papel «prazo de expiração» para uma coluna `DateTime` no
  esquema (`PSCH` v7, byte no fim do bloco como no v6); leitura filtra linha
  vencida sem gravar; varredor no relógio de 30 s dos jobs marca `SOFTDELETED`
  com motivo gerado; expiração só na origem, viajando como evento comum de
  marcação; caixa na tela de Nova tabela. Testes: tabela sem TTL byte a byte
  igual à de hoje; linha vencida some das listas e `restaurar` a devolve;
  réplica nunca expira sozinha.
- **Por que agora:** mudança de formato entra cedo; a infraestrutura de
  marcação (pedidos 97–100) está pronta. Fontes: *Appendix: Redis expires* e
  *How expires are handled in the replication link and AOF file*.
- **Premissa a medir primeiro:** **primeiro, contra a alternativa que já
  existe** — um job horário com um procedimento faz isso hoje sem tocar no
  motor (`80f0265`, `367d6ca`); medir granularidade, custo de varredura e
  correção sob replicação dos dois caminhos. Se o job der conta, o sprint
  morre e vira documentação. Depois: custo zero para tabela sem TTL no
  `varrer`.
- **Dependências:** nenhuma técnica; **compartilhado com a análise do
  Cassandra(R)** — consolidar com a integração antes de executar.
- **Não entra:** exclusão física automática (expirado se marca, não se
  expurga); TTL por campo (o Redis(R) tem `HEXPIRE`; aqui não há caso de uso
  provado); política de eviction por TTL (é o degrau do §3.4, e vem depois).

### Sprint 3 — Pipelining: contrato, cliente e número — **P**

- **Escopo fechado:** documentar o contrato (pedidos empilhados valem;
  respostas em FIFO **por conexão**; teto de lote com o motivo); teste por
  soquete que trava a ordem com operações mistas; `phxsql-cmd` ganha o modo de
  mandar um arquivo de comandos empilhado; a bancada da carga ganha a forma
  «empilhado» ao lado de linha-a-linha e lote.
- **Por que agora:** medido nesta análise — **1,45×–1,58× no loopback sem
  mudar o servidor** (§2), e o loopback é o piso do ganho. Fontes: *Redis
  Pipelining* e *It's not just a matter of RTT*.
- **Premissa a medir primeiro:** refazer **em máquina quieta** (as duas
  corridas saíram com `load` ~4, e a dispersão está do lado que favorece a
  proposta); depois, entre máquinas com RTT real, que decide o número
  publicado. Piso de utilidade: se com RTT real não passar de poucos por
  cento, o sprint vira uma seção de documentação.
- **Dependências:** nenhuma.
- **Não entra:** paralelizar o processamento por conexão (a trava global
  serializa — pipelining esconde RTT e syscalls, não compra CPU); mudar o
  protocolo; vender pipelining para ler-calcular-gravar, que agora é caso de
  procedimento no servidor (*Pipelining vs Scripting*).

### Sprint 4 — `assinar`: a grade viva — **M/G**

- **Escopo fechado:** operação `assinar` na porta de dados (tabela ou banco); o
  servidor empurra `{tabela, op, rowid}` por linha JSON após cada escrita
  confirmada; portão atômico antes de qualquer trabalho (zero assinantes =
  zero custo); a grade assina o que exibe e atualiza a linha tocada; reconexão
  recarrega a grade (a perda é um refresh, dito na tela). Testes por soquete:
  o custo zero, a queda do assinante, a não-interferência com `BULKINSERT`.
- **Por que agora:** é o único candidato de cara para o usuário; dois usuários
  na mesma grade hoje só se descobrem no 3004.
- **Premissa a medir primeiro:** carga em lote com o mecanismo presente e zero
  assinantes — diferença zero dentro do ruído, **em máquina quieta** (a lição
  dos 7% do Profiler); custo de 50 assinantes parados.
- **Dependências:** nenhuma técnica; **fronteira com a frente de replicação** —
  este sprint é tela, e empurrar evento para réplica fica explicitamente fora.
- **Não entra:** replicação por push (fire-and-forget perde evento — fonte:
  introdução de *keyspace notifications* — e réplica não pode perder, ainda
  mais com quatro modos dependendo de posição); filtros por coluna/valor;
  entrega garantida.

---

## 5. Registro dos descartes

| Candidato | Por quê | Onde |
|---|---|---|
| Rewrite do AOF sobre o `.log` | o `.log` é história (pedido 5) e posição de replicação em quatro modos; reescrever destrói os dois. Rotação já existe; compactação recusada 2× com número | §3.6 |
| MULTI/EXEC | meia transação sem rollback alimentaria o «ACID compliant» falso da marca; o WATCH já existe como `"versao"`/3004 | §3.7 |
| RDB por fork/COW | sem `fork()` portável; backup ao vivo já cobre | §3.8 |
| LRU/LFU no cache do `.ndx` | CLOCK medido; dobrar o teto compra 0,8 µs; problema não medido | §3.9 |
| Slots/resharding | frente do cluster já entregou o `REDIRECIONA`; a nota que sobra é o par `MOVED`/`ASK`, entregue a ela | §3.10 |
| Scripting Lua | a escolha foi feita e é melhor: linguagem do MySQL(R), um interpretador só | §3.11 |
| `WAIT`/`min-replicas` | frente de replicação entregou os quatro modos; fonte e limite entregues a ela | §3.12 |
| Streams e tipos novos | o `.log` já é o stream da casa; tipos chave-valor trairiam o modelo tabular | §3.13 |
| Teto de memória com eviction | **coberto** (`fe9cb30`), e na forma certa (`noeviction`); o degrau que sobra depende do Sprint 2 | §3.4 |

## 6. Candidatos compartilhados com as análises irmãs

| Candidato | Compartilhado com | O que consolidar |
|---|---|---|
| TTL por linha (Sprint 2) | análise do Cassandra(R) (TTL por célula) | uma só semântica de expiração, decidida uma vez — e medida contra a alternativa job+procedimento, que já existe |
| `fsync` fora do caminho (Sprint 1) | análise do Cassandra(R) (`commitlog_sync: periodic`, `CASSANDRA.md` §6.3) e `DESEMPENHO.md` §4.9 | o contrato do OK por modo, escrito no MANUAL |
| Push de eventos | frente de replicação (4 modos) e `CASSANDRA.md` §6.2 (long-poll no source) | tela usa fire-and-forget; réplica usa posição — nunca misturar |
| `MOVED`/`ASK` como vocabulário | frente do cluster (que já tem `REDIRECIONA`) | distinguir «o dono mudou» de «está migrando», se o keyspace um dia se repartir |
| Espera por N réplicas | frente de replicação | `WAIT`/`min-replicas` como semântica opcional, com o limite dito |
| Política de memória | análise do MariaDB(R) (se o buffer pool aparecer lá) | vocabulário único de política de memória, agora que o teto existe |

## 7. Resumo e aprovação

| # | Sprint | Tam. | Premissa que decide | Depende de |
|---:|---|---|---|---|
| 1 | `fsync` fora do caminho de quem escreve | M | p99 sob a trava global + ganho de ponta a ponta na bancada, em máquina quieta (morte: <2%) | — |
| 2 | TTL por linha → `SOFTDELETED` | M | **bater a alternativa job+procedimento, que já existe**; depois, custo zero sem TTL | consolidar com análise Cassandra(R) |
| 3 | Pipelining: contrato + cliente | P | 1,45×–1,58× medido no loopback com load ~4; refazer em máquina quieta e com RTT real | — |
| 4 | `assinar`: a grade viva | M/G | custo zero com zero assinantes, em máquina quieta; 50 assinantes parados | fronteira com a frente de replicação |

**A execução aguarda a aprovação do Adriano, sprint a sprint.** Nenhum item
deste documento está autorizado por existir aqui; a premissa de cada um se
mede antes de qualquer implementação, e a premissa que falhar mata o sprint
com o número registrado — no `DESEMPENHO.md`, como manda a regra da bateria.

### Como refazer a medição desta análise

```bash
cargo build --release -p phxsql-server
# Sobe o phxsqld na porta 5362 com base em /tmp, cria med.clientes
# (Int8 + Str(40), indice unico porId), insere 2.000 linhas por
# inserir_lote e mede 2.000 op "buscar" pelo soquete em quatro formas:
# linha a linha, e empilhado em lotes de 100/500/2000 -- com TCP_NODELAY,
# duas corridas por forma e a primeira descartada.
#
# Duas regras de convivencia, e a segunda custou caro a alguem:
#  - a porta e 5362 (ou 5762); o demo em 5199/5599 nao se toca;
#  - NAO existe `pkill` no roteiro. A maquina tem phxsqld de outros
#    agentes; encerre pelo PID que voce mesmo subiu. Um pkill largo demais
#    ja derrubou os dos outros.
# E registre o `load average` junto do numero: com a maquina disputada, o
# lado linha-a-linha varia 15% e o empilhado quase nada.
```

O script usado está descrito acima por inteiro para ser reescrito em minutos;
ele não foi versionado porque mede o protocolo como está, sem tocar no motor —
se o Sprint 3 for aprovado, a forma «empilhado» entra na bancada da carga,
versionada, que é onde número de desempenho deve morar.

---

## Nota sobre os nomes

Redis(R) é marca da Redis Ltd. MySQL(R) é marca da Oracle Corporation.
MariaDB(R) é marca da MariaDB Corporation Ab. Cassandra(R) é marca da Apache
Software Foundation. Este documento lê a documentação pública do Redis(R) para
entender decisões de projeto; nenhum código foi copiado, e tudo que os sprints
propõem seria reimplementação escrita do zero, só com a `std` do Rust, como
manda a regra da casa.
