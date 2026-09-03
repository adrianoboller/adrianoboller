# Parecer do DBA — pedido 175: o índice da chave estrangeira na declaração

> **A decisão é do dono.** Este documento não decide: ele põe o custo e o risco
> na mesa para que a decisão caiba em cinco minutos. Nada aqui foi
> implementado — nenhuma linha de código do motor mudou nesta rodada.

**A pergunta.** Quando alguém declara uma chave estrangeira conferida numa
tabela que não tem o índice que a chave exige, o motor deve (a) continuar como
está — aceitar a declaração e recusar depois, na gravação —, (b) **criar** o
índice sozinho, como o MySQL(R) e o MariaDB(R) fazem, ou (c) **recusar** já na
declaração?

**A recomendação, em uma frase:** **(b), criar o índice sozinho na filha** — mas
**depois** de fechar um defeito que este parecer encontrou e que nenhuma das
três saídas conserta: o portão `fks_conferidas` não é refeito pelo
`redeclarar_chaves_estrangeiras`, e por isso a chave que acaba de nascer
conferida **não confere nada** no handle que a declarou, e tirar uma chave faz
o `inserir` seguinte **entrar em pânico**.

---

## 1. O que muda para quem modela

Três cenários. A coluna «hoje» é medida, não suposta: saiu de rodar cada
caminho contra o motor (§6, «prova real»).

### 1.1 Declara **com** o índice

| | o que acontece |
|---|---|
| **hoje** | declara, grava, exclui. Tudo funciona, e o `excluir` da mãe recusa pela regra primordial, com o texto certo: *«esta linha tem filhas em pedidos pela chave "fk_cliente". Nunca se apaga o registro pai que tem filhos»* |
| **saída (b)** | idêntico — não há nada a criar |
| **saída (c)** | idêntico |

Este é o caso em que nada muda em nenhuma das três saídas. É também o caso que
o `docs/INTEGRIDADE.md` §6 assume ao dizer que a recusa na declaração
«quebraria a ordem legítima *declare a chave, crie o índice*».

### 1.2 Declara **sem** o índice

Aqui está o pedido, e o estado de hoje é **pior do que o pedido descreve**.

| | o que acontece |
|---|---|
| **hoje** | a declaração **aceita**. O `inserir` na filha **funciona** — ele só precisa do índice da **mãe**, que existe. E aí a mãe perde o `excluir` **inteiro** |
| **saída (b)** | o índice nasce junto com a chave; o comportamento passa a ser o do §1.1 |
| **saída (c)** | a declaração **recusa**, nomeando o índice que falta |

O «perde o `excluir` inteiro» é o achado desta análise, e é medido. O pedido
175 e a §6 dizem que dá para «declarar chave conferida sem os índices e só
descobrir no primeiro `excluir`». O que a prova real mostrou é que não é *o
primeiro excluir de uma linha com filha* — é **todo excluir daquela tabela**:

```
--- SEM o índice (o que o MANUAL ensina) ---
  inserir pedido do cliente 3     : ACEITOU
  excluir o cliente 3 (TEM filha) : recusou — «não dá para conferir as filhas de
                                    pedidos pela chave "fk_cliente", que não tem
                                    índice começando por (cliente_id)»
  excluir o cliente 7 (SEM filha) : RECUSOU — a MESMA mensagem
```

A linha que **ninguém referencia** também não sai. O motor não pergunta «esta
linha tem filha?»; ele pergunta antes «eu consigo perguntar?», e a resposta é
não. A tabela `clientes` fica sem exclusão nenhuma até alguém criar um índice
em **outra** tabela.

E o agravante que fecha o argumento: **o exemplo canônico do nosso próprio
`MANUAL.txt` cai exatamente nisso.** Em `MANUAL.txt`, seção *CHAVE ESTRANGEIRA
NO criar_tabela*:

```json
"indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}],
"chaves_estrangeiras":[
  {"nome":"fk_cliente","colunas":["cliente_id"],
   "tabela_ref":"clientes","colunas_ref":["id"], ...}]
```

Um índice, pela primária. Nenhum por `cliente_id`. Quem copiar o exemplo do
manual cria uma `clientes` que não exclui mais nada — e como a chave **nasce
conferida** desde a decisão do dono, isso vale para todo mundo que copiar o
exemplo a partir de agora, sem ter pedido a garantia e sem saber que a pediu.
Não é um risco hipotético de modelagem: é a documentação ensinando o defeito.

