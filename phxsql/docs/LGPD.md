# LGPD — a marca de coluna e a trilha `.lgpd`

Duas peças, e elas não são a mesma coisa:

- a **marca** classifica a coluna (`nao`, `pessoal`, `sensivel`). Existe desde
  a v6 do bloco de esquema, e está especificada em `FORMATO.md` §1;
- a **trilha** registra o que acontece com as colunas marcadas. É o `.lgpd`, e
  está especificada em `FORMATO.md` §7.

Este documento é o **porquê** das duas — as decisões, as recusas e os números
que as sustentam.

---

## 1. O pedido, e o que ele já dizia sozinho

> «Todas as colunas ter um atributo LGPD (x): se marcado é um dado sensível e
> deve guardar no arquivo `.lgpd` quando — data e hora, registro único, valor
> antes, valor depois, IP e quem acessou ou alterou. **No insert e delete e
> soft delete não precisa.**»

A última frase é a mais importante do pedido, e ela não é economia. É o
reconhecimento de que **os três já estavam cobertos**:

| evento | onde já estava registrado, antes desta rodada |
|---|---|
| inclusão | `.log`, com rowid, versão e instante |
| exclusão suave | `.log` + `.reason` (quem, quando, por quê) |
| exclusão física | `.log` + `.trash` (a linha inteira) + `.reason` |
| restauração | `.log` + `.reason` |
| **alteração, com antes e depois por coluna** | **em lugar nenhum** |
| **acesso** | **em lugar nenhum** |

A trilha cobre as duas últimas linhas. Registrar as quatro primeiras de novo
criaria **duas verdades sobre o mesmo fato**, e a que ficasse para trás viraria
a que engana quem audita.

O teste que trava isso é `insert_delete_e_soft_delete_nao_geram_trilha`, e ele
tem duas metades de propósito: prova que a trilha **não** grava os três, e prova
que o `.log` e o `.reason` **continuam** gravando. Sem a segunda metade, ele
passaria também num mundo em que a auditoria inteira sumiu.

---

## 2. A correção que veio da tela, e por que ela importa

O pedido original dizia «dado sensível». A tela que o Adriano desenhou depois
mostrou outra coisa: numa tabela `clientes`, ele marcou `nome`, `cpf`, `email`,
`telefone`, `endereco` e `data_nascimento` — que são dado pessoal **comum**
(grau 1), não sensível (grau 2).

Então a marca que liga a trilha é **qualquer grau acima de `nao`**, e não só o
`sensivel`. E o grau **continua existindo**, porque ele responde outra pergunta:
`sensivel` (saúde, biometria, convicção, origem racial) exige base legal própria
e é o que separa `nome` de `prontuário` num relatório jurídico. A caixa de
marcar da tela é o **atalho** de quem cadastra o campo; o grau fica ao lado,
visível e ajustável.

**A caixa nunca manda um booleano para o disco.** Se mandasse, marcar uma coluna
já classificada como `sensivel` a **rebaixaria** para `pessoal` em silêncio — e
o rebaixamento é justamente o que muda o regime legal do campo. Interface nova
não pode apagar em silêncio a classificação que alguém já fez.

### O `limite_credito`, e o valor de não responder

Na tela dele, essa coluna está marcada como «depende». Isso é **informação**, e
não indecisão: existe caso ambíguo, e o padrão para ele é **desmarcado**. A
marca é declaração de quem conhece o negócio, nunca dedução do motor — `cpf` é
óbvio, `documento` não é, e um palpite errado num relatório de conformidade é
pior que nenhum relatório, porque quem lê acredita.

É por isso que a tela de LGPD conta separadamente **quantas colunas estão sem
classificação**. Esse número não se confunde com «não tem dado pessoal»: ele
mede o trabalho que falta.

---

## 3. O defeito que estava lá o tempo todo: a marca que nenhuma tela lia

A marca era gravada (PSCH v6), era lida pelo motor, era devolvida pela op
`esquema` no campo `dado_pessoal` com o grau em texto. E a tela de LGPD
procurava um campo **booleano** chamado `pessoal`, que nunca existiu na
resposta.

Resultado: numa base com seis colunas classificadas, a tela relatava
**«0 colunas marcadas»**.

O que fez o defeito sobreviver tanto tempo foi a **honestidade** da tela: como
não achava o campo, ela dizia «não sei» em vez de «nenhum dado pessoal
encontrado» — e o próprio texto dela explicava por que os dois são diferentes.
Isso a impediu de mentir, e ao mesmo tempo fez o defeito parecer *um servidor
sem marcas* em vez de *uma tela quebrada*.

E havia um segundo buraco: a tela de **Estrutura** da tabela não tinha coluna
LGPD nenhuma — nem exibia, nem editava. A própria tela de LGPD dizia «marcar e
desmarcar é no cadastro de campos da tabela», e esse cadastro **não existia**
como operação: não havia nenhum jeito, por nenhuma porta, de mudar a marca de
uma coluna depois de criada a tabela.

> **A lição, que é prima de uma regra que já estava na casa.** «Configuração que
> não é lida mente» tem um lado espelhado: aqui o campo era lido pelo **motor** e
> ignorado pela **tela**, e o estrago é o mesmo pelo outro lado. Campo de esquema
> sem leitor na tela é pior que campo ausente, porque **quem marca acredita que
> marcou**. E o caso é ainda mais instrutivo que o do `cache_paginas`: lá faltava
> o leitor, aqui o leitor existia e olhava o nome errado — o que nenhum teste de
> motor pega, porque o motor estava certo.

O conserto foi de três partes: a tela passou a ler `dado_pessoal`; a aba
Estrutura ganhou a coluna LGPD com a caixa e o grau, editáveis; e nasceu a
operação `marcar_lgpd`, que regrava o esquema.

### Marcar depois é **declaração**, e por isso é barato

`marcar_lgpd` reusa o caminho que a chave estrangeira já usava
(`RegFile::redeclarar_chaves_estrangeiras`), e pelo mesmo motivo: a marca da v6
é **um byte por coluna no fim do bloco** e não desloca nada — `payload_len` e os
offsets das colunas ficam iguais. Nenhuma linha muda de tamanho nem de lugar, e
nenhum índice precisa ser refeito.

Numa tabela que já é v6 o bloco nem troca de tamanho, então vale sempre o
caminho barato (regravar o cabeçalho volume a volume). Numa gravada antes da v6
o bloco **cresce**, e aí o caminho caro reescreve os volumes com os slots
viajando byte a byte — o mesmo que a FK já fazia.

---

## 4. A decisão difícil: registrar acesso por operação, e não por linha

«Quem acessou» significa registrar leitura de coluna marcada. O desenho ingênuo
— um registro por linha lida, ou pior, por célula — foi **medido e recusado**.

