# Desempenho da escrita: onde dói, e o que as propostas comprariam

Documento de **medição**, não de opinião. Cada número aqui sai de um programa
que está no repositório e que qualquer pessoa roda de novo.

---

## 1. A resposta curta

> **O tempo estava no `.ndx`, e estava no CRC-32 de página inteira.** Cada
> inserção descia a B+tree relendo do arquivo as mesmas páginas — a raiz, a
> mesma para todas —, e cada leitura passava 4 KiB pelo CRC. Um cache de
> páginas de leitura tirou isso do caminho: **44,4 → 18,5 µs por linha, 2,4×**,
> sem mudar formato, sem mudar garantia e sem tocar na árvore. Depois dele, o
> cabeçalho que reserializava o esquema a cada linha (§2.0) levou a 17,0, e o
> cabeçalho do diário que ia a disco a cada evento (§2.2) levou a **15,9 µs** —
> **2,79× no total**.

A receita clássica para acelerar escrita («tire o `fsync` do caminho crítico»)
foi escrita para motores cujo gargalo é o `fsync`. O do PhxSql não era, e havia
medida para isso: na bancada de 10 milhões de linhas, o processo gastou **870 s
de CPU para 884 s de relógio (98%) e leu 0,0 MiB**. Ele passava o tempo inteiro
*calculando* — e agora se sabe calculando o quê.

Na mesma bancada depois do cache: **289 s de CPU para 303 s de relógio (95%),
0,0 MiB lidos**. Continua sendo CPU, e continua não sendo disco — só que agora
é três vezes menos CPU.

Depois do cache a divisão mudou de lugar: o `.ndx` caiu de **83,5% para 63,6%**
do tempo de uma inserção, e o `.reg` + `.log` subiu de 16,5% para 36,4% — não
porque ficaram mais lentos, mas porque o outro lado encolheu.

---

## 2. Onde o tempo vai, fator por fator

```bash
cargo run --release --example onde-doi -- 200000
```

Mesma tabela, mesmas linhas, esquemas diferentes. A conta de cada parcela sai
da subtração:

| Esquema | antes | + cache de páginas | + cabeçalho do `.reg` | + cabeçalho do `.log` | ganho |
|---|---:|---:|---:|---:|---:|
| só `.reg` (sem índice nenhum) | 7,3 µs | 6,7 µs | 5,4 µs | **4,8 µs** | 1,52× |
| + 1 índice comum | 21,5 µs | 12,2 µs | 10,9 µs | **10,2 µs** | 2,11× |
| + o mesmo índice, agora único | 30,6 µs | 12,6 µs | 11,2 µs | **10,5 µs** | 2,91× |
| + 2 índices (a forma da bancada) | 44,4 µs | 18,5 µs | 17,0 µs | **15,9 µs** | **2,79×** |

| Parcela | antes | % | agora | % |
|---|---:|---:|---:|---:|
| `.reg` + `.log` | 7,3 | 16,5% | 4,8 | 30,3% |
| <span>↳ só o `.log` (§2.2)</span> | — | — | 0,67 | 4,2% |
| primeiro índice | 14,2 | 32,0% | 5,4 | 33,9% |
| conferir a chave única | 9,1 | 20,5% | 0,3 | **1,9%** |
| segundo índice | 13,8 | 31,0% | 5,4 | 34,0% |
| **total** | **44,4** | 100% | **15,9** | 100% |

(As três colunas de cima são três medições de três *builds*, cada uma com três
corridas — a primeira corrida depois de compilar sai contaminada pelo próprio
compilador e não conta.)

### 2.0 O cabeçalho que reserializava o esquema por linha

Achado respondendo «e se o `.ndx` parasse durante a carga?»: toda inserção
chamava `gravar_cabecalho`, e ele fazia **cinco coisas, das quais uma era
necessária**:

1. serializar o **esquema inteiro** — que não muda desde que a tabela foi criada;
2. calcular o **CRC-32 desse bloco**;
3. gravar os 128 bytes do cabeçalho, com os contadores — *esta* é a necessária;
4. gravar o **bloco de esquema outra vez**, byte a byte igual ao que já estava lá;
5. perguntar o **tamanho do arquivo** para ver se precisava esticar.

O esquema é imutável depois da criação: passou a ser serializado uma vez, no
construtor, com o CRC junto. E o caminho quente ganhou um irmão que grava **só o
cabeçalho** — o bloco de esquema e o teste de tamanho ficaram onde importam, na
criação do volume.

| | antes | depois | |
|---|---:|---:|---:|
| só `.reg` | 6,7–6,9 µs | **5,2–5,4 µs** | **1,27×** |
| com 2 índices | 18,3–19,0 µs | **17,0–17,1 µs** | 1,08× |

A linha que mais mudou diz o que aconteceu: **conferir a chave única caiu de
20,5% para 2,3%**. Essa conferência é uma descida na árvore que não escreve
nada — exatamente o trabalho que o cache serve de graça. Ela não ficou mais
esperta; ela parou de reler do arquivo o que já estava em RAM.

A tabela sem índice nenhum quase não mudou (1,09×), e é o controle da
experiência: o `.reg` não usa página de `.ndx`.

### A conta que não fechava, e agora fecha

A versão anterior deste documento registrava uma **pista aberta**: o medidor
estimava, por um `strace`, ~20 toques de página por linha, e o CRC-32 de uma
página de 4 KiB custa 2,34 µs. Vinte toques dariam ~47 µs só de CRC — mais que
os 44,4 µs medidos no total. A conta não fechava.

Ela não fechava porque o número de toques era **citado**, e não medido. O
medidor agora **conta** os toques dentro do `.ndx`, e o próprio cache é quem
conta:

```
paginas servidas pelo cache ....... 8,80 por linha
paginas lidas do arquivo .......... 0,00 por linha
paginas gravadas .................. 2,06 por linha
```

São **10,86** toques por linha, e não 20. Antes do cache, os 10,86 passavam
todos pelo CRC: 10,86 × 2,34 = **25,4 µs**, de 44,4 medidos — 57% do tempo de
uma inserção era CRC-32 de página. Depois, só as 2,06 gravações pagam: 4,8 µs
de 15,9 (30%).

O acerto de cache custa a **cópia** da página, não o CRC dela. É daí que veio o
2,4×.

E a piora com o tamanho, que confirmava o diagnóstico, continua confirmando: na
carga de 10 milhões, o primeiro milhão entrava a 16.051/s e o décimo a 9.311/s.
É a árvore crescendo para além do que cabe em RAM — o cache adia esse ponto, não
o elimina.

### O cache, em uma tela

- **É de leitura.** Toda gravação atravessa para o arquivo na hora. Segurar
  página suja daria mais e trocaria uma garantia por desempenho **sem avisar**:
  hoje uma queda do *processo* não atrasa o `.ndx` em relação ao `.reg`, porque
  o `write` já entregou a página ao núcleo. Só uma queda da *máquina* faz isso.
- **A página recém-gravada fica.** É o que mais rende numa carga: a folha que
  acabou de receber uma chave é quase sempre a que vai receber a próxima.
- **Despejo por segunda chance (CLOCK).** Fila simples não serviria — a raiz, a
  página mais visitada de todas, sairia junto com as outras assim que o teto
  enchesse.
- **Teto de 2.048 páginas** = 8 MiB por `.ndx` aberto. O número saiu de uma
  varredura, e não do chute (§2.1).

### 2.2 O `.log` não atrasa o `.reg`

A tabela acima media `.reg` e `.log` **juntos**, e este documento registrava
isso como um bloco não decomposto. Ele foi decomposto, e a decomposição virou
uma mudança.

```bash
cargo run --release --example custo-do-log -- 200000
```

O `.log` já era enxuto — não reserializa nada, e o cabeçalho fica em cache na
leitura. O que ele fazia de sobra eram **duas escritas por evento**: os 44 bytes
do evento, e os 64 bytes do cabeçalho com `fim` e `qtd_eventos`.

O evento **tem** de ir na hora. O cabeçalho é um contador — e a leitura sabe
recalculá-lo varrendo os próprios eventos. Ele passou a ir no `sincronizar`:

| | antes | depois | |
|---|---:|---:|---:|
| `.log` por evento, sem imagem | 1,22 µs | **0,67 µs** | 1,82× |
| `.log` por evento, com a imagem da linha | 2,24 µs | **1,61 µs** | 1,39× |
| inserção completa, 2 índices | 17,0 µs | **15,9 µs** | 1,06× |

### O que isso custou, e o que não custou

**Não custou o evento.** Ele continua indo para o arquivo dentro da inserção,
antes de a operação terminar. O que ficou para depois foi o *contador*.

**Custou um caminho de reparo**, e ele é a parte que valia escrever com cuidado.
Uma queda antes do `sincronizar` deixa o cabeçalho atrasado em relação aos
eventos que já estão no arquivo — e, sem cura, a próxima gravação escreveria
**por cima** deles. Não seria evento invisível: seria evento destruído.

Então `abrir` varre para a frente a partir do `fim` gravado, validando cada
evento pelo **CRC que ele já carrega**, e para no primeiro que não confere ou no
fim do arquivo. A varredura é limitada ao que entrou desde o último
`sincronizar` — uma janela de centenas de eventos. Região zerada não passa: o
CRC-32 de 36 bytes zerados não é zero.

Quatro testes travam isso, e o que mais importa é
`depois_da_cura_o_novo_evento_nao_sobrescreve`.

### O que continua fora, e por quê

Guardar os **eventos** em RAM compraria os 0,67 µs restantes — 4,2%. Não foi
feito, e a razão não é de tamanho, é de natureza:

