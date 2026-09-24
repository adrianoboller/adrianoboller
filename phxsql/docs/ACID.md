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
| **A** | o conjunto de escrita é aplicado inteiro ou não é aplicado; o `ROLLBACK` não consome slot, rowid nem evento; uma queda no meio da passada é **completada** no arranque pela marca `.tx`; dentro da transação a **cascata** do `ao_alterar` entra no conjunto de escrita (ACID-C, §2.4) — o `ROLLBACK` a alcança e o `COMMIT` a conta; **fora** de transação, desde o pedido 540, a alteração solta que cascateia é uma transação de uma instrução — grava a marca antes, e queda, `SIGKILL` ou pânico no meio dela são **completados** (§2.4) | quem usa o `phxsql-store` embutido e chama `Table::atualizar` direto não tem marca: ali vale só o 490 (a filha recusa até o `reindexar`), e as filhas que ficaram na chave velha continuam lá | nada: a marca `.tx` sincroniza nos três regimes |
| **C** | tipo, tamanho, obrigatoriedade, unicidade e **integridade referencial** são impostos na gravação, em toda porta local; «nunca se mata o pai que tem filhos» vale de vez e suave | a réplica **aplica, não julga** — ela não confere o que o outro servidor já julgou; `SET NULL` não existe e não vem; a falta do índice da chave é recusada na **gravação**, não na declaração | `"verificar": false` na chave desliga a conferência daquela chave, e é escolha escrita |
| **I** | leitura suja **não acontece**; a transação vê a própria escrita; uma **instrução** lê um estado consistente; escrita contra escrita é serializada por linha; **desde 16/09/2026**, quem pedir `"leitura_repetivel": true` (ou `BEGIN ISOLATION LEVEL REPEATABLE READ`) ganha leitura repetível e ausência de fantasma, pela trava compartilhada (§4.5) | por padrão (sem pedir) **leitura repetível não existe**: entre duas instruções tudo pode mudar. Fantasma, leitura não repetível e **skew de escrita** acontecem nesse regime, e estão medidos; `SERIALIZABLE` não se reivindica em regime nenhum | `"leitura_repetivel": true` no `begin`, ou `ISOLATION LEVEL REPEATABLE READ` no `BEGIN` SQL |
| **D** | a marca `.tx` é sincronizada **antes** da passada e é o ponto de compromisso; um `COMMIT` que respondeu OK volta depois da queda nos três regimes | em `por_lote` (o padrão) e em `sistema`, uma escrita **comum** responde OK sem nenhum `fsync`; quem abre mão é quem configurou | `recursos.durabilidade`, e é o campo que mais muda o significado de «OK» |

O nome que o próprio servidor devolve em `transaction_isolation` muda com o
pedido. Por padrão, sem pedir leitura repetível, continua exato o texto de
sempre:

> *escrita serializável por tabela, leitura confirmada e não bloqueante, sem
> leitura repetível.*

Quem pediu `"leitura_repetivel": true` (ou `BEGIN ISOLATION LEVEL REPEATABLE
READ`) recebe, no mesmo campo, o nome do nível que está de fato valendo —
constantes `NIVEL_DE_ISOLAMENTO` e `NIVEL_DE_ISOLAMENTO_REPETIVEL`, em
`crates/phxsql-server/src/transacao.rs`. Ver §4.5.

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
| **A** | em 7 quedas validas no meio de um COMMIT de duas tabelas, nenhuma deixou uma tabela com linha e a outra sem | a varredura pegou P2 · P4 -- pontos de morte diferentes, e e isso que prova que ela mirou dentro da janela | sim |
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
| **I** | a corrida nao foi vazia: o escritor deu voltas enquanto o leitor perguntava | escritor solto 303 voltas / leitor 400; escritor em transacao 211 / leitor 400 | sim |
| **I** | uma varredura unica ENXERGA o estado entre as duas escritas quando o escritor nao usa transacao | 97 de 400 voltas -- e nao e defeito do leitor: o banco esta mesmo inconsistente ali, porque o escritor deixou as duas linhas fora de acordo | sim |
| **I** | com o escritor em transacao, a MESMA varredura nunca mais ve o estado intermediario | o mesmo instrumento, na mesma tabela, viu 97 vez(es) contra o escritor solto -- a diferenca e a transacao, e nada mais | sim |
| **I** | duas leituras separadas veem o par inconsistente MESMO contra um escritor em transacao -- e a leitura repetivel que falta | 73 de 400 voltas; o COMMIT e atomico, mas ele acontece INTEIRO entre a primeira leitura e a segunda | sim |
| **D** | em `por_operacao`, um INSERT que respondeu OK ja mandou o `.reg` ao disco | na mesma medicao, `por_lote` da 0 e `sistema` da 0 -- o contador distingue os regimes | sim |
| **D** | em `por_lote` (o padrao) e em `sistema`, o mesmo INSERT responde OK sem nenhum `fsync` no `.reg` | `por_operacao` deu 1 na mesma corrida | sim |
| **D** | o `fsync` da marca `.tx` acontece nos tres regimes -- ele nao olha `recursos.durabilidade` | contados [1, 1, 1] (por_operacao, por_lote, sistema); no MESMO commit o `.reg` sai [1, 0, 0] -- e a diferenca entre os dois que mostra que o regime so decide a tabela | sim |
| **D** | matar o processo logo depois de um COMMIT que respondeu OK deixa as tres linhas la, nos tres regimes | em `sistema` nenhum `fsync` de tabela aconteceu, e as linhas voltaram assim mesmo -- pela marca, e nao pelo disco da tabela | sim |
| **D** | duas insercoes comuns em `por_lote`, sem nenhum `fsync` no `.reg`, sobrevivem ao SIGKILL | a contagem de `fsync` da mesma configuracao (D1) e ZERO no `.reg` -- as linhas voltaram do cache do nucleo, nao do disco. O SIGKILL nao distingue os dois; so queda de energia distinguiria | sim |

**32 afirmações, 0 sem confirmar.** Medidas contra `phxsqld 0.18.0 (41e82efa97c8) x86_64-unknown-linux-gnu`.
<!-- FIM: afirmacoes -->

**Escopo das linhas I sobre leitura repetível e fantasma (73 de 400, 97 de
400):** a corrida acima mediu o regime **padrão**, sem nenhuma das duas
transações pedir `"leitura_repetivel": true`. Desde 16/09/2026 quem pede fecha
os dois — as provas ficam em §4.5, e o transcrito acima não muda porque ele é
gerado da corrida daquele regime.

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
| 3 | P2 nada aplicado (reaplicadas=800) | 400 | 400 |
| 4 | P4 tudo aplicado, marca pendente (ja_aplicadas=800) | 400 | 400 |
| 5 | P4 tudo aplicado, marca pendente (ja_aplicadas=800) | 400 | 400 |
| 6 | P4 tudo aplicado, marca pendente (ja_aplicadas=800) | 400 | 400 |

**7 quedas válidas, 0 com uma tabela gravada e a outra não.** Os desfechos não foram todos iguais — a varredura pegou pontos de morte diferentes, e é isso que prova que ela mirou dentro da janela.
<!-- FIM: a-queda -->

A varredura caminhou pela passada inteira — de «nada aplicado» a «tudo
aplicado», passando por cinco pontos intermediários — e em **nenhum** deles
uma tabela ficou gravada com a outra vazia. É a recuperação completando o que
faltava a partir da marca, e o relatório do arranque diz quanto ela reaplicou.

### 2.3.1 A prova do outro sentido: apagar a marca produz a metade

Medido em **07/09/2026**, com `PHX_TX_DEFEITO=apaga-a-marca` no
`bancada/transacoes/provar.py`, e é a medição que mais ensina desta seção.

A corrida limpa é a de sempre: `SIGKILL` no meio de um `COMMIT` de 3.000
linhas, banco reaberto, **3001 linhas** — 36 conferências, zero falhas.
Apagando a marca `transacao_<id>.tx` entre o `SIGKILL` e a reabertura, o banco
volta com **43 de 3.000**. Pela metade.

