# Parecer: a transação com quórum de escrita — pedido 207

**Frente F2 (cluster), 07/09/2026.** Este documento é um **parecer medido**, e
não uma entrega. O pedido 207 mandava, nesta ordem: *medir a premissa; se a
rota existir e couber inteira nesta frente, implementar; se não couber, não
entregar meia transação.* **Não coube, e o motivo tem número.** O que segue é
o que se mediu, o que a medição derrubou, e o que falta — para a rodada
seguinte começar de onde esta parou em vez de recomeçar da hipótese.

---

## 1. A premissa que o pedido carregava, e o que a medição fez com ela

A linha do `PENDENCIAS.md` dizia que o quórum precisaria da **rota do canal
aberto (`bidirecional.rs`)**. Ler o fonte parte essa frase em duas, e cada
metade teve destino diferente.

### 1.1 `bidirecional.rs` não tem canal nenhum — e nunca teve

A primeira linha do cabeçalho do módulo diz: *«A parte funda da replicação
bidirecional (multi-master), **sem rede**.»* Ele resolve conflito por carimbo,
decide quem vence no empate, evita o laço infinito pela origem gravada no
evento e identifica a linha pela chave única. **Não abre soquete, não conecta,
não empurra.** Apontá-lo como a rota do canal aberto mandava a rodada seguinte
procurar no arquivo errado — e este parecer existe, entre outras coisas, para
essa linha não sobreviver mais uma rodada.

### 1.2 O canal aberto EXISTE — e é o do pulso

O `laco_do_pulso`/`pulsar` (`crates/phxsql-server/src/servidor.rs`) abre uma
conexão de **cada nó para cada outro nó**, autentica com a credencial do
cluster e a mantém viva num laço, trocando `cluster_pulso` a cada `pulso_s`.
Num cluster de N nós são **N×(N−1)** conexões longas — e entre elas estão as
do **master para cada réplica**, abertas pelo master, já autenticadas, já
quentes.

Medido, e **contado na telemetria de cada nó** (`bancada/quorum/canal.py`), num
cluster de três:

| o que | medido |
|---|---|
| canais abertos no cluster inteiro | **6** (`pulso-<id>`, contados nos três nós) |
| canais que o **master** já segura para as réplicas | **2** |

Contados, e não deduzidos de N×(N−1): a fórmula continuaria certa depois de o
laço morrer. E o `pulso-supervisor` fica **fora** da conta — ele não conecta em
ninguém, só cuida das threads que conectam, e contá-lo inflaria o número em um
por nó.

### 1.3 O que isso derruba

> **«O master não tem como fazer ninguém buscar» é verdade na REPLICAÇÃO e
> falsa no CLUSTER.**

Na replicação pura (source/réplica) o desenho é *pull* de propósito, e o motivo
escrito é firewall: o source abre **uma** porta de entrada e nunca precisa
alcançar a réplica de volta. Num **cluster**, o próprio pulso já obriga todo
mundo a alcançar todo mundo — e o master já segura a conexão de saída que um
push usaria. **A propriedade de firewall que o desenho comprou não se perderia
com o quórum: ela já foi gasta pelo cluster**, no dia em que o pulso nasceu, e
não por este pedido.

Isso muda a rota (c) do pedido — «o master abrir conexão … quebra a
propriedade» — de *proibida* para *já paga*, **dentro de um cluster**. Fora
dele, continua valendo inteira.

---

## 2. Os números do canal

`python3 bancada/quorum/canal.py 60` — três `phxsqld` em `127.0.0.1`, no mesmo
contêiner, conexões **quentes** (abertas e autenticadas uma vez, como o
`pulsar` faz). Medido em 07/09/2026.

| o que se mede | mediana | faixa |
|---|---|---|
| **piso do canal** — uma ida e volta de `cluster_pulso`, sem dado nenhum | **0,089 ms** | 0,069 – 0,232 |
| **empurrar um evento** — `aplicar` de uma linha, pela conexão quente | **0,466 ms** | 0,337 – 34,988 |
| **quórum 2-de-3 pelo canal** (o master é um voto; espera a 1ª réplica) | **0,447 ms** | — |
| **quórum 3-de-3 pelo canal** (espera a última) | **0,498 ms** | — |

Tudo em `127.0.0.1`: **isto é o piso**, a rede real custa mais.

**O que o piso de 0,089 ms diz:** o canal em si é barato. Nenhum push pode
custar menos do que isso, e ele é ~5× menor que o custo de levar um evento —
ou seja, o preço do quórum é **o trabalho de aplicar**, não o de falar. Foi
justamente o contrário da conta que se poderia supor, e é a mesma lição do
mutex do Profiler: *diagnóstico plausível não é diagnóstico medido.*