`cargo run --release --example custo-da-trilha -- 5000
# a secao 4 (coluna externa marcada) roda com no maximo 2.000 linhas por
# rodada: o laudo de 64 KiB escreve 128 MB por rodada, e o zelador agradece`, numa varredura de 5.000
linhas com 6 colunas marcadas:

| desenho | tempo | registros | bytes de trilha |
|---|---:|---:|---:|
| **por operação** (o que está no código) | 14.782 µs | **1** | **213** |
| por linha | 35.645 µs | 5.000 | 618.893 |
| por célula (6 colunas) | — | 30.000 | ~3,7 MB |

**Por linha custa 2,41× o tempo e 2.906× os bytes.** Por célula seria mais seis
vezes isso. Numa base que é lida o dia inteiro, a trilha ficaria maior que a
tabela em poucas horas, e o custo cairia em cima da leitura, que é o caminho
quente.

E o registro por operação **não perde a pergunta que o auditor faz**, porque
guarda o **critério** da consulta:

| operação | o que fica gravado na `identidade` |
|---|---|
| `ler` | `rowid=42` |
| `buscar` | `por_cpf=["012.345.678-90"]` |
| `varrer` | `varrer indice=por_nome visao=ativas modo=cursor pular=0` |
| `replicar` (desde 17/09/2026, pedido 285) | `replicar desde=N ate=M`, com `linhas` = eventos que saíram — a mesma forma da varredura, porque `replicar` também entrega todas as linhas de uma faixa de uma vez, com o valor dentro |

«Quem viu o prontuário do fulano?» continua respondível — pela chave, quando a
leitura foi por chave; e pelo filtro mais a contagem, quando foi varredura. O
que se perde é *qual das 5.000 linhas* uma varredura tocou, e isso é honesto:
uma varredura tocou **todas**, e é isso que o registro diz.

Uma consulta que **não devolveu linha nenhuma não grava**: uma busca que não
achou ninguém não expôs dado de ninguém.

O `replicar` segue a mesma regra e foi medido separado (achado A8 da revisão
SEC, 17/09/2026; commit `eeb9925`): **+154 bytes por lote** numa tabela com
coluna marcada; tabela **sem** marca custa só o `bool` da própria conferência
— o teste `…::replicar_sem_coluna_marcada_nao_grava_trilha` prova o
comportamento velho ao lado do novo.

---

## 5. Custo zero quando não há coluna marcada

É a lição do Profiler escrita como código: **o portão vem antes do trabalho**.
A lista de colunas marcadas é montada **uma vez, na abertura da tabela**, e a
pergunta vira um `is_empty()`. Perguntar ao esquema a cada alteração percorreria
as colunas todas por linha gravada.

`cargo build --release --examples -p phxsql-store` antes de medir — binário
velho mede o passado. Com 5.000 linhas:

| tabela | inserção | alteração | registros |
|---|---:|---:|---:|
| **sem coluna marcada** | 10,15 µs/linha | **8,62 µs/linha** | 0 |
| marcada, trilha desligada no config | 9,52 µs/linha | **8,06 µs/linha** | 0 |
| marcada, 2 colunas alteradas | 9,61 µs/linha | 20,20 µs/linha | 10.000 |

A tabela sem marca e a tabela marcada-com-trilha-desligada gastam o mesmo,
dentro do ruído (8,62 contra 8,06 — a **sem marca** saiu ligeiramente mais
lenta, o que só pode ser ruído). O portão custa o que se esperava dele: nada
mensurável. E o arquivo nem chega a existir.

### O que a trilha custa quando ela acontece

**8,62 → 20,20 µs/linha** numa alteração que muda **duas** colunas marcadas: são
~5,8 µs por registro de trilha, ou **2,34×** no caminho da alteração.

Esse número é real e está aqui sem maquiagem. Três coisas o põem em escala:

1. ele só existe em tabela com coluna marcada, e só na **alteração** — inserir
   não paga nada, por desenho;
2. ele é proporcional às colunas que **mudaram**, não às marcadas: salvar a
   ficha sem mexer em nada custa zero registro;
3. cada registro paga duas chamadas de escrita (o registro e o cabeçalho do
   volume), que é exatamente o que o `.reason` já faz por exclusão.

**A hipótese que fica aberta, e ainda não foi medida:** juntar a gravação do
cabeçalho do volume das N colunas de uma mesma alteração numa só. Hoje uma
alteração de 6 colunas marcadas grava 6 cabeçalhos idênticos exceto pelo
contador. Não implementei porque seria mudar o `anexar` que os quatro diários
compartilham em espírito, e porque **medir a premissa vem antes de implementar
o item** — o item aqui é meu, e ele continua sendo palpite até alguém medir se
os 5,8 µs são mesmo a segunda escrita.

### A coluna **externa** marcada: o que ela custa, e o que custava mentir

Pedido 367, medido em 18/09/2026. A trilha de uma coluna `Bin`/`Memo` marcada
afirmava três coisas falsas, e as três saíam da mesma linha: o `atualizar`
decodifica a linha velha com `carregar_externos = false` — porque quem pediu a
decodificação foram os **índices**, que não indexam externo — e nesse modo
`Bin` e `Memo` voltam `Value::Null`. O par antes/depois comparava esse `Null`
com o valor do chamador:

| o que acontecia | o que a trilha dizia |
|---|---|
| alterar um laudo | `antes=""` — **afirmava que o campo estava vazio**, e o valor velho já tinha ido embora com o `liberar_externos` |
| **apagar** um laudo (`Memo` → `Null`) | **nada**: `Null != Null` é falso, e o filtro cortava o evento que a lei mais quer ver |
| salvar a ficha **sem tocar** na foto | `antes="" depois="2048 bytes"` — um registro dizendo que a foto mudou |

O conserto lê o valor velho do bloco **antes** de `liberar_externos` soltá-lo —
a mesma janela de que o `desindexar_texto` já dependia, duas linhas acima. E
lê só as colunas **marcadas** que moram fora do `.reg`: um anexo de dois
megabytes sem marca não entra, e por isso isto não é um
`decodificar(payload, true)`.

O custo, medido com `--example custo-da-trilha` (7 rodadas de 2.000 linhas,
mediana e faixa min–max; o **controle** é a mesma tabela e o mesmo trabalho de
escrita com a marca do laudo desligada):