**Isto não é defeito: é a demonstração do contrário.** O `SIGKILL` cai no meio
da passada de commit, então naquele instante há mesmo meia gravação no `.reg`
— é o estado normal e inevitável de um processo morto no meio do trabalho. A
marca é a **única** coisa que o resolve, reaplicando o que faltava ao reabrir.

Daí a frase que esta seção passa a carregar: **o «nunca metade» não é acidente
do caminho de escrita — é comprado pela marca**, e o preço dela fica invisível
enquanto ela está lá. Uma prova que só observasse a corrida limpa concluiria
que o disco «nunca tem meia transação», e essa aparência é exatamente o que a
marca produz.

O 43 muda a cada corrida: matar o processo no instante certo é uma corrida, e o
quanto a passada já tinha escrito depende de onde o sinal caiu. O que **não**
muda é o par de desfechos válidos — `1` ou `3001` — e o relatório de
recuperação dizendo qual dos dois.

O portão fecha nos dois sentidos em `bancada/transacoes/prova-dos-portoes.py`:
o defeito tem de **derrubar** a conferência nomeada, e a corrida sem defeito
tem de passar limpa. Recusa sem controle positivo que passa já custou dois
vereditos errados nesta casa.

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
| corrida | onde a queda caiu | veredito | índices reconstruídos pela recuperação | filhas na chave nova | filhas na chave velha |
|---|---|---|---:|---:|---:|
| 0 | COMPLETADA | CONSISTENTE | 0 | 1200 | 0 |
| 1 | COMPLETADA | CONSISTENTE | 0 | 1200 | 0 |
| 2 | COMPLETADA | CONSISTENTE | 1 | 1200 | 0 |
| 3 | COMPLETADA | CONSISTENTE | 1 | 1200 | 0 |
| 4 | COMPLETADA | CONSISTENTE | 1 | 1200 | 0 |
| 5 | COMPLETADA | CONSISTENTE | 0 | 1200 | 0 |
| 6 | COMPLETADA | CONSISTENTE | 0 | 1200 | 0 |

Vereditos: **7** CONSISTENTE — e a recuperação reconstruiu o índice de alguma filha em **3** das 7 corridas.
<!-- FIM: a-cascata -->

**E o número mudou, com o mecanismo à vista.** Aquela matriz mediu, na linha
`por_lote`, **4 de 7 consistentes e 3 de 7 parciais denunciadas** — e ela é de
**antes** do pedido 172, que pôs a recuperação a reconstruir o `.ndx` da filha
dentro do próprio `completar()`, enquanto a marca ainda existe. Esta corrida,
com os mesmos parâmetros, saiu **consistente nas sete**.

E não é coincidência de amostra: a coluna «índices reconstruídos» diz **em
quais** corridas a recuperação teve de reconstruir o índice de uma filha, e são
**três de sete** — exatamente as três que, sem o conserto, teriam caído em
`operacoes IMPOSSIVEIS`. As outras quatro saíram consistentes pelo outro
caminho, a reaplicação inteira a partir da marca. A tabela distingue os dois, e
é por isso que ela traz a coluna: «sete consistentes» sozinho não diria **por
que**, e um sete que veio de o `SIGKILL` ter errado a janela é indistinguível
de um sete que veio do conserto.

**A metade que o ACID-C fechou (dentro da transação):** a frase do 163 nasceu
*«não há transação»*, e agora há. Dentro de uma transação, a cascata do
`ao_alterar` **entra INTEIRA no conjunto de escrita** — no molde do
*super-journal* do SQLite, apontado pela pesquisa do DBA (`docs/propostas/dba-bases-2026-09.md`
§1.1). A mãe e cada filha viram uma escrita própria da lista, na ordem
pai-antes-de-filha, e a mãe aplica **sem cascatear** (a corrente já é a lista).
Com isso, dentro da transação:

- o **`ROLLBACK` alcança a cascata** — ela é a lista, e desfazer a lista a
  desfaz (`acidc_o_rollback_desfaz_a_cascata`);
- o **`COMMIT` a conta** — `gravadas` traz a mãe e as filhas, não só a mãe
  (`acidc_a_cascata_entra_no_conjunto_de_escrita_da_transacao`, §3.3);
- o **read-your-own-writes a mostra** — a filha da mesma transação acompanha a
  chave nova da mãe antes do commit (§4.4);
- a **marca `.tx` a descreve** — é a v3, e uma queda no meio recupera a corrente
  achatada pela marca, sem re-cascatear (`acidc_a_marca_v3_recupera_a_cascata_achatada`,
  `docs/FORMATO.md`).

O portão vem antes do trabalho: só o `atualizar` que mexe em chave conferida
planeja a cascata (`Table::planejar_cascata_para_lista`), e as tabelas filhas
são travadas **fora** da transação apenas quando há cascata — custo zero para o
resto.

**FORA da transação, desde o pedido 540 (24/09/2026): a alteração solta que
cascateia é uma transação de uma instrução.** Até ali uma escrita solta (sem
`BEGIN`) cascateava dentro do próprio `Table::atualizar`, sem marca: o que ela
garantia era que nada se gravava antes de a árvore inteira ser conferida, e que
uma queda no meio era **denunciada** ou **consertada** pelo arranque (pedido
172). O pedido 490 deu ao pânico a mesma garantia da queda — a filha recusava até
o `reindexar` —, mas **nenhum dos dois completava a cascata**: o `reindexar` (e,
com o 522, o próprio arranque) reconstruía o índice da filha com a órfã dentro.
Medido na revisão do papel C (P4 do `docs/propostas/parecer-dba-integridade-2026-09-24.md`)
e de novo nesta frente, com o conserto desligado: pânico depois de a mãe ir ao
disco, filhas `[5, 5]` com a mãe em 6; `SIGKILL` entre as duas filhas, `[6, 5]`
depois do arranque, e nenhuma marca no disco.

Agora o `op_atualizar`, o upsert do `op_inserir` e a sincronia do DbLink passam
pela mesma porta (`Servidor::alterar_solto`): o plano da cascata sai **antes** da
primeira escrita (o mesmo `planejar_cascata_com`, a mesma árvore conferida) e,
quando ele não é vazio, a mãe e cada filha vão **achatadas** para uma marca `.tx`
sincronizada, e são aplicadas pela **mesma passada do `COMMIT`**
(`passada_sob_a_marca`). O pânico é completado pelo reparo da trava (a marca fica
EM VOO, pedido 451), o `SIGKILL` pelo arranque, a E/S que quebra no meio pela
recuperação da hora. As filhas passam pelos punhos da passada, então entram na
janela de durabilidade como a mãe, e a marca só sai depois do `fsync` delas — a
cascata solta de antes nem punha a filha na janela. Formato da marca: **não
muda** (é a v3 de sempre, com a mãe e os elos com `cascata_na_lista`).

- **Custo, medido** (sonda da frente, `custo`, 300 alterações por rodada, três
  rodadas intercaladas, mediana/p90 em µs): a cascata solta com duas filhas
  ficou **mais rápida**, e não mais cara — em `por_lote`, 2.351–2.620 / 2.591–2.822
  antes e 1.143–1.182 / 1.294–1.658 depois (o p90 de depois fica abaixo do
  mínimo de antes; os máximos, de 5 a 8 ms nos dois, se cruzam); em
  `por_operacao`, 2.848–3.272 antes e 2.538–2.940 depois, faixas que se
  cruzam, então empate. O motivo está no código: a cascata do store sincroniza
  a mãe e cada tabela filha na hora (`aplicar_ao_alterar`, o `sincronizar` de
  que a neta precisa), e a passada troca esses `fsync` por **um** da marca, com
  as tabelas na janela. A alteração sem filha não mudou: 87–88 µs nas duas
  (o plano vazio grava por `Table::atualizar_sem_cascata`, sem refazer a
  varredura das irmãs). O `fsync` da marca cai sob a trava global, como o do
  `COMMIT`, e a catraca `alcancam-fsync-2` ficou em **23 = 23**: a seção do
  `op_atualizar` já alcançava `fsync` pela janela.
