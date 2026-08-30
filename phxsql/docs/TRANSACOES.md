# Transações no PhxSql: o desenho, e o que dele virou código

Documento de **decisão**, escrito antes da implementação e de propósito. Cada
escolha aqui está amarrada a uma linha de código que já existe, porque o que
mata um desenho de transação neste motor não é a teoria — é uma regra do
formato que ele não pode quebrar.

> **Estado:** implementado. `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`
> existem pelo protocolo e pelo SQL, com escopo e prazos declarados na
> abertura, travas de intenção na tabela e exclusivas na linha, máquina de
> estados com `ABORT_ONLY`, marca de recuperação e a tela de *Gestão de
> transações* dizendo o que passou a valer.
>
> **O que continua não existindo está na §11, com o motivo de cada um.**

---

## 1. A regra que decide tudo, e o que ela mata

> **O `.reg` nunca reaproveita slot excluído.** (`store/src/reg.rs:15`)

Ela não é preferência. É o que faz percorrer o `.reg` do início ao fim devolver
os registros na ordem em que foram digitados, e é a garantia que o dono do
projeto comprou de propósito, sabendo o preço: espaço morto que só volta com
compactação explícita — que, por sua vez, está recusada com número, porque
compactar renumeraria rowid e **rowid é endereço**.

Daí sai a pergunta difícil de qualquer transação aqui:

> Se `BEGIN; INSERT; ROLLBACK` gravou o slot e consumiu o rowid, e o rowid não
> pode ser reusado, **o que é o rollback de um insert?**

Este documento responde: **não é nada, porque o insert ainda não aconteceu.**
A defesa está na §3, e o teste que a trava é o
`o_rollback_de_um_insert_nao_queima_slot`.

---

## 2. O escopo: o que uma transação abrange

| | |
|---|---|
| **Pertence a** | uma **conexão** da porta de dados (`Sessao::ligacao`), não a um usuário e não a uma sessão HTTP |
| **Abrange** | várias operações, várias tabelas, **um** database |
| **Não abrange** | mais de um database; a porta web; o que a réplica aplica |

### 2.1 Por que a conexão, e não o usuário

Pelo motivo já escrito em `server/src/carga.rs`, sobre a reserva do
`BULKINSERT`: *«sem um id de CONEXÃO, a reserva só poderia ser identificada
pelo login — e aí duas janelas do mesmo usuário seriam o mesmo dono, o que é
exatamente o contrário de exclusivo.»* Uma transação tem o mesmo problema, com
o mesmo tamanho de estrago.

O `ExecutorLocal` — a ponte MCP, o job agendado — nasce com `ligacao: 0`, e
zero quer dizer «não há conexão a que amarrar». Por isso **nenhum dos dois abre
transação**, e a recusa diz isso com todas as letras em vez de deixar a
transação viver órfã.

### 2.2 Por que a porta web fica de fora

Também já está decidido no mesmo arquivo: *«HTTP não tem conexão para cair —
cada pedido é um. Sem ligação a que amarrar, a primeira rede de proteção não
existe.»* Uma transação aberta por uma aba de navegador que o usuário fechou
ficaria pendurada segurando tabelas, e a única rede restante seria o prazo.

A web **não fica sem atomicidade** por isso: ela manda a transação inteira em
**um** pedido, que roda inteiro dentro de uma tomada da trava. É o mesmo
caminho que o `inserir_lote` da tela já usa hoje, e é por isso que a tela nunca
precisou de `BEGIN`.

### 2.3 Por que um database só

Porque a marca de recuperação (§5) mora **dentro do diretório do database**, e
é isso que a faz viajar junto no backup e na restauração. Uma marca que
cobrisse dois databases deixaria de valer no instante em que alguém
restaurasse um deles sozinho — e restaurar um database sozinho é uma operação
que já existe (pedido 133).

Transação entre databases é *two-phase commit*, é outro projeto, e entra na
lista do que falta com esse nome. **Recusa fundamentada, não esquecimento** — e
a recusa que o servidor devolve cita o nome, para quem esbarrar saber o que
procurar.

---

## 3. Como desfazer, sem reaproveitar slot

### 3.1 A escolha: nada vai a disco antes do `COMMIT`

Dentro de uma transação, `inserir`, `atualizar` e `excluir` **não tocam em
arquivo nenhum**. Eles entram num **conjunto de escrita em memória** — a lista
ordenada do que a transação quer fazer, na ordem em que foi pedido. O
`COMMIT` aplica a lista inteira numa passada só, com a trava de dados na mão.
O `ROLLBACK` joga a lista fora.

```text
BEGIN            → nasce o conjunto de escrita, vazio
INSERT           → entra na lista; nenhum slot, nenhum rowid, nenhum evento
UPDATE           → entra na lista
ROLLBACK         → a lista é descartada. O disco nunca soube que houve nada
COMMIT           → a lista é aplicada em ordem, numa tomada da trava
```

O rollback de um insert é **zero bytes de trabalho**, e a ordem de digitação
nunca correu risco.

### 3.2 A alternativa que foi recusada, e os quatro motivos

A resposta óbvia — **o slot que «nasceu e morreu»**: gravar o slot, e no
rollback marcá-lo com um terceiro status. Foi recusada por quatro motivos, em
ordem de gravidade:

1. **O terceiro status é corrupção para todo leitor que já existe.**
   `store/src/reg.rs:143` é literal: `b == STATUS_LIVRE || b == STATUS_ATIVO`,
   e qualquer outro valor cai no ramo *«status inválido»*. Um valor novo faria
   o `verificar`, o `reparar` e a comparação com o `.bkp` chamarem de
   corrompida uma linha que está exatamente como deveria.

