# GPU e CUDA no PhxSql: onde está o processamento pesado, e ele é do tipo que uma GPU acelera?

Documento de **medição**, não de opinião. Cada número aqui sai de um programa
que está no repositório e que qualquer pessoa roda de novo:

```bash
cargo build --release --examples -p phxsql-store
cargo run --release --example onde-a-gpu-ajudaria -- 1000000
```

---

## 1. O veredito, em três linhas

> **Não compensa, e não é por pouco.** O trabalho pesado deste motor não é
> aritmético: **99,4% de uma inserção** é descida de B+tree e escrita — o
> CRC-32, único candidato lá dentro, custa **0,58%**, e mesmo instantâneo
> deixaria a inserção 1,006× mais rápida. No backup, o maior bloco contíguo de
> CPU que este motor produz, **63,0% é DEFLATE** — busca de repetição num
> dicionário que depende do byte anterior, o oposto do que uma GPU acelera — e
> o SHA-256, que é o candidato, é **12,1%**: de graça, o backup ganharia
> **1,14×**.
>
> **A agregação morre na conta do barramento, e morre em qualquer tamanho:**
> o `SUM` sobre uma coluna anda a **28.234 MiB/s** nesta CPU, **1,79× o pico
> teórico do PCIe 3.0 x16**. Não há limiar que conserte — a CPU já consome os
> bytes mais depressa do que o barramento os entregaria.
>
> **O que o dono pediu — mais velocidade no processamento pesado — existe, e
> sem CUDA:** dividir pelos 4 núcleos com a `std` que já está aqui dá
> **3,90× no ChaCha20-Poly1305, 3,59× no CRC-32 e 2,51× no SHA-256**, medidos;
> e `ORDER BY` tem **1,51×** parado numa troca de algoritmo de ordenação que
> não depende de placa nenhuma.

**O limiar em que isso muda** está na §7, e é um só: **o dia em que o
armazenamento virar colunar e plano**. Hoje uma linha é `Vec<Value>` com
`String` no monte; converter isso para um vetor plano custa uma passada
inteira pela memória — **mais do que a varredura que a GPU iria acelerar**.

---

## 2. A máquina desta medição, e o que ela não tem

Confirmado antes de qualquer conclusão, porque metade das perguntas sobre GPU
se responde olhando a máquina:

| | |
|---|---|
| CPU | Intel(R) Xeon(R) @ 2,10 GHz, **4 núcleos** |
| RAM | 16 GiB |
| `/dev/nvidia*` | **não existe** |
| `/dev/dri`, `/dev/kfd` | **não existem** |
| `nvcc`, `nvidia-smi`, `clinfo`, `rocm-smi` | **nenhum instalado** |
| bibliotecas CUDA/OpenCL no `ldconfig` | **nenhuma** |

> O único acerto de `ldconfig -p | grep -i cuda` nesta máquina é
> **`libicudata.so.74`** — que é o ICU, de internacionalização. Vale registrar
> porque é a armadilha óbvia de quem procura por substring.

**Ativar CUDA nesta sessão é impossível de fato**, e não por escolha: não há
placa, não há toolkit, e a camada de permissão desta sessão não instala nada.
O que dá para entregar é a análise medida e o desenho — que é o que está aqui.

E o que a máquina **tem**, que importa muito para a §6:

```
avx  avx2  avx512f  sha_ni  pclmulqdq  aes  sse4_2  bmi2
```

`sha_ni` é a instrução de SHA-256 em hardware; `pclmulqdq` é a multiplicação
sem carry com que se faz CRC-32 por *folding*. **As duas aceleram justamente
os nossos dois núcleos aritméticos, e as duas estão na CPU, não na GPU.**

### O ruído desta bancada, dito antes dos números

A máquina rodava outros processos durante a medição (`load average` de 4,29
com 4 núcleos). Onde a diferença entre dois números é pequena, ela está dita
como pequena; onde a comparação decide algo, ela é **emparelhada e
intercalada**, pela lição da `DESEMPENHO.md` §2.3.1. Duas medidas foram
refeitas por causa disso, e as duas estão registradas na §8.

---

## 3. Onde o tempo está, medido

