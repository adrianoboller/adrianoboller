# Técnicas de motor de banco, medidas contra o NOSSO gargalo (16/09/2026)

Papel J (pesquisa). **Só leitura e medição — nada foi implementado e nada foi
comitado por esta frente.**

O dono pediu «as melhores técnicas usadas em bancos de dados». A lei desta casa
diz o que fazer com um pedido assim: *receita de fora se mede contra o nosso
gargalo antes de virar plano*. Este documento não traz arquitetura: traz
**mecanismo, custo e o que ele nos obrigaria a mudar**, na ordem do ganho
medido contra o que dói aqui.

**Carga de todas as medições:** máquina do contêiner, 4 núcleos, 16 GiB
(13,8 GiB livres), `load average` entre 1,43 e 2,38 — anotada em cada corrida.
Binário recompilado antes de medir (`cargo build --release --examples -p
phxsql-store`), porque *medidor com binário velho mede o passado*.

---

## 0. A premissa do pedido estava morta, e é o primeiro resultado

O encargo desta pesquisa dizia, como ponto de partida:

> «**83,5% do tempo de uma inserção está no `.ndx`**; o arquivo de dados custa
> 16,5%.»

**Esse número não vale mais, e o próprio `DESEMPENHO.md` já o havia aposentado**
(§1, re-medição de 08/09/2026: `.ndx` 34,6%, `.reg`+`.log` 60,5%). Re-medido
**agora**, pelo `--example onde-doi`, três corridas por tamanho:

| n | `.reg` heap | `.log` | 1º índice | chave única | 2º índice | **`.ndx` total** | µs/linha |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 50.000 | 43,7% | 13,0% | 15,5% | 0,6% | 27,3% | **43,4%** | 6,2 |
| 200.000 | 35,6% | 12,0% | 17,5% | 10,4% | 24,4% | **52,3%** | 5,7 |
| 1.000.000 | 41,3% | 15,2% | 17,9% | 5,8% | 19,8% | **43,5%** | 5,3 |

**O `.ndx` custa 43–52%, não 83,5%.** Quem planejasse contra os 83,5% estaria
mirando um arquivo que hoje carrega menos da metade do custo — que é exatamente
o erro que esta casa já cometeu duas vezes nesta mesma busca (a localidade do
pedido 113, e o «atacar o `.ndx`» de 08/09).

*A lista do que falta também é palpite até alguém medir — inclusive quando o
item é nosso, e inclusive quando o número vem no próprio encargo.*

---

## 1. A matriz de evidência

Ordenada por **ganho medido contra o nosso gargalo**, não por elegância.

