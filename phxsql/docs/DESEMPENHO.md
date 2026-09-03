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

## 4.12 O `fsync` da exclusão: a única fase em que perdíamos, e quem escolhe

É o sprint nº 1 de `docs/SPRINTS.md`, e o único da lista que chegou com número
medido. Ele entrou **pedido**, e a §2.1 daquele documento explica por quê: com
a configuração padrão, um `excluir` que responde OK **já está no disco**, e
mudar isso por padrão mudaria o significado da resposta para todo cliente que
já existe, sem ninguém ter pedido. Retirar garantia sem pedido é o mesmo
estrago de impor guarda nova, pelo outro lado.

O interruptor é `recursos.exclusao_na_janela`, e ele nasce **desligado**.

### O que era, e por que doía

`LixeiraFile::guardar` chamava `Volumes::sincronizar` **por exclusão**, com a
razão escrita ali: «"está na lixeira" com a página ainda suja na memória não é
uma garantia». A inserção e a alteração já respeitavam
`recursos.durabilidade`; a exclusão tinha política própria, mais rígida, e
ninguém escolheu isso — o `fsync` foi parar dentro da lixeira porque a garantia
daquele arquivo depende dele, o que é certo, e acabou ficando fora da janela
que governa todo o resto.

### A medição refeita, com a condição declarada

O sprint mandou **remedir em máquina quieta**, com o critério combinado antes:
**abaixo de 2× o item morre.** A máquina desta rodada **não estava quieta** —
outros agentes rodando bancadas ao lado, `loadavg` entre 2,4 e 5,0 —, e isso
está dito aqui em vez de escondido. O que sustenta a conclusão não é a mediana:
é que as duas distribuições **não se tocam**.

O medidor não pede mais uma cópia editada do repositório, como pedia quando o
número de 7,8× foi levantado. As duas variantes saem do mesmo binário:

```bash
cargo build --release --examples -p phxsql-store       # a regra do binario velho
./target/release/examples/custo-do-excluir 200000 20000 200
PHX_EXCLUSAO_NA_JANELA=1 ./target/release/examples/custo-do-excluir 200000 20000 200
```

**O que o servidor entrega**, que é o número que importa — os dois lados com a
janela de 200 gravações que o `config.json` traz por padrão, sete corridas
alternadas:

| variante | corridas (s) | mediana |
|---|---|---:|
| hoje: `fsync` por exclusão | 4,293 · 4,412 · 4,461 · 4,517 · 5,824 · 8,504 · 18,083 | **4,517** |
| pedida a janela | 1,405 · 1,408 · 1,454 · 1,458 · 1,550 · 1,722 · 1,981 | **1,458** |

**3,10×** pela mediana. E o que sustenta: a **pior** corrida com a janela
(1,981 s) ainda é **2,17×** melhor que a **melhor** corrida sem ela (4,293 s).
Acima do critério de morte pelos dois caminhos.

**O teto**, medido à parte (janela que nunca fecha — o caso do `BULKINSERT`, em
que a reserva deixa a janela aberta de propósito), dez corridas de cada:

| variante | mediana |
|---|---:|
| `fsync` por exclusão, sem janela | 3,748 s |
| na janela, sem fechar | 0,577 s |

**6,50×** — que é o mesmo par que o `SPRINTS-CASSANDRA.md` §3 mediu como 7,8×
noutra máquina. Mesma ordem de grandeza, e a diferença é o custo do `fsync`
deste disco, não do motor.

### O que exatamente se perde na janela nova

Esta é a metade do sprint que **não** é medição — é leitura, e tinha de estar
escrita antes de uma linha de código. A ordem de escrita **não muda** em nenhum
dos dois modos: `guardar` no `.trash` continua vindo antes de `reg.excluir`. O
que muda é quem espera o disco.

**Queda do PROCESSO** (`kill -9`, OOM, `panic`, o serviço reiniciado): **não se
perde nada, e isso está provado**. O `write` já foi entregue ao sistema
operacional em toda gravação — não há buffer nosso —, e quem reabre o arquivo lê
a mesma página. A prova é `bancada/exclusao/prova-da-queda.py`, que sobe um
`phxsqld` de verdade na porta 7100 com a janela **aberta durante a corrida
inteira** (`lote_operacoes` em um milhão), manda 150 exclusões físicas pelo
soquete, mata o processo com `SIGKILL` e reabre:

```
=== controle: o comportamento de sempre (exclusao_na_janela=False) ===
  so no .reg (a exclusao nao aconteceu) : 0
  so no .trash (aconteceu, e reversivel): 150
  nos dois (duplicada)                  : 0
  EM NENHUM (o caso que mata o sprint)  : 0
=== pedida a janela (exclusao_na_janela=True) ===
  ... o mesmo, linha por linha
```

Teste unitário não provaria isso: quem fecha uma `Table` executa `Drop`, libera
descritores e volta ao teste, e nada disso é uma queda. É a mesma lição do
`BULKINSERT` — o que depende do sistema operacional se prova contra o sistema
operacional.

**Queda de ENERGIA dentro da janela.** Aqui está o caso a caso, e ele é honesto
até o fim. Duas escritas independentes, em dois arquivos, sem `fsync` entre
elas: o `.trash` (W1) e a liberação do slot no `.reg` (W2). O que estiver no
disco depois da queda dá quatro estados:

| chegou ao disco | o que se vê | |
|---|---|---|
| nem W1 nem W2 | a linha continua no `.reg` | a exclusão não aconteceu — nada se perde |
| W1 e não W2 | a linha está no `.trash` **e** no `.reg` | duplicada, e é o lado que a casa escolheu |
| W1 e W2 | excluída e reversível | o caso normal |
| **W2 e não W1** | **liberada no `.reg` e ausente do `.trash`** | **o quarto caso** |

**O quarto caso existe, e é isto que o sprint mandou procurar.** Com o `fsync`
por exclusão ele é impossível por construção: W2 nem chega a ser escrito antes
de W1 estar no disco. Sem ele, a ordem de *escrita* continua sendo W1 antes de
W2, mas a ordem de *chegada ao disco* passa a ser do sistema operacional, e o
`.reg` de uma tabela em uso já tem páginas sujas mais velhas que as do
`.trash` — o descarregamento periódico pode muito bem chegar nele primeiro.
Nenhum `fsync` na hora de fechar a janela conserta isso, porque o problema não
é a ordem em que **nós** sincronizamos: é a que o núcleo escolhe **antes** de
alguém nos perguntar.

