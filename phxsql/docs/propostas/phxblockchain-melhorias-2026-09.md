# Phxblockchain — dossiê de melhorias (papel J, 16/09/2026)

Pesquisa avançada do modo *ledger* encadeado privado, **medida contra o nosso
gargalo e as nossas pétreas antes de virar plano**. Cada técnica externa vem de
fonte primária (RFC / doc oficial / fonte), com a URL ao pé. Não é um plano de
implementação — é a matriz de evidência e a recomendação de ordem; o dono
decide o escopo.

## 0. O estado atual, medido no fonte

O Phxblockchain **não é motor novo — é MODO sobre o Padrão**, sem formato novo:
`Uuid256` + `Sequence` + append-only bastam. Reconhecido por convenção de
esquema (`ledger::e_tabela_ledger`: `hash`/`anterior` Uuid256, `altura`
Sequence, índice único `porAltura`).

| Item | Estado | Onde |
|---|---|---|
| **E1** hash SHA-256 do conteúdo canônico | **pronto** | `ledger.rs::hash_do_bloco` |
| **E2** encadeamento (`anterior`=hash do topo, `altura`) | **pronto** | `ledger.rs::preparar_bloco` |
| **E3** verificação (altura, ligação, conteúdo) | **pronto**, O(n) full re-hash | `ledger.rs::verificar_cadeia` |
| **ALTER travado** em tabela ledger (§2.1 #1) | **pronto** (15/09) | `table.rs::acrescentar_coluna` |
| **E4** raiz de Merkle | sob demanda | — |
| **E5** assinatura por bloco | sob demanda | — |
| **E6** medidor `custo-do-bloco` | **pendente** | — |

**Duas premissas que a pesquisa mediu e que reordenam a prioridade:**

1. **O Ed25519 já está escrito, à mão, zero-dep, e EM USO** — `phxsql-core/src/
   ed25519.rs` (`pub fn assinar`), mais `sha512.rs`, `x25519.rs`, `x509.rs`,
   `asn1.rs`, para o aperto de mão do fio (frente 41). Então o E5 deixa de ser
   «escrever cripto assimétrica» (a fronteira do zero-deps) e vira «chamar o que
   temos». **A receita externa erra sobre nós aqui:** ela diz que Ed25519 é o
   item mais caro/arriscado de implementar; para o PhxSql, o custo já foi pago.
2. **O custo do `verificar_cadeia` (O(n)) e do SHA-256 por bloco NÃO está
   medido** — não existe `custo-do-bloco.rs`. Tudo o que se diz sobre «a
   verificação fica cara com a cadeia longa» é **raciocinado, não medido**. E a
   casa mede SHA-256 a 2,51× em 4 núcleos *por arquivo, não por bloco*. **Medir a
   premissa vem antes de implementar o item** — o E6 é pré-requisito do E4.

## 1. As oito técnicas, medidas

Legenda de custo: **[cripto]** = precisa de assimétrica (a fronteira do
zero-deps); **[formato]** = mudança de formato em disco (entra cedo, é do DBA);
**[witness]** = precisa de armazém externo independente.

### A distinção que organiza tudo: EVIDENTE × RESISTENTE

A cadeia que temos é **tamper-EVIDENTE interna**: `verificar_cadeia` detecta
adulteração relendo a cadeia. Ela **não** é tamper-RESISTENTE: quem controla a
máquina (e as 4 réplicas) reescreve a cadeia inteira de forma coerente. **Só uma
técnica cruza essa linha — a #4 (ancoragem externa).** Todas as outras baixam o
CUSTO da prova ou o ESCOPO da divergência; nenhuma fecha o furo do adversário
que controla o nó.

| # | Técnica | Verif./prova | Cripto nova | Formato | Fecha o furo do nó? | Fonte |
|---|---|---|---|---|---|---|
| 1 | Merkle log (Certificate Transparency) | O(log n) | não | sim (guardar árvore) | não sozinha | RFC 6962 |
| 2 | **Merkle Mountain Range (MMR)** | O(log n) | não | sim (nós **write-once**) | não sozinha | OpenTimestamps/Grin |
| 3 | Ledger tables (modelo) | O(n) recompute | não | append-only já temos | não sozinha | docs MS SQL Server |
| 4 | **Ancoragem externa (WORM)** | O(1)/período | não | não | **SIM — é a peça** | OpenTimestamps / MS |
| 5 | **Ed25519 por bloco** | O(1)/bloco | **[cripto] — mas JÁ TEMOS** | 64 B/bloco | dá **não-repúdio** (autoria) | RFC 8032 |
| 6 | Merkle anti-entropia (Cassandra) | O(log n) comparar; O(n) construir | não | efêmero | resolve **divergência** entre réplicas | fonte Cassandra |
| 7 | Verificação incremental | O(cauda) linear; O(log n) só com #1/#2 | não | checkpoint protegido | só se o checkpoint for protegido (#4) | RFC 6962 §2.1.2 |
| 8 | Agilidade de hash (id por bloco) | — | não | sim (1 id/bloco, **entra cedo**) | não (é seguro-futuro) | RFC 7696 / multihash |

### 1.1 Merkle log e MMR (#1, #2) — prova de inclusão e de CONSISTÊNCIA em O(log n)

O que a cadeia linear NÃO dá: (a) provar «o bloco X está na posição i» sem
reprocessar de X até o topo (a cadeia linear é O(n); Merkle é O(log n)); (b) a
**prova de consistência** — dado o retrato de tamanho `m` e o de tamanho `n>m`,
verificar em O(log n) que a árvore antiga é **prefixo** da nova (nada foi
reescrito no meio). A cadeia linear só dá essa garantia relendo tudo.

**MMR é a variante que casa com a nossa pétrea:** os nós são **write-once**, o
append toca O(log n) nós e **nunca reescreve nó existente** — «a ordem de
digitação é sagrada, o `.reg` nunca reaproveita slot». Merkle rebalanceada (#1)
reescreve o caminho; MMR não. **São alternativas do mesmo problema, não
somáveis** — se entrar Merkle, entra como MMR.

**Veredito: sob demanda, e DEPOIS do E6.** Merkle/MMR otimizam o custo do
`verificar_cadeia`, que não está medido. Construir a árvore é [formato] e não é
grátis. Entra se — e só se — o E6 mostrar que a verificação O(n) dói de verdade
no uso real. Medir antes.

### 1.2 Ancoragem externa (#4) — a única que fecha o furo real

Publicar periodicamente o digest da cadeia (último hash) num **witness
independente e imutável**. **A variante aplicável é a WORM** (armazém
append-only fora do processo do banco, ou **as réplicas testemunhando o digest
umas das outras** — é a §2.1 #3 do DBA), **não** a do Bitcoin (OpenTimestamps
crava no blockchain público — puxaria rede e serialização de terceiro, fora do
escopo). É o que transforma tamper-evidente em tamper-resistente: adulterar a
cadeia passa a exigir adulterar também o witness datado.

**Veredito: é a peça de maior valor de segurança, e decisão do dono sobre o
MEIO.** A replicação de 4 servidores já é o witness pronto — cada réplica
registra o último `(altura, hash)` que recebeu pelo fio. Aceito como meta;
o meio (arquivo WORM local × réplicas assinando o digest) vai à mesa.

### 1.3 Ed25519 por bloco (#5) — não-repúdio, e o custo já foi pago

Assinatura assimétrica por bloco dá o que a hash-chain e o HMAC **nunca** dão:
**não-repúdio** — qualquer um verifica com a chave pública que só o dono da
privada assinou aquele bloco (prova de AUTORIA, não só de integridade). HMAC é
simétrico: quem verifica também poderia ter forjado.

A receita externa marca isto como o item mais caro (curva Edwards, corpo
2^255-19). **Para nós não é: `ed25519.rs` já existe e roda.** O custo é 64 bytes
por bloco (a coluna `assinatura` já está reservada no esquema, `COL_ASSINATURA`)
e uma chamada a `assinar`/`verificar` por bloco.

**Veredito: SOBE na prioridade por causa da premissa medida.** Deixa de ser
«sob demanda porque é caro» e vira «barato, dá não-repúdio, e o primitivo está
pronto». Continua sendo trabalho de projeto e risco (gestão de chaves: onde mora
a privada, como se roda a pública), mas o piso técnico está pago.

### 1.4 Merkle anti-entropia (#6) — localizar divergência entre as réplicas

O que falta ao nosso caso de 4 servidores: a cadeia diz «esta réplica está
íntegra por dentro», mas não diz **onde** duas réplicas divergem sem enviar o
dataset inteiro. A Merkle tree do `nodetool repair` do Cassandra compara de
cima para baixo e só transfere os ranges sob os nós diferentes — O(log n) de
comparação em vez de O(n) linha a linha. A árvore pode ser **efêmera**
(construída na hora), não precisa persistir.

**Veredito: sob demanda, ligado à replicação.** Útil quando a divergência entre
réplicas virar um problema medido; construir a árvore é O(n) e «resource
intensive» (declarado no Cassandra) — premissa a bater na bancada.

### 1.5 Verificação incremental (#7) — de graça só com Merkle

Verificar só a cauda desde uma `altura` confiada, em vez de O(n) do zero.
**Achado medido da pesquisa:** na cadeia LINEAR pura, o incremental é
inerentemente O(cauda) — não há atalho sublinear sem a estrutura de árvore. Para
O(log n) é preciso a prova de consistência do #1/#2. Ou seja: **o #7 barato
DEPENDE de adotar #1 ou #2.** Sozinho, o ganho é só «não reprocessar o prefixo».

**Veredito: consequência do E4, não item próprio.** E o checkpoint confiado
precisa da proteção do #4 — senão o adversário reescreve o checkpoint junto.

### 1.6 Agilidade de hash (#8) — o mais barato, e entra CEDO

Carregar um id de algoritmo de hash por bloco, para migrar de SHA-256 no futuro
(SHA-3/BLAKE) sem quebrar a leitura do passado. É o análogo direto do nosso
`PSCH` gravar um byte por versão: o bloco em disco volta com o algoritmo com que
foi selado. RFC 7696 é explícita sobre dados armazenados: para reverificar o
passado, é preciso saber com que algoritmo cada bloco foi selado.

**Veredito: FAZ AGORA, é a mudança de formato que entra cedo.** Custo trivial (1
id/bloco), e a pétrea do DBA manda: «mudança de formato entra cedo — enquanto não
há dado em produção é barata; depois vira migração». Não há ledger em produção
ainda. Adiar é transformar um byte em migração.

## 2. As recusas medidas

- **Blockchain pública (consenso, PoW, rede P2P):** fora de escopo, e não por
  falta de vontade — esbarra em superfície de rede que o projeto não tem. Recusa
  já registrada no `STATUS-TIPOS.md`.
- **Ancoragem no Bitcoin (OpenTimestamps calendar):** recusada — puxaria
  blockchain público e serialização de terceiro. A variante WORM (#4) entrega a
  mesma garantia sem a dependência.
- **Merkle rebalanceada (#1) no lugar do MMR (#2):** recusada por pétrea — a
  rebalanceada reescreve o caminho; o MMR é write-once, e «o `.reg` nunca
  reaproveita slot».
- **Construir Merkle antes de medir o E6:** recusado por método — a lista do que
  falta é palpite até alguém medir; Merkle otimiza um custo (O(n) do
  `verificar_cadeia`) que ninguém mediu ainda.

## 3. A ordem recomendada

1. **Agora, barato e cedo (formato):** #8 id de algoritmo por bloco, e o §2.1 #2
   (recusar `UPDATE`/`DELETE` de ledger **no motor**, no molde do append-only
   ledger do SQL Server — hoje nada em `Table::atualizar` sabe do modo). Os dois
   entram antes de haver dado em produção. **Dono B+C.**
2. **Medir a premissa:** E6 — `custo-do-bloco.rs` mede o SHA-256 por bloco e o
   O(n) do `verificar_cadeia`. Fecha a premissa que gateia o E4. **Dono J+H.**
3. **Barato agora que o Ed25519 existe:** E5 assinatura por bloco (não-repúdio),
   com a decisão de gestão de chaves. **Dono B, projeto e risco.**
4. **A peça que fecha o furo:** #4 ancoragem externa pela replicação (§2.1 #3) —
   meta aceita, meio à mesa do dono. **Dono C+B.**
5. **Sob demanda, medido:** #2 MMR (se o E6 mostrar que a verificação dói) e #6
   Merkle repair (se a divergência entre réplicas virar problema). **Dono B.**
6. **Endurecimento:** §2.1 #4 `DROP` de tabela ledger redobrado — «um pai que
   sempre tem filhos». **Dono C.**

## Fontes

- RFC 6962 (Certificate Transparency) — Merkle log, inclusão, consistência, STH:
  https://datatracker.ietf.org/doc/html/rfc6962
- Merkle Mountain Ranges (OpenTimestamps):
  https://github.com/opentimestamps/opentimestamps-server/blob/master/doc/merkle-mountain-range.md
- SQL Server ledger — updatable/append-only, database digest:
  https://learn.microsoft.com/en-us/sql/relational-databases/security/ledger/ledger-overview
- OpenTimestamps (ancoragem): https://opentimestamps.org/
- RFC 8032 (Ed25519): https://datatracker.ietf.org/doc/html/rfc8032
- Cassandra Merkle repair: fonte
  `src/java/org/apache/cassandra/utils/MerkleTree.java`
- RFC 7696 (agilidade de algoritmo): https://datatracker.ietf.org/doc/html/rfc7696
- multihash: https://github.com/multiformats/multihash

Nenhum número aqui é estimado: os limites O(log n), tamanhos e prefixos vêm
citados das RFCs/docs lidas. O que **não** medi e sinalizo como premissa a bater
na bancada: o custo real de construir/persistir a árvore/MMR sobre o nosso
`.reg`, e o O(n) do `verificar_cadeia` (o E6) — medir a premissa vem antes de
implementar o item.