| # | Técnica | Fonte primária | Que problema resolve | O nosso número nesse problema | O que nos custaria | Veredito |
|---:|---|---|---|---|---|---|
| 1 | **Divisão de página enviesada à direita** (rightmost / sequential split) | PostgreSQL `nbtsplitloc.c`; InnoDB `btr0btr.cc` | Páginas 50% cheias quando as chaves chegam crescentes | **46,7% de ocupação medida** na chave crescente, contra 76,2% na espalhada | Nada de formato: muda só a escolha do ponto de corte | **ENTRA** (ver §2) |
| 2 | **Lista de ocorrências / deduplicação** (posting list) em índice não único | PostgreSQL `nbtree/README` (dedup, PG 13) | Chave repetida armazenada inteira a cada linha | `.ndx` de 8 valores distintos = `.ndx` de 200.000 distintos: **2904 contra 2910 páginas** | Formato do `.ndx` (PSCH/PNDX novo) + varredura/remoção | **REAL, e é do DBA** (§3) |
| 3 | **Cabeçalho fora do caminho quente no `remover`** | a nossa própria `ndx.rs:968` contra `ndx.rs:1578` | 4 KiB gravados no offset 0 a cada remoção, por índice | **0,48 µs por gravação**, ×2 índices = **0,96 µs de 30,65** (3,1%) | Uma condição, igual à que o `inserir` já tem | **ENTRA — é caminho irmão** (§4) |
| 4 | **Buffer pool: capacidade contra política** (LRU‑K, 2Q, CLOCK) | PostgreSQL `storage/buffer/README` | Despejo burro sob pressão | Já temos **CLOCK** (`ndx.rs:189`). O penhasco da §21 é **capacidade**, não política — medido em §5 | Trocar a política não compra nada; subir o teto já é configurável | **NÃO é o nosso gargalo** (§5) |
| 5 | **Ring buffer / `BufferAccessStrategy`** para varredura | PostgreSQL `storage/buffer/README` (anel de 256 KiB) | Varredura despeja o conjunto quente | **Medido: 0,86–0,88×** — inserir DEPOIS da varredura é *mais rápido*; o controle sem varredura dá 0,98–1,15× | — | **HIPÓTESE MORTA, medida** (§6) |
| 6 | **Compressão de prefixo / truncagem de sufixo** em B+tree | PostgreSQL `nbtree/README` («the goal … is to improve index fan-out») | Fan-out baixo, árvore alta | Altura já é **3** (9,57 toques ÷ 3 descidas = 3,19); leque 145–254 | Formato + comparação de chave variável | **NÃO é o nosso gargalo** (§7) |
| 7 | **Chave de largura variável** (o desperdício do `Str(n)` fixo) | consequência da medição própria | Chave curta paga a largura declarada | `Str(60)` custa **146,5 B/linha** contra 59,5 do `Str(20)` — **2,5× as páginas** | Formato do `.ndx` | **REAL, mede junto com o #2** (§3) |
| 8 | **`RwLock` de verdade no caminho de leitura** | PostgreSQL LWLock; a nossa §14 | Trava serializa leitor com leitor | A trava come **20% do paralelismo na leitura e 25% na escrita** com 2 clientes; **não há segundo gargalo embaixo** (§14) | Desenho de concorrência; o `RwLock<Raiz>` **já existe** (`servidor.rs:647`) | **REAL, e não reabre a Sombra** (§8) |
| 9 | **Write-back de página com selo no despejo** | InnoDB `mtr0mtr.cc:338` / `buf0flu.cc:1243`; Aria `ma_pagecache.c:177` | CRC por chave em vez de por folha | **JÁ CONVERGIMOS** — 13,1 → 7,2 µs/linha; o CRC virou **1%** | — | **JÁ EXISTE** |
| 10 | **Checksum por página** | Cassandra 5.0.10; InnoDB; Aria | Corrupção silenciosa | **JÁ CONVERGIMOS**, e sem saber: o CRC-32 acontece no despejo | — | **JÁ EXISTE** |
| 11 | **Group commit** | InnoDB; a nossa §12.1 | `fsync` por transação | **JÁ EXISTE, 2,63× medido**; o que sobra não vale 1,5× | — | **JÁ EXISTE** |
| 12 | **Marca de «não fechei direito» + reconstrução** | Aria `ma_locking.c:460` | Índice atrasado em silêncio após queda | **JÁ EXISTE** (`sujo` no cabeçalho, `precisa_reconstruir`) | — | **JÁ EXISTE** |
| 13 | **Skip list / ART / learned index** | — | Índice **em memória** com concorrência fina | O nosso `.ndx` é paginado em disco, com CRC por página e recuperação por marca | Trocar a estrutura em disco inteira | **NÃO SE APLICA** (§9) |
| 14 | **Zone maps / min-max por página (BRIN)** | PostgreSQL BRIN | Varredura de faixa em tabela grande | A varredura de faixa **já ganha 7,98×** do MySQL(R) (§6 do `DESEMPENHO.md`) | Formato novo + manutenção | **NÃO é o nosso gargalo** |
| 15 | **Prefetch sequencial** | — | Leitura de arquivo no caminho crítico | **0,00 páginas lidas do arquivo por linha**, medido nos três tamanhos | — | **SEM OBJETO** — não há o que pré-buscar |
| 16 | **LSM / SSTable / compactação** | Cassandra 5.0.10 (`docs/CASSANDRA.md` §7.2) | Escrita massiva | — | **Quebra a ordem de digitação** (pétrea), o endereço por conta, o cursor e a réplica | **QUEBRA PÉTREA** — já recusado em `DESEMPENHO.md` §5 |

---

## 2. O item nº 1: a divisão de página, e por que a nossa pétrea a torna mais valiosa aqui do que lá

### 2.1 O número

`.ndx` de 200.000 linhas, um índice, medido agora (ocupação = mínimo teórico do
formato ÷ páginas realmente usadas):

