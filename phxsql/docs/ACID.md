# ACID no PhxSql — as quatro letras, medidas

> **A decisão sobre a frase da marca é do dono.** Este documento não decide:
> ele mede, letra por letra, e põe as opções na mesa com o custo de cada uma
> (§7). Até essa decisão, **continua valendo não escrever *ACID compliant* em
> documento técnico**.

Todo número desta página sai de `bancada/acid/prova.py`, e é
`bancada/acid/gerar-secoes.py` que os escreve aqui dentro — os blocos entre
`<!-- GERADO: … -->` e `<!-- FIM: … -->` **não se editam à mão**. O texto fora
deles é escrito à mão e o gerador não o toca.

<!-- GERADO: maquina -->
Medido contra `phxsqld 0.18.0 (41e82efa97c8) x86_64-unknown-linux-gnu`. Havia outra medição em curso na máquina no momento: **sim** — e isso não muda nenhum número desta página, porque nenhum deles é uma duração.
<!-- FIM: maquina -->

---

## 0. A lei que este documento vem remedir, e por que ela envelheceu

O `CLAUDE.md` traz a lei em vigor, e ela está datada:

> *«A folha de marca afirma ACID compliant e built-in replication. O segundo
> virou verdade… O primeiro continua falso, e continuará **enquanto não houver
> transação**: sem ela não há o A nem o I do ACID.»*

**A premissa caducou.** Há transação — `BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`,
com escopo, prazos e travas (`docs/TRANSACOES.md`) — e desde o pedido 162 ela
enxerga a própria escrita. A pergunta deixou de ser «há transação?» e passou a
ser **«o que cada letra garante, medido?»**.

A resposta não é «sim» nem «não» para nenhuma das quatro. É esta:

| letra | o que o motor **garante** | o que ele **não** garante | onde a configuração muda |
|---|---|---|---|
| **A** | o conjunto de escrita é aplicado inteiro ou não é aplicado; o `ROLLBACK` não consome slot, rowid nem evento; uma queda no meio da passada é **completada** no arranque pela marca `.tx` | a **cascata** do `ao_alterar` grava em tabela que a transação não declarou, e uma queda no meio dela pode deixar mãe e filhas divergentes — hoje **denunciado ou consertado**, nunca silencioso (§2.4) | nada: a marca `.tx` sincroniza nos três regimes |
| **C** | tipo, tamanho, obrigatoriedade, unicidade e **integridade referencial** são impostos na gravação, em toda porta local; «nunca se mata o pai que tem filhos» vale de vez e suave | a réplica **aplica, não julga** — ela não confere o que o outro servidor já julgou; `SET NULL` não existe e não vem; a falta do índice da chave é recusada na **gravação**, não na declaração | `"verificar": false` na chave desliga a conferência daquela chave, e é escolha escrita |
| **I** | leitura suja **não acontece**; a transação vê a própria escrita; uma **instrução** lê um estado consistente; escrita contra escrita é serializada por linha | **leitura repetível não existe**: entre duas instruções tudo pode mudar. Fantasma, leitura não repetível e **skew de escrita** acontecem, e estão medidos | nada — nenhum ajuste compra leitura repetível hoje |
| **D** | a marca `.tx` é sincronizada **antes** da passada e é o ponto de compromisso; um `COMMIT` que respondeu OK volta depois da queda nos três regimes | em `por_lote` (o padrão) e em `sistema`, uma escrita **comum** responde OK sem nenhum `fsync`; quem abre mão é quem configurou | `recursos.durabilidade`, e é o campo que mais muda o significado de «OK» |

O nome que o próprio servidor devolve em `transaction_isolation`, e ele
continua exato:

> *escrita serializável por tabela, leitura confirmada e não bloqueante, sem
> leitura repetível.*

---

## 1. Como cada afirmação foi provada

Trinta e duas afirmações, **cada uma com o controle da mesma corrida**. A regra
está em `bancada/acid/LEIA-ME.md` e é curta: um fenômeno só se prova
acontecendo; o que **não** acontece só vale com o instrumento acusando o caso
oposto ao lado. Esta casa já publicou um zero com um medidor cego.

A tabela abaixo é o **transcrito** do que a corrida mediu, copiado de
`resultado.json` palavra por palavra e sem acento — é evidência, e não prosa.
Reescrevê-la para ficar bonita seria pôr uma mão entre a medição e a página,
que é exatamente a mão que este documento não tem.