### 3.1 A inserção: 99,4% dela não é aritmética

`--example onde-a-gpu-ajudaria -- 1000000`, esquema de 4 colunas, 2 índices:

| | |
|---|---:|
| inserção de 1.000.000 de linhas | 7,35 s |
| por linha | **7,35 µs** |
| CRC-32 de uma página de 4 KiB | 2,15 µs |
| páginas gravadas por linha (medido pelo cache) | 0,02 |
| **CRC-32 dentro de uma inserção** | **0,043 µs — 0,58%** |
| a inserção com o CRC **instantâneo** | 7,29 µs — **1,006×** |

O resto — 99,4% — é descida de B+tree, codificação da linha e escrita nos
`.reg`/`.ndx`/`.log`. **Descer a árvore é ponteiro atrás de ponteiro:** cada
nível precisa do resultado do nível anterior para saber qual página ler. É a
forma de trabalho que uma GPU acelera pior do que qualquer outra.

A busca por chave é o mesmo caso, isolado:

| | |
|---|---:|
| 20.000 buscas por chave | 0,086 s |
| por busca | **4,30 µs** |
| bytes de aritmética uniforme envolvidos | **zero** |

> **O CRC já foi o dono do tempo, e não é mais.** Se as 8,82 páginas que uma
> linha toca pagassem CRC — como acontecia antes do cache de write-back —
> seriam ~259% do custo de hoje. O cache comprou essa diferença inteira,
> **sem GPU e sem dependência**. O candidato que a intuição aponta é o que a
> casa já resolveu por outro caminho.

### 3.2 A consulta: varredura, agregação e ordenação

Tabela de 1.000.000 de linhas carregada em memória, ocupando 46,7 MiB:

| | tempo | vazão |
|---|---:|---:|
| varrer + filtrar (o `WHERE` sem índice) | 13,5 ms | 3.468 MiB/s · 74 M linhas/s |
| varrer devolvendo tudo, **sem ordenar** (controle) | 421,6 ms | 111 MiB/s |
| varrer + **ORDER BY** | 635,4 ms | 73 MiB/s |
| ↳ **só a ordenação**, por subtração | **213,8 ms** | 34% do `ORDER BY` |
| `SUM` puro sobre uma coluna `i64` | 0,3 ms | **28.234 MiB/s** |
| ordenar 1.000.000 de chaves `u64`, só o núcleo | 19,9 ms | 384 MiB/s |

Duas leituras que decidem coisas:

1. **O `SUM` anda a 28.234 MiB/s.** Guarde esse número: ele volta na §5 e mata
   a agregação como candidato sozinho.
2. **Ordenar as linhas custa 213,8 ms; ordenar as mesmas chaves como `u64`
   custa 19,9 ms — 10,7× menos.** A diferença não é aritmética: é comparar e
   mover `Value`, que é um enum com `String` no monte. Isso é um **item de CPU**
   (§6.3), não um caso de GPU.

### 3.3 O backup: o maior bloco de CPU contígua deste motor

É o melhor candidato do repositório, então foi medido **inteiro** — a pergunta
não é «quanto custa o SHA-256», é «quanto do backup ele é».

Tabela de 236,6 MiB, zip de 40,2 MiB (5,89×):

| parcela do backup | segundos | % |
|---|---:|---:|
| **DEFLATE** dos 236,6 MiB, a 42 MiB/s | **5,62** | **63,0%** |
| SHA-256 do manifesto, a 219 MiB/s | 1,08 | 12,1% |
| o resto (ler, montar o zip, gravar) | 2,22 | 24,9% |
| **total medido** | **8,91** | 100% |

**O maior custo é o DEFLATE, e o DEFLATE é o pior candidato possível a GPU:**
LZ77 procura repetição num dicionário construído a partir dos bytes anteriores
— cada decisão depende da anterior. Com o SHA-256 de graça, o backup cairia de
8,91 s para 7,83 s: **1,14×**.