- **Provas** (`testes_do_panico_sob_a_trava`, pelo soquete):
  `panico_no_meio_da_cascata_fora_da_transacao_sai_com_a_cascata_inteira`,
  `panico_no_meio_da_cascata_do_upsert_solto_sai_com_a_cascata_inteira`,
  `a_cascata_solta_sem_queda_grava_inteira_e_a_marca_espera_o_fsync` e, contra o
  SO, `sigkill_no_meio_da_cascata_solta_o_arranque_a_completa` — um processo
  filho parado no meio da cascata, morto por `SIGKILL` (sinal 9), e o arranque
  seguinte acha `[6, 6]`, o índice da filha sem a chave velha e nenhuma marca
  sobrando. Os quatro ficam vermelhos com o conserto desligado.
- **O terceiro chamador (C1 do papel C, 24/09/2026).** A porta dizia «o velho
  não escreveu nada», e isso valia para a `op_atualizar` e o upsert. A
  sincronia do DbLink insere pelo mesmo punho **antes** de alterar a mãe: a
  passada abria um segundo punho da mãe com o primeiro sujo e recusava o
  índice (byte 52 em 1 sem atestado), a recuperação o reconstruía pelo `.reg`,
  e o `Drop` do punho velho gravava a árvore velha por cima — `buscar` pela
  chave nova dava 0, o código único entrava repetido e a órfã passava (5 de 5
  na sonda do papel C). Agora o `alterar_solto` faz o punho de quem chama
  descer ao núcleo antes da marca (`Table::descer_ao_nucleo`: o `fechar` do
  `.ndx` e do `.fts`, sem `fsync`). A outra forma — a passada receber o punho
  — foi recusada pelo braço que quebra no meio: o `completar_marca` abre o
  punho dele na mesma mãe, então a descida seria precisa de qualquer jeito, e
  o que a outra forma pouparia é uma abertura da mãe (`Table::abrir`, mediana
  40 µs, medida). Custo na `op_atualizar`, pela mesma sonda `custo` (três
  rodadas intercaladas): em `por_lote` com duas filhas, mediana 1.084–1.174 µs
  antes e 1.125–1.177 depois, faixas que se cruzam; na corrida inteira, 18.332
  chamadas `write` antes e 18.332 depois (`strace -c`). Prova:
  `a_cascata_solta_depois_de_escrever_no_mesmo_punho_nao_perde_o_indice_da_mae`,
  vermelha 5 de 5 com a descida tirada.
- **O que fica:** quem usa o `phxsql-store` embutido e chama `Table::atualizar`
  direto continua sem marca — ali vale o 490, a filha recusa até o
  `reindexar`. Os gatilhos AFTER das filhas continuam não rodando na cascata
  solta, como antes; os da mãe rodam, menos quando a recuperação precisou
  completar a cascata (a resposta traz o `aviso`, como no `COMMIT`).

O 490 continua valendo por baixo: a filha fica com a **cascata em voo** desde
que a mãe vai ao disco até o passo dela terminar, e o `Drop` que a encontra
ligada **sobe** o byte 52 — é isso que protege quem chama o `Table::atualizar`
sem servidor. Dentro de transação a cascata sempre viajou achatada na lista, e o
reparo do 451 completa a marca em voo (`panico_no_meio_da_cascata_na_transacao_sai_com_a_cascata_inteira`).

A primeira receita, a do parecer — manter a janela do `.ndx` da filha aberta
pelo passo inteiro —, **morreu medida**: com ela o `sincronizar` da filha não
desce nada, a neta confere a chave dela num segundo descritor e bate na guarda,
e `a_cascata_alcanca_a_neta` passou a recusar toda cascata de três níveis com a
avó já gravada. A marca em voo só muda o `Drop`; o caminho que termina não vê
diferença nenhuma. Prova: `panico_entre_duas_filhas_deixa_a_filha_recusando_como_um_sigkill`
(store); pelo soquete, desde o 540, a cascata solta nem chega a recusar — ela
se completa. O canto
que esta seção deixava aberto — uma filha que **outra conexão** põe sob a chave
velha entre o `empilhar` e o `COMMIT`, e que a cascata da lista não leva —
**deixou de virar órfã** no pedido 448: a conferência antes da marca (§2.5)
replaneja a árvore, acha a filha que a lista não reescreve e recusa o `COMMIT`
com zero gravado. O fantasma continua acontecendo para quem **lê** (§4.1); o que
não acontece mais é ele sobreviver como órfã.

### 2.5 A conferência antes da marca — pedido 448, 24/09/2026

**O que valia antes, medido no HEAD pelos testes do pedido:** a chave
estrangeira da transação só se conferia na passada, **depois** da marca. Com
`[mãe, filha-órfã, outra]` a resposta era «as 1 anteriores JÁ ESTÃO gravadas» —
a mãe ficava e o resto não. Com `[inserir filha→M, excluir M]` a resposta era
`COMMITTED` com `completando: true`, a filha gravada, M viva e uma marca
sobrando — e o aviso mandava reparar um `.ndx` são. E a filha que outra sessão
apontou para a chave velha entre o `empilhar` e o `COMMIT` ficava **órfã**, com
a resposta `COMMITTED` sem aviso nenhum. Fere o D2 do parecer do 426: a
transação confirmada é inteira ou não é.

**O que vale agora:** entre o `preparar_a_marca` e a marca, com a mesma trava
que a passada usa, a lista inteira passa pela `pre_conferir_a_lista`, na ordem:

* **visibilidade de PREFIXO** — a escrita *i* enxerga o disco mais as escritas
  0..*i*−1, e nenhuma das que vêm depois. Cada uma entra na sobreposição do
  handle dela só **depois** de conferida. É isso que mantém «filho antes do
  pai» **recusado** sem `DEFERRABLE`, e é isso que deixa a exclusão da mãe ver a
  filha nascida antes dela na lista;
* a **chave estrangeira nos dois sentidos** (a filha aponta para mãe viva; a mãe
  excluída não tem filha, contando as que nasceram, foram redirecionadas ou
  apagadas no prefixo), a **árvore do `ao_alterar`** replanejada contra o disco
  e o prefixo, e a **unicidade** — pelas **mesmas** guardas que a passada chama
  (`Table::pre_conferir`), e não por uma segunda cópia delas;
* **um planejador só** (achado A1 da revisão do DBA): o plano do `ao_alterar`
  é o da pré-conferência, contra o disco e o prefixo, e os elos que faltam
  **entram na lista antes da marca**, achatados, logo depois da alteração que
  os puxou; toda alteração passa a ser aplicada **sem replanejar**, na passada
  e na recuperação. A passada replanejava abrindo a filha por um segundo
  descritor, e a guarda do `.ndx` sujo pela própria passada recusava mesmo com
  o plano vazio — `[inserir pedido→A, alterar a chave de B]`, com B sem filha,
  saía pela metade com «DEFEITO DO MOTOR». A marca v3 já carregava elo
  achatado: o formato não muda. E a filha que a **própria** lista **inseriu**
  acompanha a mãe, como no PostgreSQL, no MySQL e na MariaDB. A primeira
  versão recusava `[inserir filha→5, mudar a mãe 5→6]` mandando refazer, e
  refazer dava a mesma recusa. **E, desde o pedido 515, também a alterada e a
  excluída.** Até ali, quando a lista **alterava** ou **excluía** a filha antes
  de mudar a chave da mãe, valia o elo que o `empilhar` planejava olhando só o
  disco, e ele passava por cima do que a lista tinha escrito na filha — o
  achado N2 da segunda revisão do DBA
  (`docs/propostas/parecer-dba-448-2a-2026-09-24.md`), medido igual no
  `82a17ef`. Hoje o plano do `empilhar` parte da mãe como a **transação** a vê
  e abre cada filha com o que a lista já pediu nela (o prefixo, montado só
  quando o plano abre filha — a alteração que não cascateia não paga nada), e
  o elo carrega a linha da filha como a lista a deixou:

  | lista | PostgreSQL | antes do 515 | hoje |
  |---|---|---|---|
  | `[filha id 10→11, mãe 5→6]` | id 11, código 6 | id 10 | id 11, código 6 |
  | `[filha troca de mãe 15→8, mãe 15→16]` | código 8 | código 16 | código 8 |
  | `[excluir suave a filha, mãe 25→26]` | filha excluída | filha ressuscita | excluída, código 26 |
  | `[excluir de vez a filha, mãe 5→6]` | `COMMIT` | recusa com zero gravado | `COMMIT` |

  A excluída suave acompanha a mãe **e continua excluída**: é o que o
  `atualizar` solto faz com ela, e deixá-la na chave velha a tornaria órfã no
  dia do `restaurar`. Prova: `o_elo_da_cascata_nao_desfaz_o_que_a_lista_escreveu_na_filha`;
