# A Sombra: leitura repetível, e o que ela custa

> **A Sombra existe pela leitura repetível.** Em `por_lote` — o padrão, e o
> mundo escolhido pelo dono — ela **não compra desempenho**: medido,
> ~1,00×–1,21×. Quem a defender por velocidade está defendendo um número que
> morreu medido em 04/09/2026.

Este documento foi escrito **antes** do código, e é ele que o código obedece.
Onde os dois discordarem, um dos dois está errado — e a regra da casa é que o
número medido ganha do texto.

**Ele não implementa nada.** Ele existe para que a decisão de fazer ou não
fazer seja tomada com o custo na mesa, e a decisão é do dono. O pedido 179
pediu esta primeira linha com o motivo escrito: *senão daqui a três meses
alguém a defende com um ganho de desempenho que ninguém mediu.*

**Todo número desta página traz o arquivo ou o documento onde foi conferido**
(§8). Onde não houve como medir, está escrito **«não medido»** com o que
decidiria o número — nomeado vale, estimado não.

---

## 0. O que é a Sombra, em uma tela

Uma cadeia de versões velhas pendurada no `rowid`, **fora do `.reg`**, que
nenhum caminho de escrita paga quando não há leitor com visão aberta.

Depois das sete respostas do dono de 04/09/2026
(`docs/PESQUISA-MVCC-E-FORMATO.md` §8.0), ela é:

| peça | o que o dono decidiu |
|---|---|
| onde a versão velha mora | **em RAM**, no molde da `Sobreposicao` que já entrega *read-your-own-writes* — **zero mudança de formato em disco** |
| o teto | o `transacao_prazo_min` que já existe, **5 min** (`config.rs`), com a recusa do leitor longo como segunda rede |
| o `.ndx` | **não ganha marca**: alteração de coluna indexada sob visão aberta é **recusada**, cedo e por escrito |
| o que promete | ***snapshot isolation***, e o documento diz isso — não `SERIALIZABLE` |

**O que ela não é.** Não é WAL, não é undo durável, não é reuso de espaço do
`.reg`, não é gestor de travas e não é acelerador. A undo daqui **não desfaz
nada** — toda linha do `.reg` já foi cometida —, e é por isso que ela pode
morar em RAM e morrer com o processo.

---

## 1. O que ela compra, com a prova do que hoje falha

O `docs/ACID.md` §4.1 mede os fenômenos da norma **acontecendo**, um a um, por
soquete, cada um com o controle da mesma corrida. O PhxSql é **`READ
COMMITTED`**, e quatro fenômenos acontecem. A Sombra fecha **dois**.

| fenômeno | como se mediu hoje | a Sombra fecha? |
|---|---|---|
| **leitura não repetível** | duas leituras da mesma linha na mesma transação: `50` e depois `77` | **sim** — é para isso que ela existe |
| **fantasma** | a mesma varredura na mesma transação: 2 linhas e depois 3 | **sim, mas com uma peça que as dez divergências não têm** (§1.2) |
| **perda de atualização** | as duas leram `10`, as duas somaram 1, e o final é `11` em vez de `12` | **não — e não precisa** (§1.3) |
| **skew de escrita** | as duas viram 2 de plantão, cada uma tirou a sua, sobraram **0** | **não — e ela o torna sistemático** (§1.4) |

E a matriz do `ACID.md` §4.3, que é a prova do buraco em cima de um invariante
e não de uma linha só: com o escritor **em transação**, uma varredura única
nunca vê o par quebrado (**0 de 400**), e **duas** leituras separadas o veem
**73 de 400**. *O `COMMIT` é atômico, mas ele acontece inteiro entre a primeira
leitura e a segunda.* É esse 73 que a Sombra zera.

### 1.1 Leitura não repetível — fecha, e é a única razão de ela existir

O leitor abre visão, lê `V`; um escritor comete `V'`; o leitor relê e continua
vendo `V`. Fechada a visão, ele vê `V'`. Nenhum `RwLock` faz isso: a §16 do
`docs/CONCORRENCIA.md` entregou os leitores **simultâneos**, não
**consistentes**, e a própria seção escreve isso em «o que esta medição NÃO
diz».

É defeito de **resultado**, não de tempo. Nenhuma medição de p99 o mostraria, e
é por isso que ele sobreviveu a todas as baterias de concorrência desta casa.

### 1.2 Fantasma — fecha, com uma peça que o desenho não tem

**Este é o achado deste documento, e ele é um buraco no desenho.**

As dez divergências da §7 da `PESQUISA-MVCC-E-FORMATO.md` respondem **onde a
versão velha mora**. Nenhuma delas responde a outra pergunta, que o fantasma
faz: *como a visão recusa ver uma linha que NASCEU depois dela?* Uma linha
recém-inserida **não tem versão velha** — a cadeia de sombra, por construção,
não tem nada a dizer sobre ela.

Sem essa peça, a Sombra fecharia a leitura não repetível (**50 → 77**) e
deixaria o fantasma (**2 → 3**) de pé — e **meia consistência é pior que
nenhuma**, porque uma varredura que esconde linha nova e mostra valor novo
devolve um estado do banco que nunca existiu.

