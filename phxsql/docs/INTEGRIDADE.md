# Integridade referencial: os caminhos de escrita, um a um

> **A regra primordial.** Palavra do dono: *«1 para muitos. Cascade/Restrict
> sempre. Nunca pode matar o registro pai se tem filhos em outra tabela(s). A
> opção Cascade/Cascade não existe em PhxSql.»*
>
> Em código: `ao_excluir` aceita **só** `restringir`, `ao_alterar` nasce
> `cascata`, e **chave declarada nasce conferida**.

Este documento existe porque a garantia não é de uma função: é de **todas as
portas por onde uma linha entra, sai ou volta a existir**. Enquanto uma delas
não fizer a pergunta que as irmãs fazem, ela é a próxima a virar buraco — e foi
exatamente isso que aconteceu, quatro vezes, medido por sonda.

```bash
cargo run --example sonda-fk-buracos  -p phxsql-store   # os buracos de linha e de tabela
cargo run --example sonda-replica-fk  -p phxsql-store   # a réplica, nas três ordens
cargo run --release --example custo-da-fk -- 20000 1000 # o que a garantia custa
cargo run --release --example conferir-integridade -p phxsql-store -- <dir>
```

## 1. A tabela: quem confere, quem não confere, e por quê

Derivada do código, e não de lista de ninguém. A coluna «confere» diz o que a
porta pergunta antes de gravar.

### As portas por onde uma linha nasce, muda ou volta

| caminho | arquivo:linha | confere | o quê |
|---|---|---|---|
| `Table::inserir` | `table.rs:2022` | **sim** | `conferir_fks` — a mãe existe **e está viva** |
| `Table::inserir_lote` / `BULKINSERT` | `table.rs:2149` | **sim** | linha a linha, pelo `inserir`; não há atalho por baixo |
| `Table::atualizar` | `table.rs:2243` | **sim** | `conferir_fks` + a **cascata** do `ao_alterar` |
| `Table::atualizar_se` | `table.rs:2236` | **sim** | confere a versão e delega ao `atualizar` |
| `Table::restaurar` | `table.rs:2525` | **sim** | `conferir_fks` da linha que volta |
| `Table::excluir_suave` | `table.rs:2489` | **sim** | `conferir_filhas` — pai logicamente morto também deixa órfã |
| `Table::excluir_de_vez` | `table.rs:2430` | **sim** | `conferir_filhas` |
| `Table::excluir` | `table.rs:2940` | **sim** | é o `excluir_de_vez` sem motivo escrito |
| cascata do `ao_alterar` | `table.rs:1191` e `:1365` | **sim** | planeja, confere a **árvore inteira** (`:1157`) e só então grava |
| `Table::recascatear` | `table.rs:1107` | **sim** | refaz só a cascata, pela linha antiga; idempotente |
| reaplicação da recuperação | `transacao.rs:1219` | **sim** | usa o `inserir`/`atualizar`/`excluir_*` de sempre, e **recascateia** |

### As portas que aplicam o que outro servidor já julgou

| caminho | arquivo:linha | confere | por quê **não** |
|---|---|---|---|
| `Table::aplicar_evento` | `table.rs:2770` | **não, por decisão** | §3 |
| `Table::inserir_replicado` | `table.rs:2806` | **não, por decisão** | §3 — o bidirecional |
| `Table::atualizar_replicado` | `table.rs:2817` | **não, por decisão** | §3 — também não recascateia |
| `Table::excluir_de_vez_replicado` | `table.rs:2830` | **não, por decisão** | §3 |

### As portas que mexem no esquema ou nos arquivos

| caminho | arquivo:linha | confere | o quê |
|---|---|---|---|
| `Table::redeclarar_chaves_estrangeiras` | `table.rs:549` | **sim** | recusa declarar **conferida** sobre dado que já viola (§2.4) |
| `Database::excluir_tabela` | `catalogo.rs:414` | **sim** | `quem_aponta_para` — a regra primordial no nível da tabela (§2.2) |
| `Database::renomear_tabela` | `catalogo.rs:506` | **sim** | a chave guarda a mãe pelo **nome** |
| `Table::acrescentar_coluna` | `table.rs:614` | **não precisa** | §4.1 |
| `Database::duplicar_tabela` | `catalogo.rs:614` | **não precisa** | §4.2 |
| `Database::copiar_tabela_para` | `catalogo.rs:654` | **não, por decisão** | §4.3 |
| `Preparada::confirmar` (restaurar backup) | `restaurar.rs:543` | **não, por decisão** | §4.4 |
| `Table::reparar` / `Table::reindexar` | `table.rs:501` e `:3809` | **não, por decisão** | §4.5 |