> Índice perdido se reconstrói do `.reg` com `reindexar`. **Evento perdido não
> se reconstrói.** Ele é a história, com carimbo de hora e autor, e é a posição
> de que a replicação depende — uma réplica pularia a linha em silêncio.

### 2.3 O Profiler desligado custava 7% da carga

Não estava no `onde-doi` porque não é do motor: é do **servidor**, e só aparece
quando o pedido passa pela porta. A bancada da carga em lote é que mostrou.

O ponto de captura ficava assim:

```rust
let marca = {
    let alvo = objeto_do_pedido(&linha, ..);   // Json::analisar #1 + 2 String
    let nome_op = Json::analisar(&linha)..;    // Json::analisar #2 + 1 String
    self.profiler.lock().ok().and_then(|mut p| p.chegou(..))
    //           ^ mutex por pedido        ^ e SÓ AQUI ele olha `ligado`
};
```

Com o Profiler **desligado**, todo pedido pagava dois `Json::analisar` do corpo
inteiro, três `String` e um mutex — para no fim `chegou` devolver `None`. Num
`inserir_lote` de 5.000 linhas isso é analisar meio megabyte de JSON **duas
vezes, para nada**.

O portão passou a ser um `AtomicBool` lido com `Relaxed`, antes de qualquer
trabalho:

| carga em lote pela rede, Profiler desligado | linhas/s |
|---|---:|
| antes | 40.597 · 40.653 |
| depois | **43.612 · 43.302** |

**1,07×** — dois pares de corridas, o mesmo binário sem mais nada mudando.

A regra que fica: **instrumentação desligada tem de custar zero, e o portão que
decide isso vem antes do trabalho, não depois.**

### Qual das duas coisas custava

```bash
cargo run --release --example quem-custava
```

| | custo |
|---|---:|
| um `lock`/`unlock` sem disputa | **13,2 ns** |
| `Json::analisar` de 1 linha (140 B) | 1,44 µs |
| `Json::analisar` de 5.000 linhas (304 KB) | **3.456 µs** |

Por lote de 5.000 linhas, o ponto de captura pagava **6.912 µs de parse contra
0,03 µs de lock** — o parse custava 262.000× o mutex.

**Não era o mutex.** Numa primeira redação deste documento eu escrevi que ele
era «o pior pedaço, porque serializa». A segunda parte é verdade sobre mutex em
geral e **não era verdade aqui**, por dois motivos: sem disputa ele custa
nanossegundos, e neste servidor toda operação de dado já se serializa na trava
global — que é tomada *depois* e segurada por muito mais tempo. O mutex do
profiler nunca foi o gargalo de concorrência.

O que custava era analisar meio megabyte de JSON **duas vezes para jogar fora**.
Foi por isso que a carga em lote melhorou 7% e o caminho linha a linha quase não
se moveu: lá o corpo tem 140 bytes.

O que o `AtomicBool` custa em troca é uma janela de um pedido: quem liga a
observação pode não ver o que já estava em voo. Ligar a observação no meio de um
pedido não promete pegar aquele pedido — promete pegar os próximos.

### 2.3.1 A conferência de 2026-08: o desligado custa mesmo zero?

O número de 7% acima foi tirado numa corrida e nunca refeito. Refazer achou
duas coisas: o portão continua valendo, e a **forma de medir** estava frágil.

A primeira tentativa rodou uma variante inteira, depois a outra, e comparou as
medianas. O resultado foi **impossível** — o Profiler *ligado* apareceu 1,21×
mais rápido que desligado. A causa não estava no servidor: outro processo
ocupava os mesmos quatro núcleos durante metade da corrida. Uma corrida de
cinco minutos não é uma condição; são cinco minutos de condições diferentes.

O medidor passou a ser **emparelhado** (`bancada/profiler/custo.py`): as duas
variantes ficam no ar ao mesmo tempo, em duas portas, e o trabalho é picado em
pedaços curtos que se alternam entre elas, trocando a ordem a cada volta. Um
pico de carga cai nos dois lados do par. O que se reporta é a **mediana das
razões**, com o menor e o maior ao lado.

Três binários, com a mesma árvore e só o portão trocado — `Relaxed`, `false` e
`true` —, 15 pares cada, 5.000 linhas por pedaço em lote e 1.500 uma a uma:

| comparação | em lote | uma a uma |
|---|---:|---:|
| **desligado × sem profiler nenhum** | **0,99×** (0,72–1,09) | **1,00×** (0,86–1,12) |
| portão barato × defeito da 0.17.0 | **1,13×** (0,94–1,25) | 1,02× (0,97–1,14) |
| ligado (anel 500) × desligado | 0,80× (0,75–1,03) | 0,92× (0,83–1,08) |

Lidas em ordem:

1. **O Profiler desligado custa zero.** Contra um binário em que o ponto de
   captura foi compilado fora (`if false`), a diferença é 0,99× e 1,00× — está
   dentro da largura do ruído, que nesta máquina foi de ±25% entre o melhor e
   o pior par. O `AtomicBool` cumpre o que promete. A afirmação que este
   número sustenta é «não dá para distinguir de zero», e não «é exatamente
   zero»: com esta largura de ruído, um custo abaixo de ~5% no lote passaria
   despercebido — e é bom lembrar disso antes de citar o 0,99× como se fosse
   uma medida fina.
2. **O portão vale 1,13× na carga em lote e quase nada linha a linha** — que é
   exatamente a forma que a explicação da 0.17.0 previa: o que custava era
   analisar o corpo, e o corpo do lote tem 300 KB contra 140 B do pedido
   avulso. Os 7% de então viraram 13% aqui; a máquina é outra e estava
   ocupada, e o que confirma a explicação é **o sinal e a forma** — o ganho
   está no lote e não no pedido avulso —, não a terceira casa.
3. **Ligar custa 20% no lote e 8% linha a linha.** Não é grátis, e não deveria
   ser: com o Profiler ligado o corpo é analisado, redigido e reserializado
   para o anel. É o preço de ver o texto, e ele só se paga quem pediu.

### 2.3.2 O anel se procurava do lado errado

Medir o custo do Profiler **ligado** deu um número grande demais para o que a
conta previa: na carga uma a uma o corpo tem 140 B e o `Json::analisar` dele
custa 2,07 µs, mas a diferença medida com `guardar: 20000` era de **103 µs por
linha**. Cinquenta vezes o que o parse explicava.

O que faltava não era parse: era o `terminou`. Ele acha o evento pelo serial
para costurar nele a duração e o desfecho — e procurava **do mais antigo para
o mais novo**, com `VecDeque::iter_mut().find(...)`. O evento procurado é
quase sempre o **último** que entrou: `chegou` empurra atrás, e o desfecho
chega logo depois. Com o anel cheio de 20.000, eram 20.000 comparações por
pedido para achar o que estava na ponta.

Emparelhado, com e sem `.rev()` (`bancada/profiler/custo-anel.py`):

| busca invertida, na carga uma a uma | razão |
|---|---:|
| anel de **500** (o padrão da tela) | 1,00× (0,84–1,10) |
| anel de **20.000** | **1,17×** (0,93–1,62) |

Na carga em lote as duas dão 1,00×, e tinha de dar: quatro pedidos de 5.000
linhas fazem quatro varreduras, não vinte mil. O ganho mora onde há **muitos
pedidos**, que é onde um profiler ligado dói.

O 1,17× é medido **por baixo**: o anel começa vazio em cada par e só alcança
as 20.000 entradas no meio da corrida, então boa parte dos pares foi medida
com um anel menor que o do caso ruim.

Quem sobe o teto do anel para investigar um problema é justamente quem tem
tráfego — e era ali que a instrumentação ficava mais cara **quanto mais
memória se desse a ela**. Uma palavra de conserto: `.rev()`.

A lição é velha e apareceu num lugar novo: **o número que não fecha com a conta
é o número que tem mais a dizer.** A conta do parse explicava 2 µs; a medição
mostrava 103. Aceitar o «bem, ligar custa caro» teria enterrado o achado.

### 2.1 De quanto tem de ser o teto

```bash
cargo run --release --example ordem-da-chave -- 200000
```

Mesmas 200.000 linhas, dois índices, mudando só o teto do cache. A coluna da
direita é a mesma carga com as chaves **embaralhadas**, que é o caso comum de
quem importa de outro sistema:

| Teto | RAM | chaves crescentes | chaves embaralhadas |
|---|---:|---:|---:|
| sem cache | — | 43,5 µs | 46,0 µs |
| 512 páginas | 2 MiB | 18,0 µs | 25,3 µs |
| 1.024 páginas | 4 MiB | 18,2 µs | 23,2 µs |
| **2.048 páginas** | **8 MiB** | **17,9 µs** | **21,3 µs** |
| 4.096 páginas | 16 MiB | 17,8 µs | 20,5 µs |

2.048 é o joelho: dobrar de novo compra 0,8 µs e custa mais 8 MiB por tabela
aberta. O servidor abre e fecha a tabela a cada operação, então esse teto vale
enquanto a operação dura — e a operação que importa aqui, a carga em lote,
insere milhares de linhas dentro de uma única abertura.

---

## 3. As dez propostas, uma a uma

A avaliação abaixo é da arquitetura LSM/WAL clássica (RocksDB, InnoDB tunado)
aplicada ao PhxSql. **Ela é uma boa receita — para o problema que ela descreve.**
Cinco itens já existem aqui, dois miram um gargalo que o PhxSql não tem, um
quebraria o formato, e **dois são reais**.

