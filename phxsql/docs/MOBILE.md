# O PhxSql no celular — medido contra o SQLite(R)

**Pergunta do dono:** *«como o PhxSql mobile pode ser melhor que o SQLite(R) e
o HFSQL(R) no celular?»*

**Resposta curta, e ela tem duas metades.** Em velocidade bruta de motor,
**não é**: o SQLite(R) grava em lote, varre uma faixa e exclui mais rápido, e
ocupa bem menos disco — os números estão na tabela da §2, e nenhum deles é
apertado. O PhxSql ganha em **ler por chave** e **atualizar**, que não é pouco,
mas não é o argumento.

Onde o PhxSql pode ser melhor não é velocidade: é que **o problema de um
aplicativo de celular não é gravar rápido, é sincronizar** — e a replicação com
imagem da linha, o diário por tabela, a janela de conflito por versão e a
trilha LGPD já existem no motor, medidos, enquanto no SQLite(R) cada um deles é
código escrito à mão ou produto pago de terceiro.

Este documento diz as duas metades com o mesmo cuidado. Um documento que só
elogia a casa não serve para decidir nada.

---

## 1. Como isto foi medido

`bancada/sqlite/` — cinco bancadas, as quatro regras de `bancada/LEIA-ME.md`
aplicadas, e as sete armadilhas que este par cobrou escritas em
`bancada/sqlite/LEIA-ME.md`.

```bash
cargo build --release
cargo build --release --examples -p phxsql-store   # a regra do binário velho
python3 bancada/sqlite/medir.py                    # ~12 min
python3 bancada/sqlite/medir.py --documento docs/MOBILE.md   # reescreve as tabelas
```

Três decisões de método valem repetir aqui, porque sem elas os números abaixo
seriam propaganda:

- **O SQLite(R) é biblioteca em processo; o `phxsqld` é servidor por soquete.**
  Comparar chamada de função com ida e volta de rede não é trabalho igual.
  Então a tabela principal (§2) põe o **motor** do PhxSql contra o motor do
  SQLite(R) — os dois em processo —, e o custo do transporte vai medido à
  parte, na §3.
- **A durabilidade é casada modo a modo** (§4). O SQLite(R) sincroniza por
  transação; o PhxSql tem três regimes. Agrupar de um lado e não do outro faz
  o número mentir, e esta casa já pagou isso uma vez.
- **Nada aqui é uma corrida só.** Cada fase roda cinco vezes e o que se publica
  é a mediana, com o mínimo e o máximo ao lado. A carga da máquina no momento
  da corrida fica gravada no `resultados.json`, porque **número de bancada
  rodada com um compilador ao lado não é o mesmo número** — foi medido: as
  mesmas fases saíram 30% mais lentas dos dois lados com a máquina ocupada, e
  as *razões* entre os motores se mantiveram dentro de 15%.
  **Uma única tabela foge da mediana**, e o motivo está escrito nela: o piso do
  transporte (§3) é a medida mais curta da bancada, dezenas de microssegundos,
  e disputa por processador só *acrescenta* a uma ida e volta. Ali o que se
  publica é o **menor** de nove corridas, porque a menor é a melhor estimativa
  do que o caminho de código cobra — a mediana mediria a máquina.

**As tabelas deste documento não são digitadas.**
`python3 bancada/sqlite/medir.py --documento docs/MOBILE.md` reescreve os
**sete** blocos marcados a partir do `resultados.json`, e reprova se o
documento marcar um bloco que o gerador não tem ou o gerador tiver um bloco que
o documento não marca. O selo da capa do dossiê passou quatro lançamentos com a
versão errada porque alguém digitou; seis colunas de microssegundos
envelheceriam mais calado ainda — e uma tabela desatualizada cercada de tabelas
certas é a que menos gente desconfia.

A corrida que gerou os números abaixo:

<!-- mobile:proveniencia:inicio -->
`200.000` linhas, numa maquina de 4 nucleos. **Cada parte carrega o carimbo do dia em que foi medida** -- refazer uma sozinha e comum, e um carimbo unico faria o documento datar todas pela ultima:

| parte | medida em | carga da maquina (1/5/15 min) |
|---|---|---|
| A · motor contra motor | 2026-08-30 17:01:32 | 2.34 · 2.24 · 3.44 |
| B · as fases pelo soquete | 2026-08-30 17:03:28 | 2.90 · 2.55 · 3.41 |
| C · durabilidade casada | 2026-08-30 17:05:16 | 3.65 · 2.83 · 3.41 |
| D · o piso do transporte | 2026-08-30 17:05:30 | 4.03 · 2.95 · 3.45 |
| E · o custo de uma chamada | 2026-08-30 17:06:45 | 4.07 · 3.21 · 3.50 |
<!-- mobile:proveniencia:fim -->

---

## 2. Motor contra motor: onde cada um ganha

Biblioteca contra biblioteca — o `carga` (o mesmo binário da bancada do
MySQL(R)) contra o módulo `sqlite3` do Python, que é extensão em C. Os dois com
**uma sincronização no fim** do lote, 200.000 linhas, cinco colunas, uma busca
por `id` e uma por `cidade`.

<!-- mobile:tabela-a:inicio -->
| fase | trabalho | PhxSql | SQLite (rowid) | SQLite (2 índices) | quem ganha |
|---|---:|---:|---:|---:|---|
| inserir em lote | 200.000 ops | 6,34 µs | 1,74 µs | 2,00 µs | SQLite 3,6× |
| ler por chave | 20.000 ops | 3,13 µs | 6,07 µs | 7,73 µs | **PhxSql 1,9×** |
| varrer faixa | 25.000 ops | 1,20 µs | 0,32 µs | 0,32 µs | SQLite 3,8× |
| ↳ *a mesma, o SQLite tocando toda coluna* | 25.000 ops | 1,20 µs | 0,44 µs | 0,43 µs | SQLite 2,7× |
| atualizar | 20.000 ops | 6,88 µs | 10,12 µs | 12,62 µs | **PhxSql 1,5×** |
| excluir de vez | 20.000 ops | 34,54 µs | 8,16 µs | 12,56 µs | SQLite 4,2× |
| **em disco** | 200.000 linhas | 49,6 MiB | 11,4 MiB | 14,5 MiB | SQLite 4,3× |
<!-- mobile:tabela-a:fim -->

O SQLite(R) aparece em duas colunas porque «chave primária em `id`» tem duas
traduções honestas para lá, e nenhuma é *a* certa: `id INTEGER PRIMARY KEY`
faz do `id` o próprio *rowid* (duas estruturas ao todo — é o que um aplicativo
escreveria, e é o mais rápido), e `id INTEGER NOT NULL` mais um índice único
dá três. Publicar só uma seria escolher o resultado.

<!-- mobile:dispersao:inicio -->
Dispersão das 5 rodadas, em segundos por fase:

| fase | PhxSql (mín · mediana · máx) | SQLite rowid (mín · mediana · máx) |
|---|---|---|
| inserir em lote | 1,189 · **1,269** · 1,395 | 0,340 · **0,349** · 0,368 |
| ler por chave | 0,059 · **0,063** · 0,065 | 0,100 · **0,121** · 0,126 |
| varrer faixa | 0,026 · **0,030** · 0,032 | 0,007 · **0,008** · 0,009 |
| atualizar | 0,135 · **0,138** · 0,141 | 0,185 · **0,202** · 0,224 |
| excluir de vez | 0,633 · **0,691** · 0,707 | 0,159 · **0,163** · 0,171 |
<!-- mobile:dispersao:fim -->

### Onde o SQLite(R) ganha, e por quê

**Inserção em lote.** É o maior peso de escrita, e a razão está medida em
`docs/DESEMPENHO.md`: **83,5% do tempo de uma inserção do PhxSql está no
`.ndx`**. O SQLite(R) com `id INTEGER PRIMARY KEY` mantém *uma* árvore a menos
que nós — a tabela dele **é** a árvore do `id` — e ainda escreve linhas de
tamanho variável, enquanto o `.reg` escreve um slot fixo.