## 2. Os quatro buracos que a sonda mediu, e como cada um fechou

### 2.1 «Existir» não é «estar viva»

A conferência perguntava «esta linha existe?». A mãe excluída de forma **suave**
continua no `.reg`, com a chave dela no índice — então um pedido novo nascia
apontando para um cliente que a tela não mostra mais.

É o outro lado do tempo da própria pétrea do `excluir_suave`, que já confere as
filhas *«porque pai logicamente morto deixa filha apontando para linha que a
tela não mostra mais»*. Faltando esta metade, a casa fechava a porta e deixava a
janela: não dava para **matar** a mãe com filha, mas dava para **nascer** filha
de mãe morta.

Fechado em `conferir_fks` (`table.rs:791`). O custo entrou no laço quente e está
medido em `DESEMPENHO.md` §15: **+7,03 µs/linha (+11,2%)** na chave conferida,
e **zero** em quem não pediu conferência.

### 2.2 O `DROP TABLE` matava o pai

O `excluir_tabela` apagava os oito arquivos da mãe e a filha ficava com a linha
intacta apontando para o vazio. O `renomear_tabela` **recusava o mesmo cenário**
— então o motor sabia fazer a pergunta e não a fazia ali.

E apagar é pior que renomear: o renomear deixa a filha apontando para um nome
que não existe mais; apagar deixa a filha apontando para um nome que não existe
mais **e** joga fora a linha mãe, que era a única coisa que ainda diria qual pai
era aquele.

**Não se pula a chave com `verificar: false`**, e a divergência com o
`conferir_filhas` é proposital: lá a pergunta é se a regra é *imposta*, porque o
que sai é uma linha; aqui o que sai é o **nome**, e uma declaração pendurada num
nome inexistente é uma mentira sobre o modelo mesmo quando ninguém a confere.

**O limite, escrito em vez de escondido:** a busca é no diretório do schema, o
mesmo alcance que o `renomear_tabela` já tem. Filha em **outro schema**
apontando para cá por nome qualificado não é vista. Fechar isso pede varrer
todos os schemas por `excluir_tabela`, e essa é decisão de custo que se toma com
número na mão.

### 2.3 A réplica divergia — e a causa não era a que o pedido dizia

O pedido 171 dizia que a réplica divergia «só na ordem entrelaçada». Remedido,
o estado era pior e tinha **três causas**, e as duas piores nasceram depois,
quando *chave declarada nasce conferida* ligou o portão também no caminho da
réplica. Está tudo na §3.

### 2.4 Declarar conferida sobre órfã

Dava para declarar `verificar: true` numa tabela que já tinha órfã, e a órfã
continuava lá. A tabela nascia com uma **promessa falsa** — um `verificar` que
nunca valeu para as linhas já gravadas —, e promessa falsa é pior que a ausência
dela, porque quem lê o esquema para de perguntar. É a mesma família de
*«configuração que não é lida mente»*.

A recusa fica na **declaração**, que é onde a casa já pôs a do `ao_excluir`: uma
tabela nasce uma vez e grava um milhão de vezes.

Três coisas ela deliberadamente **não** recusa, e o motivo está escrito no
código:

* **chave que já conferia** — ela já era garantida, e cobrar a varredura de novo
  tornaria caro um `ALTER TABLE` que nem toca nela;
* **falta de índice** — sem índice a varredura nem acontece: o motor procura por
  índice, nunca por varredura, e inventar aqui uma varredura que a gravação
  recusa faria a declaração medir outra coisa;
* **tabela mãe ausente** — é ordem legítima de modelagem, e já tem recusa
  própria na gravação, com o recado que nomeia a tabela que falta.

## 3. A réplica **aplica**, ela não **julga** — a recusa, com número

A tentação é conferir de novo na réplica: guarda a mais nunca fez mal a ninguém.
Aqui fez.