> A primeira versão desta medida usou um bloco sintético (`(i+k) % 251`) e deu
> **339 MiB/s** de DEFLATE — uma vazão que deixava 7,8 s dos 8,91 sem dono. O
> bloco se comprimia sozinho. Com um pedaço do `.reg` **de verdade**, 42 MiB/s,
> e a conta fecha. **Dado de mentira mede mentira**, e a conta que não fecha
> foi de novo o número que tinha mais a dizer.

---

## 4. Os núcleos aritméticos, como estão escritos hoje

| núcleo | MiB/s | % do teto de leitura da RAM |
|---|---:|---:|
| CRC-32, página de 4 KiB (`.ndx`) | 1.833 | 7,6% |
| CRC-32, bloco de 1 MiB | 1.805 | 7,5% |
| **SHA-256**, bloco de 1 MiB (backup) | **219** | 0,9% |
| ChaCha20-Poly1305 selar, página de 4 KiB | 356 | 1,5% |
| ChaCha20-Poly1305 selar, bloco de 1 MiB | 360 | 1,5% |

E o teto contra o qual todos eles correm:

| | MiB/s |
|---|---:|
| copiar 64 MiB (lê 64, escreve 64) | 11.050 |
| **somar 64 MiB (só leitura)** | **24.047** |

O teto de leitura importa por um motivo que costuma escapar: **para mandar um
byte à GPU, a CPU tem de lê-lo da RAM primeiro.** A travessia não substitui
essa leitura — ela a acrescenta.

---

## 5. A conta da travessia: quem sobrevive

A regra é dada **de presente para a GPU**: núcleo de custo **zero**, volta de
custo **zero**, barramento no **pico teórico** (especificação declarada, não
medida — não há placa aqui). Se ela perde assim, perde de qualquer jeito.

A GPU só pode ganhar se o barramento entregar os bytes **mais depressa do que
a CPU já os processa**. Onde a CPU é mais rápida que o barramento, **o tamanho
do dado não importa: não há limiar.**

| núcleo | MiB/s na CPU | PCIe 3.0 x16 (15.754) | PCIe 4.0 x16 (31.508) |
|---|---:|---|---|
| **agregação `SUM`** | **28.234** | **NUNCA — 1,79× o barramento** | limiar > 1,36 MiB, teto ~1,1× |
| varredura com filtro | 3.468 | > 0,02 MiB | > 0,02 MiB |
| CRC-32 de página | 1.833 | > 0,01 MiB | > 0,01 MiB |
| ordenação de chaves `u64` | 384 | > 0,001 MiB | > 0,001 MiB |
| ChaCha20-Poly1305 | 360 | > 0,001 MiB | > 0,001 MiB |
| SHA-256 | 219 | > 0,001 MiB | > 0,001 MiB |

O limiar é o tamanho mínimo de lote para a ida se pagar contra 5 µs de
lançamento de núcleo.

**Mas o limiar do barramento é só o primeiro dos três testes**, e passar nele
não basta. Um candidato precisa passar nos três:

| teste | pergunta |
|---|---|
| **a) Amdahl** | quanto da operação real este núcleo é? |
| **b) barramento** | os bytes atravessam mais rápido do que a CPU os come? |
| **c) forma** | o núcleo é mesmo paralelo sobre os dados? |

### Candidato a candidato, com os três testes

| candidato | (a) peso | (b) barramento | (c) forma | veredito |
|---|---|---|---|---|
| CRC-32 na inserção | **0,58%** de uma inserção | passa | paralelo | **morto por (a).** De graça: 1,006× |
| B+tree (inserir/buscar) | 99,4% da inserção | — | **cadeia serial** | **morto por (c).** Nada a dividir |
| agregação `SUM` | núcleo do `SUM` | **NUNCA no PCIe 3.0** | paralelo | **morto por (b), em qualquer tamanho** |
| varredura com filtro | a consulta inteira | passa | paralelo **se fosse plana** | **morto por (c)** — ver §7 |
| ordenação | 34% do `ORDER BY` | passa | paralelo | **morto por (a)+(c)**: teto de 1,51×, e a CPU já compra 10,7× trocando o algoritmo |
| SHA-256 do backup | **12,1%** do backup | passa | **serial por arquivo** | **morto por (a)+(c).** De graça: 1,14× |
| ChaCha20-Poly1305 | só se a tabela for cifrada | passa | **paralelo de verdade** | **a forma é certa**, mas a CPU compra 5× sem dependência (§6) |
| **DEFLATE** | **63,0% do backup** | passa | **dicionário serial** | **morto por (c)** — e é o maior custo de todos |

