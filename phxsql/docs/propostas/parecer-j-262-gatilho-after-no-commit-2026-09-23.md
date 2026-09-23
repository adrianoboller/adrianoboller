# Parecer J — pedido 262: o gatilho `AFTER` que grava dentro do `COMMIT`

**Papel J (pesquisador), 23/09/2026.** Pedido **262** do `docs/PENDENCIAS.md`
(linha 286). Ciclo da pétrea do dono de 23/09/2026: hipóteses escritas antes,
busca no help e no fonte dos quatro, régua aplicada com a soma escrita, decisão
registrada — inclusive a que morreu.

**O que este parecer acrescenta ao vizinho.** Já existe
`docs/propostas/semantica-gatilho-after-no-commit-2026-09.md` (16/09/2026), e
ele está certo no essencial. A diferença é o método e três números novos: aquele
documento mediu os quatro motores **por leitura**; este **exercitou três deles
nesta máquina** — PostgreSQL 16.13, MySQL 8.0.46 e SQLite 3.45.1 —, remediu o
defeito contra o HEAD de hoje (`520a0cb`), e mediu o bloqueio da saída (a) em
**quatro** colunas de sistema, não duas. Ele também matou uma hipótese nova que
o vizinho não tinha levantado (§6.2).

---

## 0. O resumo

| | |
|---|---|
| **O defeito** | continua vivo no HEAD `520a0cb`, medido pelo soquete hoje (§3) |
| **A premissa do pedido sobre os motores** | **morta**: os quatro convergem, não divergem. Soma **10 a 0** (§5) |
| **A premissa do pedido sobre a causa** | **alvo certo, causa incompleta** — `Confirmando` é o desvio, não a perda (§2) |
| **A régua que decidiu** | convergência dos três maduros → **aceite automático**. A ponderada não chegou a ser usada, por não haver dois lados |
| **Veredito** | **é DEFEITO**, não contrato. Contrato para o papel B em §7 |
| **Sobe ao dono?** | **Não.** Nada aqui é choque com pétrea, empate ou produto — e o porquê está escrito em §8 |

---

## 1. As hipóteses, escritas ANTES de medir

Registradas em `/tmp/.../scratchpad/hipoteses-262.md` antes da primeira corrida.
As três primeiras são as do próprio pedido; as quatro seguintes são minhas.

| # | hipótese | destino |
|---|---|---|
| **H0** | a causa é o estado `Confirmando`: sair dele resolveria | **morreu em parte** (§2) |
| **H0'** | a causa não é o estado, é a **ordem**: a lista já saiu (`mem::take`) e a marca já foi gravada antes dos `AFTER` | **venceu** (§2) |
| **H1** | (i do pedido) a sessão sai de `Confirmando` antes dos `AFTER` | **ambígua, e é por isso que morre** (§6.1) |
| **H2** | (ii do pedido) o gatilho escreve **fora** da transação que o disparou | **RECUSADA, 0 de 4 motores** (§5.3) |
| **H3** | (iii do pedido) é contrato a documentar, não defeito | **meia-verdade** (§6.3) |
| **H4** | os `AFTER` correm **antes** do ponto de compromisso, em laço, e o conjunto fecha depois deles | **venceu — e é o desenho do PG, medido** (§4.1) |
| **H5** | o conserto mínimo é **recusar** em vez de engolir | **venceu, e o canal já está provado** (§3, R5) |
| **H6** | o `rowtime` pai/filho no mesmo commit quebraria a pétrea do dono | **MORREU medida** (§6.2) |

Previsões escritas junto, sobre os motores: P1 SQLite dentro da transação; P2
MySQL idem; P3 PG idem e único com gatilho de commit-time; P4 MariaDB igual ao
MySQL, e por leitura por não haver servidor aqui. **As quatro se confirmaram.**

---

## 2. A premissa do pedido, medida antes do item

O pedido escreve a causa assim:

> *«a sessão ainda está em `Confirmando` — então o `inserir` da auditoria cai no
> `empilhar` de uma transação que é descartada logo a seguir»*

**Metade disso é verdade, e é a metade que não conserta.** O caminho no HEAD
`520a0cb`, arquivo e linha:

| passo | âncora (texto, em `crates/phxsql-server/src/servidor.rs`) | linha às 23:50 | o que acontece |
|---|---|---|---|
| 1 | `tx.estado = crate::transacao::Estado::Confirmando` | 15722 | a sessão entra em `Confirmando` |
| 2 | `std::mem::take(&mut tx.escritas)` | 15726 | **a lista sai; a transação fica viva e VAZIA** |
| 3 | `let quantas = escritas.len()` | 15729 | o `gravadas` da resposta já está decidido aqui |
| 4 | `crate::transacao::gravar_marca(&dir` | 15750 | o **ponto de compromisso**, com `fsync` |
| 5 | `let resultado = self.aplicar_conjunto` | 15762 | a passada |
| 6 | `avisos.extend(self.rodar_gatilhos_depois` | 15780 | os `AFTER` correm aqui, já **depois** do passo 4, e sem a trava |
| 7 | `fn dentro_da_transacao` / `if OPS_EMPILHAVEIS.contains(&op)` | 13975 / 14014 | o `inserir` do corpo só é barrado em `AbortOnly` e prazo vencido: **`Confirmando` passa** |
| 8 | `tx.escritas.push(escrita)` | 15124 | entra na lista **que já foi esvaziada no passo 2** |
| 9 | `self.descartar_transacao(sessao.ligacao);` → `Ok(mut t) => t.tirar(ligacao)` | 15799 → 15613 | a lista vai ao chão |