| laudo | cenário | antes | depois | registros antes → depois |
|---|---|---:|---:|---|
| 2 KiB | controle (sem marca) | 13,85 (13,28–14,34) | 14,00 (11,82–14,56) | 2.000 → 2.000 |
| 2 KiB | marcado, **intocado** | 17,94 (16,82–19,77) | 16,60 (13,99–18,45) | **4.000 → 2.000** |
| 2 KiB | marcado, alterado | 17,67 (16,33–20,92) | 22,66 (18,31–23,88) | 4.000 → 4.000 |
| 64 KiB | controle (sem marca) | 75,08 (71,59–83,58) | 77,14 (66,08–84,65) | 2.000 → 2.000 |
| 64 KiB | marcado, **intocado** | 88,48 (83,53–93,97) | 128,47 (112,21–133,20) | **4.000 → 2.000** |
| 64 KiB | marcado, alterado | 88,37 (76,02–102,06) | 139,15 (124,89–148,43) | 4.000 → 4.000 |

Em microssegundos por linha, **sobre o controle**: com 2 KiB o caso intocado
saiu de +4,09 para +2,60 (faixas cruzadas — é ruído, e a metade dos registros
que sumiu eram os falsos); o caso alterado saiu de +3,82 para +8,66. Com
64 KiB, +13,40 → +51,33 (intocado) e +13,29 → +62,01 (alterado), aí sim com as
faixas **sem se cruzar**.

**O número que põe isso em escala:** ler o bloco velho custa **0,59 µs/KiB**
(37,93 µs a mais por 64 KiB), e o `atualizar` **já pagava 0,99 µs/KiB** só para
regravar o bloco novo daquela mesma coluna (13,85 → 75,08 µs do controle, de
2 para 64 KiB). A leitura que a trilha passou a fazer custa **60% do que a
própria alteração já gastava naquela coluna** — e só em tabela que declarou
`Bin`/`Memo` como dado pessoal, que é ato deliberado de quem cadastrou o campo.

#### O atalho pelo ponteiro, avaliado e **recusado**

O `Ponteiro` de 16 bytes do `.reg` carrega `tamanho` e `crc` do conteúdo.
Comparar o ponteiro velho com o novo decidiria «mudou?» **sem I/O nenhum** —
pouparia exatamente os 37,93 µs/linha medidos acima nos 64 KiB.

Está recusado, e o motivo é do código e não do gosto: `Reg::selar_externo`
sela o conteúdo com um **nonce sorteado a cada gravação** quando a coluna é
externa **e marcada** (`reg.rs`, `externa_marcada`). Numa tabela cifrada, o
mesmo laudo regravado sem uma letra de diferença produz bytes diferentes e
CRC diferente — e o falso positivo voltaria justamente para as tabelas que
mais se protegem. Um atalho que vale só com a cifra desligada seriam dois
comportamentos com o mesmo nome.

#### Quando o valor velho **não** pode ser lido

A trilha só grava o que consegue afirmar. Se o bloco não abre (CRC estragado,
volume perdido), o registro sai com o bit `FLAG_ANTES_INDISPONIVEL` e o texto
`(indisponivel: o valor anterior nao pode ser lido)` — **não** com um `""`, que
afirmaria que o campo estava em branco. A leitura acontece com a linha nova já
gravada, então o erro **não derruba o `atualizar`**: trocar um registro de
auditoria imperfeito por uma gravação perdida seria o pior negócio dos dois.
Quem lê o arquivo decide pelo **bit**, nunca pela frase.

---

## 6. O risco concentrado: este é o arquivo mais perigoso da tabela

O `.lgpd` guarda, em claro, o valor **antes** e o valor **depois** das colunas
marcadas. Ele concentra exatamente o que a lei manda proteger: uma cópia do
`.reg` protege as linhas vivas; uma cópia do `.lgpd` entrega o **histórico** de
todas elas, inclusive das que já foram excluídas.

Vale dizer isto sem rodeio: **guardar trilha de dado sensível em claro é risco
de vazamento concentrado**, e é um risco que esta funcionalidade *cria*. Três
respostas:

1. **`0600` no disco**, aplicado na criação do volume — legível só pelo dono,
   como o cadastro de ligações do DbLink já faz. Com a cifra desligada (o
   padrão), é a única proteção que existe;
2. **a cifra** — ChaCha20-Poly1305, a mesma chave e o mesmo interruptor dos
   outros três diários (`cifra.ligada` no `config.json`). Provado: com a cifra
   ligada, um `grep` no arquivo não acha o valor, e quem tem a chave lê pela op
   `trilha`. A prova traz o **contrário** junto — no volume em claro o mesmo
   `grep` acha, senão o teste passaria com a cifra desligada;
3. **só administrador lê**, pela mesma razão do `.trash` e do `.reason`, levada
   ao extremo: quem lê a trilha não lê «houve uma alteração», lê o CPF velho e o
   CPF novo de todas as linhas de uma vez. Liberá-la por `ler` abriria por uma
   porta lateral tudo o que a permissão por tabela fecha pela porta da frente.

### Senha nunca vai para a trilha — e a redação **analisa**, não recorta

Se uma coluna marcada guardar segredo, o registro grava o **tamanho em bytes** e
uma marca de redigido, e liga um bit de `flags` para a tela mostrar um cadeado
em vez do texto. São duas conferências, e as duas olham **estrutura**:

1. **a coluna declarada.** O nome vem do esquema, que é o lugar onde alguém
   declarou o que aquilo é. Decide antes de olhar o conteúdo — inclusive quando
   o conteúdo é a senha ainda em texto puro, que é o caso pior;
2. **o valor que se analisa como hash.** `senha::e_hash` não procura padrão
   dentro do texto: ele **destrincha** a linha nos quatro campos do formato
   (`pbkdf2-sha256$iterações$sal$derivado`), confere o algoritmo, o número de
   iterações e o hexadecimal dos dois lados. Se destrincha, é um hash — venha da
   coluna que vier, chame-se ela como se chamar.

A segunda é o que pega o hash gravado numa coluna de nome inocente, e é
literalmente o corolário da casa: **redigir analisando, nunca recortando**. A
primeira erra para o lado seguro de propósito — a lista de nomes casa por
*contém*, então uma coluna `hash_arquivo` é redigida sem precisar. Aqui o falso
positivo custa uma linha de trilha que diz «(redigido)» onde podia dizer um
valor; o falso negativo custa uma senha em claro no arquivo mais perigoso da
tabela. Entre os dois erros, este código escolhe sempre o primeiro.

E o que **não se analisa não vira texto, vira tamanho**: `Value::Bin` sai como
`"N bytes"`. Uma biometria é exatamente o dado que a lei manda proteger, e
copiá-la para a trilha seria concentrar o pior num arquivo só.

### A trilha **não** se registra a si mesma

Ler a trilha é acessar dado pessoal, e a pergunta «quem leu a trilha?» tem de
ter resposta. Ela tem, e **não é dentro da própria trilha**. Dois motivos:

1. **a recursão prática.** Cada abertura da tela acrescentaria um registro, que
   apareceria na próxima abertura, que acrescentaria outro. Em pouco tempo a
   trilha de uma tabela seria majoritariamente a história de quem a auditou,
   com os fatos sobre o dado afogados no meio. Auditoria que atrapalha a própria
   leitura não é auditoria;
2. **seria o lugar errado.** A operação exige `Administrar`, e **toda** operação
   que passa por essa porta já é gravada no registro de acessos do servidor —
   data, hora, IP, login, operação, base, tabela e se deu certo. «Quem leu a
   trilha da tabela X, quando e de onde» se responde lá, que é o arquivo de
   quem-chamou-o-quê. A trilha responde outra pergunta: o que aconteceu com o
   **dado**. Misturar as duas faria cada uma responder pior.

---

## 7. Ligada por padrão, e por que isso não quebra a regra da casa

«Guarda nova entra pedida, não imposta» existe para que uma proteção nova não
pare quem escreveu o cliente antes dela. Aqui **nada para**, e vale entender por
quê antes de alguém mudar isto:

a trilha só acontece em tabela que tem coluna marcada, e marcar é um ato
deliberado de quem cadastrou o campo. **Nenhuma tabela que existe hoje sem marca
muda de comportamento** — não ganha arquivo, não paga custo, não responde
diferente. Quem marcou uma coluna já declarou que ali há dado pessoal; a trilha
é a consequência legal dessa declaração.

O interruptor (`lgpd.alteracoes` e `lgpd.acessos` no `config.json`) existe para
quem precise **desligar** — uma carga de migração, um ambiente de teste, um
disco pequeno —, e não para quem precise ligar. Os dois lados são separados
porque custam diferente: a alteração é barata e é a que a lei pede primeiro; o
acesso é o que uma base muito lida gera em volume.

E o teste que mais importa é o do comportamento **velho**: `sem_marca_nada_muda`
abre, lê, grava, verifica e reabre uma tabela sem marca nenhuma, e confere que
o `.lgpd` **não existe** em nenhum momento.

---

## 8. Aprendizados desta rodada

### Frutíferos

- **O portão do custo-zero funciona, e está medido.** Tabela sem coluna marcada
  gasta o mesmo que gastava (8,62 contra 8,06 µs/linha, ruído) e não cria
  arquivo. O portão é uma lista montada na abertura, não uma varredura do
  esquema por linha.
- **Por operação ganha de por linha em 2,41× tempo e 2.906× bytes**, sem perder
  a pergunta do auditor, porque o critério da consulta vai gravado.
- **A redação por análise pega o que a redação por nome não pega.** O teste
  `hash_em_coluna_de_nome_inocente_e_redigido` grava um hash real numa coluna
  chamada `observacao` e prova que ele não sai.
- **A marca existia, era gravada, era devolvida — e nenhuma tela a mostrava.**
  Ver §3. É o achado de maior valor desta rodada, e não estava no pedido.
- **A garantia de «não tocar não gera registro» existia e só cobria coluna
  inline** (pedido 367). O comentário acima do `trilhar_alteracao` jurava que
  salvar a ficha sem mexer em nada não geraria seis registros — e jurava certo
  para as seis colunas `Str`, e errado para a sétima que mora no `.memo`.
  Comentário que se declara resolvido é o motivo de ninguém olhar de novo.
- **Um sentinela que significa duas coisas mente para o segundo chamador.**
  `decodificar(payload, false)` devolve `Value::Null` para dizer «não
  carreguei o externo», e `Value::Null` também é o valor legítimo de uma coluna
  vazia. Para os índices, que foram quem pediu o modo, os dois casos dão no
  mesmo. Para a auditoria, um é «estava em branco» e o outro é «não sei» — e a
  trilha gravava o primeiro nos dois.

### Infrutíferos, e o que eles ensinaram

- **A hipótese do cabeçalho por registro continua aberta.** Suspeito que boa
  parte dos 5,8 µs por registro seja a segunda escrita (o cabeçalho do volume),
  mas **não medi**, então não implementei. Fica escrito para não voltar como
  ideia nova sem número: medir a premissa vem antes de implementar o item.
- **Eu reproduzi uma armadilha que já estava documentada.** Pus o teste do
  interruptor junto com os outros, e ele desligava a trilha no meio da corrida
  paralela: o `acesso_e_um_registro_por_operacao` achava zero onde esperava um.
  **Não falhou na primeira rodada nem na segunda — falhou na terceira**, porque
  é corrida, e corrida não falha sempre. O `diario.rs` já tinha escrito essa
  armadilha com todas as letras; eu li e caí nela.

  Duas consequências: o teste do interruptor foi para um arquivo próprio (cada
  arquivo de teste é um **processo**, e processo não divide global com processo)
  **e** ganhou um mutex, porque separar o arquivo não resolvia a corrida dos dois
  testes dele entre si — eu teria trocado uma corrida por outra menor, que é o
  jeito mais fácil de achar que se consertou alguma coisa. **Teste que falha às
  vezes é pior que teste que falta**: o que falta se vê; o que pisca vira «roda
  de novo que passa» até alguém parar de acreditar na bateria inteira.
- **A prova pelo soquete achou o que o teste unitário não acharia.** A primeira
  rodada da prova da cifra passou nos três primeiros `assert` e falhou nos dois
  do `grep` — e a causa não era o código: o servidor com cifra **não tinha
  subido** («Address already in use»), e eu estava lendo o arquivo do servidor
  antigo, em claro. O teste falhando disse a verdade. Se ele não tivesse a
  prova pelo contrário (o `grep` **acha** no volume em claro), eu teria um teste
  que passaria com a cifra desligada.

---

## 9. Como exercitar

```bash
# os números desta página
cargo build --release --examples -p phxsql-store   # binario velho mede o passado
cargo run --release --example custo-da-trilha -- 5000

# os testes
cargo test -p phxsql-store --test trilha-lgpd
cargo test -p phxsql-store --test interruptor-da-trilha
cargo test -p phxsql-store --lib trilha
```

Pelo protocolo:

```json
{"op":"marcar_lgpd","database":"loja","tabela":"clientes",
 "colunas":{"nome":"pessoal","cpf":"pessoal","laudo":"sensivel",
            "limite_credito":"nao"}}

{"op":"trilha","database":"loja","tabela":"clientes","limite":50}
{"op":"trilha","database":"loja","tabela":"clientes","tipo":"acesso"}
{"op":"trilha","database":"loja","tabela":"clientes","rowid":42}
```

Na tela: **Estrutura** da tabela traz a coluna LGPD (caixa + grau, editáveis), e
**LGPD** no menu traz o mapa das marcas — clicar numa linha abre a trilha
daquela tabela.

---

## 10. Retenção e expurgo da trilha (pedido 368)