Duas observações que os três testes tornam visíveis:

**O SHA-256 não é paralelo sobre os bytes.** Merkle–Damgård: o bloco N+1
precisa do estado que o bloco N produziu. Não se divide o hash de **um**
arquivo entre threads. O que dá para paralelizar é **arquivo contra arquivo**,
e uma tabela do PhxSql tem **sete** (`.reg`, `.ndx`, `.bin`, `.memo`, `.log`,
`.trash`, `.reason`). Uma GPU com dezenas de milhares de threads teria sete
fluxos independentes para ocupá-las.

**O maior custo medido em todo este documento — o DEFLATE, 63% do backup — é
justamente o menos paralelizável.** Não é coincidência: compressão boa é
compressão que usa o contexto anterior.

---

## 6. A alternativa que não fere a regra da casa

A regra fundadora é **zero dependências externas, só a `std`** — e ela está
medida, não afirmada: `Cargo.lock` tem **7 pacotes, todos crates deste
repositório**, e `cargo build --offline --release` termina com código 0.

### 6.1 Os 4 núcleos, medidos

`std::thread::scope`, sobre 16 blocos independentes de 1 MiB:

| núcleo | 1 thread | 4 threads | ganho |
|---|---:|---:|---:|
| **ChaCha20-Poly1305 selar** | 45,0 ms | 11,6 ms | **3,90×** |
| **CRC-32** | 8,8 ms | 2,4 ms | **3,59×** |
| **SHA-256** | 72,5 ms | 28,9 ms | **2,51×** |

Perto do 4× ideal, porque estes três são presos à **conta**, e não à banda de
memória. O contraste com a varredura é o que ensina: `paralelo.rs` documenta
que a varredura em memória rende **1,8× e não 4×**, porque ali o limite é a
RAM. **Quem é preso à banda não divide — e é exatamente quem a GPU também não
ajudaria**, pelo mesmo motivo, com um barramento ainda mais estreito no meio.

O motor já tem a peça: `phxsql_core::paralelo::mapear_faixa`, com teto
configurável por `recursos.threads`.

### 6.2 SIMD: o caminho existe, e o compilador sozinho já dá — e tira

Rust estável chega a AVX2 e a `sha_ni` por `std::arch`, **sem crate nenhuma**.
Antes de escrever intrínseco, porém, valia medir o que o compilador acha
sozinho. Os mesmos binários, só trocando a bandeira, corridas intercaladas:

| núcleo | `x86-64` genérico | `-C target-cpu=native` | |
|---|---:|---:|---:|
| **ChaCha20-Poly1305** | 355–359 MiB/s | **452–460 MiB/s** | **1,28×** |
| CRC-32 | 1.799–1.821 MiB/s | 1.805–1.806 MiB/s | 1,00× |
| **SHA-256** | 215–221 MiB/s | **143–144 MiB/s** | **0,66× — pior** |

Três resultados, e o terceiro é o que ninguém esperava:

1. **O ChaCha20 vetoriza sozinho: 1,28×.** Faz sentido: são quatro colunas de
   estado independentes, que é o que o autovetorizador procura.
2. **O CRC-32 não muda.** A tabela *slice-by-16* já é o que ela é: consulta de
   memória, não aritmética vetorizável. Para acelerá-lo de verdade seria
   preciso `pclmulqdq` escrito à mão — e o polinômio é o nosso (0xEDB88320),
   então dá, mas é código `unsafe` a conferir contra vetor oficial.
3. **O SHA-256 fica 1,5× MAIS LENTO com `native`.** Reproduzido em três pares
   intercalados, sempre no mesmo sentido. É o aviso prático: **`target-cpu=native`
   não é ganho de graça**, e ligar essa bandeira no build de lançamento teria
   trocado 1,28× no ChaCha por −34% no SHA-256, sem ninguém notar.