**Confere com a medição anterior, e a explica.** A `bancada/quorum/medir.py`
publicou `levar 0,475 ms` e `quorum 2-de-3 0,661 ms` com o Python puxando à
mão, abrindo o caminho a cada volta. Pelo canal quente: **0,466 ms** e
**0,447 ms**. A diferença de ~0,2 ms no 2-de-3 é o aperto de mão que aquela
média carregava e que o master **não pagaria** — ele já tem a conexão aberta.

---

## 3. Por que NÃO foi implementado nesta frente

A rota existe, é mais barata do que se supunha, e ainda assim **não cabia
inteira**. O que falta não é o transporte: é tudo o que decide o que o «ok»
significa. Meia transação aqui seria pior que nenhuma, porque um commit que
diz «gravei com quórum» sem cumprir uma destas quatro coisas mente sobre
durabilidade — e é exatamente sobre isso que o Cassandra® já nos avisou de
graça, com o `QUORUM` dele querendo dizer «N processos copiaram para um
`mmap`».

### 3.1 O que o «ok» da réplica significa — e hoje não significa nada disso

O `aplicar` de hoje devolve `ok` quando **aplicou no `.reg` e no `.ndx` da
réplica**, sob a política de durabilidade **dela**. Com `durabilidade:
"por_lote"` (o padrão), isso é «está no page cache do sistema», não «está no
disco». São três garantias com o mesmo nome — recebeu, aplicou, aplicou e
sincronizou —, e **o quórum tem de dizer qual delas conta**, no protocolo e no
documento. Enquanto não disser, o número que ele publica é o do Cassandra®: um
`QUORUM` que soa como disco e é memória.

### 3.2 O empurrar não existe como operação

O medidor puxou do master (`replicar`) e aplicou na réplica (`aplicar`) **do
lado de fora**. Para o master fazer isso por dentro, no instante do commit,
falta a peça: uma ordem `replicar_empurrar` que o master mande **pelo canal do
pulso**, carregando o evento recém-gravado, e que a réplica confirme. Hoje o
`pulsar` só sabe mandar `cluster_pulso`, e o laço dele dorme `pulso_s` entre
uma volta e outra — o commit não pode esperar um segundo por um pulso.

### 3.3 O prazo, e o que acontece quando ele estoura

Quórum é **contar confirmações com relógio**. Falta decidir e escrever:
quantos milissegundos o commit espera; o que ele devolve quando N não
confirmam (a linha **já está gravada** no master — desfazê-la exigiria a
transação que ainda não existe); e o código de erro dessa recusa. Sem isso o
comportamento no caso ruim — que é o único caso que importa numa garantia —
fica indefinido.

### 3.4 A troca de disponibilidade é decisão de produto, e ela é irreversível na prática

O próprio pedido já registra: **sem N réplicas alcançáveis o commit falha,
onde hoje aceita.** Trocar «sempre aceita» por «às vezes recusa» muda o
contrato de quem já escreve neste banco. Por isso ele nasce **pedido, não
imposto** — e por isso a peça que já entrou nesta frente foi só o **campo**,
com a tela dizendo com todas as letras que ele ainda não é imposto.

---

## 4. O que JÁ entrou, e por que essa parte não é meia transação

Duas peças, e as duas se sustentam sozinhas:

1. **O campo `cluster.quorum_minimo`** — lido do `config.json`, devolvido pela
   op `config`, gravável pela tela. Ele entra agora porque **mudança de
   formato entra cedo**: enquanto não há dado em produção é barato, depois
   vira migração. O pedido 207 já decidiu onde ele mora (no bloco `cluster`,
   ao lado de `nos`), e adiar o campo só adiaria a migração.

2. **A honestidade sobre ele, dita pelo SERVIDOR** — a op `config` devolve
   `"quorum_imposto": false` ao lado do valor. Não é uma frase da tela: duas
   telas divergem no dia em que uma for atualizada e a outra não, e a que
   envelhece é sempre a que ninguém compila. Quando o quórum passar a valer,
   quem muda esse `false` é o mesmo commit que fizer o commit esperar.

Campo que finge efeito é pior que campo ausente — esta casa já pagou por isso
com o `recursos.cache_paginas`, que passou três versões no `config.json`, no
MANUAL e na tela **sem uma linha de código o lendo**. A diferença entre aquele
caso e este é uma só: aqui o servidor **declara** que não lê.

---

## 5. O caminho, para a rodada seguinte

Na ordem, e cada passo com o número que o justifica:

1. **Decidir e escrever o que o «ok» significa** (§3.1). Sem isto, tudo o que
   vier depois publica uma garantia que ninguém definiu.
2. **`replicar_empurrar`**: o master manda o evento pelo canal do pulso e a
   réplica confirma. O piso está medido — **0,089 ms** de ida e volta vazia, e
   **0,466 ms** carregando um evento.
3. **O laço do pulso vira canal de duas ordens**: hoje ele dorme `pulso_s`
   entre voltas, e o commit não pode esperar isso. É o único ponto que exige
   mexer numa thread que já funciona — e é onde uma frente sozinha erraria em
   silêncio, porque o teste que quebra é o de eleição, não o de escrita.
