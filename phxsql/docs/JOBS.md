# Jobs de execução — estado, aviso por e-mail e as lições

O básico (o que é um job, o cadastro, o relógio, o histórico) está na seção
11.1 do `MANUAL.txt`. Este documento guarda o **desenho** do estado por job e
do aviso por e-mail, e os aprendizados da bateria que os provou — inclusive os
infrutíferos, para a mesma ideia não voltar sem medição.

## O estado por job

A resposta de `jobs` (apelido `job_listar`) traz por job um `estado`:
`rodando` / `desligado` / `ok` / `falhou` / `agendado` / `nunca_rodou` — nessa
ordem de prioridade. O que acontece AGORA ganha do que já aconteceu, e o
desligado ganha do histórico: um job fora da agenda não está "ok", está fora
da agenda.

Três decisões que não são óbvias lendo a lista:

- **A tela não recalcula nada.** O estado sai de `jobs::estado_do_job`, o
  vencido da mesma `hora_de_rodar` do relógio e a próxima prevista de
  `Agenda::proximo_ms`, que é a mesma conta olhada do outro lado. Duas
  implementações da mesma regra é o jeito clássico de a tela e o relógio
  discordarem.
- **A última corrida sobrevive ao reinício; o agendamento não.** O `Registro`
  semeia a última corrida de cada job da cauda do `jobs.log` no arranque —
  é isso que deixa a tela dizer "falhou às 03:00" depois de um restart. Mas o
  `ultimos` do agendamento continua zerando de propósito: um "a cada 6 h"
  deve rodar logo depois do arranque, e semear o relógio do log mudaria esse
  comportamento sem ninguém pedir.
- **`parado` é o vencido que ninguém vai rodar**: ligado, hora vencida, sem
  corrida em andamento e **sem relógio no ar** — o relógio só sobe no
  arranque, e só se já havia job ligado. O caso típico é real: o primeiro job
  ligado pela tela fica esperando um reinício que ninguém fez.

### Hipótese descartada: "parado" por janela de atraso

O primeiro desenho media o atraso: um vigia anotaria desde quando cada job
está vencido e chamaria de parado quem passasse de uma janela (3× o período
do relógio). Morreu por dois motivos: com o relógio no ar, um job vencido
roda em até 30 s — o atraso **nunca acumula**, e a janela só compraria alarme
falso no arranque (todo `cada_minutos` nasce vencido) e ~90 s a mais de espera
na prova. E o predicado `vencido && sem relógio` responde a mesma pergunta
sem estado extra, com a MESMA função da tela. Se um dia o relógio puder
morrer com jobs ligados (hoje não pode: é um `loop` sem saída), a janela
volta a ser candidata — por isso ela fica registrada aqui.

## O aviso por e-mail

Dois avisos, pelo cliente SMTP que já existia (`email.rs`, o do disco
apertado — nada de segundo cliente):

- **falhou** — sai do `rodar_job`, depois de registrar no histórico (o
  histórico nunca depende de o relé estar no ar), numa thread própria (quem
  dispara pode ser a tela, e ela não espera timeout de relé).
- **parado** — sai de um vigia próprio de 60 s, que só existe se o aviso foi
  pedido, e dorme antes da primeira conferência para o arranque terminar de
  subir o relógio.

Regras herdadas do vigia de disco, de propósito: silêncio de `repetir_horas`
por job e por tipo de aviso; quando o problema alivia (o job roda), a chave
sai do mapa e a próxima falha avisa na hora. E a de sempre: **nenhuma senha em
mensagem nenhuma** — o corpo leva job, operação, login do dono, agenda, hora,
duração e o erro.

**Opt-in**: `alertas.email.avisar_jobs` (padrão `false`). Sem o campo, quem
configurou e-mail para o disco continua recebendo só o do disco; sem bloco de
e-mail, nada muda. Vale mesmo com `alertas.ligado` falso — o aviso de jobs não
depende do vigia de disco, e por isso o endereço é validado no arranque também
nesse caminho.

## A prova (`bancada/jobs/prova-avisos.py`)

SMTP falso em socket puro + um `phxsqld` próprio em 5303/5703. Cinco passos,
~3 minutos (o passo 3 espera a volta real do vigia — encurtar o relógio para
o teste seria provar outro relógio):

1. job roda bem → estado `ok`, nenhum e-mail;
2. job falha → estado `falhou`, UM e-mail com o motivo no corpo (e sem
   `pbkdf2`/senha em lugar nenhum); a mesma falha dentro do silêncio não
   repete;
3. job ligado num arranque sem relógio → ficha diz `nunca_rodou` + `parado`,
   e o vigia manda o e-mail de "sem rodar" em ≤75 s;
4. servidor SEM bloco de e-mail → mesmos eventos, ZERO conexões SMTP;
5. servidor COM e-mail ligado mas SEM `avisar_jobs` → job falha, ZERO
   conexões SMTP. É o corte fino de "guarda pedida, não imposta".

### Prova real (defeito reposto → teste falha)

- **Portão do opt-in removido** (`avisar_jobs` ignorado): o passo 5 falha —
  primeiro na tela (`aviso_email.ligado` vira `true` sem ninguém pedir), e o
  e-mail sairia. Com o conserto, verde.
- **Estados trocados** (`ok`↔`falhou` em `estado_do_job`): caem o unitário
  `o_estado_segue_a_prioridade` e o de soquete
  `o_estado_conta_a_historia_do_job`.

O primeiro defeito ensinou algo antes de ser desfeito: reposto "no portão",
ele caiu primeiro numa **cópia** da condição que o `op_jobs` tinha para
pintar a tela — o portão estava escrito em dois lugares. A cópia virou uma
chamada a `aviso_de_jobs_ligado()`, que agora é o lugar ÚNICO da decisão:
a tela que mente sobre o aviso é exatamente o tipo de furo que ninguém acha
por leitura.

## A tela (e o que a captura achou)

A lista pinta o estado em `pino`, mostra última corrida (com o desfecho e o
erro no `title`) e a próxima prevista, e ganhou três botões por linha —
Rodar/Ligar-Desligar em amarelo, Excluir em vermelho, sempre contorno. O
quadro "Aviso por e-mail" diz a verdade da configuração em cada um dos três
casos (ligado / e-mail sem `avisar_jobs` / sem e-mail).

O que só a captura mostrou (interface só se prova exercitando, de novo):

- **A tabela estourava a largura e os botões de ação sumiam** da tela — data
  com ano e segundos, agenda quebrando em três linhas, legenda de 360 px.
  Conserto: dia/mês + hora:minuto na lista (o carimbo completo continua no
  histórico logo abaixo), agenda sem quebra, legenda em 250 px, e os botões
  da linha **quebram** quando falta largura em vez de empurrar a tabela.
- `.leg` só tinha estilo dentro de formulários: numa célula de tabela a
  "legenda" saía do tamanho do dado. O estilo novo é escopado por
  `.linha-job` — um `td .leg` global morderia as outras tabelas que usam a
  mesma palavra.

E uma armadilha operacional que quase virou acidente: o `$!` de um
`nohup ... &` embrulhado devolveu o PID do *wrapper*, não do `phxsqld` — o
`kill` matou o embrulho e o servidor velho ficou segurando a porta. Antes de
matar um `phxsqld`, conferir no `ps` que o `--config` dele é o seu; é a mesma
regra da casa de nunca tocar num servidor que não é o da sua prova.