* **a alteração herda a marca de excluída da linha como a transação a vê**
  (pedido 492, M4 da primeira revisão do DBA). `[excluir suave M, atualizar
  M.nome]` confirmava com M **viva** dentro da transação e a mantinha excluída
  fora: o `empilhar` copiava a marca do disco. A pergunta «o pedido mandou a
  coluna de sistema?» estava escrita três vezes e faltava numa quarta — o
  upsert solto, que ressuscitava a linha excluída **fora** de transação e a
  mantinha excluída dentro. Hoje é uma função só (`valores::herda_a_marca`),
  e a linha de onde a marca vem é a do disco fora da transação e a da
  transação dentro dela (`Servidor::linha_na_transacao`, pela mesma dobra da
  leitura, só das escritas daquela linha). Vale também para a linha que
  nasceu na própria transação, que não herdava marca nenhuma, e para o upsert
  com `atualizar`, que mesclava o SET sobre a linha do disco. Prova:
  `alterar_a_linha_excluida_nao_a_ressuscita_fora_nem_dentro`;
* a cascata que **já estava na lista** (empilhada com filhas) tem de **cobrir**
  o plano refeito: a filha que ele acha e que nenhuma escrita adiante reescreve
  ou foi escrita pela lista — e o elo dela entra —, ou foi apontada para a
  chave velha **por outra sessão** depois do `empilhar`, e aí o `COMMIT`
  recusa dizendo isso: ela ficaria órfã, e numa transação nova o plano já a
  encontra;
* a linha que a sobreposição guarda é a **que o store vai gravar** (achado A3):
  completada pelo DEFAULT, pela `Sequence` (a nova prevista na inserção, a
  mantida na alteração), pela coluna calculada e pelas colunas de sistema,
  **pelas mesmas funções da gravação** e sem consumir contador — prever não
  pode abrir buraco na `Sequence`. Vale para a pré-conferência, para a leitura
  dentro da transação e para o plano da cascata no `empilhar`, que via a
  `Sequence` não mandada como chave que virou NULO e levava o NULO às filhas;
* **NULL não colide num índice único** (achado A4), a regra dos quatro motores
  — aceite automático. Ela estava escrita duas vezes e as duas divergiam: o
  store dava `DUPLICADO` no segundo NULL e a conferência do servidor o pulava.
  Hoje há um predicado só (`Table::participa_da_unicidade`), e o `inserir`, o
  `atualizar`, a troca de chaves da marca, o `reindexar` e as duas
  conferências da transação perguntam a ele. O formato não muda: o NULL sempre
  se codificou assim; mudou quem colide.

**A recusa:** erro do dado (`INTEGRIDADE`, `DUPLICADO`), **zero aplicado, sem
marca**, `repetir` falso, a transação termina, e a mensagem nomeia a posição, a
tabela e o rowid («o COMMIT recusou a escrita 2 de 3 (inserir em pedidos, rowid
1) ANTES da marca»). É o que o PostgreSQL faz com restrição que falha no
`COMMIT`. Quebra que não é do dado (E/S, tabela congelada) devolve a lista e
deixa a transação `ACTIVE`, como o `preparar_a_marca` já fazia.

**A chave se confere na linha FINAL — pedido 514, 24/09/2026.** «Chave
estrangeira que falha sai com zero gravado» valia só para a chave que o pedido
MANDA. O store conferia as mães na linha **crua**, antes de `completar`, da
`Sequence` e de `aplicar_regras`, e a pré-conferência herdou essa ordem. Então a
coluna preenchida pelo próprio motor escapava, dentro e fora da transação. É o
achado N1 da segunda revisão do DBA, medido igual antes e depois do 448:

* um pedido com `cod_cliente` vindo do DEFAULT 7, sem mãe 7, era gravado órfão;
* um item com `cod_cliente` calculado (`x+0`) era gravado com 9 e alterado para
  8, sem mãe nenhuma.

Hoje a ordem «preencher, depois conferir» mora numa função só,
`Table::linha_final`. O `inserir`, o `atualizar` e os dois braços da
pré-conferência passam por ela, e a `conferir_as_maes` só é chamada dali. Os
três maduros conferem a linha final: aceite automático. Isso vale também para a
`Sequence` que é FK (a tabela 1-para-1): quem se confere é o número gerado.

**E a cascata do `ao_alterar`, que escreve na filha por outro caminho.** A
revisão do DBA mediu o irmão (P1): com a chave da filha **calculada**
(`cod_cliente = x + 0`) e em cascata, a mãe trocava de 9 para 8, a cascata
levava o 8 e a calculada o desfazia para 9. Fora de transação, antes do 514 a
filha ficava órfã calada. Depois do 514 vinha um erro, mas só **depois** de a mãe
estar gravada. Fechou por dois lados:

* **na declaração**, que é onde os três maduros recusam o mesmo par
  (PostgreSQL `tablecmds.c` REL_17, MySQL 8.0, MariaDB ERROR 1905): chave sobre
  coluna calculada não aceita `ao_alterar` cascata nem anular, e a recusa
  nomeia a coluna e pede `"restringir"`. O padrão cascata também cai, porque
  quem não escreveu nada não escolheu nada;
* **para a tabela que já nasceu com o par**, que continua abrindo, lendo e
  gravando: a conferência da árvore passa cada filha pela linha final
  (`Table::linha_do_elo`), e a alteração da mãe recusa **antes da primeira
  escrita**, com zero gravado. O mesmo caminho pega o CHECK da filha que o
  valor novo violaria (medido: mãe em 200 e filha em 9, antes).

**O preço na `Sequence`, medido:** a conferência agora vem depois da `Sequence`,
e a linha recusada pela chave **gasta um número**, como a recusada pelo CHECK e
pela unicidade já gastavam, e como gastam os quatro motores. Conferir **antes**
não dá: o DEFAULT e a calculada podem usar o número gerado. Mas o buraco **não é
inevitável, é adiado**: consumir o número só depois da última guarda é o que o
`rownum` já faz (pedido 291), e isso fica para depois da versão (P2). E o buraco
só aparece onde o handle vive além da operação. A op solta pelo servidor abre a
tabela a cada pedido, e o número gasto some com o handle sem ir ao cabeçalho
(ids 1, 2, medidos pelo DBA). O `inserir_lote` e o handle longo deixam o buraco
(ids 1, 4, 5 com duas recusas no lote).

A prova está em `crates/phxsql-store/tests/fk-na-linha-final.rs` e em
`testes_transacoes::pedido_514`, no servidor.

**A passada continua conferindo, como cinto.** Recusa dela depois de a
pré-conferência aprovar é defeito do motor, e a resposta diz isso. A prova do
cinto roda com a pré-conferência desligada por um interruptor de teste
(`sem_a_pre_conferencia_o_cinto_da_passada_diz_o_que_ficou`).

**Dois buracos da sobreposição fecharam antes**, com prova própria
(`crates/phxsql-store/tests/sobreposicao-da-pre-conferencia.rs`): o `buscar`
não achava pela chave nova a linha do disco que o prefixo alterou (e a achava
pela velha), e a conferência de «mãe viva» lia por baixo da marca pendente. Com
qualquer um dos dois reposto, a pré-conferência **recusa transação válida**
(`[mãe nova, filha, alterar a chave da mãe com cascata]`) — medido, não lido.

