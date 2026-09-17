# Parecer do papel C (DBA) — 20 caixas, 1 servidor: o que o formato permite e o que ele PROÍBE

**Data:** 17/09/2026, 14:1x UTC · **Commit lido:** `32b5b19` · **Papel:** C (DBA
sênior). **Só leitura** — nenhuma linha de código foi escrita nesta frente, e a
pergunta de inventário («o que existe hoje») é de outra.

**A pergunta do dono:** *«operação de supermercado, 20 caixas e 1 servidor. O
servidor cai, os operadores não percebem, a gravação é local, e ao subir o
servidor a replicação sobe imediata e transparente. Hoje o PhxSql faria isso?»*

**O que este parecer responde:** o que **este formato em disco** e **estas
pétreas** permitem num arranjo de 20 escritores independentes convergindo para
um, e onde eles dizem **não**. Cada afirmação traz arquivo e linha, ou a origem
e a data do número. Onde a resposta só sai medindo, está escrito que só sai
medindo, e a bancada que a mediria está na §7.

---

## 0. A redução do cenário a uma pergunta de formato

O enunciado tem quatro exigências, e três delas são de formato:

1. **«a gravação é local»** → cada caixa é um `phxsqld` que aceita escrita com o
   servidor fora do ar. Formato: cada caixa tem o **seu** `.reg`, o **seu**
   contador de `rowid`, o **seu** `rownum` e o **seu** contador de `Sequence`
   (`FORMATO.md` §«Ordem de digitação», §«A coluna de sistema `rownum`»).
2. **«os operadores não percebem»** → nenhuma escrita do caixa pode depender de
   uma leitura do servidor. Formato: **toda conferência que lê outra tabela é
   local** — `conferir_fks_com` abre a mãe do **disco daqui**
   (`table.rs:1375`).
3. **«ao subir, a replicação sobe imediata e transparente»** → 20 diários
   convergem num `.reg` só, sem operador no meio.
4. «transparente» → sem perda e sem intervenção humana.

Nesta casa, (1)+(3) juntos são **20 escritores e um ponto de convergência**. O
motor tem exatamente dois caminhos de convergência, e eles não são
intercambiáveis. É a §1.

---

## 1. Rowid ou chave: o cenário exige o bidirecional, e o unidirecional está MORTO nele

### 1.1 O que cada caminho faz

| | `aplicar_evento` (unidirecional) | `aplicar_por_chave` (bidirecional) |
|---|---|---|
| identidade da linha | **rowid** | **chave única de UMA coluna** |
| onde mora | `table.rs:3984-4106` | `servidor.rs:4737-4900` |
| confere o rowid? | **sim, e PARA** — `table.rs:4083-4090` | não: rowid e `rownum` são locais |
| exige | um source só por tabela | `chave_unica` (`bidirecional.rs:158-174`) |
| conflito | não existe | «mais recente vence» pelo carimbo |

### 1.2 Por que o unidirecional morre por construção aqui

O `aplicar_evento` insere e compara: se o rowid que saiu **aqui** não bate com o
que veio de **lá**, devolve `Corrompido` e a replicação daquela tabela para
(`table.rs:4083-4090`). Isso é uma conferência forte e de graça — e ela vale
**porque só um servidor escreve**.

Com 20 caixas, a colisão é **certa no segundo evento da segunda caixa**: o
`.reg` de cada caixa começa em `rowid = 1` e nunca reaproveita slot, então a
venda 1 do caixa 3 e a venda 1 do caixa 7 são as duas `rowid = 1`. O primeiro
lote que o servidor aplicasse da segunda origem cairia no fail-stop.

E **não há atalho**, porque o pedido 292 já decidiu o irmão disso e a decisão
vale aqui: *«o unidirecional para pelo mesmo desenho e assim fica, porque lá a
aplicação é por rowid e o `.reg` nunca reaproveita slot — pular um evento
deslocaria todos os rowids seguintes e trocaria um problema que PARA por um que
DIVERGE EM SILÊNCIO»* (`PENDENCIAS.md`, pedido 292). A restrição que causa a
divergência **é a ordem de digitação**, e ela é pétrea.

### 1.3 O caminho que sobrevive, e o preço exato

**O cenário exige o `aplicar_por_chave`** — papel `multi` nos 21 nós. Ele
sobrevive à pétrea, e a sobrevivência está escrita no cabeçalho do módulo:

> *«A ordem de digitação é sagrada EM CADA SERVIDOR: cada `.reg` mantém a SUA
> ordem de chegada, e o insert local de A e o de B podem ganhar o mesmo rowid.»*
> (`bidirecional.rs:34-40`)