<!-- GERADO: afirmacoes -->
| letra | o que se afirma | controle da mesma corrida | confere |
|---|---|---|---|
| **A** | o ROLLBACK de tres INSERT nao consome slot nenhum do `.reg` | o COMMIT das mesmas tres consome 3 (1 -> 4) | sim |
| **A** | o controle: o COMMIT das mesmas tres linhas consome 3 slots | — | sim |
| **A** | em 7 quedas validas no meio de um COMMIT de duas tabelas, nenhuma deixou uma tabela com linha e a outra sem | a varredura pegou P2 · P3 · P4 -- pontos de morte diferentes, e e isso que prova que ela mirou dentro da janela | sim |
| **A** | com avo<-mae(cascata)<-neta(restringir), trocar a chave da avo e recusado e a AVO fica no valor antigo | a recusa e nomeada: INTEGRIDADE | sim |
| **A** | o controle: tirada a neta, a mesma alteracao passa e a MAE acompanha a avo | — | sim |
| **A** | nenhuma queda no meio da cascata deixou mae e filhas divergentes SEM o relatorio do arranque denunciar | a corrida achou 0 cascata(s) parcial(is) denunciada(s) em 7, e 7 consistente(s) -- o instrumento distingue os dois | sim |
| **C** | indice unico recusa chave repetida | o caso legitimo passa na mesma tabela; a recusa e DUPLICADO | sim |
| **C** | a filha sem mae e recusada na GRAVACAO, nao so declarada | o caso legitimo passa na mesma tabela; a recusa e INTEGRIDADE | sim |
| **C** | coluna obrigatoria recusa NULL | o caso legitimo passa na mesma tabela; a recusa e TIPO_INVALIDO | sim |
| **C** | texto numa coluna inteira e recusado | o caso legitimo passa na mesma tabela; a recusa e TIPO_INVALIDO | sim |
| **C** | texto que nao cabe em Str(20) e recusado, nunca truncado | o caso legitimo passa na mesma tabela; a recusa e LIMITE_EXCEDIDO | sim |
| **C** | excluir a mae que tem filha e recusado -- de vez E suave | a mae SEM filha (rowid 2) e excluida na mesma corrida | sim |
| **C** | filha marcada continua restringindo a mae, e mae marcada nao aceita filha nova | a mae SEM filha nenhuma sai de vez na mesma corrida | sim |
| **C** | chave declarada sem pedir `verificar` JA confere; com `verificar: false` a orfa entra | o mesmo INSERT orfao, nas duas tabelas, da os dois desfechos -- o instrumento nao esta recusando tudo | sim |
| **C** | chave sem indice na filha e ACEITA na declaracao e recusada no `excluir` da mae, nomeando o indice que falta | a mesma mae, com indice na filha (`maes`), recusa com o texto da regra primordial e nao com este | sim |
| **C** | `ao_excluir: cascata` e recusado na DECLARACAO; `restringir` passa | a mesma tabela, so trocando a acao, nasce ou nao nasce | sim |
| **I** | outra sessao NAO ve a escrita nao confirmada | a PROPRIA transacao ve 999 na mesma corrida -- o instrumento enxerga escrita nao gravada quando ela e dela | sim |
| **I** | duas leituras da MESMA linha dentro da mesma transacao devolvem valores diferentes | a primeira leu 50 e a segunda 77, com um COMMIT de outra sessao no meio | sim |
| **I** | a mesma varredura, repetida dentro da transacao, devolve uma linha que nao existia na primeira | 2 -> 3 linhas, com um INSERT de outra sessao no meio | sim |
| **I** | duas escritas SEM `versao` e SEM transacao: as duas leram 10 e somaram 1, e o final e 11 em vez de 12 | a MESMA sequencia com `versao` e recusada (CONFLITO), e com transacao a segunda espera o LOCK TIMEOUT e desiste (EM_TRANSACAO) | sim |
| **I** | o controle da trava: a linha 2 continua gravavel enquanto a transacao segura a linha 1 | — | sim |
| **I** | as duas viram 2 de plantao, cada uma tirou a sua, e no fim sobraram 0 | as duas transacoes CONFIRMARAM (nenhuma foi recusada) -- as travas sao por linha e as linhas eram diferentes | sim |
| **I** | dentro da transacao a MAE ja aparece com a chave nova e a FILHA ainda aponta para a antiga; o COMMIT acerta as duas | a mae muda dentro (42) e a filha nao (1) -- se a sobreposicao estivesse desligada, a mae tambem nao mudaria | sim |
| **I** | a corrida nao foi vazia: o escritor deu voltas enquanto o leitor perguntava | escritor solto 302 voltas / leitor 400; escritor em transacao 224 / leitor 400 | sim |
| **I** | uma varredura unica ENXERGA o estado entre as duas escritas quando o escritor nao usa transacao | 97 de 400 voltas -- e nao e defeito do leitor: o banco esta mesmo inconsistente ali, porque o escritor deixou as duas linhas fora de acordo | sim |
| **I** | com o escritor em transacao, a MESMA varredura nunca mais ve o estado intermediario | o mesmo instrumento, na mesma tabela, viu 97 vez(es) contra o escritor solto -- a diferenca e a transacao, e nada mais | sim |
| **I** | duas leituras separadas veem o par inconsistente MESMO contra um escritor em transacao -- e a leitura repetivel que falta | 68 de 400 voltas; o COMMIT e atomico, mas ele acontece INTEIRO entre a primeira leitura e a segunda | sim |
| **D** | em `por_operacao`, um INSERT que respondeu OK ja mandou o `.reg` ao disco | na mesma medicao, `por_lote` da 0 e `sistema` da 0 -- o contador distingue os regimes | sim |
| **D** | em `por_lote` (o padrao) e em `sistema`, o mesmo INSERT responde OK sem nenhum `fsync` no `.reg` | `por_operacao` deu 1 na mesma corrida | sim |
| **D** | o `fsync` da marca `.tx` acontece nos tres regimes -- ele nao olha `recursos.durabilidade` | contados [1, 1, 1] (por_operacao, por_lote, sistema); no MESMO commit o `.reg` sai [1, 0, 0] -- e a diferenca entre os dois que mostra que o regime so decide a tabela | sim |
| **D** | matar o processo logo depois de um COMMIT que respondeu OK deixa as tres linhas la, nos tres regimes | em `sistema` nenhum `fsync` de tabela aconteceu, e as linhas voltaram assim mesmo -- pela marca, e nao pelo disco da tabela | sim |
| **D** | duas insercoes comuns em `por_lote`, sem nenhum `fsync` no `.reg`, sobrevivem ao SIGKILL | a contagem de `fsync` da mesma configuracao (D1) e ZERO no `.reg` -- as linhas voltaram do cache do nucleo, nao do disco. O SIGKILL nao distingue os dois; so queda de energia distinguiria | sim |