**A peça, e ela é barata por um motivo que se pode nomear:** a linha já carrega
uma ordem de nascimento em disco. A coluna de sistema **`rownum`** é um
contador por tabela, atribuído na gravação, que **nunca reaproveita número**
(`crates/phxsql-core/src/schema.rs`, `COLUNA_ROWNUM`; o contador vive nos bytes
92..100 do cabeçalho do volume 1, `reg.rs`). Como nada vai a disco antes do
`COMMIT`, a ordem do `rownum` **é** a ordem de commit. Então o filtro de
fantasma é uma comparação por linha lida, contra a marca que a visão anotou ao
abrir — **sem byte novo, sem arquivo novo, sem estrutura nova**.

E as três ressalvas, porque «barato» sem o motivo é o erro que o pedido 179
manda não repetir:

* **`rownum` é `Option`**: `Schema::coluna_rownum()` devolve `None` numa tabela
  gravada antes da v5 do esquema (`schema.rs`). Tabela sem a coluna precisa de
  registro de nascimento em RAM, ou a visão sobre ela **recusa** — e recusar é
  o estilo desta casa.
* **`rownum` é por TABELA**, e uma visão sobre duas tabelas precisa de um
  instante só. Ou a marca das duas se toma junto, **sob a trava**, no `abrir
  visão`, ou a visão não é consistente entre tabelas — e aí ela mente sobre
  exatamente o tipo de invariante que o `ACID.md` §4.3 mede.
* **o `rowid` NÃO serve** para isso, e essa é a armadilha: na partição
  alfanumérica ele não é monotônico no tempo — *«a Silva digitada primeiro mora
  no `_S`, com rowid alto, e a Alves digitada depois mora no `_A`, com rowid
  1»* (`docs/FORMATO.md`). É a mesma razão pela qual o `rownum` existe.

**Consequência para a ordem de trabalho, e ela é do DBA:** se a Sombra for
feita, **o filtro de nascimento se desenha ANTES da cadeia de versões**, porque
é ele que decide se a visão é uma visão ou meia visão.

### 1.3 Perda de atualização — não fecha, e não precisa

O `ACID.md` mede as três respostas de hoje na mesma corrida: duas escritas
soltas perdem uma (`11` em vez de `12`); mandando `"versao"`, a segunda volta
**`CONFLITO`**; dentro de transação, a segunda espera o `LOCK TIMEOUT` e volta
**`EM_TRANSACAO`**.

A Sombra **não toca o caminho de escrita** — a trava exclusiva e a trava por
linha continuam onde estão —, então as três respostas continuam exatamente como
foram medidas. Ela não fecha esse fenômeno e **não o reabre**, que é a parte que
precisa estar escrita: *snapshot isolation* mal-feito costuma reabri-lo, quando
a escrita passa a decidir a partir da foto em vez do estado corrente. **Aqui a
escrita nunca lê da sombra.**

### 1.4 Skew de escrita — não fecha, e ela o torna SISTEMÁTICO

*Snapshot isolation* admite *write skew* por definição
(`PESQUISA-MVCC-E-FORMATO.md` §5.7; resposta 6 do dono). O `ACID.md` já o mede
acontecendo: duas transações viram 2 de plantão, cada uma tirou a sua, as duas
confirmaram, e sobraram **0**.

E há um agravante que precisa estar escrito **antes** de alguém prometer:
hoje o fenômeno depende de uma **corrida** — em `READ COMMITTED` cada leitura
busca o estado do momento, e a segunda transação **pode** enxergar o trabalho da
primeira e desistir. Com a visão congelada no começo, ela **nunca** enxerga: o
que hoje acontece por azar passa a acontecer sempre que o padrão ocorrer.

**A Sombra melhora a leitura e piora esse caso.** Fechá-lo é `SERIALIZABLE`
(SSI), que é outro item, com gestor de travas próprio e transações que
**abortam** — e o cliente tem de saber repetir.

---

## 2. As dez divergências, e a restrição nossa que causou cada uma

A lei da casa: *o que torna uma lógica nossa não é tê-la reescrito, é tê-la
**re-decidido contra as nossas restrições**, e a prova de que passou pela nossa
cabeça é a divergência.* A pergunta que decide é uma só — **onde esta lógica
diverge da de origem, e qual restrição nossa causou a divergência?**

As oito receitas foram lidas no fonte e estão em
`docs/PESQUISA-MVCC-E-FORMATO.md` §10 (InnoDB, PostgreSQL(R), Oracle(R), SQL
Server(R), Firebird, RocksDB, LMDB/Bolt e Cassandra(R)). **A ordem de digitação
matou quatro** antes de qualquer código (§6.1 a §6.5 de lá).

