# Changelog

Tudo que mudou no PhxSql, do mais novo para o mais antigo.

Formato: cada versão traz **Corrigido** primeiro — defeito é o que o leitor
precisa achar rápido —, depois **Adicionado**, **Mudado** e **Sabido**. A
seção *Sabido* lista o que ainda não funciona, para ninguém descobrir sozinho.

Os números são **medidos**, nunca estimados.

---

## 0.15.0 — 2026-08-28

**Replicação funcionando**, **carga em lote** e o **salto para a página 500** —
os três estavam escritos como o que faltava, e os três saíram.

### Corrigido

- **A bissecção pelo `rownum` estava errada na partição alfanumérica, e errada
  em silêncio.** Ali o `rownum` não cresce com o rowid: a Silva digitada
  primeiro mora no `_S`, com rowid alto, e a Alves digitada depois mora no
  `_A`, com rowid 1 — número de ordem 1 num rowid maior que o do número 2.
  Bissetar uma sequência que não está ordenada devolve a linha errada sem
  reclamar. Nesse modo o motor agora varre, procurando o **menor** número de
  ordem maior ou igual ao alvo. Teste novo em `tests/alfanumerica.rs` prova
  que os rowids saem fora de ordem — e falha se um dia saírem crescentes, para
  não continuar provando outra coisa.

- **`phxsql listar` lia a tabela inteira para mostrar vinte linhas.** Numa
  tabela de 200.000 com memo, 382 ms para uma tela que cabe no terminal.
  Agora o teto entra na leitura, e o comando ganhou `--pular`.

- **Duas sobras da versão anterior**: um comentário duplicado no caminho da
  importação e um doc-comment órfão de função que mudou de arquivo. Zero
  avisos do clippy de novo.

### Adicionado

- **A replicação Master → Réplica está no ar.** Quatro servidores medidos em
  `bancada/replicacao/`, com o Master e três espelhos:

  | | |
  |---|---|
  | Master, com a imagem no diário | 18.773 linhas/s |
  | Aplicação, por réplica (as três em paralelo) | 4.273 eventos/s |
  | Atraso de uma escrita até as três | 1,3 s a 2,1 s |
  | Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 343 ms + 1,0 s |
  | Retrato SHA-256 das quatro tabelas, no fim | idênticos |

  A bancada não compara «quantas linhas»: compara um SHA-256 de **cada linha
  inteira**, com `rowid` e `rownum` juntos. O `rowid` entrar na conta é o
  ponto — ele não é transmitido: o `.reg` nunca reaproveita slot, então uma
  réplica que aplicou tudo na ordem chega ao mesmo número sozinha. Se não
  chegar, divergiu, e a replicação **para ali** em vez de espalhar.

- **`.log` v2 com a imagem da linha.** Era a única peça que faltava, e ela é
  o payload **cru** do `.reg` mais o **conteúdo** dos anexos — não os
  ponteiros, que são offsets desta máquina e apontariam para qualquer coisa na
  outra. Atrás de `replicacao.imagem_da_linha`, ligada sozinha num `source`.
  Medido, mesma tabela e mesmas 100.000 linhas: **10% mais devagar e um diário
  5,1× maior** (44 → 223 bytes por evento).

- **`posicao`, `replicar` e `aplicar` no protocolo**, e o laço da réplica
  dentro do próprio `phxsqld` — uma thread por origem, `papel: replica` e uma
  origem no `config.json` bastam. A tabela que ainda não existe na réplica
  nasce do **bloco de esquema cru** do source, e não de uma remontagem coluna a
  coluna a partir de JSON.

- **A senha da réplica não fica em claro nem viaja.** Ela se autentica pelo
  mesmo desafio-resposta do resto do protocolo, com a chave derivada do
  `senha_hash` que mora no `config.json` dela.

- **Cascata**: uma réplica pode ser origem de outra. Master → Slave01 → Slave03
  mediu 1.827 ms contra 1.679 ms do primeiro salto.

- **`inserir_lote`: várias linhas num pedido só.** Medido com 20.000 linhas
  pela rede, contra o mesmo trabalho linha a linha: **2.715 → 25.985 linhas/s
  (9,6×)**. O ganho não é do disco — cada linha custa o mesmo lá dentro — e sim
  de tudo que acontecia POR LINHA e passa a acontecer uma vez: abrir os sete
  arquivos, tomar a trava, o `fsync`.

- **Colar em vez de montar.** O mesmo pedido aceita texto em **JSON, CSV, TXT,
  XML e HTML**, e adivinha o formato pelo conteúdo. A primeira linha manda: as
  colunas casam pelo **nome**, não pela posição. `importar_conferir` lê e
  mostra o que entendeu sem gravar nada — é o que a tela de Importar usa, e o
  botão de gravar só acende depois que a conferência passa. Na linha de
  comando, `phxsql importar`.

- **`pular` deixou de andar até a posição.** Quando a posição de uma linha na
  lista *é* o `rownum` dela, o início da página sai de uma bissecção. Medido
  numa tabela de 200.000 linhas, pelo protocolo, pedindo 200 linhas:

  | `pular` | bissecção | passo |
  |---:|---:|---:|
  | 200 | 7 ms | 6 ms |
  | 20.000 | 7 ms | 18 ms |
  | 100.000 | 6 ms | 72 ms |
  | 199.800 | 6 ms | **131 ms** |

  A bissecção é **plana** — e os 6 ms dela são decodificar e serializar as 200
  linhas, não achar o começo. Dentro do motor, sem a rede e sem a serialização:
  **180 µs contra 55 ms** no meio de uma tabela de 200.000, e **164 µs contra
  246 ms** numa de 800.000. Os dois caminhos devolvem a mesma página — o
  exemplo `custo-da-pagina` afirma isso e falha se deixar de ser verdade.

- **`salto` na resposta do `varrer`**: `"bisseccao"` ou `"passo"`. A diferença
  entre os dois é de ordem de grandeza, e quem monta uma tela grande precisa
  saber qual está pagando — e o que fazer com a tabela para pagar o outro.

- **`visiveis` voltou a existir na resposta, e agora é barato.** Sai de dois
  contadores do cabeçalho: `registros − marcadas` são as ativas, `marcadas` são
  as excluídas. Era por essa conta não existir que o `total` tinha saído na
  0.14.0. Com ela, «página 3 de 40» voltou para a grade sem custar varredura.

- **Caixa «ir para a página» na grade**, com o botão `fim ⏭` ao lado. Salto
  para a página 500 de uma tabela de 200.000: **116 ms** medidos no navegador,
  incluindo o desenho da tela. O número da página sobrevive a navegar por
  cursor: `anterior` desconta um, `próxima` soma um.

- **`desde_rownum` no `varrer`**: a página que começa no número de ordem N,
  inclusive. É o cursor de quem guardou o número de ordem em vez do rowid.
  `rownum_inicio` e `rownum_fim` vêm na resposta.

- **`--pular` no `phxsql listar`**, e o rodapé diz por onde a página foi
  achada e qual o `--pular` da próxima.

### Mudado

- **`.log` v1 → v2**: o cabeçalho do evento passou de 36 para 44 bytes, e o
  evento deixou de ter largura fixa. Isso cobra um preço: até a v1 o evento N
  morava no offset `64 + N × 36` e pular era uma conta; agora chegar ao evento
  N é caminhar pelos anteriores. O que salva a leitura é o `qtd_eventos` do
  cabeçalho de cada volume — um volume inteiro se pula sem abrir.

- **O CRC do evento passou a cobrir a imagem**, e não só o cabeçalho. A imagem
  é o que a réplica grava **como dado**: um byte trocado ali entraria na
  réplica sem ninguém notar.

- **`.reg` v3 → v4**: o contador `marcadas` nos bytes 108..116 do volume 1.
  Arquivo da v3 não abre — e não abrir é o ponto: ele traria zero ali, zero
  quer dizer «nenhuma linha marcada», e o motor concluiria que a posição é o
  `rownum` numa tabela onde não é. A página sairia errada em silêncio.

- **O contador de marcadas vai ao disco na mesma operação que o muda**, e não
  no `sincronizar` — 128 bytes a mais por exclusão suave. Um contador que só é
  gravado depois volta atrás numa queda, e este não é número de vitrine: é ele
  que decide se o salto pode confiar no `rownum`.

- **`verificar` reconta as marcadas varrendo** em vez de acreditar no
  cabeçalho, e corrige de passagem. `Relatorio` ganhou o campo. É o mesmo
  caminho que o reparo chama.

### Sabido

- **A réplica aplica mais devagar do que o master escreve** — 4.273 eventos/s
  contra 18.773 linhas/s, com as três competindo pela mesma máquina. Sob carga
  sustentada elas ficam para trás. A razão está no caminho: aplicar decodifica
  a imagem para `Value` e **reencoda** o payload, em vez de gravar os bytes que
  vieram. Gravar o payload direto, remendando só os ponteiros dos anexos, é o
  próximo ganho grande.

- **O atraso da réplica é o intervalo do laço, não o trabalho.** Com
  `reconectar_em: 2` uma escrita leva de 1,3 s a 2,1 s para chegar. Baixar o
  intervalo baixa o atraso e sobe o tráfego de perguntas em vão; o `long-poll`
  — o source segurar a resposta até ter novidade — ainda não existe.

- **O JSON da replicação vai em claro**, e a imagem vai em hexadecimal, que
  dobra o tamanho. Não há TLS no transporte: por enquanto ele depende do túnel.

- **Não há transação, e o lote não muda isso.** Se a linha 700 de mil falhar,
  as 699 anteriores ficam gravadas: o `.reg` não reaproveita slot, então
  desfazer deixaria 699 buracos. Por isso o padrão é parar na primeira
  recusada; quem importa dado sujo de propósito passa `parar_no_erro: false` e
  recebe a lista do que ficou de fora, com o número da linha.

- **`1.500` continua ambíguo.** Mil e quinhentos ou um e meio? O motor
  converte `1.500,50` e `1,500.50` — o último separador é o decimal — e deixa
  `1.500` como está, em vez de escolher por conta própria.

- **Com buraco, o salto volta a andar.** Uma única linha excluída — de vez ou
  marcada — derruba a igualdade entre posição e `rownum` na tabela inteira, e
  o `pular` volta aos 131 ms. É correto: a posição realmente mudou. Mas é uma
  degradação em degrau, e não gradual: quem paginava a 6 ms passa a 131 com
  uma exclusão. Um índice de posição resolveria, ao preço de mantê-lo.

- **Por índice o salto continua sendo posição pura.** A ordem da chave não tem
  relação com a ordem de chegada, então não há `rownum` a bissetar ali.

---

## 0.14.0 — 2026-08-28

Paginação por **cursor**, a coluna de sistema **`rownum`**, e a partição
**alfanumérica** — `Clientes_A.reg` até `Clientes_Outros.reg` — com o descritor
`.pag` ao lado.

### Corrigido