**Varredura da faixa.** Aqui há uma diferença de motor que a bancada isolou em
vez de argumentar. O `carga` decodifica a **linha inteira** de cada rowid da
faixa; o `sum(valor)` do SQLite(R) toca uma coluna só — não é trabalho igual, e
seria fácil deixar passar. Por isso a tabela tem a **segunda linha** de
`varrer`: nela o SQLite(R) é obrigado a somar algo de **cada** coluna, e a
diferença entre as duas é o tamanho exato dessa vantagem dele. Ele fica mais
lento, e continua na frente. Ou seja: **parte da folga vem de saber ler uma
coluna sem ler as outras, e o resto é motor mesmo.** O PhxSql, hoje,
materializa a linha inteira ou nada.

**Exclusão de vez.** Esta o PhxSql compra de propósito: a exclusão física copia
a linha inteira para o `.trash` e o motivo para o `.reason` antes de liberar o
slot. O SQLite(R) não faz nada disso. É trabalho **a mais** do nosso lado, e é
a funcionalidade que o `docs/LGPD.md` chama de «a exclusão que deixa rastro».
Quem não a quiser paga bem menos — mas hoje não há como não a querer, e isso é
uma decisão a revisar para o caso do aparelho.

**Disco.** É a diferença mais larga da tabela, e num telefone ela decide.

<!-- mobile:disco:inicio -->
Onde os 49,6 MiB do PhxSql estão:

| arquivo | tamanho | por linha | do total |
|---|---:|---:|---:|
| `.reg` | 23,3 MiB | 122 B | 47% |
| `.ndx` | 17,9 MiB | 94 B | 36% |
| `.log` | 8,4 MiB | 44 B | 17% |
<!-- mobile:disco:fim -->

O `.reg` custa o dobro do arquivo inteiro do SQLite(R) por uma razão que é
**escolha de formato, não desperdício**: o slot é de largura fixa, e é isso
que faz `offset = base + (rowid−1) × slot_size` — o endereço da linha 500.000
sai de uma multiplicação, sem descer árvore nenhuma. É a mesma escolha que dá o
`buscar` mais rápido da tabela acima. O SQLite(R) guarda `Produto 00000042` em
17 bytes; nós guardamos em 40, porque a coluna é `Str(40)`.

E o `.log` é o **diário de replicação** — a última coluna da tabela diz a fatia
dele. Não é *overhead*: é exatamente a peça que a §5 defende, e a única linha
deste documento em que um custo medido e um recurso são a **mesma coisa**. Num
aparelho ele precisa de teto, e `recursos.diario_volume_mib` existe para isso.

### Onde o PhxSql ganha

**Ler por chave**, que é a operação que um aplicativo de celular faz o dia
inteiro — abrir uma ficha, carregar um cadastro, resolver uma referência. E
**atualizar**, que é a segunda.

Vale entender por quê, porque é estrutural e não sorte: achar o rowid custa
uma descida de árvore nos dois; **buscar a linha** custa, no PhxSql, uma
multiplicação e uma leitura no deslocamento calculado, e no SQLite(R) uma
segunda descida (ou a mesma, quando o `id` é o *rowid*). O preço disso é o
disco da tabela anterior. É a mesma decisão vista pelos dois lados.

---

## 3. O que o soquete custa — e por que a forma no celular é outra

O `phxsqld` de hoje é um servidor. No aparelho ele **não pode** ser (§6), mas
enquanto for, o transporte custa — e a única forma honesta de dizer quanto é
medir o transporte sozinho.

<!-- mobile:transporte:inicio -->
| pedaço da ida e volta | µs |
|---|---:|
| o JSON do cliente (Python), sem soquete nenhum | 2,90 |
| o *loopback* e as chamadas de sistema | 11,20 |
| **o caminho do pedido do `phxsqld`** | **20,21** |
| total, por pedido de 183 bytes | 34,32 |

Este é o único número do documento publicado pelo **menor** e não pela mediana: disputa por processador só *acrescenta* a uma ida e volta, então a menor corrida é a melhor estimativa do que o caminho cobra. Nas 9 corridas, a ida e volta completa ficou entre **34,32** e **59,26** µs.
<!-- mobile:transporte:fim -->