| # | divergência | contra quem | **a restrição NOSSA que a causou** | estado no desenho escolhido |
|---:|---|---|---|---|
| 1 | a visão é **um número**, e não um trio com a lista de ativos | InnoDB (`m_ids`), PostgreSQL(R) (`xip`) | a transação **empilha em RAM e só aplica no `COMMIT`**, sob a trava que serializa: **toda linha do `.reg` já foi cometida**, então não há o que pular | **de pé** |
| 2 | a linha **não** carrega identificador de transação | InnoDB (`DB_TRX_ID`, 6 bytes) | **corolário da 1** — a pergunta «esta linha é de quem não cometeu?» não existe aqui | **de pé, mas não é decisão própria** (§2.2) |
| 3 | o ponteiro é **índice de 24 bits** num diretório por tabela, não endereço estruturado | InnoDB (`DB_ROLL_PTR`, 7 bytes) | o cabeçalho do slot tem **3 bytes livres sempre e 11 na v4**, medidos no `reg.rs`, e o CRC **não cobre o cabeçalho** | **sem objeto** — a resposta 1 do dono pôs a sombra em RAM, e não há ponteiro em disco (§2.2) |
| 4 | o registro é **delta**, não a foto inteira | o `.trash` desta casa | o `.trash` copia a linha inteira e copiaria **megabytes de `Memo`** numa alteração que mexeu num inteiro — e em RAM esse byte é o recurso escasso | **de pé, e mais forte em RAM** |
| 5 | a coluna externa vai por **conteúdo**, não por prefixo mais referência | InnoDB (`BTR_EXTERN_FIELD_REF_SIZE`) | o `.bin`/`.memo` **reaproveita bloco liberado** — a pétrea de não reaproveitar vale para o `.reg`, não para os externos —, e a foto voltaria sendo *a foto de outra linha* (`FORMATO.md`) | **de pé** |
| 6 | o `.ndx` **adia** a remoção em vez de marcar a entrada | InnoDB (*delete-marking*), zheap | a folha é `chave completa + rowid`, `ck_len = key_len + 8`: um bit de marca muda a **largura** da entrada, e vira segundo formato a versionar. E remover **não rebalanceia** (`FORMATO.md` §2), logo remover é adiável | **sem objeto, e por um motivo mais forte que o registrado** (§2.3) |
| 7 | quando o teto estoura, quem é recusado é o **LEITOR** | Oracle(R) recusa o leitor (`ORA-01555`); PostgreSQL(R) não tem teto e paga em *bloat* | recusar o escritor deixaria um leitor longo parar toda a gravação — e a gravação **já regride sob disputa: 0,51× com dois clientes**, medido | **de pé** |
| 8 | resolve na leitura por **marca monotônica**, não por relógio | Cassandra(R) (`Cells.resolveRegular`, carimbo de hora) | a marca é gerada **sob a trava global que já existe** — sem relógio, sem carimbo do cliente, sem empate. *A propriedade que eles compram com protocolo, nós já temos de graça pelo defeito que estamos tentando remover* | **de pé** |
| 9 | **nada desaparece** na resolução | Cassandra(R) (o perdedor some sem aviso) | o `CASSANDRA.md` §7.4 já recusou *«trocar uma recusa por uma perda silenciosa»*; aqui não há disputa de autoria, há duas épocas da mesma linha | **de pé** |
| 10 | a fusão é sobre **duas** fontes, não sobre N | Cassandra(R) (memtable + N SSTables) | temos o slot corrente e a cadeia — e a cadeia é **vazia no caso comum**, o que faz o custo cair a um teste de ponteiro nulo | **de pé** |

### 2.1 Todas têm restrição nomeada — e três merecem uma frase a mais

**Sim: as dez nomeiam uma restrição nossa.** Nenhuma delas é «façamos
diferente para não parecer cópia». Mas o inventário honesto não para aí, porque
**divergência com restrição nomeada não é o mesmo que decisão independente ainda
de pé**:

* **a 2 é corolário da 1.** Ela não carrega restrição própria: some junto se a
  1 cair. São **nove** decisões, e não dez.
* **a 3 perdeu o objeto.** Ela responde *onde cabe o ponteiro em disco*, e a
  resposta 1 do dono (RAM, zero formato) apagou a pergunta. Continua correta e
  continua na prateleira para o dia em que a opção B voltar — mas **defendê-la
  hoje é defender um desenho que o dono não escolheu**.
* **a 6 perdeu o objeto por um motivo mais forte do que o registrado** — e esse
  motivo é da §2.3, que é o segundo achado deste documento.

Sobram, no desenho escolhido, **sete decisões nossas de pé**: 1, 4, 5, 7, 8, 9
e 10.

### 2.2 O que a resposta 1 do dono fez com os 3 bytes livres

O cabeçalho do slot tem **24 bytes** (`SLOT_CAB`, `reg.rs`), e conferido byte a
byte nesta revisão: `flags` (byte 1) e `res` (2..4) **não têm uma escrita nem
uma leitura** no `reg.rs`, e o `tempero` (16..24) só é gravado dentro do ramo
`if material.cifrado()`. São **3 bytes livres sempre, 11 na v4**. O CRC cobre
`slot[SLOT_CAB..]` — **só o payload** —, então escrever ali não invalidaria
nada.