Ou seja: **a pétrea não se quebra, ela se relativiza ao nó**. E é aqui que o
DBA tem de dizer a consequência inteira, porque ela não está escrita em lugar
nenhum com estas palavras:

> **Com 20 caixas, «a ordem de digitação» deixa de existir como ordem única do
> negócio.** No `.reg` do servidor as vendas ficam na ordem em que **chegaram
> pela rede**, que é a ordem em que cada caixa reconectou — não a ordem em que
> foram digitadas na loja. O `rowid` e o `rownum` do servidor continuam sendo
> ordem de digitação **dele**, e dele é a chegada.

Isso não é defeito: é o que o formato promete e nada mais. Mas quem ler o `.reg`
do servidor esperando a ordem cronológica da loja vai ler errado, e **hoje não
há coluna no dado que devolva a ordem da loja** — só o carimbo do evento no
`.log`, que é por tabela e não é campo da linha. É exatamente o buraco que o
`rowtime`/`rowstamp` do `PSCH` v10 fecha (§4 e §6).

### 1.4 O que o bidirecional recusa antes de começar

- **Tabela sem chave única de UMA coluna é recusada**, com o motivo gravado em
  `replicacao_estado.recusas` (`servidor.rs:4490-4504`). Chave **composta**
  também fica de fora (`bidirecional.rs:160`). Num modelo de supermercado,
  `itens_da_venda` com primária composta `(venda, sequencia)` — que é a
  modelagem natural — **não replica**. Ela precisa de uma coluna única própria.
- **Mais de dois nós nunca foi provado.** `REPLICACAO.md:778-780`: *«Não é para
  mais de dois ainda. O desenho (origem por evento) suporta malha, mas só o par
  foi provado na bancada.»* 21 nós é malha em estrela, não par.

---

## 2. A identidade da venda: globalmente única SEM coordenação, e o que isso custa no disco

### 2.1 O que existe hoje, medido no fonte

| peça | existe? | onde | serve offline? |
|---|---|---|---|
| `Sequence` (contador do `.reg`) | sim | `table.rs:2612-2641`, contador em `reg.rs:747`, bytes 36..44 do cabeçalho | **sim, e é o perigo** |
| `inicio`/`passo` por nó | **não** | decidido (pedido 290), **falta implementar** | — |
| `Uuid` v7 monotônico | sim | `uuid.rs:265-289`, contador de 12 bits + 62 bits sorteados | **sim** |
| `Uuid` que nasce sozinho de valor NULO | **não** | `AUTONUMBER.md` §A.5 e §B.2.5 | — |
| `Uuid` pedido pelo cliente com a palavra `"novo"` | **sim** | `valores.rs:1010-1019` | **sim** |
| `colisao_de_criacao` (o olho que vê o estrago) | sim | `bidirecional.rs:146-149` | detecta, não impede |

**A resposta direta:** hoje, quem gera o número da venda com o servidor fora do
ar é **o `phxsqld` do próprio caixa**, e só há uma forma que funciona sem
coordenação: o `Uuid` v7, pedido em texto com `"novo"`. A `Sequence` **também**
funciona offline — e é por isso que ela é o perigo, não a solução.

### 2.2 A `Sequence` como está: 20 caixas na MESMA faixa é perda de dado, medida

Todo caixa começa em 1 e anda de 1. Venda 1 existe em 20 `.reg`. No servidor, o
casamento é **por chave** com «mais recente vence»: das 20 vendas de número 1,
**sobrevive uma** — e as outras 19 morrem no `.reg` de origem também, porque o
evento volta e sobrescreve.

O número já está medido nesta casa, no par: **4 inserções → 2 linhas**
(`AUTONUMBER.md` §A.3, bloco 24 da sonda de 07/09/2026; pedido 229(a)). O olho
que vê existe — `colisao_de_criacao` conta e grita (`servidor.rs:4806-4825`) —
mas ele **não impede**: conta o estrago depois de feito.

**Com 20 caixas o defeito não é 10× pior que no par: ele é o modo normal de
operação.** Toda venda do dia colide, porque todos os contadores partem do
mesmo lugar e andam junto.

### 2.3 As duas identidades possíveis, no disco

Contas feitas nesta frente a partir de `FORMATO.md` §«Chave completa» (entrada
de folha = `key_len + 8`; nó interno = `+8`) e §«Página da árvore» (entradas a
partir do offset 32 de uma página de 4096):

| chave | componente | entrada de folha | por página | entrada interna | por página |
|---|---:|---:|---:|---:|---:|
| `Sequence`/`UInt8` | 1+8 = 9 B | 17 B | **239** | 25 B | **162** |
| `Uuid` v7 | 1+16 = 17 B | 25 B | **162** | 33 B | **123** |