Duas coisas foram feitas com esse achado, e nenhuma delas é escondê-lo:

1. **`Table::sincronizar` passou a fechar o `.trash` primeiro e o `.reg` por
   último.** Não fecha o buraco — fecha a metade dele que era nossa: com a
   ordem antiga, uma queda no meio do próprio fechamento da janela produzia o
   quarto caso *de propósito*, e não por azar. O teste
   `o_trash_fecha_antes_do_reg` trava isso.
2. **O item entrou pedido.** Ligado, `exclusao_na_janela` é a escolha de trocar
   uma rede de recuperação estreita por 3,10× — e a linha perdida nessa janela
   é uma linha que **alguém mandou apagar**, com o motivo já gravado no
   `.reason`, e não uma linha que alguém queria manter. Desligado, nada disso
   existe.

E um consolo que não é garantia, mas é verdade e vale escrever: **o `.reg`
nunca reaproveita slot excluído.** No quarto caso o *payload* da linha continua
fisicamente no slot, com o byte de estado virado — quem investiga com uma
ferramenta que leia o slot ainda acha os bytes. O que se perde de verdade é o
conteúdo das colunas externas (`.bin`/`.memo`), cujos blocos a exclusão liberou
e um insert seguinte pode reaproveitar.

### O efeito na bancada: a fase vira

E ela vira por dois motivos, não um. O primeiro é o ganho. O segundo é que a
fase `excluir` da bancada **não comparava trabalho igual** — do lado do
MySQL(R) as 20.000 instruções vão dentro de um `START TRANSACTION … COMMIT`,
que é **um** `fsync` para as vinte mil; do nosso lado eram **vinte mil**. É a
mesma família dos dois erros que a `bancada/LEIA-ME.md` conta, e desta vez o
erro era contra nós.

Medido nesta máquina, 1.000.000 de linhas, 20.000 exclusões, duas corridas de
cada (`python3 bancada/medir.py 1000000`):

| | PhxSql | MySQL(R) | |
|---|---:|---:|---|
| hoje (`fsync` por exclusão) | 6,30 s · 16,59 s | 1,45 s · 1,90 s | **perde 4,3×** |
| pedida a janela | 0,91 s · 0,96 s | 1,80 s · 1,91 s | **ganha 1,9×** |

Repare também na **estabilidade**: com a janela as duas corridas dão 0,91 e
0,96; sem ela, 6,30 e 16,59. A fase que o §6 registrava como «varia demais
entre corridas» variava porque esperava disco vinte mil vezes numa máquina
compartilhada.

Com a fase virada, **é a primeira vez que o PhxSql ganha em todas as cinco** —
e o `resultados.json` do repositório continua sendo a corrida de 10.000.000 com
o comportamento padrão, porque o padrão não mudou.
## 4.13 A trava de dados presa atrás de uma leitura de rede

O laço da réplica tomava a trava global de dados na **primeira linha** de
`alcancar_tabela` e a segurava até o fim — e no meio dela mora
`replica::puxar`, que é uma **ida e volta de rede**. `alcancar_tabela_bidi`
fazia o mesmo, e pior: tomava `self.dados.lock()` cru, sem passar pelo
`travar_dados()`, então a telemetria — que existe justamente para cronometrar
a posse da trava — **não enxergava esse caminho**.

Numa rede sã isso é invisível: a resposta chega em microssegundos. Com um
corte silencioso a leitura fica pendurada até o prazo de leitura de **30 s** do
cliente da réplica, e a trava vai junto.

### O ponto exato, e por quanto tempo — as duas testemunhas

A bancada é `bancada/replicacao/trava.py` (quatro estágios, ~1,5 min, portas
7050-7055). Ela é a versão de **loopback** dos estágios `a3-congelamento` e
`b-abraco` da bancada de contêiner: no lugar do cabo cortado há um **tubo** em
Python entre a réplica e o source, que repassa byte a byte até alguém mandar
emudecer e a partir daí segura os dois soquetes abertos sem repassar nada. Do
ponto de vista da réplica é o mesmo silêncio.

Duas testemunhas, e elas têm de contar a mesma história — a de fora (um
cliente cronometrando `ping`, que não toca na trava, contra `varrer`, que
precisa dela) e a de dentro (`totais.trava_ms` da telemetria da própria
réplica, que só existe porque `travar_dados()` é o ponto único).

Com o source escrevendo sem parar e o tubo emudecido por 40 s:

| | antes | depois |
|---|---:|---:|
| pior `ping` na réplica | 4 ms | 5 ms |
| **pior `varrer` na réplica** | **30.079 ms** | **6 ms** |
| trava na mão, pela telemetria da réplica | 35,8 s de 40 | 11,4 s de 40 |

**Os 30.079 ms são o número do defeito**, e ele bate com o prazo de leitura de
30 s configurado em `replica::ligar` — não é coincidência: é o relógio que
soltava a trava. A bancada de contêiner tinha medido 29.456 ms com `ping` de
6 ms; o loopback reproduz o mesmo em outro lugar, o que é a confirmação de que
o mecanismo não dependia do Docker.

A linha da telemetria pede um cuidado: as duas colunas **não são
comparáveis**, porque a sonda passou de 52.650 para 231.385 idas e voltas na
mesma janela — depois do conserto ela consegue perguntar cinco vezes mais, e
os 11,4 s são quase todos trabalho dela. Quem compara é o `varrer`.

### O abraço mortal do bidirecional, sem corte nenhum

No modo multi os **dois** lados rodam este laço. Cada um segurava a própria
trava esperando a resposta do outro — que só podia vir depois de o outro
soltar a dele, porque servir `replicar` chama `travar_dados()`. Isso é um
abraço mortal de verdade, e ele se desfazia apenas quando o prazo de 30 s
estourava nos dois, deixando `EAGAIN` no diário de cada um.