**E, mesmo assim, a recomendação deste documento é NÃO usá-los.** O motivo é
que a sombra mora em RAM e esses bytes moram em disco: um índice gravado no
slot **sobrevive ao processo que o deu**, e depois de um reinício ele aponta
para uma posição de um diretório que não existe mais — ou, pior, para a de outra
linha, num diretório novo que reusou o número. A defesa da §7.2 da pesquisa (a
sombra carrega o próprio `rowid`, como o `.trash` faz no offset 12) transforma
isso de erro silencioso em recusa, o que é a metade certa da solução — mas o
ponteiro continua sendo estado durável apontando para estado volátil, que é
exatamente o casamento que o `FORMATO.md` evita em todo o resto do motor.

**O que substitui, e por que é barato — dito com o mecanismo, e não com a
palavra «barato»:** um mapa `rowid → sombra` em RAM, no molde exato da
`Sobreposicao` que já existe (`BTreeMap<RowId, Troca>` mais a lista dos
nascidos, em `table.rs`). O que o torna barato **não** é o `BTreeMap`: é o
portão do §7.5 da pesquisa — um `bool` por tabela, «há visão aberta?», lido
**antes** de tocar o mapa. Sem visão aberta o mapa não é consultado, não é
alocado e não é comparado; o caminho quente de hoje não muda de forma nenhuma.
*Instrumentação desligada tem de custar zero, e o portão que decide isso vem
antes do trabalho.*

### 2.3 A divergência 6 tem um buraco, e ele é maior que o registrado

A §7.4 da pesquisa chama o adiamento do `.ndx` de *«a divergência de que mais
me orgulho»*, e o argumento é: o índice passa a **superinformar** — devolve
rowids cuja chave corrente é outra —, *«e quem filtra é a verificação que o
leitor já vai fazer de qualquer jeito»*.

**Conferido no fonte, essa frase é falsa para quem não tem visão aberta — que é
o caso comum.** `Table::buscar` (`crates/phxsql-store/src/table.rs`) devolve o
que o `.ndx` respondeu **sem reler a chave corrente da linha**; a única
filtragem que ele faz é a das linhas pendentes da própria transação. Um leitor
comum, que nunca pediu MVCC, passaria a receber linhas sob uma chave que elas
não têm mais.

**E o pior consumidor desse índice não é um leitor: é a integridade
referencial.** O `Table::conferir_fks` responde *«existe este pai?»* chamando
`mae.buscar(&indice, &chave)` e usando o resultado como prova de existência. Com
uma entrada velha adiada no índice da mãe, a resposta seria **sim** para um pai
cuja chave já mudou — e entraria uma órfã. Do outro lado, a entrada velha no
índice da filha faria o `excluir` da mãe recusar uma exclusão legítima.

**Isso não é um detalhe do MVCC: é a regra primordial da integridade** — *nunca
se mata o pai que tem filhos* —, e ela não tem visão aberta nenhuma para
filtrar. Então:

* a resposta 4 do dono (**recusar** a alteração de coluna indexada sob visão
  aberta) está certa, e por um motivo mais forte que o registrado: não é só
  «é a única que preserva o zero-formato» — é a única que não põe uma resposta
  errada no caminho da conferência de chave;
* e o adiamento, se algum dia voltar, **volta com o preço inteiro**: cada
  consumidor de índice — `buscar`, `varrer_indice`, `intervalo`, a página
  ordenada e as duas pontas do `conferir_fks` — passa a precisar do filtro,
  ligado por um portão que hoje não existe em nenhum deles. *Guarda nova entra
  pedida, não imposta*, e o adiamento é uma guarda **imposta** a quem nunca a
  pediu.

---

## 3. O que ela custa, item a item

### 3.1 O que está MEDIDO

| custo | número | onde foi medido |
|---|---|---|
| o desempenho que ela **não** compra, no padrão | **1,21× e 1,00×** (`por_lote`, carga de 50) e **1,00× · 0,91× · 1,13× · 1,02×** (carga de 1.000) | `CONCORRENCIA.md` §11.2 e §11.2-bis |
| o desempenho que ela compra **fora** do padrão | **3,23× e 2,77×** (`por_operacao`, carga de 50) | `CONCORRENCIA.md` §11.2-bis |
| o mecanismo dos dois números | leitor sozinho **738 µs**; com outro leitor **911 µs**; com um escritor **2.527 µs** — a diferença é o `fsync` sob a trava, **1.267–1.371 µs** | `CONCORRENCIA.md` §11.2-bis e §7.1 |
| a trava presa, para comparar com qualquer coisa que ela acrescente | gravando `por_lote` **136–145 µs**; lendo 50 linhas **240–265 µs** | `CONCORRENCIA.md` §7.1-bis |
| a marca monotônica sob a trava | um `lock` sem disputa custa **13,2 ns**, e o incremento é uma soma | `DESEMPENHO.md` §14 |
| quantos caminhos de leitura precisariam consultar a sombra | o mapa da trava conta **28** seções de «leitura com varredura» e **10** de «leitura curta»; a ficha compartilhada do `RwLock` alcança **uma** operação | `CONCORRENCIA.md` §1.2 e §16 |
| o teto de RAM que já se multiplica por leitor | uma grade **ordenada** toca **1.668 páginas = 6,52 MiB** do cache do `.ndx` (81% do teto de 8 MiB por tabela aberta); sem ordem, **0**; busca por chave, **3** | `CONCORRENCIA.md` §16.7 |
| os tetos que já existem e nos quais ela cabe | `transacao_prazo_min` **5 min**, `transacao_max_linhas` **100.000** (~20 MiB pelo comentário do próprio campo), `memoria_max_mb` **0 = sem teto**, `conexoes_max` **64** | `crates/phxsql-server/src/config.rs` |
| o cabeçalho do slot, que **não** está cheio | **3 bytes livres sempre, 11 na v4**; o CRC cobre só o payload | `crates/phxsql-store/src/reg.rs` (conferido nesta revisão) |