- **O servidor nunca ligava `TCP_NODELAY` nas conexões que aceita** — só o
  cliente DbLink ligava. O Nagle segurava cada resposta por até 40 ms
  esperando mais bytes para encher um pacote, e nunca vinham: a resposta tinha
  acabado. Medido na porta de dados com 20.000 linhas: **1 ms de servidor e
  44 ms de relógio**. Depois: **1,3 ms**.

  Trinta e três vezes, numa opção de soquete de uma linha, e valia para **toda**
  operação do protocolo e para todo clique da tela. Achado medindo o relógio
  contra o `ms` que a própria resposta declara — ler o código não acharia, não
  há nada errado escrito.

- **O `varrer` lia a tabela inteira para devolver uma página.** `varrer_com`
  decodifica cada linha **com os anexos** do `.bin` e do `.memo`, monta tudo em
  memória, e só então o servidor jogava fora tudo menos as primeiras `max`.

  Medido com o exemplo `custo-da-pagina`, a mesma página de 200 linhas:

  | linhas na tabela | antes | pelo cursor |
  |---:|---:|---:|
  | 100.000 | 181 ms | não mensurável |
  | 400.000 | 749 ms | não mensurável |
  | 800.000 | **3.176 ms** | não mensurável |

  O custo crescia com a **tabela**, e não com a página — pior que o
  `LIMIT`/`OFFSET` de qualquer motor, porque o `OFFSET` ao menos não carrega o
  blob.

- **A grade da tela listava os baldes como tabelas separadas.** O catálogo só
  sabia tirar sufixo **numérico**, então `clientes_A.reg`, `clientes_B.reg` e
  companhia apareciam na árvore como se fossem 37 tabelas. Agora o sufixo de
  letra conta como volume — mas **só quando o `_A` está ao lado**, porque uma
  tabela que por acaso se chame `dados_X` continua sendo ela mesma.

- **Os arquivos externos saíam com sufixo de letra.** O `.log`, o `.bin`, o
  `.memo`, o `.trash` e o `.reason` não se partem por letra: rolam por tamanho.
  Um `clientes_B.log` se leria como «o diário do balde B», e o diário é da
  tabela inteira. Achado olhando o `ls` do diretório depois de criar a tabela
  pela tela.

### Adicionado

- **Paginação por cursor no protocolo e na grade.** `depois` e `antes` levam o
  rowid onde a página parou; a resposta devolve `cursor_inicio`, `cursor_fim`,
  `ha_mais` e `ha_antes`. `pular` continua como modo de compatibilidade, e a
  resposta declara qual dos dois foi usado em `modo`.

  `ha_mais` sai de **uma** leitura além do teto, e não de contar a tabela:
  contar para mostrar «página 3 de 40» é o item mais caro da tela numa tabela
  grande, e é o que ninguém lê.

  Dentro do navegador, 20 páginas encadeadas numa tabela de 20.000 linhas:
  **4,0 ms de média, 4,9 ms a pior**, sem crescer com a profundidade. Por
  posição no mesmo ponto: **16,1 ms**.

- **Coluna de sistema `rownum`** — o número de ordem de chegada da linha, em
  toda tabela. O motor preenche; não se escreve à mão e não se ajusta. **Nunca
  reaproveita número**: se reaproveitasse, uma linha nova apareceria *atrás* de
  um cursor parado e a paginação passaria a pular registro sem avisar. Alterar
  não renumera.

- **`rowid_do_rownum`: a bissecção.** O `rownum` cresce com o `rowid`, porque o
  `.reg` guarda as linhas na ordem de chegada — então achar a linha de número
  500.000 num milhão custa **vinte leituras**, sem índice nenhum a manter.

- **Partição alfanumérica.** 37 volumes fixos — `A`..`Z`, `0`..`9`, `Outros` —
  e a linha vai para o arquivo da letra dela. O rowid é atribuído como
  `(balde − 1) × registros_por_arquivo + slot`, que é a **inversa exata** da
  conta que `localizar` já fazia: nenhum caminho de leitura mudou, o `.ndx` não
  mudou, o espelho não mudou.

  Acento cai na letra sem acento; vazio e o que não for letra nem algarismo vão
  para `Outros`; o balde que nunca recebeu linha não ganha arquivo.

- **`.pag`, o descritor de partição**, em JSON indentado ao lado da tabela.
  Diz o modo, a coluna de referência, a conta do endereço por extenso, e o que
  cada balde tem. **Gerado, nunca lido pelo motor** — a verdade continua no
  bloco de esquema e nos cabeçalhos dos volumes. Apagar não quebra a tabela.

### Mudado

- **Esquema `PSCH` v4 → v5** (a coluna `rownum`) e **`.reg` v2 → v3** (o
  contador do `rownum` nos bytes 92..100, e os slots do balde em 100..108).

- **`total` saiu da resposta do `varrer`.** Produzi-lo exigia exatamente a
  varredura que esta versão removeu. No lugar entrou `registros`, que sai do
  cabeçalho e não custa nada. Cliente que lia `total` precisa trocar.

- **Junção e união não devolvem `rownum`**, pela mesma razão de não devolverem
  `softdeleted`: dois números de ordem, de tabelas diferentes, não paginam
  coisa nenhuma.

### Sabido

- **Alterar a coluna de referência de uma tabela alfanumérica é recusado.**
  Mudaria o arquivo em que a linha mora, e com ele o rowid — que é a identidade
  dela em todo índice. O caminho é excluir e inserir de novo, e a mensagem diz
  isso.

- **O teto passa a ser por letra.** Num cadastro brasileiro o `_S` enche muito
  antes do `_K`, e quem enche primeiro derruba a inserção daquela letra com as
  outras 36 ainda com espaço. É a conta a fazer ao dimensionar.

- **O cursor é o rowid, e por índice ele não vale.** O índice devolve rowid na
  ordem da *chave*, e «continuar depois do rowid X» não quer dizer nada ali —
  o próximo da chave pode ter rowid menor. Por índice a paginação é por
  posição, e a resposta declara isso.

- **Não há salto para «a página 500».** O cursor sabe ir e voltar uma página;
  ir direto para a milésima exigiria contar, que é justamente o que foi
  removido. Quem precisa de um ponto específico usa `rownum` com a bissecção.
  *(Resolvido na 0.15.0: o `pular` passou a bissetar, e a contagem voltou a
  partir do cabeçalho.)*

- **Uma tabela chamada `dados_X` e o balde X de uma tabela `dados` se escrevem
  igual.** A presença do `_A` separa os dois casos, mas criar as duas no mesmo
  diretório continua sendo uma colisão de nome que o motor não recusa.

---

## 0.13.0 — 2026-08-28

**Excluir deixou de ser uma coisa só.** Toda tabela ganhou a coluna de sistema
`softdeleted`, e dois arquivos novos entraram: o `.trash`, com a linha inteira
antes de ela sumir, e o `.reason`, com o porquê de cada exclusão. Os dois são
de quem administra.

### Corrigido

- **A grade de dados estava com os valores desalinhados do cabeçalho.** Cada
  célula era montada como `<td>${celulaValor(...)}</td>`, e `celulaValor` já
  devolve o `<td>` inteiro — o navegador fecha o primeiro e abre outro, então
  **cada valor ganhava uma célula vazia na frente**. A linha saía com o dobro
  de células do cabeçalho, e todo dado aparecia uma coluna à direita do nome
  dele. Achado abrindo a página no Chromium e contando as células do DOM, não
  lendo o código: o defeito estava em duas telas, a principal inclusive.

- **Um `atualizar` de rotina ressuscitava linha marcada como excluída.** O
  servidor monta a linha inteira a partir do JSON, e a coluna de sistema
  ausente virava `false` — sem erro, sem aviso, e a linha reaparecia na lista.
  Agora, quando o pedido não fala da coluna, ela **mantém o que a linha já
  tinha**. Achado escrevendo o teste, antes de existir na tela.

- **A lixeira dizia «0 anexos» para linha que tinha anexo.** A listagem não
  carrega os anexos de propósito — um memo de megabytes vezes trezentas linhas
  vira uma resposta que ninguém usa —, e o contador saía do vetor vazio em vez
  do cabeçalho do registro. Quem investigasse concluiria que a foto nunca
  existiu, que é o oposto do que o `.trash` serve para provar. Agora o contador
  vem do cabeçalho, o campo externo aparece como «anexo · não carregado» em vez
  de `NULL`, e há um botão que traz aquela linha inteira.

### Adicionado

- **Exclusão suave, e ela é o padrão.** `excluir` marca a linha: ela some das
  listas e continua inteira no `.reg`, com os anexos, e `restaurar` desfaz. A
  física acontece com `"fisico": true`.

  O padrão é o reversível porque **o irreversível não pode ser escolhido por
  omissão**: um cliente que manda `excluir` sem dizer mais nada está pedindo
  «tira isto da minha lista», e é isso que ele recebe.

- **O `.trash`: a linha inteira, antes de sumir.** Gravada e **sincronizada
  antes** de o slot do `.reg` ser liberado. Guardar depois de liberar teria uma
  janela em que a linha não existe em lugar nenhum, e uma queda dentro dela não
  tem conserto; guardar antes tem a janela oposta, que se resolve olhando.
  Entre perder e duplicar, o motor duplica. Há teste que fecha a tabela **sem
  sincronizar** e reabre, para provar que a garantia não depende de um
  `sincronizar` posterior.

  Guarda o *payload* byte a byte **mais o conteúdo dos anexos** — e não os
  ponteiros. Os blocos do `.bin` são liberados na exclusão e podem ser
  reaproveitados pela próxima inserção: com ponteiros, a foto voltaria sendo a
  de outra linha. Há teste que exclui, insere vinte linhas por cima e confere
  que a foto que volta ainda é a certa.

- **O `.reason`: quem, quando e por quê.** O `.log` diz que houve uma exclusão
  no rowid tal; o que ele não tem onde dizer — o evento dele tem 36 bytes
  fixos — é o motivo. Guarda a frase, a identidade da linha (a chave primária,
  em texto, porque «rowid 4173» não diz nada seis meses depois), o usuário e um
  UUID v7 do próprio evento. **Sobrevive à linha**: o expurgo da lixeira é
  registrado aqui antes de o dado sair.

- **Motivo obrigatório por tabela**, escolhido na criação. Marcado, o motor
  recusa qualquer exclusão sem frase escrita, antes de qualquer gravação.

- **Os três arquivos do administrador.** `lixeira` e `motivos` exigem
  `administrar`; o `.log` mantém a permissão `diario`, que já existe e que só
  um administrador concede. A razão está no conteúdo: quem só tem `ler` perdeu
  o direito àquela linha no instante em que ela foi excluída, e a lixeira
  devolveria o direito por outra porta.

- **Na tela:** o botão Excluir abre um diálogo com os dois modos e o campo do
  motivo — e não um `confirm()`, que só sabe perguntar sim ou não. A grade
  ganhou o par «ativas / excluídas», com botão de restaurar em cada linha
  marcada. Lixeira e Motivos têm tela própria, no menu Tabelas e no botão novo
  da barra. A coluna de sistema **não** vira campo de formulário: oferecer um
  `select` com «verdadeiro / falso» convidaria a excluir digitando, sem motivo
  registrado.