**32 afirmações, 0 sem confirmar.** Medidas contra `phxsqld 0.18.0 (41e82efa97c8) x86_64-unknown-linux-gnu`.
<!-- FIM: afirmacoes -->

---

## 2. A — atomicidade

### 2.1 O que o `COMMIT` promete, e o que a marca `.tx` promete

O desenho está em `docs/TRANSACOES.md` §3 e §5, e cabe em duas frases: **nada
vai a disco antes do `COMMIT`**, e antes de a passada de commit tocar qualquer
arquivo o conjunto de escrita inteiro é gravado e **sincronizado** numa marca
`transacao_<id>.tx` dentro do diretório do database.

O que a marca **promete**:

* que a pergunta do contrato — *«depois de reiniciar, o banco determina de
  forma inequívoca se esta transação foi COMMITTED ou ABORTED?»* — tem resposta
  em todos os instantes (`docs/TRANSACOES.md` §5.4);
* que a reaplicação é **idempotente pelo rowid**: cada operação diz o slot que
  devia ter escrito e o conteúdo, e slot já ativo passa adiante;
* que a recuperação anda **para a frente**, nunca para trás — desfazer exigiria
  devolver slots gravados, e a ordem de digitação é sagrada;
* desde a **v2**, que a **linha antiga** do `atualizar` viaja junto, para que a
  reaplicação consiga replanejar a cascata (`Table::recascatear`).

O que ela **não** promete:

* **não** é um WAL de páginas: não há página suja confirmada para refazer, e
  não há *full-page-write*. Uma escrita rasgada no `.reg` é **detectável** pelo
  CRC-32 do slot e recuperável pelo espelho `.bkp`, não pela marca;
* **não** cobre DDL: `ALTER`/`CREATE` dentro de transação são **recusados**, e
  não silenciosamente confirmados (`docs/TRANSACOES.md` §11.4);
* **não** cobre mais de um database — isso é *two-phase commit*, recusado com o
  nome (§2.3 de lá);
* **não** cobre as escritas da **cascata**, que não entram no conjunto de
  escrita. É a §2.4 aqui.

### 2.2 O `ROLLBACK` não consome slot

A regra pétrea é que o `.reg` nunca reaproveita slot excluído. Daí o desenho de
«nada a disco antes do `COMMIT`»: o rollback de um `INSERT` é zero byte de
trabalho, porque o insert ainda não aconteceu. Medido:

<!-- GERADO: a-slots -->
| momento | slots do `.reg` |
|---|---:|
| antes de abrir a transação | 1 |
| depois de `BEGIN` + 3 `INSERT` + `ROLLBACK` | 1 |
| depois de `BEGIN` + as **mesmas** 3 + `COMMIT` | 4 |
<!-- FIM: a-slots -->

O controle é o `COMMIT` das **mesmas** três linhas na mesma corrida. Sem ele,
«os slots não mudaram» poderia ser um `esquema` que não conta nada.

### 2.3 Queda de verdade no meio de um `COMMIT` que toca duas tabelas

`SIGKILL` real, varredura de atrasos, e a pergunta não é «as N linhas estão
lá?» — é «o banco consegue dizer, sem ambiguidade, qual dos dois desfechos
aconteceu?». O que **reprova** é metade: uma tabela com linha e a outra sem.

<!-- GERADO: a-queda -->
| atraso do `SIGKILL` | onde a queda caiu | linhas em `a` | linhas em `b` |
|---|---|---:|---:|
| 0 | P2 nada aplicado (reaplicadas=800) | 400 | 400 |
| 1 | P2 nada aplicado (reaplicadas=800) | 400 | 400 |
| 2 | P2 nada aplicado (reaplicadas=800) | 400 | 400 |
| 3 | P3 parcial (reaplicadas=216 ja_aplicadas=584) | 400 | 400 |
| 4 | P3 parcial (reaplicadas=71 ja_aplicadas=729) | 400 | 400 |
| 5 | P3 parcial (reaplicadas=134 ja_aplicadas=666) | 400 | 400 |
| 6 | P4 tudo aplicado, marca pendente (ja_aplicadas=800) | 400 | 400 |