Consequência, com ocupação teórica de 100% (o teto, nunca o real):

| linhas | `Sequence`: folhas / altura / MiB | `Uuid`: folhas / altura / MiB |
|---:|---|---|
| 1.000.000 | 4.185 / 3 / 16,3 | 6.173 / 3 / 24,1 |
| 10.000.000 | 41.842 / 4 / 163,4 | 61.729 / 4 / 241,1 |
| 100.000.000 | 418.411 / 4 / 1.634,4 | 617.284 / 4 / 2.411,3 |

**A altura não muda; o tamanho sim: +47,5%.** Quem paga isso é a memória de
página — o `.ndx` guarda 2.048 páginas por arquivo aberto, 8 MiB
(`FORMATO.md` §«As páginas quentes ficam em RAM»).

### 2.4 A conclusão de localidade, e ela INVERTE a expectativa

A intuição da casa é «v4 espalha, v7 não» (`uuid.rs:1-20`), e ela está certa
**num escritor só**. Com 20 caixas convergindo, a conta muda, e é o DBA que tem
de dizer:

- **`Sequence` com faixa (`passo=N`, `inicio=i`)**: cada caixa tem a **sua**
  série crescente, e as séries **não se intercalam por tempo, e sim por
  volume de venda**. O caixa movimentado está em 1.000.001 e o caixa parado em
  10.020. No `.ndx` do servidor isso são **20 pontos de anexação**, um por
  caixa — 20 folhas quentes, 80 KiB, **cabem folgado nos 8 MiB de cache**. A
  localidade se preserva, multiplicada por 20 em vez de perdida.
- **`Uuid` v7**: uma folha quente só **enquanto todos os caixas estão em dia**.
  O caixa que passou quatro horas fora traz chaves com carimbo de **quatro
  horas atrás** — elas entram no **meio** da árvore, numa região construída por
  anexação e portanto cheia, forçando divisão de página. O ganho do v7 some
  exatamente no evento para o qual este projeto existe: **a reconexão**.

**Parecer de C sobre a identidade:** a `Sequence` **com faixa** é a melhor
chave para este cenário, e por quatro motivos de formato, não de gosto:

1. 8 bytes contra 16 — 47,5% menos `.ndx` (§2.3);
2. localidade preservada na reconexão, que é onde o v7 a perde (§2.4);
3. **o número diz de que caixa a venda saiu, e isso custa ZERO byte**:
   `numero mod passo == inicio`. Com `Uuid` a origem da linha não está no dado
   — está só nos 2 bytes de origem do evento do `.log` (`log.rs`, cabeçalho de
   44 bytes), que não é campo da linha e some do retrato;
4. é um número que vai para o **cupom**, e a casa já recusou por escrito
   reaproveitar número de `Sequence` de linha excluída (`AUTONUMBER.md` §B.3):
   *«um número reemitido é dois documentos com a mesma identidade»*.

**E o preço, que é a parte dura:** o `passo` vai gravado no `PSCH`
(decisão do dono, 17/09 07:10), e **`passo` não se muda depois**. Mudá-lo
reabriria faixas já usadas — é a ordem de digitação sendo quebrada pela porta
dos fundos. Então **o número de caixas tem de ser escolhido uma vez, com folga,
antes do primeiro cupom**, e o 21º caixa que a loja comprar em 2028 **não cabe**
se o `passo` tiver sido 20.

O `Uuid` v7 é a saída que **não** tem esse teto, e é por isso que ele fica como
alternativa declarada, não como recusa: quem não consegue fixar N escolhe 16
bytes e perde o «de que caixa veio».

### 2.5 O índice único aguenta? Sim — e o problema não é o índice

`ndx.existe` é consultado antes de gravar (`table.rs:3158-3166`) e **não é
calado na réplica** (§3). Ele aguenta a chave dos dois tipos. O que **não**
aguenta é a operação: chave repetida vinda de uma segunda origem **para o par
naquela tabela** (pedido 292, forma nova), e o desempate é humano
(`replicacao_pular`). Com 20 caixas na mesma faixa isso não é caso-limite: é a
primeira venda do dia.

---

## 3. «Só existe filho se o pai existir primeiro»: o que `julga_integridade` cala, e o que NÃO cala

`table.rs:1315-1317` — uma linha: `!self.como_replica`. A marca liga em par no
`aplicar_evento` (`:3992-3996`), no `inserir_replicado` (`:4019-4025`), no
`atualizar_replicado` (`:4028-4035`) e no `excluir_de_vez_replicado`
(`:4040-4047`).