4. **O prazo e o código de erro** (§3.3), com bancada do caso ruim: réplica
   morta no meio do commit.
5. **Ligar o campo**: `cluster.quorum_minimo` passa a ser lido no caminho de
   escrita, `quorum_imposto` vira `true`, e a linha do `CAMPOS_EDITAVEIS` vira
   `a_quente` no **mesmo commit**.

**O que este parecer NÃO recomenda:** implementar o quórum sem o passo 1. É a
única ordem em que o passo 4 tem como ser testado — e um quórum sem definição
de garantia não é uma funcionalidade pela metade, é uma promessa errada
inteira.

### 5.1 As decisões do dono, tomadas em 10/09/2026

O passo 1 e mais duas sub-decisões da §3.3 fecharam com o Patrão. O que muda no
caminho acima:

- **O «ok» significa «aplicou e sincronizou»** (decisão 207): a réplica só
  confirma depois do `fsync`. É o que o passo 2 (`replicar_empurrar`) tem de
  esperar antes de contar o voto.
- **O prazo é um campo** (207-A): `cluster.quorum_prazo_ms`, ao lado de
  `quorum_minimo`. **O padrão NÃO se crava agora** — fixa-se depois de medir o
  round-trip com `fsync` na réplica, número que ainda não existe. Isto é o
  passo 4 alimentando o passo 5.
- **A falha recusa e desfaz** (207-B, saída iii): quando N réplicas não
  confirmam no prazo, a escrita — envolvida numa transação (pedido 162) — faz
  `ROLLBACK` no master. **CONDICIONADO**: o C (DBA) confirma primeiro que o
  `ROLLBACK` compõe com o evento que já foi empurrado para as réplicas que
  confirmaram; se não compuser sem desfazer também na réplica, a saída cai para
  (i) «aceita e avisa». Este é o **primeiro passo do build**, e é de projeto e
  risco: nenhuma linha de código antes do veredito do DBA.

### 5.2 O veredito do DBA (10/09/2026) — (iii) NÃO compõe, cai para (i)

O parecer do C (DBA) rodou antes de qualquer código e voltou firme: **(iii) não
compõe**, e por regra escrita em 207-B cai para **(i) «aceita e avisa»** nesta
rodada. As evidências, com file:line:

- **O `ROLLBACK` do pedido 162 é pré-commit, não é undo.** `transacao.rs:9-13`:
  nada vai a disco antes do COMMIT; o ROLLBACK joga a lista da RAM fora, zero
  bytes. Depois que a linha foi gravada (COMMIT aplicado), **não há ROLLBACK** —
  removê-la fisicamente reaproveitaria slot, o que fura «o `.reg` nunca
  reaproveita slot» e «a ordem de digitação é sagrada». Provado por
  `o_rollback_de_um_insert_nao_queima_slot` (`servidor.rs:32985`).
- **O desenho já tinha proibido este cenário por escrito:** `transacao.rs:15-19`
  — «um INSERT gravado e depois revertido deixaria um buraco permanente -- e,
  pior, teria de deixar o MESMO buraco na replica». É a saída (iii) palavra por
  palavra.
- **As réplicas que confirmaram já aplicaram+`fsync`** (decisão 207) e o aplicar
  é só-para-a-frente: `log.rs:96-100` só conhece Inclusao/Alteracao/Exclusao, não
  há reverter. Forçar recuo bate em rowid divergente e **trava a replicação**
  (`table.rs:3670-3677`) — divergência dura, não auto-curável.

**A saída (iii) só volta a existir se o Patrão REABRIR a 207** para tornar o
aplicar da réplica revogável via um protocolo de duas fases (prepare/commit,
espelhando o `.tx` do master). Isso é protocolo novo, **contradiz o «ok =
aplicou e sincronizou»** da 207 (viraria um quarto sentido de «ok»), é de
projeto e risco, e tem premissa a medir (o round-trip de prepare+commit, ainda
não medido). Fica registrado como caminho, decisão do dono — não como código.

A premissa que envelheceu, registrada: a §3.3 dizia «a transação que ainda não
existe». Existe desde o pedido 162 — e é o que torna (iii) possível. A §3.3
fica com esta ressalva.

---

## 6. Onde estão os números

| número | de onde sai |
|---|---|
| canais abertos, pulso quente, empurrar, quórum pelo canal | `bancada/quorum/canal.py` → `bancada/quorum/resultados-canal.json` |
| gravar / levar / quórum 2-de-3 e 3-de-3 com conexão fria | `bancada/quorum/medir.py` → `bancada/quorum/resultados.json` |
| o sono que não era transporte | `docs/cognicao/cognicao_o-sono-nao-e-o-transporte_20260907_0930.md` |
| o que o `QUORUM` do Cassandra® significa no fonte deles | `docs/CASSANDRA.md` |