200.000 linhas, metade escrita em cada lado ao mesmo tempo, rede perfeitamente
sã, com um cliente sondando alfa durante a carga:

| | antes | depois |
|---|---:|---:|
| escrita do cliente | 33,0 s | **1,7 s** |
| as mesmas 200.000 num servidor sozinho | 2,4 s | 2,4 s |
| razão | **14,0×** | **0,71×** |
| `EAGAIN` novos no diário | +1 alfa, +1 beta | 0 e 0 |
| pior `ping` durante a carga | 5 ms | 11 ms |
| **pior `varrer` durante a carga** | **31.375 ms** | **67 ms** |

O `0,71×` não é mágica: são duas metades escritas em paralelo em dois
servidores contra um servidor escrevendo tudo sozinho. O que importa é que
**deixou de ser 14×**.

Com 1.000.000 de linhas o antes fica ainda mais feio, e aparece a assinatura
que o contêiner já tinha visto: **240,7 s contra 12,1 s — 19,8× — com sete
`EAGAIN` de cada lado**. Sete e sete, o mesmo empate, em máquina diferente e
por caminho diferente. Contagem igual dos dois lados é o que só um abraço
produz.

### O conserto: três fases, e a regra que sai delas

`alcancar_tabela` e `alcancar_tabela_bidi` estão partidas em três:

1. **com a trava** — garantir o database, abrir (ou criar) a tabela, ler a
   posição local; soltar;
2. **sem a trava** — ler o lote inteiro do soquete;
3. **com a trava** — reabrir a tabela, reler a posição, aplicar, soltar.

A regra geral: **nenhuma leitura de rede acontece com a trava de dados na
mão.** O `posicao` do começo da rodada já era assim; o `puxar` passou a ser.

A releitura da posição na fase 3 não é enfeite: entre a fase 2 e a 3 a trava
esteve solta e alguém pode ter escrito ali. Quando a posição andou, o lote é
**descartado** e o laço pede de novo a partir de onde a tabela está agora —
descartar custa uma ida e volta, aplicar torto custaria o dado.

### O preço, medido: zero — e a primeira conta estava errada

Partir em fases obriga a **abrir e fechar a tabela uma vez por lote** em vez de
uma vez por rodada, e cada abertura nasce com o cache de páginas do `.ndx`
vazio. Era a suspeita óbvia, e a bancada parecia confirmá-la: alcançar 200.000
eventos caiu de ~67.400 para ~55.400 eventos/s, **−17%**, e o pior `varrer` do
cliente durante a aplicação chegou a **292 ms**.

*Diagnóstico plausível não é diagnóstico medido.* O culpado não era a
abertura: era um `tabela.sincronizar()` que eu tinha deixado **dentro** da fase
3 — **400 `fsync` num alcance de 200.000 eventos, em vez de um**, todos com a
trava na mão. Tirando-o de lá e deixando um único `sincronizar` no fim do
alcance (exatamente onde ele estava antes do conserto), o custo some:

| 200.000 eventos, rede sã | eventos/s | pior `varrer` do cliente |
|---|---:|---:|
| antes do conserto | 67.406 | **2.727 ms** |
| conserto com `fsync` por lote | 55.433 | 292 ms |
| **conserto com `fsync` por alcance** | **68.000** | **76 ms** |

Mesma vazão, e o cliente da réplica deixou de esperar **2,7 segundos** durante
um alcance de rotina — porque antes a trava ficava presa pelo alcance inteiro,
mesmo com a rede perfeitamente sã. Esse número é o que mais importa no dia a
dia: ele não precisa de corte nenhum para aparecer.

A garantia de durabilidade não mudou: a `Table` que sai de escopo leva as
páginas sujas ao arquivo pelo `Drop` do `NdxFile`, que é a proteção de sempre
contra queda do **processo**; a proteção contra queda da **máquina** continua
sendo o `sincronizar` único, no mesmo lugar de antes.

### O teto de memória, que passou a ser obrigatório

Enquanto o lote era lido **com** a trava na mão, o tamanho dele era o menor dos
problemas. Agora ele mora inteiro na memória da réplica até a trava chegar, e
«quanto isso pode crescer» virou pergunta com resposta obrigatória. A resposta
não podia ser «o que o outro lado mandar»: `read_line` sem teto aceita uma
linha do tamanho da memória da máquina, e quem escolhe o tamanho é o outro
lado do fio.

Duas peças, uma em cada ponta:

- **no source**, `TETO_DO_LOTE_SERVIDO` = 16 MiB de imagem por resposta. O
  `max` do pedido conta *eventos*, e evento não tem tamanho fixo: 500 linhas de
  60 bytes são 30 KiB e 500 linhas com um memo de 200 KiB são 100 MiB, que em
  hexadecimal viram 200 MiB de texto. Lote curto não perde nada — `ate` e `fim`
  saem do que foi realmente lido, e a réplica só pergunta de novo. E o lote
  **nunca sai vazio** por causa do teto: o primeiro evento entra sempre, senão
  uma linha maior que o teto pararia a replicação para sempre em vez de
  atrasá-la;
- **na réplica**, `TETO_DA_RESPOSTA` = 128 MiB por linha lida, o dobro do que
  um par sadio produz. Estourou, a recusa é `LIMITE_EXCEDIDO` **com o número
  dentro** — o que é bem melhor que o `Killed` do núcleo, que não diz nada.

### A queda da conexão entre a leitura e a aplicação

O conserto abre uma janela que antes não existia: a conexão pode morrer com o
lote já na memória e ainda não gravado. As três saídas possíveis são perder o
lote, aplicá-lo duas vezes, ou aplicá-lo inteiro uma vez — e só a terceira
presta.

Na réplica fiel a posição nasce do **diário daqui**, então ela só anda depois
de o lote estar gravado: queda antes da fase 3 devolve exatamente o mesmo lote
na próxima rodada, e não há meio-lote possível porque o lote inteiro chega
antes de a trava ser pedida. No bidirecional a posição consumida é estado
próprio e só é gravada **depois** da aplicação; repetir um lote ali é inofensivo
porque o casamento é por chave e a regra é «mais recente vence».