### 3.1 O que ela CALA (varredura dos sete portões)

| portão | linha | o que deixa de acontecer no servidor |
|---|---:|---|
| FK na inclusão | `table.rs:3114` | **não confere se o pai existe** |
| FK na alteração | `table.rs:3414` | idem |
| FK na restauração | `table.rs:3745` | idem |
| `conferir_filhas` na exclusão | `table.rs:3638` | **mata o pai que tem filhos** |
| planejamento da cascata | `table.rs:1844` | não recascateia (certo: o evento vem por conta própria) |
| `DEFAULT`, coluna calculada e `CHECK` | `table.rs:2902` | **não valida regra de esquema** |
| `CHECK` de coluna nova | `table.rs:874` | — (caminho de DDL) |

Traduzido para a loja: **no servidor, a regra primordial da integridade não é
imposta.** Nem a conferência do pai, nem a proibição de matar pai com filhos.

Isso está certo como decisão, e a decisão tem número: conferir de novo na
réplica custou **0 de 2 eventos** nas ordens «mãe primeiro» e «filha primeiro»,
e **1 evento a mais que o source** na entrelaçada (`INTEGRIDADE.md` §3, tabela).
A garantia é da **origem**; a da réplica é de **fidelidade**.

### 3.2 O que ela NÃO cala — e é o que trava o par

- **Unicidade.** O laço de `table.rs:3158-3166` roda sempre. Foi o achado do
  pedido 292, e a decisão do dono (forma nova, 17/09 09:0x) é **parar o par
  naquela tabela, marcado e gritado**, com saída humana por `replicacao_pular`.
- **Aridade e tipo** (`conferir_aridade`), **coluna obrigatória**
  (`montar_payload`, `table.rs:2658`).

### 3.3 A conclusão que só o papel C pode dar, e ela é o eixo deste parecer

> **A integridade referencial do PhxSql é local a cada `.reg`, e a UNIÃO de 20
> bancos íntegros não é um banco íntegro.**

Cada caixa impôs a pétrea contra o **subconjunto dele**. O servidor não
reconfere — e **não pode** reconferir, porque reconferir é a doença medida do
`INTEGRIDADE.md` §3. Então o resultado da convergência é um banco em que:

- **órfã transitória é NORMAL** — a venda e os itens são tabelas diferentes, a
  replicação anda **por tabela** e não há ordem entre elas
  (`REPLICACAO.md`:820-830, «a posição é por tabela»);
- **órfã permanente é POSSÍVEL e ninguém a vê** — se o disco do caixa 7 morrer
  entre o evento de `itens` e o de `vendas`, os itens ficam no servidor
  apontando para uma venda que nunca vai chegar. Nada no motor acusa: a
  conferência está calada por desenho, e o `.log` **não tem id de transação**
  (`log.rs:134-158`, confirmado no parecer de replicação de 17/09 §1);
- **o commit do caixa NÃO é atômico do outro lado.** `REPLICACAO.md`:908-911
  ainda diz *«Não há transação, então não há ordem global entre tabelas»* — a
  premissa caducou (há transação desde o pedido 162), o **efeito não**: o par
  venda+itens que o caixa commitou junto chega ao servidor em rodadas
  diferentes, por tabelas diferentes, e **não há marca dizendo que faltava
  metade**.

E o lado do excluir é pior que o do inserir, porque é **silêncio com dano**:
um `excluir` replicado chama `excluir_de_vez_replicado`, que **não** chama
`conferir_filhas`. Uma venda cancelada num caixa e um item chegando de outro
caminho deixam órfã no servidor **sem erro nenhum** — a pétrea «nunca se mata o
pai que tem filhos» é imposta nos 20 caixas e **ausente exatamente no arquivo
que a loja inteira consulta**.

**O que existe para ver isso**, e é a substituição honesta: o verificador de
consistência (`phxsql-store/src/integridade.rs`,
`--example conferir-integridade`, `INTEGRIDADE.md` §5) — ele **relata** órfã com
tabela, chave, rowid e valor, e **não conserta**, pelo motivo escrito ali: órfã
pode ser lixo de importação ou a única cópia de um pedido. Num arranjo de 20
caixas ele deixa de ser ferramenta de diagnóstico e vira **rotina de operação
diária**, e isso precisa estar no contrato antes de a primeira loja subir.

---

## 4. «Impossível o filho ter a MESMA data do pai»: alcançável, e só dentro de um nó

### 4.1 O estado da decisão

