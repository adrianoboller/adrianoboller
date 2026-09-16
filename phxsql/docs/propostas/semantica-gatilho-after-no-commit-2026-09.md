# Gatilho `AFTER` que grava no `COMMIT` — a semântica dos quatro motores, medida contra o nosso gargalo

**Pesquisa do papel J, 16/09/2026.** Pedido **262** do `docs/PENDENCIAS.md`.
Medições feitas hoje, pelo soquete, contra o binário construído do commit
`69b9d58` (`rustc 1.94.1`, `target/release/phxsqld` de 16/09/2026 13:21).
Toda citação de fora traz a URL da **fonte primária** — RFC, manual do
fabricante ou fonte do motor. Nenhum número aqui é citado de memória.

---

## 0. O resumo, em cinco linhas

1. **A premissa escrita no pedido 262 está errada.** Ela diz que os três
   motores maduros *divergem* em gatilho que escreve na própria transação.
   Medido na documentação primária e no fonte: os **quatro** convergem, sem
   exceção — e o Cassandra, que nem transação tem, converge junto.
2. **O defeito existe e é pior do que o pedido descreve**: ele é *calado no
   sucesso e barulhento no erro* — o gatilho que grava certo some sem aviso, e
   o que viola uma chave aparece em `gatilhos_avisos`.
3. **A trava da tabela de auditoria JÁ é tomada** pela transação, na abertura
   (`tabelas_efetivas`). O motor declara a intenção, paga a trava, e joga a
   escrita fora.
4. **O veredito da régua é aceite automático** — convergência unânime, e
   nenhuma pétrea nossa se opõe ao comportamento.
5. **A recomendação é (c) HOJE e (a) como destino**, nessa ordem e por um
   motivo medido: o que bloqueia (a) é a coluna `Sequence` não existir antes da
   passada do commit — medido `"id": null` dentro da transação —, e o conserto
   disso já está planejado noutro documento desta casa.

---

## 1. A premissa do pedido 262, medida antes do item

O pedido 262 fecha com:

> *«os três motores divergem em gatilho que escreve na própria transação, então
> aqui vale a régua ponderada e não a convergência»*

**Isto é palpite, não medição** — e a lei desta casa manda medir a premissa do
item antes do item, inclusive quando o item é nosso (`CLAUDE.md`, a lição do
pedido 113). Medido, a premissa cai por inteiro: a divergência entre eles não é
sobre *onde a escrita do gatilho vai parar*; é sobre *quando o gatilho dispara*,
e nessa segunda pergunta **três dos quatro nem têm o mecanismo** — só o
PostgreSQL sabe disparar um gatilho no commit.

Consequência direta: **a régua ponderada não decide este caso, porque não há
dois lados.** Quem decide é a convergência, e ela é unânime.

---

## 2. O defeito, confirmado por medição própria

### 2.1 O caminho, arquivo e linha

| passo | arquivo:linha | o que acontece |
|---|---|---|
| 1 | `crates/phxsql-server/src/servidor.rs:14112` | `tx.estado = Estado::Confirmando` — **a sessão entra em `Confirmando` aqui** |
| 2 | `servidor.rs:14116` | `std::mem::take(&mut tx.escritas)` — a lista sai da transação; **a transação continua existindo, agora vazia** |
| 3 | `servidor.rs:14140` | `gravar_marca` — o ponto de compromisso, `fsync` |
| 4 | `servidor.rs:14152` | `aplicar_conjunto` — a passada; o `NEW` de cada linha é lido do disco em `servidor.rs:14300` |
| 5 | `servidor.rs:14167-14171` | `drop(trava)` e então `rodar_gatilhos_depois` — **os `AFTER` rodam com a sessão ainda em `Confirmando`** |
| 6 | `servidor.rs:12365-12406` | o `inserir` do corpo do gatilho passa por `dentro_da_transacao`, que **não olha o estado** a não ser para `AbortOnly`, e desvia para `empilhar` |
| 7 | `servidor.rs:13516` | `tx.escritas.push(escrita)` — a gravação da auditoria entra na lista da transação **que já foi esvaziada** |
| 8 | `servidor.rs:14189` | `self.descartar_transacao(sessao.ligacao)` — a lista, com a linha da auditoria dentro, é jogada fora |