| padrão da chave | ck_len | leque | páginas | mínimo | **ocupação** |
|---|---:|---:|---:|---:|---:|
| Int8 **crescente**, único | 16 | 254 | 1.688 | 788 | **46,7%** |
| Int8 **espalhado**, único | 16 | 254 | 1.035 | 788 | **76,2%** |
| Int8, 8 distintos | 16 | 254 | 1.682 | 788 | **46,9%** |
| Int8, 100 distintos | 16 | 254 | 1.622 | 788 | **48,6%** |
| Str(20) crescente | 28 | 145 | 2.910 | 1.380 | **47,4%** |
| Str(20), 8 distintos | 28 | 145 | 2.904 | 1.380 | **47,5%** |

**O caso crescente é o PIOR, e é justamente o caso comum.** A chave espalhada
tira 76,2%; a crescente, 46,7%.

### 2.2 A causa, no nosso fonte

`crates/phxsql-store/src/ndx.rs:1040`:

> `let meio = entradas.len() / 2;`

Divisão 50/50 **incondicional**, sem caso especial para a folha mais à direita.
Quando as chaves só crescem, a metade da esquerda nunca mais recebe nada e fica
meia vazia para sempre.

### 2.3 Por que isto nos atinge MAIS do que aos outros

A chave completa é `chave_do_usuário + rowid` (`ndx.rs:898`), e **o rowid é
monotônico e nunca reaproveitado — é a pétrea da ordem de digitação.**
Consequência que a tabela acima mostra medida: mesmo num índice de **8 valores
distintos**, o rowid faz a inserção cair sempre à direita *dentro de cada
grupo* — 46,9%. A nossa pétrea **espalha o padrão de append por todos os
índices**, inclusive os que pareceriam aleatórios.

Ou seja: a técnica vale mais aqui do que na origem dela, e vale por causa de
uma restrição nossa. É o teste de «inspiração, não cópia» passando **a nosso
favor** — a divergência tem restrição nomeada.

### 2.4 As fontes primárias, e o teste dos três motores

**PostgreSQL**, `src/backend/access/nbtree/nbtsplitloc.c`, verbatim:

> «If the page is the rightmost page on its level, we instead try to arrange to
> leave the left split page fillfactor% full. […] In this way, when we are
> inserting successively increasing keys (consider sequences, timestamps, etc)
> we will end up with a tree whose pages are about fillfactor% full, **instead
> of the 50% full result that we'd get without this special case**.»

Essa última frase descreve a nossa medição palavra por palavra. **Nós somos o
ramo «without this special case».**

**InnoDB** (MariaDB e MySQL compartilham o motor), `storage/innobase/btr/btr0btr.cc`,
verbatim:

> «We use eager heuristics: if the new insert would be right after the previous
> insert on the same page, we assume that there is a pattern of sequential
> inserts here.»

Peso: **PostgreSQL 4 + MariaDB 3 + MySQL 2 = 9**. O SQLite (peso 1) **não foi
conferido em fonte primária** — ver §10, lacuna 1. O veredito não depende dele.

**Nenhuma pétrea se opõe:** o `.reg` não é tocado, a ordem de digitação não é
tocada, o leiaute da página não muda, arquivo velho continua sendo lido. Pela
decisão do dono de 11/09/2026, **o comportamento entra sem perguntar** — e o
*meio* continua nosso.

### 2.5 O que se espera ganhar, e o que é raciocinado

- **Medido:** a ocupação de hoje é 46,7%, e o formato comporta 100%.
- **Raciocinado, não medido:** a 90% de fator de enchimento, as 1.688 páginas
  cairiam para ~876 — **1,93× menos páginas de `.ndx`**. Isso (a) quase metade o
  arquivo, (b) **dobra a capacidade efetiva do cache**, que a §5 mostra ser a
  restrição real, e (c) reduz a altura da árvore na escala grande.
- **O que decidiria na bancada:** o `--example onde-doi` nos três tamanhos e o
  `custo-da-chave-a-mais` depois da mudança — o penhasco da §5 tem de **andar
  para a direita** na proporção da densidade. Se não andar, a explicação da §5
  está errada e o ganho é só de espaço.

---

## 3. Os itens nº 2 e nº 7: o que o `.ndx` guarda que não precisava guardar

Medido agora, 200.000 linhas, um índice sobre a coluna:

| coluna indexada | distintos | `.ndx` | páginas | **B/linha** | `.reg` |
|---|---:|---:|---:|---:|---:|
| `Str(20)` | 8 | 11.616 KiB | 2.904 | **59,47** | 12.109 KiB |
| `Str(20)` | 100 | 11.344 KiB | 2.836 | 58,08 | 12.109 KiB |
| `Str(20)` | 10.000 | 9.272 KiB | 2.318 | 47,47 | 12.109 KiB |
| `Str(20)` | 200.000 (todos) | 11.640 KiB | 2.910 | 59,60 | 12.109 KiB |
| `Str(60)` | 8 | 28.608 KiB | 7.152 | **146,47** | 19.922 KiB |
| `Str(60)` | 200.000 (todos) | 28.648 KiB | 7.162 | 146,68 | 19.922 KiB |

Dois achados, e os dois surpreendem:

1. **A cardinalidade não muda quase nada.** 8 valores distintos ocupam o mesmo
   que 200.000 (2.904 contra 2.910 páginas). O índice guarda a chave inteira,
   de largura fixa, uma vez por linha — repetida ou não. É exatamente o que a
   deduplicação do PostgreSQL ataca: *«We deduplicate non-pivot tuples in
   non-unique indexes to reduce storage overhead, and to avoid (or at least
   delay) page splits»* (`nbtree/README`).
2. **Um índice `Str(20)` custa quase o mesmo que os dados** (11,6 contra 12,1
   MiB), e um `Str(60)` custa **1,44× os dados** (28,6 contra 19,9 MiB).

**A divergência que teríamos, e a restrição que a causa:** o PostgreSQL
deduplica **preguiçosamente, só na hora da divisão**, porque as duplicatas dele
são lixo de MVCC que pode sumir. Aqui **não há MVCC** (a Sombra está parada por
decisão do dono) e **o rowid é crescente e nunca reaproveitado**. Logo a nossa
lista de ocorrências seria sempre **um append no fim**, e poderia ser feita
**na hora, não na divisão** — o que o PostgreSQL não pode fazer. A restrição
nossa (ordem de digitação) torna a técnica *mais simples* aqui do que lá.

**Veredito: real, e é do papel C.** É mudança de formato do `.ndx`, e *mudança
de formato entra cedo*. Não entra por esta frente: entra por parecer do DBA,
com a premissa medida antes — e a premissa está medida acima.

---

## 4. O item nº 3: o caminho irmão que ficou

`docs/DESEMPENHO.md` §24.5 registra: *«O inserir passa pelo write-back; o
remover não.»* **Fui ao fonte e a frase está imprecisa — e o que há embaixo é
pior, porque é mais fácil de consertar.**

Os dois caminhos tratam a **mesma variável** de jeitos opostos, a 610 linhas um
do outro:

`ndx.rs:968` (`inserir_ja_conferido`):

```
self.indices[idx].qtd_chaves += 1;
// O contador nao justifica 4 KiB por chave: ele vai no `sincronizar`,
// e `verificar` sabe recalcula-lo. A ESTRUTURA vai na hora.
if self.estrutura_mudou {
    self.gravar_cabecalho()?;
}
```

`ndx.rs:1577` (`remover`):

```
self.indices[idx].qtd_chaves = self.indices[idx].qtd_chaves.saturating_sub(1);
self.gravar_cabecalho()?;
```

A página, nos dois, vai por `gravar_pagina` — **os dois passam pelo write-back**.
O que difere é o **cabeçalho**: o `inserir` o condicionou a `estrutura_mudou`; o
`remover` o grava sempre. É o mesmo defeito que o comentário do próprio campo
(`ndx.rs:322`) diz já ter sido encontrado **três vezes** nesta base, terminando
com *«Cabeçalho de arquivo não pertence ao caminho quente»* — e o `remover` o
mantém lá.

**Medido agora**, instrumento próprio e independente (`rustc -O`, 200.000
repetições, três corridas, carga 1,96):

| o que | µs |
|---|---:|
| `gravar_cabecalho` inteiro (alocar 4 KiB + `seek(0)` + `write`) | **0,479 – 0,508** |
| só o `seek`+`write` de 4 KiB | 0,408 – 0,422 |
| copiar 4 KiB (`clone`) | **0,051 – 0,053** |

*(Esta última confere, por instrumento independente, os 0,06 µs que o `onde-doi`
mede por dentro — dois instrumentos concordando.)*