### 1.3 Declara para tabela que **ainda não existe**

| | o que acontece |
|---|---|
| **hoje** | a declaração **aceita** (é ordem legítima de modelagem, e está escrito assim no `table.rs:552`). O `inserir` recusa depois, nomeando a tabela que falta |
| **saída (b)** | **funciona igual, e sem atrito**: o índice a criar é o da **filha**, que é a tabela que está aqui. A mãe ausente não atrapalha nada |
| **saída (c)** | não muda nada aqui, porque a recusa da (c) seria por falta de índice, não por falta de mãe |

Este cenário é um **ponto a favor da (b)**, e vale registrar por quê: a
objeção natural à criação automática seria «e se a outra tabela não existir?».
Ela não se aplica. A assimetria dos dois motores de referência — criam do lado
da filha, recusam no DDL do lado da mãe — existe justamente porque **criar
índice em tabela alheia não cabe a quem declara a chave**, e do lado da filha
não há tabela alheia nenhuma.

---

## 2. O que isso ACRESCENTA ao esquema em disco

Esta é a seção que só o DBA levanta, e ela tem duas metades que costumam ser
confundidas numa só: o **bloco de esquema** (`PSCH`, dentro do `.reg`) e o
**arquivo de índices** (`.ndx`). Criar um índice mexe nos dois.

### 2.1 O bloco `PSCH`: +26 bytes, no MEIO do bloco

O `Schema::serializar` (`crates/phxsql-core/src/schema.rs:1068`) grava a lista
de índices **entre** as colunas e as chaves estrangeiras, prefixada por um
`u16` de contagem. Por índice:

```
2 + n (nome) | 1 (byte de sinalizadores: único no bit 0, primário no bit 1)
             | 2 (quantas colunas) | por coluna: 2 (posição) + 1 (desc/nocase)
```

Medido para um índice de uma coluna chamado `fk_pedidos_cliente`: o bloco vai
de **434 para 460 bytes — +26**.

Porque a lista fica no meio, tudo o que vem depois dela — as chaves
estrangeiras, a paginação, o byte de motivo obrigatório e as marcas de dado
pessoal — **desloca**. Isso é irrelevante para a leitura, e o motivo é do
formato: o leitor é sequencial e cada bloco é prefixado por contagem.

### 2.2 Isso quebra banco que já existe? **NÃO** — e o motivo é de formato

Respondido lendo o formato, não supondo:

* **A versão do `PSCH` não muda.** Continua a **v7**. A lista de índices já é
  prefixada por `u16` desde a **v2**, então um índice a mais não é campo novo:
  é uma iteração a mais no laço que já existe. Nada de repetir o exercício da
  v6 ou da v7, em que um bloco novo teve de ir para o **fim** para que a versão
  anterior parasse antes dele.
* **Nada que já está gravado é reinterpretado.** O `payload_len` não muda, o
  `slot_size` não muda, o offset de nenhuma coluna muda, o **rowid** não muda.
  Índice não é coluna: ele não entra no slot.
* **A ordem de digitação não é tocada.** O `.reg` continua sem reaproveitar
  slot excluído.
* **Tabela gravada antes continua com os índices que tem.** É o mesmo alcance
  que a decisão «chave declarada nasce conferida» já tem, e pelo mesmo motivo:
  o disco volta com o que foi gravado nele. Muda o que **nasce** daqui em
  diante.

### 2.3 Mas o bloco CRESCE — e crescer tem dois caminhos, um deles caro

`RegFile::regravar_esquema` (`crates/phxsql-store/src/reg.rs:991`) escolhe
entre dois:

* **barato** — o bloco novo cabe antes do `data_offset` (a folga do alinhamento
  de 64 deixa de 0 a 63 bytes sobrando): regrava só o cabeçalho de cada volume;
* **caro** — não cabe: **cada volume é reescrito byte a byte** num arquivo ao
  lado e trocado por `rename`.

Se os +26 bytes caem no barato ou no caro **é um cara ou coroa no resto da
divisão por 64**. No caso medido sobravam **14 bytes** de folga, os 26 não
couberam, e o `data_offset` foi de **576 para 640** — o caminho caro.