O dono decidiu em 17/09 07:10, e a régua dos motores corrigiu em 07:5x
(pedido 289): **duas colunas, 16 bytes** — `rowstamp` (`UInt8`, contador puro do
nó, começa em 1, nunca toca o relógio) e `rowtime` (`DateTime`, relógio de
parede, **pode empatar sem mentir**). O desenho byte a byte está em
`parecer-dba-psch-v10-2026-09-17.md` §4.3. **Falta implementar.**

O número que fundou a decisão: **12 eventos gravados num único milissegundo**,
medido em 17/09 sobre `log.rs:19-20` — resolução de relógio não desempata nada.

### 4.2 Com 20 relógios independentes, a garantia é alcançável? **Parcialmente — e a parte que falta é intransponível enquanto o caixa estiver offline.**

O parecer do v10 já escreveu o limite (§5):

> *«A garantia “pai estritamente antes do filho” vale DENTRO de um nó. Entre
> dois masters `multi`, os carimbos vêm de dois contadores independentes e podem
> se intercalar. Não se reivindica ordem global entre nós sem prova.»*

Aplicado a 20 caixas, isso se parte em dois casos, e a diferença entre eles é a
decisão de modelagem que este parecer impõe:

**(a) Pai e filha nascem no MESMO caixa.** Venda e itens da venda. Aqui a
pétrea **se cumpre**, e de graça: o contador é do processo
(`parecer-dba-psch-v10` §4.1), a transação empilha o conjunto de escrita na
ordem pedida (`transacao.rs:218-221`), o pai **tem** de vir antes porque a FK do
filho exige que ele exista, e o carimbo **viaja na imagem** e a réplica honra o
que veio (decisão do dono, 289). No servidor, `pai.rowstamp < filha.rowstamp`
continua valendo, porque os dois números saíram do mesmo contador.

**(b) Pai e filha nascem em caixas DIFERENTES.** Pagamento no caixa 7 de uma
venda aberta no caixa 3; devolução no balcão de um item vendido num caixa. Aqui
a pétrea **não se cumpre, e não há formato que a cumpra com o caixa offline**:

- os dois `rowstamp` saem de contadores independentes e **empatam ou invertem**;
- o remédio conhecido — carimbo do nó *i* na faixa `carimbo ≡ i (mod N)`
  (`parecer-dba-psch-v10` §5) — **mata o empate, não cria a ordem**: dois
  números distintos podem estar na ordem errada;
- o remédio que **cria** a ordem é o empurrão de Lamport (ao aplicar um evento,
  o contador local sobe para pelo menos o carimbo recebido; o valor **gravado**
  continua sendo o que veio, então o retrato SHA-256 não diverge — é o mesmo
  remédio do `anotar_sequencia`, `reg.rs:812`, e **custa zero byte na
  linha**). Só que ele **exige que o evento do pai tenha chegado antes de o
  filho nascer** — e o cenário do dono diz explicitamente que não chegou: o
  servidor estava caído.

**Parecer de C:** a ordem causal entre nós **não se fabrica offline**. Nenhum
carimbo, nenhuma resolução, nenhuma faixa. O que existe são duas saídas, e as
duas são de modelagem, não de código:

- **A que eu recomendo — restrição de modelo:** *toda relação pai/filha do
  caminho de venda nasce no mesmo caixa.* A venda e os itens, sempre. O que
  atravessa caixas (pagamento consolidado, devolução) **não é filha por FK
  conferida**: é lançamento próprio com referência **não conferida**
  (`"verificar": false`, que é escolha escrita, `valores.rs` / pétrea da chave
  conferida). A pétrea continua inteira porque não há filha declarada sem pai.
- **A que substituiria, se o dono quiser a ordem entre nós:** o empurrão de
  Lamport **mais** a exigência de que o filho só possa nascer depois de o pai
  ter sido **visto aqui** — que é o que a FK conferida já faz, e que **derruba
  a exigência (2) do enunciado** («os operadores não percebem»). Não dá para
  ter as duas. É uma escolha do dono, não minha.

---

## 5. A janela: o servidor de pé e um caixa ainda não sincronizado

### 5.1 O que o segundo caixa vê

**Nada.** A linha não existe no servidor, e não existe no caixa 7 que pergunta.
A replicação é **assíncrona por pull** (`REPLICACAO.md` §15, primeiro item: *«Não
é replicação síncrona»*), com lote de **500 eventos** ou **16 MiB**
(`servidor.rs:649,661`) por rodada, e a rodada é o `reconectar_em` da origem.
Não há quórum de escrita (`docs/propostas/quorum-de-escrita.md` é proposta) e
não há leitura que espere.

### 5.2 As garantias que se perdem — nomeadas

