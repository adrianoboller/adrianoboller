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

`cargo run --release --example custo-da-trilha -- 5000`, numa varredura de 5.000
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

«Quem viu o prontuário do fulano?» continua respondível — pela chave, quando a
leitura foi por chave; e pelo filtro mais a contagem, quando foi varredura. O
que se perde é *qual das 5.000 linhas* uma varredura tocou, e isso é honesto:
uma varredura tocou **todas**, e é isso que o registro diz.

Uma consulta que **não devolveu linha nenhuma não grava**: uma busca que não
achou ninguém não expôs dado de ninguém.

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