**O custo:** 0,48 µs × 2 índices = **0,96 µs por exclusão**, de **30,65 µs**
medidos hoje = **3,1% do excluir**, ou **10,8% da parcela `.ndx` remover**
(8,86 µs). Confirma-se pelo núcleo na mesma corrida: `write precos.ndx` = **4,00
por exclusão** com 2 índices — duas páginas despejadas e **dois cabeçalhos**.

**É seguro?** Raciocinado do fonte, não medido: sim, pelo mesmo argumento do
`inserir`. A marca de sujo já foi ao disco por `gravar_pagina` (`ndx.rs:745-748`)
antes desta linha; o `fechar` grava o cabeçalho quando `sujo || estrutura_mudou`
(`ndx.rs:870`), e depois de uma remoção `sujo` é verdadeiro; e o `verificar`
sabe recalcular `qtd_chaves` varrendo. **O que decidiria na bancada:** a prova
da tomada do pedido 253, repetida — é ela que provou que o cabeçalho regravado
leva o byte de sujo ao disco, e é ela que tem de continuar verde.

**Dimensão honesta:** 3,1% não é manchete. A manchete do excluir continua sendo
a busca reversa (**28,4%** medido hoje, e **13,9× o excluir inteiro** com 30
irmãs no diretório). Este item entra porque é **o caminho irmão de um conserto
que já foi feito**, não porque é grande.

---

## 5. O item nº 4: o penhasco da §21 tem causa, e ela é capacidade

`DESEMPENHO.md` §21.2 deixou uma pergunta aberta, e nomeada como aberta:

> «O que ele **não** diz: *por quê*. A suspeita é que as páginas quentes de 15
> árvores deixam de caber no cache. **Efeito medido, causa nomeada e não
> medida.**»

**Fechei essa pergunta, e por um caminho que não exige tocar em código:** se a
causa é a capacidade do cache, o penhasco tem de **andar** quando a tabela
cresce, porque o teto é fixo em 2.048 páginas (`PAGINAS_PADRAO`, `ndx.rs:108`).
Medido agora, `--example custo-da-chave-a-mais`, quatro tamanhos:

| linhas | µs/chave extra antes | µs/chave extra depois | **onde o penhasco cai** |
|---:|---|---|---|
| 12.500 | 0,65 – 1,01 | — | **não aparece até 17 índices** |
| 25.000 | 0,72 – 0,79 | 1,01 | entre **15 e 17** |
| 50.000 | 0,81 – 0,87 | 3,95 | entre **8 e 15** |
| 100.000 | 0,91 – 0,98 | 3,83 | entre **4 e 8** |

**O penhasco anda para a esquerda na medida exata em que a tabela cresce.** Ele
não é propriedade de «15 índices»: é o produto `índices × páginas por índice`
cruzando o teto do cache.

A aritmética fecha. Com chaves `Int8` espalhadas, ck_len = 16, leque = 254, e
ocupação medida de ~76% → ~175 chaves por folha:

| linhas | páginas por índice | índices que cabem em 2.048 | penhasco medido |
|---:|---:|---:|---|
| 12.500 | ~71 | 28 | nenhum até 17 ✓ |
| 25.000 | ~143 | 14 | entre 15 e 17 ✓ |
| 50.000 | ~286 | 7 | entre 8 e 15 ✓ |
| 100.000 | ~575 | 3,6 | entre 4 e 8 ✓ |

**Quatro previsões, quatro acertos.** A causa nomeada em §21.2 está agora
**medida**.

**E é isto que mata a pergunta sobre a política de despejo.** Quando o conjunto
de trabalho excede o cache em 2–4×, **nenhuma política o salva** — LRU-K, 2Q e
CLOCK só decidem *quais* páginas sobrevivem, não quantas cabem. O PostgreSQL usa
**clock sweep**, não LRU (`storage/buffer/README`: *«Each buffer header contains
a usage counter, which is incremented (up to a small limit value) whenever the
buffer is pinned»*) — ou seja, a mesma família que a nossa (`ndx.rs:189`, segunda
chance). A única diferença é que o deles conta até um limite pequeno e o nosso é
um booleano.

**Veredito: trocar a política não é o nosso gargalo.** Os dois alavancas reais
são **capacidade** (`recursos.cache_paginas`, que já é configurável) e
**densidade de página** — que é o item nº 1 e o nº 2 desta matriz. É o mesmo
alvo por outro caminho, e é o que ranqueia o nº 1 em primeiro.