Isso não se prova por teste unitário — é a lição do `BULKINSERT`. Provado por
soquete, no estágio `queda`: **10 cortes de conexão de verdade no meio de um
alcance de 200.000 eventos**, e a soma de verificação dos dois lados fecha
igual, com as mesmas linhas e os **mesmos slots** —
`('1aa1e8124df2cba0', 200000, 200000)` dos dois lados. Contar linhas não acharia
uma que atravessou errada; o número de slots é o que prova que os dois
chegaram à mesma numeração sozinhos.

### A hipótese que morreu: aumentar o lote

Com a leitura fora da trava, o tamanho do lote deixou de trocar contra o tempo
de trava presa e passou a trocar só contra memória — o que parecia o convite
óbvio para subir `LOTE` de 500 para 2.000 e amortizar a abertura da tabela.
Medido, com o `fsync` por lote ainda no lugar:

| `LOTE` | eventos/s | pior `varrer` do cliente |
|---:|---:|---:|
| 500 | 54.100 – 58.893 | 8 – 9 ms |
| 2.000 | 58.025 – 70.022 | 19 – 56 ms |

**Recusado, e por um motivo que não estava na conta de vazão:** o source lê
`max` eventos com `diario_com_imagem(desde, max)` **antes** do corte por bytes,
então quadruplicar o lote quadruplica o pior caso de memória de quem serve —
exatamente a direção contrária do teto que este mesmo trabalho acabou de
declarar. E, depois que o `fsync` saiu da fase 3, não havia mais o que
amortizar: a vazão com `LOTE = 500` já é a de antes do conserto. A hipótese
morreu duas vezes, e a segunda foi por ter deixado de existir o problema que
ela resolvia.
## 4.14 `ALTER TABLE ADD COLUMN`: a inferência era «minutos», e são 5,5 s

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
cargo build --release --bin phxsqld
python3 bancada/replicacao/trava.py            # os quatro estagios, ~1,5 min
python3 bancada/replicacao/trava.py congela    # so o corte silencioso
PHX_LINHAS=1000000 python3 bancada/replicacao/trava.py abraco
```

A guarda que trava a regressão é `trava-atras-da-rede`, no
`bancada/guardas/catalogo.py`, e o teste é
`crates/phxsql-server/tests/trava-atras-da-rede.rs` — por soquete, com um
source de mentira que emudece no `replicar`. Ele tem **prazo próprio de 8 s**
em cada sonda, e isso não é detalhe: com o defeito reposto a sonda não falha,
ela **pendura** por 30 s, e uma bateria que pendura não reprova ninguém — ela
trava. Medido: a guarda leva 14,1 s com o defeito reposto e 1,3 s com a árvore
limpa, e a mensagem de reprovação já traz o diagnóstico pronto (*«`varrer` sem
resposta em 8 s; o `ping`, que não precisa da trava, respondeu em 570 µs»*).

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

**Sobre a exclusão, honestamente.** *(Esta leitura vale para o padrão de
fábrica, e ela ficou explicada na §4.12: a variação entre corridas é o disco
sendo esperado 20.000 vezes numa máquina compartilhada. Quem pedir
`recursos.exclusao_na_janela` vira esta linha — 0,91 s contra 1,45 s do
MySQL(R) numa bancada de 1.000.000 nesta máquina.)*

É a única fase em que o PhxSql *espera
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

## 9. A guarda de reentrância da trava: medida, e ela não aparece

A trava de dados ganhou uma pergunta a mais: *«esta thread já a tem?»*. Ela
existe porque `std::sync::Mutex` não é reentrante e pedir de novo a trava que
está na mão da própria thread **parava o servidor inteiro**, para sempre, sem
log e sem pilha — aconteceu três vezes neste projeto. A guarda transforma isso
num erro nomeado. O desenho está em `TRANSACOES.md` §8.3.

O problema é onde ela mora: `travar_dados` é o caminho de **toda** leitura e
**toda** escrita. Uma pergunta a mais ali é uma pergunta por operação, no
servidor inteiro. Então ela foi medida, e não estimada.

```bash
cargo run --release -p phxsql-server --example custo-da-trava 3000000 9
```

Dois cenários sobre o mesmo `Mutex`, no mesmo processo, com as rodadas
**intercaladas** (1,2, 1,2, …) para que a deriva da máquina atinja os dois
igual:

| Cenário | mediana | faixa |
|---|---:|---|
| 1. `lock` + `unlock`, sem guarda | **15,45 ns** | 15,21..19,44 |
| 2. o mesmo, mais a guarda (uma leitura e duas escritas numa `Cell` de thread) | **15,40 ns** | 14,84..18,04 |

**A guarda = −0,05 ns por tomada**, contra um espalhamento de 4,23 ns dentro de
um cenário sozinho. O sinal negativo é o que ele parece: a diferença está
**abaixo da resolução do próprio medidor**. A conclusão honesta é «a guarda não
aparece», e não um número.

E o denominador, medido no mesmo processo, num servidor limpo:

| Operação | µs/operação |
|---|---:|
| `inserir` | 136,59 |
| `ler` (a mais barata que passa pela trava — o **pior caso** para a guarda) | 41,31 |

Mesmo tomando o limite superior do ruído como se fosse a guarda, ela ficaria em
**0,01%** da operação mais barata do servidor. Repetido em duas execuções
independentes, com −0,12 ns e −0,05 ns.

O `lock` sem disputa em 15,45 ns confirma, de outro ângulo, o 13,2 ns que já
estava registrado neste documento — e continua valendo a lição que veio com
ele: **diagnóstico plausível não é diagnóstico medido.**

---

## 12. A transação: o group commit aceito com número, e o passo seguinte morto

Medido com `cargo run --release -p phxsql-server --example custo-da-transacao`,
mediana de 5 rodadas **intercaladas** (medir um cenário inteiro de cada vez põe
toda a deriva da máquina dentro de um deles, e ela vira «custo»).

### 12.1 O group commit: 2,63×, e o que sobra não vale 1,5×

O `COMMIT` chegou chamando `sincronizar()` por tabela — um `fsync` por commit.
A decomposição de um commit de **uma linha** mostrou onde estava o tempo:

| pedaço | ms | o que é |
|---|---|---|
| commit inteiro, com `fsync` por commit | **1,199** | o comportamento anterior (`durabilidade: por_operacao`) |
| só a marca `transacao_<id>.tx` | 0,289 | escrever + `fsync` + apagar o ponto de compromisso |
| a linha, com o `fsync` amortizado | 0,050 | o trabalho de verdade |
| **o resto** | **0,860** | o `fsync` da tabela, cobrado por commit |

Uma inserção **solta**, sem transação nenhuma, custa **0,061 ms** — quase o
mesmo que a linha dentro do commit grande. A diferença inteira era o `fsync`
que a transação estava forçando e que o resto do servidor já não paga, porque
passa pela **janela de durabilidade**.

**A receita de fora mirava outro gargalo.** *Group commit* clássico amortiza
`fsync` entre commits **concorrentes**; aqui a trava única serializa tudo e
**nunca há dois commits em voo para agrupar**. O que havia para amortizar era
o `fsync` da tabela contra a janela que já existia.

**Por que adiar é seguro, e é o `.tx` que responde:** quem decide se a
transação aconteceu é a **marca**, não o `fsync` da tabela. Ela já está
sincronizada quando a passada começa, então adiar o `fsync` não adia a decisão
— adia só o momento em que o dado alcança o disco, e a marca é o bilhete que o
traz de volta. **A ordem é a peça:** a marca só é apagada *depois* de a tabela
sincronizar.

| | ms por commit de 1 linha |
|---|---|
| antes (`fsync` por commit) | 1,199 |
| depois (`fsync` na janela) | **0,457** |
| **ganho** | **2,63×** |

**E o passo seguinte morre medido.** Agrupar os `fsync` das *marcas* entre
commits é a única coisa que sobra. O piso irredutível — marca (0,289) mais
trabalho (0,050) — é **0,341 ms** contra os 0,455 de hoje: **1,34×**, abaixo do
critério de morte de **1,5×** acordado antes da medição. A marca não se adia;
ela é o ponto de compromisso. **Recusa registrada com o número**, para a ideia
não voltar sem medição.

### 12.2 `LOCK MODE AUTO` contra `EXCLUSIVE`: 1,55×, e o conflito é artificial

O caso que matou o exclusivo-por-padrão: 64 caixas, **cada um numa linha
diferente** da mesma tabela.

| modo | ms | passaram | barrados |
|---|---|---|---|
| `AUTO` (intenção na tabela, exclusiva na linha) | **50,5** | 64 | 0 |
| `EXCLUSIVE` (tabela inteira) | 78,4 | 64 | 0 |

**1,55× a favor do `AUTO`**, e nenhum dos dois perde trabalho: o `EXCLUSIVE`
não recusa ninguém, ele **serializa** — cada caixa espera o anterior soltar a
tabela. O número é o preço de uma disputa que não existia.

Por isso `AUTO` é o padrão e `EXCLUSIVE` se pede. E por isso o `INSERT` trava o
**fim da tabela** e não uma linha: ali a disputa é real, porque o próximo slot é
um só.

### 12.3 Otimista contra pessimista, na MESMA linha

64 clientes disputando a linha 1. O otimista é a janela de conflito de escrita
que já existia (o cliente manda `"versao"` e repete quando o servidor recusa);
o pessimista é a trava de linha da transação.

| | ms | gravações | tentativas gastas |
|---|---|---|---|
| otimista (`versao`) | **28,0** | 64 | **133** |
| pessimista (trava de linha) | 71,8 | 64 | **64** |

**Nenhum dos dois é o certo sempre, e o número diz por quê.** Com 64 clientes o
otimista ainda ganha em tempo e já gasta 2,1× mais tentativas; o pessimista
nunca desperdiça uma. É a **subida dessa razão** — e não o relógio — que diz
quando trocar: cada tentativa perdida do otimista é uma leitura mais uma
gravação recusada, e ela cresce com a disputa enquanto a do pessimista fica em
uma por cliente.

Numa segunda rodada o otimista gastou **303** tentativas para as mesmas 64
gravações. A variação é do próprio mecanismo: quem perde a corrida relê e tenta
de novo, e quantos perdem depende do escalonamento. **Essa instabilidade é
informação**, e não ruído — o pessimista mediu 64 nas duas rodadas.

---

## 13. Os três a um milhão de linhas — e o piso que valia 59,6% de uma barra

`bancada/comparacao/medir.py`, três rodadas, tabela de 1.000.000 de linhas,
20.000 operações nas fases pontuais, os três motores **intercalados na mesma
rodada**. A medição crua está em `bancada/comparacao/um-milhao.json` e o
gráfico sai dela por `grafico.py`, que **recusa desenhar** se o arquivo não
existir.

### Por que uma bancada nova

Já havia duas: `bancada/medir.py` (PhxSql × MySQL(R)) e `bancada/sqlite/`
(PhxSql × SQLite(R)). Somar as duas tabelas daria três colunas e **nenhuma
comparação** — as medidas são de dias diferentes, com cargas diferentes na
máquina, e parte da diferença passaria a ser do ambiente. É o mesmo erro de
comparar escalas diferentes, com outra roupa.

### As medianas

| fase | PhxSql | SQLite(R) | MySQL(R) | MySQL(R) menos o piso |
|---|---:|---:|---:|---:|
| inserir 1.000.000 | 9,928 s | **2,557 s** | 12,342 s | — |
| buscar 20.000 | 0,164 s | 0,166 s | 2,481 s | 1,002 s |
| atualizar 20.000 | **0,277 s** | 1,028 s | 3,537 s | 2,058 s |
| excluir 20.000 | 1,053 s | **0,574 s** | 4,063 s | 2,583 s |

Por operação: buscar 8,2 µs contra 8,3 e 124,1; atualizar 13,8 contra 51,4 e
176,8; excluir 52,6 contra 28,7 e 203,1. Inserção: **100.724 linhas/s** contra
391.099 do SQLite(R) e 81.025 do MySQL(R).

### O achado: mais da metade da barra de busca do MySQL(R) não é o motor dele

Os três não têm a mesma forma, e não há como dar: o SQLite(R) é biblioteca em
processo, o `carga` do PhxSql também, e **o MySQL(R) é daemon que recebe texto
por soquete**. Não existe MySQL(R) embutido nesta máquina.

O que se pode fazer é medir o tamanho disso. 20.000 instruções que não fazem
trabalho nenhum (`DO 1;`), pelo mesmo caminho: **1,479 s**.

| fase | quanto da barra do MySQL(R) é piso |
|---|---:|
| buscar | **59,6%** |
| atualizar | 41,8% |
| excluir | 36,4% |

Sem esse número teríamos publicado *«o PhxSql busca 15,16× mais rápido que o
MySQL(R)»*. Descontado o piso são **6,12×** — ainda a nosso favor, e agora é um
número sobre motores em vez de um número sobre formatos. **Vitória que vem do
formato é a mentira mais convincente que existe**, e esta casa já a publicou
três vezes: duas a favor do outro motor e uma a favor do nosso.

### Onde perdemos, dito sem rodeio

**A inserção, para o SQLite(R), por 3,88×** (2,557 s contra 9,928 s). E o
**excluir, por 1,83×** (0,574 contra 1,053). Ganhamos o `atualizar` por 3,72×,
e o `buscar` **empata**: 164 ms contra 166 ms, com as faixas inteiramente
sobrepostas (151–215 contra 158–232 ms). Empate é empate — o gráfico foi
consertado para **não contornar vencedor quando as faixas se cruzam**, porque
contornar ali seria publicar ruído da máquina como resultado.

**E o disco:** 253,6 MiB contra 57,3 do SQLite(R) e 104,0 do MySQL(R) —
**4,42×** e **2,44×**. É o preço do modelo de arquivos separados, e no celular
essa é a pergunta inteira (`docs/MOBILE.md`).

### O que a escolha do esquema do SQLite(R) vale

Ele não tem tradução única para «chave em `id` mais índice em `cidade`»:
`id INTEGER PRIMARY KEY` são duas estruturas, `NOT NULL` mais `UNIQUE INDEX`
são três. Rodam as duas, e o publicado é o `rowid` — o que **casa com o
InnoDB** e o que **favorece o SQLite(R)**:

| fase | `rowid` | `2ind` | |
|---|---:|---:|---:|
| inserir | 2,557 s | 2,914 s | 1,14× |
| buscar | 0,166 s | 0,216 s | 1,31× |
| atualizar | 1,028 s | 1,074 s | 1,04× |
| excluir | 0,574 s | 0,744 s | 1,30× |

A escolha vale de 1,04× a 1,31× conforme a fase. Publicar a que nos favorece
teria melhorado três dos nossos quatro números sem o motor ter feito nada.

### A dispersão, que é por isso que o bigode existe

O `atualizar` do MySQL(R) foi **22,969 s na primeira rodada e 3,479 s na
terceira** — 6,6× entre corridas iguais, provavelmente o `buffer pool` ainda
digerindo o milhão recém-inserido. Uma rodada só teria decidido esse número, e
teria decidido errado nas duas direções possíveis.

### A regra 1 estava sendo violada, e nenhum tempo denunciava

Ao montar esta bancada apareceu um defeito na `bancada/medir.py`: ela grava
`'2024-10-04'` em **toda** linha, enquanto o `carga.rs` e a bancada do
SQLite(R) gravam `20000 + (i % 400)`. **Dado diferente, do mesmo tamanho** — e
invisível em qualquer medida de tempo.

O conserto não foi só gravar a data certa. Nasceu a fase `conferir` do
`carga.rs`, que soma o que existe na tabela e obriga os três a chegarem ao
**mesmo estado** antes de qualquer tempo ser publicado: contagem de linhas,
soma de `valor` e soma de `cadastro`, em três marcos — depois de inserir,
depois de atualizar e depois de excluir.

O marco do meio não é enfeite: `atualizar` e `excluir` mordem exatamente os
mesmos 20.000 alvos, então no marco final o efeito do `atualizar` já
desapareceu junto com as linhas excluídas. Sem ele, a fase `atualizar` não
teria prova nenhuma.

E os totais **conferem contra a forma fechada**, não só entre si: a soma de
`valor` de 1 a 1.000.000 é 410.099.600.000 e a de `cadastro` é 20.199.500.000,
calculadas à parte. Os três motores chegaram nas duas.

**Prova real nos dois sentidos:** repor a data constante faz a bancada reprovar
com `cadastro 400.000.000` contra `403.990.000` — e 400.000.000 é exatamente
20.000 linhas × dia 20.000, que é a assinatura do defeito. Sem o defeito, ela
publica.

### Como refazer

```bash
cargo build --release --examples -p phxsql-store   # a regra do binário velho
service mysql start
python3 bancada/comparacao/medir.py                # ~15 min
python3 bancada/comparacao/grafico.py
```

## 14. A premissa da SP000011 medida: a trava global **custa**, e agora com controle

A SP000011 é «remoção do `Mutex<Instancia>` global», e ela chega com a premissa
embutida: *a trava global custa caro*. Esta casa já errou exatamente este
diagnóstico — escrevi que «o mutex era o pior pedaço, porque serializa», e
medido o `lock` sem disputa custava **13,2 ns** contra **3.456 µs** do parse do
lote: 262.000× menos. Por isso a premissa foi medida **antes** de a sprint
começar.

### Por que medir EFEITO, e não o que a telemetria já grava

A telemetria já registra `espera_ms_s` — quanto se esperou na fila da trava.
Seria o número fácil, e seria **estado**. Já houve prova aqui que passou por
engano justamente por conferir estado em vez de efeito. O que se mede é a
**vazão total com N clientes em paralelo**: se a trava serializa, N clientes
entregam o que 1 entrega.

### O confundidor que invalidaria tudo — e o controle que o mata

Clientes como *threads* do Python seriam limitados pela GIL, e a vazão ficaria
chata **mesmo com o servidor perfeitamente paralelo**: o medidor "provaria" a
serialização do servidor medindo a do próprio medidor. Duas defesas:

1. os clientes são **processos** separados, sem GIL comum;
2. e há uma curva de **controle**: o `ping`, que percorre o mesmo soquete, o
   mesmo JSON, o mesmo despacho e o mesmo cliente — e **não toma a trava de
   dados**. O que o `ping` não escalar não é culpa da trava.

A ociosidade da máquina é lida do `/proc/stat` **durante** a carga, porque
platô com CPU sobrando e platô com a máquina no teto são a mesma curva sem
esse número.

### O resultado, duas amostras, 4 núcleos

Ganho de vazão sobre um cliente:

| carga | 1 cliente | **2 clientes** | 4 clientes | CPU em 2 |
|---|---|---|---|---|
| `ping` — **sem** a trava | 1,00× | **1,99× / 1,98×** | 2,98× / 2,69× | 52–54% |
| `varrer` — leitura | 1,00× | **1,59× / 1,51×** | 1,49× / 1,56× | 52% |
| `inserir` — escrita | 1,00× | **1,49× / 1,45×** | 1,39× / 1,53× | 52–53% |

**O veredito está na coluna de 2 clientes, e não na de 4** — com 4 clientes a
máquina está em 89–99% e o número é piso, não veredito. Com **2 clientes e
metade da máquina ociosa**, o mesmo caminho entrega **1,99×** sem a trava e
**1,51–1,59×** com ela.

**A premissa da SP000011 está CONFIRMADA, e desta vez com controle:** a trava
global come cerca de **20% do paralelismo disponível na leitura e 25% na
escrita já com dois clientes**, e a CPU ociosa prova que não é a máquina. Com 4
clientes o teto da trava fica em ~1,5× enquanto a máquina oferece ~2,7–3,0×.

O que **não** se conclui daqui: qual desenho a substitui. Trava por tabela,
`RwLock` para separar leitor de escritor e MVCC (SP000016) são três respostas
diferentes, e escolher entre elas é outra medição — esta só diz que há o que
ganhar, e quanto.

### O discriminador: e a trava, ou ha um segundo gargalo embaixo dela?

A §14 fechou dizendo o que **não** concluía — qual desenho substitui a trava.
O primeiro passo dessa escolha é barato: pôr **cada cliente numa tabela
própria**. Se houvesse um segundo ponto de disputa *abaixo* da trava (páginas
do `.ndx` compartilhadas, o mesmo arquivo, o mesmo índice), tabelas separadas
escalariam melhor.

| carga | 1 | 2 clientes | 4 clientes |
|---|---|---|---|
| `ping` — sem a trava | 1,00× | 1,96× | 3,31× |
| ler — **mesma** tabela | 1,00× | 1,67× | 1,59× |
| ler — **tabelas separadas** | 1,00× | **1,70×** | 1,68× |
| gravar — **mesma** tabela | 1,00× | 1,45× | 1,35× |
| gravar — **tabelas separadas** | 1,00× | **1,43×** | 1,28× |

Escalam **igual** — 1,70 contra 1,67 na leitura, 1,43 contra 1,45 na escrita,
dentro do ruído das duas amostras da §14.

**E aqui eu quase publiquei o contrário.** O impulso foi escrever «logo trava
por tabela não compra nada» — e isso é falso, porque *trava por tabela não
existe para ser medida*: com uma trava global, clientes em tabelas diferentes
disputam exatamente como na mesma tabela, e o resultado é o previsto pela
construção. Um experimento que confirma o que já se sabia não vira veredito
sobre um desenho que ninguém escreveu.

**O que o número DIZ, corretamente:** não há um segundo gargalo escondido
embaixo da trava. Se houvesse disputa de página, de índice ou de arquivo,
tabelas separadas teriam escalado melhor, e não escalaram. Logo o ~2× de folga
que a §14 mediu está **inteiro** atrás da trava única, sem nada no caminho —
que é o que torna a SP000011 valer o trabalho, e o que faltaria saber antes de
começar.

**O que continua por medir**, e só depois de existir: se a separação certa é
por tabela, por `RwLock` (leitor com leitor) ou por MVCC (SP000016). A pista
que a §14 já dá é que a leitura custa **20× mais por operação** que a escrita
(153 contra 3.462 op/s), então ela segura a trava por muito mais tempo — o que
favorece o `RwLock`, mas *favorecer* não é medir.

```bash
python3 bancada/concorrencia/a-trava-serializa.py   # SEGUNDOS= e LINHAS= ajustam
```

### 14.1 Quanto a trava fica PRESA, e quanto disso é o `fsync` (03/09)

O parágrafo acima termina com uma promessa em aberto — *«a leitura custa 20×
mais por operação que a escrita […] então ela segura a trava por muito mais
tempo — o que favorece o `RwLock`, mas favorecer não é medir»*. Isto é a
medição, e ela responde de quebra o item que o `CONCORRENCIA.md` §8 listava
como **não medido**: quanto o `fsync` sob a trava custa em milissegundos.

**Como se mede, e por que não com um cronômetro do lado de fora.** Cronometrar
o pedido pelo cliente mede rede, JSON, despacho, portão de permissão e a fila.
A pergunta é outra — *quanto a trava fica presa*, que é o que a próxima conexão
espera. Esse número só se vê de dentro, e a telemetria já o via: o `Drop` do
`TravaMedida` soma, em microssegundos, o tempo de cada posse.

**O experimento é um PAR**, e não um número solto: `durabilidade: por_lote` (o
`fsync` a cada 200 operações) contra `por_operacao` (em todas). Mesmo binário,
mesmo caminho, mesma trava — a única diferença é a frequência do `fsync`, então
a diferença entre as curvas **é** o `fsync` sob a trava. Sozinha, uma rodada
poria na conta do `fsync` também o `open` das sete tabelas, o índice e o JSON.

Duas baterias limpas, com o `quieta.Vigia` aprovando as duas (4.000 gravações e
400 leituras cada, tabela de uma coluna com índice primário):

| durabilidade | trava presa GRAVANDO | trava presa LENDO (`varrer` 50) | pedido inteiro, gravando |
|---|---:|---:|---:|
| `por_lote` (padrão) | **121–137 µs** | 3.122–3.187 µs | 238–278 µs |
| `por_operacao` | **1.404–1.492 µs** | 2.955–3.207 µs | 1.609–1.674 µs |

**O `fsync` sob a trava custa 1.267–1.371 µs por gravação — 10,3× a 12,3× o
tempo que uma gravação segura a trava sem ele.**

**O controle é o que separa isto de um palpite**, e ele é a linha do meio: a
leitura toma a **mesma** trava e não sincroniza nada. Entre as duas baterias ela
andou **1,01×** e **0,95×** — ficou parada, como tinha de ficar. Se ela tivesse
subido junto com a escrita, quem subiu teria sido a máquina, e o arnês diz isso
em vez de deixar a deriva virar «custo do fsync».

**E o achado que muda a ordem de trabalho:** no padrão desta casa, **uma leitura
segura a trava 23× mais tempo que uma gravação** (3.122 contra 137 µs). O
«favorece o `RwLock`» da §14 deixa de ser inferência e vira número — e a
prioridade entre «encurtar as 23 seções que alcançam `fsync`» e «separar leitor
de leitor» depende da durabilidade configurada:

* com `por_lote`, a gravação é o barato: 137 µs contra 3.122 µs da leitura;
* com `por_operacao`, as duas ficam da mesma ordem: 1.404 contra 2.955 µs.

Isto conta **por operação**, e não por carga: quem decide o total é a frequência
com que cada uma é chamada, e isso o medidor não sabe. O que ele descarta é a
suposição de que o `fsync` sob a trava seja, no padrão, o pedaço grande.

```bash
python3 bancada/concorrencia/quanto-a-trava-fica-presa.py   # GRAVACOES= LEITURAS=
```

## 15. A integridade referencial: o que a garantia custa, medido

### 15.1 «Existir» não é «estar viva»: +7,0 µs por linha, e por que se paga

A conferência da chave estrangeira perguntava «esta linha existe?». A mãe
excluída de forma **suave** continua no `.reg`, com a chave dela no índice —
então um pedido novo nascia apontando para um cliente que a tela não mostra
mais. É a órfã por construção, e é o outro lado do tempo da pétrea do
`excluir_suave`, que já confere as filhas pela mesma frase.

Fechar isso põe uma leitura a mais **no laço quente**, e o custo não se estima.
Medido com `--example custo-da-fk` (20.000 filhas, 1.000 mães), mediana de três
corridas por lado, com o binário recompilado entre elas:

| | sem a pergunta | com a pergunta | diferença |
|---|---|---|---|
| chave declarada, **não** conferida | 8,13 µs/linha | 8,02 µs/linha | ruído |
| chave **conferida** | 62,84 µs/linha | 69,87 µs/linha | **+7,03 µs (+11,2%)** |
| — só a busca no índice (o resto) | 10,18 µs | 16,12 µs | +5,94 µs (+58%) |

Três coisas que o número diz, e que a estimativa não diria:

1. **Quem não pediu conferência não paga nada.** A lista `fks_conferidas` é o
   portão do custo-zero, e ele continua antes do trabalho.
2. **A conta bate com a natureza do trabalho.** O que entrou foi uma leitura de
   slot do `.reg` da mãe por linha gravada — a marca sai do **byte** da coluna
   de sistema, sem decodificar a linha nem carregar `.bin`/`.memo`. Ler a linha
   inteira para olhar um byte custaria os anexos da mãe por filha gravada.
3. **O gargalo da conferência continua sendo outro.** Abrir a mãe custa
   **46,8 µs**, 70,8% do total; a garantia nova mora nos 5,9 µs do índice. Quem
   quiser baratear a chave conferida ataca a abertura, não esta pergunta.

A tabela **sem** a coluna `softdeleted` (gravada antes da v4 do esquema) não
paga nem a comparação: a pergunta é um `Option` que sai `None`.

### 15.2 A cascata na réplica: o evento que nunca chegava

Não é custo, é correção, e mora aqui porque foi a **medição** que a achou.

A cascata do `ao_alterar` grava na filha por um handle próprio, aberto pelo
motor. Ele nascia com o padrão — imagem no diário **desligada** —, então o
evento de alteração da filha ia para o diário sem a imagem da linha, e a
réplica o recusava com «veio sem imagem». Medido em `--example
sonda-replica-fk`: o source dizia `pedidos: 2 eventos` e a réplica só aplicava
**1**, nas três ordens de entrega.

A família do defeito é a do KiB do rodapé: a garantia valia só para o caminho
que passou pela mão de quem a ligou. Quem replica liga a imagem na tabela que
**abre**; a que o motor abre por baixo tinha de sair igual.

```bash
cargo run --release --example custo-da-fk -- 20000 1000
cargo run --example sonda-replica-fk -p phxsql-store
```

## Como refazer tudo

```bash
cargo run --release --example onde-doi -- 200000       # a tabela do §2
cargo run --release --example custo-do-alter -- 50000 200000 1000000  # a §4.12
cargo run --release --example custo-do-sync            # os modos de durabilidade
cargo run --release --example custo-do-excluir -- 200000 20000 200   # o fsync da exclusao, §4.12
python3 bancada/exclusao/prova-da-queda.py             # a queda do processo, §4.12
cargo run --release --example custo-da-pagina -- 800000 200
cargo run --release --example indice-em-lote -- 1000000   # o lote do §4.3
cargo run --release --example onde-a-gpu-ajudaria -- 1000000  # o caso da GPU, §4.11
cargo run --release --example adiar-vale-quando -- 200000 # o ponto de virada
cargo run --release --example quanto-ocupa -- 1000000 5   # a ocupação do §4.7
python3 bancada/medir.py 10000000                      # o comparativo do §6
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
python3 bancada/bateria/prova-bateria.py --medir            # o §4.10
cargo run --release -p phxsql-server --example custo-da-trava 3000000 9  # a §9
cargo build --release --examples -p phxsql-server        # binario velho mede o passado
cargo run --release -p phxsql-server --example custo-da-transacao 200 64  # a §12
python3 bancada/transacoes/provar.py                     # a transacao pelo soquete
python3 bancada/concorrencia/a-trava-serializa.py        # a premissa da SP000011, a §14
cargo run --release --example custo-da-fk -- 20000 1000   # o custo da chave conferida, a §15
```