O passo 6 é a porta: `dentro_da_transacao` recusa em `AbortOnly` e recusa por
prazo vencido, mas **`Confirmando` passa** e cai em `OPS_EMPILHAVEIS`.

### 2.2 O que foi medido (sonda pelo soquete, servidor de verdade)

Tabelas `clientes` e `auditoria`, gatilho
`CREATE TRIGGER audita AFTER INSERT ON clientes FOR EACH ROW INSERT INTO auditoria …`.

| # | cenário | resposta medida | auditoria depois |
|---|---|---|---|
| M1 | `inserir` **fora** de transação | `{"rowid":1,"registros":1}` | **1 linha** — `{"id":1,"evento":"entrou"}` |
| M2 | `BEGIN` → `inserir` → `COMMIT` | `{"transaction_state":"COMMITTED","gravadas":1}`, **sem `gatilhos_avisos`** | **vazia** |
| M4 | `BEGIN` → `atualizar` → `COMMIT` (AFTER UPDATE) | `{"transaction_state":"COMMITTED","gravadas":1}`, sem avisos | **vazia** |
| M5 | `BEGIN` → `inserir` → `ROLLBACK` | `{"descartadas":1}` | vazia, e **o AFTER nem chega a rodar** (correto) |
| A6 | o mesmo por SQL puro (`BEGIN`/`INSERT`/`COMMIT`) | idem M2 | **vazia** |

O pedido 262 está confirmado: `COMMIT => {"transaction_state":"COMMITTED",
"gravadas":1}` e `AUDITORIA => []`, sem `gatilhos_avisos`.

### 2.3 A agravante que o pedido não registrou: **calado no sucesso, barulhento no erro**

Mesmo gatilho, corpo trocado para gravar sempre a mesma chave primária:

| # | cenário | resposta medida |
|---|---|---|
| A2 | fora de transação, chave duplicada no gatilho | `gatilhos_avisos: ["gatilho \"audita\" falhou: [SP000020] chave duplicada: indice unico pk ja tem essa chave"]` |
| A3 | **dentro** da transação, chave duplicada no gatilho | **o mesmo aviso aparece**, na resposta do `COMMIT` |
| A4 | **dentro** da transação, gravação **válida** | **nenhum aviso**, e a linha some |

Ou seja: o motor **relata o gatilho que falhou e esconde o gatilho que deu
certo**. É a pior combinação possível de um observador, e é ela que transforma
o pedido 262 de «gap» em «mentira sobre o dado»: quem lê a resposta conclui que
a auditoria foi gravada porque nada reclamou.

### 2.4 A contradição interna: a trava é tomada para uma escrita que será jogada fora

Medido com escopo declarado — `begin` com `"scope":["clientes"]`,
`"scope_mode":"STRICT"`:

```
tabelas_declaradas = ['loja/clientes']
tabelas_efetivas   = ['loja/auditoria', 'loja/clientes']
```

O `escopo_efetivo` (`servidor.rs:12683`) faz o fecho transitivo pelos alvos dos
gatilhos e **põe `auditoria` no escopo da transação, com a trava tomada, desde o
`BEGIN`**. O comentário logo acima, em `servidor.rs:12679-12682`, diz com todas
as letras:

> *«O corpo de um gatilho grava noutra tabela com `INSERT INTO`, e isso acontece
> de verdade — o `rodar_gatilhos_depois` executa.»*

Dentro de uma transação, **não acontece**. O `docs/TRANSACOES.md:415` repete a
mesma frase. É exatamente o padrão que o `CLAUDE.md` já nomeou uma vez:
*comentário que se declara resolvido é o motivo de ninguém olhar de novo.*

E a leitura útil disto para o conserto: **metade da máquina de (a) já existe** —
a intenção é declarada, o alcance é calculado e a trava está na mão. O que falta
é a escrita entrar na lista.

### 2.5 O que **não** quebrou, e isso importa