2. **O status novo é desnecessário: o `.reg` já sabe desfazer um insert.**
   Quando um índice único recusa a chave depois do slot gravado, o `inserir`
   chama `self.reg.excluir(rowid)`. A marca «nasceu e morreu» **já existe**,
   disfarçada de `LIVRE`. O que ela não resolve é o item 3.

3. **Na replicação, o slot queimado tem de ser queimado dos dois lados.**
   O `aplicar_evento` para a replicação quando o rowid que a réplica gerou não
   bate com o do evento. Queimar o mesmo slot na réplica exigiria **mandar para
   ela a inclusão e depois a exclusão** — quer dizer, **a transação revertida
   chegaria aplicada na réplica**, exatamente o que não pode acontecer (§6).

4. **O buraco é permanente.** A compactação está recusada com número. Uma carga
   de 2.500 linhas que falha na linha 1 e é revertida deixaria 2.500 slots
   mortos para sempre.

### 3.3 O que a escolha custa, dito antes de alguém descobrir

| Custo | Tamanho | O que se faz com ele |
|---|---|---|
| **Memória** | o conjunto de escrita inteiro fica em RAM | teto em `recursos.transacao_max_linhas` (padrão 100.000, ~20 MiB). Estourou, a operação é **recusada com erro nomeado** e a transação vai para `ABORT_ONLY` — nunca engolida, nunca vazada para disco pelas costas |
| **Ler o que a própria transação escreveu** | não entrega | uma consulta dentro da transação **não vê** o que ela mesma inseriu. Está declarado na §4.4 e a tela diz |
| **Falha no meio do `COMMIT`** | é a única janela que sobra | é a §5, e é por isso que ela é peça da transação e não detalhe |
| **Buraco na sequência** | o `AUTO_INCREMENT` é consumido ao aplicar | a numeração acontece dentro do `inserir`, na passada — um `ROLLBACK` não queima número nenhum, porque a passada não chegou a acontecer |

O terceiro item merece a mitigação que este desenho tem: **o que pode falhar é
conferido na hora do `INSERT`, não na hora do `COMMIT`.** A conversão da linha,
os gatilhos `BEFORE` e a unicidade rodam ao empilhar — contra o índice **e**
contra as chaves que a própria transação já empilhou. Sobrando só falha de E/S
no `COMMIT`, a recuperação da §5 tem uma resposta única e simples.

### 3.4 O que a transação NÃO empilha, e por quê

`OPS_EMPILHAVEIS` é `inserir`, `atualizar`, `excluir` e `restaurar`. Escrita
que não está nessa lista **é recusada** dentro de uma transação, nomeando o que
a lista tem.

**Ela não confirma a transação aberta por conta própria**, e essa é a decisão.
O MySQL(R) e o Oracle confirmam quando chega um DDL, e é uma armadilha
conhecida: quem escreveu `BEGIN; …; CREATE TABLE; ROLLBACK` acha que desfez e
não desfez. Recusar é a resposta honesta enquanto o DDL não for transacional.

`inserir_lote` fica de fora, e a ausência é deliberada: ele **já é atômico
sozinho** — roda inteiro dentro de uma tomada da trava — e empilhá-lo linha a
linha estouraria o teto de memória com uma carga que não precisava de transação
nenhuma.

Tabela com **partição alfanumérica** (por letra) recusa `INSERT` dentro de
transação. Ali o slot depende do balde em que a linha cai, e o balde sai de uma
regra que mora dentro do `Table`; reproduzi-la fora seria uma segunda
implementação dela, e sem o rowid alvo a marca de recuperação deixa de ser
idempotente. Recusa fundamentada, e não esquecimento.

---

## 4. O isolamento e as travas

### 4.1 Não se pode segurar a trava global entre pedidos

A tentação é grande porque seria serialização de graça: `BEGIN` toma a trava,
`COMMIT` solta. **É a doença que este projeto já mediu.** Em `REPLICACAO.md`
§17, com a trava presa atravessando uma ida e volta de rede numa réplica
cortada em silêncio, `varrer` esperou **29.456 ms** enquanto o `ping` respondia
em 6 ms. Uma transação aberta por um cliente que foi almoçar faria o mesmo, e
não por engano de implementação: por desenho.

**Então a transação NÃO segura a trava global.** Ela a toma e solta operação a
operação, exatamente como hoje — e a espera por uma trava de transação acontece
**fora** da trava de dados, sempre.

### 4.2 O que substitui — e esta seção MUDOU

**A decisão anterior, e ela está registrada porque foi trocada:** o desenho
escrito escolhia **reserva de tabela, sem espera**. Ao tocar uma tabela pela
primeira vez, a transação a reservava inteira; quem esbarrasse recebia a recusa
na hora, sem esperar. O argumento era forte e continua verdadeiro: **sem espera
não há grafo de espera, e sem grafo não há ciclo** — o abraço mortal ficava
impossível por construção, o que é uma resposta mais forte do que detectá-lo
depois de existir.

**Por que ela caiu.** O preço era um conflito **artificial**. Quinhentos caixas
vendendo, um mexendo no pedido 9001/produto 100 e outro no 18223/987: não há
disputa nenhuma de verdade, e a trava de tabela criava uma. Medido, com 64
caixas em linhas diferentes: **50,5 ms com `AUTO` contra 78,4 ms com
`EXCLUSIVE`** — 1,55× de conflito que não existia (`DESEMPENHO.md` §12.2).

**O que entrou no lugar.** Uma hierarquia de duas alturas:

| modo | na tabela | na linha |
|---|---|---|
| `AUTO` (padrão) | intenção (`IX`) | exclusiva |
| `ROW` | intenção (`IX`) | exclusiva |
| `TABLE` | exclusiva (`X`) | não precisa |
| `EXCLUSIVE` | exclusiva (`X`) | não precisa |

Duas intenções convivem; a exclusiva não convive com nada. Duas transações
mexendo em linhas diferentes da mesma tabela **não se veem**.

**O que paga pela volta da espera: a declaração prévia do escopo.** Travar por
linha traz a espera de volta, e com ela a possibilidade de ciclo. Com as
tabelas conhecidas na abertura, o gestor adquire **sempre na mesma ordem
canônica**, e o ciclo clássico — A pega `pedidos` e quer `estoque` enquanto B
pega `estoque` e quer `pedidos` — deixa de existir.

> **A ordem canônica aqui é o nome qualificado em caixa baixa.** O que a ordem
> exige é uma ordem total *estável* sobre as tabelas; qualquer uma serve, desde
> que todas as transações usem a mesma. Um id numérico interno teria de ser
> inventado, gravado e mantido estável entre restaurações de backup — e seria a
> segunda verdade sobre a mesma tabela.

**E a garantia não é total. Isto tem de ficar dito, e não vendido.**

* A ordenação mata o ciclo entre **tabelas**.
* Ciclo entre **linhas da mesma tabela** continua possível: A trava a linha 5 e
  depois quer a 9, B trava a 9 e depois quer a 5.
* Para esse caso a resposta não é prevenir, é **limitar**: o `LOCK TIMEOUT`
  transforma a espera num erro nomeado com o número, e nunca numa thread
  pendurada.
* E a **expansão dinâmica** (§4.5) reintroduz a possibilidade de ciclo entre
  tabelas, porque a ordem canônica só vale para o que foi declarado na
  abertura. Quem quer a garantia declara o escopo inteiro.

**Este motor não promete «sem deadlock».** Ele promete ordem canônica entre
tabelas declaradas e espera limitada em todo o resto.

O `INSERT` trava o **fim da tabela**, e não uma linha: o próximo slot é
`slots() + 1` e ele é um só. Duas transações que anexam ao mesmo tempo preveem
o mesmo rowid, e sem essa trava a segunda descobriria isso na passada de
commit, com metade do trabalho gravado.

> **A ordem em que as travas são tomadas foi corrigida por uma corrida que a
> revisão achou, e ela merece estar escrita.** A primeira versão calculava o
> rowid previsto com a trava de dados na mão e só pedia a trava do fim
> **depois** de soltá-la. Nessa fresta uma escrita comum de outra conexão podia
> anexar. O `COMMIT` até descobriria a divergência — e esse erro é visível. O
> estrago vinha depois: a **recuperação** encontraria o slot ocupado pela linha
> do outro, o trataria como «já aplicado», e descartaria a nossa **em
> silêncio**.
>
> Hoje as travas são tomadas **antes** da trava de dados. O que elas precisam
> saber sai do pedido — a tabela e o rowid alvo —, então cabem ali, e com o fim
> travado o `slots()` não se move mais debaixo do cálculo. O teste é o
> `escrita_comum_nao_anexa_enquanto_a_transacao_segura_o_fim`.

As redes contra trava órfã são as três de sempre, e nenhuma delas basta
sozinha: a queda da conexão solta, o `TIMEOUT` solta, e o `COMMIT`/`ROLLBACK`
solta.

### 4.3 Uma escrita comum não espera

Quem escreve **sem** `BEGIN` respeita a trava de quem tem, mas **não espera**:
recebe `4005 EM_TRANSACAO` com `repetir: true`, na hora. Um pedido solto não
declarou `LOCK TIMEOUT` nenhum, e inventar uma espera para ele mudaria o tempo
de resposta de todo cliente que já existe.

### 4.4 O nível, dito sem enfeite

| | |
|---|---|
| **Entre escritores** | **serializável por linha** nas tabelas da transação, com a tabela inteira quando o modo é `TABLE`/`EXCLUSIVE` |
| **Para quem lê** | **read committed**, e sem bloquear: um leitor nunca vê dado não confirmado, porque **não há dado não confirmado em lugar nenhum** — ele ainda está em RAM |
| **Para a própria transação** | **nada.** Ela não tem *snapshot*: entre duas leituras dela, outra transação pode ter confirmado. E ela não vê as próprias escritas |

**Não é ANSI SERIALIZABLE**, e não vai ser chamado assim. O nome que o servidor
devolve em `transaction_isolation` é o que ele é:

> *escrita serializável por tabela, leitura confirmada e não bloqueante, sem
> leitura repetível.*

E fica registrado o que este desenho **não** compra: paralelismo. A trava única
continua serializando toda operação, uma de cada vez. Transação e concorrência
fina são frentes diferentes; confundi-las é o que faz uma prometer a outra.

### 4.5 O escopo declarado: `SCOPE`, e os dois modos

```sql
BEGIN TRANSACTION
  SCOPE (clientes, pedidos, pediditens, estoque)
  SCOPE MODE STRICT
  TIMEOUT 5s
  LOCK TIMEOUT 500ms
  STATEMENT TIMEOUT 2s
  LOCK MODE AUTO;
```

O mesmo pela porta de dados, que é por onde os clientes falam:

```json
{"op":"begin","database":"loja",
 "scope":["clientes","pedidos","pediditens","estoque"],
 "scope_mode":"strict","lock_mode":"auto",
 "timeout":"5s","lock_timeout":"500ms","statement_timeout":"2s"}
```