O piso foi decomposto em três porque atribuir tudo «ao soquete» seria
diagnóstico plausível em vez de medido — a lição que o mutex do Profiler
deixou. O `loopback` e as chamadas de sistema custam uma fração; **o caminho do
pedido do `phxsqld` é a maior parte**, e é ele que a frente do PhxSql Embutido
(FFI) tira do caminho.

Com o piso na mão, as mesmas fases pelo soquete:

<!-- mobile:soquete:inicio -->
| fase | pelo soquete | menos o piso | a biblioteca (A) |
|---|---:|---:|---:|
| `inserir` | 173,30 µs | 138,98 µs | 6,34 µs |
| `inserir_lote` | 18,23 µs | 18,23 µs | 6,34 µs |
| `buscar` | 132,04 µs | 97,73 µs | 3,13 µs |
| `varrer` | 56,04 µs | 56,04 µs | 1,20 µs |
| `atualizar` | 153,68 µs | 119,37 µs | 6,88 µs |
| `excluir` | 563,98 µs | 529,67 µs | 34,54 µs |
<!-- mobile:soquete:fim -->

**Uma linha por ida e volta é o pior caso possível**, e é o que a coluna
`inserir` mostra. Mas o remédio não é rede mais rápida — é **não perguntar
tanto**:

<!-- mobile:lote:inicio -->
| linhas por chamada | chamadas | µs por linha |
|---:|---:|---:|
| 1 | 20.000 | 1.403,42 |
| 10 | 2.000 | 206,27 |
| 100 | 200 | 41,47 |
| 1.000 | 20 | 17,11 |
| 5.000 | 4 | 16,01 |
<!-- mobile:lote:fim -->

Esta varredura não estava planejada. Ela nasceu de um número que não fechava:
o `inserir_lote` em blocos de 200 saiu mais lento **por linha** que em blocos
de 1.000, numa tabela menor e com a árvore mais rasa. O piso do transporte não
explicava — dividido por 200 linhas, ele some. A explicação plausível era «tem
custo fixo por chamada»; plausível não é medido, então mediu-se, e a curva diz
onde está o joelho.

Isso é conselho prático para quem escrever o aplicativo hoje, com o servidor
que existe: **o tamanho do lote vale mais que qualquer ajuste de rede.**

---

## 4. Durabilidade: o regime que um aplicativo de celular usa de verdade

Aplicativo de telefone não carrega em massa. Ele grava **uma linha quando a
pessoa toca em salvar** — e aí quem manda é o `fsync`, não o motor.

Os três regimes do PhxSql casados um a um com o compromisso equivalente do
SQLite(R), 20.000 linhas, o mesmo tamanho de janela dos dois lados:

<!-- mobile:durabilidade:inicio -->
| regime | o que se arrisca | PhxSql (lote da janela) | SQLite | razão |
|---|---|---:|---:|---:|
| `por_operacao` | nada, nem em queda de energia | 24,542 s (uma a uma, menos o piso) | 15,599 s | 1,57× |
| `por_lote` | a janela — 200 gravações ou 200 ms | 0,884 s | 0,490 s | 1,81× |
| `sistema` | o que o sistema não descarregou | 1,178 s | 0,058 s | 20,42× |
<!-- mobile:durabilidade:fim -->

A leitura que importa é a primeira linha, e ela se lê **comparando a razão dela
com a da inserção em lote na §2**: com durabilidade por operação a distância
entre os dois motores encolhe muito. O `fsync` domina o custo, e o que separa
um motor do outro fica pequeno debaixo dele. É o regime mais caro dos três e é
justamente aquele em que a diferença menos importa — boa notícia para o caso do
celular, e notícia ruim para qualquer argumento de velocidade em geral, dos
dois lados.

É também a linha mais barulhenta da bancada: `fsync` nesta máquina virtual
varia bastante, e a mesma medida saiu entre 1,2× e 1,6× em corridas de dias
diferentes. A conclusão — «encolhe muito» — sobrevive às duas; o número exato,
não. Por isso está escrito assim.