**O custo de formato em disco é ZERO**, e essa é a única linha desta tabela que
é boa notícia: nem `.reg` v6, nem marca no `.ndx`, nem extensão nova. As listas
`EXTENSOES` (6) e `EXTENSOES_TODAS` (10) do `catalogo.rs` **não mudam** — e
elas são o lugar onde esta casa já esqueceu extensão nova **duas vezes**, a
segunda deixando para trás a trilha de dados pessoais.

### 3.2 O que fica NOMEADO e NÃO MEDIDO

Esta frente não mede tempo por decisão: há outras frentes na máquina, e número
medido em máquina disputada é número que o `quieta.Vigia` reprova. O que falta
fica com **o que decidiria o número**:

1. **O teto exclusivo do MVCC depois do `RwLock`.** Os 3,23× e 2,77× foram
   medidos com a trava ainda em `Mutex`. O `escolher-o-desenho.py` calcula esse
   teto como *p99 com um escritor ao lado ÷ p99 com dois leitores ao lado*, e a
   §16 do `CONCORRENCIA.md` acabou de baratear o denominador — quatro clientes
   lendo passaram de
   1,6×–1,8× para 3,8×–3,9×. **A direção é previsível (a razão cresce); o
   número não existe.** *Decide-se rodando `LINHAS_LIDAS=50
   python3 bancada/concorrencia/escolher-o-desenho.py` com a máquina parada e o
   vigia aprovando duas baterias.*
2. **Quantos bytes uma sombra ocupa numa linha larga.** O oráculo do roteiro
   mostrou a *history list* indo de **7 a 207** com um leitor aberto. *Decide-se
   com o cenário reproduzido e o delta medido em bytes por versão; se 200
   versões de uma linha larga estouram um teto razoável, a opção A morre
   medida — e isso é resultado tão válido quanto ganho.*
3. **Quanto custa gravar uma versão velha DURÁVEL.** O `excluir` é o protótipo
   pronto: é a única operação que já escreve versão velha em arquivo separado e
   já sincroniza antes de liberar o slot. *Decide-se com um terceiro laço, de
   `excluir`, no `quanto-a-trava-fica-presa.py`.* Sob a resposta 1 do dono ele
   não decide mais entre RAM e disco — decide se a recusa do leitor longo é
   confortável ou apertada.
4. **O custo por linha de consultar a sombra com visão aberta.** Uma busca em
   `BTreeMap` por linha lida, mais o filtro de nascimento. *Decide-se medindo a
   `Sobreposicao` que já existe: ela faz exatamente isso hoje, no
   read-your-own-writes, e ninguém cronometrou.*
5. **Quantos leitores longos existem numa carga real.** Nenhuma medição desta
   casa conta com que frequência alguém faz duas leituras dependentes dentro de
   uma transação. Sem esse número, não se sabe **quantas vezes por dia** a
   leitura repetível faz falta — e é ele que separa «defeito real» de «defeito
   correto». *Decide-se contando, na telemetria, transações com mais de uma
   leitura da mesma tabela.*
6. **O custo do registro de nascimento** para tabela sem `rownum` (anterior à
   v5 do esquema). *Decide-se contando quantas tabelas de uma base real não têm
   a coluna; se forem zero, a recusa é gratuita.*

### 3.3 O que acontece com backup, restauração e replicação

**Backup: não muda, e não ajuda.** O `op_backup` toma a ficha exclusiva e a
segura pela cópia inteira — o comentário no `servidor.rs` diz por quê: *«é o
que "consistente" quer dizer sem transação: nenhuma escrita acontece no meio»*.
E o `backup::executar_zip` copia **arquivos**, não linhas. Como a sombra mora em
RAM e o `.reg` em disco carrega só a versão corrente, **abrir uma visão não daria
ao backup uma cópia consistente sem trava**. Quem esperar isso da Sombra vai se
decepcionar, e é melhor que se decepcione aqui.

**Restauração: não muda.** Não há arquivo novo para restaurar, e a sombra morre
com o processo — que é a consequência de ela não desfazer nada.