---

## 6. O item nº 5: a hipótese que morreu medida

A técnica é boa e a fonte é primária. PostgreSQL, `storage/buffer/README`:

> «A page that has been touched only by such a scan is unlikely to be needed
> again soon, so instead of running the normal clock-sweep algorithm and blowing
> out the entire buffer cache, a small ring of buffers is allocated. […] For
> sequential scans, a 256KB ring is used.»

E nós temos o padrão que ela descreve: a §19 do `DESEMPENHO.md` é exatamente
«a grade ORDENADA lia o índice inteiro». A hipótese era: *a varredura despeja o
conjunto quente do `inserir`.*

Medida agora, com **controle** (200.000 linhas, 2 índices, lotes de 20.000):

| | µs/linha | contra o quente |
|---|---:|---:|
| A) inserir com o cache quente | 6,54 / 6,12 | 1,00× |
| B) inserir **logo após varrer 220.000 chaves** | 5,64 / 5,37 | **0,86× / 0,88×** |
| C) inserir de novo, **sem varredura no meio** (controle) | 6,40 / 7,01 | 0,98× / 1,15× |

**A hipótese morreu.** Inserir depois da varredura é *mais rápido*, não mais
lento, e o controle C mostra que a dispersão entre lotes (0,98–1,15×) engole
qualquer efeito. Sem o controle C este número teria sido lido como ganho ou
como perda conforme a corrida — é ele que impede a conclusão errada.

**Limite honesto da medição:** a 200.000 linhas o `.ndx` inteiro ainda cabe no
cache do sistema operacional, e o medidor conta **0,00 páginas lidas do arquivo
por linha**. **O que decidiria na bancada:** uma tabela cujo `.ndx` exceda a RAM
da máquina, onde o despejo custa leitura de disco de verdade. Até lá, a recusa
vale para o regime que medimos, e está registrada para não voltar sem número.

---

## 7. O item nº 6: por que a compressão de prefixo não nos compra a árvore

A técnica existe e a fonte diz para que serve. PostgreSQL, `nbtree/README`:

> «The goal of suffix truncation of key attributes is to improve index fan-out.»

Fan-out melhor serve para **baixar a altura da árvore**. A nossa altura, medida
por dentro:

| n | páginas tocadas por linha | ÷ 3 descidas | altura |
|---:|---:|---:|---:|
| 50.000 | 8,10 | 2,70 | ~3 |
| 200.000 | 8,82 | 2,94 | ~3 |
| 1.000.000 | 9,57 | 3,19 | **~3** |

*(Três descidas por linha: a conferência de unicidade mais os dois `inserir`.)*

Com leque de 145–254, a altura 3 comporta de 3 a 16 milhões de chaves e a
altura 4 chega perto de um bilhão. **Dobrar o leque não tira um nível** na
escala em que operamos — o logaritmo é plano demais. E o efeito de cache que a
compressão traria de brinde já está contado nos itens nº 1 e nº 2, que o
compram mais barato (o nº 1 sem mudar formato nenhum).

O que **sobra** de verdadeiro na família é o item **nº 7**: a largura fixa do
`Str(n)`. Um `Str(60)` cobra 146,5 B/linha medidos, e isso não é fan-out — é
desperdício de largura declarada. Mas ele mede junto com a deduplicação e é a
mesma decisão de formato, do mesmo papel C.

---

## 8. O item nº 8: o que sobra na concorrência sem reabrir o que o dono fechou

**A fronteira, respeitada:** a Sombra/MVCC está **parada por decisão do dono**
(`docs/SOMBRA.md`), e a leitura repetível **já entrou em 16/09/2026 pela trava,
pedida** — via (b) do `SOMBRA.md` §5b. Esta frente **não reabre** nem um nem
outro. O que segue é o que sobra depois disso.

O nosso número já está medido (`DESEMPENHO.md` §14), com controle:

- A trava come **~20% do paralelismo na leitura e ~25% na escrita**, já com dois
  clientes e metade da máquina ociosa.
- **Não há um segundo gargalo embaixo dela:** clientes em tabelas separadas
  escalam igual aos da mesma tabela (1,70 contra 1,67 na leitura; 1,43 contra
  1,45 na escrita). A folga de ~2× está inteira atrás da trava.