A replicação anda por **tabela**, cada uma com a sua posição no diário. Não
existe ordem global entre tabelas, e nem poderia existir sem serializar o
cluster inteiro. Medido com `clientes.ins → pedidos.ins → clientes.alt` no
source e os quatro eventos replicados:

| ordem de entrega | antes | depois |
|---|---|---|
| mãe primeiro | `pedidos` com **0 de 2** eventos, linha inexistente | 2 de 2, `Int(2)` |
| filha primeiro | `pedidos` com **0 de 2** eventos | 2 de 2, `Int(2)` |
| entrelaçada | 1 evento **a mais** que o source (cascata refeita) | 2 de 2, `Int(2)` |

Os três eram divergência, e os dois primeiros eram **perda de dado causada pela
guarda**. A garantia de integridade é da **origem**, que a impôs quando aceitou
a escrita; a garantia da réplica é de **fidelidade**, conferida por SHA-256 de
cada linha. Conferir duas vezes não soma as duas: troca a segunda pela primeira.

Três coisas mudaram, e a terceira não era uma decisão — era um defeito:

1. a réplica **não confere** chave estrangeira;
2. a réplica **não refaz** a cascata: o source já cascateou, e o evento que a
   cascata dele gerou vem replicado por conta própria;
3. **a cascata do source passou a gravar a imagem no diário da filha.** A
   cascata abre a filha num handle próprio, e ele nascia com a imagem
   **desligada** — o evento ia para o diário sem a imagem da linha, e a réplica
   o recusava com «veio sem imagem», nas três ordens. É a família do KiB do
   rodapé: a garantia valia só para o caminho que passou pela mão de quem a
   ligou. Quem replica liga a imagem na tabela que **abre**; a que o motor abre
   por baixo tinha de sair igual.

**A marca é de UM evento, não do handle.** O par liga/desliga fica no
`aplicar_evento`, com o trabalho num interno: um `return` no meio deixaria o
handle sem portão para a escrita local seguinte, e portão que se apaga sozinho é
pior que portão nenhum, porque ninguém procura por ele.

### 3.1 O bidirecional, que uma varredura ingênua não encontra

O bidirecional **não passa pelo `aplicar_evento`**: ele casa por **chave**, não
por rowid — o rowid e o rownum são locais, e a ordem de digitação de cada
servidor é sagrada nele. Então ele chama o `inserir`/`atualizar`/`excluir_de_vez`
de sempre, e caía no mesmo buraco **com consequência pior**.

Medido pela leitura do fluxo (`servidor.rs:2865`, chamado de `aplicar_lote_bidi`
pelo `?`): a chave era conferida, o evento da filha que chegasse antes da mãe era
recusado, o erro subia pelo `?`, `desde = lote.ate` nunca executava, a posição
não andava — e o mesmo lote voltava na rodada seguinte. Para sempre. **Não é uma
linha perdida: é o par de servidores parado.**

A peça mora no store, com o motivo escrito (`inserir_replicado`,
`atualizar_replicado`, `excluir_de_vez_replicado`); no servidor mudam três
chamadas e nada mais.

### 3.2 O que CONTINUA conferindo, e por que é diferente

A reaplicação da recuperação (`transacao.rs:1219`) confere e recascateia, e a
diferença é de **natureza**: ali a escrita é local e está sendo refeita neste
mesmo servidor, então o julgamento também é local e as tabelas todas estão no
mesmo disco. Não há ordem entre servidores para atrapalhar.

## 4. Onde a decisão foi NÃO conferir

### 4.1 `acrescentar_coluna`: não precisa

Ela não toca em valor de coluna que participe de chave — a chave já tem de estar
declarada, e as colunas dela já existem. O que **poderia** quebrar é a posição:
`ForeignKey.colunas` guarda índice, não nome, e inserir coluna no meio empurraria
todas. Isso já está fechado, e num lugar só: `Schema::com_coluna` remapeia
`IndexColumn.coluna`, `ForeignKey.colunas` e a coluna de partição juntas, com o
motivo escrito lá.

### 4.2 `duplicar_tabela`: não precisa

A cópia fica no mesmo diretório e a chave dela aponta para a mesma mãe. O efeito
real é o oposto do perigo: a mãe passa a ter **duas** filhas, e o
`conferir_filhas` — que varre o diretório — enxerga as duas.