**Replicação: não muda no fio, e muda onde dói.** O `.reg` continua ganhando
**um slot por linha nova e nenhum por versão**, então os rowids da réplica
continuam batendo com os do source e o `aplicar_evento` não para. Essa é a
condição inteira sob a qual a §4.1 do `CONCORRENCIA.md` autoriza o MVCC, e ela
é honrada. **O que muda é o consumo de RAM da réplica**: o modo D (*read
replica*) existe justamente para hospedar leitor longo, e é lá que a cadeia de
sombra cresce mais — no servidor que também está aplicando eventos sem parar.
**Não medido**, e o que decidiria o número é o item 2 da §3.2 rodado contra uma
réplica em regime.

### 3.4 O que ela custa em código, dito sem a palavra «barato»

O pedido 179 traz a armadilha por escrito: a §11.3 do `CONCORRENCIA.md` dizia
que o `RwLock`
«custa uma linha», e isso era a premissa que o próprio 164 já tinha matado —
`RwLock<Instancia>` **compila de primeira e está errado**. Então aqui não se diz
«pequeno»; diz-se **o quê**:

* **um portão por tabela** (`bool` «há visão aberta?»), lido antes de tudo — é
  o que faz o custo desligado ser zero;
* **um mapa por tabela**, no molde da `Sobreposicao`, e a marca global
  incrementada sob a trava que já é tomada;
* **um registro de visões abertas**, que a purga consulta para saber qual é a
  mais velha. É a peça com o custo escondido: um leitor com a **ficha
  compartilhada** (`CONCORRENCIA.md` §16) não pode alterar estado dentro da
  `Raiz`, então abrir e
  fechar visão pede ou a ficha exclusiva — que desfaz o paralelismo de leitura
  recém-comprado — ou **uma segunda trava**, tomada por dentro da primeira. Esta
  casa tem três guardas de abraço mortal e uma catraca de «trava fora do ponto
  único» exatamente por causa desse tipo de peça. **Não medido**, e o que
  decidiria é o mapa da trava rodado sobre o desenho, antes do código;
* **a consulta em cada caminho de leitura que a visão alcança** — e a §1.2 do
  `CONCORRENCIA.md` conta 28 + 10 seções de leitura, enquanto a ficha
  compartilhada alcança **uma** operação. A visão que só valesse no `varrer`
  quebraria justamente o par que o `ACID.md` mede: `ler` + `ler`, **73 de 400**;
* **duas recusas novas** e o texto delas, que entra pela fábrica de idiomas
  (`erro.versao_recolhida` para o leitor longo, e a da alteração de coluna
  indexada) — em seis idiomas, com a catraca do conferidor no mesmo commit.

---

## 4. O que ela NÃO resolve

* **O comboio do fecho de janela (pedido 180).** Medido: o p99 do escritor
  cresce **2,25× e 2,13×** de K=1 para K=4 tabelas, e o do **leitor que lê uma
  tabela que ninguém escreve**, **2,01× e 1,96×**. Não é leitor-contra-escritor:
  é um escritor segurando a trava global por trabalho que **não é dele** — as
  tabelas dos outros. Nem `RwLock` nem MVCC o consertam; com a Sombra, os
  leitores continuariam todos parados atrás do mesmo comboio.
* **Escritor contra escritor.** A regressão de **0,51× com dois clientes**
  continua inteira: a trava é tomada antes de qualquer noção de versão existir.
* **Leitor contra leitor.** Isso é a SP000011, e ela **já entrou** para o
  `varrer` (`CONCORRENCIA.md` §16): quatro clientes passaram de 1,6×–1,8× para
  3,8×–3,9×. As
  outras 75 seções continuam exclusivas.
* **`SERIALIZABLE`.** É *snapshot isolation*, e o *write skew* é a diferença
  (§1.4).
* **Backup consistente sem trava** (§3.3).
* **A cascata dentro da transação.** O *read-your-own-writes* não alcança a
  cascata (`ACID.md` §4.4), e a Sombra não muda isso: ela guarda versões
  velhas, não antecipa escritas futuras.

---

## 5. As alternativas mais baratas, com o custo de cada uma

Um documento que só apresenta o desenho grande está pedindo **aprovação**, não
decisão.

### (a) Perguntar em UMA instrução — custo zero de código

O `ACID.md` §4.3 mede: contra um escritor **em transação**, uma varredura única
nunca vê o par quebrado (**0 de 400**); duas leituras separadas o veem **73 de
400**. Boa parte do que se quer da leitura repetível já está entregue para quem
pergunta de uma vez só.

**Custa:** documentação, e uma linha no manual do cliente. **Não fecha:** o que
não cabe numa instrução — o relatório que percorre três tabelas, a conferência
que soma antes e depois.

### (b) Leitura repetível **pela trava**, pedida — custo: escritor esperando

O cliente que **pedir** (`"leitura_repetivel": true`, no estilo do `"versao"`)
segura a ficha compartilhada por toda a transação, e a consistência sai por
exclusão em vez de por versão.