### Mudado

- **Esquema `PSCH` v3 → v4.** A v4 acrescenta a coluna de sistema e o byte do
  motivo obrigatório. Tabela gravada na v3 **continua abrindo e lendo
  exatamente como está** — ela só não tem exclusão suave, e a mensagem de erro
  diz isso em vez de ler lixo.

  A coluna entra em `Schema::new`, que é o caminho de criar; a leitura do disco
  usa outro caminho, que não acrescenta nada. Se acrescentasse, cada linha de
  uma tabela v3 passaria a ser lida com os *offsets* deslocados — e
  **silenciosamente**, porque o CRC do slot continuaria batendo: os bytes
  seriam os mesmos, só a interpretação mudaria. Há teste que trava isso.

- **A coluna entra no fim da lista**, para que os *offsets* das colunas do
  usuário não mudem de lugar. `inserir` com N−1 valores preenche `false`;
  `atualizar` com N−1 mantém o que a linha tinha.

- **`varrer` ganhou `visao`**: `ativas` (padrão), `excluidas`, `todas`. Sem o
  filtro por padrão, marcar não faria nada.

- **Junção e união não devolvem a coluna de sistema.** Uma junção traria duas —
  `c.softdeleted` e `p.softdeleted` —, e as duas seriam falso em toda linha,
  porque a junção só lê linha ativa.

### Sabido

- **A lixeira não devolve a linha para o `.reg`.** Ela guarda, mostra e deixa
  baixar; restaurar de lá exige reinserir, e a linha volta com **outro rowid** —
  o `.reg` não reaproveita slot, nem por restauração. Quem quer volta pelo mesmo
  rowid usa a exclusão suave, que é para isso.

- **O `.trash` e o `.reason` não são cifrados nem compactados.** Compactar
  arquivo append-only exige rotacionar e reescrever, e cifrar exige uma cifra
  de bloco que o projeto ainda não tem: há SHA-256, HMAC e PBKDF2 escritos aqui,
  mas nenhum AES. Enquanto isso, quem tem acesso ao disco lê os dois — a
  proteção é a permissão do sistema de arquivos, e não o formato.

- **A listagem da lixeira carrega o resultado inteiro na memória**, como a
  exportação. Serve para investigar; não serve para varrer uma lixeira de
  milhões de linhas.

- **Filtrar por visão num caminho de índice custa uma leitura por linha.** O
  índice devolve rowid e a marca está no registro. É o preço de pedir
  ordenado; a varredura direta não paga nada.

---

## 0.12.0 — 2026-08-28

A tabela sai em **sete formatos**, com o XLSX e o DOCX escritos aqui, e o
espelho `.bkp` entra no fluxograma de onde estava faltando.

### Corrigido

- **O cabeçalho da planilha saía com a cor da zebra e sem negrito.** Ele
  apontava para o estilo de índice 1, que é o «texto listrado». O Excel(R) não
  reclama de índice errado — ele obedece. Os índices do `cellXfs` agora têm
  nome (`estilo::CABECALHO`, `estilo::DATA_ZEBRA`, …) e há teste que confere a
  correspondência, porque número solto ali já custou caro uma vez.

- **A tabela do DOCX estava sem o `w:tblGrid`, que é obrigatório.** O Word(R)
  tolera a falta, então o defeito passaria despercebido até alguém abrir o
  arquivo noutro programa; o python-docx recusou o documento inteiro.

- **O `.bkp` não aparecia na seção 7 do dossiê** — justamente a que desenha o
  fluxo de gravação. Quem lia via cinco arquivos sendo escritos e concluía que
  o espelho era cópia feita depois. Não é: ele é escrito **no mesmo instante**
  que o principal, no mesmo offset. A figura ganhou a caixa do espelho e a da
  janela de durabilidade, que também faltava. Achado pelo Adriano lendo o
  dossiê.

### Adicionado

- **Exportar em CSV, TXT, JSON, XML, HTML, XLSX e DOCX.** Botão na barra e
  item no menu. Os dois formatos do Office são ZIP de XML, e o projeto já
  escreve ZIP com DEFLATE desde o backup: o que parecia exigir biblioteca são
  os mesmos tijolos que já estavam aqui. **Nenhuma crate entrou.**

- **A planilha sai formatada**, não crua: cabeçalho pintado, zebra nas linhas,
  painel congelado abaixo do cabeçalho, autofiltro em todas as colunas e
  largura medida das 500 primeiras linhas. O documento sai em paisagem, com o
  cabeçalho repetindo a cada página.

- **Data em planilha sai como número com formato**, e não como texto. Texto
  que parece data não ordena, não filtra por período e não entra em conta. A
  diferença entre a época do Excel(R) e a nossa é de 25.569 dias, e é só isso.

- **O HTML exportado leva filtro embutido** e não busca nada na rede: abre em
  máquina sem internet e continua funcionando.

- **`docs/MULTILINK.md`** — por que o pacote MULTILINK não dá para ligar por
  `.rlib` e qual é o caminho que funciona.

### Mudado

- **`FORMATO.md`, `MANUAL.txt` e `README.md`** passaram a dizer que a tabela é
  de cinco arquivos **mais um sexto opcional**, com a descrição de quando o
  `.bkp` é escrito, quando é lido e o que `reparar` faz nos dois sentidos.

### Sabido

- **O MULTILINK não entra por `.rlib`.** O pacote traz só binários — os fontes
  que o manifesto promete não estão nele —, e o `.rlib` foi compilado pelo
  rustc 1.98 contra o 1.94 daqui: **provado rodando o linkador** (E0514), não
  suposto. O formato do `.rlib` não é estável entre versões do compilador,
  então igualar resolveria hoje e quebraria na próxima atualização de qualquer
  um dos lados. Fora isso, um `.rlib` é dependência externa — a regra que
  sustenta o projeto —, não há fachada C que contorne, e o licenciamento é por
  máquina com prazo: linkar faria o servidor de dados inteiro passar a exigir
  licença válida para subir. O caminho é **falar por protocolo**, como o DbLink
  já faz.

- **A exportação carrega o resultado inteiro na memória** antes de escrever.
  Serve para o que uma pessoa abre no Excel(R); não serve para despejar uma
  tabela de dez milhões de linhas.

- **O DOCX não pagina coluna demais.** Em paisagem cabem umas doze colunas
  legíveis; acima disso a tabela aperta. Para tabela larga, XLSX.

---

## 0.11.0 — 2026-08-28

Os monitores da máquina no painel, o aviso de disco por e-mail, e o
**DbLink** — o banco de fora aparecendo na mesma grade que os daqui.

### Corrigido

- **O percentual de disco dividia pelo tamanho errado.** A conta era
  `usado / total`, e o certo é `usado / (usado + livre)`, como a do `df`.
  Reserva de sistema de arquivos e cota não estão à disposição de ninguém, e
  contá-las como livres faz um disco cheio parecer vazio. Na máquina onde isto
  foi medido o `df` dizia **55% usado** e a conta antiga dava **8%** — com 8%,
  um alerta de «menos de 10% livre» nunca dispararia e o disco encheria calado.
  Achado rodando o servidor, não lendo o código.

- **O e-mail do alerta não atravessava relé de sete bits.** O assunto levava o
  «ç» de «espaço» cru no cabeçalho, e cabeçalho de e-mail é ASCII por
  definição (RFC 5322); o corpo ia em UTF-8 cru declarado como 7 bits, e um
  relé sem `8BITMIME` tem licença para cortar o oitavo bit. Agora o assunto sai
  em palavra codificada da RFC 2047 e o corpo em base64. Conferido decodificando
  o que um relé de verdade recebeu, com um leitor independente.

- **`.botao.perigo` pintava vermelho sobre laranja.** A regra trocava a borda e
  a cor do texto mas não apagava o fundo do `.botao`, e o botão de excluir
  ficava ilegível — na tela de usuários, que já era assim, e na nova de DbLink.

### Adicionado

- **Monitores da máquina no painel:** CPU, memória, placas de rede, discos
  físicos e espaço livre de cada caminho que o servidor usa. Tudo do `/proc`,
  que o núcleo publica em texto; o espaço livre do `df`, porque exige
  `statvfs`, que não está na `std`. Nenhuma crate entrou. Os monitores renovam
  sozinhos a cada quatro segundos, e a primeira leitura **se declara primeira**:
  `/proc` traz contador desde o arranque, e taxa precisa de dois instantes.

- **Aviso de disco apertado, por e-mail.** Dois limites no OU — percentual e
  piso em MB —, porque cada um sozinho erra de um lado: 10% de 8 TB não são
  aperto, e 1 GB livre num disco de 20 GB são. O cliente SMTP é escrito aqui,
  com a `std`.

- **DbLink.** Botão na barra, definições no menu Configurações, e o protocolo
  do MySQL(R) escrito à mão. As tabelas do banco de fora na lista, o conteúdo
  na **mesma grade** das tabelas daqui — agrupar, buscar, totalizar e paginar
  valem igual. Testado contra um MySQL(R) 8.0.46 de verdade.

- **SHA-1**, conferido contra os vetores do FIPS 180-4. Entrou por causa do
  `mysql_native_password` e só por isso: não é usado em lugar nenhum do formato
  do PhxSql — senha continua em PBKDF2-HMAC-SHA256, integridade em CRC-32 e
  SHA-256. Quem define o protocolo é o outro lado.

- **`alertas` e `dblink` no `config.json`**, e o caminho do `base` **já
  resolvido** na tela de configuração: caminho relativo vale a partir de onde o
  servidor foi iniciado, e subir por outro caminho passa a ver outro banco.

- **As sete junções do diagrama**, mais `UNION` e `UNION ALL`. Na tela se
  escolhe **clicando no desenho de Venn**, com o SQL equivalente escrito
  embaixo de cada um. Chave composta, teto que se declara, e as três armadilhas
  do SQL respeitadas: nulo não casa com nulo, família errada é recusada na
  entrada em vez de devolver zero linhas parecendo resposta, e decimal casa por
  valor e não por escala.

- **`criar_tabela` com nome qualificado.** *(corrigido)* `filial.clientes`
  gravava cinco arquivos chamados `filial.clientes.reg` na **raiz** do banco.
  Toda leitura separa o ponto em schema e tabela desde sempre; só a criação não
  separava. A tabela nascia inalcançável e o servidor respondia «criada».

- **Erro com código estável.** A resposta traz `codigo`, `nome`, `classe` e
  `repetir` além do texto. Sem código, integrar exige comparar **texto** — e
  melhorar a redação de uma mensagem quebraria o cliente sem ninguém perceber.
  Número publicado não muda, e há teste que falha se mudar.