### 4.3 `copiar_tabela_para`: decisão, com o custo medido

Colar a filha num database onde a mãe ainda não está é ordem legítima de
trabalho — cola-se uma, cola-se a outra —, e recusar obrigaria uma ordem que a
tela não tem como impor. A cópia é **byte a byte**: preserva a ordem de digitação
e os rowids, e reinserir linha a linha para conferir perderia os dois.

O que a decisão custa está **medido em teste** em vez de suposto
(`colar_a_filha_sem_a_mae_passa_e_o_verificador_acha_a_orfa`): a tabela colada
nasce com órfã. O que a torna aceitável são as duas saídas que existem depois —
o motor **recusa** a próxima gravação dizendo que a tabela mãe não existe naquele
banco, e o verificador acha a órfã sem que ninguém precise desconfiar dela.

Trocar isto por uma recusa exige número: quantas colagens legítimas quebrariam
contra quantas órfãs evitadas. Sem esse número, a recusa seria preferência.

### 4.4 Restaurar backup: não é o lugar

Um backup é o retrato de um **database inteiro**, e ele é internamente
consistente ou não é — e se não é, quem diz isso é o verificador, não uma recusa
que impediria restaurar. Recusar aqui trocaria «restaure e confira» por «não
restaure», que é a pior das duas na hora em que se precisa de um backup.

### 4.5 `reparar` e `reindexar`: outro defeito

Os dois consertam arquivo, não modelo. O `reparar` pode trazer de volta um slot
que estava ilegível, e isso pode ressuscitar uma órfã — mas negar o reparo por
causa disso trocaria um arquivo consertado por um arquivo quebrado. O
verificador é a ferramenta certa depois de um reparo.

## 5. O verificador de consistência

`crates/phxsql-store/src/integridade.rs`, com
`--example conferir-integridade`. Ele faz três perguntas e **relata**:

1. **A tabela mãe existe?** Chave apontando para tabela inexistente não é órfã: é
   modelo quebrado, e nenhuma linha filha pode ser gravada enquanto estiver assim.
2. **Há índice dos dois lados?** Na mãe para responder «existe este pai?» ao
   gravar a filha, e na filha para responder «alguém aponta para esta linha?» ao
   apagar a mãe. Hoje essa exigência é imposta na **gravação**, não na
   declaração — então a falta só aparece no dia do primeiro `excluir`. Aqui ela
   aparece antes.
3. **Cada linha filha tem mãe VIVA?** A mesma pergunta que o `conferir_fks` faz.

### Por que ele não conserta

Consertar dado do dono sem ele pedir é pior que o defeito. Uma órfã pode ser
lixo de importação, e pode ser a única cópia de um pedido cujo cliente alguém
apagou por engano — e as duas são **indistinguíveis daqui**. Apagar a órfã
destrói a segunda; inventar a mãe inventa dado. O que ele faz é o que só ele
pode fazer: dizer **onde está**, com tabela, chave, rowid e valor.

### Três decisões de relatório

* **Falha de estrutura ≠ falha de linha.** Um índice que falta trava a chave
  inteira; misturar os dois faria o relatório dizer «um milhão de violações»
  quando o que falta é um índice só.
* **Chave sem `verificar` sai marcada.** Órfã sob chave não conferida não quebrou
  promessa nenhuma, e dizer o contrário mandaria o dono consertar o que ele
  escolheu deixar.
* **NULO satisfaz** — o mesmo `MATCH SIMPLE` da gravação. Conferir aqui e não lá
  faria o verificador acusar linha que o motor aceita.

### O que ele custa

A mãe abre **uma vez por chave**, e não por linha: abrir custa 46,8 µs medidos
(`DESEMPENHO.md` §15), e pagá-lo por linha faria a varredura de um milhão de
linhas custar mais de um minuto só em abertura. As linhas contam uma vez por
tabela, e não uma por chave — somar duas vezes faria o relatório inflar o próprio
trabalho.

Tabela que não abre **não trava a varredura**: entra em `nao_abriram`, num balde
próprio. O defeito dela é dela, e somá-lo às violações faria uma tabela
corrompida esconder as órfãs das outras.

## 6. O que ainda não está fechado