| # | Proposta | Estado no PhxSql | Veredito |
|---:|---|---|---|
| 1 | WAL exclusivamente sequencial | O `.reg` **já é** *append-only*: `rowid = slots + 1`, endereço por multiplicação, nenhuma página reescrita | **Aponta para o arquivo errado.** Um WAL existe para transformar escrita aleatória de página em sequencial. Não há escrita aleatória no `.reg` — há no `.ndx` |
| 2 | MemTable em RAM | Existe `TabelaMemoria`/`SelectMemory` (87× medido), mas é cache de **leitura** | **Meia peça, do outro lado.** Como buffer de escrita ajudaria o `.ndx` |
| 3 | Single writer + fila MPSC | O servidor **já** serializa tudo numa trava global única | **Já é assim** — e o roteiro quer o contrário: trava por tabela. O gargalo de concorrência é o excesso de serialização, não a falta |
| 4 | Três modos de durabilidade | Existem, com esses três nomes: `por_operacao`, `por_lote`, `sistema` | **Já existe, e medido:** 1.289 → 18.264 → 24.858 → 26.301 linhas/s (20,4×) |
| 5 | Não atualizar índice secundário na hora | Todos os índices são mantidos dentro da inserção | **REAL, e é o maior.** Ver §4 |
| 6 | UUID v7 ou sequência, nunca v4 | `Uuid` v4/v7 (RFC 9562), `Uuid256` e `Sequence` prontos; o dossiê tem uma seção sobre por que v7 | **Já existe** |
| 7 | Não alterar o arquivo principal no INSERT | O `.reg` só anexa. Sem *double-write*, sem divisão de página no arquivo de dados | **Já é assim** |
| 8 | Segmentos imutáveis, SSTable, compactação | — | **Incompatível.** Ver §5 |
| 9 | Buffers grandes em vez de escritas pequenas | Escreve por slot; são 2,06 páginas de `.ndx` gravadas por linha, medidas | **Medido, e é pequeno.** Um `lseek` custa 0,10 µs: mesmo 41 chamadas por linha dariam 4,0 µs de 15,9. O que custa nessas gravações é o **CRC** (4,8 µs), não a chamada |
| 10 | Pré-alocar o WAL | Os volumes crescem conforme escrevem | **Aplicável aos volumes**, ganho provavelmente pequeno pela mesma razão do item 9 |

---

## 4. O item que vale: o índice fora do caminho crítico

É o único da lista que a medição sustenta, e o número é grande:

| Se sair do caminho crítico | µs por linha | ganho |
|---|---:|---:|
| nada (hoje) | 15,9 | — |
| o segundo índice | 10,5 | 1,51× |
| os dois índices e a conferência | 4,8 | **3,31×** |

(Os mesmos números antes do cache de páginas eram 44,4 / 30,6 / 7,3 — ganho de
1,45× e 6,1×. O cache já cobrou boa parte do que adiar o índice cobraria, e o
teto de 6,1× virou 3,15×. E §4.2 mostra que esse teto **não se realiza** com o
`reindexar` de hoje.)

**Mas há uma linha que não dá para cruzar, e ela é do formato.** A conferência
de unicidade acontece **antes de qualquer escrita**, e não depois — porque o
`.reg` nunca reaproveita slot. Uma inserção recusada *depois* de gravar deixaria
um buraco permanente, e uma tabela que recebe muita chave repetida iria inchando
sem nunca crescer.

Então a proposta se divide em duas, com veredito diferente:

- **Índice NÃO único, adiado** — seguro. A chave entra numa fila e um
  trabalhador de fundo a insere. Nada depende dela para decidir se a linha
  entra. Ganho medido do segundo índice: **1,45×**.
- **Índice ÚNICO, adiado** — não. Ele é a própria decisão de aceitar ou recusar
  a linha. Adiá-lo é aceitar gravar primeiro e descobrir depois, que é
  exatamente o buraco permanente. Fica.

Há um terceiro caminho, que ninguém propôs: **manter o índice no caminho
crítico, mas em lote** — ordenar as chaves antes de descer a árvore, para que
chaves vizinhas caiam na mesma folha. Era o item que este documento colocava em
primeiro lugar. **A medição mudou o veredito, e a ordem.**

### 4.1 Ordenar as chaves do lote: o que a medição disse

```bash
cargo run --release --example ordem-da-chave -- 200000
```

As mesmas linhas, os mesmos índices, mudando só a **ordem** em que as chaves
chegam. A diferença entre as duas é o **teto** do que ordenar o lote pode
recuperar:

| Forma | crescentes | embaralhadas | a desordem custa |
|---|---:|---:|---:|
| 1 índice único, chave inteira | 12,5 µs | 13,5 µs | 1,08× |
| 1 índice único, chave de texto | 12,6 µs | 13,9 µs | 1,10× |
| 2 índices (a forma da bancada) | 17,9 µs | 21,3 µs | **1,19×** |

**Antes do cache de páginas, a desordem custava 1,06×** — e ordenar teria
comprado praticamente nada. A hipótese que colocava este item em primeiro lugar
estava certa sobre o alvo (o `.ndx`) e errada sobre o mecanismo: o custo não era
de *localidade*, era de **reler e recalcular CRC da mesma página**. Com tudo em
RAM, mudar a ordem de chegada não mudava nada.

Depois do cache, a localidade finalmente importa — e vale 1,19× na forma da
bancada.

**Não implementado, e a razão está no formato.** Ordenar as chaves de um lote
exige conhecer os rowids antes de inserir no `.ndx`, o que exige gravar o `.reg`
antes — e aí uma falha no meio da fase do índice deixa linhas gravadas sem
chave, sem como desfazer (o `.reg` não reaproveita slot). Hoje uma falha de
índice desfaz a linha inteira. Trocar isso por «rode `reindexar`» é rebaixar uma
garantia rara, mas real, por 1,19%… por **1,19×**. Fica registrado com o número,
para a decisão ser tomada com ele na mão e não sem ele.

**O que dá para fazer hoje, de graça:** quem importa um arquivo **já ordenado
pela chave primária** carrega 1,19× mais rápido. É uma linha de documentação,
não de código.

### 4.2 Parar o `.ndx` durante a carga e reconstruir no fim

A ideia é a mais tentadora da lista: durante uma carga, deixar o índice
parado — o `.reg` sozinho insere a **148 mil linhas/s** contra 54 mil com dois
índices — e reconstruir tudo de uma vez no fim.

```bash
cargo run --release --example indice-adiado -- 200000
```

Com a reconstrução **dentro da conta**, que é onde ela tem de estar:

| 200.000 linhas, chaves embaralhadas | inserir | reindexar | total | ganho |
|---|---:|---:|---:|---:|
| hoje (os dois índices na hora) | 3,93 s | — | 3,93 s | — |
| adiar **os dois** | 1,25 s | 2,54 s | 3,79 s | **1,02×** |
| adiar **só o não único** | 2,72 s | 1,19 s | 3,91 s | 1,01× |

**Um por cento.** E a razão está em três linhas de `Table::reindexar`:

```rust
while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
    let chave = self.codificar_chave(i, &valores)?;
    self.ndx.inserir(i, &chave, id)?;   // ← uma descida na árvore por chave
}
```

Reconstruir hoje é **chave a chave** — o mesmo trabalho do caminho de dentro,
feito depois. Adiar não apaga trabalho: move de lugar. E como o trabalho movido
é idêntico, o total não muda.

### O ganho existe, e está no lote — não no adiar

Uma reconstrução **em lote** de verdade seria outra coisa: varrer o `.reg` uma
vez, codificar as chaves, **ordenar**, e encher as folhas em sequência montando
os níveis de cima por cima. Nenhuma descida. O piso disso, medido:

| | |
|---|---:|
| varrer o `.reg` e codificar 200.000 chaves (uma vez, serve aos dois índices) | 0,21 s |
| ordenar as chaves, por índice | 0,03 s |
| páginas a encher, em sequência | 4.047 |

Contra os **2,54 s** que o `reindexar` de hoje cobra pelos dois.

**Então a ordem de trabalho é a inversa da intuição:** primeiro a construção em
lote da B+tree, depois o adiamento. Adiar sem ela compra 1%; a construção em
lote sozinha já acelera todo `reindexar` e todo reparo, sem mexer no caminho de
escrita.

### 4.3 A construção em lote, feita — e o que ela mudou

`NdxFile::construir_em_lote` não desce a árvore nenhuma vez: ordena as chaves,
enche as folhas em sequência e monta os níveis de cima por cima dos de baixo.
Um milhão de chaves, `--example indice-em-lote`, duas corridas:

| | montar | páginas | varrer |
|---|---:|---:|---:|
| uma a uma (o `reindexar` de antes) | 7,72 s | 6.136 | 0,036 s |
| **em lote** | **0,31 s** | 5.271 | 0,028 s |

**23× a 25×.** E com o `reindexar` barato, o `--example indice-adiado` passou a
dizer outra coisa: adiar os dois índices vale **3,28×** (era 1,02×), e adiar só
o não único — o caminho que não abre mão da unicidade — vale **1,59×**.

O **enchimento das folhas** foi medido, e não herdado. 70% é a folga clássica e
não compra nada, porque inserção aleatória já assenta perto de 69% de ocupação
sozinha:

| enchimento | páginas | varrer | crescer 10% | páginas novas |
|---:|---:|---:|---:|---:|
| 70% | 6.028 | 0,035 s | 0,804 s | 0 |
| **80%** | **5.271** | **0,028 s** | **0,770 s** | **0** |
| 90% | 4.683 | 0,026 s | 0,901 s | 2.342 |
| 100% | 4.213 | 0,023 s | 0,984 s | 2.110 |