Medido, com a mesma tabela e só o comprimento do nome mudando para atravessar
o alinhamento:

| linhas | barato (cabe) | CARO (reescreve) | razão |
|---:|---:|---:|---:|
| 1.000 | 345,8 µs | 1.024,4 µs | 3,0× |
| 10.000 | 358,1 µs | 2.224,5 µs | 6,2× |
| 100.000 | 490,4 µs | 14.534,1 µs | **29,6×** |

O caro **cresce com a tabela**; o barato é constante. Numa tabela **paginada**
todos os volumes são reescritos, e essa multiplicação eu não medi (§5).

### 2.4 O `.ndx`: hoje não existe «acrescentar um índice»

Este é o item de maior peso técnico do parecer, e ele não aparece em nenhum
dos dois manuais porque é nosso.

`NdxFile::criar` escreve o **diretório de índices** na página 0 a partir do
esquema, e **trunca** o arquivo. A única máquina que existe para fazer um
índice nascer é `Table::reindexar` (`table.rs:3991`), que chama `NdxFile::criar`
e reconstrói **todos** os índices da tabela — não há
`NdxFile::acrescentar_indice`. Confirmado varrendo a API pública do `ndx.rs`.

Consequência direta, e é ela que decide o preço da saída (b):

> **Criar um índice sozinho custa hoje o preço de reconstruir o `.ndx` inteiro,
> não o preço do índice novo.**

O teto do diretório, que ninguém vai encostar mas o formato impõe: a página 0
tem 4096 − 128 = **3.968 bytes**, e a entrada de um índice chamado
`fk_pedidos_cliente` ocupa `2+n+1+4+8+8` = **41 bytes** → cabem **96 índices**
por tabela. Um índice automático por chave estrangeira não chega perto.

### 2.5 E o que ele custa em disco, para sempre

O índice que o modelador não escreveu ocupa espaço que o modelador não pediu.
Medido, o `.ndx` da mesma tabela com um e com dois índices:

| linhas | `.ndx` 1 índice | `.ndx` 2 índices | a mais | por linha |
|---:|---:|---:|---:|---:|
| 0 | 8 KiB | 12 KiB | 4 KiB | — |
| 1.000 | 32 KiB | 60 KiB | 28 KiB | 28,7 B |
| 10.000 | 264 KiB | 472 KiB | 208 KiB | 21,3 B |
| 100.000 | 2.596 KiB | 4.280 KiB | **1.684 KiB** | **17,2 B** |

**+65% no `.ndx`** a 100.000 linhas. É o custo permanente, e ele é o mesmo
quer o índice tenha sido criado à mão ou pelo motor — a diferença é **quem
sabe que ele existe**.

---

## 3. O custo, medido

Máquina compartilhada com outras frentes compilando (4 CPUs). Estatística:
**mínimo de 7 voltas**, e não mediana — interferência de vizinho só *soma*
tempo, então o mínimo é o que mais se aproxima do custo sem vizinho. Carga
durante as corridas usadas aqui: 2,1 a 2,7.

### 3.1 Declarar a chave numa tabela **VAZIA** — que é quando se modela

| | tempo |
|---|---:|
| declarar hoje (tabela vazia) | **374,7 µs** |
| o `.ndx` que a saída (b) acrescentaria | **+70,4 µs** |
| **total da saída (b)** | **445,1 µs** |

**A resposta é ~0.** Setenta microssegundos, e desses o trabalho real é criar
uma árvore vazia. Na tabela vazia a diferença entre as três saídas é
**invisível para quem modela**, e o pedido tinha razão em prever isso.

### 3.2 Declarar a chave numa tabela **COM DADO**

| linhas | declarar hoje | + o `.ndx` da saída (b) | total (b) |
|---:|---:|---:|---:|
| 0 | 0,4 ms | 0,1 ms | 0,4 ms |
| 1.000 | 4,2 ms | 1,8 ms | 6,0 ms |
| 10.000 | 37,5 ms | 19,5 ms | 57,1 ms |
| 100.000 | **367,0 ms** | **180,5 ms** | **547,4 ms** |