Nos outros dois regimes a distância volta, e é honesto dizer de onde ela vem —
inclusive porque a resposta fácil está errada. **Não é durabilidade**, e
**também não é a ida e volta**: a coluna do PhxSql manda as linhas no tamanho
da janela, 200 por chamada, então o piso da §3 dividido por 200 dá menos de
0,2 µs por linha e some. O que sobra é o **custo fixo por chamada** que a
tabela da §3 mediu — em blocos de 200, a curva da §3 diz o que essa linha
custa, e ela bate. Ou seja: as duas últimas linhas desta tabela comparam o
**servidor** com uma **biblioteca**, e é por isso que a primeira linha, onde o
`fsync` afoga tudo, é a que responde à pergunta do celular.

E há uma leitura desconfortável, que fica: comparado com o SQLite(R) na mesma
janela, o caminho do pedido custa mais que o trabalho que ele carrega. Isso não
é um defeito de bancada — é a forma de hoje, e é exatamente o que a §8 propõe
trocar.

---

## 5. O argumento que não é velocidade

Aqui está o caso de verdade, e cada afirmação abaixo foi conferida no código
antes de ser escrita.

### 5.1 Replicação embutida, medida — e o SQLite(R) não tem nenhuma

Esta é a diferença que decide, e ela não aparece em micro-bancada nenhuma.

Um aplicativo de celular fica **offline** e reconecta. O problema dele não é
gravar rápido: é *o que aconteceu enquanto eu estava sem rede, e como isso vai
parar no servidor sem desfazer o trabalho de mais ninguém*. Esse problema, no
PhxSql, **já está resolvido dentro do motor**:

Todas as seções da tabela abaixo são de `docs/REPLICACAO.md`, e todos os
números são de lá — não foram remedidos nesta rodada, e a bancada deles é a
`bancada/replicacao/`:

| a peça | seção | o que foi medido lá |
|---|---|---|
| `.log` v2 com a **imagem da linha** | §3 | master 34.048 linhas/s |
| aplicação na réplica | §14 | 17.450 eventos/s por réplica, três em paralelo |
| retomada pela posição depois de cair | §13 | 323 ms para voltar, 4.000 eventos alcançados |
| **«mais recente vence»** por carimbo | §12 | conflito resolvido nos dois sentidos, 1,0 s e 1,1 s |
| quatro modos, eleição e promoção | §9 a §12 | a bancada dos oito estágios |

No SQLite(R) **não há replicação nenhuma**. Sincronizar com um servidor é
código escrito à mão — uma tabela de saída, um carimbo por linha, um
reconciliador — ou produto pago de terceiro. Não é um detalhe que falta: é o
projeto inteiro do aplicativo.

E há um detalhe de desenho que vale mais que a lista: **o `.log` por tabela já
é o diário do que aconteceu offline, na forma que a réplica sabe aplicar.** Não
é um recurso a construir; é o arquivo que já está lá, ao lado do `.reg`, com a
linha dentro. A fatia de disco do `.log` na tabela da §2 é exatamente isso — e
é a única linha deste documento em que um custo medido e um recurso são a
**mesma coisa**.

**Com todas as letras, o que ainda não existe:** o empacotador da replicação
**offline** — juntar os eventos de um intervalo do `.log` num pacote e mandar.
`docs/HFSQL.md` §4 já registrava que «o formato quase a permite de graça: a
posição é o ordinal do evento no `.log`, então um arquivo com os eventos de um
intervalo é um pacote de sincronização. Falta o empacotador e a conferência de
conflito». Continua faltando. O que existe é o transporte **online**, e para
um aplicativo que reconecta ele resolve o caso comum.

### 5.2 A janela de conflito por versão

O pedido 123 é, literalmente, a peça que um protocolo de sincronia precisa: o
`.reg` guarda uma **versão por registro** desde a v1, o cliente devolve a
versão que leu, e o servidor recusa com o erro 3004 quando ela não é mais a
atual. Conferir custa 24 bytes de leitura.

Duas coisas a mais que importam para o caso do aparelho:

- **A conferência é pedida, não imposta.** Quem manda `"versao"` ganha a
  garantia; quem não manda continua como antes. Um aplicativo antigo não quebra
  no dia em que o servidor ganha a guarda.