Decisão do dono, 24/09/2026: *«Sim, após 5 anos pode limpar ou pelo admin»*.
As duas metades entraram, e as duas chamam **o mesmo motor**
(`Servidor::expurgar_trilha_da_tabela` → `Table::fechar_volume_da_trilha` +
`Table::preparar_expurgo_da_trilha` → `TrilhaFile::planejar_expurgo`):

- **pelo prazo** — `lgpd.retencao_anos` no `config.json`, padrão **5**, `0`
  desliga. Um relógio (`retencao-trilha`) faz uma passada por dia, a primeira
  um minuto depois do arranque. Sem prazo, a thread nem sobe. O **intervalo**
  sai do relógio monotônico (`Instant`) e só o **limite** do prazo sai do
  relógio de parede: um ajuste de hora não cala a passada nem faz duas no
  mesmo dia (achado A6 do papel C);
- **pelo administrador** — a op `expurgar_trilha` (`database`, `tabela`,
  `ate` ou `ate_ms`, `motivo`, e `fechar_ativo` opcional), só `Administrar`.

### A op é do nó (condição C2 do papel C, e o pedido 499)

A trilha é um arquivo **local** de cada servidor — o fio da réplica não a
carrega. Por isso `expurgar_trilha` roda em servidor somente-leitura e em
réplica, sem recusa e sem redirecionamento ao primário, e apaga **a trilha
daquele nó e só a dela**. Cada nó com `retencao_anos` faz a própria passada;
limpar a trilha de todos os nós é mandar a op a cada um.

O irmão `esvaziar_lixeira` tinha o mesmo furo para o `.trash`, medido pelo
soquete com source e réplica de verdade (`tests/lixeira-da-replica.rs`): a
réplica aplica a exclusão do source pelo `excluir_de_vez` de sempre e guarda a
linha inteira no `.trash` **dela**; o `esvaziar_lixeira` do source não vira
evento (o diário continuou em 4 eventos) e não chega lá; e o da réplica
recusava com `[SP000025] acesso negado: servidor em modo somente leitura`. A
linha apagada a pedido do titular ficava no disco da réplica para sempre.

**As duas estão no `OPS_DO_NO`, e continuam no `OPS_ESCRITA`.** A primeira
versão do 499 tirou o esvaziar do `OPS_ESCRITA`, e o parecer do DBA
(`docs/propostas/parecer-dba-faceis-c-2026-09-24.md`) mediu o preço: aquela
lista responde seis perguntas, e tirar a op dela mudou as seis —
`BEGIN; esvaziar_lixeira; ROLLBACK` esvaziava e não voltava, e o esvaziar
passava por cima da trava de outra transação. O `expurgar_trilha` tinha o mesmo
furo desde o 368. Agora a pergunta «grava o dado replicado?» (o portão do
somente-leitura, da réplica de leitura e do cluster) lê uma lista própria, e
as duas continuam escrita para tudo o mais: não entram em transação, esbarram
na trava, contam como escrita na telemetria e no catálogo. Continuam pedindo
`administrar` e `motivo`, e o rastro vai ao `.reason` antes.

**O que o `.trash` por nó muda** — a ressalva que o pedido **297** já cobra
(«mesmo `.trash`/`.reason`: NÃO, por desenho»), e que o 499 tornou ação do
operador, e não só efeito da replicação:

- **(a)** um expurgo por LGPD se manda **a cada nó**: o esvaziar do source não
  chega à réplica, e o da réplica não volta ao source;
- **(b)** depois de uma **promoção**, a lixeira do novo primário é a da
  réplica: a linha que o source antigo já expurgou **reaparece** na `lixeira`
  do promovido, se ninguém esvaziou a réplica;
- **(c)** o **backup** de cada nó leva o `.trash` **dele**: restaurar o backup
  da réplica devolve o que o source já tinha esvaziado;
- **(d)** esvaziar a réplica apaga a **última cópia** de uma linha que o source
  já esvaziou — é a intenção da LGPD, e é também o fim da recuperação por ali.

### Os volumes — formato B

O primeiro corte deste pedido mediu que o expurgo por volume **não alcançava a
tabela padrão**: sem `registros_por_arquivo`, a trilha era um arquivo só, o
ativo para sempre (§«A premissa», abaixo). O papel C decidiu o formato B, e ele
entrou na mesma versão (`FORMATO.md` §7, «Nomes e volumes»):

- o **ativo** é sempre `<tabela>.lgpd`; os **fechados** são
  `<tabela>_NNN.lgpd`, com `NNN` igual ao `volume` do cabeçalho — no mínimo
  três dígitos, **sem teto**, nunca reusado;
- a trilha **não segue** a paginação do `.reg`: nem `registros_por_arquivo`,
  nem `max_arquivos`, nem `recursos.diario_volume_mib`;
- o ativo **fecha** por tamanho (`lgpd.volume_mib`, padrão 64, conferido no
  append), por **idade** do primeiro registro acima de `lgpd.volume_dias`
  (padrão 30, conferida na passada e na op, nunca no laço quente) ou a
  **pedido** (`fechar_ativo`). Fechar é renomear e fazer nascer o seguinte;
  nenhum byte de registro ou de cabeçalho muda, e o ativo vazio não fecha;
- o ativo nasce com o **maior fechado + 1**, por uma listagem do diretório,
  uma vez na vida dele — e uma queda entre o `rename` e o nascimento cai na
  mesma regra;
- trilha gravada antes abre **sem reescrita**: o arquivo único vira o ativo
  volume 1; a paginada `_001`…`_K` vira fechados e o ativo nasce `K+1`; a de
  sufixo `_0001` é achada pelo nome antigo.
- o ativo **mais curto que o cabeçalho** é nascimento interrompido pela queda:
  conta como ausente, a tabela abre, e o próximo evento o faz nascer de novo
  com o mesmo número. Sem isso, um `clientes.lgpd` de 0 byte trancava a
  tabela inteira com «failed to fill whole buffer» (segunda revisão do papel
  C, B2 — o arquivo de 0 byte foi simulado com `set_len`, e a queda que o
  produz não foi reproduzida contra o sistema operacional);
- sem ativo, **o expurgo o faz nascer antes** de apagar, e o selo o leva ao
  disco junto do rastro: o número dele só existia na listagem dos fechados que
  o expurgo apaga (P3).