* **Filha em outro schema** não é vista pelo `excluir_tabela` nem pelo
  `renomear_tabela` (§2.2). O verificador tem o mesmo alcance: ele varre um
  diretório.
* **A exigência de índice dos dois lados é imposta na gravação, não na
  declaração.** Dá para declarar uma chave conferida sem os índices e só
  descobrir no primeiro `excluir`. O verificador relata; a recusa na declaração
  quebraria a ordem legítima «declare a chave, crie o índice».

## 7. O que MySQL(R) e MariaDB(R) fazem — medido contra o nosso gargalo

Receita de fora se mede contra o nosso gargalo **antes de virar plano**, e é o
que esta seção faz: cada mecanismo dos dois manuais aparece com a citação, com
o veredito para o PhxSql, e com o número quando há número. Três confirmam
desenho nosso, dois viram pedido, e um responde uma pergunta que estava aberta.

### 7.1 O teto da cascata: eles pararam em 15, nós em 16

> «Cascading operations may not be nested more than 15 levels deep.»
> — MySQL 8.4, *FOREIGN KEY Constraint Differences*

O nosso `TETO_DA_CASCATA` é **16**, e foi escolhido sem consultar ninguém, pelo
mesmo raciocínio: é limite de **trabalho**, não detector de ciclo — ciclo
conferido não se popula, e isso está medido. Achar o InnoDB no mesmo lugar não
prova que 16 é certo; prova que a **natureza** do limite é a mesma, e é isso
que vale registrar. **Nada a fazer.**

### 7.2 Conferência imediata: os dois grandes fazem igual, e a pergunta se aposenta

O padrão SQL tem restrições `DEFERRABLE`, adiadas até o commit. **Nenhum dos
dois implementa**: o MariaDB documenta `NO ACTION` como *«Synonym for
RESTRICT»*, e o pedido de recurso (MDEV-26097) segue fechado sem implementação.

O PhxSql confere **na hora**, e isso deixava no ar a dúvida «será que devíamos
adiar?». A dúvida está respondida: adiar não é o que os motores de referência
fazem. **Nada a fazer** — e a pergunta não volta sem alguém trazer um caso.

### 7.3 A cascata não dispara gatilho — coincidência que vira decisão escrita

> «Cascaded foreign key actions do not activate triggers.» — MySQL 8.4
> «Foreign key actions do not activate triggers.» — MariaDB KB

A nossa cascata também não dispara, mas **por acidente de camada**: o `store`
não conhece gatilho, e o interpretador mora no servidor. Comportamento certo
pelo motivo errado é comportamento que a próxima refação quebra sem perceber.
Agora está escrito como **decisão**, com quem mais a tomou.

### 7.4 A auto-referência: eles RECUSAM, nós passamos em silêncio — é defeito

> «If `ON UPDATE CASCADE` or `ON UPDATE SET NULL` recurses to update the same
> table it has previously updated during the same cascade, **it acts like
> RESTRICT**. This means that you cannot use self-referential
> `ON UPDATE CASCADE` [...]. This is to prevent infinite loops.»
> — MySQL 8.4, *FOREIGN KEY Constraint Differences* (a KB do MariaDB traz a
> mesma frase)

O nosso `planejar_ao_alterar` faz `if irma == eu { continue; }`: a
auto-referência **sai do plano sem dizer nada**. `funcionarios.chefe_id ->
funcionarios.id` alterada na chave deixa as filhas apontando para o valor
velho, e o `atualizar` devolve `Ok`.

Os dois motores escolheram o **oposto do silêncio**: recusam a operação. Numa
casa cuja doutrina é que órfã que ninguém vê é pior que órfã que dá erro, o
nosso lado da comparação é o errado. **Virou pedido 174, e FECHOU na mesma
rodada:** onde havia `continue` há recusa nomeando a chave e a tabela, estreita
de propósito — só quando a coluna **referenciada** mudou. Guarda
`auto-referencia-em-silencio`, provada nos dois sentidos.

### 7.5 O índice na declaração: havia uma terceira saída, e nós não a vimos

> «MySQL requires indexes on foreign keys and referenced keys [...] **Such an
> index is created on the referencing table automatically if it does not
> exist.**» — MySQL 8.4, *FOREIGN KEY Constraints*
> «If a foreign key constraint is added to a column without an index, InnoDB
> will automatically create an index to enforce the foreign key constraint.»
> — MariaDB KB