**Custa** exatamente o que o MVCC existe para não pagar: o escritor espera o
leitor. Com o número na mesa, dá para dimensionar: uma página de grade segura a
trava **240–265 µs**, e isso ninguém sente; um relatório de cinco minutos
pararia toda a gravação até o `transacao_prazo_min` o matar. **Custa também** a
disciplina que a §16 do `CONCORRENCIA.md` acabou de comprar: ficha compartilhada
segurada entre
pedidos, com as guardas de reentrância no caminho.

**Compra:** leitura repetível **e** ausência de fantasma, sem sombra, sem
purga, sem marca e sem mapa. É, disparado, a alternativa mais barata de
construir — e a mais cara de operar para quem tem leitor longo. *Guarda nova
entra pedida, não imposta:* quem não pedir continua exatamente como hoje.

### (c) Só o filtro de nascimento — **RECUSADA aqui, com o motivo**

Fechar apenas o fantasma custa quase nada (§1.2): uma comparação de `rownum`
por linha lida. É tentador, e está **recusado**: uma varredura que esconde a
linha nova e mostra o valor novo devolve um estado que **nunca existiu no
banco**. *Meia consistência é pior que nenhuma*, porque a inteira o cliente sabe
que não tem.

### (d) Cópia do resultado no cliente — custo: nenhum nosso

Quem precisa de duas leituras coerentes pode ler uma vez e reusar. É o que toda
aplicação faz hoje, e está aqui para que a lista de opções não pareça mais curta
do que é.

---

## 6. A recomendação, e a ordem — a decisão é do dono

**A recomendação desta frente é: não construir a Sombra agora, e construí-la
quando um cliente pedir leitura repetível.** Os motivos, em ordem de peso:

1. **A urgência morreu com a resposta 1 do dono.** O argumento que punha a
   SP000016 cedo era *«mudança de formato entra cedo — enquanto não há dado em
   produção é barata, depois vira migração»*. **Com a sombra em RAM não há
   mudança de formato nenhuma.** O que era «decida agora ou pague migração
   depois» virou «decida quando precisar». Este é o argumento decisivo, e ele é
   uma consequência da decisão do próprio dono, não uma preferência minha.
2. **O desempenho não a justifica no mundo escolhido.** `por_lote` é o padrão e
   é o mundo para o qual se otimiza: ~1,00×–1,21×. Em `por_operacao` ela vale
   3,23× e 2,77×, e quem escolhe entre os dois mundos é o `recursos.durabilidade`
   do dono do banco.
3. **A correção a justifica, mas ninguém contou a frequência.** O buraco é real
   e está medido (73 de 400), e **quantas vezes por dia ele morde é o item 5 da
   §3.2** — não medido. Construir um gestor de versões para um defeito cuja
   frequência ninguém contou é o mesmo erro que o pedido 113 já pagou: *medir a
   premissa do item vem antes de implementar o item.*
4. **Há duas coisas mais baratas na frente dela.** A (a) custa documentação; a
   (b) custa uma guarda pedida e entrega mais do que a Sombra na leitura — e
   nenhuma das duas fecha o caso do leitor longo, que é o único território
   exclusivo dela.
5. **E há um item medido, maior, no mesmo território:** o comboio do fecho
   (pedido 180), com 2,25× no escritor e 2,01× no leitor **que não escreve
   nada**. Ele morde todo mundo hoje; a leitura repetível morde quem faz duas
   leituras dependentes.

**Se a decisão for construir**, a ordem que esta frente recomenda, e o motivo de
cada passo:

1. **o filtro de nascimento primeiro** (§1.2) — é ele que decide se a visão é
   visão ou meia visão, e as dez divergências não o resolvem;
2. **o registro de visões e a purga em seguida** — é a peça com o custo
   escondido (§3.4), e é onde uma segunda trava pode desfazer o paralelismo que
   a §16 do `CONCORRENCIA.md` comprou;
3. **a cadeia de versões por último** — é a parte que já tem desenho, molde
   (`Sobreposicao`) e prova escrita (§7);
4. e **antes de qualquer uma das três**, o item 1 da §3.2: remedir o teto
   exclusivo com o `RwLock` no lugar. O número que justificaria a obra foi
   medido contra uma trava que não existe mais.

**A ordem final é do dono**, e o pedido 179 diz isso com todas as letras:
*«fica a decisão do dono sobre a ordem»*. Este documento põe o custo na mesa;
não decide por ele.

---

## 7. Como se prova, nos dois sentidos

Prova real é nos dois sentidos: **o teste tem de FALHAR com o defeito reposto** e
passar com o conserto. Teste que passa por engano é pior que teste que falta.