**A leitura que muda o argumento:** declarar hoje numa tabela de 100.000 linhas
**já custa 367 ms**. Ele não é grátis e nunca foi — desde que a chave nasce
conferida, a declaração varre linha a linha para não prometer o que não pode
cumprir (`table.rs:562`, §2.4 do `INTEGRIDADE.md`). Decomposto:

| linhas | varredura de conferência | regravar o `PSCH` | total |
|---:|---:|---:|---:|
| 0 | 61,7 µs | 320,6 µs | 374,7 µs |
| 1.000 | 3.597 µs | 384,9 µs | 4.163 µs |
| 10.000 | 36.255 µs | 404,4 µs | 37.523 µs |
| 100.000 | **365.107 µs** | 460,2 µs | 366.955 µs |

A varredura **é** o custo; a regravação do esquema é ruído ao lado dela. Então
a saída (b) acrescenta **49%** a uma conta que já se paga, e não uma conta
nova.

### 3.3 A premissa do pedido estava 9× pessimista

O pedido 175 cita **2,2 µs por linha**, de
`--example custo-do-reindexar-no-arranque`. Reconferido hoje, o medidor oficial
continua dizendo isso — **2,12 µs/linha, 211,8 ms a 100.000**. Mas esse é o
custo de reconstruir o **`.ndx` inteiro**, com os dois índices e 200.000
chaves. O custo do **índice que se está criando** é outro:

| linhas | reindexar 1 índice | reindexar 2 índices | **marginal do 2º** | por linha |
|---:|---:|---:|---:|---:|
| 1.000 | 1.629,0 µs | 1.813,3 µs | 184,3 µs | 0,18 µs |
| 100.000 | 159.907 µs | 180.453 µs | **20.546 µs** | **0,21 µs** |

*(a linha de 10.000 saiu dentro do ruído em duas corridas — 0,285 µs e
0,055 µs — e por isso não entra na conta.)*

**0,21 µs por linha, não 2,2.** Nove vezes menos. E a razão de os 2,2 valerem
mesmo assim, hoje, é a §2.4: **sem `NdxFile::acrescentar_indice`, paga-se a
reconstrução inteira**. Os dois números são verdadeiros, e a diferença entre
eles é exatamente o valor de escrever essa função:

* com a máquina de hoje: **180 ms** a 100.000 linhas;
* com um construtor de um índice só: **21 ms** a 100.000 linhas.

*Medir a premissa do item vem antes de implementar o item — inclusive quando o
item é nosso.* Aqui a premissa não morreu: ela ficou nove vezes menor, e o
alvo mudou de «vale a pena criar o índice?» para «vale a pena escrever o
construtor de um índice só?».

### 3.4 O custo que NÃO acaba

Um índice a mais é uma descida a mais na árvore em **cada inserção**, para
sempre. Medido inserindo 50.000 linhas uma a uma, mínimo de 5 voltas, **três
corridas em momentos diferentes**:

| corrida | 1 índice | 2 índices | a mais | |
|---|---:|---:|---:|---:|
| 1 (carga 4,1) | 9,14 µs | 9,59 µs | 0,45 µs | +4,9% |
| 2 (carga 3,2) | 8,32 µs | 9,26 µs | 0,94 µs | +11,3% |
| 3 (carga 2,2) | 8,61 µs | 9,70 µs | 1,09 µs | +12,7% |

As duas corridas mais calmas concordam em **~1,0 µs/linha (+12%)**; a mais
carregada destoa por um fator de dois. **Este é o número que menos consegui
fechar**, e está reportado com a dispersão em vez de escolhido pela corrida
mais conveniente (§5). A ordem de grandeza bate com o que a casa já mediu para
a chave conferida no laço quente — +7,03 µs/linha, +11,2%, `DESEMPENHO.md` §15
— e é sobre uma linha de **duas colunas**, que é o caso mais favorável ao
índice a mais: numa linha larga a proporção cai.

Esse é o único custo da saída (b) que **não** é o custo de um índice que a
pessoa teria de criar de qualquer jeito. E ele só existe se o motor criar um
índice que ninguém queria — o que só acontece para quem declarou a chave e
**não queria** a conferência, e esse caminho já tem porta escrita:
`"verificar": false`.

---

## 4. As três saídas, com o preço de cada uma

### (a) Continuar como está