A §6 deste documento diz que exigir índice **na declaração** «quebraria a ordem
legítima *declare a chave, crie o índice*», e por isso a recusa ficou na
gravação. A frase está certa e a conclusão não: os dois motores não recusam
**nem** adiam — eles **criam**. Havia uma terceira saída, e ela não foi
considerada.

Do lado da mãe os dois recusam no DDL (MySQL: erro 1005 / errno 150), porque
criar índice na tabela **alheia** é decisão que não cabe a quem declara a
chave. A assimetria é dos dois, e faz sentido.

**Custo medido do lado que se cria sozinho:** construir índice sai a **2,2 µs
por linha** (`--example custo-do-reindexar-no-arranque`), e na declaração a
tabela costuma estar **vazia** — o caso caro é o de quem declara chave sobre
tabela que já tem dado, e mesmo esse são 219 ms a 100.000 linhas. **Vira
pedido.**

### 7.5.1 A medição do 175: o número é ZERO, e o zero não responde nada

Decisão do dono, 04/09/2026: **medir primeiro** quantas tabelas declaram chave
sem o índice — se a resposta for zero, o pedido vira documentação e não código.
Medido em 04/09/2026, 05:35.

**O instrumento já existia, e usá-lo era a primeira decisão.** A tentação era
escrever um varredor novo; a regra que a impediu é a que esta casa pagou nesta
mesma rodada — *conferidor que discorda de conferidor não acalma ninguém*. A
conferência de índice dos dois lados está em `integridade::conferir_chave`, e
é a **mesma** que a gravação usa; um segundo varredor mediria com outra régua e
teria o direito de discordar. O `--example conferir-integridade` é o
instrumento.

**O corpus desta máquina:** 63 arquivos `.reg`, em cinco diretórios —
`bancada/phxsql` (1), `bancada/profiler/srv-a/base/loja` (30),
`srv-b/base/loja` (30), `srv-log` (1) e `srv-sonda` (1).

| diretório | tabelas | chaves declaradas | violações |
|---|---:|---:|---:|
| `bancada/phxsql` | 1 | 0 | 0 |
| `bancada/profiler/srv-a/base/loja` | 30 | 0 | 0 |
| `bancada/profiler/srv-b/base/loja` | 30 | 0 | 0 |
| `bancada/profiler/srv-log/base/loja` | 1 | 0 | 0 |
| `bancada/profiler/srv-sonda/base/loja` | 1 | 0 | 0 |
| **total** | **63** | **0** | **0** |

**O número é zero, e ele é inútil como resposta.** Zero-porque-tudo-está-
-indexado e zero-porque-ninguém-declara-chave são achados diferentes, e este é
o segundo: a bancada que criou essas 63 tabelas é a do profiler, e ela não
declara chave estrangeira nenhuma (`grep` por `declarar_fk`,
`chaves_estrangeiras` e `estrangeira` em `bancada/profiler/`: nenhuma
ocorrência). **Não há base de produção nesta máquina para medir.** O pedido
continua aberto pela mesma decisão que o mandou medir: o número tem de sair de
uma base real, e a régua para tirá-lo está pronta.

**E a tentativa achou dois defeitos que o número não acharia.**

**(a) O instrumento dizia «limpo» tendo medido nada.** `catalogo::tabelas_em`
devolve lista vazia para caminho que não é diretório — correto para quem abre
uma instância que ainda não tem pasta, e mentira no `--example`: apontado ao
**nível errado** (as tabelas de um servidor moram em
`<servidor>/base/<banco>/`, e a primeira corrida desta medição foi assim) ou a
um caminho **inexistente**, ele imprimia `limpo: nenhuma violacao` e **saía
0**. É o código de saída que um script de manutenção lê. Um erro de digitação
no caminho comprava «limpo» para sempre.

O conserto ficou no `--example`, e não em `tabelas_em`: mudar a função
quebraria os chamadores para quem a lista vazia é a resposta certa. Hoje
`tabelas == 0` sai **2** — o mesmo código de «não deu para varrer», porque é o
que é — e o recado diz onde as tabelas ficam. **Nada conferido não é limpo.**