O que isso **não** mede: o ganho de escrever `sha_ni` e `pclmulqdq` à mão. Esse
número não existe aqui e **não vai ser citado de fora** — é a próxima bateria,
se alguém quiser abri-la, e ela tem de vir com os vetores FIPS 180-4 do lado.

### 6.3 O `ORDER BY`, que é o achado de graça desta rodada

Da §3.2: ordenar 1.000.000 de linhas custa **213,8 ms**; ordenar as mesmas
1.000.000 de chaves como `u64` custa **19,9 ms**. **10,7×**, na mesma CPU, sem
thread e sem SIMD — só não movendo `Value` com `String` dentro do laço de
comparação.

O desenho é o clássico: ordenar um vetor de `(chave, índice)` e permutar as
linhas **uma vez** no fim. Teto no `ORDER BY` inteiro: de 635,4 ms para ~422 ms,
**1,51×** — que é, por coincidência exata, todo o teto que uma GPU **de graça**
teria neste mesmo item. **A CPU compra o mesmo, sem placa.**

Não implementado nesta rodada — está registrado como item, com o número na
mesa, que é como esta casa registra.

---

## 7. O limiar em que o veredito muda

«Não compensa» sem dizer **quando compensaria** é opinião com número em cima.
O limiar é um só, e não é de tamanho de dado — é de **formato**:

> **O dia em que o armazenamento virar colunar e plano.**

Hoje uma linha é `Vec<Value>`, e `Value` é um enum com `String` no monte. Para
mandar isso a uma GPU é preciso primeiro **serializar para um vetor plano de
largura fixa**. Essa conversão é uma passada inteira pela memória, com
alocação — ela custa **mais do que a varredura de 13,5 ms que a GPU iria
acelerar**. É por isso que a varredura morre no teste (c) e não no do
barramento.

Com formato colunar plano, a conta muda de lado, e dá para escrevê-la com os
nossos números:

| | |
|---|---:|
| varredura de 46,7 MiB na CPU, hoje | 13,5 ms |
| subir 46,7 MiB pelo PCIe 3.0, no pico | 2,96 ms |
| varrer 46,7 MiB numa GPU a ~900 GB/s | ~0,05 ms |

Com o dado **residente** na VRAM — subido uma vez, consultado muitas —, a
varredura passaria a custar centésimos de milissegundo contra 13,5 ms. Aí sim
o ganho seria de uma ordem de grandeza, e a conversa mudaria.

**As três condições, juntas, para reabrir o caso:**

1. **formato colunar plano** por coluna, sem indireção por valor — que é
   mudança de formato, não de driver, e a casa tem regra sobre isso
   («mudança de formato entra cedo»);
2. **conjunto quente residente em VRAM**, para o PCIe ser pago uma vez e
   amortizado por muitas consultas;
3. **consulta presa à conta e não à banda** — hoje o `SUM` anda a 1,79× o
   PCIe 3.0, então mesmo residente ele não seria o item que paga a placa.

E há um limiar que **não** muda com dado nenhum: **a inserção e a busca por
chave**. Elas são cadeia de dependência por definição do formato — a B+tree e
a ordem de digitação. Nenhum tamanho de tabela, nenhuma placa e nenhuma
geração de PCIe altera isso.

---

## 8. O custo declarado de adotar CUDA

Sem retórica, e sem tratar como objeção moral — a decisão é do dono. Cada
item é uma consequência verificável:

| custo | hoje | com CUDA |
|---|---|---|
| **compilar offline** | `cargo build --offline` sai com **código 0**, medido nesta rodada | deixa de sair: `nvcc` e `libcudart` são externos ao `std` e à árvore |
| **dependências** | **7 pacotes no `Cargo.lock`, todos nossos** | mais o toolkit NVIDIA(R) (~3–6 GiB) e, em Rust, ou uma crate de ligação ou FFI `unsafe` escrito à mão |
| **compilação cruzada para Windows** | funcionou de primeira, é a prova da regra | passa a exigir toolchain de GPU **dos dois lados** |
| **rodar em máquina sem placa** | um binário, roda em tudo | ou não roda, ou **dois caminhos** e um teste para cada |
| **testar** | um caminho, e o CI é esta máquina | o caminho GPU **não é testável aqui** — esta sessão é a prova: sem placa, sem driver, sem toolkit |
| **a regra fundadora** | zero dependências externas | quebrada, e ela é o motivo de os quatro itens acima valerem hoje |