**Custo de escrever:** zero.

**Custo de usar, medido:** a mãe perde o `excluir` **inteiro** (§1.2), e o
exemplo do nosso próprio `MANUAL.txt` cai nisso. O erro aparece longe da causa
— o modelo se escreve num dia, o `excluir` falha num outro — e a mensagem, que
é boa, é lida por quem não estava lá quando a chave foi declarada.

**O que ela custa de verdade:** a diferença entre uma casa cuja doutrina é
«recusar cedo custa um erro lido enquanto se modela; recusar tarde custa um
banco inteiro modelado errado» e uma casa que faz o contrário na única porta
onde escolheu fazer o contrário. Esse argumento já está escrito, com essas
palavras, no `table.rs:538` — sobre o **outro** motivo de recusa da mesma
função.

### (b) Criar o índice sozinho, na filha

**Custo de escrever:** um caminho de esquema que acrescente um `IndexDef` (o
`Schema` tem `com_chaves_estrangeiras` e `com_coluna`; não tem `com_indice`),
mais a reconstrução do `.ndx`. Se quiser o preço bom, mais um
`NdxFile::acrescentar_indice` — §3.3 diz quanto ele compra: 180 ms → 21 ms a
100.000 linhas.

**Custo de rodar, medido:** ~0 na tabela vazia (**70 µs**); +49% sobre uma
declaração que a 100.000 linhas já custa 367 ms; +17,2 B por linha no `.ndx`
para sempre; **~1,0 µs por inserção**, para sempre.

**Risco:** o caminho caro do §2.3 dispara sem ninguém pedir — a criação de um
índice pode reescrever cada volume da tabela byte a byte, e a pessoa só pediu
uma chave.

### (c) Recusar na declaração

**Custo de escrever:** o menor dos três — a conferência já existe
(`indice_que_cobre`) e já roda, dentro do `conferir_chave`, que já devolve
`Falha::SemIndiceNaFilha`. Hoje ela é **descartada** porque é falha «de
estrutura». Bastaria deixar de descartá-la.

**Custo de usar:** quebra a ordem *declare a chave, crie o índice* — que é a
frase da §6, e ela **está certa**. Quem modela pela API teria de inverter a
ordem; quem modela pelo diagrama ER (`op_declarar_fk`, que é «o que o editor
do diagrama chama quando alguém puxa uma coluna até a coluna de outra tabela»)
receberia uma recusa ao arrastar a linha, e teria de sair da tela para criar
um índice — que, aliás, **não tem operação de protocolo**: não existe
`criar_indice` no despachar. Hoje só se cria índice criando a tabela.

**E é a saída que nenhum dos dois motores escolheu do lado da filha.** Eles
recusam do lado da **mãe**, no DDL, e criam do lado da filha. Escolher (c) é
escolher o contrário dos dois nas duas pontas.

### A recomendação

**(b)**, e o motivo é o que a (c) revelou sem querer: **não existe operação de
criar índice neste motor.** Recusar na declaração manda a pessoa fazer uma
coisa que o protocolo não oferece — e uma recusa que aponta para uma porta que
não existe é pior que a recusa tardia de hoje, porque não tem conserto no
caminho que ela indica.

Entre (a) e (b), a conta é curta: no caso que importa — modelar, tabela vazia —
a (b) custa **70 µs** e apaga um defeito que a nossa própria documentação
ensina. No caso raro — declarar sobre tabela cheia — ela acrescenta 49% a uma
conta que já se paga, e que já é dominada pela varredura que a chave conferida
obriga.

**Com três condições, e elas não são detalhe:**

1. **O índice nasce só na FILHA.** Do lado da mãe continua a recusa de hoje. É
   a assimetria dos dois motores, e o motivo é bom: criar índice em tabela
   alheia não cabe a quem declara a chave.
2. **O índice criado se diz.** A resposta da operação tem de dizer que criou e
   com que nome, e o nome tem de sair da chave. Índice que aparece sem ninguém
   ter pedido e sem ninguém ser avisado é o que faz um `.ndx` crescer 65% sem
   explicação — e é o irmão de «configuração que não é lida mente»: **estrutura
   que nasce sem ser dita também mente**.