De 90% para cima a folha fica sem folga: crescer aloca milhares de páginas e
fica **mais lento** do que na árvore mais frouxa, e a varredura mais rápida não
paga isso. 80% é a ocupação mais densa que ainda absorve 10% de crescimento sem
alocar uma página.

> A primeira versão desse medidor deu 100% de graça, porque as chaves de
> crescimento entravam **acima** da faixa — e chave maior que todas vai sempre
> para a última folha, então a divisão que o enchimento deveria provocar nunca
> acontecia. Medidor com furo mede o furo.

### 4.4 E o adiamento em si: medido, e ele quase nunca compensa

Com o lote pronto, o adiamento virou item de implementar. **Medi antes**, e o
número o derrubou.

O 1,59× do `indice-adiado` é o caso da tabela **vazia**. Mas `reindexar`
reconstrói sobre a tabela **inteira**, e não sobre as linhas que acabaram de
entrar. Para uma tabela com 200.000 linhas, carregando M
(`--example adiar-vale-quando`):

| M | manter o índice | adiar (carga + refazer) | ganho |
|---:|---:|---:|---:|
| 200.000 (dobra a tabela) | 3,255 s | 2,149 + 0,512 = 2,662 s | **1,22×** |
| 100.000 | 1,620 s | 1,075 + 0,403 = 1,477 s | 1,10× |
| 40.000 | 0,653 s | 0,441 + 0,315 = 0,757 s | **0,86×** |
| 20.000 | 0,330 s | 0,219 + 0,286 = 0,505 s | 0,65× |
| 4.000 | 0,067 s | 0,044 + 0,264 = 0,308 s | 0,22× |

O ponto de virada fica perto de **M ≈ N/3**. Abaixo dele adiar **custa** tempo,
e o teto — dobrar a tabela de uma vez — vale 1,22×, e não os 1,59× que o caso
da tabela vazia sugeria.

E o preço não é só de tempo: adiar exigiria **marcar índice suspenso no
formato** do `.ndx`, porque uma queda no meio da carga deixaria uma árvore com
chaves faltando e nada dizendo isso — busca respondendo errado em silêncio, que
é o pior defeito que este projeto já teve três vezes. Formato novo, estado novo
que pode encalhar uma tabela, para ganhar 1,22× no melhor caso e perder na
maioria.

**Fica de fora, com o número na mesa.** O que o faria valer é outra coisa, e
maior: reconstruir só sobre as linhas novas e **fundir** a série ordenada na
árvore existente, em vez de refazê-la. Aí o custo passaria a depender de M, e
não de N+M.

E o teto tem uma trava que não é de desempenho: **o índice único não pode ser
adiado**. Ele é a própria decisão de aceitar ou recusar a linha, e a conferência
acontece antes de qualquer gravação porque o `.reg` nunca reaproveita slot —
descobrir a duplicata depois deixaria um buraco permanente por linha recusada.
Sobra adiar o não único, que na forma da bancada vale 1,19 s de 3,93.

---

## 4.5 A réplica: a causa registrada estava errada

Estava escrito em dois documentos que a réplica ficava para trás porque
«aplicar decodifica a imagem para `Value` e **reencoda** o payload, em vez de
gravar os bytes que vieram». Com o lote da B+tree pronto, o item virou o
próximo da fila — e a primeira coisa foi medir a acusação.

`--example onde-doi-na-replica`, 20.000 eventos, sem rede no meio:

| | µs/evento |
|---|---:|
| hexadecimal da imagem, no source | 3,48 |
| montar o JSON do lote | 1,21 |
| analisar o JSON, na réplica | 2,44 |
| hexadecimal da imagem, na réplica | 0,62 |
| `aplicar_evento` (decodifica + insere) | 16,15 |
| **o caminho todo, sem rede** | **23,90** |
| uma inserção local pura, para comparar | 15,80 |

`aplicar_evento` custa **16,15 µs** e uma inserção local custa **15,80**. A
acusação vale **0,35 µs** — e a réplica media **229 µs por evento**. Os outros
205 nunca estiveram nesse caminho.

### Onde eles estavam

**1. O source varria o diário desde o começo a cada lote.** Desde que o evento
deixou de ter largura fixa, chegar ao evento N é caminhar pelos N−1 anteriores
lendo o cabeçalho de cada um. `--example custo-do-desde`, diário de 100.000:

| P | ler 500 a partir de P | por evento |
|---:|---:|---:|
| 0 | 0,56 ms | 1,11 µs |
| 50.000 | 20,36 ms | 40,72 µs |
| 90.000 | 36,32 ms | **72,65 µs** |

Perfeitamente linear em P — e o total, quadrático. Alcançar os 100.000 de 500
em 500 gastava **4,07 s só do lado de quem serve**, ou 40,7 µs por evento
entregue, com três réplicas fazendo isso ao mesmo tempo sob a trava global do
master.

Com uma **marca de posição**, **0,09 s: 45×**. Ela é uma *dica*: uma errada faz
a leitura começar no lugar errado e o CRC do evento recusar, ou cair depois do
`fim` e devolver vazio. Nenhum dos dois entrega evento errado.

Ela mora no **servidor**, e não na tabela, porque a tabela é aberta e fechada a
cada pedido — e são pedidos seguidos que ela serve. E são **várias por tabela**:
um source atende réplicas em posições diferentes, e uma marca só seria empurrada
para frente pela mais adiantada e nunca serviria às outras. Foi essa correção
que trouxe o número de 7.835 para 17.450.

**2. O laço dormia depois de toda rodada.** O `reconectar_em` é o intervalo
entre perguntas **em vão**; dormir depois de uma rodada que aplicou eventos é
dormir enquanto o source escreve. Erro continua dormindo, de propósito.

**3.** E `bytes_para_hex` fazia um `format!` — e uma alocação de `String` — **por
byte** da imagem: 3,48 → 0,24 µs por evento, **14,5×**.

### O resultado, na bancada dos quatro servidores

| | antes | agora |
|---|---:|---:|
| master, com a imagem no diário | 28.914 linhas/s | 34.048 |
| **aplicação, por réplica (as três em paralelo)** | **4.273 ev/s** | **17.450** |
| alcançar 100.000 eventos | 18,7 s | **5,7 s** |
| exclusão física até as três | 1.952 ms | **140 ms** |
| réplica derrubada: alcançar 4.000 eventos | 1,0 s | 0,3 s |

**4,08×.** As três juntas aplicam ~52.000 eventos/s contra os 34.048 que o
master escreve — que era o pedido.

> A lição repete a do Profiler, e por isso vale escrevê-la de novo: o
> diagnóstico plausível sobrevive porque ninguém o mede. Aqui ele apontava para
> o lado errado do fio.

---

## 4.6 Abrir a tabela lia o arquivo INTEIRO

A pergunta era simples: continuamos perdendo do MySQL(R) no insert mesmo com o
`BULKINSERT`? A resposta é sim — e procurar o porquê achou outra coisa.

O primeiro número que não fechava: num processo só, inserir custa **16,0 µs por
linha com 200 mil e 16,4 com seis milhões** — não degrada. Mas a bancada mostra
a taxa caindo de 54.180 para 37.712 linhas/s, e ela carrega em lotes de 50.000
**abrindo e fechando a tabela em cada lote** — 200 processos para dez milhões.

Então: abrir custa mais conforme a tabela cresce? `--example abrir-cresce`:

| linhas | abrir | `.reg` | `.ndx` | `.log` |
|---:|---:|---:|---:|---:|
| 500.000 | 35,45 ms | 34,02 | 0,00 | 0,00 |
| 1.000.000 | 68,99 ms | 68,85 | 0,01 | 0,00 |
| 1.500.000 | 105,32 ms | 104,10 | 0,00 | 0,01 |
| 2.000.000 | 142,57 ms | 138,80 | 0,00 | 0,00 |

Linear, e tudo no `.reg`. A causa, em uma linha de código:

```rust
let bruto = std::fs::read(&primeiro)?;   // o volume INTEIRO
```

Ele trazia o volume inteiro para a RAM para tirar dele **128 bytes de cabeçalho
e o bloco de esquema**. Numa tabela sem paginação esse volume é a tabela toda:
**69 ms por milhão de linhas**, a cada abertura. Duas leituras curtas no lugar:

**138,80 ms → 0,03 ms, e agora é plano.**

### O que isso valeu, e o que não valeu

| na bancada de 10 milhões | antes | depois |
|---|---:|---:|
| inserir | 273,8 s | 265,2 s |
| **buscar** | 4,04 s | **1,21 s** |
| **varrer** | 5,04 s | **2,22 s** |
| **atualizar** | 3,38 s | **1,26 s** |
| excluir | 8,94 s | 7,34 s |

**No insert, 3%** — e a explicação é o cache do sistema: durante a carga o
arquivo está quente, e ler 400 MiB dele é um `memcpy`. Nas fases de leitura ele
está frio, e a mesma leitura vai ao disco: é lá que os segundos estavam.

Com isso o PhxSql passou a **ganhar em três das quatro** operações restantes,
`buscar` inclusive, que antes empatava.

### O que sobrou, e o suspeito que a medição já derrubou

A taxa de inserção ainda **cai com o tamanho** — 54.180 para 37.712 linhas/s.