> **Aviso sobre os números desta coluna, e ele é medido.** Outra frente está
> editando o `servidor.rs` **ao vivo** nesta rodada. Entre dois `grep` meus,
> separados por cerca de vinte minutos, todo o `op_commit` **andou 97 linhas**
> (o `Confirmando` saiu de 15625 para 15722). **Ancore pelo TEXTO da coluna 2**;
> o número é o retrato de um instante, e este documento não tem como mantê-lo.

**A leitura que muda o conserto:** o estado `Confirmando` decide **para onde a
escrita é desviada** (passo 7). Quem a **perde** são os passos 2, 4 e 9 — a
lista já foi retirada, a marca já foi selada, e a transação será descartada.
Trocar o estado por `Ativa` no passo 1 não salvaria nada: a escrita cairia
exatamente no mesmo `tx.escritas` vazio do passo 8, e o passo 9 a jogaria fora
do mesmo jeito.

É o padrão do pedido 113 outra vez — **alvo certo, causa incompleta**. Quem
implementasse a frase do pedido ao pé da letra mexeria no passo 1 e mediria
zero.

---

## 3. O defeito, remedido no HEAD de hoje

Sonda pelo soquete, servidor de verdade (`target/release/phxsqld` construído
hoje do `520a0cb`), porta 7462, base isolada. Tabelas `clientes` e `auditoria`,
gatilho `CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW INSERT INTO
auditoria (id, ev) VALUES (NEW.id, 'entrou')`.

| # | cenário | medido |
|---|---|---|
| **R1** | `inserir` **fora** de transação (controle) | `{"rowid":1,"registros":1}` · **auditoria = 1 linha** |
| **R2** | `BEGIN` → `inserir` → `COMMIT` | `{"transaction_state":"COMMITTED","gravadas":1,"ms":0}`, **sem `gatilhos_avisos`** · **auditoria continua com 1 linha** (a do R1) · `clientes = [1,2]` |
| **R3** | `inserir` manual na auditoria logo depois | `{"rowid":2,"registros":2}` — **sem buraco**: a reserva do empilhar fantasma é devolvida |
| **R4** | coluna `Sequence` lida **dentro** da transação | `{"id":null,"rownum":0,"rowstamp":0,"rowtime":"1970-01-01 00:00:00,000"}` · depois do commit: `{"id":1,"rownum":1,"rowstamp":5,"rowtime":"2026-09-23 23:31:20,962"}` |
| **R5** | o mesmo com um gatilho que **falha** (chave duplicada) | `"gatilhos_avisos": ["gatilho \"audita2\" falhou: [SP000020] chave duplicada…"]` — **o AFTER roda mesmo, e o canal de aviso funciona no COMMIT** |
| **R6** | `BEGIN` → `inserir` → `ROLLBACK` | `{"descartadas":1}` · auditoria intacta — **o AFTER nem roda**, que é o correto |
| **R7** | `begin` com `"scope":["clientes"]`, `"scope_mode":"STRICT"` | `tabelas_declaradas:["loja/clientes"]` mas `tabelas_efetivas:["loja/auditoria","loja/clientes"]` |

Três leituras destes números:

1. **O pedido 262 está confirmado, palavra por palavra, sete dias depois.**
2. **O motor é calado no sucesso e barulhento no erro** (R2 × R5): o gatilho que
   grava certo some sem aviso; o que viola uma chave aparece. É a pior
   combinação num observador — quem lê a resposta do `COMMIT` conclui que a
   auditoria foi gravada porque nada reclamou.
3. **A trava é paga por uma escrita que vai ao chão** (R7): o `escopo_efetivo`
   (`fn escopo_efetivo`, 14293) já faz o fecho transitivo pelos alvos dos gatilhos e
   põe `auditoria` no escopo desde o `BEGIN`. O motor **declara a intenção, paga
   a trava e joga a escrita fora** — e metade da máquina do conserto, portanto,
   já existe.

E o comentário de o comentário `# O GATILHO entra, e alcanca de verdade`, acima de `fn escopo_efetivo` (14289) diz hoje, com todas as letras:

> *«O corpo de um gatilho grava noutra tabela com `INSERT INTO`, e isso acontece
> de verdade — o `rodar_gatilhos_depois` executa.»*

Dentro de uma transação, **não acontece**. `docs/TRANSACOES.md:415-416` repete a
mesma frase. É o padrão que o `CLAUDE.md` já nomeou: *comentário que se declara
resolvido é o motivo de ninguém olhar de novo.*