| conferência | medido |
|---|---|
| a ordem de digitação do `.reg` | **intacta**. O `empilhar` fantasma reserva o rowid e o `descartar_transacao` o devolve: a inserção real seguinte em `auditoria` saiu com **rowid 2**, sem buraco |
| a transação do cliente | **confirmada de verdade** — `clientes` tem a linha, `gravadas: 1`, marca `.tx` removida |
| `ROLLBACK` | correto: sem passada, sem `AFTER`, sem auditoria |

Não há corrupção. Há **perda silenciosa de escrita**, que é outra coisa e é pior
de descobrir.

---

## 3. O que os quatro motores fazem, na fonte primária

### 3.1 A matriz de evidência

| motor | o `AFTER` roda dentro da transação que o disparou? | a gravação dele entra no **mesmo** commit? | e se a transação faz `ROLLBACK` depois? | dispara no **commit**? |
|---|---|---|---|---|
| **PostgreSQL** (peso 4) | **sim**, sempre | **sim** | desfeita junto | **sim, e só ele** — `CONSTRAINT TRIGGER … DEFERRABLE INITIALLY DEFERRED` |
| **MariaDB** (peso 3) | **sim**, com motor transacional | **sim** | desfeita junto | não existe o mecanismo |
| **MySQL** (peso 2) | **sim**, com motor transacional | **sim** | desfeita junto | não existe o mecanismo |
| **SQLite** (peso 1) | **sim** — o corpo é um *sub-programa* da MESMA instrução | **sim** | desfeita junto | não existe o mecanismo |
| *Apache Cassandra®* (base permanente, sem voto) | não há transação; o gatilho **aumenta a mutação** | **sim** — «*returned mutations are atomically updated*» | — | — |

### 3.2 PostgreSQL — o único que responde à nossa pergunta exata

A documentação diz as duas coisas:

> *«The execution of an `AFTER` trigger can be deferred to the end of the
> transaction, rather than the end of the statement, if it was defined as a
> constraint trigger.»*
>
> *«In all cases, a trigger is executed as part of the same transaction as the
> statement that triggered it, so if either the statement or the trigger causes
> an error, the effects of both will be rolled back.»*
>
> — https://www.postgresql.org/docs/17/trigger-definition.html

E o **mecanismo** está no fonte, que é o que interessa para nós. Em
`src/backend/access/transam/xact.c` (ramo `REL_17_STABLE`), dentro de
`CommitTransaction()`:

- linha **2205**: `if (s->state != TRANS_INPROGRESS) elog(WARNING, …)` — ao
  entrar no commit a transação ainda está **em progresso**;
- linhas **2211-2230**, com o comentário do próprio fonte:
  *«Do pre-commit processing that involves calling user-defined code, such as
  triggers. … Since closing cursors could queue trigger actions, triggers could
  open cursors, etc, we have to keep looping until there's nothing left to do.»*
  — é um `for (;;)` que chama `AfterTriggerFireDeferred()` (linha **2222**) e só
  sai quando nada mais é enfileirado;
- linha **2311**: `s->state = TRANS_COMMIT;`
- linha **2325**: `latestXid = RecordTransactionCommit();`, com o comentário
  *«This is where we durably commit.»*

Fonte: https://raw.githubusercontent.com/postgres/postgres/REL_17_STABLE/src/backend/access/transam/xact.c

**A regra que sai daí, e que é a resposta ao pedido 262:** o gatilho de
commit-time do PostgreSQL roda **antes** do registro durável do commit, com a
transação ainda capaz de escrever, e num **laço** — porque o que o gatilho grava
pode disparar mais gatilhos. O nosso `gravar_marca` (`servidor.rs:14140`) é o
equivalente exato do `RecordTransactionCommit()`, e nós rodamos os `AFTER`
**depois** dele. É essa inversão, e só ela, que produz o pedido 262.

### 3.3 MariaDB — a frase mais direta dos quatro

> *«With transactional engines, triggers are executed in the same transaction as
> the statement that invoked them.»*
>
> — https://mariadb.com/kb/en/trigger-overview/

E a impossibilidade de fugir da transação:

> *«Statements that perform explicit or implicit commits or rollbacks are not
> permitted.»* — https://mariadb.com/kb/en/stored-function-limitations/

### 3.4 MySQL