- **O merge marca quem MEXEU, não quem perguntou por último.** Dois usuários
  que editaram campos diferentes da mesma linha saem com os dois trabalhos, sem
  escolher nada. É exatamente o caso do celular: duas pessoas, a mesma ficha,
  uma delas offline por meia hora.

### 5.3 Cifra, e o que ela cobre

| | PhxSql | SQLite(R) |
|---|---|---|
| em repouso | ChaCha20-Poly1305 **por coluna marcada**, slot v5 do `.reg` (`docs/FORMATO.md` §1), conferido contra RFC 8439 | extensão **paga** (SEE) ou *fork* de terceiro (SQLCipher) |
| no fio | aperto estilo Noise — X25519 + HKDF + ChaCha20-Poly1305 (`docs/CIFRA-DO-FIO.md`) | não se aplica: é biblioteca, não há fio |

«Por coluna marcada» é uma diferença que conta a favor num aparelho: cifrar o
CPF e deixar o nome da cidade em claro custa o que a coluna custa, e o
`offset = base + (rowid−1) × slot_size` continua valendo porque o slot cresce
16 bytes **uma vez**. Cifrar o arquivo inteiro cobraria em toda leitura.

Num aparelho que sai de casa no bolso, a cifra em repouso não é enfeite. E o
limite está escrito no próprio documento da cifra do fio: **sem o pino, ela
protege de escuta passiva e nada mais** — não é TLS e não pretende ser.

### 5.4 Trilha LGPD

A marca por coluna (`nao`, `pessoal`, `sensivel`) e o `.lgpd` registrando
quando, quem, de onde, valor antes e valor depois. Num aparelho que carrega
dado pessoal de terceiros, isso é obrigação legal e não recurso. No SQLite(R) é
gatilho escrito à mão numa tabela de auditoria — que funciona, e que ninguém
escreve até o dia da fiscalização.

### 5.5 Arquivos separados por tabela

O SQLite(R) é **um arquivo só**. O PhxSql é uma pasta por banco e sete arquivos
por tabela — o que significa que dá para sincronizar `pedidos` e não
sincronizar `fotos`, apagar uma tabela sem tocar nas outras, e mandar uma
tabela pelo backup sem mandar o banco. Num aparelho com cota de dados e de
disco, escolher o que viaja é a diferença entre caber e não caber.

E o contrário também é verdade, e vale escrever: **um arquivo só é mais fácil
de mover.** Copiar o banco do SQLite(R) para o cartão, mandar por anexo ou
restaurar de um backup é um `cp`; aqui é uma árvore de diretórios, sete
arquivos por tabela e um `.pag` por volume. Quem já perdeu meia cópia sabe a
diferença. O `empacotar`/`restaurar_backup` desta casa existe justamente porque
a cópia não é um `cp` — é código, e código tem defeito.

---

## 6. Onde o SQLite(R) ganha, e isto tem de estar escrito com a mesma clareza

Nada do que está acima muda nenhuma das cinco linhas abaixo.

**1. Já está no aparelho.** Android e iOS trazem SQLite. Usá-lo custa **zero
byte** de aplicativo. O PhxSql soma **6,8 MB** só de binário ARM64 estático
(medido em `docs/EMPACOTAMENTO.md` §7.1), mais o disco de §2. Numa loja de
aplicativos, seis megabytes e oitocentos são uma conversa.

**2. Maturidade, e a distância é enorme.** O SQLite(R) tem uma das suítes de
teste mais completas que existem e roda em bilhões de aparelhos há vinte anos.
São **269.649 linhas de C** medidas neste repositório (`docs/PLANO.md` §1),
com décadas de casos de canto encontrados por gente de verdade. Aqui são
**1.328 testes** (o número da capa do dossiê, contado por
`cargo test --workspace`). Não é a mesma ordem de grandeza, e nenhum argumento
de arquitetura compra isso.

**3. SQL muito mais completo.** O `phxsql-sql` é escrito aqui e cobre o que
cobre. O SQLite(R) tem `WITH RECURSIVE`, funções de janela, `JSON1`, FTS5,
R-Tree, planejador com estatísticas.