- A leitura segura a trava **20× mais tempo** por operação que a escrita — o que
  *favorece* separar leitor de escritor, e «favorecer não é medir».

**E o fonte diz uma coisa que o §14 não registra:** o `RwLock` **já está lá**.
`crates/phxsql-server/src/servidor.rs:647` é `dados: RwLock<Raiz>`, e o
comentário acima dele explica o desenho — `&Raiz` alcança só tabela de leitura,
`&mut Raiz` alcança a `Instancia` inteira. Ou seja, a peça existe e a questão
real é **quantos caminhos de leitura hoje pegam o empréstimo exclusivo sem
precisar**. Isso é uma contagem, não um projeto.

**Veredito: real, e é o item mais barato de investigar de toda a matriz**,
porque a estrutura já está construída. **O que decidiria:** contar, caminho por
caminho, quais operações de leitura tomam `&mut Raiz` e quais poderiam tomar
`&Raiz`, e re-rodar `bancada/concorrencia/a-trava-serializa.py`. Nenhuma pétrea
é tocada: não é MVCC, é granularidade de empréstimo.

---

## 9. O item nº 13: skip list, ART e learned index

As três resolvem o mesmo problema, e não é o nosso. São estruturas **de
memória**: a skip list é a memtable do RocksDB e do Cassandra; a ART é índice
de memória (HyPer, DuckDB); o learned index troca a busca por um modelo
ajustado aos dados.

O nosso `.ndx` é **paginado em disco**, com CRC-32 por página, marca de sujo no
cabeçalho e reconstrução a partir do `.reg`. Trocar a estrutura significaria
refazer as quatro coisas — e a medição diz que não há prêmio: a busca dentro da
página (`lower_bound_folha`, binária sobre entradas de largura fixa) **não
aparece** no perfil. O que aparece é **cópia de página** (8–13%) e **gravação**,
e nenhuma das três técnicas ataca isso.

**Veredito: não se aplica** — resolvem busca em RAM, e a nossa conta é de
páginas.

---

## 10. As lacunas — o que esta pesquisa NÃO conseguiu medir ou conferir

1. **SQLite não foi conferido em fonte primária.** `sqlite.org/src` respondeu
   com verificação anti-robô e o espelho do GitHub truncou o `btree.c` antes do
   `balance_quick`. *Documentação não é acesso concedido.* O peso ponderado já
   soma **9** sem ele (PG 4 + MariaDB 3 + MySQL 2), então o veredito do item nº
   1 não depende disso — mas a conferência fica em aberto.
2. **O ganho do item nº 1 é raciocinado, não medido.** Sei a ocupação de hoje
   (46,7%) e o que o formato comporta; **não medi** o µs/linha depois da
   mudança, porque medir exigiria implementar, e esta frente não implementa.
3. **A segurança de tirar o `gravar_cabecalho` do `remover`** é raciocinada do
   fonte (§4), não provada. A prova é a bancada da tomada do pedido 253.
4. **A recusa do ring buffer (§6) vale só no regime medido** — `.ndx` menor que
   a RAM. Fora dele, está por medir.
5. **Deduplicação e chave variável não têm número de *ganho*** — têm número de
   *desperdício* (§3). O ganho depende do desenho, e o desenho é do papel C.
6. **Fan-out por subagente não aconteceu:** esta sessão não recebeu a ferramenta
   de convocação (`pesquisa-motor`, `pesquisa-bancada`). Tudo acima foi medido e
   conferido por esta frente, o que torna as lacunas 1 e 2 mais pesadas do que
   seriam com as duas frentes de apoio.

---

## 11. A recomendação

**Em ordem de ganho medido contra o nosso gargalo:**

1. **Divisão enviesada à direita no `inserir_folha`** — o único item da matriz
   que é ganho grande, **sem mudança de formato**, com convergência de peso 9 e
   **nenhuma pétrea contra**. Ocupação medida hoje: 46,7% onde cabe ~90%. Ataca
   ao mesmo tempo o tamanho do `.ndx`, a capacidade efetiva do cache e o
   penhasco da §5. É a recomendação principal, e entra pela porta do papel B com
   a bancada do papel F atrás.