**Custo, medido** (`--example custo-da-pre-conferencia`, só o `COMMIT`, n/2
mães e n/2 filhas na mesma lista, mediana e faixa min–max; a tabela é da
primeira versão, e a refeita com os consertos da revisão dá 72,3 → 110,3 ms
intercalada e 69,0 → 99,5 ms em blocos a 10.000, 5 rodadas cada; a 100.000,
uma rodada só e sem o binário de antes na mesma corrida, 1.430,7 ms
intercalada e 1.331,3 ms em blocos — 1,70× e 1,58× dos 841 ms de baixo, ainda
abaixo dos 2×):

| escritas | arrumação | antes do 448 | com a pré-conferência | razão |
|---|---|---|---|---|
| 10.000 (5 rodadas) | intercalada | 72,6 ms (69,4–74,7) | 107,0 ms (105,6–136,1) | 1,47× |
| 10.000 (5 rodadas) | em blocos | 73,2 ms (70,5–74,6) | 102,2 ms (99,8–109,3) | 1,40× |
| 100.000 (3 rodadas) | intercalada | 841,8 ms (823,4–989,3) | 1.330,9 ms (1.259,0–1.336,6) | 1,58× |
| 100.000 (3 rodadas) | em blocos | 841,3 ms (777,9–883,3) | 1.233,8 ms (1.207,7–1.243,6) | 1,47× |

As faixas não se cruzam: o custo é real, e fica abaixo do teto de 2× que o
parecer pôs antes de embarcar. Com a busca **linear** que a sobreposição tinha,
o `COMMIT` de 10.000 ia a **6.550 ms (90×)** intercalada e **8.380 ms (114×)**
em blocos — o índice das chaves pendentes (`ChavesPendentes`) entrou por causa
desse número, e não antes dele.

**E o custo da alteração de chave, que a revisão do DBA achou quadrático**
(achado A2): o plano de cada alteração abria a filha com uma **cópia** da
sobreposição dela. Com `n` filhas na lista e depois `n` alterações de chave
(`custo-da-pre-conferencia chaves`), o `COMMIT` foi a 3.583,8 / 15.181,2 /
92.618,7 ms para n = 2.000 / 4.000 / 8.000. A sobreposição passou a ser
**dividida por `Arc`**, sem cópia — o plano só a lê — e o índice das chaves
pendentes que um lado monta fica montado para o outro. A prova no teste conta
as cópias, e não o tempo: `a2_o_plano_da_cascata_nao_copia_a_sobreposicao_da_filha`.
Medido intercalado com o binário de antes do 448 (`82a17ef`), mediana de 3:

| n | antes do 448 | primeira versão | com o `Arc` |
|---|---|---|---|
| 2.000 | 207,0 ms (188,9–226,0) | 3.583,8 ms | 628,7 ms (627,8–663,8) |
| 4.000 | 375,1 ms (358,5–398,5) | 15.181,2 ms | 1.345,2 ms (1.302,1–1.348,2) |
| 8.000 | 740,4 ms (724,9–783,4) | 92.618,7 ms | 2.763,1 ms (2.724,2–2.977,0) |

**Linear de novo** (×2,14 e ×2,05 a cada dobra). O que sobra é uma constante,
e ela passa do teto de 2×: **3,0× a 3,7×** do `COMMIT` de antes, cerca de
345 µs por alteração contra 93 µs. O motivo está medido pelo que o código faz:
cada alteração de chave replaneja a cascata contra o disco e o prefixo, e o
plano abre a filha e lê o esquema das irmãs — trabalho que antes do 448 o
`COMMIT` não fazia, porque ninguém replanejava. É a mesma família do «cada
conferência reabre a mãe» (pedido 494, ⏸), e se conserta ali, emprestando o
handle em vez de reabrir.

**Formato em disco: não muda.** A marca `.tx` v3, o `PSCH` e o `.ndx` ficam
intactos; a sobreposição mora na RAM.

**O que continua fora, e tem pedido:**

* a chave estrangeira conferida na **instrução** (459): os quatro motores
  conferem ali, e aqui a filha órfã ainda só é recusada no `COMMIT`;
* a trava na mãe (460): hoje, quando duas transações disputam a mesma mãe,
  perde a da filha, recusada no `COMMIT`, em vez de esperar como nos três
  maduros;
* os achados N4 e N5 da segunda revisão do DBA (517, 518). O N1 (514), o N2
  (515, o elo por cima da filha alterada na lista) e o N3 (516, o elo
  implícito por cima da leitura repetível de outra transação, §4.5) fecharam;
* ~~o **OLD** do gatilho BEFORE UPDATE que roda no `empilhar` era a linha do
  **disco**~~ — **fechado no pedido 538** (24/09/2026). O OLD passou a ser a
  `linha_na_transacao` do 492, o mesmo motor de «a linha que esta transação
  vê»: o gatilho de delta de estoque na transação 5→3→1 dava **−4** e dá **−2**,
  como o PostgreSQL 16 e o MySQL 8.0 (e como o nosso fora de transação). Os
  irmãos que chamam o mesmo gatilho na mesma instrução foram junto: o upsert
  que vira alteração (`[qtd 1→3, upsert SET qtd = 0]` dava −1, dá −3), o BEFORE
  DELETE (a linha que a lista pôs em 3 saía pelo OLD 5 do disco) e a linha
  nascida na própria transação, que pelo disco nem disparava o gatilho. O AFTER
  UPDATE e o AFTER DELETE já estavam certos: rodam no `COMMIT` com o OLD lido na
  passada, imediatamente antes de cada escrita. Provas:
  `o_old_do_before_update_e_a_linha_que_a_transacao_ve` e
  `o_old_do_before_delete_e_a_linha_que_a_transacao_ve`, vermelhas com o OLD de
  volta ao disco;
* a unicidade da **instrução** (o `empilhar`) ainda olha a linha crua. Medido
  numa coluna única com DEFAULT 7 e o 7 já no disco: a instrução empilha, e o
  `COMMIT` recusa com zero gravado. O dado fica certo; o que muda é onde a
  recusa cai: a transação inteira termina, e não só a instrução. É a família do
  459.

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

**E vale dentro da mesma tabela — pedido 491, 24/09/2026.** O
`conferir_filhas_com` pulava a própria tabela (`if irma == eu { continue }`),
então excluir o chefe que tem subordinado em `funcionarios.chefe_id ->
funcionarios.id` respondia `Ok` — de vez e suave, fora e dentro de transação
(`[inserir 11→10, excluir 10]` confirmava). Hoje a auto-referência entra na
mesma volta, com o **próprio handle** no papel da filha — o que o
`conferir_fks_com` já fazia do outro lado da chave, e o que faz a
pré-conferência ver o subordinado nascido na lista. A linha que aponta **só
para si mesma** sai, como no PostgreSQL, que confere a chave depois de a linha
sair — decisão do dono sobre o empate 5×5 que o papel J mediu
(`docs/propostas/pesquisa-autolaco-2026-09-24.md`). O irmão no catálogo também fechou: o `renomear_tabela` deixava a chave
no nome velho e a regra parava de valer na tabela renomeada; agora recusa,
como já recusava quando a filha é outra tabela. Provas:
`excluir_o_chefe_que_tem_subordinado_recusa_de_vez_e_suave`,
`fora_da_transacao_o_chefe_com_subordinado_nao_sai`,
`na_transacao_o_subordinado_novo_segura_o_chefe` e
`renomear_recusa_a_tabela_que_aponta_para_si_mesma`.

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

### 3.3 O C dentro da transação: fechado pelo ACID-C