**4. É ACID de verdade. O PhxSql não é.** Não há transação — sem ela não há o
A nem o I, e o que existe é o desfazer de **uma** inserção quando o índice
recusa. Está sendo feita noutra frente, e enquanto não estiver pronta a frase
*ACID compliant* continua falsa sobre o PhxSql, inclusive na folha de marca.
Para um aplicativo que grava uma venda com itens, isso é decisivo: hoje, se a
terceira linha falhar, as duas primeiras ficam.

**5. É biblioteca, sem porta e sem *daemon*.** No iOS isso não é preferência,
é a regra: o sistema não permite processo em segundo plano de longa duração nem
um aplicativo escutando porta para outros usarem. No Android o sistema mata
processo em segundo plano com liberdade. **A forma do `phxsqld` de hoje não
cabe em nenhum dos dois** — e é disso que trata a §8.

---

## 7. Sobre o HFSQL(R) no celular — o que se sustenta, e o que eu não apurei

Esta seção é curta de propósito, e o motivo é o que ela mesma diz.

**O que se sustenta**, do material que está no repositório
(`docs/HFSQL.md`, lido da documentação técnico-comercial da PC SOFT, versão
2013-10):

- A folha lista **quatro tipos de replicação**, e dois deles interessam aqui:
  **com dispositivos móveis** e **offline** (sem link permanente). Ou seja, no
  desenho deles a sincronia com aparelho é um recurso do produto, não um
  projeto do cliente.
- O modelo de arquivos separados por papel é o mesmo que o PhxSql copiou de
  propósito — `.fic`/`.ndx`/`.mmo`/`.ftx` lá, sete arquivos aqui.

**O que eu NÃO apurei, e por isso não afirmo:**

- **Não tenho material sobre o HFSQL Classic embutido no WINDEV(R) Mobile.**
  Não há folha, manual nem medição dele neste repositório. Não sei o formato
  em disco, não sei o consumo, não sei o que a replicação móvel dele
  transporta nem com que garantia.
- **A frase «tabelas soltas sem cuidado» não foi apurada.** Ela é do dono, e
  fica registrada como o que é: uma impressão de quem usou, não um fato
  medido aqui. O material que **existe** aponta na direção oposta — uma folha
  que anuncia replicação móvel e offline não descreve tabelas soltas. Qual das
  duas descreve o produto de hoje, eu não sei, e a folha é de 2013.
- **Não há bancada contra o HFSQL(R).** Nem aqui nem no `bancada/`. Todos os
  números deste documento são contra o SQLite(R), e a única comparação com o
  HFSQL(R) que este projeto tem é **item a item de folha contra código**, em
  `docs/HFSQL.md` — que é outra coisa, e que a própria §5 de lá diz não ter
  um único número reproduzível do lado deles.

Apurar isto exigiria o material do WINDEV(R) Mobile, e ele não está aqui.

---

## 8. A forma certa do PhxSql no aparelho

Não é o `phxsqld` de hoje. **Nenhum mini-servidor escutando porta**, por dois
motivos que não são de gosto:

- **O iOS proíbe.** Nem processo em segundo plano de longa duração, nem
  aplicativo escutando porta para outros aplicativos usarem.
- **O Android mata.** Processo em segundo plano é encerrado com liberdade, e um
  banco que morre no meio de uma gravação é pior que banco nenhum.

A forma que cabe é a mesma que o SQLite(R) usa, e por isso ele cabe:

```
   ┌─ o aplicativo (Swift / Kotlin) ────────────────────┐
   │                                                    │
   │   camada FFI em C  ──►  libphxsql  (estática)      │
   │                          .reg .ndx .log …          │
   │                                                    │
   │   cliente de sincronia  ──── TCP ────►  phxsqld    │
   │   (o laço da réplica, do lado de cá)    no servidor│
   └────────────────────────────────────────────────────┘
```

**Duas peças, e as duas já têm metade pronta:**

1. **A biblioteca embutida.** O motor já é `phxsql-store` + `phxsql-core`, sem
   dependência externa nenhuma — é isso que faz a compilação cruzada funcionar
   de primeira. Falta a **camada FFI em C**, que é a frente do PhxSql Embutido,
   e é ela que apaga o «caminho do pedido» medido na §3.