1. **Unicidade global.** Dois caixas podem aceitar, cada um no seu disco, o
   mesmo CPF novo, o mesmo código de cliente, a mesma NF-e. O índice único é
   local. A conta chega no servidor como **conflito de unicidade**, que **para a
   replicação daquela tabela** e espera um humano (pedido 292). *Em 20 caixas, a
   unicidade não é uma garantia do banco: é uma esperança sobre o comportamento
   dos operadores.*
2. **Read-your-own-writes entre nós.** Existe dentro da transação local
   (pedido 162) e **não existe** entre caixas. O isolamento entregue sem pedir
   é `READ COMMITTED`, e a leitura repetível (16/09, pela trava, pedida) é
   **local ao servidor que a concede** — trava de um caixa não alcança outro.
   Ver `docs/ACID.md` §2.4/§3.3/§4.4.
3. **Leituras monotônicas.** Um caixa que pergunta ao servidor e depois ao seu
   próprio disco vê o dado aparecer e sumir, conforme quem respondeu.
4. **E a que mais dói na loja, porque ela é da regra do conflito e não da
   janela: contador de estoque.** «Mais recente vence» é **sobrescrita de
   linha**, não decremento. Vinte caixas baixando o mesmo SKU de 100 para 99,
   cada um no seu disco, convergem para **99** — e não para 80. Não é defeito de
   implementação: é o que o casamento por chave com carimbo faz, por desenho
   (`bidirecional.rs:16-30`). *Quantidade em coluna de linha não é replicável
   por «mais recente vence»* — o estoque tem de ser **movimento** (uma linha por
   baixa, que só insere e nunca sobrescreve), e aí o formato ajuda: `.reg`
   append-only é exatamente isso. **Esta é uma afirmação de leitura; a bancada
   que a prova está na §7.**
5. **A atomicidade do commit do caixa** — §3.3.

### 5.3 O que a casa promete hoje sobre isso

Promete o que está escrito e nada mais: assíncrona, atraso normal igual ao
`reconectar_em`, «réplica não é backup», e — no `multi` — «mais recente vence,
com relógios em NTP». O que **não** está escrito em lugar nenhum, e tem de estar
antes de uma loja subir, é que **nessa janela o banco não tem unicidade, não
tem integridade referencial no ponto de convergência, e não tem ordem entre
nós**. Sem essa frase no contrato, a promessa «transparente» do enunciado é
maior que o produto.

---

## 6. Mudança de formato: o que muda, e o que é mais barato AGORA

### 6.1 A regra, aplicada

*Mudança de formato entra cedo.* Aqui isso tem um sentido operacional preciso:
**antes do primeiro cupom fiscal**. Depois, `acrescentar_coluna` reescreve o
`.reg` inteiro slot a slot (`FORMATO.md` §1.1), e numa tabela de vendas de um
ano isso é **janela de parada da loja**.

### 6.2 O que este cenário obriga, item a item

| # | o que muda | arquivo | custo agora | custo depois |
|---|---|---|---|---|
| 1 | **`passo` da `Sequence`** (pedido 290) | `PSCH` v10 | 1 `u64` por coluna `Sequence`; ausente = 1 | reescrever o `PSCH` de toda tabela **e adivinhar em que faixa cada número já gravado nasceu** — não é migração, é adivinhação (pedido 290) |
| 2 | **`inicio` por nó** | identidade do nó + marca d'água nos bytes 36..44 do `.reg` | **ZERO byte** (o invariante é `proxima_sequencia ≡ inicio (mod passo)`) | idem |
| 3 | **`rowstamp` + `rowtime`** (pedido 289) | `PSCH` v10, 16 B/linha | slot 118 → 134 (+13,6%); 1.125,3 → 1.277,9 MiB em 10M linhas | `acrescentar_coluna` reescreve tudo; **source e réplica sobem JUNTOS** |
| 4 | **faixa `mod N` do `rowstamp`** | mesma coluna, zero byte | de graça se entrar junto | adivinhar em que faixa cada linha nasceu |
| 5 | **coluna única de uma coluna em `itens_da_venda`** | esquema da aplicação | uma coluna a mais | reescrita da tabela filha inteira |

**O item mais barato agora e impossível depois é o par (1)+(4): o N.** E aqui
está o achado desta frente que nenhum dos pedidos tem:

> **O `passo` da `Sequence` (pedido 290) e o módulo do `rowstamp`
> (`parecer-dba-psch-v10` §5) são o MESMO número N, e hoje estão em dois pedidos
> diferentes, cada um com a sua justificativa.** Declarar dois N diferentes é
> possível e é armadilha: o nó cujo `inicio` é 3 na `Sequence` e 7 no carimbo
> passa a ter duas identidades numéricas, e a primeira conferência que cruzar as
> duas acusa um nó que não existe. **Um N, declarado uma vez, com folga acima do
> número de caixas, no mesmo bump.**