**E o nome da tabela não pode ser o de um volume** (segunda revisão, B1): o
ativo da trilha de `x_001` é `x_001.lgpd` com volume 1 — nome e número do
fechado 1 da trilha de `x` —, e o papel C mediu o expurgo de `x` apagando-o.
Nenhuma conferência de cabeçalho separa os dois; quem separa é a
**declaração**. `criar_tabela`, `duplicar_tabela`, `copiar_tabela_para` e
`renomear_tabela` fazem a mesma pergunta por uma função só
(`exigir_nome_que_volta`, no `catalogo.rs`): o nome que o catálogo não lê de
volta como ele mesmo é recusado. `x_001` e `pedidos_2025` recusam;
`x_historico` passa. O sufixo de **letra** ainda não: `x_A` nasce e some da
árvore (pedido 506; o separador de volume é o 508). A prova do `copiar_tabela_para` achou um teste antigo
que colava para `pedidos_2026` — a cópia nascia e sumia da árvore do destino.

Com isso, **o maior tempo de guarda de um registro é o prazo + `volume_dias` +
cerca de dois dias** — duas passadas diárias, a que fecha o volume dele e a
que o apaga —, com o servidor no ar: 5 anos e ~32 dias no padrão. E **nunca
antes** do prazo: o volume só sai quando o registro mais novo dele passou do
limite.

E o teto de volumes que o papel C mediu (achado A1: com `max_arquivos 3`, a
71ª alteração gravava a linha e o `.log` e **falhava na trilha**) deixou de
existir para a trilha, porque o número dela não tem fim. O mesmo teto continua
valendo para o `.log` — no mesmo esquema, ele bate na alteração 135 —, e isso
é o outro lado do A1, que não é deste pedido.

### A regra de quem sai

**Volume inteiro, nunca registro.** O `.lgpd` é append-only e o UUID v7 ordena
por tempo: derrubar a janela mais velha inteira apaga o que venceu sem
reescrever arquivo nenhum — e com a cifra ligada, cortar um volume ao meio
seria re-selar cada registro num offset novo.

1. Sai o volume **fechado** cujo registro **mais novo** é anterior ao limite.
2. O **volume ativo nunca sai**, nem com todo registro vencido. A passada
   fecha o ativo velho **antes** de planejar; o que sai é o volume que acabou
   de fechar, se já passou do prazo.
3. O volume de **fronteira** (algum registro do limite em diante) fica
   **inteiro** — e com ele todos os seguintes. O que sai é sempre um
   **prefixo**: a trilha que sobra começa num instante e não tem buraco no
   meio, então «temos tudo desde X» continua verdade.
4. «Mais novo» é o **máximo** dos carimbos, conferido registro a registro — e
   não o último: um ajuste de relógio para trás faz um registro do meio ser mais
   novo que o último.
5. Todo registro que conta para **sair** tem o CRC conferido; carimbo que não
   confere para o expurgo com erro, e nada sai. O CRC cobre o corpo como vai ao
   disco: o plano não precisa da chave da cifra.

O prazo em anos é de **calendário** (`datahora::recuar_anos`): cinco anos antes
de 24/09/2026 10h é 24/09/2021 10h. De 29/02 para um ano comum cai em **28/02**
— o dia mais cedo, que apaga menos e nunca antes.

### Três fases, e a do meio fora da trava global

1. **com a trava**: fecha o ativo (por idade ou a pedido), planeja e grava o
   rastro no `.reason`;
2. **sem a trava**: leva o rastro ao disco (`fsync` por um descritor próprio,
   **sem** consumir as marcas de escrita do processo — consumi-las fora da
   trava tiraria o `fsync` da escrita de outra instância);
3. **com a trava**: apaga os volumes.

A ordem é a do `esvaziar_lixeira` — o motivo sobrevive ao dado — e aqui ela é
**tipo**: só `Expurgo::selar` devolve o `ExpurgoSelado` que a fase 3 aceita.
Fazer o `fsync` com a trava na mão, como o irmão faz, subiria a catraca
`alcancam-fsync-2` do mapa da trava de 23 para 24; medido depois do conserto,
ela continua em **23**. O preço de soltar a trava no meio é pago na fase 3: ela
confere de novo que nenhum volume planejado virou o ativo e relê o **bilhete**
de cada um (UUID do primeiro registro + fim lógico) — uma tabela excluída e
recriada com o mesmo nome no meio do caminho teria os mesmos números de volume
e outro conteúdo. Qualquer divergência recusa **antes** do primeiro `unlink`.
E um `Mutex` do processo deixa um expurgo por vez, para o relógio e o
administrador não planejarem o mesmo volume duas vezes.

**O rastro é a intenção selada; quem diz o que saiu é o diretório** (achado A5).
Pode sobrar rastro sem apagamento — queda entre as fases, recusa na fase 3, ou
um `unlink` que falha no meio. No último caso o erro devolve a **lista
parcial** («parou no volume 2 (…); já tinham saído [1]»), em vez de perder o
que já tinha acontecido. A passada seguinte recomeça do diretório.

### O rastro

No `.reason` da tabela: tipo `4` (expurgo), rowid `0`, o usuário de quem pediu
(`0` no relógio, que é o próprio servidor), o `motivo`, e na identidade
`.lgpd volumes 1-3 (3 volume(s), N registro(s)); mais novo …; limite …`.
**Nenhuma chave de linha** — a trilha guarda a chave primária em texto em cada
registro, e o rastro de que ela foi apagada não pode ser o lugar onde ela
sobrevive. Quem chamou, de que IP e quando fica também no `acessos.log`, como
toda op. Sem volume a derrubar não há rastro: nada aconteceu.

**Quem separa este expurgo do `esvaziar_lixeira` é o bit 0 do byte 9** do
registro (`FLAG_EXPURGO_DA_TRILHA`, condição C1 do papel C), e nunca o texto
da identidade. A op `motivos` o mostra como `"expurgo_da_trilha": true`. Não é
um tipo `5` porque o binário anterior recusaria o `.reason` inteiro; o byte 9
estava sempre em 0, é coberto pelo CRC e pela etiqueta da cifra, e o leitor de
antes não o lê (`FORMATO.md` §6).

### O que a resposta diz — inclusive o que NÃO fez

`volumes` (número, registros, mais novo, bytes), `registros`, `restam`,
`fechou` (o volume que fechou nesta chamada, por idade ou a pedido), e a
**parada** pela chave: `volume_ativo`, `fronteira` ou `sem_trilha`, com
`retido_vencido_desde` preenchido quando ficou dado vencido num volume que
ainda não pode sair inteiro — ele sai quando o volume fechar. O relógio
escreve uma linha por passada no log do servidor, com os ativos fechados por
idade, e ela **nomeia** as tabelas com dado vencido retido.

### A premissa, medida — e o que a fechou

O pedido partiu de «a trilha já é volumada pelo mesmo corte dos diários». Na
primeira entrega isso era verdade **só para tabela paginada**, medido pelo
fonte e pelo disco:

- **tabela sem `registros_por_arquivo`** — o padrão do `criar_tabela` — tinha
  a trilha num **arquivo único** (`clientes.lgpd`), o ativo **para sempre**: o
  expurgo por volume **nunca** a alcançava;