**7 quedas válidas, 0 com uma tabela gravada e a outra não.** Os desfechos não foram todos iguais — a varredura pegou pontos de morte diferentes, e é isso que prova que ela mirou dentro da janela.
<!-- FIM: a-queda -->

A varredura caminhou pela passada inteira — de «nada aplicado» a «tudo
aplicado», passando por cinco pontos intermediários — e em **nenhum** deles
uma tabela ficou gravada com a outra vazia. É a recuperação completando o que
faltava a partir da marca, e o relatório do arranque diz quanto ela reaplicou.

### 2.4 A cascata: o que era verdade, o que mudou, e o que sobra

O pedido 163 escreveu, e a frase virou lei citada: **«não há transação: a
cascata não é atômica»**. Ela envelheceu em duas metades, e as duas foram
medidas aqui.

**A metade que FECHOU: a recusa acontece antes da primeira escrita.** O pedido
169 pôs `Table::atualizar` a conferir a **árvore inteira** da cascata antes de
gravar, e o 173 fez o mesmo no `Table::recascatear` da recuperação. Provado por
soquete, com `avó ← mãe (cascata) ← neta (restringir)`: trocar a chave da avó é
recusado nomeando a neta, e **a avó fica no valor antigo**. O controle na mesma
corrida: apagada a neta de vez, a mesma alteração passa e a mãe acompanha.

**A metade que SOBRA: uma queda no meio da cascata.** `aplicar_ao_alterar`
reescreve as filhas uma a uma e sincroniza cada uma no fim do laço dela — fora
da janela de `recursos.durabilidade`, porque a cascata sincroniza por conta
própria. Uma queda ali pode deixar a mãe no valor novo e parte das filhas no
velho. Medido, com **os mesmos parâmetros** da matriz publicada em
`docs/TRANSACOES.md` §5.7 (1.200 filhas, 7 passos, `por_lote`), para que os
dois números sejam comparáveis:

<!-- GERADO: a-cascata -->
| corrida | veredito | filhas na chave nova | filhas na chave velha |
|---|---|---:|---:|
| 0 | CONSISTENTE | 1200 | 0 |
| 1 | CONSISTENTE | 1200 | 0 |
| 2 | CONSISTENTE | 1200 | 0 |
| 3 | CONSISTENTE | 1200 | 0 |
| 4 | CONSISTENTE | 1200 | 0 |
| 5 | CONSISTENTE | 1200 | 0 |
| 6 | CONSISTENTE | 1200 | 0 |

Vereditos: **7** CONSISTENTE
<!-- FIM: a-cascata -->

**E o número mudou.** Aquela matriz mediu, na linha `por_lote`, **4 de 7
consistentes e 3 de 7 parciais denunciadas** — e ela é de **antes** do pedido
172, que pôs a recuperação a reconstruir o `.ndx` da filha dentro do próprio
`completar()`, enquanto a marca ainda existe. Esta corrida, com os mesmos
parâmetros, saiu **consistente nas sete**. O conserto do 172 aparece no número.

O que **continua** verdadeiro, e não se apaga: a cascata escreve em tabela que
a transação não declarou, então o `ROLLBACK` não a alcança — e é por isso que
o **C** não está inteiro. A leitura honesta da frase do 163 hoje é: *a cascata
não é atômica por desenho; o que ela garante é que nada é gravado antes de a
árvore inteira ser conferida, e que uma queda no meio dela é **denunciada** no
relatório do arranque ou **consertada** por ele — nunca silenciosa*.

---

## 3. C — consistência

### 3.1 O que é imposto na GRAVAÇÃO

Cada linha abaixo é um par medido: a violação **recusada** e o caso legítimo
**aceito**, na mesma tabela e na mesma corrida. Guarda que recusa tudo
protegeria o mesmo número e não serviria para nada — é a mesma razão de
`ao_excluir_so_aceita_restringir` ter um irmão.

<!-- GERADO: c-guardas -->
| garantia | a violação | o caso legítimo, na mesma corrida |
|---|---|---|
| unicidade num índice único | `DUPLICADO` | aceito |
| chave estrangeira: filha sem mãe | `INTEGRIDADE` | aceito |
| coluna obrigatória com `NULL` | `TIPO_INVALIDO` | aceito |
| tipo da coluna | `TIPO_INVALIDO` | aceito |
| texto maior que a coluna | `LIMITE_EXCEDIDO` | aceito |
| **regra primordial**: matar a mãe que tem filha, de vez | `INTEGRIDADE` | mãe sem filha: aceita |
| **regra primordial**: matar a mãe que tem filha, suave | `INTEGRIDADE` | — |
| filha **marcada** ainda restringe a mãe | `INTEGRIDADE` | mãe sem filha nenhuma: aceita |
| mãe **marcada** não aceita filha nova | `INTEGRIDADE` | — |
| chave declarada **sem pedir** `verificar` já confere | `INTEGRIDADE` | com `"verificar": false` a órfã entra: sim |
| `"ao_excluir": "cascata"` na declaração | `ESQUEMA_INVALIDO` | `restringir` nasce: sim |
| chave conferida **sem o índice na filha** | declaração: `ACEITOU`; `excluir` da mãe: `INTEGRIDADE` | — |
<!-- FIM: c-guardas -->