- **`sessoes` e `encerrar_sessao`** — quem está falando com o servidor agora, o
  que cada um executa e há quanto tempo, e como derrubar. Porta de dados e
  sessões do navegador na mesma lista.

- **`estatisticas`** — percentis, histograma de faixas que dobram, as mais
  demoradas, e uso por tabela, operação, usuário e código de erro. A média some
  de propósito: mil respostas de 1 ms e uma de 30 s dão média de 30 ms.

- **`checksum` de tabela** e **tempo no ar** no `ping`.

### Sabido

- **Não há TLS em lugar nenhum** — nem no SMTP nem no DbLink. A `std` não traz
  TLS e o projeto não aceita crate. O e-mail serve para relé interno na porta
  25; o DbLink, para rede interna ou túnel. A senha não viaja em texto nos dois
  casos, mas o **dado devolvido pelo DbLink viaja**.

- **Do `caching_sha2_password` só o caminho rápido**, que vale quando o
  servidor já tem a senha em cache. O completo exige TLS ou a chave RSA. Quando
  o servidor pede o completo, o erro diz isso e as duas saídas.

- **Não há compactação (`OPTIMIZE TABLE`).** O `.reg` nunca reaproveita slot
  excluído, e compactar significaria reescrever `rowid` — que é endereço. Uma
  tabela com muitas exclusões cresce e não encolhe: é consequência aceita da
  ordem de digitação ser garantida, não esquecimento. Detalhes em
  `docs/COMPARACAO.md`.

- **O código de erro é por variante, não por situação.** `ESQUEMA_INVALIDO`
  cobre desde config errado até chave de junção incompatível.

- **Junção é de duas tabelas por vez, e só por igualdade.** `ON a.x > b.y` não
  existe: o *hash join* casa por igualdade. `WHERE` sobre o resultado da junção
  também não — a tela filtra depois, na grade.

- **PostgreSQL(R) ainda não conecta.** A definição já pode ser guardada; o
  cliente não existe.

- **O monitor de CPU, memória e rede só existe no Linux.** Fora dele a tela diz
  que não sabe medir, em vez de mostrar zero. O espaço em disco continua
  valendo, porque vem do `df`.

---

## 0.10.0 — 2026-08-28

Uma correção de **perda silenciosa de dado** sob gravação concorrente, a
gravação **20× mais rápida** com durabilidade configurável, e a seção
`recursos` no `config.json`.

### Corrigido

- **Duas gravações simultâneas na mesma tabela sobrescreviam uma a outra.**
  Abrir uma tabela lê o cabeçalho, e o cabeçalho traz `slot_count` — o contador
  que decide onde a próxima linha vai. O servidor tomava a trava para abrir,
  **soltava**, e só então tomava de novo para gravar. Nessa fresta duas
  operações abriam a tabela, as duas guardavam `slot_count = N`, e as duas
  gravavam no rowid N+1: a segunda por cima da primeira, sem erro nenhum.

  Aparecia como «chave duplicada» quando havia índice único sobre a coluna
  — o índice pegava. **Sem índice único, a linha simplesmente sumia.**

  A trava passa a cobrir abrir *e* gravar, como um bloco só. Um teste em
  `tests/tabela.rs` deixa o contrato escrito: duas aberturas disputam o mesmo
  rowid, e por isso quem abre precisa serializar.

### Adicionado

- **Seção `recursos` no `config.json`**: durabilidade, tamanho do lote, cache
  de páginas, teto de memória, threads, percentual de CPU, conexões e usuários
  simultâneos. `conexoes_max` no topo continua valendo, para config antigo não
  parar de subir.

- **Durabilidade configurável**, e é o que acelera a gravação. Medido com
  20.000 linhas na mesma tabela:

  | quando sincroniza | linhas/s | ganho |
  |---|---:|---:|
  | a cada linha (o que o servidor fazia) | 1.289 | — |
  | a cada 100 | 18.264 | 14,2× |
  | a cada 1.000 | 24.858 | 19,3× |
  | só no fim | 26.301 | 20,4× |

  **95% do tempo de uma inserção era `fsync`.** Depois de tirá-lo, a inserção
  custa 37,5 µs, dos quais 65% são os dois índices — que é o gargalo seguinte,
  não este.

  Os bytes vão para o sistema operacional em toda gravação, sempre: um `write`
  direto, sem buffer nosso. Outro processo vê o dado na hora, sincronizado ou
  não. O `fsync` protege de uma coisa só: perder energia antes de o sistema
  descarregar a página.

- **Relógio de fundo** que fecha a janela de durabilidade quando ninguém grava.
  Sem ele, a última venda do dia às 18h ficaria sem `fsync` a noite inteira.

- **`sequencias`** e **`ajustar_sequencia`**: o contador de cada tabela do banco
  num lugar só, e o caminho do administrador para zerar ou pular uma faixa. O
  número continua morando no cabeçalho do `.reg` de cada tabela — a operação
  junta para mostrar, não cria uma segunda cópia.

- **`custo-do-sync`**, o medidor que produziu a tabela acima.

### Sabido

- `por_lote` é o padrão. Quem precisa de durabilidade por operação — um
  livro-razão, por exemplo — põe `"durabilidade": "por_operacao"` e paga os 20×.
- O `cpu_percentual` não é cota do sistema operacional: é quantos núcleos o
  trabalho dividido usa.
- `cache_paginas` e `memoria_max_mb` são lidos e mostrados, mas ainda **não são
  impostos**: o buffer pool do `.ndx` é o trabalho seguinte, e é ele quem vai
  usá-los.

---

## 0.9.0 — 2026-08-28

Duas peças de análise: o agrupamento da grade chega ao nível do Janus GridEX(R)
e do DevExpress(R), e a **tabela dinâmica** ganha assistente e um motor de
tabulação cruzada no servidor.

### Adicionado — tabela dinâmica

- **Operação `pivotar`**, que cruza uma tabela por dois eixos e resume as
  células. A agregação acontece **no servidor**, e é o ponto: um pivot resume —
  cem mil linhas viram uma grade de vinte por doze —, e trazer as cem mil para
  o navegador somar seria pagar o transporte do que vai ser jogado fora.

- **Junção por tabela de consulta.** Cruzar «vendas pela cidade do cliente»
  exige a cidade, que mora na outra tabela. A forma ingênua — uma busca no
  índice por linha de venda — custaria uma descida na árvore por linha. Aqui a
  tabela de consulta é lida **uma vez** para um mapa em memória e o cruzamento
  vira acesso direto: é o *hash join*, e para a forma de dado que um pivot cruza
  (muitos fatos, poucas dimensões) ele é a escolha certa. Teto de 500.000 linhas
  por tabela de consulta, dito no erro quando estoura.

- **Seis resumos**: soma, média, contagem, mínimo, máximo e valores distintos.
  Contagem é o único que dispensa campo de valor.

- **Granularidade de data**: cada valor, por dia, mês, trimestre ou ano. Cruzar
  venda por dia daria uma coluna por dia do ano; o que se quer é por mês ou
  trimestre, e isso é escolha de quem monta, não propriedade do dado. Os rótulos
  saem em ordem lexicográfica crescente (`2026-01`, `2026-T1`), então ordenar
  texto já ordena tempo.

- **Assistente de três passos** na interface (botão *Pivot*, `Alt+7`): quais
  tabelas entram — com as junções propostas a partir das chaves estrangeiras
  declaradas —, que campo vai em cada eixo (arrastando), e o resultado com total
  por linha, por coluna e geral. Mais «copiar como CSV» e «ver o pedido», que
  mostra o JSON equivalente pela porta 5000.

### Adicionado — agrupamento da grade

- **Ordem por nível**: a seta na pastilha inverte crescente/decrescente daquele
  nível. Agrupar por mês quase sempre quer o mais recente em cima. A direção é
  guardada por *campo* e não por posição, então arrastar a pastilha para outro
  lugar não vira a ordem de quem ficou no lugar dela.
- **Rodapé por grupo**, com o total alinhado **na coluna** e não numa tira de
  texto — é assim que se compara um total com os valores acima. Num grupo de
  trinta linhas o cabeçalho já rolou para fora da tela quando o total interessa.
- **Total geral** da grade, sobre o conjunto filtrado inteiro: ele não muda ao
  virar de página, porque um rodapé que muda ao virar de página não é total de
  nada.
- **Expandir tudo / recolher tudo**, e um botão que liga e desliga o rodapé por
  grupo.

### Corrigido

- **`Sequence` aparecia como campo de texto** na paleta do pivot. É um contador.

### Sabido

- O pivot lê até 5.000.000 de linhas por cruzamento. Acima disso o número
  devolvido seria de uma amostra, e amostra sem aviso é pior que recusa.
- A junção é por igualdade de uma coluna com a chave primária da tabela de
  consulta (ou a coluna nomeada em `chave`). Não há junção por faixa nem
  composta.
- Célula vazia quer dizer «nenhuma linha caiu ali», não zero — e os dois são
  informações diferentes.

---

## 0.8.0 — 2026-08-28

**Duas mudanças de formato**, e as duas entram agora porque não há dado em
produção: o campo ganhou identidade e metadados, e o volume aprendeu a cortar
pelo calendário. Junto vem a gestão do banco inteiro — catálogo, configurações,
diretivas e copiar/colar.

### Corrigido

- **Um `onclick` no `#painel` vazava para a tela seguinte.** A gestão do banco
  pendurou o clique no próprio painel, e o `folha()` troca o *conteúdo* do
  painel, não o *elemento* — o tratador sobrevivia à troca de tela e disparava
  na próxima. Clicar em «Configurações e diretivas» abria SysColumns. Corrigido
  em dois lugares: o tratador foi para o container das operações, e o `folha()`
  passou a limpar o `onclick` do painel por garantia.

- **O botão primário ocupava a linha inteira** numa barra de ações. O `.botao`
  nasceu com `width:100%` para o cartão de entrada, onde é o único da linha.

- **A tela de partições calculava por divisão**, que é a conta certa para a
  partição por faixa e errada para a por período: quatro meses apareciam como um
  volume só. Agora lê as fronteiras que o `esquema` devolve.

### Adicionado — formato

- **Esquema `PSCH` versão 3.** Cada coluna passa a carregar `id`, `caption`,
  `descricao` e `mascara`, e cada índice um bit de **primário**. A leitura ainda
  aceita a versão 2: tabela gravada antes abre, ganha um `id` v7 sorteado na
  hora e os textos vazios.

  O `id` é um UUID v7 **nunca reaproveitado**, e existe para que renomear a
  coluna não quebre nada: uma tela ou um relatório apontam para ele, e renomear
  troca só o `nome`. Os metadados moram no `.reg`, com o resto do esquema, pela
  mesma razão que o esquema mora ali — um dicionário externo se perde, se
  desatualiza, e obriga quem copia os cinco arquivos a copiar um sexto.