3. **Quem manda `"verificar": false` não ganha índice nenhum.** Declarar sem
   conferir é escolha escrita; criar índice para ela seria cobrar o custo
   permanente do §3.4 de quem dispensou a garantia.

**E uma precedência, que é a parte deste parecer que eu não esperava
escrever:** nada disso deve entrar antes do §6.

---

## 5. O que eu NÃO consegui medir

Listado porque **papel que não está cumprindo aparece como não cumprindo**.

* **A tabela paginada.** Medi um volume só. O caminho caro do §2.3 reescreve
  **cada** volume; numa tabela com dez volumes o multiplicador é dez, e eu não
  o medi.
* **A tabela cifrada.** O `cab_len` é 192 em vez de 128, então a folga do
  alinhamento cai em outro ponto e o cara ou coroa do §2.3 tem outra moeda.
  Não medi.
* **O custo permanente por inserção (§3.4) tem 2× de dispersão** — 0,45, 0,94 e
  1,09 µs/linha em três corridas. A máquina estava compartilhada com outras
  frentes compilando o tempo todo, e não consegui uma janela ociosa. As duas
  corridas mais calmas concordam, mas «duas de três concordam» não é o mesmo
  que medido.
* **A replicação.** Não medi o que acontece com um índice criado sozinho num
  *source*: se o esquema não viaja por DDL, a réplica fica com um índice a
  menos que a origem, e é a réplica que precisa dele para o `excluir`. Isto
  merece medição própria antes de qualquer implementação — é o tipo de defeito
  que só aparece no encontro de duas frentes.
* **A concorrência.** Um segundo handle aberto enquanto o índice é criado. O
  `conferir_fks` já tem um limite de **visibilidade** conhecido e escrito
  (mãe aberta em outro lugar com escrita pendente); criar índice dentro da
  declaração acrescenta uma janela nova, e eu não a explorei.
* **Quantas tabelas desta base declaram chave sem o índice** — o pedido pede
  este número, dizendo que «se a resposta for zero o pedido vira documentação
  e não código». **Medi, e a resposta é zero, mas a medição não vale o que
  parece:** a única tabela em disco do repositório é `bancada/phxsql/precos`, e
  `--example conferir-integridade bancada/phxsql` responde *«1 tabela(s),
  0 chave(s) declarada(s)»*. Zero chaves declaradas, então zero chaves sem
  índice. Isso mede o **repositório**, não uma base de cliente — e base de
  cliente ainda não existe. O que existe é o exemplo do `MANUAL.txt`, que não é
  uma tabela mas é a receita de todas as que vierem. **Mudança de formato entra
  cedo**: enquanto não há dado em produção, decidir isto é barato.

---

## 6. O que eu encontrei sem procurar: o portão fica velho

Isto não é o pedido 175. Apareceu ao provar o cenário do §1.3, e não cabe
esconder num parecer de integridade referencial.

`Table` guarda um portão chamado `fks_conferidas` — as **posições** das chaves
que pedem conferência, montado na abertura (`table.rs:454`) para que uma tabela
sem chave não pague nada. Duas operações o refazem depois de mexer no esquema:
`acrescentar_coluna` (`:685`) e `remarcar_dado_pessoal` (`:4083`).

**`redeclarar_chaves_estrangeiras` (`:562`) não refaz.** Ela atualiza
`self.esquema` e grava o `.pag`, e o portão continua com as posições de quando
a tabela foi aberta. É a única das três cujo trabalho **é** mexer nas chaves
estrangeiras.

### Prova real, nos dois sentidos

**(A) A chave nasce conferida e não confere.** Handle aberto sem chave, chave
conferida declarada nele, e uma órfã gravada em seguida:

```
o esquema diz verificar = true
inserir com cliente 999 (INEXISTENTE): >>> ACEITOU <<<
depois de REABRIR, o mesmo insert : recusou — «fk_cliente: não existe
                                    clientes(id) com esse valor»
```

O disco diz `verificar: true`. O motor grava a órfã assim mesmo. É **a mesma
promessa falsa** que a §2.4 do `INTEGRIDADE.md` recusa na outra direção — ali a
casa se recusa a declarar conferida sobre órfã que já existe, e aqui deixa
nascer uma órfã nova sob uma chave que acabou de se declarar conferida.