O suspeito era o **cache de páginas do `.ndx` nascer vazio a cada processo**.
`--example cache-frio` compara, no mesmo processo, um lote de 50.000 com o cache
herdado do lote anterior contra um lote logo depois de fechar e reabrir a
tabela:

| já tinha | cache | µs/linha | páginas lidas do arquivo |
|---:|---|---:|---:|
| 100.000 | quente | 16,20 | 0,00 |
| 150.000 | **FRIO** | 16,02 | 0,00 |
| 500.000 | quente | 16,23 | 0,00 |
| 550.000 | **FRIO** | 16,17 | 0,00 |

**±1,6%, e nenhuma página lida do arquivo nos dois casos.** O suspeito está
errado, e a razão é simples depois de vista: as chaves entram em ordem
crescente, então a inserção vai sempre para o caminho mais à direita — meia
dúzia de páginas, que o cache reaquece nas primeiras linhas do lote. **É o
quinto diagnóstico plausível que a medição derruba neste documento.**

### O tamanho do buraco, medido

Mesmo esquema, mesmo código, 6 milhões de linhas:

| | tempo | µs/linha |
|---|---:|---:|
| **um processo só** (`carga inserir 6000000`) | 104,4 s | **17,40** |
| a bancada, 120 processos de 50.000 | 138,3 s | 23,0 |

**33,9 s de diferença, ou ~283 ms por lote** — e nem a abertura do `.reg` (hoje
0,03 ms) nem o cache frio explicam isso. O que sobra por lote é o
`sincronizar()`, que a bancada faz uma vez a cada 50.000 linhas por definição —
e o `fsync` de um arquivo que cresce até 1,5 GiB. O relógio menos a CPU da
corrida inteira dá 13,8 s, então o `fsync` não pode ser a diferença toda.

**Está em aberto, e o número está aqui para quem for medir.** A corrida de um
processo foi feita com a máquina ocupada, o que a favorece menos, não mais.

---

## 4.7 Compactar `.log`, `.trash` e `.reason`: o ganho é zero, e o motivo não é o registrado

O pedido 101 pede para **cifrar e compactar** os três diários. A pendência
registrava dois bloqueios: *«não temos cifra de bloco»* (resolvido — §8 do
`SEGURANCA.md`) e *«compactar append-only exige rotacionar e reescrever»*.

O segundo bloqueio foi medido antes de virar plano, e o resultado é o outro:
**não é preciso rotacionar nada, e mesmo assim o ganho é zero.**

```bash
cargo run --release --example quanto-ocupa -- 1000000 5
```

### O que uma tabela de 1 milhão de linhas ocupa

Cinco colunas, dois índices, 5% de exclusões (metade suave, metade de vez):

| arquivo | MiB | % da tabela | volumes |
|---|---:|---:|---:|
| `.reg` | 116,35 | 45,26% | 4 |
| `.ndx` | 89,73 | 34,90% | 1 |
| `.log` | 44,06 | **17,14%** | 1 |
| `.trash` | 3,67 | 1,43% | 1 |
| `.reason` | 3,27 | 1,27% | 1 |
| **total** | **257,08** | 100% | |

Os três do pedido somam **51,00 MiB — 19,84% da tabela**. Não é desprezível: a
premissa do item, de que ali há espaço a recuperar, **está certa**.

### E o DEFLATE encolhe mesmo

Não estimado — os bytes de verdade passados pelo `zip::deflate` que o backup já
usa:

| arquivo | razão | de → para |
|---|---:|---|
| `.reg` | 4,79× | 116,35 → 24,29 MiB |
| `.ndx` | **8,26×** | 89,73 → 10,87 MiB |
| `.log` | 4,08× | 44,06 → 10,79 MiB |
| `.trash` | 3,97× | 3,67 → 0,93 MiB |
| `.reason` | 2,80× | 3,27 → 1,17 MiB |

(O DEFLATE deste repositório anda a **34,0 MiB/s** — medido na mesma rodada, e
não citado de outro dia.)

### O número que derruba o item

> **Volumes fechados de `.log` + `.trash` + `.reason`, com 1 milhão de linhas:
> zero.**

Os três cortam volume por **bytes**, não por linhas, e o padrão é
`bytes_por_arquivo = 1 GiB`. Um evento sem imagem tem 44 bytes: o `.log`
só fecha o primeiro volume em **~24,4 milhões de eventos**. Com um milhão de
linhas os três estão, cada um, num único arquivo — e esse arquivo é o que ainda
está recebendo escrita.

Ou seja: **compactar por volume fechado pouparia exatamente 0 byte**, não
porque compactar não funcione — funciona, 4,08× — mas porque não há o que
compactar. O bloqueio registrado continua de pé, e por um motivo diferente do
escrito: não é que rotacionar seja difícil, é que **a rotação não acontece
nessa escala**.

Que a rotação resolveria, quando acontece, também foi medido — a mesma tabela
com volume de 512 KiB, para forçar o corte:

| arquivo | volumes | fechados | razão |
|---|---:|---:|---:|
| `.log` | 18 | 17 | 4,06× |
| `.trash` | 2 | 1 | 4,08× |
| `.reason` | 2 | 1 | 2,90× |

Com volume curto há o que compactar, e ele encolhe o mesmo tanto. **A decisão
que falta não é «como compactar», é «de quanto em quanto o diário deve
rotacionar»** — e essa é uma linha no `config.json`, não uma arquitetura.

### E o espaço não está onde o pedido olha

O `.ndx` sozinho ocupa **34,90%** e comprime **8,26×**: compactar só ele
pouparia **78,86 MiB**, contra 38,51 MiB dos três do pedido juntos — **2,0×
mais**.

Isso **não é uma recomendação**. O `.ndx` se lê em acesso aleatório, e é
exatamente sobre ele que o cache de páginas comprou os 2,40× da §1;
comprimi-lo trocaria espaço em disco por trabalho de CPU no caminho mais quente
que este motor tem. Está aqui porque a pergunta do item era sobre espaço, e a
resposta honesta sobre espaço é que os três diários **não são onde ele está**.

### Veredito

| | |
|---|---|
| **os três ocupam** | 51,00 MiB de 257,08 — 19,84% |
| **compactados poupariam** | 38,51 MiB — 14,98% da tabela |
| **compactando volume fechado, hoje** | **0 byte** — não há volume fechado |
| **falta para haver** | rotação do diário em algo menor que 1 GiB |

**Não implementado, e de propósito.** Escrever a compactação hoje entregaria
código que não comprime nada em nenhuma tabela real, e o teste que o cobrisse
teria de forçar um volume de 512 KiB para ver efeito — que é o sinal clássico
de funcionalidade medida contra si mesma. O que entra antes é o número acima
virar uma decisão sobre `bytes_por_arquivo` dos diários.

### 4.7.1 O corte virou configurável — e a premissa caiu

A decisão que faltava entrou: `recursos.diario_volume_mib` no `config.json`
(ver `phxsql_store::diario`). **Zero, que é o padrão, não mexe em nada** — vale
o `bytes_por_arquivo` do esquema, 1 GiB, byte por byte como antes. O `.bin` e o
`.memo` não passam por ele: o corte de um anexo é outro assunto, e juntá-los
faria mexer no diário mexer nas fotos.

Com isso, «não há volume fechado» deixou de ser uma propriedade do formato e
virou **uma escolha**. Então a medida foi refeita, na mesma tabela de 1 milhão
de linhas e 5% de exclusões — `--example quanto-ocupa -- 1000000 5 <MiB>`:

| corte do diário | volumes do `.log` | fechados | economia | % da tabela |
|---|---:|---:|---:|---:|
| 1 GiB (padrão) | 1 | 0 | **0,00 MiB** | 0,00% |
| 8 MiB | 6 | 5 | 30,61 MiB | 11,91% |
| 4 MiB | 12 | 11 | 33,56 MiB | 13,06% |
| 1 MiB | 45 | 44 | **37,78 MiB** | **14,70%** |

A `.trash` e o `.reason` só fecham volume a **1 MiB**: eles têm 3,67 e 3,27 MiB
contra os 44,06 do `.log`, e um corte único para os três é grosseiro por isso —
o que fecha o diário não chega perto de fechar os outros dois.

**A recusa anterior não estava errada; a razão dela é que era circunstancial.**
«Compactar pouparia exatamente zero» era verdade, e continua sendo com o corte
padrão. Com o corte pequeno passa a poupar 14,70% da tabela, que é dinheiro
real. O item foi medido de novo justamente porque *medir a premissa vem antes
de implementar o item*, e desta vez a premissa mudou de lado.

### 4.7.2 O outro lado da conta, que ninguém tinha medido

Economia não é o número inteiro. Um volume compactado **não se lê por dentro**:
para servir um lote de 500 eventos à replicação é preciso inflá-lo inteiro. E o
servidor abre e fecha a tabela a cada pedido, então não há cache que segure o
volume inflado entre um lote e o seguinte.

Medido na mesma rodada:

| corte | inflar o volume | ler 500 eventos como está hoje | quantas vezes |
|---|---:|---:|---:|
| 8 MiB | 25,88 ms | 3,14 ms | **8×** |
| 4 MiB | 13,33 ms | 2,95 ms | **5×** |
| 1 MiB | 3,37 ms | 4,50 ms | **1×** |

O custo escala com o tamanho do volume, e a 1 MiB ele **desaparece**: inflar um
volume de 1 MiB custa menos que a leitura de 500 eventos já custa hoje. Isso
derruba a objeção óbvia contra compactar — e é o oposto do que eu teria escrito
sem medir.

O que **não** desaparece é o teto. `max_arquivos` continua valendo, e cortar
pequeno o encolhe na mesma proporção:

| corte | 999 volumes dão | eventos sem imagem |
|---|---:|---:|
| 1 GiB (padrão) | 1 TiB | ~24 bilhões |
| 8 MiB | 7,8 GiB | ~190,5 milhões |
| 4 MiB | 3,9 GiB | ~95,2 milhões |
| 1 MiB | 1,0 GiB | ~23,8 milhões |

A 1 MiB o diário inteiro passa a caber no que **um** volume guarda hoje. Quem
cortar pequeno precisa subir `digitos` para 4 no `CREATE TABLE` — que o formato
já suporta — ou aceitar o teto menor.

### 4.7.3 O veredito, revisado

| | |
|---|---|
| **o corte do diário** | configurável, `recursos.diario_volume_mib`; padrão 0 = não mexe |
| **compactar volume fechado, com corte de 1 MiB** | poupa 37,78 MiB — 14,70% da tabela |
| **custo de leitura a 1 MiB** | nenhum: 3,37 ms de inflate contra 4,50 ms que a leitura já custa |
| **custo de leitura a 8 MiB** | 8× |
| **preço escondido** | o teto do diário cai na mesma proporção do corte |
| **compactar só o `.ndx`** | pouparia 78,86 MiB — **2,1× mais**, e sem tocar no diário |

**A compactação continua não implementada, e agora por outro motivo.** Não é
mais «não há o que compactar»: há, e a conta fecha. É que os 14,70% que ela
compra estão atrás de um formato de volume comprimido, de um caminho de leitura
que infla, e de um comando de compactação — e o mesmo esforço aplicado ao
`.ndx` compraria 2,1× mais espaço. **O espaço continua não estando onde o
pedido olha**, e essa frase agora está sustentada por três cortes medidos em
vez de um.

---

## 4.8 O write-back entrou — e o gargalo mudou de lugar

`gravar_pagina` passou a deixar a página **suja em RAM**; o CRC-32 e o `write`
acontecem no despejo, no fechamento ou no `sincronizar`. É o que o InnoDB faz
(`mtr0mtr.cc:338` marca, `buf0flu.cc:1243` sela) e o Aria também
(`PCBLOCK_CHANGED`, `PAGECACHE_WRITE_DELAY`).

`--example onde-doi`, dois índices, o empilhamento da rodada:

| | µs/linha |
|---|---:|
| 0.17.0 | 16,4 |
| + cabeçalho do `.ndx` fora do caminho da chave (§4.6b) | 14,5 |
| + CRC slice-by-16 | 13,1 |
| + **cache write-back** | **7,5** |

**2,19×**, e a forma mudou: `.reg`+`.log` virou **60,8%** e os dois índices
29,4%. O `.ndx` deixou de ser o dono do tempo. E o ganho **se mantém a 3
milhões de linhas** — 7,5 µs lá também.

### O falso culpado que ficou registrado aqui por algumas horas

A primeira versão desta seção dizia: «a bancada mal se mexeu (265,2 → 261,8 s)
porque o **esquema** dela custa 2,2× — o `Decimal` e o `Date` levam a inserção
de 7,50 para 16,61 µs». Tinha tabela, tinha medição, e estava **errada**.

A prova que a derrubou, em três passos:

1. `--example abrir-contra-criar`: tabela recém-criada e tabela reaberta
   inserem igual (7,48 contra 7,46 µs) — não era o caminho de abertura;
2. o mesmo esquema de 5 colunas medido por um exemplo novo dava **8,0 µs**,
   e pelo `carga` dava 16,9 — **na mesma máquina quieta**;
3. `ls -l` no binário: `target/release/examples/carga` era das 01:56,
   **anterior ao write-back**. `cargo build --release` **não recompila os
   examples**, e a bancada chama o binário direto.

Recompilado: **7,92 µs/linha, 126.280 linhas/s** — o esquema da bancada custa
~0,4 µs a mais que o simples (5%), não 2,2×. O «custo da codificação da linha»
que esta seção mandava investigar era o custo de um binário velho.

**É o sétimo diagnóstico plausível que este documento derruba, e este era
nosso duas vezes**: a medição estava certa, o medidor é que media o passado.

> **Medidor com binário velho mede o passado.** `cargo build --release` não
> recompila os examples; antes de qualquer medição, `cargo build --release
> --examples -p phxsql-store`. A bancada de 261,8 s rodou com um `carga`
> anterior ao write-back — o número oficial está sendo refeito.

### 4.9 O `sincronizar` no caminho da operação: 8 µs/linha, medido

A leitura do Cassandra (`docs/CASSANDRA.md`) apontou: lá o cliente **nunca**
executa `fsync` — nem no modo `batch`, quem sincroniza é uma thread própria
(`AbstractCommitLogService.java:154`). Aqui, a `Durabilidade::PorLote` fecha a
janela **dentro** da 200ª operação.

O critério foi combinado antes de medir: abaixo de 0,46 µs/linha (2% da
bancada), o item morre. `--example custo-do-fsync`, esquema da bancada,
trechos intercalados:

| tabela com | a cada 200 | uma vez só | delta |
|---:|---:|---:|---:|
| 1.000.000 | 16,13 µs | 7,99 µs | **8,14 µs** |
| 3.000.000 | 16,92 µs | 8,05 µs | **8,87 µs** |

**O item vive, e é grande**: o `sincronizar` a cada 200 **dobra** o custo por
linha. O `fsync` em si é ~0,8 ms ÷ 200 = 4 µs; o resto é o write-back sendo
neutralizado — sincronizar a cada 200 descarrega as páginas sujas antes de a
folha encher, e o CRC volta a ser pago por poucas chaves em vez de por
centenas.

O que fazer com isso é **decisão de garantia, não de código**: tirar o `fsync`
do caminho da operação é exatamente o modo `periodic` do Cassandra — o OK deixa
de significar «durável» e passa a significar «recebido». O `BULKINSERT` já dá
isso a quem pede, para carga. Estender ao caminho comum muda o contrato de todo
cliente, e guarda nova entra pedida, não imposta.

### 4.10 A bateria de ponta a ponta: o gatilho e a chave, medidos

```bash
cargo build --release
cargo build --release --examples -p phxsql-store
python3 bancada/bateria/prova-bateria.py --medir --rodadas 3
```

A bateria de `bancada/bateria/` faz os seis itens do pedido como um usuário
faria — cria o banco, cria as tabelas, gera as chaves, pendura os gatilhos,
chama os procedimentos e carrega 5.000 linhas — e no fim **mede**. Os quatro
cenários fazem o mesmo trabalho (as mesmas 5.000 linhas, o mesmo formato de
tabela, os mesmos dois índices) e mudam **só o gatilho**; cada rodada usa uma
tabela nova, e as rodadas são intercaladas.

#### O que um gatilho custa por linha

5.000 linhas em lotes de 1.000, três rodadas intercaladas:

| cenário | mediana | linhas/s | µs/linha | diferença | veredito |
|---|---:|---:|---:|---:|---|
| sem gatilho | 0,148 s | 33.837 | 29,55 | — | — |
| `BEFORE` que normaliza um campo | 0,160 s | 31.163 | 32,09 | +2,54 | **não aparece acima do ruído** |
| `AFTER` que só calcula | 0,169 s | 29.510 | 33,89 | +4,33 | **não aparece acima do ruído** |
| `AFTER` que grava auditoria | 0,583 s | 8.577 | 116,60 | **+87,04** | acima do ruído |
| uma a uma (`inserir`, 5.000 viagens) | 1,385 s | 3.611 | 276,92 | — | outro trabalho |

**Maior espalhamento dentro de um cenário sozinho: 7,73 µs/linha** — é a régua
com que a coluna «diferença» se lê, e é por isso que as duas primeiras linhas
não viram número.

A conclusão que vale é a última: **o gatilho que só decide custa o que a
medição não consegue separar do ruído; o gatilho que ESCREVE custa 87 µs por
linha, quase quatro vezes a inserção inteira.** E o motivo é conhecido e já
está escrito neste documento por outro caminho: cada `INSERT` do corpo de um
`AFTER` sai pelo `executar_derivado` como um `inserir` completo — toma a trava,
**abre a tabela de destino (sete arquivos)**, grava e fecha. É a mesma conta
que fez o `inserir_lote` existir: *vinte mil inserções pela rede eram vinte mil
aberturas de tabela*. Dentro do gatilho, a carga de 5.000 linhas com auditoria
são 5.000 aberturas da tabela de auditoria.

Isso é um **item de desempenho identificado, e não consertado nesta rodada**:
juntar os `INSERT` que os `AFTER` de um lote produzem num `inserir_lote` só,
por tabela de destino, é o desenho óbvio — e muda a ordem em que os gatilhos
enxergam o mundo, então é decisão, não ajuste.

#### A chave: v7 contra v4 contra `Sequence` — e por que a escala é a medição

O `uuid.rs` afirma, na própria documentação do módulo, que chave **aleatória**
espalha a inserção por folhas diferentes da B+tree e chave **crescente** cai
sempre na folha da direita. A afirmação nunca tinha sido medida aqui. E ela
**só se mede em duas escalas**: enquanto o `.ndx` inteiro cabe na cache de
páginas, espalhar não custa quase nada — a folha «longe» também está na
memória. Uma escala só mediria a cache e chamaria isso de chave.

Mesma tabela, mesmas colunas, mesmos dois índices, três rodadas intercaladas,
`cache_paginas: 2048` (8 MiB):