**Parâmetros nomeados, e não posicionais.** A forma posicional —
`Transaction(clientes, pedidos, estoque, 5s)` — não estende: entrou o segundo
prazo, não há onde ele caiba sem quebrar quem já escreveu, e não há como dizer
*qual* dos três prazos é aquele. E ela mistura tabela com duração na mesma
lista, onde a quarta posição só não é uma tabela porque termina em `s`.

**As cláusulas não têm ordem.** Ordem obrigatória é uma regra que existe para
facilitar o analisador, e o preço dela é pago por quem digita.

| modo de escopo | o que faz com tabela não declarada |
|---|---|
| `DYNAMIC` (**padrão**) | acolhe, toma a trava na hora e **anota a expansão**, que aparece na ficha |
| `STRICT` | **recusa**, nomeando a tabela e o escopo |

`STRICT` é melhor para ERP e financeiro, e é o que o desenho preferia. **Ele
não pode ser o padrão**: `STRICT` por omissão recusaria toda escrita de todo
cliente que nunca declarou escopo, e isso é exatamente a regra pétrea da casa —
*guarda nova entra pedida, não imposta*. **Sem `SCOPE` nenhum, nada muda**, e
esse é o teste que mais importa (`sem_transacao_nada_muda`).

### 4.6 Escopo efetivo: o declarado mais o que o catálogo alcança

> Escopo declarado + dependências do catálogo = **escopo efetivo**

Ele é calculado na abertura e aparece **separado** do declarado na ficha —
`tabelas_declaradas`, `tabelas_efetivas` e `tabelas_expandidas`. Juntar as
listas numa só esconderia exatamente a informação pela qual a separação existe:
quais tabelas entraram sem ninguém pedir, e por onde.

**O gatilho entra, e alcança de verdade.** O corpo de um gatilho grava noutra
tabela com `INSERT INTO`, e o `rodar_gatilhos_depois` executa isso. Os alvos
saem da **árvore já compilada** do corpo, e não do texto: procurar `INSERT
INTO` por comparação de texto quebra calado no dia em que alguém escrever o
mesmo comando com outro espaçamento — a mesma armadilha de resolver texto de
tela comparando a frase. O fecho é transitivo, com teto de voltas, porque um
gatilho que grava na própria tabela é legítimo.

**A chave estrangeira NÃO entra, e isso foi conferido em vez de suposto.**

A tentação era somar as tabelas apontadas por FK com `ao_excluir`/`ao_alterar`
em cascata. Elas não alcançam nada: **o motor declara a chave estrangeira e não
a impõe.** Há teste travando isso pelo nome —
`a_chave_e_declarada_mas_ainda_nao_e_imposta_na_gravacao` — e o comentário dele
diz que uma linha filha apontando para um pai que não existe entra sem
reclamação. Sem imposição não há cascata, e sem cascata a FK não toca tabela
nenhuma.

Somá-las travaria tabelas que a transação nunca vai tocar, e a ficha mostraria
um alcance que não existe — que é exatamente a linha que não se imprime porque
não se mede. **No dia em que aquele teste falhar**, o `escopo_efetivo` é o
lugar de acrescentar o braço da FK, e o comentário lá diz isso.

### 4.7 Os três prazos, e quem encerra

| prazo | o que limita | padrão |
|---|---|---|
| `TIMEOUT` | a transação **inteira** | `recursos.transacao_prazo_min`, 5 min |
| `LOCK TIMEOUT` | quanto se aceita **esperar por outro** | `recursos.transacao_lock_timeout_ms`, 500 ms |
| `STATEMENT TIMEOUT` | quanto **uma operação** pode levar | `recursos.transacao_statement_ms`, 0 = sem prazo |

São problemas diferentes, e um número só não responde aos três: uma transação
pode ser curta e mesmo assim esperar demais por uma trava, e uma operação pode
demorar sem que nem a transação nem a espera tenham estourado.

**Quem encerra é o gestor de transações, nunca uma thread morta.** Matar a
thread deixaria a trava de dados presa, o conjunto de escrita órfão e o contador
de transações abertas errado. Estourado o prazo, a transação vai para
`ABORT_ONLY`, **solta as travas na hora**, joga a lista fora, e a próxima
operação recebe `6002 TRANSACAO_ABORTADA` com o número do prazo dentro. A
conexão continua viva, e o cliente descobre pelo erro — que é o que ele sabe
tratar.

**Onde o `STATEMENT TIMEOUT` morde, dito em vez de escondido.** Ele usa a
máquina de cancelamento cooperativo que já existe — a mesma do
`telemetria_encerrar` —, e por isso é conferido nos **pontos de cancelamento
que existem**: o `Atividade::siga`, chamado entre duas unidades de trabalho
seguras pelos laços longos (a conversão de uma carga, a exportação). Uma
inserção de **uma** linha não tem ponto de cancelamento no meio, e não poderia
ter: parar entre gravar o slot e manter o índice deixaria os dois discordando.
Um prazo que só morde onde há laço é um prazo honesto; um campo que promete
cortar qualquer coisa seria configuração que mente.

### 4.8 Otimista e pessimista: nenhum dos dois é o certo sempre

A **janela de conflito de escrita** (pedido 123) já existia: o cliente manda
`"versao"` e o servidor recusa se a linha mudou. É controle **otimista**, e
resolve escrita-contra-escrita sem travar nada. A trava de linha é
**pessimista**, e resolve o mesmo caso travando.

Um não substitui o outro, e a escolha é de quem escreve o cliente:

| | otimista (`versao`) | pessimista (trava de linha) |
|---|---|---|
| bloqueia? | nunca | sim, até o `LOCK TIMEOUT` |
| disputa baixa | ganha: zero espera, zero trava | paga a trava sem precisar |
| disputa alta na MESMA linha | vira tentativa e erro | uma tentativa por cliente, sempre |
| custo de uma perda | reler e regravar | esperar |

Medido com 64 clientes na **mesma** linha (`DESEMPENHO.md` §12.3): o otimista
levou **28,0 ms gastando 133 tentativas** para 64 gravações; o pessimista levou
**71,8 ms gastando exatamente 64**. Nesse ponto o otimista ainda ganha em
tempo, e já gasta 2,1× mais tentativas — é a subida dessa razão, e não o
relógio, que diz quando trocar.

---

## 5. Se o processo morrer no meio

### 5.1 A marca: `transacao_<id>.tx`, no diretório do database

Antes de a passada de `COMMIT` tocar em qualquer arquivo, o conjunto de
escrita inteiro é gravado num arquivo próprio e **sincronizado**. O formato
está em `docs/FORMATO.md`.

O `rowid alvo` é conhecido **antes** da passada: o `.reg` sempre anexa no fim,
e o próximo rowid é `slots() + 1` mais quantas inserções a transação já
empilhou para aquela tabela. Com o fim da tabela travado, ninguém pode mudar
isso no meio. É essa previsibilidade que torna a recuperação exata.

A ordem é a mesma que a lixeira já usa e pelo mesmo motivo
(`store/src/lixeira.rs`): grava e **sincroniza** a intenção antes de mexer no
alvo, porque *«a ordem inversa tem uma janela em que o registro não existe em
lugar nenhum, e essa janela não tem conserto depois.»*

**A linha vai em bytes, e não em JSON, e isso foi medido no próprio código:**
`valor_para_json` escreve `Time` e `DateTime` como texto ISO e
`json_para_valor` desses dois só aceita número. A volta não fecha, e uma
recuperação que reconstrói a linha errada é pior do que uma que não reconstrói
nada. A codificação própria tem uma etiqueta por variante, e o teste
`a_linha_volta_igual_nas_catorze_variantes` fecha o laço.

### 5.2 A recuperação anda para a frente, nunca para trás

Ao abrir um database, um `.tx` órfão significa: **alguém morreu no meio de um
commit**. A recuperação **completa o commit** — reaplica as operações que
faltam, e apaga o `.tx`.

Nunca desfaz. Não é escolha estética: desfazer exigiria devolver slots já
gravados, que é a regra da §1. Andar para a frente é a única direção que o
formato permite, e o `.tx` é o que torna isso possível — sem ele, não se sabe
para onde ir.

A reaplicação é **idempotente pelo rowid**: cada operação diz o slot que devia
ter escrito e o conteúdo. Slot já ativo — passa adiante. Slot livre e no fim —
grava. É por isso que o `.tx` guarda o rowid alvo, e não só a linha.

Ela roda no arranque, **antes de o servidor abrir a porta**: deixar isso para o
primeiro pedido responderia «depende de quem chegar primeiro» a uma pergunta
que precisa ser inequívoca.

### 5.3 O relatório, e só o que ele mede

```text
PHXSQL Recovery -- base /var/phxsql
  transacoes achadas ............ 2
  marcas ilegiveis descartadas .. 1   (commit que nunca comecou)
  transacoes completadas ........ 1
  operacoes reaplicadas ......... 37
  operacoes ja aplicadas ........ 12
  tempo ......................... 8 ms
```

O relatório do capítulo tinha uma linha de **páginas refeitas**. Ela não existe
aqui e não vai ser inventada: não há página suja confirmada para refazer, e uma
linha que imprime zero para sempre é pior do que linha nenhuma. Linha
`operacoes IMPOSSIVEIS` só aparece quando há alguma — ver a §5.5.

E ele **não sai quando não há marca nenhuma**, que é o arranque de sempre: um
bloco dizendo zero em toda subida treina quem opera a não ler o relatório.

### 5.4 O contrato, respondido ponto a ponto

> «Se o computador perder energia exatamente nesta instrução, depois de
> reiniciar o banco conseguirá determinar de forma inequívoca se esta transação
> foi COMMITTED ou ABORTED?»

| onde a energia cai | a resposta |
|---|---|
| antes de a marca estar sincronizada | **ABORTED.** Não há marca e nenhum byte de dado foi tocado |
| durante o `fsync` da marca | **ABORTED.** Ela fica truncada, o CRC não confere, e marca que não confere é commit que nunca começou |
| depois da marca, no meio da passada | **COMMITTED.** A marca está inteira no disco e a recuperação completa o que falta |
| depois da passada, antes de a janela de durabilidade fechar | **COMMITTED.** A marca ainda está lá de propósito (§8), e a recuperação reaplica |
| depois de a marca ser apagada | **COMMITTED**, e sem trabalho nenhum |

**O `fsync` da marca é o ponto de compromisso.** Antes dele a transação não
aconteceu; depois dele ela aconteceu, mesmo que o arquivo de dado ainda não
saiba.

### 5.5 O que continua sem cobertura, dito

* Uma queda **entre** a última operação da passada e o `unlink` do `.tx` faz a
  recuperação reaplicar um commit que já estava inteiro — e ela encontra todos
  os slots já certos e não faz nada. Custa uma varredura. É seguro.