A lacuna que mudou de nome duas vezes — *«a cascata escreve em tabela que a
transação não declarou»* — **fechou dentro da transação** (ACID-C). A cascata do
`ao_alterar` entra no conjunto de escrita: cada filha vira uma escrita da lista,
o escopo efetivo passa a mostrá-la (expansão dinâmica, `docs/TRANSACOES.md`
§4.6), o `ROLLBACK` a alcança e o `COMMIT` a conta em `gravadas`. A prova está
em `acidc_a_cascata_entra_no_conjunto_de_escrita_da_transacao` e nas irmãs de
rollback e de recuperação. Ver a §2.4 para o mecanismo (super-journal) e o preço
(portão antes do trabalho, tabelas filhas travadas só quando há cascata).

E a restrição que a lista viola deixou de ser descoberta com metade dela no
disco (pedido 448, §2.5): a chave estrangeira nos dois sentidos, o plano do
`ao_alterar` e a unicidade se conferem na lista inteira antes da marca, com
visibilidade de prefixo — o que mantém «só existe filho se o pai existir
primeiro» valendo na ordem da lista, e não só no fim dela. E a chave se
confere na linha que vai ao disco, inclusive a que o DEFAULT ou a coluna
calculada preenchem (pedido 514, §2.5).

O que **não** entra por aqui, e continua sendo o que derruba *ACID compliant*
seco: por padrão o isolamento é `READ COMMITTED` (§4), sem pedir não há
leitura repetível, e a cascata de uma escrita SOLTA (fora de transação) não é
atômica por desenho (§2.4). Desde 16/09/2026 quem pede `"leitura_repetivel":
true` ganha leitura repetível pela trava (§4.5); o nome que ainda não se
reivindica é `SERIALIZABLE`.

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

**Não é ANSI `SERIALIZABLE`**, e não pode ser chamado assim — o nível de cima é
o que o padrão entrega, **sem pedir nada**: leitura não repetível e fantasma
acontecem, e o skew de escrita acontece. Entre **escritores**, a serialização
é real e por linha: a segunda escrita espera o `LOCK TIMEOUT` e recebe um erro
nomeado, ou, se for escrita comum sem transação, recebe `4005 EM_TRANSACAO`
com `repetir: true` na hora, sem esperar nada.

**Desde 16/09/2026, quem pede sai deste nível.** `"leitura_repetivel": true`
(ou `BEGIN ISOLATION LEVEL REPEATABLE READ`) fecha a leitura não repetível e o
fantasma pela trava compartilhada — §4.5, onde está também o que acontece com
o skew de escrita nesse regime (não há detector de impasse; o prazo resolve).
`SERIALIZABLE` continua não se reivindicando em regime nenhum.

### 4.3 A matriz que responde o que a transação compra para quem LÊ

Duas linhas com a soma constante — 100 sai de uma e entra na outra — e um
escritor transferindo sem parar. O leitor pergunta de duas formas: **uma**
instrução (um `varrer` que devolve as duas linhas) e **duas** (`ler` + `ler`).
Conta-se quantas vezes a soma veio quebrada.

<!-- GERADO: i-matriz -->
| o leitor pergunta | escritor **sem** transação | escritor **em** transação |
|---|---:|---:|
| **uma** instrução (`varrer` devolve as duas linhas) | 97 de 400 |    0 de 400 |
| **duas** instruções (`ler` + `ler`) | 4 de 400 | 73 de 400 |

A corrida não foi vazia: o escritor deu **303** voltas na coluna da esquerda e **211** na da direita, contra 400 perguntas do leitor em cada.

E os **estados** que a instrução única viu contra o escritor sem transação, que é o número que separa «o leitor rasgou a leitura» de «o banco estava mesmo inconsistente»: `(50,50)` 157 · `(49,51)` 146 · `(50,51)` 52 · `(49,50)` 45. O escritor passa **uma** ida e volta em cada estado do meio da transferência e **três** em cada estado em acordo — a frequência tem de sair 3:1:3:1, e sai.
<!-- FIM: i-matriz -->

**Linha de cima — é o que a transação compra.** Sem ela, uma varredura única
enxerga o banco no meio da transferência em cerca de um quarto das perguntas, e
isso **não é defeito do leitor**: o banco está mesmo inconsistente ali, porque
o escritor deixou as duas linhas fora de acordo. Com a transação, o mesmo
instrumento, na mesma tabela, nunca mais vê aquele estado. A única diferença
entre as duas colunas é a transação.

**Linha de baixo — é o que faltava por padrão.** Duas leituras separadas veem o
par inconsistente **mesmo** contra um escritor em transação: o `COMMIT` é
atômico, mas ele acontece **inteiro** entre a primeira leitura e a segunda. É a
leitura repetível que a corrida acima mediu — sem nenhuma das duas transações
pedir `"leitura_repetivel": true`. Desde 16/09/2026 quem pede fecha isto pela
trava (§4.5); esta matriz não foi remedida sob pedido e continua valendo,
sem alteração, para quem **não** pede.

E uma leitura que **não** se deve fazer dessa matriz: o número baixo da célula
de baixo à esquerda **não é garantia nenhuma**. O mesmo par de leituras quebra
dezenas de vezes na coluna ao lado, então o instrumento enxerga; ali ele é
baixo porque o ciclo do escritor solto é curto e os pedidos se alternam, e está
escrito aqui para ninguém o ler como proteção. Quem quiser a garantia de duas
leituras coerentes sem pedir nada não a tem em regime nenhum — é a leitura
repetível que, por padrão, não existe (§4.5 para quem pede).

### 4.4 Duas imprecisões que ficam, e a terceira que o ACID-C fechou

As duas primeiras são do pedido 162 e continuam valendo: na ordem do **índice**
a linha pendente sai no fim (ela não está no `.ndx`), e `Sequence`/`rownum` só
nascem no `COMMIT`, então dentro da transação saem nulos.

**A terceira, agora FECHADA (ACID-C):** o *read-your-own-writes* **alcança a
cascata**. Dentro da transação, alterar a chave da mãe faz a mãe aparecer com a
chave nova **e a filha acompanhar** — porque a cascata agora vira `Escrita`, e a
sobreposição, que é montada a partir do conjunto de escrita, a lê como qualquer
outra linha pendente. Antes deste conserto a filha continuava apontando para a
chave antiga até o `COMMIT`; hoje a prova
`acidc_a_cascata_entra_no_conjunto_de_escrita_da_transacao` mede a filha em `2`
dentro da transação, e a de rollback mede que ela volta a `1` no `ROLLBACK`. Ver
a §2.4 e a §3.3.

### 4.5 A leitura repetível pela trava, pedida — 16/09/2026

O gap que as §4.1 a §4.3 medem — leitura não repetível e fantasma acontecendo
por padrão — foi **reaberto pelo dono e resolvido**, sem construir a Sombra
(`docs/SOMBRA.md`, via (b) da §5): quem **pede** ganha os dois fechados pela
trava. A Sombra/MVCC continua parada — este não é o caminho dela.

**Como se pede.** Protocolo: `begin` com `"leitura_repetivel": true` (alias
`"repeatable_read": true`). SQL: `BEGIN [TRANSACTION] ISOLATION LEVEL
REPEATABLE READ`, em qualquer ordem com SCOPE/TIMEOUT/LOCK TIMEOUT/LOCK
MODE/STATEMENT TIMEOUT. `ISOLATION LEVEL READ COMMITTED` é aceito e é o padrão
(não viaja no pedido). `READ UNCOMMITTED` é aceito e vale READ COMMITTED — como
o PostgreSQL(R), o motor nunca lê sujo. `ISOLATION LEVEL SERIALIZABLE` recusa,
nomeando o que existe. `SET TRANSACTION ISOLATION LEVEL X` continua recusado (o
tradutor não guarda estado de sessão), e a mensagem aponta `BEGIN ISOLATION
LEVEL REPEATABLE READ`.