**(B) Tirar uma chave faz o `inserir` seguinte entrar em pânico.** Handle
aberto com duas chaves, uma removida, e um `inserir` limpo em seguida:

```
thread 'main' panicked at crates/phxsql-store/src/table.rs:808:23:
index out of bounds: the len is 1 but the index is 1
```

`conferir_fks` faz `&self.esquema.chaves_estrangeiras()[i]` com `i` vindo do
portão velho. A lista encolheu; a posição não. Se a lista tivesse sido
**reordenada** em vez de encolhida, não haveria pânico — haveria a conferência
da **chave errada**, calada.

### O alcance, medido e não suposto

**Hoje isto não chega ao protocolo.** `abrir_travada` (`servidor.rs:6278`) abre
uma `Table` nova a cada pedido, e nem `op_declarar_fk` nem `op_excluir_fk`
gravam linha no mesmo handle depois de mexer no esquema. **O defeito é
latente** — mas a bala está na câmara: qualquer reaproveitamento de handle,
qualquer transação que declare uma chave e grave em seguida, e qualquer chamador
da API do `store` cai nele. O `--example sonda-fk-buracos` faz exatamente essa
sequência.

### Por que isto vem ANTES do pedido 175

Porque a saída (b) **acrescenta um segundo estado velho ao mesmo handle**. Se a
declaração passar a mexer no `.ndx`, o `self.ndx` do handle que declarou também
fica para trás — e aí o portão desatualizado deixa de ser uma conferência que
não roda e passa a ser uma árvore que não existe para quem a acabou de criar.

O conserto é **uma linha**, do mesmo formato das duas irmãs, e não é meu de
escrever neste parecer:

```rust
self.fks_conferidas = fks_conferidas_do_esquema(&self.esquema);
```

E a prova real que ela pede é a de cima, nos dois sentidos: (A) tem de recusar
a órfã **sem reabrir**, e (B) tem de deixar de entrar em pânico. Ambas falham
hoje, e é isso que faz delas prova.

### Um terceiro, menor, na mesma vizinhança

`op_declarar_fk` responde `"imposta": false` (`servidor.rs:9394`), com o
comentário *«a chave é DECLARADA. Quem chama precisa poder dizer a verdade na
tela sem conhecer o motor de cor»* — e o doc-comment da função ainda diz
*«declarar não é impor: [...] nenhuma gravação a confere — há teste que trava
esse comportamento»*. Desde «chave declarada nasce conferida», isso está ao
contrário: `verificar` chega `true` por omissão
(`valores.rs:247`), e o campo que existe para a tela dizer a verdade **diz a
mentira**. É documentação e resposta de protocolo envelhecidas junto com a
decisão que as invalidou, e a tela que confiar nelas mostra o oposto do que o
motor faz.

E há um teste travando a mentira, com o comentário *«a resposta diz a verdade
que a tela precisa repetir»* logo acima (`servidor.rs:20239`). O mesmo teste
monta uma `pedidos` com **um índice só, pela primária**, e declara a chave por
cima — é o §1.2 inteiro, dentro da suíte, **verde**, porque ele nunca tenta
excluir um cliente.

---

## 7. Como refazer as medições

Os medidores desta análise não moram na árvore, porque ela é compartilhada e
este parecer não implementa nada. Eles se refazem assim — um `Cargo.toml` com
`phxsql-core` e `phxsql-store` por caminho, e cinco binários:

| binário | o que mede | §|
|---|---|---|
| `medidor-175` | o bloco `PSCH`, declarar hoje decomposto, o `.ndx` marginal | 2.1, 3.1–3.3 |
| `caro` | o caminho barato contra o caro da regravação; a prova real do manual | 2.3, 1.2 |
| `cenario3` | declarar para tabela ausente; o teto do diretório do `.ndx` | 1.3, 2.4 |
| `bytes` | o que o índice a mais ocupa em disco | 2.5 |
| `laco` | o que ele custa em cada inserção, para sempre | 3.4 |
| `portao` | as duas provas do §6, nos dois sentidos | 6 |

Os dois medidores que **já** moram na árvore e foram reconferidos:

```bash
cargo run --release --example custo-do-reindexar-no-arranque -p phxsql-store
cargo run --release --example conferir-integridade -p phxsql-store -- bancada/phxsql
```