* **O caso sem conserto**, e ele é um só: se a passada gravou o slot e depois o
  liberou (o `inserir` desfazendo a si mesmo por falha de E/S no índice), o
  slot fica dentro da faixa e **livre**. O `.reg` não reaproveita slot, então
  aquela linha não volta para o lugar dela. A recuperação **não esconde isso**:
  a operação entra em `operacoes IMPOSSIVEIS` com a tabela e o rowid, e o
  arranque imprime. Perder a linha em silêncio seria pior do que dizer que ela
  se perdeu.

* Uma marca cuja **tabela não abre mais** (alguém apagou a tabela entre a queda
  e o arranque) cai no mesmo lugar, pelo mesmo motivo.

### 5.6 A lição que o próprio teste do `SIGKILL` deu

A primeira versão da prova por soquete exigia **sempre** as 3.000 linhas depois
do `SIGKILL`, e ela era instável **por construção** — o que a fez falhar uma
vez em quatro. Matar o processo no instante certo é uma corrida, e **os dois
desfechos são legítimos**: o sinal pode cair antes de o `fsync` da marca
terminar, e aí ela fica truncada, o CRC não confere, e isso é um commit que
nunca começou.

O erro não estava no motor: estava na pergunta que o teste fazia. A pergunta do
contrato **não é** «as 3.000 estão lá?» — é «o banco consegue determinar de
forma inequívoca?». Então o que se exige é **nunca metade**: ou 3.000, ou
nenhuma, e o relatório diz qual das duas sem o teste ter de adivinhar.

É a mesma família do teste que passa por engano, pelo outro lado: um teste que
**falha** por engano treina quem o vê a repetir a rodada até ficar verde — e aí
ele deixa de valer para os dois lados.

---

## 6. A replicação: uma transação revertida não chega aplicada

### 6.1 O desenho não muda o `.log` — e essa é a resposta

A regra desta frente é dura: *«réplica que não conhece a versão nova continua
aplicando»*. Este desenho a cumpre da forma mais forte possível: **não existe
versão nova.** O `.log` não ganha campo, não ganha flag e não ganha operação.

* **Uma operação nova no `.log` quebraria toda réplica antiga.**
  `Operacao::de_tag` devolve `Corrompido` para qualquer tag que não seja 1, 2
  ou 3. Uma tag `BEGIN` não seria ignorada — ela **pararia a replicação**.
* **Um identificador de transação não cabe no cabeçalho.** Os 44 bytes estão
  cheios, e os «reservados» já foram gastos pela `origem`.
* **No corpo também não cabe:** ele é a imagem da linha, e um prefixo ali seria
  lido como coluna por toda réplica antiga.

### 6.2 Por que não é preciso mexer nele

Porque **a transação aberta não produz evento nenhum.** Nada foi gravado, logo
nada foi journalizado, logo não há o que servir.

E o `COMMIT` produz os eventos na ordem, de uma vez, dentro de uma tomada da
trava — indistinguíveis de um `inserir_lote` de hoje para quem os aplica. Uma
réplica de qualquer versão, inclusive uma anterior a esta rodada, aplica sem
saber que houve transação.

**Uma transação revertida não chega aplicada na réplica porque ela não chega,
ponto.**

---

## 7. O custo para quem NÃO usa transação

**A regra:** se acrescentar algo mensurável ao caminho de quem nunca abre uma
transação, o desenho está errado — e volta para a mesa.

O único acréscimo no caminho comum é **um portão, e ele vem antes de qualquer
trabalho**: um `AtomicUsize` com o número de transações abertas, lido com
`load(Relaxed)`. Zero transações abertas — que é o servidor inteiro hoje — e
nenhuma estrutura de transação é consultada, nenhum `Mutex` é tomado, nenhuma
`String` é montada, nenhum campo do pedido é lido de novo.

Isto é literalmente a lição que o Profiler cobrou: *«o portão que decide isso
vem ANTES do trabalho»*.

E o teste que mais importa não é o do recurso novo: é o
**`sem_transacao_nada_muda`** — quem nunca manda `BEGIN` vê exatamente o
comportamento de hoje, inclusive a mensagem literal do `inserir_lote` sobre as
linhas gravadas antes do erro.

---

## 8. O group commit: medido, aceito, e o passo seguinte morto

Chegou de fora, e a regra da casa mandou medir antes de virar plano. O critério
de morte foi acordado **antes** da medição: **abaixo de 1,5× a hipótese morre.**

A decomposição de um commit de uma linha (`--example custo-da-transacao`,
mediana de 5 rodadas intercaladas):

| pedaço | ms | o que é |
|---|---|---|
| commit inteiro, `fsync` por commit | 1,199 | o comportamento anterior |
| só a marca `.tx` | 0,289 | o ponto de compromisso |
| a linha, com o `fsync` amortizado | 0,050 | o trabalho de verdade |
| **o resto** | **0,860** | o `fsync` da tabela, por commit |

Uma inserção **solta**, sem transação, custa 0,061 ms — justamente porque ela
passa pela **janela de durabilidade** que já existia.

**A conclusão, e ela é o oposto da receita:** o group commit clássico amortiza
`fsync` entre commits **concorrentes**, e este servidor não tem commits
concorrentes — a trava única serializa tudo, e nunca há dois em voo para
agrupar. O que havia para amortizar era outra coisa: o `fsync` da tabela, que a
transação estava forçando por commit enquanto o resto do servidor já usava a
janela.

**O conserto foi deixar o commit usar a janela que já existe**, e ele é seguro
por um motivo que só o `.tx` dá: **quem decide se a transação aconteceu é a
marca, não o `fsync` da tabela.** A marca já está sincronizada quando a passada
começa, então adiar o `fsync` não adia a decisão — adia só o momento em que o
dado alcança o disco, e a marca é o bilhete que o traz de volta.