2. **O cliente de sincronia.** O laço da réplica já existe **dentro do
   `phxsqld`** (`crates/phxsql-server/src/replica.rs`): puxa pela posição,
   aplica com a imagem da linha, resolve conflito por carimbo. No aparelho ele
   vira uma tarefa do aplicativo, que roda quando há rede — e não um servidor
   que espera alguém bater na porta.

A diferença entre as duas formas não é de arquitetura bonita: é que a primeira
**é permitida** nas duas lojas e a segunda não.

### O que falta, com o tamanho de cada coisa

| | o que falta | onde está o bloqueio |
|---|---|---|
| 1 | **camada FFI em C** (`cdylib`/`staticlib` + cabeçalho) | não existe. É a frente do PhxSql Embutido |
| 2 | **Android: o link** | o alvo `aarch64-linux-android` **compila**; falha no ligador por não achar a libc do Android (bionic). Falta o NDK, e é limite do ambiente, não do código (`docs/EMPACOTAMENTO.md` §7.5) |
| 3 | **Android: a camada JNI** | não existe |
| 4 | **iOS: tudo** | o alvo `aarch64-apple-ios` exige SDK da Apple e Xcode, que só existem em macOS. Não dá nem para tentar aqui |
| 5 | **`musl` não produz `cdylib`** | os alvos ARM estáticos de hoje não geram biblioteca dinâmica (`docs/EMPACOTAMENTO.md` §7.4). Para o aparelho o caminho é `staticlib` ligado dentro do aplicativo, ou o alvo `gnu`/`android` — e aí a restrição do `musl` deixa de importar |
| 6 | **o empacotador da replicação offline** | §5.1 — o formato quase o permite de graça, mas ele não existe |
| 7 | **transação** | §6, item 4. É o que falta para uma venda com itens ser tudo ou nada |
| 8 | **disco** | §2. A folga que a tabela mostra, num aparelho, é decisão e não detalhe: o `.log` tem teto (`diario_volume_mib`), mas o slot fixo do `.reg` é o **formato**, e mexer nele é migração |

Nada disso é «faltou notar». São, em ordem, uma frente que já corre, um SDK
que não está nesta máquina, um Mac que não existe aqui, e três decisões de
projeto.

---

## 9. O que este documento NÃO estabelece

- **Não mede o PhxSql num telefone.** Não há telefone aqui. O que existe é a
  prova de que o binário ARM64 **roda e grava** sob emulação
  (`bancada/arm/provar.sh`), e o custo real de uma placa continua exigindo a
  placa.
- **Não mede a biblioteca embutida**, porque ela ainda não existe. A coluna
  «PhxSql» da §2 é o motor chamado em processo pelo `carga`, que é a melhor
  aproximação disponível do que a FFI entregaria — mas é aproximação, não a
  coisa.
- **Não compara com o HFSQL(R)** — §7.
- **Não diz o que HÁ dentro do «caminho do pedido».** A §3 mede que ele custa
  o que custa, e mede que não é o soquete nem o `loopback`. O que ela **não**
  faz é abrir o número por dentro: quanto é `Json::analisar`, quanto são os
  portões, quanto é a trava global, quanto é serializar a resposta. Isso é a
  próxima hipótese, e ela tem medidor pronto — o `--example onde-doi` fez
  exatamente esse trabalho para a inserção. Enquanto não for medido, qualquer
  frase sobre *por que* ele custa isso é diagnóstico plausível, e esta casa já
  publicou um desses (o «mutex que serializa», que era 262.000× menor que o
  parse ao lado dele).
- **Não mede consumo de bateria, nem tempo de abertura a frio, nem
  comportamento sob pressão de memória do sistema**, que são as três coisas que
  mais reprovam um banco em telefone — e nenhuma delas se mede numa máquina
  Linux de quatro núcleos.
- **Não prova nada no Windows sobre o motor no aparelho.** A prova do `.exe`
  sob `wine` (`bancada/windows/`, `docs/EMPACOTAMENTO.md` §6.1) entrou na mesma
  rodada que este documento e responde a outra pergunta: *o binário roda?*.
  Ela não diz nada sobre desempenho, nem sobre Android, nem sobre iOS.