### 6.3 A interseção com o `PSCH` v10 e com o pedido 314 — e ela é grave para uma loja

O pedido 314 diz: **tabela em modo ledger não migra para o v10**
(`table.rs:800-807` recusa coluna nova em tabela ledger, e o motivo está certo —
o hash de cada bloco cobre o conteúdo na ordem do esquema).

Num supermercado, a tabela ledger é **exatamente a fiscal** — o cupom, o
espelho, o que não pode ser adulterado. Consequência, e ela precisa ir para a
mesa do dono junto com o resto:

> **A tabela que mais precisa provar «o pai veio antes do filho» é a única que
> não pode receber a coluna que prova.** Uma tabela ledger que já tem cadeia
> fica em v9 para sempre. A saída limpa existe e é de modelagem: **a tabela
> fiscal nasce em v10**, nunca migra — mas isso só vale se o v10 entrar **antes
> da primeira loja**, e é mais um item na conta do «agora».

### 6.4 O que NÃO precisa mudar

- **`.ndx`**: nada. A codificação já ordena `UInt8` e `Uuid` por `memcmp`
  (`FORMATO.md` §«Codificação de chave que preserva ordem»).
- **`.log`**: nada estrutural. Os 2 bytes de origem já existem — **mas ver a
  restrição 8**.
- **`.reg`**: além das duas colunas, nada. O contador de `Sequence` já mora nos
  bytes 36..44 e o de `rownum` nos 92..100.

---

## 7. O que eu NÃO consegui provar por leitura

| # | afirmação | por que a leitura não basta | a bancada que prova |
|---|---|---|---|
| 1 | 21 nós em estrela convergem | só o **par** foi provado (`REPLICACAO.md`:778) | `bancada/replicacao` em contêiner, 21 nós, com corte de rede por nó (o molde do §17 já existe) |
| 2 | a colisão de faixa vira perda em massa com 20 caixas | o número medido é do **par** (4→2, 07/09) | repetir o bloco 24 da sonda com 20 origens e contar linhas sobreviventes |
| 3 | `Sequence` com faixa dá 20 pontos quentes e cabe nos 8 MiB de cache | é aritmética de página, não medição | `--example onde-doi` com 20 séries intercaladas; contar toques de página (o medidor já conta por dentro) |
| 4 | o `Uuid` v7 de um caixa atrasado divide páginas no meio da árvore | idem | inserir 100k chaves com carimbo de 4 h atrás numa árvore de 10M e medir ocupação de folha e toques |
| 5 | estoque por «mais recente vence» perde decrementos | leitura do desenho do conflito | 20 escritores baixando o mesmo SKU; conferir o saldo final contra a soma dos movimentos |
| 6 | «a replicação sobe **imediata**» | `absorver_diario_local` (`servidor.rs:4685-4731`) recomeça do evento 0 **a cada arranque do processo** e decodifica a imagem de todo evento já gravado | medir o tempo da **primeira** rodada após reinício, com diário de 1 dia, 1 mês e 1 ano |
| 7 | o mapa de toques cabe na RAM | `MapaDeToques.toques` é `HashMap<String,Toque>` **sem teto**, uma entrada por chave distinta do diário inteiro | medir RSS do servidor após absorver 1M, 10M chaves |
| 8 | órfã permanente passa sem ninguém ver | leitura dos portões calados | matar o caixa entre o evento da filha e o da mãe; rodar `conferir-integridade` no servidor |
| 9 | o commit do caixa chega partido | `.log` sem id de transação (`log.rs:134-158`) | commitar venda+itens, cortar a rede no meio da rodada, ler o servidor |

---

## 8. As recusas de C (o **não**, com a pétrea nomeada)

**NÃO 1 — Convergir 20 caixas pelo caminho unidirecional (`aplicar_evento`).**
Pétrea: **a ordem de digitação é sagrada**. Os rowids colidem por construção, o
fail-stop dispara no segundo evento da segunda origem, e a única forma de
contorná-lo (pular evento) desloca todos os rowids seguintes — troca um
problema que **para** por um que **diverge em silêncio**. Já decidido no
pedido 292 para o irmão deste caso.

**NÃO 2 — Subir 20 caixas com a `Sequence` na faixa de fábrica (`inicio=1`,
`passo=1`).** Não é pétrea, é perda de dado medida: 4 inserções → 2 linhas no
par (07/09/2026). Com 20 nós, toda venda do dia colide. **Este é um não de
bloqueio: o cenário não sobe sem o pedido 290 implementado**, ou sem trocar a
identidade por `Uuid` v7.