**A ordem é a peça, e ela não se inverte:** a marca só é apagada **depois** de
a tabela sincronizar. Apagar antes abriria a janela em que o dado não está no
disco e não há bilhete nenhum para recuperá-lo.

**Ganho medido: 2,63×** (1,199 → 0,457 ms por commit de uma linha), no mesmo
binário e na mesma rodada.

**E o passo seguinte morre medido.** Agrupar os `fsync` das *marcas* entre
commits é a única coisa que sobra, e o piso irredutível (marca + trabalho) é
0,341 ms contra os 0,455 de hoje: **1,34×**, abaixo do critério de 1,5×. A
marca não se adia — ela é o ponto de compromisso. **Recusa registrada com o
número**, para a ideia não voltar sem medição.

---

## 9. As classes de erro

O §29 do capítulo, e a aplicação precisa da distinção porque a ação dela é
outra em cada caso:

| classe | exemplos | o que acontece |
|---|---|---|
| **instrução** | chave duplicada, tipo errado, rowid inexistente, `SIGNAL` de gatilho, acesso negado | a instrução é cancelada, a transação **continua `ACTIVE`**, corrigir e repetir funciona |
| **transação** | teto de linhas estourado, E/S no meio da passada, prazo estourado, formato corrompido | vai para **`ABORT_ONLY`**; só o `ROLLBACK` passa |
| **queda da conexão** | o soquete caiu | desfeita sozinha, sem ninguém para avisar |

A classe **sai da faixa do código de erro**, e não de uma lista escrita à mão:
esquema (2xxx), dado (3xxx) e acesso (4xxx) cancelam a instrução; formato
(1xxx), sistema (5xxx) e execução (6xxx) derrubam a transação. Erro novo cai na
classe certa sozinho, e as duas não têm como divergir — é a mesma decisão do
`PhxError::classe`.

Em `ABORT_ONLY`, **até a leitura recusa**, como no PostgreSQL(R): a transação
está suja e não serve para mais nada. E `ROLLBACK TO SAVEPOINT` **não a
resgata** — a diferença para o PostgreSQL(R) é deliberada: lá *todo* erro
aborta a transação, e o `SAVEPOINT` existe justamente para resgatar quem errou
uma instrução. Aqui erro de instrução não aborta nada, então só chega a
`ABORT_ONLY` o que põe em dúvida o próprio conjunto de escrita. Voltar a um
ponto não desfaz essa dúvida.

Dois códigos novos, e eles não mudam nunca:

| código | nome | classe | repetir? |
|---|---|---|---|
| 4005 | `EM_TRANSACAO` | acesso | **sim** — quem segura vai soltar |
| 6002 | `TRANSACAO_ABORTADA` | execução | não — o pedido não é o problema |

---

## 10. `SAVEPOINT`: quase de graça, e por quê

Não se copia a transação: guarda-se um **índice na lista** de escrita
empilhada. `ROLLBACK TO SAVEPOINT` trunca o `Vec` naquele ponto e a transação
**continua aberta**.

Num motor que já gravou as escritas, voltar a um ponto exige desfazer páginas.
Aqui o conjunto de escrita está em RAM, e é isso que torna a operação barata —
**a ideia é do capítulo que o dono mandou, e foi ela que tornou o `SAVEPOINT`
possível nesta rodada em vez de na seguinte.**

O que o truncamento precisa levar junto: **as chaves únicas já empilhadas**.
Sem refazê-las, a chave de uma linha descartada continuaria barrando a próxima
igual a ela, e o `SAVEPOINT` deixaria de desfazer de verdade.

Nome repetido **destrói** o ponto anterior e cria um novo, que é o que o SQL
manda. `RELEASE SAVEPOINT` tira o ponto e os criados depois dele, **sem tocar
no trabalho**.

---

## 11. O que NÃO entrou, e o motivo de cada um

Esta seção existe para as ideias não voltarem sem medição.

### 11.1 MVCC — não implementar

**Aqui o rowid é o endereço**, e é o que dá o O(1). Uma segunda versão da linha
pede um segundo slot, logo um segundo rowid — e isso quebra **duas** coisas ao
mesmo tempo: a regra pétrea da ordem de digitação, e a replicação, cujo
`aplicar_evento` **para** quando o rowid diverge do que o source mandou.

Não é falta de vontade nem de tempo: é incompatível com o formato. A decisão
sobre rowid-como-endereço é do dono do projeto e não voltou.

**A boa notícia, e ela é metade do que se quer do MVCC:** *readers
non-blocking* **este desenho já entrega**, por outro caminho e sem MVCC nenhum.
Como nada vai a disco antes do `COMMIT`, um leitor concorrente nunca vê escrita
não confirmada e **nunca espera por escritor**. O que continua faltando — e que
só o MVCC daria — é **leitura repetível** ao longo de um leitor longo, e **ler o
que a própria transação escreveu**.

### 11.2 WAL, undo log, PageLSN, full-page-write, VACUUM

Todos existem para o problema que o desenho de «nada a disco antes do COMMIT»
**não tem**: não há página suja confirmada para refazer, nem versão velha para
limpar. O `.tx` é a intenção inteira, e ele é apagado quando deixa de valer.

**O full-page-write merece a conferência que ele pede, e ela foi feita:** o
`.reg` guarda slots de tamanho fixo **com CRC-32**, então uma escrita rasgada é
**detectável, e não silenciosa** — o leitor recusa o slot em vez de devolver
metade velha e metade nova.

