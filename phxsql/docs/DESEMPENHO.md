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

O que eu **não** faria agora: WAL, MemTable de escrita e group commit. Eles
resolvem o gargalo do InnoDB, e a medição diz que ele não é o nosso.

---

## Como refazer tudo

```bash
cargo run --release --example onde-doi -- 200000       # a tabela do §2
cargo run --release --example custo-do-sync            # os modos de durabilidade
cargo run --release --example custo-da-pagina -- 800000 200
cargo run --release --example indice-em-lote -- 1000000   # o lote do §4.3
cargo run --release --example adiar-vale-quando -- 200000 # o ponto de virada
python3 bancada/medir.py 10000000                      # o comparativo do §6
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```