Três leituras que essa tabela carrega e que não são óbvias:

**A marca de exclusão conta dos dois lados, e a assimetria é proposital.** Uma
filha logicamente morta **continua** restringindo a mãe, e uma mãe logicamente
morta **não** aceita filha nova. São perguntas diferentes — `conferir_filhas`
pergunta «alguém aponta para esta linha?» e `conferir_fks` pergunta «este pai
está **vivo**?» (pedido 171, §2.1 de `docs/INTEGRIDADE.md`) — e as duas
respostas seguem a mesma pétrea: *órfã que ninguém vê é pior que órfã que dá
erro*.

**A chave declarada nasce conferida, e o interruptor só existe para o outro
lado.** Quem quer declarar sem conferir manda `"verificar": false`, e aí é
escolha escrita em vez de omissão. Medido nos dois sentidos: o mesmo `INSERT`
órfão é recusado na chave que nasceu conferida e aceito na que pediu para não
conferir.

**A recusa muda de lugar conforme o quê.** `"ao_excluir": "cascata"` é recusado
na **declaração** — uma tabela nasce uma vez e grava um milhão de vezes, então
recusar cedo custa um erro lido enquanto se cria a tabela. Já a **falta do
índice** que a chave exige é aceita na declaração e recusada na **gravação**,
quando a mãe tenta morrer. Isso não é incoerência: é a ordem legítima *declare
a chave, crie o índice*, e as três saídas possíveis estão pesadas em
`docs/PARECER-175-INDICE-NA-DECLARACAO.md`, com a decisão pendente do dono.

### 3.2 O que **não** é imposto, e por decisão escrita

* **A réplica aplica, ela não julga.** As quatro portas que aplicam o que outro
  servidor já julgou — `aplicar_evento`, `inserir_replicado`,
  `atualizar_replicado`, `excluir_de_vez_replicado` — **não** conferem chave
  estrangeira, e isso é decisão, com o preço medido: quando *chave declarada
  nasce conferida* ligou o portão também ali, a réplica passou a **recusar** a
  filha que a origem já tinha aceitado, e `pedidos` ficou com **0 de 2**
  eventos. A guarda causava a perda de dado que existe para impedir. Está em
  `docs/INTEGRIDADE.md` §3, com as guardas do pedido 171. **Esta bancada não
  reprova isso**: ela sobe um servidor só, e afirmação sobre dois servidores se
  prova com dois — a fonte da prova fica nomeada em vez de refeita mal.
* **`copiar_tabela_para`** e **restaurar backup** não conferem, cada um com o
  motivo em `docs/INTEGRIDADE.md` §4.3 e §4.4.
* **`SET NULL` não existe e não vem**: anular a coluna da filha para poder
  matar a mãe é a cascata disfarçada que a regra primordial recusa.
* **Não há `CHECK`**, nem restrição de domínio além do tipo e do tamanho.

### 3.3 Onde o C não está inteiro, dito sem enfeite

A lacuna é uma, e mudou de nome duas vezes sem sumir: **a cascata escreve em
tabela que a transação não declarou** (`docs/TRANSACOES.md` §4.6), então um
`ROLLBACK` não alcança a filha e o escopo efetivo não a mostra. Enquanto isso
valer, o **C** é *imposto na gravação e não coberto pela transação*.

---

## 4. I — isolamento

Esta é a letra mais fácil de exagerar, e por isso é a que aqui vem com o nome
de cada fenômeno e a prova de cada um, por soquete.

### 4.1 Os fenômenos, um a um

<!-- GERADO: i-fenomenos -->
| fenômeno da norma | acontece? | como se mediu |
|---|---|---|
| **leitura suja** | **não** | outra sessão leu `50` enquanto a transação via `999` na própria escrita não confirmada |
| **leitura não repetível** | **sim** | duas leituras da mesma linha na mesma transação: `50` e depois `77` |
| **fantasma** | **sim** | a mesma varredura na mesma transação: 2 linhas e depois 3 |
| **perda de atualização** entre escritas soltas | **sim** | as duas leram `10`, as duas somaram 1, e o valor final é `11` em vez de `12` |
| a mesma, mandando `"versao"` | **não** | a segunda gravação volta `CONFLITO` |
| a mesma, dentro de transação | **não** | a segunda espera o `LOCK TIMEOUT` e volta `EM_TRANSACAO` |
| **skew de escrita** | **sim** | as duas transações viram 2 de plantão, cada uma tirou a sua linha, as duas confirmaram, e sobraram **0** |
<!-- FIM: i-fenomenos -->