| chave primária | 100.000 linhas | 1.000.000 linhas |
|---|---:|---:|
| `Uuid` v7 (crescente) | 27,38 µs · 36.527/s | **28,15 µs** · 35.527/s |
| `Uuid` v4 (sorteado) | 30,04 µs · 33.284/s | **42,39 µs** · 23.588/s |
| `Sequence` (o motor numera) | 23,36 µs · 42.801/s | 24,14 µs · 41.426/s |
| **v4 ÷ v7** | **1,10×** | **1,51×** |
| **v7 ÷ Sequence** | 1,17× | 1,17× |

Maior espalhamento dentro de um cenário: **1,31 µs/linha**. As diferenças estão
todas muito acima dele.

Três leituras, e a segunda é a que importa:

1. **A hipótese do módulo está certa — e o número dela cresce com a tabela.**
   A 100.000 linhas o v4 custa 1,10×, que é quase nada; a 1.000.000 custa
   1,51×. Publicar só o primeiro número teria enterrado o motivo de o v7
   existir; publicar só o segundo teria escondido que abaixo de certa escala
   ele não paga.
2. **O que separa os dois não é o v4 ficar caro: é o v7 não ficar.** O custo do
   v7 vai de 27,38 para 28,15 µs quando a tabela cresce dez vezes — 2,8%. O do
   v4 vai de 30,04 para 42,39 — 41%. *Chave crescente mantém o custo por linha
   constante enquanto a tabela cresce.* É essa a frase que o formato promete, e
   agora ela tem número.
3. **O `Sequence` é 1,17× mais barato que o v7 nas DUAS escalas, e a constância
   é a explicação.** Se a vantagem fosse de localidade, ela cresceria com a
   tabela como a do v4 cresce. Ela não cresce: é o preço fixo de uma chave de
   16 bytes contra uma de 8 na página do `.ndx` — cabem menos chaves por
   página, e a árvore fica proporcionalmente mais alta. Escolher entre os dois
   é escolher entre 17% e um identificador que não revela quantas linhas a
   tabela tem.

#### O infrutífero, que também é resultado

**A comparação com `bancada/carga/resultados.json` (39.287 linhas/s em lote)
não foi feita, de propósito.** É a mesma pergunta e **não é o mesmo trabalho**:
lá a chave é `Int8` e o lote tem 5.000 linhas; aqui a chave é `Uuid` de 16
bytes, há uma segunda coluna `Uuid` indexada e o lote tem 1.000. Encostar os
dois números lado a lado produziria um «a carga caiu 14%» que não é sobre
nada — e é exatamente o erro que o `bancada/LEIA-ME.md` registra duas vezes.
O controle honesto desta bateria está dentro dela: o cenário «sem gatilho».

---

## 4.11 GPU/CUDA: medido contra o nosso gargalo, e recusado com o número

Chegou o pedido «GPU CUDA ativar para ajudar em processamento pesado». Vale a
mesma regra que derrubou o WAL/LSM: **receita de fora se mede contra o nosso
gargalo antes de virar plano**. O documento inteiro está em `docs/GPU.md`, com
o medidor `--example onde-a-gpu-ajudaria`; aqui ficam só os números que mudam
a leitura deste documento.

**O candidato aritmético dentro da inserção não existe mais.** O CRC-32 é hoje
**0,58%** de uma inserção de 7,35 µs — instantâneo, ele daria **1,006×**. Este
documento registra na §2 que ele já foi 57%; o cache de write-back comprou essa
diferença inteira, sem placa nenhuma.

**O backup foi decomposto, e o dono do tempo não era o suspeito:**

| parcela do backup de uma tabela de 236,6 MiB | segundos | % |
|---|---:|---:|
| **DEFLATE**, a 42 MiB/s no `.reg` real | **5,62** | **63,0%** |
| SHA-256 do manifesto, a 219 MiB/s | 1,08 | 12,1% |
| o resto (ler, montar o zip, gravar) | 2,22 | 24,9% |
| **total** | **8,91** | 100% |

O maior custo de CPU contígua deste motor é o **DEFLATE** — e ele é o **menos**
paralelizável de todos, porque LZ77 procura repetição num dicionário que
depende dos bytes anteriores. Nunca tinha sido medido; fica registrado.

**A conta que mata a agregação, e mata em qualquer tamanho:** o `SUM` sobre uma
coluna `i64` anda a **28.234 MiB/s** nesta CPU, contra **15.754 MiB/s** de pico
*teórico* do PCIe 3.0 x16 — **1,79×**. Quando a CPU consome os bytes mais
depressa do que o barramento os entregaria, não há limiar de tamanho: a GPU
perde sempre.

**E o que o pedido queria existe, do lado da CPU.** Dividindo pelos 4 núcleos
com a `std` que já está aqui (`paralelo.rs`):

| núcleo, 16 blocos de 1 MiB | 1 thread | 4 threads | ganho |
|---|---:|---:|---:|
| ChaCha20-Poly1305 selar | 45,0 ms | 11,6 ms | **3,90×** |
| CRC-32 | 8,8 ms | 2,4 ms | **3,59×** |
| SHA-256 | 72,5 ms | 28,9 ms | **2,51×** |

Perto do 4× ideal porque os três são presos à **conta**. O contraste com a
varredura em memória — que rende 1,8× e não 4×, por ser presa à **banda** — é a
mesma fronteira que decide o caso da GPU.

### Dois achados desta bateria que valem para este documento

**1. Um item de desempenho novo, de graça: o `ORDER BY`.** Ordenar 1.000.000 de
linhas custa **213,8 ms**; ordenar as mesmas 1.000.000 de chaves como `u64`
custa **19,9 ms** — **10,7×** na mesma CPU. A diferença é comparar e mover
`Value`, que é enum com `String` no monte, dentro do laço de comparação. Ordenar
`(chave, índice)` e permutar uma vez no fim levaria o `ORDER BY` inteiro de
635,4 ms para ~422 ms: **1,51×**, sem thread, sem SIMD e sem dependência.

**2. `-C target-cpu=native` não é ganho de graça.** Três pares intercalados, os
mesmos binários trocando só a bandeira:

| núcleo | genérico | `native` | |
|---|---:|---:|---:|
| ChaCha20-Poly1305 | 355–359 MiB/s | 452–460 MiB/s | **1,28×** |
| CRC-32 | 1.799–1.821 MiB/s | 1.805–1.806 MiB/s | 1,00× |
| **SHA-256** | 215–221 MiB/s | **143–144 MiB/s** | **0,66× — pior** |

Ligar a bandeira no build de lançamento teria trocado 1,28× no ChaCha por
**−34% no SHA-256**, e ninguém perceberia.

> E a armadilha de medição que esta bateria pagou, porque ela morde qualquer
> medidor futuro: a primeira versão deu **12,5 bilhões de MiB/s** de banda de
> memória e **192 milhões de MiB/s** de CRC-32, porque `crc32(&pagina)` é
> função pura de um `slice` invariante e o compilador a ergueu para fora do
> laço. `std::hint::black_box` na entrada **e** na saída conserta. Número
> impossível é fácil de pegar; o perigoso é o mesmo erro rendendo um número
> plausível.

---

## 4.12 `ALTER TABLE ADD COLUMN`: a inferência era «minutos», e são 5,5 s

O item 25 de `docs/SPRINTS.md` chegou com o custo escrito como **inferência**,
e com a palavra: *«a casa dos minutos para dez milhões — inferido, não
medido»*. Medi antes de aceitar o desenho, com `--example custo-do-alter`, e a
inferência estava errada por quase duas ordens de grandeza.

O medidor não chuta dez milhões: ele mede vários tamanhos, mostra que o custo
por linha é constante, e o maior deles **é** dez milhões.

| linhas | `.reg` antes | alterar | µs/linha | MiB/s (lê + escreve) |
|---:|---:|---:|---:|---:|
| 50.000 | 5,6 MiB | 0,023 s | 0,470 | 504 |
| 200.000 | 22,5 MiB | 0,107 s | 0,536 | 441 |
| 1.000.000 | 112,5 MiB | 0,536 s | 0,536 | 441 |
| 4.000.000 | 450,1 MiB | 3,096 s | 0,774 | 306 |
| **10.000.000** | **1.125,3 MiB** | **5,53 s** | **0,553** | **427** |

A tabela é a do medidor: `Int8` + `Str(40)` + `Str(20)` + `Decimal(15,2)` mais
as duas colunas de sistema, e a coluna nova é `Str(12)` — 12 bytes a mais por
slot, de 118 para 130. O custo é **linear e dominado pelo disco**: 427 MiB/s é
a soma do arquivo lido com o escrito, e a passada é sequencial dos dois lados.
A linha dos 4 milhões saiu mais lenta (306 MiB/s) por pressão de cache da
máquina, e ela fica na tabela justamente para não fingir que a medição é
silenciosa.

Para comparar: **construir** a mesma tabela de dez milhões levou 90,4 s. A
alteração custa **6,1% do que custou digitar o dado** — e é por isso que
reescrever, que parecia a saída cara, é a barata.

### As três saídas, com número

**(a) reescrever a tabela** — a escolhida. 5,53 s por dez milhões de linhas,
uma passada, e o rowid preservado porque a ordem é preservada. Ela **não** faz
mais nada: não troca tipo de coluna, não tira coluna, não cria índice.