- **Chave primária de verdade.** Até aqui só havia «índice único», e chave
  primária é mais: é a identidade da linha. Só um índice pode ser primário, ele
  é sempre único, e nenhuma coluna dele aceita nulo — uma identidade nula não
  identifica. As três conferências acontecem no `Schema::new`.

  O papel de uma coluna — primária, estrangeira, composta — **não é gravado na
  coluna**: sai dos índices e das chaves estrangeiras, que são a verdade. Marcar
  no próprio campo criaria uma segunda verdade que divergiria no primeiro
  `ALTER`.

- **Partição por período: mensal, bimestral, semestral e anual.** O volume corta
  quando o período de uma coluna de data vira — ou quando enche, o que vier
  primeiro, porque `registros_por_arquivo` continua sendo teto.

  O endereço não pode sair de divisão quando o corte depende do calendário: dois
  meses rendem quantidades diferentes. Então **cada volume grava no próprio
  cabeçalho** o rowid em que começou e o período em que abriu, e a tabela de
  fronteiras se remonta lendo esses cabeçalhos na abertura. Achar o volume de um
  rowid vira uma busca binária num vetor de dezenas de posições, em vez de uma
  divisão. Sem arquivo extra e sem bloco que cresce.

  **A linha atrasada não volta**: um lançamento de janeiro digitado em março
  entra no volume de março. Voltar significaria escrever no meio de um arquivo
  já fechado, quebrando de uma vez a ordem de digitação e o endereço contíguo.
  Por isso o período de um volume é *o período em que ele abriu*.

- `Paginacao::com_max_arquivos` e `com_modo`; `Periodo` com `chave`,
  `primeiro_mes` e `rotulo`.

### Adicionado — protocolo

- **`copiar_tabela`**, que atravessa databases e schemas. A permissão de criar é
  conferida **no destino**, à parte: sem isso, quem pode ler um banco e não pode
  criar no outro conseguiria escrever onde não devia.
- **`sistabelas`** e **`siscolunas`** (também `systables` e `syscolumns`): o
  catálogo em forma de dado.
- `criar_tabela` aceita `caption`, `descricao`, `mascara` e `id` por coluna,
  `primario` por índice, e `particao` + `particao_coluna`.
- `esquema` devolve os metadados, o papel de cada coluna nas chaves, o modo de
  partição e a **tabela de fronteiras dos volumes**.

### Adicionado — interface

- **Gerir banco** (`Alt+6`), com 15 itens: tabelas, SysTables, SysColumns,
  copiar tabela, configurações, diretivas, editor de menu, conexões, arquivos
  bloqueados, transações, backup/restauração — e, apagados dizendo o que falta,
  triggers, procedures, jobs e modo exclusivo.
- **Configurações gerais do servidor, do banco e dos usuários**, cada uma com
  sua tela, mais **diretivas de acesso ao banco** com os seis portões na ordem
  em que fecham.
- **Copiar e colar tabela** entre bancos, com área de transferência.
- **Cadastro de campos** com id, nome, caption, tipo, tamanho, máscara,
  obrigatoriedade e descrição, e a chave primária escolhida por rádio.
- **Tabela particionada** com grade que mostra como o volume vai cortar, antes
  de gravar — porque depois não muda.
- **Configurações e diretivas da tabela**: a geometria decidida na criação, os
  índices e chaves, os volumes no disco, e o que a tabela herda do servidor.
- **Editor de menu**: troca o nome exibido de qualquer item. Fica no navegador
  de quem mexeu, não no servidor — é preferência de quem opera, não política do
  banco.

### Sabido

- **As telas de configuração leem, não gravam.** Gravar o `config.json` pela
  porta web significaria que uma sessão roubada abre o firewall, esvazia a lista
  de comandos proibidos e cria um supervisor. Criar e alterar usuário pela web
  tem o mesmo problema, com credencial no meio. As telas dizem qual campo mexer.
- **Triggers, procedures, jobs e modo exclusivo continuam não existindo.** As
  telas mostram o que falta e de que dependem; elas não os implementam.
- **Restaurar backup ainda não existe.** Copiar de volta é decidir o que fazer
  com o que está lá, e isso precisa de desenho.
- Mudar a partição de uma tabela existente continua sendo criar outra e copiar
  as linhas — o que refaz os rowids, que é exatamente o motivo de não ser
  automático.

---

## 0.7.0 — 2026-08-28

A tela ganha **gestão de tabelas**. Criar, duplicar, reparar e excluir tabela
passam a existir no protocolo — três operações que a interface pedia e o
servidor não tinha.

### Corrigido

- **Um servidor `somente_leitura` teria deixado apagar tabela.** As três
  operações novas entraram no despacho e ficaram fora de `OPS_ESCRITA`, a lista
  que o modo somente-leitura consulta. `criar_tabela` e `excluir_tabela`
  passariam num servidor marcado como só de leitura. Como a lista é escrita à
  mão, o conserto veio com um teste que a percorre.

- **`criar_schema` estava prometido em dois lugares e não existia.** Aparecia na
  tabela de permissões do `docs/USUARIOS.md` e em `OPS_ESCRITA`; pedir pela rede
  respondia «operacao desconhecida». A biblioteca já sabia criar a pasta —
  faltava a operação. Agora existe, e a tela de nova tabela tem o campo.

- **A largura do sufixo entrava depois do teto de volumes.** `Paginacao::nova`
  confere o teto contra os três dígitos do padrão, então pedir 9.999 volumes era
  recusado *antes* de o quarto dígito existir. Entrou `Paginacao::com_max_arquivos`,
  e a ordem passou a ser largura primeiro, teto depois.

- **«Sem teto» não existe, e o padrão fingia que sim.** O sufixo tem largura
  fixa: com três dígitos o volume 1000 não teria nome de arquivo. Teto omitido
  agora vira o maior que cabe no sufixo — 999 com três dígitos —, em vez de zero,
  que o validador recusava com uma mensagem que não ajudava quem preencheu a tela.

- **A árvore roubava a tela de quem pintasse depois dela.** `montarArvore`
  terminava sempre clicando no Painel; criar uma tabela redesenhava a árvore,
  voltava para a grade — e meio segundo depois o painel chegava por cima. Quem
  vai pintar a própria tela passa `montarArvore(false)`.

### Adicionado

- **Operação `criar_tabela`**, com colunas, índices, schema e paginação. O tipo
  da coluna aceita as três formas que aparecem na prática — `Int8`,
  `Decimal(15,2)` e a forma que o próprio `esquema` devolve —, e a razão é uma
  só: o que a leitura do esquema **devolve** tem de voltar como entrada, senão
  duplicar uma tabela exigiria traduzir cada tipo à mão. As colunas do índice
  vão por **nome**, não por posição: posição muda quando alguém reordena.

- **Operação `duplicar_tabela`**, que copia os cinco arquivos byte a byte. A
  cópia nasce com os **mesmos rowids e a mesma ordem de digitação** — o que uma
  reinserção linha a linha não daria.

- **Operação `excluir_tabela`**, que apaga os cinco arquivos e o espelho
  `.bkp`, todos os volumes de cada um. Exige a permissão `administrar`, não
  `excluir` — poder perder uma linha não é poder perder a tabela — e o nome da
  tabela repetido no campo `confirmar`. A conferência de qual arquivo pertence
  a qual tabela exige o sufixo todo em algarismos: sem isso, excluir `precos`
  levaria `precos_historico` junto.

- **Operação `criar_schema`**, a pasta dentro do database.

- **Botão e menu «Tabelas»**, com as oito operações sobre a tabela escolhida:
  estrutura, editar conteúdo, partições, duplicar, reparar tabela, reparar
  índice, nova tabela e excluir. `Alt+5` abre a grade.

- **Tela de partições**, que mostra em que volume cada faixa de rowid cai e com
  que nome de arquivo. As faixas são **conta, não busca** —
  `volume = (rowid−1) ÷ por_arquivo + 1` —, e a tela diz por que não dá para
  editá-las depois: mudar o divisor mudaria o endereço de cada registro já
  gravado.

- **Tela de nova tabela**, com colunas e índices montados linha a linha, os 21
  tipos com o que cada um custa em bytes, e schema opcional.

- **Gestão de transações no menu Ferramentas.** A tela mostra a **ausência**:
  não há `BEGIN`, `COMMIT` nem `ROLLBACK`, então ela não traz lista de
  transações abertas — uma lista vazia daria a entender que o mecanismo existe e
  está parado. Lista o que de fato existe e o que falta, na ordem.

- `digitos` e `bytes_por_arquivo` na resposta de `esquema`, sem os quais não dá
  para escrever o nome do volume: `_1` e `_001` são arquivos diferentes.

### Mudado

- O menu **Tabela** virou **Tabelas** e absorveu a gestão. Dois menus vizinhos
  com nomes quase iguais obrigariam a adivinhar em qual está cada operação.

- Novo menu **Ferramentas**, espelho da barra pelo teclado.

- A ferramenta *Transações* deixa de ser um botão apagado.

### Sabido

- **Continua sem transações.** A tela nova diz isso; ela não as implementa.
- A **CLI ainda não cria tabela** — só o protocolo e a interface.
- `buscar` e `desbloquear` continuam sem tela.

---

## 0.6.0 — 2026-08-28

A interface deixa de só navegar. **30 das 32 operações** têm tela agora — eram
14 há três versões.

### Adicionado

- **View Database, no padrão Browse → Form do Clarion(R)**, de onde este
  projeto vem. Ferramenta *View DB*, menu *Arquivo → View Database*, `Alt+4`,
  ou um clique no nome do database na árvore.

  A grade lista as tabelas com registros, slots, colunas e índices; um clique
  abre o conteúdo; um clique numa linha abre a **ficha**, com um campo por
  coluna e do tipo certo — caixa de texto para `Memo`, sim/não para `Bool`, e a
  dica do formato no lugar. **Salvar grava, Excluir apaga, Nova linha inclui.**

  Fecha as quatro operações que existiam no servidor e não tinham porta:
  `ler`, `inserir`, `atualizar` e `excluir`. Sobram `buscar` e `desbloquear`.

  Detalhes que a ficha respeita: campo em branco é **nulo**; coluna obrigatória
  tem asterisco; `Sequence` em branco faz o motor numerar; `Uuid` aceita a
  palavra `"novo"`. E o aviso da exclusão diz que **o slot não é
  reaproveitado** — é assim que a ordem de digitação se mantém.

- **`[+]` na árvore**, ao lado de *Bancos de dados*, para criar um database.

- **About no menu Ajuda**, abrindo a **tela de créditos** com a fênix do
  projeto Phoenix, quem fez o quê, e a lista honesta do que o motor se apoia:
  RFC 9562, FIPS 180-4, RFC 4231, RFC 8032, RFC 1951 e os demais — cada um
  escrito aqui e conferido contra o vetor oficial.

### Corrigido

- **A fênix vinha com um retângulo azulado no tema claro.** Os pixels de fora
  do símbolo têm alfa 1 a 3 em azul no arquivo de origem — quase-transparente
  não é transparente. 22.789 pixels zerados antes de embutir. É o mesmo defeito
  que a capa do dossiê já teve, e a mesma lição: alfa quase-zero se enxerga.