- **tabela paginada** cortava a trilha em `bytes_por_arquivo` do esquema —
  **1 GiB** por padrão. A 139 bytes por registro, 1 GiB são **~7,7 milhões**
  de registros: a mil eventos por dia — conta sobre o número medido, não
  medida —, **~21 anos** para fechar o primeiro volume.

O formato B fecha os dois: o corte por **idade** faz a tabela de pouco
movimento fechar volume a cada 30 dias, e o corte por **tamanho** passou a ser
da trilha (64 MiB: ~483 mil registros de 139 bytes — conta, não medida).
Provado contra o sistema operacional com a tabela **padrão** (abaixo).

### O custo, medido

`cargo run --release --example custo-do-expurgo -p phxsql-store -- 64 8`
(24/09/2026, formato B, cache do núcleo **quente**, 5 repetições, 490.000
registros em 9 volumes de 8 MiB, 139 bytes/registro):

| o quê | mediana | min / max |
|---|---:|---:|
| plano com **tudo vencido** (anda por 56 MiB fechados) | **67,4 ms** (1,05 ms/MiB) | 47,6 / 67,7 |
| plano com **nada vencido** (para no primeiro registro) | **0,096 ms** | 0,096 / 0,114 |
| `ler(0, 0)` da trilha inteira — referência, **não** o mesmo trabalho | 577,2 ms | 576,2 / 587,7 |

O «nada vencido» caiu de **0,94 ms** (primeira entrega, mesmo medidor) para
**0,096 ms**. A explicação provável é a listagem: a trilha paginada de antes
perguntava pela existência dos `max_arquivos` números (999 `statx`, que o papel
C mediu em 625 µs), e a do formato B anda do ativo para baixo até faltar um —
**provável, não medida aqui** por contagem de chamadas.

O plano roda com a trava global na mão. O caso caro é o de **fronteira**: a
passada diária anda pelo pedaço vencido do volume que atravessa o limite até
achar o primeiro registro novo. Com o corte de 64 MiB, até ~67 ms por passada
— **conta** sobre o 1,05 ms/MiB medido, quente; frio custa mais e não foi
medido. Derrubar é um `unlink`; fechar é um `rename`.

### O que o expurgo NÃO alcança, dito em vez de escondido

- **cópias de backup já feitas** guardam os volumes que o expurgo apagou; o
  prazo delas é o `backup.manter`;
- **o `unlink` e o `rename` não são duráveis contra queda de energia**: nenhum
  `unlink` nem `rename` desta casa faz `fsync` do diretório (pedido 467), e o
  fechamento do formato B segue os irmãos. Numa queda logo depois, um volume
  apagado pode voltar — o dado nunca some a mais, pode sobrar, e a passada
  seguinte o derruba de novo; um fechamento pode voltar ao nome de ativo, e
  a trilha abre pelo cabeçalho dele como antes;
- **a trilha de outro nó**: a op e a passada são do nó (acima);
- **o auditor que pagina a exportação com `pular`** pode pular registros sem
  aviso se um expurgo acontecer entre duas páginas (achado A4, fora deste
  pedido; o formato B não o muda).

### Hipóteses, e as que morreram

- **Decidir pelo ÚLTIMO registro do volume** (uma leitura só, append-only):
  **morreu** — relógio que volta faz um registro do meio ser o mais novo, e o
  volume sairia com registro dentro do prazo. Prova:
  `o_relogio_que_volta_nao_engana_o_criterio`.
- **Decidir pelo registro mais VELHO**: derruba o volume de fronteira. Prova:
  `o_expurgo_derruba_so_volume_inteiro_e_para_na_fronteira`.
- **Expurgar por registro, reescrevendo o volume**: recusada no parecer do DBA
  e confirmada aqui — reescreve arquivo append-only e, cifrado, re-sela cada
  registro num offset novo.
- **`fsync` do rastro com a trava na mão, como o `esvaziar_lixeira`**:
  funcionaria, e subiria a catraca `alcancam-fsync-2` para 24. Recusada.
- **Um tipo `5` no `.reason`**: o binário anterior recusa o arquivo inteiro.
  Morreu no parecer do papel C; entrou o bit do byte 9.
- **Separar os dois expurgos pelo prefixo `.lgpd` da identidade**: é decidir
  pela frase. Morreu pelo mesmo parecer (achado A3).
- **Numerar sempre e cortar só por tamanho** (H1 do papel C): a tabela de pouco
  tráfego nunca fecha volume. **Cortar por tempo reescrevendo** (H2): reescreve
  append-only. Venceu H3, o ativo de nome fixo — PG, MariaDB e MySQL convergem
  em rodar por tamanho e a pedido; por tempo, 6 × 3; nome fixo, 5 × 4.

### As provas (cada uma falha com o defeito reposto)