Um slot **pode** cruzar fronteira de setor: o tamanho dele sai do esquema e não
é alinhado a 512 nem a 4096 bytes. O que isso significa aqui: uma escrita
rasgada na fronteira corrompe o slot, o CRC acusa, e a linha é recuperável pelo
espelho `.bkp` quando ele está ligado. O que o full-page-write compraria é
escrever a página inteira no journal antes de tocá-la — e para isso seria
preciso um journal de páginas, que é justamente o WAL que este desenho não tem
e não precisa. **Não entra**, e o motivo é que a detecção já existe e o reparo
já existe; o que falta seria uma terceira cópia para um caso que o espelho
cobre.

### 11.3 Detecção de deadlock

O grafo de espera existe agora que há espera (§4.2), então a resposta honesta
não é «impossível» — é **prevenir onde dá e limitar onde não dá**:

* entre **tabelas declaradas**: a ordem canônica **impede** o ciclo, o que é
  mais forte que detectá-lo;
* entre **linhas** e sob **expansão dinâmica**: o ciclo é possível, e o `LOCK
  TIMEOUT` o transforma num erro nomeado com o número.

Um detector de ciclo entregaria: (a) matar a vítima mais barata em vez de a que
esperou mais, e (b) um erro mais cedo. O preço é um grafo mantido em toda
aquisição de trava e uma varredura a cada espera. **Não entra nesta rodada**, e
o que o faria entrar é uma medição mostrando espera de `LOCK TIMEOUT` cheio
acontecendo em produção.

### 11.4 DDL transacional

O `ALTER TABLE ADD COLUMN` que acabou de entrar **já tem duas fases e ponto de
compromisso**: escreve todos os `*.novo`, sincroniza, e só então troca com
`rename`, volume 1 primeiro. Isso é atomicidade de **uma** operação de DDL, e é
mais do que a maioria dos motores de arquivo entrega.

O que falta para ser DDL **dentro de transação**:

1. o conjunto de escrita teria de guardar operações de **estrutura**, não só de
   linha — e a marca `.tx` teria de saber descrevê-las;
2. a recuperação teria de saber completar um `rename` pela metade, o que é
   outro tipo de idempotência;
3. e o pior: um `ALTER` empilhado mudaria o esquema **debaixo** das escritas de
   linha já empilhadas na mesma transação, que foram convertidas contra o
   esquema antigo.

**Não implementado nesta rodada**, e a recusa é a da §3.4: o DDL dentro de
transação é **recusado**, e não silenciosamente confirmado.

### 11.5 Transação entre databases

*Two-phase commit.* Recusa fundamentada, §2.3.

---

## 12. *ACID compliant*: o que passou a valer, e o que não

A folha de marca afirma *ACID compliant*, e o `CLAUDE.md` dizia que era falso
porque sem transação não há o **A** nem o **I**. Esta rodada muda parte disso, e
a única resposta útil é a precisa:

| letra | estado | com precisão |
|---|---|---|
| **A** — atomicidade | **entregue** | o conjunto de escrita é aplicado inteiro ou não é aplicado; o `ROLLBACK` não deixa slot, rowid nem evento; uma queda no meio da passada é completada pela marca |
| **I** — isolamento | **entregue, com o nome certo** | *escrita serializável por tabela, leitura confirmada e não bloqueante, sem leitura repetível.* **Não é ANSI SERIALIZABLE** e não pode ser chamado assim: não há leitura repetível, e a transação não vê as próprias escritas |
| **C** — consistência | **parcial, e a parte que falta tem nome** | tipo, unicidade e gatilhos são conferidos ao empilhar. **A integridade referencial continua não sendo imposta** (§4.6) — a chave estrangeira é catálogo. Enquanto isso valer, o **C** não está inteiro |
| **D** — durabilidade | **entregue, e configurável** | a marca `.tx` é sincronizada antes da passada e é o ponto de compromisso; uma queda depois dela é completada no arranque. Com `durabilidade: sistema` quem abre mão é quem configurou, e está escrito |

**Então: continua sendo errado escrever *ACID compliant* sem qualificação.** O
que se pode escrever, e é verdade: *atomicidade e durabilidade entregues,
isolamento entregue no nível declarado acima, consistência dependente da
integridade referencial que o motor ainda não impõe.*

---

## 13. O pré-requisito, que entrou na rodada anterior

A transação mora em cima da trava de dados, então a trava precisava ter **um**
dono antes de a transação existir. As 13 tomadas fora do `travar_dados()`
entraram para dentro dele, a reentrância deixou de pendurar o servidor e virou
erro nomeado, e o teste `so_um_lugar_toma_a_trava` conta as tomadas no próprio
fonte. O relato inteiro está no histórico deste documento e em
`DESEMPENHO.md` §9.

---

## 14. Como se prova

| o que | onde |
|---|---|
| o formato da marca, a ida e volta das 14 variantes, o CRC, o truncamento | `transacao.rs`, testes do módulo |
| a matriz de travas, a ordem canônica, os dois caixas | `travas.rs`, testes do módulo |
| o protocolo inteiro: `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`, escopo, prazos, classes de erro, recuperação | `servidor.rs`, `testes_transacoes` |
| o SQL: os três sinônimos, as cláusulas sem ordem, `500ms` que não vira `500s` | `phxsql-sql/src/transacao.rs` |
| **pelo soquete**, com `SIGKILL` no meio de um `COMMIT` | `bancada/transacoes/provar.py` |
| que cada teste ainda pega o defeito que o motivou | `bancada/guardas/catalogo.py` |
| os números | `--example custo-da-transacao`, e `DESEMPENHO.md` §12 |