**Mecanismo.** A transação que pediu toma a trava **compartilhada (S)**
(`Trava::Compartilhada`, `crates/phxsql-server/src/travas.rs`) em cada tabela
que **lê**, pelo portão único `dentro_da_transacao` →
`travar_leitura_repetivel` → `esperar_trava` (respeita LOCK TIMEOUT e o
TIMEOUT da transação). A S fica até COMMIT/ROLLBACK/estouro de prazo/queda da
conexão (`soltar_tudo`). Leitores compartilham a S entre si. A S barra
escritor alheio: transação (intenção/exclusiva/linha) espera pelo
`esperar_trava`; escrita autocommit é recusada NA HORA com `EM_TRANSACAO`
nomeando quem segura (`barrado_por_travas`, em `portoes_do_pedido`). A S é
barrada por intenção/exclusiva/linha alheia. O `INSERT` disputa o fim da
tabela (`FIM_DA_TABELA`), então sob S não entra: **sem fantasma**, de graça.
Várias tabelas lidas saem coerentes porque cada uma toma a S pelo mesmo
portão. A ficha (`op transacao`) devolve `"leitura_repetivel": true/false` e
`transaction_isolation` com o texto do nível que está valendo (§0, §4.2).

**O elo que só o COMMIT descobre também espera o leitor — pedido 516,
24/09/2026.** A cascata **implícita** (T1 muda a chave da mãe sem filha
nenhuma; depois a filha nasce na chave velha por outra sessão) era escrita pelo
`COMMIT` de T1 numa linha que T1 nunca travou, e passava por cima da S de T3:
T3 lia 5 e relia 6 (N3 da segunda revisão do DBA ao 448). Hoje cada elo que a
pré-conferência acrescenta toma a trava pelo mesmo caminho de toda escrita da
transação (escopo, tabela, linha — `travar_para_escrever`), **sem esperar**,
porque o `COMMIT` está com a trava de dados na mão. Barrado, o `COMMIT` recusa
com `EM_TRANSACAO` nomeando a tabela e quem segura, **nada gravado, a
transação continua ativa** (`repetir: true`), e o `COMMIT` seguinte, depois de
T3 terminar, confirma. Prova:
`o_elo_implicito_respeita_a_leitura_repetivel_de_outra_transacao`.

**E dois COMMITs que se barram não giram para sempre** (condição C1 do papel
C). Não esperar trava no `COMMIT` tirou o abraço com a trava global e trouxe
outro: T1 barrado por T2 e T2 barrado por T1, os dois mandados repetir, e
repetindo — **1.870 rodadas em 11 s** na sonda do DBA, sem ninguém sair. «Sem
espera não há grafo de espera» (`travas.rs`) deixa de valer quando a
repetição que o recado manda fazer é a espera, só que do lado do cliente.
Cada `COMMIT` barrado anota quem o barrou (`Transacao::commit_barrado_por`), e
se a corrente dessas anotações volta a ele há ciclo: a **mais nova** do ciclo
cede — `TRANSACAO_ABORTADA`, `repetir: false`, travas soltas na hora pela mesma
porta do prazo estourado —, e a mais velha continua mandada repetir e passa na
vez seguinte. **A idade é escolha nossa, não cópia de motor** (R2 da
re-checagem do papel C): o PostgreSQL aborta uma das transações do impasse
sem ordem garantida, e o InnoDB aborta a mais leve, pelas linhas alteradas.
Aqui a regra é determinística — o mesmo ciclo cede sempre pela mesma, quem
quer que o descubra — e garante progresso no molde do *wait-die*: a mais
velha nunca cede. A corrente só atravessa transação **ativa** (a que está em
`ABORT_ONLY` não confirma mais, e esperar por ela é espera comum), e o
`rollback_para` apaga a aresta, porque o elo que a criou pode ter saído com a
lista — sem isso a outra cedia num ciclo que já não existia (R1, prova
`voltar_ao_savepoint_apaga_a_aresta_do_commit_barrado`).
Prova, nas duas ordens: `dois_commits_que_se_barram_cedem_pela_mais_nova`
(vermelho sem o desempate: 20 rodadas e os dois ainda em `EM_TRANSACAO`) e
`o_ciclo_de_commits_barrados_cede_pela_mais_nova` (o ciclo de três e a corrente
sem volta). **E o desempate tem teto desde o pedido 539**: o `COMMIT` confere o
prazo da transação antes da marca, com a trava de dados na mão, pela mesma porta
do prazo estourado (`estourar_prazo` → `abortar_soltando`). Medido pelo papel C
antes: `COMMITTED` 600 ms depois de um prazo de 200 ms, porque o `COMMIT` é
operação de controle, o portão não olha o prazo delas e a varredura só rodava no
`begin` e no `transacoes` — o desfecho dependia de uma terceira conexão. Agora:
`TRANSACAO_ABORTADA`, `repetir: false`, zero linhas, travas soltas; o
`ROLLBACK` fecha. Vale também para a lista vazia. Prova:
`o_commit_depois_do_prazo_nao_grava` (vermelho sem o conserto: `COMMITTED`,
`gravadas: 1`). E o vizinho dele, o pedido 559: a **varredura** do prazo, que
roda no `begin` de outra conexão sem a trava de dados, deixou de encerrar a
transação que está **no** `COMMIT` — encerrá-la soltava as travas de quem ainda
gravava (medido antes: `ABORT_ONLY` e as travas soltas no meio da passada) —, e
a lista devolvida ao fim de um `COMMIT` recusado não desfaz mais um
`ABORT_ONLY` que tenha chegado no meio (medido antes: voltava `ACTIVE`, com a
lista e sem trava nenhuma). Provas:
`a_varredura_do_prazo_nao_mexe_na_transacao_que_esta_confirmando` e
`devolver_a_lista_nao_desfaz_o_abort_only`.

**O update perdido do elo planejado no `empilhar` fechou no pedido 537**
(24/09/2026). O elo travava a **tabela** filha (intenção) e o fim dela, e não
cada linha: T2 gravava `x = 1` na filha, solta ou em transação, e o `COMMIT` de
T1 regravava `x = 0`, a linha que o `empilhar` viu (medido pelo papel C, P1).
Agora são duas redes:

- **a trava da LINHA** de cada filha do plano, pelo `travar_para_empilhar` (o
  mesmo caminho de toda escrita da transação, esperando o `LOCK TIMEOUT` fora
  da trava de dados). A escrita solta na filha recusa na hora
  (`EM_TRANSACAO`, `repetir`), a de outra transação espera o `LOCK TIMEOUT`
  dela, e as duas passam depois do `COMMIT` de T1. A filha que só aparece no
  plano refeito é travada numa volta seguinte (quatro voltas no máximo; a
  quinta recusa a instrução). Prova: `o_elo_do_empilhar_trava_a_linha_da_filha`;
- **o elo refeito no `COMMIT`**: antes da marca, o elo do `empilhar` se refaz
  sobre a linha ATUAL (o disco com o prefixo da lista), levando **só a chave**
  — as colunas que o elo muda, e só onde a filha ainda tem o valor de antes.
  É a rede para o que a trava não alcança: a cascata **solta** de outra mãe da
  mesma filha escreve nela sem perguntar por trava de linha. Medido: o
  vendedor 3 vira 4 fora de transação, a filha vai junto, e o elo de T1 (que
  viu `cod_vend 3`) regravava a linha inteira — o `COMMIT` recusava pela
  `fk_vend`. Agora confirma, com as duas chaves novas. Prova:
  `o_commit_leva_so_a_chave_do_elo_sobre_a_linha_atual`. A marca recebe a linha
  refeita, e o formato não muda.

O desempate do 516 não reabre ciclo sem teto: a trava nova é de **instrução**
(espera o `LOCK TIMEOUT`), e o ciclo de `COMMIT`s continua cedendo pela mais
nova. Medido pela sonda do papel C (`c1` e `c1misto`), antes e depois: as seis
bordas saem iguais linha a linha, menos a C1-C2, em que o `COMMIT` da mais nova
com o prazo vencido passa a dizer o prazo (539) em vez do ciclo; o ciclo misto
continua terminando no TIMEOUT da transação da instrução (2.005 ms, com 2.000
de prazo).