> *«An error during either a `BEFORE` or `AFTER` trigger results in failure of
> the entire statement that caused trigger invocation. For transactional tables,
> failure of a statement should cause rollback of all changes performed by the
> statement. Failure of a trigger causes the statement to fail, so trigger
> failure also causes rollback.»*
>
> — https://dev.mysql.com/doc/refman/8.4/en/trigger-syntax.html

E o mesmo fecho da porta de saída:

> *«`START TRANSACTION` cannot be used within a stored function or trigger.»*
> — https://dev.mysql.com/doc/refman/8.4/en/stored-program-restrictions.html

### 3.5 SQLite — a prova está no fonte, não no manual

O manual do `CREATE TRIGGER` não afirma nada sobre transação (procurei; ele fala
de `RAISE(ROLLBACK|ABORT|FAIL|IGNORE)` e de `BEFORE`/`AFTER` e nada mais —
https://www.sqlite.org/lang_createtrigger.html). O fonte responde sem ambiguidade:
`src/trigger.c` compila o corpo do gatilho como um **sub-programa da VDBE** e o
chama de dentro do programa da instrução:

- `codeRowTrigger()` — *«Create and populate a new TriggerPrg object with a
  sub-program implementing trigger pTrigger»* (linha **1234**);
- `sqlite3CodeRowTriggerDirect()` — *«Code the `OP_Program` opcode in the parent
  VDBE. P4 of the `OP_Program` is a pointer to the sub-vdbe containing the
  trigger program.»* (linhas **1415-1421**).

Fonte: https://raw.githubusercontent.com/sqlite/sqlite/master/src/trigger.c

Corpo de gatilho no SQLite **é a mesma instrução**. Não existe lugar para ele
gravar que não seja a transação corrente.

### 3.6 Apache Cassandra® — a base de conhecimento permanente converge junto

Sem transação nenhuma, o Cassandra chega ao mesmo contrato pelo outro lado:

```java
/**
 * Called exactly once per CF update, returned mutations are atomically updated.
 *
 * @param update - update received for the CF
 * @return additional modifications to be applied along with the supplied update
 */
public Collection<Mutation> augment(Partition update);
```

— `src/java/org/apache/cassandra/triggers/ITrigger.java`, ramo `cassandra-5.0`:
https://raw.githubusercontent.com/apache/cassandra/cassandra-5.0/src/java/org/apache/cassandra/triggers/ITrigger.java

E o executor concatena as mutações do gatilho com as originais antes de aplicar
(`TriggerExecutor.java:147`, `mergeMutations(Iterables.concat(originalMutations,
augmentedMutations))`), recusando explicitamente o caso em que a atomicidade não
pode ser dada: *«Counter mutations and trigger mutations cannot be applied
together atomically»* (linha **142**).

**Cinco motores independentes, quatro famílias, um único contrato:** *o que o
gatilho grava é aplicado atomicamente com a escrita que o disparou.* Nenhum
deles oferece a alternativa que o PhxSql pratica hoje (gravar num lugar que será
descartado) nem a alternativa (b) (gravar fora, em unidade separada).

---

## 4. O veredito pela régua da casa

**Camada 1 — convergência.** Os três maduros convergem, e o quarto junto:
*a gravação do gatilho pertence à transação que o disparou*. Pela ordem do dono
de 11/09/2026, convergência dos três maduros é **verdade absoluta e aceite
automático, sem perguntar** — desde que nada nosso se oponha.

**Camada 2 — a régua ponderada não chega a ser usada**, e o motivo é medido:
não há dois lados. Se alguém insistir em aplicá-la à pergunta estreita («o
gatilho pode disparar no commit?»), o PostgreSQL vota sozinho (**4 a 0**), e o
voto dele é *«pode, e nesse caso ele roda ANTES do registro durável»*. Os outros
três não votam porque não têm o mecanismo. O resultado é o mesmo pelos dois
caminhos.

**Camada 3 — o choque com pétrea.** Passei as três que o pedido nomeia:

| pétrea | se opõe? | por quê |
|---|---|---|
| «só existe filho se o pai existir primeiro» | **não, e ela EMPURRA para (a)** | sob (a), a gravação do gatilho entra na mesma passada, e a passada já empresta as mães abertas (`servidor.rs:14249`, `MaesAbertas`) — é o conserto P0 que o teste `p0_pai_empilhado_e_visivel_a_fk_da_filha_no_mesmo_commit` (`servidor.rs:34946`) trava. Sob (b), a filha gravada pelo gatilho seria conferida contra o disco, fora da transação, e voltaria a ser o defeito de 11/09 |
| *read-your-own-writes* do pedido 162 | **não** | é a mesma família: hoje a leitura enxerga o empilhado e a **gravação do gatilho** não chega a existir |
| «não se entrega meia garantia» | **opõe-se a (b)** | ver §5.2: (b) abre uma janela de queda entre duas unidades atômicas |
| zero dependências externas | **não** | nada aqui pede biblioteca |
| a ordem de digitação do `.reg` | **não** | medido em §2.5: o rowid reservado é devolvido, sem buraco |

**Conclusão da régua:** o comportamento entra por aceite automático. O **meio** é
nosso, e é onde mora todo o custo.

---

## 5. As três saídas do pedido 262, cada uma com o seu número

### 5.1 (a) «a sessão sai de `Confirmando` antes dos AFTER» — CERTA no destino, e o nome dela é outro

Sair de `Confirmando` não basta e não é o que o PostgreSQL faz. O que ele faz é
**fechar o conjunto de escrita só DEPOIS de os gatilhos rodarem**, num laço, e
só então registrar o commit. Traduzido para a nossa máquina:

1. tirar a lista da transação (`servidor.rs:14116`) **sem** encerrá-la;
2. rodar os `AFTER` de cada escrita da lista, **antes** de `gravar_marca`, com a
   sessão num estado que aceite `empilhar`;
3. repetir enquanto os gatilhos empilharem escritas novas (o teto já existe:
   `CADEIA_MAXIMA = 8`, `servidor.rs:23067`);
4. `gravar_marca` com a lista **completa**, e então a passada.

**O obstáculo, medido hoje e não raciocinado.** O `AFTER` precisa do `NEW` «como
a linha FICOU gravada», e hoje ele sai de `t.ler(e.rowid)` **depois** da passada
(`servidor.rs:14300`). Antes da passada a linha não está pronta — a coluna
`Sequence`, o `padrao`, a coluna calculada e o `CHECK` são aplicados dentro de
`Table::inserir_com_maes_opt` (`crates/phxsql-store/src/table.rs:2962-2999`).
Medido pelo soquete, numa tabela com `id` do tipo `Sequence`:

| momento | a linha lida |
|---|---|
| **fora** de transação, `NEW` visto pelo gatilho | `evento = "id=1 nome=Ana"` — o número existe |
| **dentro** da transação, antes do `COMMIT` | `{"rowid":2,"id":null,"nome":"Bia","rownum":null}` |
| depois do `COMMIT` | `{"rowid":2,"id":2,"nome":"Bia","rownum":2}` |

Rodar o `AFTER` antes da passada **hoje** gravaria uma auditoria com `id=null` —
trocar uma perda silenciosa por um dado errado, que é pior.

**E a boa notícia é que o conserto disso já está escrito nesta casa, noutro
documento e por outro motivo.** O `docs/AUTONUMBER.md` §B.4 lista, como item
**5** da ordem de entrada: *«numerar no `INSERT`, dentro da transação — reserva
na `Escrita` + devolução — não muda formato, não quebra cliente»*. E o §«2. E o
preço disso é a ficha mestre-detalhe» descreve exatamente o sintoma que eu medi
acima. **Esses dois pedidos são o mesmo pedido visto de dois lados**, e o 262
depende do 5 do AUTONUMBER: numerado no `INSERT`, o `NEW` existe antes da marca,
e o gatilho pode rodar onde o PostgreSQL o roda.

Custo residual de (a), **raciocinado e não medido** (o que o decidiria está na
§7):

- a trava de dados é tomada no topo do `op_commit` (`servidor.rs:14089`) e o
  `empilhar` toma a sua própria: o laço dos `AFTER` precisa rodar **antes** do
  `travar_dados`, ou o abraço mortal já documentado em `servidor.rs:14338`
  acontece de novo;
- `gravadas` passaria a contar as linhas do gatilho (de `1` para `2` no caso
  medido) — é mudança visível ao cliente, e por isso é decisão de quem manda no
  contrato;
- falha de `AFTER` dentro da transação passa a poder **abortar o commit**, que é
  a semântica dos quatro; hoje ela é aviso. A justificativa escrita hoje em
  `servidor.rs:15970-15975` — *«a escrita já aconteceu e não há transação que a
  desfaça»* — vale para o caminho **sem** transação e **não** vale dentro de uma;
- custo para quem não usa gatilho: **zero**, e isso já está garantido pelo
  `ha_gatilhos` (`AtomicBool`, lido em `servidor.rs:14221`).

### 5.2 (b) «o gatilho escreve fora da transação que o disparou» — RECUSADA, com o número

Tem uma vantagem real e barata: seria quase um reordenamento (mover o
`descartar_transacao` de `servidor.rs:14189` para antes do laço de gatilhos),
e o comportamento ficaria igual ao caminho sem transação, que já funciona (M1).

**Recusada por três medidas:**

1. **Zero dos cinco motores faz isso.** PostgreSQL, MySQL, MariaDB e SQLite
   proíbem o corpo de gatilho de commitar por conta própria (§3.3, §3.4); o
   Cassandra funde as mutações em vez de separá-las (§3.6). Recusar unanimidade
   de cinco exige uma pétrea nossa, e não há nenhuma aqui.
2. **Abre uma janela de queda que hoje não existe dentro da transação.** Entre o
   `fsync` da marca (`servidor.rs:14140`) e a gravação da auditoria haveria duas
   unidades atômicas distintas: quem cair no meio acorda com a venda gravada e a
   auditoria não — e a recuperação **não tem como saber**, porque a marca não
   fala dela. É meia garantia entregue como inteira.
3. **Reabre o defeito de 11/09 para o gatilho que grava filha.** Fora da
   transação, `Table::conferir_fks` lê a mãe do disco. A mãe está no disco nesse
   instante (a passada já correu), mas a *cascata* e as travas da transação já
   não valem — e o escopo efetivo, que hoje inclui a tabela do gatilho (§2.4),
   deixaria de significar coisa alguma.

### 5.3 (c) «é contrato a documentar, não defeito» — VERDADEIRA como medida de hoje, FALSA como destino

É verdadeira num ponto que a régua não alcança: **o PhxSql já diverge dos quatro
em QUANDO o `AFTER` dispara**, e essa divergência é legítima e é nossa. Nos
quatro motores o `AFTER` comum roda no fim da instrução; aqui a instrução não
grava nada — ela empilha (`servidor.rs:12971-12981`) —, então não existe «fim da
instrução» onde o `AFTER` possa rodar. Isso não é cópia mal feita: é
consequência de o nosso conjunto de escrita ir inteiro para uma marca
idempotente.

Mas contrato **não** pode ser «some calado». O que o motor tem de avisar, e é
o mínimo enquanto (a) não entra:

1. **`empilhar` recusa quando o estado é `Confirmando`**, nomeando a causa —
   algo como *«este `inserir` veio de um gatilho AFTER disparado no COMMIT: a
   transação já está confirmando e não aceita escrita nova. Grave a auditoria
   fora da transação, ou aguarde o pedido 262»*. A recusa vira `gatilhos_avisos`
   pelo caminho que já existe (`servidor.rs:16039-16041`), **sem** derrubar o
   commit;
2. a `CREATE TRIGGER` **avisa na criação** quando o corpo de um `AFTER` grava,
   porque a informação já existe ali: `escopo_por_gatilho` (`servidor.rs:12727`)
   tira os alvos da árvore compilada, e não do texto;
3. `docs/TRIGGERS.md` §8 e `docs/TRANSACOES.md:415` são corrigidos no mesmo
   passo — a primeira ainda diz **«não há transação no PhxSql»**, que é falso
   desde o pedido 162, e a segunda afirma que a gravação do gatilho «acontece de
   verdade».

O ganho de (c) é o que o `CLAUDE.md` chama de a diferença entre as duas formas:
uma recusa nomeada custa um aviso lido; uma gravação sumida custa uma auditoria
que ninguém sabe que não existe.

---

## 6. A recomendação

**(c) agora, (a) como destino, e as duas no mesmo pedido — nunca (b).**

| ordem | o que | por quê, com o número |
|---|---|---|
| 1 | **(c) o aviso**: `empilhar` recusa em `Confirmando` e a recusa vira `gatilhos_avisos` | o motor hoje é *calado no sucesso e barulhento no erro* (§2.3, medido). Inverter isso é barato e não muda dado nenhum |
| 2 | **item 5 do `docs/AUTONUMBER.md` §B.4** — numerar no `INSERT`, dentro da transação | é o bloqueio medido de (a): `"id": null` dentro da transação (§5.1). O próprio documento o classifica como *não muda formato, não quebra cliente* |
| 3 | **(a) o laço à PostgreSQL**: `AFTER` antes da marca, conjunto fechado depois deles | convergência unânime de cinco motores (§3), e a metade da máquina já existe — o escopo efetivo já inclui a tabela do gatilho e a trava já está tomada (§2.4, medido) |

**Guarda nova entra pedida, não imposta**: o passo 1 muda uma resposta que hoje
é `ok` silencioso para `ok` **com aviso** — nenhum cliente que hoje funciona
passa a falhar, porque hoje nenhum cliente recebe a linha de auditoria de
qualquer jeito. O passo 3 é que muda `gravadas` e pode abortar commit, e por
isso ele quer decisão do dono antes de entrar.

### 6.1 Onde a nossa lógica DIVERGE da do PostgreSQL, e qual restrição causa a divergência

A lei da casa cobra esta seção: se não diverge em lugar nenhum, não passou pela
nossa cabeça.

| ponto | PostgreSQL | PhxSql, e a restrição nossa |
|---|---|---|
| quando o `AFTER` comum dispara | fim da **instrução** | fim do **COMMIT** — a instrução não grava, ela empilha (`servidor.rs:12971`), porque a marca `.tx` precisa da lista inteira para ser idempotente |
| o que é o «registro do commit» | `RecordTransactionCommit()` **depois** de tudo | `gravar_marca` **antes** da passada — é uma marca de *intenção* redo, não um registro de conclusão. Daí a inversão que o 262 expõe |
| `DEFERRABLE` como escolha do usuário | existe, por gatilho | **não entra**: aqui todo `AFTER` já é adiado ao commit por consequência do desenho, e um segundo eixo de adiamento seria ajuste sem caso de uso |
| falha de `AFTER` | derruba a transação, sempre | **fora** da transação continua sendo aviso (a escrita já aconteceu e não há o que desfazer); **dentro**, passa a poder abortar. A divergência é por contexto, e é medida: hoje são dois caminhos diferentes, não um |
| sequência consumida no `INSERT` | sim, e **não devolve** no rollback | aqui a reserva **se devolve** (`docs/AUTONUMBER.md` §«1. O `ROLLBACK` devolve o número»), porque numeração de documento fiscal não aceita buraco. O item 5 do B.4 mantém a devolução |

---

## 7. Lacunas — o que eu **não** pude medir

| lacuna | o que decidiria na bancada |
|---|---|
| o custo em tempo do laço de `AFTER` antes da marca | uma corrida de `bancada/transacoes` com 1, 10 e 1.000 linhas por commit, com e sem gatilho, comparando o `ms` do `COMMIT`. Hoje todos os meus commits deram **0-1 ms** e esse relógio não separa nada |
| se rodar o `AFTER` fora da trava de dados, antes do `travar_dados`, abre corrida com outra conexão | duas conexões de verdade, uma commitando com gatilho e a outra gravando na tabela da auditoria, com o `LOCK TIMEOUT` medido em relógio de parede — é o molde do `bancada/transacoes/provar.py` |
| quanto do `Table::inserir_com_maes_opt` é «preparar a linha» e quanto é «gravar» | contar as linhas de cada metade e medir o custo de cada uma com `--example onde-doi`; é o que diz se o item 5 do AUTONUMBER é barato mesmo |
| se algum cliente hoje depende do silêncio | varredura do `MANUAL.txt`, do `docs/TRIGGERS.md` e da interface web por promessa de auditoria transacional. Não fiz |
| MySQL/MariaDB no **fonte** (só usei o manual do fabricante) | `sql/sql_trigger.cc` e `sql/sp_head.cc`; o manual foi considerado fonte primária por ser do próprio fabricante, e as duas frases citadas são inequívocas |

---

## 8. O que foi avaliado e RECUSADO, com o número

Esta seção existe para a mesma proposta não voltar sem medição.

| recusado | número / citação que recusa |
|---|---|
| **A premissa do pedido 262** («os três motores divergem») | os quatro **convergem**: PostgreSQL *«a trigger is executed as part of the same transaction as the statement that triggered it»*; MariaDB *«triggers are executed in the same transaction as the statement that invoked them»*; MySQL *«trigger failure also causes rollback»*; SQLite: `OP_Program`, sub-programa da mesma instrução |
| **Usar a régua ponderada aqui** | não há dois lados: 3 dos 4 não têm gatilho de commit-time. A régua serve para divergência, e não há |
| **(b) gatilho escreve fora da transação** | 0 de 5 motores; abre janela de queda entre duas unidades atômicas (§5.2) |
| **Rodar o `AFTER` antes da passada HOJE, sem o item 5 do AUTONUMBER** | medido: a linha empilhada lê `{"id":null,"rownum":null}` — a auditoria sairia com `id` nulo |
| **Culpar o `upsert` (pedido 245)** | medido: acontece em `AFTER INSERT` simples (M2) e em `AFTER UPDATE` (M4), sem upsert nenhum. O 262 é pedido próprio, como o texto dele já dizia |
| **Suspeitar de corrupção de rowid / buraco no `.reg`** | medido: a inserção real seguinte em `auditoria` saiu com **rowid 2**, sem buraco — a reserva do `empilhar` fantasma é devolvida |
| **Achar que o `SCOPE STRICT` protegeria** | medido: `tabelas_efetivas = ['loja/auditoria','loja/clientes']` declarando só `clientes` — o fecho transitivo pelo gatilho já põe a auditoria no escopo, e a escrita some do mesmo jeito |

---

## 9. Como refazer a medição na próxima sessão

Sem isto o documento envelhece. O roteiro inteiro é o protocolo de sempre, pelo
soquete, com o servidor de verdade:

```
# 1. binario novo -- medidor com binario velho mede o passado
cargo build --release -p phxsql-server --bin phxsqld --offline

# 2. um config isolado (base propria, porta livre, web desligado),
#    no molde de bancada/transacoes/provar.py -- Ligacao/fala/morrer

# 3. a sonda, em sete pedidos:
{"op":"criar_database","database":"loja"}
{"op":"criar_tabela","database":"loja","tabela":"clientes",  ...}   # id Int8 obrigatoria + pk
{"op":"criar_tabela","database":"loja","tabela":"auditoria", ...}
{"op":"sql","database":"loja","texto":"CREATE TRIGGER audita AFTER INSERT ON clientes
   FOR EACH ROW INSERT INTO auditoria (id, evento) VALUES (NEW.id, 'entrou')"}
{"op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"Ana"}}  # controle
{"op":"begin"} {"op":"inserir", ...} {"op":"commit"}                                  # o defeito
{"op":"varrer","database":"loja","tabela":"auditoria","max":100}
```

Três armadilhas que **eu** paguei montando isto, e que a próxima sessão não
precisa pagar:

- a resposta vem embrulhada: as linhas estão em `resultado.linhas`, e não no
  topo. Ler `r["linhas"]` devolve `[]` **para tudo** e faz o controle parecer
  quebrado junto com o defeito;
- o escopo declarado do `begin` chama-se **`scope`** (ou `escopo`), nunca
  `tabelas` — `lista_do_escopo`, `servidor.rs:22722`. Mandar `tabelas` não dá
  erro: o escopo simplesmente não existe, e o `STRICT` parece frouxo;
- o `padrao` de uma coluna é **expressão**: `"SC"` é lido como nome de coluna e
  a criação é recusada; o literal é `"'SC'"`. E a coluna `Sequence` que entra na
  primária precisa de `"obrigatoria": true`.

Os arquivos de sonda desta rodada são temporários e **não** foram versionados,
por decisão: o valor está no roteiro acima, que se refaz em cinco minutos, e não
num script que envelheceria calado ao lado de uma bateria que ninguém roda.