| o que se prova | passa quando | **falha com o defeito reposto quando** |
|---|---|---|
| **leitura repetível** | o leitor abre visão e lê `V`; o escritor comete `V'`; o leitor relê e ainda vê `V`; fecha, reabre e vê `V'` | desligada a consulta à sombra, a segunda leitura devolve `V'` — o teste tem de ficar vermelho |
| **fantasma** | a mesma varredura, repetida na mesma visão, devolve o **mesmo número de linhas** depois de um `INSERT` alheio confirmado | desligado o filtro de nascimento, ela devolve 2 e depois 3 — que é o número que o `ACID.md` mede hoje |
| **meia consistência não passa** | a varredura repetida devolve o mesmo conjunto **e** os mesmos valores | ligado só o filtro de nascimento, o teste tem de reprovar: esconder linha nova e mostrar valor novo é estado que nunca existiu |
| **custo zero desligado** | `varrer` e `inserir` sem visão aberta, antes e depois, dentro da tolerância do `quieta.Vigia` | movido o portão para **depois** da montagem do registro, a curva de `inserir` cai — é o defeito do Profiler reposto |
| **a recusa é do LEITOR** | teto minúsculo, leitor longo aberto, escritor gravando além do teto: o **leitor** recebe `erro.versao_recolhida` e o escritor mantém a vazão de um cliente | trocado o alvo da recusa, o escritor bloqueia e a vazão cai — o teste tem de ver a queda |
| **a replicação não sente** | bancada de replicação (modo A) com a Sombra ligada: os rowids da réplica batem com os do source | forçada a alocação de um slot por versão, o `aplicar_evento` **para** — e é ele que dá o veredito |
| **a integridade não vê índice sujo** | com visão aberta, a alteração de coluna indexada é **recusada**, e o `conferir_fks` continua respondendo pela chave corrente | adiada a remoção da entrada velha, o `conferir_fks` aceita uma órfã — e é este o teste que decide a §2.3 |
| **o comportamento VELHO não muda** | um cliente que nunca abre visão lê exatamente o que lia antes, com os mesmos números | é o teste que tem de existir primeiro: *guarda nova entra pedida, não imposta* |

---

## 8. Onde cada número desta página foi conferido

**Desta casa, em documento:**

* `docs/ACID.md` §4.1 e §4.3 — os quatro fenômenos medidos acontecendo, a
  matriz 97/0/4/73 de 400, e o nível `READ COMMITTED`.
* `docs/CONCORRENCIA.md` §1.2 (as 76 seções e as classes), §1.3 (o `fsync` sob a
  trava), §4.1 a §4.3 (a objeção da §11.1 e o que envelheceu), §7.1 e §7.1-bis
  (a trava presa e o custo do `fsync`), §10.5 (0,51×), §11.2 e §11.2-bis (os
  tetos e as medianas), §16 e §16.6 e §16.7 (o `RwLock` entregue, o ganho e a
  RAM por leitor).
* `docs/PESQUISA-MVCC-E-FORMATO.md` §5 a §8 — as oito receitas lidas no fonte, o
  que foi recusado com o número, o desenho da Sombra e as sete respostas do
  dono.
* `docs/DESEMPENHO.md` §14 — o `lock` sem disputa em 13,2 ns.
* `docs/FORMATO.md` — a remoção do `.ndx` que não rebalanceia, o `rownum` e a
  exceção da partição alfanumérica, e o bloco externo que se reaproveita.
* `docs/PENDENCIAS.md` pedidos 164, 179, 180, 183 e 187.

**Conferido no fonte nesta revisão** (é o que esta frente acrescenta em
verificação, e não em prosa):

* `crates/phxsql-store/src/reg.rs` — `SLOT_CAB = 24`; o CRC cobre
  `slot[SLOT_CAB..]`; `flags` e `res` sem escrita nem leitura; `tempero` só sob
  `if material.cifrado()`; `proximo_rownum` nos bytes 92..100.
* `crates/phxsql-store/src/table.rs` — a `Sobreposicao` (`BTreeMap<RowId,
  Troca>` mais os nascidos); `Table::buscar`, que devolve o que o `.ndx` diz sem
  reler a chave corrente; `Table::conferir_fks`, que usa esse `buscar` como
  prova de existência do pai.
* `crates/phxsql-store/src/catalogo.rs` — `EXTENSOES` (6) e `EXTENSOES_TODAS`
  (10).
* `crates/phxsql-store/src/lixeira.rs` — o registro do `.trash` carrega o
  `rowid` no offset 12.
* `crates/phxsql-core/src/schema.rs` — `COLUNA_ROWNUM` e `coluna_rownum()`, que
  devolve `None` em tabela anterior à v5 do esquema.
* `crates/phxsql-server/src/config.rs` — `transacao_prazo_min` 5,
  `transacao_max_linhas` 100.000, `memoria_max_mb` 0, `conexoes_max` 64,
  `cache_paginas` 2.048.
* `crates/phxsql-server/src/servidor.rs` — o `op_backup` segurando a ficha
  exclusiva pela cópia inteira.

**Nada aqui foi copiado de motor nenhum.** As oito receitas foram lidas como
descrição e norma, e o que entrou neste documento são as **divergências** — cada
uma com a restrição nossa que a causou (§2). *Inspiração que pula a medição não
é inspiração: é cópia com outro nome.*