**(b) coluna «à direita», com duas larguras de slot convivendo** — o formato
**não permite**, e a conferência é de uma linha: o `slot_size` é **um campo
só**, `u32` nos bytes 16..20 do cabeçalho do volume, e é dele que sai o
endereço de toda linha. Duas larguras exigiriam um mapa `rowid → offset`, que
é 8 bytes por linha (80 MiB para dez milhões) e uma **busca** onde hoje há uma
multiplicação. Esse é o preço medido do outro lado: numa tabela de 200.000
linhas, ler pela conta custa **1,08 µs** e ler passando por uma descida de
árvore custa **2,55 µs** — **2,36×**, em *toda* leitura, para poupar uma
passada *uma* vez. Recusada com o número.

**(c) só em tabela vazia** — o que o Aria exige para desligar índice. Custa
**0,6 ms**, e é honesta; ela apenas não resolve o problema que existe: quem
precisa de coluna nova precisa dela no segundo mês, com dado dentro. Ela
continua sendo o caminho de quem declara a coluna obrigatória sem padrão, que
é o único caso em que a alteração é recusada numa tabela com linha.

### O que isto custa de espaço, durante

Pico de **2× o `.reg`** (o velho e o `*.novo` convivem até a troca), mais 2× o
`.bkp` quando há espelho. Para dez milhões de linhas: 1,1 GiB viram 2,3 GiB no
pico, e 1,2 GiB depois. É o preço de a queda no meio deixar o arquivo velho
inteiro ou o novo inteiro, e nunca um meio-termo.

### Como refazer

```bash
cargo build --release --examples -p phxsql-store
./target/release/examples/custo-do-alter 50000 200000 1000000
./target/release/examples/custo-do-alter 10000000     # ~1,5 GiB livres
```

---

## 5. Por que LSM não cabe dentro do motor atual

Segmentos imutáveis com compactação é uma boa arquitetura, e é incompatível com
quatro coisas que **já funcionam aqui** — não por gosto, por dependência:

1. **A ordem de digitação.** É a regra que define o projeto: percorrer o `.reg`
   devolve as linhas na ordem em que foram digitadas. Compactação reordena.
2. **O endereço por conta.** `offset = data_offset + (rowid−1) × slot_size`.
   Numa LSM a linha muda de arquivo quando o segmento é compactado, e o rowid
   deixa de ser endereço.
3. **A paginação por cursor e o salto por bissecção.** Os dois saem de graça
   *porque* a ordem lógica é a ordem física. Sem isso, voltam a exigir índice.
4. **A garantia da replicação.** Uma réplica chega aos mesmos rowids sem que
   ninguém os transmita, porque o rowid é `slots + 1` e nada os reordena. Numa
   LSM essa garantia não existe.

A saída correta é a que a própria proposta sugere: **dois motores**, escolhidos
por tabela. Um `PHX-LSM` para log, telemetria e IoT — onde a ordem de digitação
não é sagrada e a escrita massiva é tudo — ao lado do motor atual para o ERP.
Isso é um projeto próprio, não um ajuste.

---

## 6. Comparativo com a concorrência

Bancada de **10 milhões de linhas**, mesma máquina, mesmo trabalho
(`bancada/`). Positivo = PhxSql mais rápido:

| Fase | PhxSql 0.16.0 | PhxSql 0.17.0 | MySQL(R) | |
|---|---:|---:|---:|---|
| inserir 10.000.000 | 884,3 s | **303,0 s** | 115,2 s | 0,38× (era 0,13×) |
| buscar 20.000 por chave | 5,08 s | **2,62 s** | 2,60 s | **0,99×** — empate |
| varrer faixa | 3,94 s | **3,28 s** | 26,19 s | **7,98×** |
| atualizar | 4,44 s | **1,92 s** | 6,33 s | **3,30×** |
| excluir | 4,85 s | 8,16 s | 6,25 s | 0,77× — ver abaixo |

O cache de páginas mudou quatro das cinco linhas: a inserção ficou **2,92×** mais
rápida, a busca por chave **empatou** com o MySQL(R) (era metade da velocidade
dele), e a alteração passou de 1,36× para 3,30×.

**Sobre a exclusão, honestamente.** É a única fase em que o PhxSql *espera
disco*: 4,3 s de CPU para 8,16 s de relógio. Ela grava a linha inteira no
`.trash` e **sincroniza antes** de liberar o slot — a ordem que garante que a
linha nunca deixa de existir nos dois lugares ao mesmo tempo. O número anterior
(4,85 s) saiu de outra corrida, em outro estado de disco, e repetir a fase
sozinha na mesma máquina deu de **0,80 s a 2,76 s**. Ou seja: esta linha varia
demais entre corridas para sustentar «piorou» ou «melhorou», e nada no caminho
dela mudou nesta versão. Fica publicada como medida, com a instabilidade dita.

A leitura sequencial continua sendo onde o formato de slot fixo paga: **8× o
MySQL(R)**. A inserção é onde ele cobra — e cobra 3× menos que cobrava.

### Onde o PhxSql já ganha, e por quê

| | Medido | Por quê |
|---|---|---|
| Varrer faixa | 8,0× o MySQL(R) | slot de largura fixa, sem página, sem MVCC |
| Página no meio de 800 mil linhas | 164 µs contra 246 ms (1.500×) | a ordem lógica é a física: achar é uma conta |
| Consulta em memória | 87× o disco | `SelectMemory` com mapas por coluna |
| Resposta do protocolo | 44 ms → 1,3 ms (33×) | `TCP_NODELAY`, que faltava |

### O que mudou nesta versão

| | Antes | Agora | |
|---|---:|---:|---|
| Inserção pela rede, linha a linha vs. lote | 2.659/s | 43.302/s | **16,3×** |
| Carga em lote, sem reserva vs. com `BULKINSERT` | 43.500/s | 66.500/s | **1,53×** |
| Inserção local, 2 índices (`onde-doi`) | 22.516/s | 62.763/s | **2,79×** |
| Página por posição no fim de 200 mil linhas | 131 ms | 6 ms | **22×** |
| Contar as linhas visíveis | varredura inteira | dois campos do cabeçalho | O(1) |
| Replicação | não existia | 4.357 eventos/s por réplica | — |

A carga pela rede agora tem script: `bancada/carga/medir.py`. O número anterior
(2.715 → 25.985 linhas/s) foi medido **à mão**, sem programa que o refizesse —
e o motor mudou desde então. Os dois lados batem no linha a linha (2.715 e
2.659), que é o controle; o lote subiu de 25.985 para 39.287 por causa do cache
de páginas.

---

## 7. O que eu faria a seguir, nesta ordem

Pela medição, e não pela moda:

1. **CRC incremental por nó**, em vez de recalcular a página inteira. A conta do
   CRC agora fecha (§2): das 2,06 páginas gravadas por linha, cada uma paga
   2,34 µs de CRC — **4,8 µs de 15,9, ou 30%**. É o maior pedaço isolado que
   sobrou, e é o mesmo alvo do cache por outro lado: o cache tirou o CRC da
   leitura, isto tiraria o da gravação.
2. ~~**Construção em lote da B+tree**~~ — **feita** (§4.3): 23× a 25×, de
   7,72 s para 0,31 s num milhão de chaves. O adiamento que ela deveria
   destravar foi medido depois e **não compensa** (§4.4): ganha 1,22× no melhor
   caso, perde abaixo de M ≈ N/3, e cobraria um estado novo no formato.
3. **Ordenar as chaves do lote** (§4.1): 1,19× medido, e uma garantia a
   rebaixar. O número está na mesa; a decisão não é técnica sozinha.
4. **Buffer de escrita maior**, para baixar as chamadas de sistema por linha.
   Ganho pequeno pela medida: um `lseek` custa 0,10 µs, e mesmo 41 por linha
   dariam 4,1 µs — enquanto o CRC de gravação sozinho já custa 4,8.
5. **Trava por tabela.** Não acelera uma inserção; acelera o servidor com muita
   gente. É outro eixo, e o roteiro já o previa.
6. **`ORDER BY` por chave, não por linha** (§4.11): **1,51×** medido, e é o item
   mais barato desta lista — ordenar `(chave, índice)` e permutar uma vez, em
   vez de mover `Value` com `String` dentro do laço de comparação.
7. **Dividir os núcleos aritméticos pelos 4 processadores** (§4.11): **3,90×**
   na cifra, **3,59×** no CRC, **2,51×** no SHA-256. A peça existe
   (`paralelo.rs`); falta chamá-la de dentro do backup e da cifra.
8. **O DEFLATE do backup**, que é **63,0% dele** a 42 MiB/s (§4.11) e nunca
   tinha sido medido. Não é candidato a paralelismo fácil — o dicionário é
   serial —, mas é o maior custo de CPU contígua deste motor.

O que eu **não** faria agora: WAL, MemTable de escrita e group commit. Eles
resolvem o gargalo do InnoDB, e a medição diz que ele não é o nosso. **Nem
CUDA**, e agora com número: `docs/GPU.md` e a §4.11.

---

## Como refazer tudo

```bash
cargo run --release --example onde-doi -- 200000       # a tabela do §2
cargo run --release --example custo-do-alter -- 50000 200000 1000000  # a §4.12
cargo run --release --example custo-do-sync            # os modos de durabilidade
cargo run --release --example custo-da-pagina -- 800000 200
cargo run --release --example indice-em-lote -- 1000000   # o lote do §4.3
cargo run --release --example onde-a-gpu-ajudaria -- 1000000  # o caso da GPU, §4.11
cargo run --release --example adiar-vale-quando -- 200000 # o ponto de virada
cargo run --release --example quanto-ocupa -- 1000000 5   # a ocupação do §4.7
python3 bancada/medir.py 10000000                      # o comparativo do §6
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
python3 bancada/bateria/prova-bateria.py --medir            # o §4.10
```