**(b) O lado da MÃE nunca tinha sido provado.** A conferência é dos dois lados
desde o começo, e o teste era de um só: `indice_que_falta_na_filha_e_falha_de_
estrutura` existia desde a sonda; `SemIndiceNaMae` estava escrito e **nenhum
teste o exercitava**. Contar «tabelas que declaram chave sem o índice» apoiaria
metade da resposta num ramo não provado. O irmão entrou —
`indice_que_falta_na_mae_e_falha_de_estrutura` — com prova real nos dois
sentidos: com o `saida.push(...Falha::SemIndiceNaMae)` reposto por um
descarte, ele **falha**; com o código certo, passa. E o teste da filha
**passou nas duas rodadas**, que é a medida exata do que ele não cobria.

### 7.6 Desligar a conferência deixa resíduo, e o MySQL(R) diz isso ao lado do interruptor

> «**Enabling `foreign_key_checks` does not trigger a scan of table data**,
> which means that rows added to a table while `foreign_key_checks` was
> disabled are not checked for consistency when `foreign_key_checks` is
> re-enabled.» — MySQL 8.4

É exatamente a nossa §4: `copiar_tabela_para` nasce com órfã (medido, com
teste), restaurar backup não confere, `verificar: false` é escolha escrita. A
diferença não está no comportamento — está em **onde a verdade fica escrita**.
O MySQL põe a frase ao lado do interruptor, no manual que quem desliga está
lendo. **Nada a construir; há o que dizer no lugar certo.**

### 7.7 A recuperação: os dois DIVERGEM, e é a divergência que responde o 172

Aqui os manuais não concordam, e a discordância é o achado.

**MariaDB — repara sozinho, e é o padrão de fábrica:**

> «If enabled each time the server opens a MyISAM table, it checks whether it
> has been marked as crashed [...] mysqld will run a check and then attempt to
> repair the table, **writing to the error log beforehand**.»
> `aria_recover_options`, padrão **`"BACKUP,QUICK"`** desde a 10.2.4.

**MySQL/InnoDB — recusa, e manda o DBA ligar à mão:**

> «Only set `innodb_force_recovery` to a value greater than 0 in an emergency
> situation [...] **Values of 4 or greater can permanently corrupt data
> files.**»

Quem está certo depende de **o que o reparo toca**, e é aí que a nossa situação
se decide. O MySQL explica o próprio risco:

> «If the recovery wouldn't be able to recover all rows [...] automatic repair
> aborts with an error message [...] **If you specify `FORCE`**, a warning like
> this is written instead: *Found 344 of 354 rows when repairing*.»

Ou seja: o perigo do reparo automático é **perder linha**, e ele só existe
quando o reparo mexe no arquivo de **dados**. O MariaDB nomeia a metade segura:

> `REPAIR TABLE ... QUICK` — «**will not modify the data file**, only attempting
> to repair the index file.»

**O nosso caso é o `QUICK`, e não o `FORCE`.** O `reindexar()` trunca o `.ndx`
e o reconstrói **lendo o `.reg`**, que está íntegro: o arquivo de dados é
*append-only* e a queda foi na escrita do índice. Nenhuma linha pode se perder,
porque nenhuma linha é reescrita. O risco que fez o InnoDB recusar **não existe
aqui**.

E há um segundo número que muda a leitura, e ele é nosso, não dos manuais: **a
recuperação já reconstrói.** `transacao.rs:1176` chama `reindexar()` para toda
tabela **nomeada na marca**, e conta em `indices_reconstruidos`. O que ela não
alcança é a **filha da cascata**, por motivo estrutural: `completar()` itera
`marca.operacoes`, e o `Escrita` só nasce da tabela pedida explicitamente
(`servidor.rs:8280` e `:8329`) — a cascata nunca vira `Escrita`.

Então o pedido 172 não pergunta «devemos passar a reparar sozinhos?». Nós já
reparamos. Ele pergunta **por que o reparo não alcança a filha**, e a resposta
é a mesma forma de defeito da §5.5.4 do `docs/TRANSACOES.md`: o mecanismo
existe num caminho e não no irmão.

**Custo:** 2,2 µs por linha, linear — 219 ms a 100.000 linhas, 1,16 s a
500.000. É o mesmo que a recuperação já paga pelas outras tabelas, e paga uma
vez, no arranque, por filha que a cascata tocou.