---

## 0.5.5 — 2026-08-28

### Adicionado

- **Barra de ferramentas** com as quinze pedidas, cada uma com ícone em SVG
  desenhado aqui e cor da paleta da marca. **Dez funcionam de verdade**; cinco
  aparecem apagadas, com um ponto âmbar, e clicar nelas diz o que falta e do
  que depende.

  Botão que parece funcionar e não funciona custa mais caro do que botão que
  falta: o primeiro só se descobre no meio do trabalho. Sumir com eles da lista
  seria esconder o roteiro; ligá-los a um aviso genérico seria fingir.

  | Ferramenta | Estado |
  |---|---|
  | Start/Stop | mostra o serviço; parar e subir pela tela ainda não |
  | **Query** | **novo** — `SelectMemory` com coluna, operador, valor e teto. Não é SQL, e a tela diz isso |
  | Usuários, Bancos, Repair, Backup, Ajuda | já existiam, agora com atalho |
  | **Diretivas** | **novo** — comandos proibidos, IPs permitidos, somente leitura, firewall, espelho |
  | **Conexões** | **novo** — conexões agora, sessões web, acessos e de onde vêm |
  | **Replicação** | **novo** — papel e portas, dizendo que são configuração e não serviço |
  | Duplicar, Transações, Importar, Server Mail, Blockchain | apagadas — não existem |

  A cor agrupa por família, não por gosto: quinze ferramentas para oito
  matizes, então a repetição é inevitável e precisa significar alguma coisa.

- A tela de consulta fecha a sexta das sete operações que não tinham porta:
  `selecionar_memoria`. **A interface passa de 25 para 26 das 32.** Continuam
  sem tela `inserir`, `atualizar`, `excluir`, `ler`, `buscar` e `desbloquear`
  — ou seja, a edição de dados.

### Corrigido

- **Duas cores da barra não existiam.** `--pend` e `--acento-2` são tokens do
  dossiê, não da interface: Repair e Blockchain saíam com a cor do texto. O
  teste de navegador leu a cor computada e mostrou. De quebra, `--vermelhao` e
  `--laranja` são a **mesma cor** no tema claro, por decisão da marca —
  escolher entre os dois seria escolher nada.

- **`folha()` apagava qual tabela estava aberta.** Carregar a tabela na RAM
  mostrava uma folha, a folha zerava `est.atual`, e a ferramenta de consulta
  abria com o database vazio — o erro saía com uma barra solta, `/naoexiste`.
  Quem escolhe uma tabela na árvore continua com ela escolhida; o que muda é o
  que está na tela.

---

## 0.5.4 — 2026-08-28

### Corrigido

- **O Centro de Controle estava marcado «pronto» e não edita dados.** Contando
  as operações que a tela realmente alcança: **25 de 32**. Faltam `inserir`,
  `atualizar`, `excluir`, `ler`, `buscar`, `selecionar_memoria` e
  `desbloquear` — ou seja, a interface **navega os dados mas não os altera**.

  Virou «parcial» no README e no dossiê. É o mesmo erro da chave estrangeira:
  marcar como pronto o que existe pela metade.

### Adicionado

- `docs/PENDENCIAS.md` refeito numa **tabela única** com os 64 pedidos na ordem
  em que foram feitos, com ☑️ feito, ◐ parcial e ☐ planejado. O saldo:
  **54 feitos, 4 parciais, 6 planejados**.

---

## 0.5.3 — 2026-08-28

### Adicionado

- **Barra de menu tradicional** no Centro de Controle: *Arquivo · Tabela ·
  Memória · Administração · Ver · Ajuda*, com ícone, atalho à direita e
  separadores, como manda o gênero.

  O motivo de existir está na conta: a interface usava **14 das 31 operações**
  do servidor. Backup, conferência de backup, reparo pelo espelho, reindex, a
  tabela em memória inteira e a configuração **não tinham porta de entrada
  nenhuma na tela** — existiam só para quem falasse o protocolo na mão.

  Teclado: a letra sublinhada abre o menu com **Alt**, as setas andam entre
  itens e entre menus, **Esc** fecha. Mais `F5` para atualizar, `Ctrl+B` para
  o backup e `Alt+1/2/3` para Painel, Estrutura e Conteúdo.

  Os itens que precisam de uma tabela ficam cinzas enquanto não houver uma, e
  o estado é recalculado na hora de abrir o menu — não na carga da página.
  As ações que mexem (reindexar, reparar) pedem confirmação.

- **Recado na barra** (`avisar`), que aparece e some sozinho. Um `alert()`
  interromperia quem está trabalhando para dizer "backup pronto"; erro fica
  mais tempo na tela, porque erro se lê.

### Corrigido

- **O menu fechava no mesmo clique que o abria** — defeito meu, achado no
  teste de navegador e invisível na leitura do código. `abrirMenu` refazia o
  `innerHTML` da barra para atualizar o cinza dos itens; isso destruía o
  elemento clicado, o `ev.target` virava um nó solto, e o
  `closest("#menubar")` do fechar-ao-clicar-fora devolvia `null`. Agora só o
  `disabled` dos botões existentes é atualizado.

---

## 0.5.2 — 2026-08-28

### Corrigido

- **Um byte trocado no cabeçalho do slot apagava o registro em silêncio.**
  Achado provando o espelho `.bkp` com um servidor de verdade e o `.reg`
  estragado à mão.

  O byte de status de um slot só pode valer 0 (livre) ou 1 (ativo). A leitura
  testava `slot[0] != ATIVO` e respondia `None` — que é a resposta certa para
  um registro excluído e a **errada** para um registro inteiro. Com o status
  virando lixo (254, no teste), o servidor respondia `{"ok": true,
  "resultado": null}`: nem erro, nem aviso, nem consulta ao espelho, que tinha
  a cópia boa ali do lado.

  O `reparar` errava pelo mesmo motivo, e pior: dava o slot por bom
  (`slot[0] != ATIVO ||` curto-circuitava o CRC), reportava
  `reparados: 1, integro: true` e deixava o registro perdido. Só o `verificar`
  percebia, e sem poder consertar: *"cabeçalho diz 11 registros, varredura
  achou 10"*.

  Agora status inválido é **corrupção**, não estado: cai na mesma segunda
  chance da falha de CRC, e o erro diz qual dos dois aconteceu. Depois do
  reparo o `.reg` volta a bastar sozinho.

  Dois testes de regressão, e o segundo é o contraponto: **excluir continua
  devolvendo `None` sem erro e sem acionar o espelho** — se o conserto tivesse
  passado do ponto, toda exclusão viraria corrupção.

### Sabido

- A segunda chance cobre payload corrompido e status inválido. **Não cobre o
  caso em que o bit trocado deixa o status exatamente em 0**: aí o slot fica
  indistinguível de uma exclusão legítima. Resolver isso exige usar o `.log`
  como desempate — ele registra toda exclusão com data e hora —, e é trabalho
  de outra rodada.

---

## 0.5.1 — 2026-08-28

Rodada de desempenho. Antes de repartir trabalho por nucleos, valia conferir se
o trabalho precisava existir — e nao precisava.

### Mudado

- **CRC-32 slice-by-8: a insercao ficou 3,1× mais rapida.** O medidor apontou o
  CRC da pagina inteira como o custo dominante: 10 µs por pagina de 4 KiB, e
  ~17 toques de pagina por linha inserida, porque toda leitura e toda gravacao
  do `.ndx` passa os 4096 bytes pelo laco byte a byte.

  O laco tem dependencia serial — cada volta precisa do CRC da anterior para
  indexar a tabela —, o que o prende a uma leitura de memoria por byte. Com
  oito tabelas, os oito bytes de uma palavra sao consultados em paralelo pelo
  processador. Mesmo polinomio, mesmo resultado, nenhuma mudanca de formato.

  | | antes | depois |
  |---|---:|---:|
  | só `.reg` | 6,5 µs | 6,3 µs |
  | +1 índice | 50,0 µs | 19,6 µs |
  | +1 único | 132,3 µs | 43,2 µs |
  | +2 índices | 177,1 µs | **56,5 µs** |
  | linhas/s | 5.645 | **17.700** |

  O CRC isolado sai de 10,00 µs por pagina (0,41 GB/s) para 2,34 µs (1,75 GB/s).

  Como um CRC diferente invalidaria todo arquivo ja gravado, o laco byte a byte
  ficou no codigo como definicao de referencia, e ha teste comparando os dois em
  todo tamanho de 0 a 300 bytes com quatro sementes, mais a pagina de 4096.

### Adicionado

- **`phxsql-core/src/paralelo.rs`** — divisao de faixa entre nucleos com
  `std::thread::scope`, sem dependencia externa. Nao e um `rayon`: e o pedaco
  de `rayon` que este projeto usa.

  A ordem do resultado e **sempre** a do laco sequencial — cada pedaco junta o
  seu num vetor proprio e os vetores sao concatenados na ordem dos pedacos. Uma
  consulta que mudasse de ordem conforme o numero de nucleos da maquina seria
  pior do que uma consulta lenta.

- **Varredura em memoria dividida entre nucleos.** A consulta sem atalho de
  mapa e o unico trecho do motor que divide bem: tudo em RAM, nada gravado, cada
  linha independente das outras. Um milhao de linhas, 4 nucleos: **36 ms → 20 ms**.

- `examples/paralelo.rs` e `examples/onde-doi.rs`, os dois medidores que
  sustentam os numeros acima.

### Sabido

- **O ganho da varredura paralela e 1,8×, nao 4×.** O filtro por linha e barato,
  entao a varredura e presa a banda de memoria e nao a conta: mais nucleo nao
  compra mais banda. O numero esta aqui para ninguem esperar escala linear.

- **A insercao continua monothread, e nenhuma thread a acelera.** Inserir uma
  linha e uma descida na B+tree em que cada passo depende do anterior. O que
  falta para varias conexoes gravarem ao mesmo tempo nao e thread — o servidor
  ja abre uma por conexao —, e **trava por tabela em vez da trava unica global**
  que hoje serializa todo acesso a dados.

---

## 0.5.0 — 2026-08-28

### Adicionado

- **Três tipos de identificador**, todos de largura fixa e inteiros dentro do
  slot — nenhum vai para o `.bin`, nenhum custa um ponteiro.

  | Tipo | Bytes | O que é |
  |---|---:|---|
  | `Uuid` | 16 | UUID de 128 bits do RFC 9562, v4 e v7 |
  | `Uuid256` | 32 | identificador de 256 bits — **não é um UUID**, o padrão só define 128. Existe porque um SHA-256 cabe exato |
  | `Sequence` | 8 | contador crescente da tabela, atribuído na inserção |