**O que ficou, e é achado novo desta frente:** a cascata SOLTA de outra mãe da
mesma filha **não pergunta pela trava de transação da filha** — o portão das
escritas soltas (`barrado_por_travas`) só olha a tabela do pedido. **O refazer
do `COMMIT` NÃO impede update perdido**, ao contrário do que esta seção dizia:
medido pelo papel C (P1 do `docs/propostas/parecer-dba-integridade-2-2026-09-24.md`),
ele cobre **só o elo** planejado no `empilhar`. Quando a própria T1 escreveu a
filha (`x = 1`), a escrita dela regrava a linha inteira com a chave que ela viu,
e desfaz a cascata solta que passou no meio: o `COMMIT` recusa pela `fk_vend`
e T1 perde o trabalho (P1 a), ou — se o vendedor 3 renasce antes do `COMMIT` —
**confirma com a filha no vendedor NOVO 3**, a mãe errada (P1 b). O upsert
solto passa pela trava X de T1 e responde OK, e o `COMMIT` de T1 o apaga (P1 d);
a leitura repetível relê a filha (P1 c). O alcance é o da escrita solta que
grava linha sem nomeá-la — a filha da cascata solta, a linha do upsert solto e
as da sincronia do DbLink —, e o conserto é outro pedido.

**Custo e limite.** Custo zero para quem não pede — o gancho devolve antes de
qualquer trava. A recusa por LOCK TIMEOUT é do **leitor** que pediu (o escritor
mantém a vazão). **Não há detector de impasse**: duas transações repetíveis
que leram a mesma tabela e tentam escrever recebem LOCK TIMEOUT nos dois
sentidos — o prazo resolve, sem nenhuma completar com um resultado quebrado.
**Não se reivindica `SERIALIZABLE`**: o que se afirma é só o que os testes
medem. Quem não pede continua exatamente como antes (READ COMMITTED) — guarda
nova entra pedida, não imposta.

**Provas** (`crates/phxsql-server/src/servidor.rs::testes_leitura_repetivel`):
`sem_pedir_a_leitura_continua_nao_repetivel` (controle),
`pedindo_a_leitura_e_repetivel_e_o_escritor_espera`, `pedindo_nao_ha_fantasma`,
`a_recusa_e_do_leitor_que_pediu`,
`duas_repetiveis_que_leram_a_mesma_tabela_nao_se_atropelam` (esta última
escrita depois da medição abaixo). Prova real nos dois sentidos, medida em
16/09/2026: com o gancho removido do portão, **3 falharam e o controle
passou**. Em `crates/phxsql-sql/src/transacao.rs`: `isolation_level_na_abertura`
e `set_isolation_level_nomeia_o_nivel_real`. Ver `docs/PENDENCIAS.md` #246.

---

## 5. D — durabilidade

### 5.1 O que cada regime promete

`recursos.durabilidade` tem três valores, e ele muda o significado de «OK»:

| regime | o que um `OK` de escrita quer dizer | quando a tabela vai ao disco |
|---|---|---|
| `por_operacao` | **está no disco** | dentro da própria operação |
| `por_lote` (**padrão**) | está no núcleo, e vai ao disco quando a janela fechar | ao fechar `lote_operacoes` (200) ou `lote_milissegundos` (200 ms), o que vier antes; o relógio de fundo fecha mesmo sem tráfego |
| `sistema` | está no núcleo, e vai ao disco quando o sistema operacional quiser | nunca por conta própria — só no próximo arranque. A exceção é a marca de índice sujo: a **subida** do byte 52 do `.ndx` vai ao disco por `fdatasync`, uma por tabela até o próximo `sincronizar` (pedido 533), porque sem ela a queda deixaria o índice mentir calado |

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
  completada no arranque. Dentro da transação a cascata do `ao_alterar` **entra
  no conjunto de escrita** (ACID-C, super-journal): o `ROLLBACK` a alcança e o
  `COMMIT` a conta. Desde o pedido 448 a lista inteira se confere **antes** da
  marca (§2.5): chave estrangeira, cascata ou unicidade que falha sai com zero
  gravado, e não mais com a parte da frente aplicada. Desde o pedido 514 isso
  vale também para a chave que vem do DEFAULT ou de coluna calculada. Fora de transação,
  desde o pedido 540, a alteração solta que cascateia é uma transação de uma
  instrução: a marca vai antes, e queda, `SIGKILL` ou pânico no meio dela são
  completados (§2.4). Só quem chama o `Table::atualizar` do store embutido
  continua sem marca, com a garantia do 490 (a filha recusa até o `reindexar`).
* **C — consistência: imposta na gravação; dentro da transação a cascata é
  coberta.** Tipo, tamanho, obrigatoriedade, unicidade e integridade
  referencial são conferidos em toda porta local de escrita, e «nunca se mata o
  pai que tem filhos» vale nos dois excluires — desde o pedido 491 também
  quando o pai e o filho moram na mesma tabela. A chave estrangeira se confere
  na linha final, inclusive a que o DEFAULT ou a coluna calculada preenchem
  (pedido 514). Dentro da transação a cascata do
  `ao_alterar` passou a entrar no conjunto de escrita (ACID-C), então o
  `ROLLBACK` a desfaz e o escopo efetivo a mostra. A réplica aplica e não julga,
  por decisão medida.
* **I — isolamento: leitura confirmada por padrão; leitura repetível pela
  trava, pedida.** `READ COMMITTED` é o que se entrega sem pedir nada, com
  escrita serializada por linha entre transações. Quem pede
  `"leitura_repetivel": true` (ou `ISOLATION LEVEL REPEATABLE READ`) ganha
  leitura repetível e ausência de fantasma pela trava compartilhada, desde
  16/09/2026 (§4.5) — pagando o escritor esperar o leitor. `SERIALIZABLE`
  continua não se reivindicando em regime nenhum, e sem pedir a transação
  continua comprando só a consistência de **uma** instrução.
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
| **(b) manter *ACID compliant* seco** | a palavra que o mercado procura | **é falso hoje**, e o ponto que o derruba não é opinião: `SERIALIZABLE` não existe (o nome não se reivindica em regime nenhum), e o skew de escrita está medido acontecendo por padrão. A leitura repetível **deixou de ser o ponto que derruba** — desde 16/09/2026 ela existe pela trava, para quem pede (§4.5) —, mas o que falta continua bastando para reprovar a frase seca. Um comprador técnico que rodar esta bancada acha em cinco minutos |
| **(c) qualificar na própria frase** — *ACID com isolamento **read committed*** | verdadeiro, verificável, e é o que MySQL(R) e PostgreSQL(R) fazem no padrão deles | a frase fica mais longa; e obriga a manter a qualificação em todo lugar que a repetir |
| **(d) trocar por uma afirmação que é inteira** — p. ex. *transações atômicas e duráveis, integridade referencial imposta* | tudo o que se afirma está medido nesta página, letra por letra | não usa a sigla, então não casa com busca por «ACID» |

**A recomendação desta frente, e ela é recomendação e não decisão: (c) ou
(d).** As duas são verdadeiras hoje; (b) não é, e (a) joga fora mais do que
precisa. Entre as duas, (c) casa com o vocabulário do mercado e é o que os dois
grandes fazem; (d) é mais forte tecnicamente porque não pede nota de rodapé.

**O que continua falso em qualquer redação, e não pode aparecer:**
*SERIALIZABLE*, *snapshot isolation*, *MVCC*, e *ACID compliant* **sem**
qualificação. *Leitura repetível* **sem qualificação** também continua falsa —
por padrão ela não existe —, mas desde 16/09/2026 é verdade **qualificada**:
*leitura repetível sob pedido, pela trava* (§4.5). O MVCC está recusado com o
motivo em `docs/TRANSACOES.md` §11.1 — aqui o rowid é endereço.

**O que a marca já pode afirmar sem ressalva nenhuma**, porque está medido
nesta página: *transações com `BEGIN`/`COMMIT`/`ROLLBACK` e `SAVEPOINT`*,
*commit atômico e recuperação automática no arranque*, *integridade referencial
imposta na gravação*, *durabilidade configurável*, e *leitura repetível sob
pedido, pela trava*.

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
