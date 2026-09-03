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