Quando o pedido 175 virar código, os cinco de cima devem virar **um**
`--example custo-do-indice-na-declaracao`, versionado — porque script que
resolveu algo não pode morrer com a sessão.

---

## Apêndice — a prova do §6, para colar e rodar

Dos seis medidores, este é o único que não é um laço de cronômetro: é a
**prova real de um defeito vivo**, e quem consertar a linha do §6 precisa
dele para mostrar que ela falha antes e passa depois. Os outros cinco a
tabela do §7 descreve o suficiente para reescrever; este não se reescreve
de memória, porque o valor dele está na sequência exata.

Um `Cargo.toml` com `phxsql-core` e `phxsql-store` por caminho, e:

```rust
//! O portao `fks_conferidas` e montado na ABERTURA e NAO e refeito pelo
//! `redeclarar_chaves_estrangeiras`. Prova nos dois sentidos.

use phxsql_core::schema::{Column, ForeignKey, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::table::Table;
use std::path::Path;

fn mae(d: &Path) {
    let e = Schema::new(
        "clientes",
        vec![Column::new("id", ColumnType::Int4).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap();
    let mut t = Table::criar(d, e).unwrap();
    t.inserir(&[Value::Int(1)]).unwrap();
    t.sincronizar().unwrap();
}

fn pedidos(d: &Path, fks: Vec<ForeignKey>) -> Table {
    let e = Schema::new(
        "pedidos",
        vec![
            Column::new("id", ColumnType::Int4).obrigatoria(),
            Column::new("cliente_id", ColumnType::Int4),
        ],
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico().primaria(),
            IndexDef::new("porCliente", vec![IndexColumn::asc(1)]),
        ],
    )
    .unwrap()
    .com_chaves_estrangeiras(fks)
    .unwrap();
    Table::criar(d, e).unwrap()
}

fn fk(nome: &str) -> ForeignKey {
    ForeignKey::new(nome, vec![1], "clientes", vec!["id".into()])
}

fn main() {
    // --- A: declarar num handle aberto NAO liga a conferencia -------------
    let d = std::env::temp_dir().join(format!("phx-portao-a-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    mae(&d);
    let mut p = pedidos(&d, Vec::new());
    p.redeclarar_chaves_estrangeiras(vec![fk("fk_cliente")]).unwrap();
    println!("A) declarei a chave conferida no handle ABERTO.");
    println!(
        "   o esquema diz verificar = {:?}",
        p.esquema().chaves_estrangeiras()[0].verificar
    );
    match p.inserir(&[Value::Int(1), Value::Int(999)]) {
        Ok(_) => println!("   inserir com cliente 999 (INEXISTENTE): >>> ACEITOU <<<"),
        Err(e) => println!("   inserir com cliente 999 (INEXISTENTE): recusou -- {e}"),
    }
    p.sincronizar().unwrap();
    drop(p);
    let mut p = Table::abrir(&d, "pedidos").unwrap();
    match p.inserir(&[Value::Int(2), Value::Int(999)]) {
        Ok(_) => println!("   depois de REABRIR, o mesmo insert: ACEITOU"),
        Err(e) => println!("   depois de REABRIR, o mesmo insert: recusou -- {e}"),
    }
    drop(p);
    std::fs::remove_dir_all(&d).ok();

    // --- B: tirar uma chave deixa posicao velha apontando para o vazio ----
    println!("\nB) tiro a segunda chave de um handle que abriu com duas:");
    let d = std::env::temp_dir().join(format!("phx-portao-b-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    mae(&d);
    let mut p = pedidos(&d, vec![fk("fk_a"), fk("fk_b")]);
    p.redeclarar_chaves_estrangeiras(vec![fk("fk_a")]).unwrap();
    println!("   chaves no esquema agora: {}", p.esquema().chaves_estrangeiras().len());
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        p.inserir(&[Value::Int(1), Value::Int(1)])
    }));
    match r {
        Ok(Ok(_)) => println!("   inserir: aceitou"),
        Ok(Err(e)) => println!("   inserir: recusou -- {e}"),
        Err(_) => println!("   inserir: >>> PANICO <<< (indice velho no vetor novo)"),
    }
    std::fs::remove_dir_all(&d).ok();
}
```