**NÃO 3 — Reconferir integridade referencial no servidor para «salvar» a
pétrea.** Já foi medido e custa dado: 0 de 2 eventos nas duas ordens simples, e
um evento a mais na entrelaçada (`INTEGRIDADE.md` §3). Conferir duas vezes não
soma as duas garantias: troca a segunda pela primeira.

**NÃO 4 — Calar a unicidade no ponto de convergência para não parar o par.**
Recusado por C em 17/09 e mantido: o índice no disco passaria a mentir sobre si
mesmo, e violação de índice único **não se cura** quando o próximo lote chega,
ao contrário da FK.

**NÃO 5 — Prometer «pai estritamente antes do filho» quando pai e filha nascem
em caixas diferentes com o servidor fora do ar.** Não há formato que cumpra. O
que se promete é a garantia **por nó**, e a modelagem passa a exigir que a
relação conferida nasça no mesmo nó.

**NÃO 6 — Bump de `PSCH` parcelado.** Dois bumps para duas colunas de sistema
são duas migrações e dois eventos de parada. O `passo`, o `inicio`, o
`rowstamp`, o `rowtime` e a faixa `mod N` entram no **mesmo v10**, com **um N
só**.

**NÃO 7 — Chamar isto de «transparente» sem o contrato da §5.3.** Não é recusa
técnica, é recusa de promessa: o produto faz menos do que a frase diz, e
*funcionalidade que promete mais do que faz tem de dizer que faz menos*.

---

## VEREDITO

**Hoje, NÃO** — o caminho por rowid está morto neste cenário pela ordem de
digitação, o caminho por chave existe e sobrevive à pétrea mas nunca foi provado
além do par, a `Sequence` de fábrica transforma toda venda do dia numa colisão
medida, e a integridade referencial **não é imposta no ponto de convergência**;
com o `PSCH` v10 inteiro (um N só), identidade em faixa por caixa, pai e filha
nascendo no mesmo caixa e o verificador de consistência virando rotina diária,
**passa a ser possível** — e continua sendo, por formato, um arranjo sem
unicidade global, sem ordem entre nós e sem atomicidade de commit entre tabelas.

## RESTRIÇÕES (numeradas, para o dono decidir uma a uma)

1. **Papel `multi` nos 21 nós.** O unidirecional não serve (NÃO 1). Consequência:
   toda tabela replicada precisa de **chave única de UMA coluna**; composta é
   recusada (`bidirecional.rs:160`) — `itens_da_venda` tem de ganhar coluna
   própria.
2. **Um N só, escolhido antes do primeiro cupom, com folga.** Vale para o
   `passo` da `Sequence` (290) e para a faixa do `rowstamp` (v10 §5). Mudá-lo
   depois reabre faixas usadas: é a ordem de digitação pela porta dos fundos.
3. **Pai e filha com chave conferida nascem no MESMO caixa.** O que atravessa
   caixas é referência **não conferida**, escrita como escolha (`"verificar":
   false`), nunca por omissão.
4. **Quantidade não se replica por «mais recente vence».** Estoque é
   **movimento** (linha por baixa), nunca coluna sobrescrita. §5.2(4), a provar
   pela bancada 5 da §7.
5. **A tabela fiscal nasce em v10 e nunca migra** (pedido 314): ledger com
   cadeia não aceita coluna nova, e o motivo está certo.
6. **`conferir-integridade` vira rotina diária no servidor**, com dono e
   horário. É a única coisa que vê a órfã permanente, e ele **relata, não
   conserta** — por decisão registrada.
7. **O contrato diz, por escrito, o que se perde na janela**: unicidade global,
   integridade no ponto de convergência, ordem entre nós, atomicidade do commit
   entre tabelas. §5.3.
8. **Os 21 `id_servidor` são conferidos entre SI, não só contra o servidor.**
   `hash_id` é de 16 bits (`bidirecional.rs:70-79`) e a conferência de colisão
   existe **por par** (réplica × source). Calculado nesta frente: com 21 ids a
   probabilidade de alguma colisão é **0,31996%**, contra **0,00153%** no par que
   foi provado — **209,7× mais**. Dois caixas colidindo entre si **não são vistos
   por ninguém**, e o efeito é o servidor suprimir, calado, os eventos de um ao
   repassá-los para o outro (o filtro `para` casa por hash). A conferência de
   colisão precisa ser do **conjunto**, na subida.
9. **«Imediata» tem de ser medida.** A primeira rodada após um arranque
   reabsorve o diário local inteiro, evento a evento, decodificando cada
   imagem (`servidor.rs:4685-4731`). §7, itens 6 e 7.