- **UUID v7, e o motivo é medido.** Os 48 bits altos de um v7 são o relógio em
  milissegundos, em big-endian; como a chave do `.ndx` guarda os bytes na ordem
  natural, comparar bytes é comparar tempo. Chave aleatória manda cada inserção
  para uma folha diferente da B+tree; chave crescente cai sempre na folha mais à
  direita, que já está na memória.

  É exatamente onde a bancada dói: a inserção cai de 5.089 linhas/s no primeiro
  milhão para 3.626/s no décimo, com o disco parado e a CPU em 99%. É a árvore
  sendo semeada, não o disco.

- **Monotonia de verdade.** Dois v7 no mesmo milissegundo sairiam fora de ordem
  se dependessem só do relógio, então os 12 bits de `rand_a` viram um contador
  (método 1 da seção 6.2 do RFC 9562): nasce sorteado a cada milissegundo novo e
  soma 1 a cada id seguinte; estourou, o relógio anda 1 ms para frente em vez de
  repetir. O gerador nunca devolve valor menor ou igual ao anterior, nem entre
  *threads* — há teste que pede vinte mil seguidos e exige que cada um cresça.

  O layout se confere contra o vetor do apêndice A.6 do próprio RFC.

- **A sequência, e a diferença para o rowid.** O rowid é a *posição física* do
  registro e não se escolhe; a sequência é dado — nasce onde se quiser, é
  gravada à mão e continua de onde parou. Valor escrito à mão **empurra o
  contador** para depois dele, senão a próxima numeração automática passaria por
  cima do que já existe. Excluir não devolve o número. Numa alteração, nulo
  mantém o número que a linha já tinha: a sequência identifica a linha, e
  renumerar trocaria a identidade dela.

- Pelo protocolo o id viaja em texto e **sai sempre na forma canônica
  minúscula** — um id que se escreve de dois jeitos vira dois ids no olho de
  quem lê. A palavra `"novo"` no lugar do valor pede ao servidor que gere um
  (`"v4"` força a versão sorteada); `Uuid256` aceita o prefixo `0x`.

- `crates/phxsql-store/examples/identificadores.rs` — monta uma tabela de
  blocos encadeados pelo hash, com a altura numerada pela sequência. Existe
  porque criar tabela ainda só se faz escrevendo Rust.

- Seção 4 do dossiê e seção 8 do `docs/FORMATO.md`.

### Mudado

- **Formato em disco**: os bytes 36..44 do cabeçalho do `.reg`, antes
  reservados, passam a guardar o próximo valor da sequência. Zero continua
  significando "nunca usada", então `.reg` antigo abre sem conversão.

- Uma sequência por tabela: duas dividiriam o mesmo contador do cabeçalho, o
  que só pareceria defeito. O esquema recusa na criação.

### Sabido

- **A sequência sozinha não é chave única.** O contador só vai ao disco no
  `sincronizar`; queda de energia antes disso o faz voltar atrás, e números já
  gravados podem repetir. Quem precisa de unicidade declara um índice `unico`
  sobre a coluna — aí é o índice que recusa.

- Um `.reg` gravado com estes tipos **não abre** numa versão anterior do
  binário: a tag do tipo é desconhecida lá, e o erro é claro.

---

## 0.4.1 — 2026-08-28

Rodada de revisão: nada de recurso novo, só o que a leitura do próprio projeto
achou de errado.

### Corrigido

- **A bancada media coisas diferentes dos dois lados.** Na varredura por faixa
  o MySQL(R) recebia `COUNT(*) + SUM(valor)` sobre 1.250.000 linhas enquanto o
  PhxSql lia 20.000 — mesma pergunta, 1,6% do trabalho. O «5× mais rápido» que
  saía dali não era o motor: era o serviço menor. A fase `varrer` de
  `examples/carga.rs` passou a ler a faixa inteira e somar o valor, e a
  medição de dez milhões foi **refeita do zero**.

  É o segundo erro deste tipo — o primeiro favorecia o MySQL(R), este
  favorecia o PhxSql. Por isso a bancada ganhou uma quarta regra: *mesma
  quantidade de trabalho*, não só mesma forma de pergunta.

  A prova de que agora está igual não é a promessa, é a soma: os dois motores
  devolvem 1.250.000 linhas e **5.576.201.000,00**, o mesmo total até o
  centavo, por dois códigos sem uma linha em comum.

  E o resultado sobreviveu ao conserto — a varredura continua a favor do
  PhxSql, por **3,3×** em vez dos 5× que a montagem errada prometia. A nova
  medição: inserção 20,7× mais devagar (4.039 linhas/s contra 83.492), busca
  pontual 2,6× mais devagar, exclusão 2,0×, atualização empatada, varredura
  3,3× mais rápida. Escreve 2,29 GiB onde o MySQL(R) escreve 32,03; ocupa
  2,27 GiB onde ele ocupa 0,88.

- **Campo com nome errado no `config.json` era silencioso.** Quem quisesse
  trocar a porta escreveria `"porta": 5001`, e o campo se chama `bind`: o
  servidor subia na 5000 sem uma palavra. O arranque agora lista os campos que
  não reconheceu e diz que o valor foi ignorado. Não vira erro — config antigo
  continua subindo.

- **Seis marcas de terceiros sem o `(R)`**: `MySQL` em `docs/REPLICACAO.md` e
  no dossiê, `HFSQL` em dois módulos, `SQLite` e `Clarion` no `docs/PLANO.md`.

- **O painel tem sete gráficos, não nove.** O README e o dossiê diziam nove.
  Contados: um de área, um de anel e cinco de barras.

- **A versão que o servidor anunciava estava errada.** O `Cargo.toml` do
  workspace ainda dizia `0.1.0` enquanto este changelog ia em 0.4.0 e os
  pacotes saíam com 0.4.0 no nome. Como `VERSAO` é `env!("CARGO_PKG_VERSION")`,
  o `ping`, o `quem_sou` e o rodapé do Centro de Controle respondiam `0.1.0` a
  quem perguntasse. Cliente que decide compatibilidade pela versão estava
  recebendo a resposta errada há três lançamentos.

- **Números velhos no dossiê.** A capa dizia 276 testes (são 280) e 3.184
  linhas de doc (são 3.261); o rodapé ainda dizia *PhxSql 0.3.0 · 19.242
  linhas · 69 KB de interface*, três números defasados de uma vez. Remedidos:
  20.224 linhas de Rust, 158 KiB de interface, 280 testes. A regra do projeto é
  medir, e ela vale para o documento que apresenta o projeto.

- **A bancada não estava no dossiê.** A comparação com o MySQL(R) em dez
  milhões de registros — a maior medição já feita aqui — existia só em
  `bancada/` e no roteiro, como uma linha marcada «pronto». Virou a seção 16,
  com a figura, a tabela dos oito números e o diagnóstico da inserção.

- **Três pedidos não estavam nem registrados.** Triggers, stored procedures e
  jobs foram pedidos e não constavam do roteiro do dossiê — nem como «a fazer».
  Ausência que não está escrita é ausência que se esquece.

### Adicionado

- **`docs/PENDENCIAS.md`.** A revisão do que falta, em um lugar só: o que foi
  pedido e não existe, o que depende de decisão do Adriano, o que está travado
  de fora, o checklist das perguntas já respondidas, e o único buraco que a
  medição apontou sem ninguém pedir.

- **`empacotar.sh`.** Monta os pacotes de Linux e Windows e o zip de fontes.
  Os das rodadas anteriores foram feitos à mão — pacote que ninguém consegue
  refazer é pacote em que não se deve confiar. O zip de fontes sai de
  `git archive`, que respeita o `.gitignore` de graça.

- **`docs/dossie/numeros-da-bancada.py`.** A figura, a tabela e o diagnóstico
  da seção 16 passam a ser **gerados** de `bancada/resultados.json`. Número
  digitado envelhece calado; número gerado não tem como divergir da medição.

- **`.gitignore`** para os 2,4 GB que a bancada cria em `bancada/phxsql/`.

---

## 0.4.0 — 2026-08-27

### Adicionado

- **Painel.** A primeira tela depois do login: o servidor inteiro em gráficos
  — bancos, registros, usuários, conexões, acessos, recusados, IPs bloqueados
  e tabelas em RAM nos números do topo; operações por hora nas últimas 24 h;
  operações mais pedidas; usuários por nível; maiores tabelas; de onde vêm os
  acessos; e quem mais usou.

  Tudo de **uma** chamada — a operação `painel` agrega no servidor. Dez
  chamadas deixariam a tela dez vezes mais lenta só pela ida e volta. E o
  painel conta **só o que quem está olhando poderia abrir**: base sem
  permissão de leitura não entra na conta.

  Os gráficos são SVG escrito à mão — barras, área e anel —, como o resto do
  projeto. Usam `currentColor` e os tokens do tema, então trocam de cor com o
  sol/lua sem uma linha a mais.

- **O phx-grid v0.8.0 na aba Conteúdo.** O grid do ecossistema Phoenix, ES5
  estrito e sem dependência. Arrastar um cabeçalho para a faixa de cima
  **agrupa**, com contagem e agregados por grupo; vários níveis empilham e as
  pastilhas reordenam arrastando. Vieram junto a busca global e a paginação.

  As colunas saem do **esquema** da tabela, não de uma lista escrita à mão —
  tabela nova aparece certa sem tocar na página. E o grid segue o tema do
  console.

- **Comparação medida com o MySQL(R)**, 10.000.000 de registros, em
  `bancada/`. Tudo para ser refeito: `python3 bancada/medir.py 10000000`.

- **Espelho `.bkp`** (`"espelho": true`): toda escrita no `.reg` vai também
  para um irmão, e a leitura tenta o espelho quando o CRC falha. `phxsql
  reparar` conserta nos dois sentidos e **conta** o que não teve salvação.

- **Três portas de replicação:** `envio` e `retorno` separadas, validadas
  contra a porta de dados, a da web e uma contra a outra.

- `phxsqld --pagina` escreve o Centro de Controle num arquivo — da **mesma**
  função que serve o navegador.

### Corrigido

- **Ligar o espelho apagava a cópia boa.** `espelhar()` copiava o `.reg` por
  cima do `.bkp` existente: estragar o principal e religar o espelho destruía
  a segunda chance. Um teste pegou. Agora só semeia o que ainda não existe.

- **Erro de medição na bancada.** A primeira versão mandava ao MySQL(R) um
  único `WHERE id IN (…)` e ao PhxSql vinte mil buscas separadas — 41× a
  favor do MySQL(R) pela *forma da pergunta*, não pelo motor. Corrigido para
  uma instrução por operação dos dois lados; o SELECT pontual passou de 41×
  perdendo para 3,4×, e o UPDATE de perdendo para empate.

- **Gráficos desproporcionais.** O `viewBox` de 620 dentro de cartões de
  ~370 px encolhia o desenho inteiro em 0,6 — texto de 12 px virava 7 px.
  Cada gráfico passa a nascer com a largura do cartão que o recebe.

- **Colisão de nome entre o relay e o backup**: o campo do servidor remoto se
  chamava `destino`, e `destino` já era o diretório do backup. Renomeado.

### Sabido