---

## 4. A matriz dos quatro motores — célula por célula, com a procedência

Três dos quatro foram **exercitados nesta máquina hoje**. A coluna «de onde
saiu» diz qual foi corrida e qual foi leitura; nenhuma célula é de memória.

### 4.1 A matriz

| motor | o `AFTER` roda dentro da transação que o disparou? | o `ROLLBACK` desfaz a gravação do gatilho? | o corpo pode `COMMIT` sozinho? | existe gatilho disparado **no commit**? | a gravação **dele** entra no commit? |
|---|---|---|---|---|---|
| **PostgreSQL 16.13** (peso 4) | **sim** — `P1: auditoria=1` dentro da tx | **sim** — `P3` volta a 1 | **não** — `ERROR: invalid transaction termination` | **sim, e só ele** — `CONSTRAINT TRIGGER … DEFERRABLE INITIALLY DEFERRED` | **sim** — `P4a=1` antes, `P4b=2` depois do `COMMIT` |
| **MariaDB** (peso 3) | **sim** (leitura) | sim, por consequência | **não** (leitura, cadeia de duas páginas) | **não existe o mecanismo** | — |
| **MySQL 8.0.46** (peso 2) | **sim** — `M1: auditoria=1` dentro da tx | **sim** — `M3=2` → `M4=1` | **não** — `ERROR 1422 (HY000)` | **não existe o mecanismo** | — |
| **SQLite 3.45.1** (peso 1) | **sim** — `C1: auditoria=1` dentro da tx | **sim** — `C3=2` → `C4=1` | **não** — `Error: in prepare, near "COMMIT": syntax error` | **não existe o mecanismo** | — |
| *Apache Cassandra® 5.0* (base permanente, sem voto) | não há transação; o gatilho **aumenta a mutação** | — | — | — | **sim** — «*returned mutations are atomically updated*» |

### 4.2 De onde saiu cada célula

**PostgreSQL 16.13 — por CORRIDA**, servidor local subido nesta máquina
(`pg_ctlcluster 16 main start`), roteiro em `psql -q -t -X`:

```
P1 dentro-da-tx auditoria=1
P2 depois-do-commit auditoria=1
P3 depois-do-rollback auditoria=1
P4a antes-do-commit (deferido ainda nao correu) auditoria=1
P4b DEPOIS do commit auditoria=2
P4c a linha do gatilho de commit-time: entrou 3
P5 cascata de commit-time: auditoria2=1
P6 a linha do cliente sobreviveu? id=9 conta=0     (o AFTER deferido que falha DERRUBA o commit)
P7 o cliente 50 entrou? conta=0                    (COMMIT no corpo: invalid transaction termination)
```

`P5` é a célula que mais importa e que nenhuma leitura entrega: a gravação feita
**pelo** gatilho de commit-time disparou **outro** gatilho de commit-time, e o
segundo também gravou, no mesmo commit. **O laço existe, medido, não só lido.**

