# O reparo cura a trava e não a thread; e a tomada no desenrolar não é pânico com a trava na mão

**Estado:** PENDENTE

## O que aconteceu

Pedido 451, A2 e M2 da revisão adversária do DBA
(`docs/propostas/parecer-dba-451-2026-09-24.md`). O reparo da trava de dados
roda no `Drop` do `TravaMedida` e devolve a trava ao serviço. As provas eram
todas numa thread de **atendimento**, cuja morte leva a conexão, e o cliente vê.

- **A2.** Nas threads de **serviço** (`relogio-gravacao`, `replica-*`,
  `replica-cluster`, `backup-agendado`, `relogio-jobs`), o reparo dava certo e
  a thread morria mesmo assim, sem ninguém para subi-la. Com a H1 o processo
  inteiro recusava, o que era visível; com o reparo, a degradação ficou calada.
- **M2.** O portão do reparo era só `std::thread::panicking()`. O `AoSair` de
  uma conexão que cai por um pânico **fora** da trava toma a trava no
  desenrolar, para soltar a carga reservada. O `Drop` dela via `panicking()`
  verdadeiro e reparava (ou abortava) por um pânico que nunca tocou em dado.

## O que eu concluí primeiro, e estava errado

Duas vezes a mesma coisa: tomei o **estado compartilhado** como a vítima
inteira do pânico. Na A2, curar a trava parecia curar o servidor, e a thread
morta era «o preço de sempre de um pânico». Na M2, «a thread está desenrolando»
parecia o mesmo que «o pânico aconteceu com a trava na mão». A `std` já fazia a
distinção que eu não fiz: o guard guarda o `panicking()` da tomada, e não
envenena ao cair num desenrolar que começou antes dele.

## O que a medição disse

Pelo processo filho (o próprio binário de testes reexecutado):

- A2, com o gancho do fecho armado na thread `relogio-gravaca`, e sem a regra
  da família: «o panico no relogio-gravacao nao derrubou o processo, e a janela
  parou de fechar sozinha: 1 marca(s) de commit ainda no disco 2 s depois de um
  relogio de 150 ms».
- M2, com o reparo que falha armado e um pânico no `despachar` de uma conexão
  com carga reservada, com o portão só no `panicking()`: «um panico FORA da
  trava derrubou o processo ... ExitStatus(unix_wait_status(6))».

## A regra

Depois de curar o estado compartilhado, pergunte quem **morreu** junto e se
alguém vai ver essa morte. E, antes de reagir a `panicking()` num `Drop`,
pergunte se o pânico começou com o recurso na mão ou antes de tomá-lo.

## Como está guardado hoje

- A família da thread sai do `telemetria::subir`, o único `spawn` do servidor
  (`telemetria::familia_desta_thread`), e pânico com a trava numa thread
  `servico` aborta (H5). Guarda `panico-em-thread-de-servico-morre-calado`.
- `TravaMedida::tomada_no_desenrolar`, e a guarda
  `reparo-no-desenrolar-de-panico-de-fora`.
- **O buraco:** a regra da família vale só para o pânico **com a trava de dados
  de ESCRITA na mão**. Uma thread de serviço que morre com a de leitura, ou
  fora de trava, continua morrendo calada, porque leitor não envenena; a do
  pulso é o caso já nomeado no `docs/SEGURANCA.md` §21.6.

## Adendo: o preço que eu declarei estava errado (segunda revisão do DBA)

Escrevi que um job que entra em pânico sob a trava «derruba o servidor na
cadência do job». Não li o agendador antes de escrever o preço: o `ultimos` dos
jobs zera a cada arranque e o backup começa em `ultimo = 0`, e com zero o
`hora_de_rodar` diz que venceu. O laço tem a cadência do **arranque**: pânico,
abort, sobe, roda de novo, abort, até alguém desligar o job à mão
(`docs/SEGURANCA.md` §24.2, `MANUAL.txt`). A regra para a próxima: o preço de
uma queda de propósito se lê em **quem roda de novo depois da subida**, e não
em quem causou a primeira. Não medido; é leitura do código
(`docs/propostas/parecer-dba-451-2a-2026-09-24.md`, T1 e N1).