A leitura suja é o único «não» da lista, e ele tem uma razão estrutural em vez
de uma guarda: **não existe dado não confirmado em lugar nenhum** — ele ainda
está em RAM, no conjunto de escrita, e a sobreposição que o torna visível está
presa à **conexão**. É essa fronteira que separa *read-your-own-writes* de
leitura suja.

### 4.2 O nível da norma

<!-- GERADO: i-nivel -->
> **READ COMMITTED**, e nada acima disso.

Os fenômenos que **acontecem** e que impedem o nível seguinte: **leitura não repetível**, **fantasma**. E o **skew de escrita**, que a leitura moderna cobra do `SERIALIZABLE`, acontece.
<!-- FIM: i-nivel -->

**Não é ANSI `SERIALIZABLE`**, e não pode ser chamado assim — a leitura
repetível não existe e o skew de escrita acontece. Entre **escritores**, a
serialização é real e por linha: a segunda escrita espera o `LOCK TIMEOUT` e
recebe um erro nomeado, ou, se for escrita comum sem transação, recebe
`4005 EM_TRANSACAO` com `repetir: true` na hora, sem esperar nada.

### 4.3 A matriz que responde o que a transação compra para quem LÊ

Duas linhas com a soma constante — 100 sai de uma e entra na outra — e um
escritor transferindo sem parar. O leitor pergunta de duas formas: **uma**
instrução (um `varrer` que devolve as duas linhas) e **duas** (`ler` + `ler`).
Conta-se quantas vezes a soma veio quebrada.

<!-- GERADO: i-matriz -->
| o leitor pergunta | escritor **sem** transação | escritor **em** transação |
|---|---:|---:|
| **uma** instrução (`varrer` devolve as duas linhas) | 97 de 400 |    0 de 400 |
| **duas** instruções (`ler` + `ler`) | 1 de 400 | 68 de 400 |

A corrida não foi vazia: o escritor deu **302** voltas na coluna da esquerda e **224** na da direita, contra 400 perguntas do leitor em cada.

E os **estados** que a instrução única viu contra o escritor sem transação, que é o número que separa «o leitor rasgou a leitura» de «o banco estava mesmo inconsistente»: `(49,51)` 154 · `(50,50)` 149 · `(49,50)` 49 · `(50,51)` 48. O escritor passa **uma** ida e volta em cada estado do meio da transferência e **três** em cada estado em acordo — a frequência tem de sair 3:1:3:1, e sai.
<!-- FIM: i-matriz -->

**Linha de cima — é o que a transação compra.** Sem ela, uma varredura única
enxerga o banco no meio da transferência em cerca de um quarto das perguntas, e
isso **não é defeito do leitor**: o banco está mesmo inconsistente ali, porque
o escritor deixou as duas linhas fora de acordo. Com a transação, o mesmo
instrumento, na mesma tabela, nunca mais vê aquele estado. A única diferença
entre as duas colunas é a transação.

**Linha de baixo — é o que falta.** Duas leituras separadas veem o par
inconsistente **mesmo** contra um escritor em transação: o `COMMIT` é atômico,
mas ele acontece **inteiro** entre a primeira leitura e a segunda. É a leitura
repetível que não existe, medida sobre um invariante em vez de sobre uma linha
só.

E uma leitura que **não** se deve fazer dessa matriz: o número baixo da célula
de baixo à esquerda **não é garantia nenhuma**. O mesmo par de leituras quebra
dezenas de vezes na coluna ao lado, então o instrumento enxerga; ali ele é
baixo porque o ciclo do escritor solto é curto e os pedidos se alternam, e está
escrito aqui para ninguém o ler como proteção. Quem quiser a garantia de duas
leituras coerentes não a tem em regime nenhum — é a leitura repetível que não
existe.

### 4.4 Duas imprecisões nomeadas, e a terceira que esta rodada achou

As duas primeiras são do pedido 162 e continuam valendo: na ordem do **índice**
a linha pendente sai no fim (ela não está no `.ndx`), e `Sequence`/`rownum` só
nascem no `COMMIT`, então dentro da transação saem nulos.

**A terceira:** o *read-your-own-writes* **não alcança a cascata**. Dentro da
transação, alterar a chave da mãe faz a mãe já aparecer com a chave nova e a
filha **ainda apontar para a antiga**; o `COMMIT` acerta as duas. O mecanismo é
o mesmo da §3.3: a sobreposição é montada a partir do conjunto de escrita, e a
cascata nunca vira `Escrita`. Não é defeito novo — é o alcance da sobreposição,
e está medido para não voltar como surpresa.

---

## 5. D — durabilidade

### 5.1 O que cada regime promete

`recursos.durabilidade` tem três valores, e ele muda o significado de «OK»:

| regime | o que um `OK` de escrita quer dizer | quando a tabela vai ao disco |
|---|---|---|
| `por_operacao` | **está no disco** | dentro da própria operação |
| `por_lote` (**padrão**) | está no núcleo, e vai ao disco quando a janela fechar | ao fechar `lote_operacoes` (200) ou `lote_milissegundos` (200 ms), o que vier antes; o relógio de fundo fecha mesmo sem tráfego |
| `sistema` | está no núcleo, e vai ao disco quando o sistema operacional quiser | nunca por conta própria — só no próximo arranque |