Confirmação no fonte da versão que eu rodei
(`REL_16_STABLE`, `src/backend/access/transam/xact.c`,
https://raw.githubusercontent.com/postgres/postgres/REL_16_STABLE/src/backend/access/transam/xact.c),
dentro de `CommitTransaction()`:

- **2189** — *«Since closing cursors could queue trigger actions, triggers could open cursors, etc, we have to keep looping until there's nothing left to do.»*
- **2191** — `for (;;)`
- **2196** — `AfterTriggerFireDeferred();`
- **2268** — `s->state = TRANS_COMMIT;`
- **2275-2277** — *«This is where we durably commit.»* · `latestXid = RecordTransactionCommit();`

**A ordem é inequívoca: os `AFTER` deferidos correm ANTES de o estado virar
commit e ANTES do registro durável.** O nosso `gravar_marca`
(`gravar_marca` (15750)) é o equivalente do `RecordTransactionCommit()`, e nós
rodamos os `AFTER` em **15780**, depois dele. **É essa inversão, e só ela, que
produz o pedido 262.**

Documentação, fonte primária do fabricante
(https://www.postgresql.org/docs/16/trigger-definition.html), conferida hoje:

> *«The execution of an `AFTER` trigger can be deferred to the end of the
> transaction, rather than the end of the statement, if it was defined as a
> constraint trigger.»*
>
> *«In all cases, a trigger is executed as part of the same transaction as the
> statement that triggered it, so if either the statement or the trigger causes
> an error, the effects of both will be rolled back.»*

**MySQL 8.0.46 — por CORRIDA**, servidor local (`service mysql start`),
tabelas `ENGINE=InnoDB`:

```
M1 dentro-da-tx auditoria=1
M2 depois-do-commit auditoria=1
M3 antes-do-rollback auditoria=2
M4 depois-do-rollback auditoria=1
M5 COMMIT no corpo          -> ERROR 1422 (HY000): Explicit or implicit commit is not allowed in stored function or trigger.
M6 START TRANSACTION no corpo -> ERROR 1422 (HY000): idem
M7 gatilho AFTER que falha  -> ERROR 1062; clientes id=3 conta=0  (a instrucao inteira caiu)
```

**SQLite 3.45.1 — por CORRIDA**, `sqlite3 t.db`:

```
C1 dentro-da-tx auditoria=1
C2 depois-do-commit auditoria=1
C3 antes-do-rollback auditoria=2
C4 depois-do-rollback auditoria=1
C5 COMMIT no corpo -> Error: in prepare, near "COMMIT": syntax error
```

O manual do `CREATE TRIGGER` (https://www.sqlite.org/lang_createtrigger.html)
**não afirma nada** sobre transação — a corrida responde o que a página cala.

**MariaDB — por LEITURA, e é a única célula que não exercitei.** Não há servidor
nem cliente MariaDB nesta máquina (`which mariadb` vazio). Fontes primárias do
fabricante, em cadeia de duas páginas:

> *«With transactional engines, triggers are executed in the same transaction as
> the statement that invoked them.»* — https://mariadb.com/kb/en/trigger-overview/

> *«All of the restrictions listed in Stored Function Limitations»* aplicam-se a
> gatilhos — https://mariadb.com/kb/en/trigger-limitations/ — e ali:
> *«Statements that perform explicit or implicit commits or rollbacks are not
> permitted.»* — https://mariadb.com/kb/en/stored-function-limitations/

**Correção ao documento de 16/09:** ele citou a página de *stored function*
diretamente como se ela falasse de gatilho. Ela não fala — quem fecha a cadeia é
a página de *trigger limitations*, que remete a ela. A conclusão é a mesma; a
procedência estava curta por uma página.

**Apache Cassandra® 5.0 — por LEITURA do fonte:**

```java
/**
 * Called exactly once per CF update, returned mutations are atomically updated.
 */
public Collection<Mutation> augment(Partition update);
```
— `src/java/org/apache/cassandra/triggers/ITrigger.java`, ramo `cassandra-5.0`:
https://raw.githubusercontent.com/apache/cassandra/cassandra-5.0/src/java/org/apache/cassandra/triggers/ITrigger.java

---

## 5. A régua aplicada, com a soma escrita

### 5.1 Pergunta A — «a gravação do gatilho pertence à transação que o disparou?»

| motor | peso | voto |
|---|---|---|
| PostgreSQL | 4 | sim (corrida) |
| MariaDB | 3 | sim (leitura) |
| MySQL | 2 | sim (corrida) |
| SQLite | 1 | sim (corrida) |

**Soma: 10 a 0.** Os três maduros convergem → **aceite automático**, pela ordem
do dono de 11/09/2026. **A régua ponderada NÃO foi usada**, e o motivo é medido:
ela serve para divergência, e não há dois lados. A premissa do pedido 262 («os
três motores divergem em gatilho que escreve na própria transação») está
**morta**.

### 5.2 Pergunta B — «o gatilho disparado NO COMMIT que grava: onde vai a gravação?»

Só o PostgreSQL tem o mecanismo. Ele **vota sozinho: 4 a 0**, e o voto dele,
medido em `P4b`/`P5`/`P6`, é:

> a gravação entra no **mesmo** commit; ela roda **antes** do registro durável;
> ela roda **em laço**, porque o que o gatilho grava pode disparar mais gatilhos;
> e a falha dela **derruba o commit inteiro**.

Os outros três somam **0** — não por discordarem, mas por não terem o mecanismo.
Os dois caminhos (convergência e ponderada) dão o mesmo resultado.

### 5.3 Pergunta C — «o gatilho pode escrever FORA da transação?» (a hipótese H2)

| motor | peso | recusa | de onde |
|---|---|---|---|
| PostgreSQL | 4 | `ERROR: invalid transaction termination` | corrida |
| MariaDB | 3 | *«explicit or implicit commits or rollbacks are not permitted»* | leitura |
| MySQL | 2 | `ERROR 1422 (HY000)` | corrida |
| SQLite | 1 | `syntax error near "COMMIT"` | corrida |

**Soma: 0 a 10.** Nenhum dos quatro permite. Três recusam **no analisador ou em
tempo de execução**, medido aqui — não é convenção de manual, é porta fechada.

### 5.4 O crivo das pétreas

| pétrea | opõe-se? | medida |
|---|---|---|
| a ordem de digitação do `.reg` é sagrada | **não** | R3: o `inserir` seguinte saiu com `rowid 2`, **sem buraco** |
| só existe filho se o pai existir primeiro | **não — EMPURRA para (a)** | sob (a) a gravação do gatilho entra na mesma passada, que já empresta as mães abertas; sob (b) ela seria conferida contra o disco, fora da transação |
| nunca se mata o pai que tem filhos | **não** | nada aqui toca `ao_excluir` |
| impossível o filho ter a mesma data do pai | **não** | §6.2, medido: `rowstamp` do pai < do filho em **100 de 100** pares |
| zero dependências externas | **não** | nada aqui pede biblioteca |
| guarda nova entra pedida, não imposta | **opõe-se a uma PARTE de (a)** | §7.2: abortar o commit por falha de `AFTER` quebraria cliente que hoje recebe aviso |

---

## 6. As hipóteses que morreram, com o motivo

### 6.1 H1 — «a sessão sai de `Confirmando` antes dos AFTER» — **morre por ambiguidade**

A frase tem duas leituras, e **nenhuma das duas é o conserto**:

- *«trocar o valor do campo `estado`»* → **não muda nada**, medido pelo código na
  §2: a escrita continua caindo no `tx.escritas` vazio do passo 8 e continua
  sendo descartada no passo 9. Zero.
- *«encerrar a transação antes dos AFTER»* → isso **é** a hipótese H2 com outro
  nome, e H2 está recusada 0 a 10 (§5.3).

O conserto certo não é *sair* de `Confirmando`: é **não ter chegado lá ainda**
quando os `AFTER` correm — que é H4, e é o que o PG faz (o `s->state =
TRANS_COMMIT` da linha 2268 acontece **depois** do laço da 2191).

### 6.2 H6 — «o `rowtime` pai/filho no mesmo commit quebra a pétrea do dono» — **MORREU medida**

Levantei-a ao ver `rowtime` no R4 e fui medir antes de escrever. Sonda pelo
soquete, **100 pares mãe/filha com FK, num único commit**:

```
R10: pares=100   pai e filho com a MESMA data = 98   pai DEPOIS do filho = 0
     pares com rowstamp do pai < do filho = 100 de 100
```

E num lote de 200 linhas no mesmo commit:

```
R9: linhas=202  rowtime distintos=4  rowstamp distintos=202
    rowtime REPETIDOS = 198 de 202     rowstamp REPETIDOS = 0 de 202
```

**Parece um furo e não é: é o desenho decidido.** O pedido **289** do
`PENDENCIAS.md` (☑️, decisão do dono de 17/09/2026 07:10 UTC, revista pela régua
dos motores) escolheu exatamente **duas colunas**: um contador puro por nó, que
cumpre a ordem e não depende de relógio, mais uma data e hora comum, «que pode
empatar sem mentir». É o que eu medi: `rowstamp` ordena **100/100**; `rowtime`
empata **98/100**, por construção e com o motivo escrito.

**Valor de matar esta hipótese aqui:** ela tira a última objeção à saída (a). A
gravação do gatilho pode entrar na mesma passada sem risco nenhum para o
invariante do dono, porque quem carrega a ordem é o `rowstamp`, e ele é
monotônico. Fica registrado para não voltar sem medição.

### 6.3 H3 — «é contrato, não defeito» — **meia-verdade, e a metade certa não salva**

A metade verdadeira: o PhxSql **já diverge dos quatro em QUANDO o `AFTER`
dispara**, e essa divergência é legítima e é nossa. Nos quatro, o `AFTER` comum
roda no fim da **instrução**; aqui a instrução não grava — ela **empilha**
(`tx.escritas.push(escrita)` (15124)) —, então não existe «fim da instrução» onde ele possa
rodar. Isso é consequência de o conjunto de escrita ir inteiro para uma marca
idempotente, e não cópia mal feita.

A metade falsa, e é a que decide: **contrato não pode ser «some calado».**
Nenhum dos quatro perde a escrita — eles a gravam (10 a 0) ou recusam a
tentativa (0 a 10). *Perder em silêncio não é opção de nenhum motor medido.*
Medido no R2: nem a resposta, nem `gatilhos_avisos`, nem o log dizem qualquer
coisa. **É defeito.**

---

## 7. O contrato do conserto, para o papel B

**Veredito: DEFEITO.** Duas etapas, nesta ordem, e a primeira **não depende** da
segunda.

### 7.1 Etapa 1 — a recusa nomeada (barata, não quebra ninguém)

**O que muda.** `dentro_da_transacao` (`fn dentro_da_transacao` (13975)) passa a barrar
`Estado::Confirmando` antes do desvio para `empilhar` (`if OPS_EMPILHAVEIS.contains(&op)`, 14014), devolvendo erro
**nomeado** — algo como *«este `inserir` veio de um gatilho `AFTER` disparado no
COMMIT: a transação já está confirmando e não aceita escrita nova»*. O erro sobe
pelo caminho que **já existe e já está provado**: `rodar_gatilhos_depois`
(o `avisos.push` do `gatilho {:?} falhou: {e}`, 17978) o transforma em `gatilhos_avisos`, e a resposta do
`COMMIT` o carrega — medido no **R5**, que é hoje o único caso em que o canal
acende.

**Por que barrar e não gravar.** Gravar exige a etapa 2. Barrar troca *«a
gravação some e nada avisa»* por *«a gravação não acontece e o motor diz»* — que
é a diferença entre as duas formas que o próprio pedido 262 nomeia.

**Não quebra cliente**: hoje **nenhum** cliente recebe a linha de auditoria de
qualquer jeito (R2). O `COMMIT` continua `COMMITTED`, `gravadas` não muda, e o
que nasce é um aviso a mais. É «guarda nova entra pedida, não imposta» cumprida
por construção — nada que hoje funciona passa a falhar.

**Texto que vai junto, no mesmo commit** — são afirmações **falsas** hoje,
medidas:

| arquivo:linha | o que diz hoje | por que é falso |
|---|---|---|
| o comentário `# O GATILHO entra, e alcanca de verdade`, acima de `fn escopo_efetivo` (14289) | *«o corpo de um gatilho grava noutra tabela… e isso acontece de verdade»* | dentro da transação, não acontece (R2) |
| `docs/TRANSACOES.md:415-416` | a mesma frase | idem |
| `docs/TRIGGERS.md:419` | *«`BEGIN`/`COMMIT` no corpo → **não há transação no PhxSql**»* | há transação desde o pedido 162 |
| `docs/TRIGGERS.md:435` | *«o limite que vale repetir: **não há transação**»* | idem |

### 7.2 Etapa 2 — o laço à PostgreSQL (o destino)

**O que muda**, e a ordem sai medida do `xact.c` (§4.2):

1. tirar a lista da transação **sem** encerrá-la e **sem** entrar em
   `Confirmando` (hoje, o `tx.estado = … Confirmando` e o `mem::take` logo abaixo);
2. rodar os `AFTER` de cada escrita **antes** de `gravar_marca`, com a
   sessão num estado que aceite `empilhar`;
3. **repetir** enquanto os gatilhos empilharem escritas novas — o teto já existe
   (`CADEIA_MAXIMA`, e `rodar_gatilhos_depois` já conta a profundidade em
   `if nivel >= CADEIA_MAXIMA` (17942));
4. só então `gravar_marca` com a lista **completa**, e a passada.

**O bloqueio, medido hoje e maior do que o documento de 16/09 dizia.** O `AFTER`
precisa do `NEW` «como a linha FICOU gravada», e hoje ele sai de `t.ler(e.rowid)`
**depois** da passada. Antes da passada a linha não está pronta — medido no
**R4**, são **quatro** colunas de sistema, não duas:

| coluna | dentro da transação | depois do commit |
|---|---|---|
| `id` (`Sequence`) | `null` | `1` |
| `rownum` | `0` | `1` |
| `rowstamp` | `0` | `5` |
| `rowtime` | `1970-01-01 00:00:00,000` | `2026-09-23 23:31:20,962` |

Rodar o `AFTER` antes da passada **hoje** gravaria uma auditoria com `id` nulo e
data de 1970 — trocar perda silenciosa por **dado errado**, que é pior. O
destravamento já está escrito nesta casa, por outro motivo: `docs/AUTONUMBER.md`
§B.4 **item 5** — *«numerar no `INSERT`, dentro da transação — reserva na
`Escrita` + devolução — não muda formato, não quebra cliente»*. **Os dois
pedidos são o mesmo pedido visto de dois lados**, e o 262 depende daquele.

**Duas decisões de contrato que o papel B NÃO inventa — decididas aqui:**

- **falha de `AFTER` dentro da transação continua sendo AVISO, não aborto.** O PG
  aborta (P6, medido), e nós **divergimos** — a restrição nossa que causa a
  divergência é a pétrea *«proteção que quebra todo cliente antigo não é
  proteção, é estrago»*: hoje o `COMMIT` com gatilho que falha devolve
  `COMMITTED` + aviso (R5), e transformar isso em erro derrubaria cliente que
  funciona. Quem **quiser** o aborto pede — e aí é escolha escrita, como o
  `"verificar": false` da chave.
- **`gravadas` NÃO passa a contar as linhas do gatilho.** Somá-las mudaria um
  número que o cliente já lê. A linha do gatilho vai num campo próprio
  (`gravadas_por_gatilho`), e quem não olha continua vendo o que via.

### 7.3 A prova real, nos dois sentidos — e ela sai de graça

O defeito está **vivo no HEAD**, então o lado «falha com o defeito reposto» não
precisa de mecanismo nenhum de reposição: **escreva a conferência primeiro e
rode-a contra o binário de hoje.**

| conferência | tem de FALHAR contra | tem de PASSAR contra |
|---|---|---|
| **C1** `o_after_que_grava_no_commit_nao_some_calado` — após `BEGIN`/`inserir`/`COMMIT`, ou a auditoria tem a linha, **ou** `gatilhos_avisos` traz o motivo nomeado | `520a0cb` de hoje: `gatilhos_avisos` **ausente** e auditoria **vazia** (R2 medido) | a etapa 1 |
| **C2** `o_commit_sem_gatilho_nao_muda_nada` — o comportamento **velho**: `BEGIN`/`inserir`/`COMMIT` sem gatilho devolve `gravadas:1`, sem `gatilhos_avisos` | uma etapa 1 que barrasse escrita demais | as duas etapas |
| **C3** `o_after_do_commit_grava_na_mesma_passada` — auditoria com **1** linha, e `rowstamp` dela **> ** o da linha que a disparou | a etapa 1 (que recusa) e o HEAD | a etapa 2 |
| **C4** `o_after_do_commit_nao_roda_no_rollback` — o irmão que fica: `ROLLBACK` → auditoria intacta | um conserto que rodasse o `AFTER` cedo demais | HEAD (R6 já passa) e as duas etapas |
| **C5** `o_rowid_da_auditoria_nao_ganha_buraco` — o `inserir` seguinte sai com `rowid` consecutivo | uma etapa 2 que reservasse e não devolvesse | HEAD (R3 já passa) e as duas etapas |

**Onde elas moram.** `bancada/transacoes/provar.py` — isto depende do estado da
**sessão** através de uma conexão de verdade, e teste unitário monta a `Sessao`
à mão. O `prova-dos-portoes.py` ao lado é a máquina de defeito reposto que já
existe; a chave nova entra no `DEFEITOS` dele com o trecho que **tem** de
derrubar. **C2 é a mais importante das cinco** — numa regra de permissão ou de
recusa nova, o teste que mais importa é o do comportamento *velho*.

**E o irmão que não pode ficar para trás.** *Conserto entra no caminho que o
motivou, e o caminho irmão fica.* Irmão aqui é **quem chama as mesmas funções na
mesma ordem**: o `rodar_gatilhos_depois` é chamado do `op_commit` (15780) e do
caminho **sem** transação. A etapa 1 mexe no portão `dentro_da_transacao`, que é
comum — então quem for consertar confira se o caminho sem transação continua
intacto (R1 é a linha de base: `auditoria = 1 linha`).

---

## 8. O que sobe ao dono: **nada** — e o porquê

Passei os três filtros da pétrea, um a um:

| filtro | veredito |
|---|---|
| **choque com pétrea nossa** | **não há.** Crivo completo na §5.4, com R3 e R10 medidos. A única tensão (abortar o commit) foi **resolvida pela própria pétrea** em §7.2: entra pedida, não imposta |
| **empate real que a matriz não decide** | **não há.** 10 a 0 na pergunta A, 4 a 0 na B, 0 a 10 na C |
| **decisão de produto** | **não há**, depois de §7.2: as duas mudanças visíveis ao cliente (aborto e `gravadas`) foram desenhadas para **não** mudar o que o cliente vê hoje |

É a lição do pedido **340** aplicada: um item que chega à mesa com «entra por
aceite automático» escrito no corpo não devia ter chegado. Este não chega.

---

## 9. Onde a nossa lógica DIVERGE da do PostgreSQL, e qual restrição causa

A lei cobra esta seção: se não diverge em lugar nenhum, não passou pela nossa
cabeça — passou pelos nossos dedos.

| ponto | PostgreSQL 16 (medido) | PhxSql, e a restrição nossa |
|---|---|---|
| quando o `AFTER` comum dispara | fim da **instrução** (P1: visível antes do commit) | fim do **COMMIT** — a instrução não grava, ela empilha (`tx.escritas.push(escrita)` (15124)), porque a marca `.tx` precisa da lista inteira para ser idempotente |
| o que é o «registro do commit» | `RecordTransactionCommit()` **depois** de tudo (xact.c:2277) | `gravar_marca` **antes** da passada (`gravar_marca` (15750)) — é marca de *intenção* redo, não registro de conclusão. Daí a inversão que o 262 expõe |
| `DEFERRABLE` como escolha do usuário | existe, por gatilho (P4) | **não entra**: aqui todo `AFTER` já é adiado ao commit por consequência do desenho; um segundo eixo de adiamento seria ajuste sem caso de uso |
| falha de `AFTER` | **derruba a transação** — P6: `clientes id=9 conta=0` | **continua sendo aviso por padrão**, e o aborto entra **pedido** — pétrea «proteção que quebra todo cliente antigo é estrago» |
| a ordem no dado | o relógio **empata de propósito**; a ordem mora no `xmin` | duas colunas por decisão do dono (pedido 289): `rowstamp` ordena (100/100 medido), `rowtime` pode empatar (98/100 medido) |
| sequência consumida no `INSERT` | sim, e **não devolve** no rollback | aqui a reserva **se devolve** — numeração de documento fiscal não aceita buraco (R3: sem buraco) |

---

## 10. O que eu NÃO alcancei, dito como não alcançado

| lacuna | o que decidiria na bancada |
|---|---|
| **MariaDB não foi exercitado** — não há servidor nem cliente nesta máquina (`which mariadb` vazio). A célula dele é **leitura**, em cadeia de duas páginas do fabricante | subir um MariaDB e repetir o roteiro `M1`-`M7`. A previsão escrita antes (P4) é «igual ao MySQL»; nada a contradiz, e nada a mediu |
| **o custo em tempo do laço de `AFTER` antes da marca** | `bancada/transacoes` com 1, 10 e 1.000 linhas por commit, com e sem gatilho. Todos os meus commits deram **0-2 ms** (R2: 0 ms; R9: 16 ms para 200 linhas) e esse relógio não separa nada |
| **se rodar o `AFTER` fora da trava de dados abre corrida** | duas conexões de verdade, uma commitando com gatilho e a outra gravando na auditoria, com o `LOCK TIMEOUT` em relógio de parede. Há uma catraca em `servidor.rs:38948` que já diz *«o AFTER roda SEM a trava»* — mexer nisso mexe nela |
| **quanto do `Table::inserir_com_maes_opt` é «preparar a linha» e quanto é «gravar»** | `--example onde-doi`. É o que diz se o item 5 do `AUTONUMBER.md` é barato mesmo |
| **se algum cliente hoje depende do silêncio** | varredura do `MANUAL.txt`, do `docs/TRIGGERS.md` e da interface web por promessa de auditoria transacional. **Não fiz** |
| **MySQL/MariaDB no FONTE** | só usei manual do fabricante + corrida. `sql/sql_trigger.cc` e `sql/sp_head.cc` ficariam para quem quiser a linha exata |

---

## 11. O que foi avaliado e RECUSADO, com o número

Para a mesma proposta não voltar sem medição.

| recusado | o número que recusa |
|---|---|
| **a premissa do 262** («os três motores divergem») | **10 a 0**: os quatro convergem. Três medidos por corrida nesta máquina |
| **usar a régua ponderada aqui** | não há dois lados; 3 dos 4 nem têm gatilho de commit-time |
| **H1 «sair de `Confirmando`»** | zero: a escrita continua caindo no `tx.escritas` esvaziado pelo `mem::take` e descartada pelo `descartar_transacao` |
| **H2 «o gatilho escreve fora da transação»** | **0 a 10**. Três recusas medidas: `invalid transaction termination` (PG), `ERROR 1422` (MySQL), `syntax error` (SQLite) |
| **H3 «é só contrato»** | R2: nem resposta, nem aviso, nem log. Nenhum dos quatro perde escrita em silêncio |
| **H6 «o `rowtime` quebra a pétrea do dono»** | `rowstamp` do pai < do filho em **100 de 100** pares; o empate de `rowtime` (98/100) é o desenho decidido no pedido 289 |
| **rodar o `AFTER` antes da passada HOJE** | R4: **quatro** colunas de sistema vazias — `id:null`, `rownum:0`, `rowstamp:0`, `rowtime:1970-01-01` |
| **culpar o `upsert` (pedido 245)** | R2 acontece em `AFTER INSERT` simples, sem upsert nenhum |
| **suspeitar de buraco no `.reg`** | R3: `rowid 2`, consecutivo. A reserva do empilhar fantasma é devolvida |
| **achar que o `SCOPE STRICT` protegeria** | R7: `tabelas_efetivas` já inclui `loja/auditoria`, e a escrita some do mesmo jeito |

---

## 12. Como refazer a medição na próxima sessão

Sem isto o documento envelhece.

```bash
# 1. binario novo -- medidor com binario velho mede o passado
cargo build --release -p phxsql-server --bin phxsqld --offline

# 2. os motores de fora, nesta maquina
pg_ctlcluster 16 main start      # PostgreSQL 16.13
service mysql start              # MySQL 8.0.46
# sqlite3 nao precisa de servidor

# 3. o roteiro dos tres: tabela + tabela de auditoria + AFTER INSERT que grava,
#    e cinco perguntas -- dentro da tx / depois do commit / depois do rollback /
#    COMMIT no corpo / gatilho que falha.
#    No PG, mais duas: CONSTRAINT TRIGGER ... DEFERRABLE INITIALLY DEFERRED que
#    grava, e um segundo deferido sobre a tabela de auditoria (o laco).

# 4. a sonda do nosso, pelo soquete: config isolado, web desligado,
#    cifra_fio.exigir=false, no molde de bancada/transacoes/provar.py
```

Quatro armadilhas que **eu** paguei montando isto, e que a próxima sessão não
precisa pagar:

- as linhas vêm em `resultado.linhas`, **não** no topo da resposta. Ler
  `r["linhas"]` devolve `[]` para tudo e faz o controle parecer quebrado junto
  com o defeito;
- o `begin` com escopo exige **`database`** junto, senão recusa com
  *«SCOPE precisa do "database"»* — e o campo chama-se **`scope`**, nunca
  `tabelas`;
- `buscar` exige `"indice"`; para ler o que está empilhado dentro da transação,
  use **`varrer`**;
- o `psql` rodado por `su postgres -c` **não lê arquivo do scratchpad**
  (permissão). Mande o roteiro pela **entrada padrão**; e `COMMIT` dentro de um
  corpo de gatilho no MySQL precisa de `DELIMITER`, senão o erro que volta é do
  cliente e não do motor — eu quase registrei um `ERROR 1064` como se fosse a
  recusa do MySQL, e a recusa de verdade é `ERROR 1422`.