| teste | defeito reposto que o derruba |
|---|---|
| `o_expurgo_derruba_so_volume_inteiro_e_para_na_fronteira` | critério pelo registro mais velho; o append que nunca fecha por tamanho |
| `o_relogio_que_volta_nao_engana_o_criterio` | critério pelo último registro |
| `o_expurgo_so_derruba_prefixo_e_nao_abre_buraco` | seguir adiante depois da fronteira |
| `o_volume_ativo_nunca_sai_mesmo_vencido` | tirar a guarda do ativo no plano |
| `a_fase_de_apagar_recusa_o_volume_ativo_mesmo_num_plano_adulterado` | tirar a guarda do ativo no `unlink` |
| `o_bilhete_recusa_volume_trocado_entre_o_plano_e_o_unlink` | tirar a conferência do bilhete |
| `o_unlink_que_falha_no_meio_diz_o_que_ja_saiu` | o erro do `unlink` subir cru, sem a lista parcial |
| `selar_leva_o_rastro_ao_disco_e_recusa_sem_rastro` | tirar o `fsync` do selo |
| `a_trilha_continua_depois_do_expurgo` | `existentes()` andando de 1 para cima |
| `a_tabela_sem_paginacao_fecha_por_idade_e_expurga` | `fechar_se_velho` que nunca fecha |
| `o_fechamento_por_idade_so_passa_do_corte` | fechar sem olhar a idade |
| `o_fechamento_pedido_renomeia_e_nasce_o_seguinte` | fechar o ativo vazio |
| `a_queda_entre_o_rename_e_o_nascimento_cai_na_mesma_regra` | sem ativo, supor o volume 1 em vez de listar |
| `volume_com_outro_numero_no_cabecalho_e_recusado` | não conferir o número do cabeçalho contra o nome |
| `o_ativo_mais_curto_que_o_cabecalho_e_nascimento_interrompido` | a abertura sem a regra do nascimento interrompido («failed to fill whole buffer») |
| `nascimento_interrompido_pelo_tamanho_e_pela_versao` | a régua sem o caso do cabeçalho cifrado cortado |
| `a_tabela_abre_com_o_ativo_da_trilha_de_zero_byte` (`tests/trilha-lgpd.rs`) | a abertura sem a regra: `Table::abrir` tranca, o P1 do papel C |
| `o_expurgo_depois_da_queda_nao_reusa_o_numero` | o plano sem o nascimento do ativo que falta (volume reusado); o selo sem o `fsync` dele |
| `criar_recusa_nome_que_o_catalogo_leria_como_volume` (catálogo) | o `criar_tabela` sem a pergunta: `x_001` nasce |
| `duplicar_e_copiar_recusam_nome_que_o_catalogo_leria_como_volume` (catálogo) | o `duplicar_tabela` ou o `copiar_tabela_para` sem a pergunta |
| `renomear_recusa_nome_que_o_catalogo_leria_como_volume` (catálogo) | o `renomear_tabela` sem a pergunta |
| `a_trilha_paginada_de_antes_vira_fechados_e_o_ativo_nasce_depois` (fixture) | sem ativo, supor o volume 1 |
| `a_trilha_de_sufixo_de_quatro_digitos_de_antes_continua_legivel` (fixture) | não procurar o nome antigo |
| `o_bit_do_expurgo_separa_a_trilha_da_lixeira_no_disco` | o rastro sem o bit do byte 9 |
| `o_teto_de_volumes_da_trilha_nao_existe_mais` | o código de antes (`82a17ef`): cai na alteração 64 |
| `o_relogio_da_retencao_fecha_por_idade_e_so_derruba_o_que_passou_do_prazo` (servidor) | limite = agora; a passada que não fecha antes do plano; `fechar_se_velho` que nunca fecha; fechar sem olhar a idade |
| `o_expurgo_pede_administrar_e_roda_no_servidor_somente_leitura` (servidor) | `expurgar_trilha` de volta no `OPS_ESCRITA`; `fechar_ativo` ignorado |
| `o_administrador_expurga_so_volume_fechado_e_o_rastro_fica` (servidor) | o rastro sem o bit do byte 9 |
| `prazo_zero_desliga_o_relogio_antes_do_trabalho` (servidor) | tirar o portão do prazo zero |
| `a_retencao_da_trilha_nasce_em_cinco_e_recusa_o_torto` (config) | ler o prazo com `inteiro_ou` |
| `o_corte_da_trilha_nasce_em_64_e_30_e_recusa_o_torto` (config) | aceitar corte zero |

As fixtures em `tests/fixtures/trilha-antes-do-b/` são trilhas **gravadas pelo
código de antes** (commit `82a17ef`), com o gerador no `LEIA-ME.md` ao lado — a
migração se prova abrindo o que a versão anterior escreveu, e não o que esta
imagina que ela escreveria.

E as de ponta a ponta, conferidas pelo **diretório** e não pelo que a tabela
diz de si: `o_expurgo_pela_tabela_padrao_grava_o_rastro_e_so_derruba_volume_fechado`
(`tests/trilha-lgpd.rs`) e `o_expurgo_da_trilha_cifrada_decide_sem_abrir_o_corpo`
(`tests/cifra-dos-diarios.rs`, quatro volumes fechados a pedido e os 200
registros decifrados antes e 50 depois).

E a prova **contra o sistema operacional**, com o `phxsqld` de verdade, a
tabela **padrão** (sem `registros_por_arquivo`) e o `config.json` lido pelo
binário (`lgpd.volume_mib: 1`, `lgpd.retencao_anos: 5`):
`python3 bancada/lgpd/prova-do-expurgo.py`. Corrida de 24/09/2026: 1.200
alterações de uma coluna marcada em 0,3 s fecharam 2 volumes de 1 MiB
(`clientes_001`, `clientes_002`) com o ativo `clientes.lgpd` recebendo; o
leitor sem `administrar` foi recusado e o `ate` de 2099 também, sem nada sair;
o expurgo do administrador levou 2 volumes e 1.120 registros e o diretório
ficou só com `clientes.lgpd`; o rastro saiu no `.reason` com o bit C1 e sem o
CPF; a op `trilha` leu os 80 que sobraram; a trilha seguiu para
`clientes_003.lgpd`; e `fechar_ativo` fechou o ativo 4 e o levou no mesmo
expurgo. Por fim, **o relógio sozinho, pelo prazo**: com o servidor parado, os
560 registros de `clientes_005.lgpd` foram para seis anos atrás e os 40 do
ativo para quatro (CRC refeito pelo script, conferido antes pela mesma régua);
o servidor subiu de novo e, **60 s** depois, a primeira passada derrubou o
volume de seis anos, fechou o ativo de quatro **por idade** e o guardou
(`clientes.lgpd` + `clientes_006.lgpd` no `ls`), com a linha
`retencao da trilha: 1 volume(s) e 560 registro(s) expurgados em 1 tabela(s)
com trilha, 1 ativo(s) fechado(s) por idade` no log e o rastro assinado pelo
usuário `0`.

Os dois vermelhos dela, com o binário refeito com o defeito:

- **o append que nunca fecha por tamanho** — o comportamento da tabela padrão
  antes do formato B: `a trilha da tabela padrao nao virou volume: 20000
  alteracoes em 4.6 s, e no disco ['clientes.lgpd']`. Numa corrida anterior,
  antes de o script ganhar o teto de alterações, o mesmo defeito deixou
  **1,87 GB num arquivo só** em menos de cinco minutos;
- **`fechar_se_velho` que nunca fecha**: a passada derrubou o volume de seis
  anos mas não fechou o ativo — `0 ativo(s) fechado(s) por idade`, e o `ls`
  sem `clientes_006.lgpd` —, duas falhas.

**Sem prova de vermelho**: o intervalo do relógio por `Instant` (A6). Está no
tipo — `Option<Instant>` e `Duration` —, mas provar que um ajuste de hora não
cala a passada pediria injetar relógio na thread, e isso não entrou.

```bash
cargo test -p phxsql-store --lib trilha::testes
cargo test -p phxsql-store --test trilha-lgpd
cargo test -p phxsql-store --test cifra-dos-diarios expurgo
cargo test -p phxsql-server --lib testes_expurgo_da_trilha
cargo run --release --example custo-do-expurgo -p phxsql-store -- 64 8
python3 bancada/lgpd/prova-do-expurgo.py      # depois de cargo build --release
```

```json
{"op":"expurgar_trilha","database":"loja","tabela":"clientes",
 "ate":"2021-09-24","motivo":"prazo de guarda vencido"}
```