E a marca `.tx` **não obedece a esse campo**: `gravar_marca` chama `sync_all`
incondicional. O regime decide quando a **tabela** sincroniza; quem decide se a
transação aconteceu é sempre a marca. Contado por `strace`, sem tempo nenhum:

<!-- GERADO: d-fsync -->
| operação | regime | `.tx` | `.reg` | `.ndx` | `.bin` | `.memo` | `.log` | `.trash` | `.reason` | total |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| um `INSERT` comum | `por_operacao` | 0 | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **8** |
| um `INSERT` comum | `por_lote` (**padrão**) | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |
| um `INSERT` comum | `sistema` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **0** |
| um `COMMIT` de uma linha | `por_operacao` | 1 | 1 | 2 | 1 | 1 | 1 | 1 | 1 | **9** |
| um `COMMIT` de uma linha | `por_lote` (**padrão**) | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **1** |
| um `COMMIT` de uma linha | `sistema` | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | **1** |
<!-- FIM: d-fsync -->

O controle está dentro da própria tabela: `por_operacao` mostra o `.reg` indo
ao disco, o que prova que o cano `strace → regex → contador` **enxerga** um
`.reg` sincronizado quando existe um. Sem essa célula, «zero `fsync` no `.reg`»
poderia ser o defeito ou poderia ser o instrumento surdo — que foi exatamente o
que escondeu o pedido 186 por tanto tempo.

### 5.2 O achado do pedido 186, e a lição que ele deixou

O fecho da janela **não sincronizava o `.reg`**: `Volumes::sincronizar`
percorria os arquivos abertos, e um `Table` recém-aberto nunca tinha tocado o
volume do `.reg`, porque `RegFile::abrir` lê o cabeçalho com um `File` direto,
fora do cache. O `reg.sincronizar()` devolvia `Ok(())` tendo mandado **zero**
`fsync`, e o `.ndx` ia ao disco duas vezes no mesmo fecho — índice durável
apontando para dado que não foi é pior que perder os dois. Consertado na onda 1;
a catraca `TETO_FSYNC_POR_FECHO` foi **aposentada** e nasceu a V2, no número
medido do dia.

**E a lição vale mais que o conserto: por que ele nunca apareceu.** A bateria
de durabilidade prova com `SIGKILL`, e **página suja no cache do núcleo
sobrevive a processo morto**. Toda a prova por queda passava, com o defeito de
pé, porque a prova não conseguia ver o defeito.

### 5.3 O que a queda prova, e o que ela não prova

<!-- GERADO: d-queda -->
| regime | linhas depois do `SIGKILL` | o relatório do arranque |
|---|---:|---|
| `por_operacao` | 3 | não saiu — não havia marca (a tabela já estava no disco) |
| `por_lote` (**padrão**) | 3 | achadas=1, completadas=1, reaplicadas=0, já aplicadas=3, impossíveis=0 |
| `sistema` | 3 | achadas=1, completadas=1, reaplicadas=0, já aplicadas=3, impossíveis=0 |

E o contraponto, que é o ponto desta seção: **2** inserções comuns em `por_lote`, com **zero** `fsync` no `.reg`, também voltaram inteiras depois do `SIGKILL`.
<!-- FIM: d-queda -->

**O que ela prova.** Um `COMMIT` que respondeu OK volta inteiro depois de o
processo morrer, nos três regimes — inclusive em `sistema`, onde nenhum `fsync`
de tabela aconteceu. Ele volta **pela marca**, e não pelo disco da tabela: é a
demonstração direta de que a marca é o ponto de compromisso.

**O que ela não prova, e isto vale mais que um veredito bonito.** Duas
inserções **comuns** em `por_lote`, com **zero** `fsync` no `.reg` medido na
mesma configuração, também voltaram inteiras depois do `SIGKILL`. Elas não
voltaram do disco — voltaram do cache do núcleo, que a morte do processo não
esvazia. **O `SIGKILL` não distingue «está na mídia» de «está no cache do
núcleo».** Só queda de energia distinguiria, e nenhum processo em espaço de
usuário provoca uma.

Daí a divisão de trabalho desta bancada, e ela é a resposta honesta à pergunta
da durabilidade:

* **quem mede durabilidade é a contagem de `fsync`** — determinística, imune a
  máquina ocupada, e a única que separa os três regimes;
* **o `SIGKILL` só prova o protocolo de commit** — que a marca decide o
  desfecho e que a recuperação completa o que faltava.

O que uma queda de energia arriscaria — páginas do `.ndx`/`.reg` que só
existiam no *write-back* do núcleo e nunca chegaram à mídia — continua
**descrito e não medido**, aqui como em `docs/TRANSACOES.md` §5.7 e
`docs/DESEMPENHO.md` §4.12. E a marca, por ser sincronizada sempre, continua
sendo o bilhete que traz o commit de volta mesmo que a tabela tenha perdido
bytes: a reaplicação é idempotente pelo rowid e reescreve o que faltar.