2. **O cabeçalho fora do `remover`** (§4) — 0,96 µs de 30,65 medidos, 3,1%.
   Pequeno, mas é **caminho irmão de um conserto já feito**, e a lei desta casa
   diz que é exatamente esse o defeito que ninguém acha por leitura. Vai junto
   com a prova da tomada do 253.
3. **Contar os caminhos de leitura que tomam `&mut Raiz`** (§8) — a peça já
   existe no `servidor.rs:647`; é contagem antes de projeto, e há ~20% de
   paralelismo medido em cima da mesa. Não reabre a Sombra.
4. **Parecer do DBA sobre deduplicação e chave de largura variável** (§3) — o
   desperdício está medido (um índice `Str(60)` custa 1,44× os dados). É mudança
   de formato, e *mudança de formato entra cedo*.

**O que NÃO deve voltar sem número novo:** troca de política de despejo (§5 — é
capacidade, não política), ring buffer para varredura (§6 — hipótese morta
medida, 0,86×), compressão de prefixo para baixar a árvore (§7 — a altura já é
3), prefetch (item 15 — zero páginas lidas do arquivo), skip list/ART/learned
index (§9 — resolvem busca em RAM) e LSM (item 16 — quebra a ordem de digitação).

**E o aviso que vale mais que a lista:** quem planejar contra os «83,5% no
`.ndx`» estará mirando um número que já morreu duas vezes. Hoje são **43–52%**,
medidos nesta máquina, nesta data.

---

## Como refazer as medições deste documento

```bash
cd /home/user/adrianoboller/phxsql
cargo build --release --examples -p phxsql-store    # binario velho mede o passado

# §0 -- a divisao do inserto, tres tamanhos
./target/release/examples/onde-doi 50000
./target/release/examples/onde-doi 200000
./target/release/examples/onde-doi 1000000

# §4 -- o excluir dividido, com as chamadas contadas pelo nucleo
./target/release/examples/custo-do-excluir 200000 20000

# §5 -- o penhasco andando com o tamanho da tabela
for n in 12500 25000 50000 100000; do
  ./target/release/examples/custo-da-chave-a-mais $n
done
```

As medições de **ocupação de página** (§2.1), **cardinalidade e largura** (§3),
**custo do cabeçalho** (§4) e **poluição do cache** (§6) saíram de instrumentos
próprios desta frente, escritos fora do repositório (só `std` e as duas crates
da casa, por caminho). Eles **não foram versionados**: se algum destes itens
virar trabalho, o medidor correspondente deve nascer como `--example` no
`phxsql-store`, que é onde ele não morre com a sessão.

---

## 12. Aviso ao integrador: outra frente mexeu no caminho medido, DURANTE a medição

Durante esta pesquisa, outra frente alterou `crates/phxsql-store/src/table.rs`
(mtime 16:35:46) e `crates/phxsql-store/examples/custo-do-excluir.rs`
(16:31:27) — **exatamente o caminho do excluir que a §4 mede**. Pelo diff, ela
está implementando o **item 3 da §24.5** do `DESEMPENHO.md` («a linha lida três
vezes», 4,34 µs, 15,5%), pondo um portão antes de ler a linha.

O que isso faz com os números deste documento:

- **Não afeta** §2 (ocupação), §3 (cardinalidade e largura), §5 (o penhasco) nem
  §7 (altura da árvore): todos saem do `ndx.rs`, que **não foi tocado** — não
  aparece em `git status`, e o meu binário foi construído do mesmo `ndx.rs`
  que está comitado.
- **Afeta o denominador da §4.** Os **30,65 µs** do excluir foram medidos com o
  binário de **antes** dessa mudança. Se a frente vizinha comprar os ~2,8 µs que
  o item 3 promete, o excluir cai para ~27,8 µs e o cabeçalho do `remover`
  passa de **3,1% para ~3,5%** — o item fica *mais* significativo, não menos, e
  a recomendação não muda de ordem.
- **A §6 (poluição do cache) é medida de tempo** e foi tomada nessa janela. O
  efeito era nulo e o controle estava no mesmo processo, então a conclusão se
  sustenta; o número exato, não.

**Quem integrar tem de re-rodar o `custo-do-excluir` depois que a frente vizinha
pousar**, porque a §4 e a §24.4 passarão a falar de dois excluires diferentes
com o mesmo nome. É o caso clássico desta casa: *há defeito que só aparece no
encontro das frentes.*