O item que mais pesa é o penúltimo, e ele é próprio desta casa: um caminho que
o CI não exercita é um caminho que ninguém prova. A regra escrita aqui é *«o
que depende do sistema operacional se prova contra o sistema operacional»* —
um caminho CUDA dependeria de um driver que a bancada não tem.

**E o custo se pagaria com quê?** Com os tetos da §5: 1,006× na inserção,
1,14× no backup, e «nunca» na agregação. Nenhum deles paga a lista acima.

---

## 9. O que fazer com o pedido, então

O pedido é legítimo — o dono quer o processamento pesado mais rápido. A
resposta medida é que o ganho existe e está **do lado da CPU**:

| item | ganho medido | custo |
|---|---:|---|
| dividir SHA-256/CRC/cifra pelos 4 núcleos | **2,51× a 3,90×** | `std::thread`, a peça já existe (`paralelo.rs`) |
| ordenar por chave `u64` + permutar uma vez | **1,51×** no `ORDER BY` | um algoritmo, nenhuma dependência |
| `pclmulqdq`/`sha_ni` à mão, por `std::arch` | **não medido** | `unsafe` + vetores oficiais; é a próxima bateria |
| CUDA | 1,006× a 1,14×, e «nunca» na agregação | a §8 inteira |

E o maior de todos continua sendo o que a §3.3 achou: **63% do backup é
DEFLATE a 42 MiB/s**. Ele não é candidato a GPU, mas é candidato a atenção —
e nunca foi medido antes desta rodada.

---

## 10. A hipótese que morreu, e o que ela ensinou

Registrada porque **hipótese que morre medida é resultado válido**, e é o que
impede a ideia de voltar sem número:

- **«GPU acelera o processamento pesado do banco»** — morreu em três testes
  independentes: Amdahl (o candidato é 0,58% da inserção e 12,1% do backup),
  barramento (o `SUM` já é 1,79× o PCIe 3.0) e forma (B+tree, SHA-256 e
  DEFLATE são cadeias seriais).
- **«o backup é dominado pelo SHA-256»** — morreu medido: é **DEFLATE, 63%**,
  contra 12,1% do SHA-256.
- **«`target-cpu=native` é ganho de graça»** — morreu medido, e no sentido
  contrário: **SHA-256 fica 0,66×**.
- **«o CRC-32 é o alvo dentro da inserção»** — já tinha morrido antes desta
  rodada, quando o cache de write-back o levou de ~259% do custo atual para
  0,58%. O candidato que a intuição aponta é o que a casa já resolveu.

E a armadilha de medição que esta rodada pagou, para a próxima não pagar:

> **O otimizador apaga o laço que você quer medir.** A primeira versão deste
> medidor deu **12,5 bilhões de MiB/s** de banda de memória e **192 milhões de
> MiB/s** de CRC-32 — porque `crc32(&pagina)` é função pura de um `slice` que
> não muda dentro do laço, e o compilador a ergueu para fora. `std::hint::black_box`
> nos dois lados — entrada e saída — conserta. Números impossíveis são fáceis
> de pegar; o perigoso é o mesmo erro rendendo um número **plausível**.

---

## Como refazer tudo

```bash
cargo build --release --examples -p phxsql-store    # senão o medidor mede o passado
cargo run --release --example onde-a-gpu-ajudaria -- 1000000

# a conferência do target-cpu=native (§6.2), em binários separados
RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=/tmp/alvo-nativo \
  cargo build --release --example onde-a-gpu-ajudaria -p phxsql-store
```

E a conferência da máquina, que vem antes de tudo:

```bash
ls /dev/nvidia* /dev/dri /dev/kfd 2>&1     # não existem aqui
command -v nvcc nvidia-smi clinfo rocm-smi # nenhum
grep -m1 flags /proc/cpuinfo | tr ' ' '\n' | grep -E '^(avx2|avx512f|sha_ni|pclmulqdq)$'
```