Os dois pontos onde nem essa rede fecha estão escritos e não escondidos: o slot
gravado e depois liberado por falha de E/S no índice (`operacoes IMPOSSIVEIS`,
`docs/TRANSACOES.md` §5.5) e o `.ndx` da filha sujo no meio de uma cascata
(§5.5.3, hoje reconstruído pela recuperação — ver §2.4 aqui).

---

## 6. O resumo em uma frase por letra

* **A — atomicidade: entregue.** O conjunto de escrita é tudo-ou-nada, o
  `ROLLBACK` não consome slot nem rowid, e uma queda no meio da passada é
  completada no arranque. A cascata fica fora do conjunto de escrita, e uma
  queda no meio dela é denunciada ou consertada, nunca silenciosa.
* **C — consistência: imposta na gravação, não coberta pela transação.** Tipo,
  tamanho, obrigatoriedade, unicidade e integridade referencial são conferidos
  em toda porta local de escrita, e «nunca se mata o pai que tem filhos» vale
  nos dois excluires. A réplica aplica e não julga, por decisão medida.
* **I — isolamento: leitura confirmada, sem leitura repetível.** `READ
  COMMITTED` pela norma, com escrita serializada por linha entre transações. A
  transação compra a consistência de **uma** instrução; entre duas instruções
  não há nada, e o skew de escrita acontece.
* **D — durabilidade: configurável, e o padrão não é «no disco».** A marca
  `.tx` é sincronizada sempre e é o ponto de compromisso da transação. Fora de
  transação, `por_lote` responde OK antes de o dado ir à mídia — e é escolha de
  quem configura.

---

## 7. A frase da marca: as opções, e o custo de cada uma

A folha de marca (`marca/`) afirma *ACID compliant*. **A decisão é do dono**;
o que segue é o custo medido de cada saída.

| opção | o que se ganha | o que custa |
|---|---|---|
| **(a) tirar a afirmação** | zero risco de contestação; nenhum documento precisa de nota de rodapé | perde-se uma palavra que o mercado procura, e que hoje é **em boa parte** verdade — atomicidade e durabilidade estão entregues e medidas |
| **(b) manter *ACID compliant* seco** | a palavra que o mercado procura | **é falso hoje**, e o ponto que o derruba não é opinião: `SERIALIZABLE` não existe, leitura repetível não existe, e o skew de escrita está medido acontecendo. Um comprador técnico que rodar esta bancada acha em cinco minutos |
| **(c) qualificar na própria frase** — *ACID com isolamento **read committed*** | verdadeiro, verificável, e é o que MySQL(R) e PostgreSQL(R) fazem no padrão deles | a frase fica mais longa; e obriga a manter a qualificação em todo lugar que a repetir |
| **(d) trocar por uma afirmação que é inteira** — p. ex. *transações atômicas e duráveis, integridade referencial imposta* | tudo o que se afirma está medido nesta página, letra por letra | não usa a sigla, então não casa com busca por «ACID» |

**A recomendação desta frente, e ela é recomendação e não decisão: (c) ou
(d).** As duas são verdadeiras hoje; (b) não é, e (a) joga fora mais do que
precisa. Entre as duas, (c) casa com o vocabulário do mercado e é o que os dois
grandes fazem; (d) é mais forte tecnicamente porque não pede nota de rodapé.

**O que continua falso em qualquer redação, e não pode aparecer:**
*SERIALIZABLE*, *snapshot isolation*, *MVCC*, *leitura repetível*, e *ACID
compliant* **sem** qualificação. O MVCC está recusado com o motivo em
`docs/TRANSACOES.md` §11.1 — aqui o rowid é endereço.

**O que a marca já pode afirmar sem ressalva nenhuma**, porque está medido
nesta página: *transações com `BEGIN`/`COMMIT`/`ROLLBACK` e `SAVEPOINT`*,
*commit atômico e recuperação automática no arranque*, *integridade referencial
imposta na gravação*, *durabilidade configurável*.

---

## 8. Como se prova

```bash
cargo build --release
python3 bancada/acid/prova.py          # mede, grava resultado.json
python3 bancada/acid/gerar-secoes.py   # reescreve os blocos deste documento
```

| o que | onde |
|---|---|
| as 32 afirmações desta página, com o controle de cada uma | `bancada/acid/prova.py`, e `bancada/acid/LEIA-ME.md` para o método |
| o desenho da transação e da marca `.tx` | `docs/TRANSACOES.md` |
| a matriz completa ponto de morte × regime | `docs/TRANSACOES.md` §5.7 e `bancada/durabilidade/prova.py` |
| a integridade referencial, porta por porta, derivada do código | `docs/INTEGRIDADE.md` |
| o que a concorrência entrega e o que ela não entrega | `docs/CONCORRENCIA.md` |
| as quatro armadilhas que esta bancada pagou antes de confiar num número | `bancada/acid/LEIA-ME.md` |