O que a medição diz e ninguém deve esconder: **a inserção é o nosso buraco**
— 3.685 linhas/s contra 95.301 do MySQL(R), e é CPU, não disco. Continuam
faltando triggers, stored procedures, jobs, transporte de replicação,
start/stop pela interface, transações e TLS.

**280 testes**, clippy limpo, zero dependências externas.

---

## 0.3.0 — 2026-08-27

### Corrigido

- **O nível de usuário quase afrouxou todo `config.json` existente.** O padrão
  do campo novo `nivel` era `leitor`, e isso mudava o comportamento de quem já
  tinha config: base sem regra explícita passava de *nega tudo* para *lê tudo*.
  Um teste antigo (`sem_curinga_e_sem_base_nega_tudo`) quebrou e apontou o
  problema. Existe agora `Nivel::Nenhum`, que é o padrão, e o teste antigo
  passa sem alteração — que é a prova de que nada mudou para quem já tem
  config.

- **`phxsqld --usuarios` mentia sobre quem podia o quê.** Escrevia
  `(nenhuma)` para usuário sem regra de base, mesmo quando o nível dava poder,
  e mostrava `supervisor` numa coluna em vez do nível. Agora mostra o nível e
  o que ele concede.

### Adicionado

- **Nível de usuário:** `nenhum`, `leitor`, `operador`, `dono`, `admin`. Cada
  um contém o anterior, e há teste que percorre as dez atividades para
  garantir. A regra de uma base específica ganha do nível, inclusive para
  **tirar** poder — dá para dar `admin` a alguém e ainda assim fechar uma base.

- **Backup em ZIP**, com o DEFLATE (RFC 1951) escrito neste projeto — Huffman
  fixo mais casamento LZ77. Nome
  `BancoNome_Admin_Data_HoraMin.zip`, com o manifesto dentro.

  A prova não é o teste de ida e volta com o próprio código; é o mundo abrir:
  `unzip -t` passa todos os CRC, e o `zipfile` do Python extrai e confere byte
  a byte contra o original. **18.311 → 2.406 bytes, 87% menor.**

- **Backup agendado**, seção `backup` no `config.json`, desligada por padrão.
  `hora` (uma vez por dia) ou `cada_horas`, com `manter` para a retenção. O
  relógio confere de minuto em minuto em vez de dormir até a hora — dormir
  horas seguidas é frágil. A faxina só apaga arquivo com a cara dos nossos.
  Todo backup agendado entra no `acessos.log`.

### Sabido

Continua tudo da 0.2.0: replicação sem transporte, sem start/stop pela
interface, sem transações, sem TLS, sem compactação, sem SQL, sem MCP, sem
ODBC.

**276 testes**, clippy limpo, zero dependências externas.

---

## 0.2.0 — 2026-08-27

### Corrigido

- **Sondagem de travessia de diretório não contava violação.** Nome de
  database, tabela ou schema com `..` ou barra já era recusado pelo motor,
  mas era recusado **calado**: não contava tentativa e não gerava bloqueio.
  Auditado com seis sondagens seguidas (`../../../etc`, `/etc`, `C:\dados`,
  byte nulo, quebra de linha): seis recusas, seis linhas no `acessos.log`,
  **zero bloqueios**. Quem sondasse podia tentar a noite inteira.

  Agora é violação grave, na mesma classe de comando proibido: **bloqueia na
  primeira tentativa** e cria a regra de firewall. Conferido contra servidor
  de verdade — uma sondagem, um bloqueio, uma regra.

  A separação está em `catalogo::nome_hostil`, deliberadamente distinta de
  `validar_nome`: `"minha tabela!"` é um nome ruim (alguém errou, recusar
  basta); `"../../etc/passwd"` não é nome nenhum.

- **Colisão de nome entre o relay e o backup.** O campo que escolhe o servidor
  remoto se chamava `destino` — e `destino` já era o diretório do backup.
  Resultado: todo pedido de backup ia parar no relay e voltava com "esta
  interface não fala com outro servidor". Renomeado para `servidor`. Achado
  ligando as duas peças, não lendo o código.

- **`fe_de_bytes` do Ed25519 lia sete bytes onde precisa de oito.** O pedaço
  do meio perdia o bit 152. Passou despercebido no teste do ponto base — que
  tem esse bit em zero — e só apareceu quando os vetores da RFC 8032 rodaram.
  É exatamente por isso que a regra "criptografia se confere contra vetor
  oficial" existe.

- **Duas cores presas ao tema escuro.** O gradiente da tela de entrada e a
  tinta do botão eram literais. No tema claro o botão ficava com tinta quase
  preta sobre vermelho escuro. Viraram token.

### Adicionado

- **Tabela em memória e `SelectMemory`.** A tabela inteira em RAM, com
  consulta que não toca em disco. Filtros (`=`, `!=`, `<`, `<=`, `>`, `>=`,
  `contem`, `comeca`, `termina`, `nulo`, `nao_nulo`), ordenação múltipla,
  projeção de colunas, `pular` e teto. Filtro de igualdade numa coluna
  mapeada evita a varredura, e a resposta diz qual mapa usou.

  **Medido** (`cargo run --release --example memoria`, 50.000 linhas, a mesma
  pergunta pelos dois caminhos):

  | caminho | tempo | linhas examinadas |
  |---|---:|---:|
  | varrendo o `.reg` | 55.878 µs | 50.000 |
  | `SelectMemory` | 641 µs | 8.333 |

  **87×.** Carga para a RAM: 53 ms, 2.205 KB de valores. O exemplo confere as
  duas respostas linha por linha antes de imprimir o número.

  Nada entra em memória sozinho, e toda escrita atualiza a cópia residente
  **dentro da mesma trava** do disco — não existe janela em que os dois
  discordem.

- **Chave assimétrica Ed25519 como segundo fator.** Escrito do zero, mais o
  SHA-512 que ele exige. Conferido contra os quatro vetores da RFC 8032
  seção 7.1, o vetor de 1023 bytes, e os quatro do FIPS 180-4 para o SHA-512.

  E a prova que vale mais: um cliente de teste que assina com a implementação
  **de referência** da RFC (Python puro, independente desta) gerou a mesma
  chave pública e teve a assinatura aceita pelo servidor.

  `phxsqld --gerar-chave` imprime o par uma vez. `"chave_publica"` no usuário
  do `config.json` passa a exigir assinatura no login, sobre o **mesmo**
  desafio da senha — então a assinatura também vale uma vez só.

- **Sistema de backup com manifesto conferível.** Cópia mais um `backup.json`
  com o SHA-256 de cada arquivo, e um comando que lê tudo de volta e confere.
  Acha arquivo que sumiu, arquivo que mudou (mesmo do mesmo tamanho) e
  arquivo que apareceu sem estar no manifesto.

  ```
  phxsql backup <base> <destino>        com o servidor parado
  phxsql conferir-backup <destino>      sai com erro se não bater
  {"op":"backup","destino":"..."}       com o servidor no ar, sob a trava
  ```

- **Alternador de tema, sol ☀️ e lua 🌙.** Paleta clara completa, começando no
  que o sistema pede e lembrando a escolha por navegador. O vermelhão
  escurece para `#c63c0a` no claro, por contraste — a mesma adaptação que o
  dossiê já fazia.

- **Campos de conexão no login:** servidor (IP ou DNS), porta, usuário, senha,
  chave privada e database. A porta que aparece é a que o servidor
  **realmente** escuta, lida do `/saude`.

- **Console para mais de um servidor.** Apontar o login para outro endereço
  abre uma conexão para ele, mantida viva pela sessão. `web.servidores`
  começa **vazio** — interface que fala com qualquer endereço é proxy aberto
  de saída.

- **`replicacao.escuta`:** o socket onde o *source* serve os eventos, separado
  da porta de dados. O config recusa colisão com a porta de dados e com a
  da web.

### Mudado

- Nomes de bancos de terceiros na documentação passam a levar **(R)**.
  Exceções deliberadas: nomes de pacote (`rusqlite` é identificador, não
  marca) e citações literais de texto alheio.
- `memoria_carregar`, `memoria` e `SelectMemory` pedem permissão de **ler**,
  não de administrar: é o mesmo dado do disco por outro caminho.
- O arranque avisa alto quando o papel de replicação não é `isolado`, porque
  o transporte de eventos ainda não existe.

### Sabido — o que ainda não funciona

- **Replicação não transporta evento.** A configuração entra e valida; o
  desenho está em `docs/REPLICACAO.md`; o `.log` v2 com imagem da linha é o
  próximo passo. Hoje o papel é só um rótulo.
- **Start/stop do serviço de dados pela interface** não existe. Parar a porta
  5000 sem derrubar o processo exige mexer no laço de aceitação, e prefiro
  fazer isso inteiro a fazer pela metade.
- **Sem transações**, logo sem o A nem o I do ACID.
- **Sem TLS.** O tráfego vai em claro; a credencial não, quando se usa
  desafio-resposta ou chave.
- **Sem compactação**, sem camada SQL, sem MCP, sem ODBC.
- `crypto.subtle` com Ed25519 é recente; navegador sem suporte não assina, e
  a página diz isso em vez de fingir.

**254 testes**, clippy limpo, zero dependências externas.

---

## 0.1.0 — 2026-08-27

Primeira versão que roda ponta a ponta.

### Adicionado

- **Os cinco arquivos:** `.reg` (registros na ordem de digitação, CRC por
  registro, esquema embutido), `.ndx` (B+tree com divisão de páginas, chave
  composta, ASC/DESC/NOCASE/único), `.bin` e `.memo` (blocos com CRC e
  contabilidade de espaço morto), `.log` (diário datado das três operações).
- **Ordem de digitação como garantia:** slot excluído nunca é reaproveitado.
- **Paginação em volumes** `_001`, `_002`, … com abertura preguiçosa. O
  volume sai da aritmética do rowid, então o índice não paga nada por ela.
- **Hierarquia** database → schema → tabela, em diretórios.
- **Chave estrangeira** no esquema, com CASCADE / RESTRICT / SET NULL.
- **Reindex:** recria o `.ndx` do zero a partir do `.reg`.
- **Servidor TCP na porta 5000**, protocolo JSON Lines, `config.json`.
- **Log de acessos** por IP, com data e hora ao milissegundo — inclusive das
  tentativas recusadas.
- **Cadastro de usuários** com senha em PBKDF2-HMAC-SHA256 de 210.000
  iterações, e permissão por base em dez atividades.
- **Login por desafio-resposta** (a senha não trafega) e por Base64.
- **Política, blacklist e gancho de firewall:** comando proibido bloqueia o
  IP na hora; token e senha errados contam tentativa.
- **Centro de Controle:** interface web embutida no binário, servida pelo
  próprio `phxsqld`.
- **Linha de comando** com nove comandos, e compilados para Linux e Windows.

### Sabido

Zero dependências externas — só a `std`. JSON, CRC-32, SHA-256, HMAC, PBKDF2
e Base64 escritos aqui, cada um conferido contra vetor oficial.
