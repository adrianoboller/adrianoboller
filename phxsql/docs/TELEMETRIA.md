# Telemetria — o painel ao vivo, as threads e o encerrar

O molde é o **SQL Check da Idera(R)**: faixas de séries no topo e, embaixo, um
painel em que cada atividade viva é uma **bolha** — o tamanho é o peso, a cor é
o estado, e clicar abre o descritivo inteiro.

**Onde ela fica:** o botão `Telemetria` na barra de ferramentas, logo depois de
*Conexões* — e também em **Ferramentas → Telemetria ao vivo…**, pelo teclado. O
Profiler é o vizinho, nos dois caminhos: as três respondem à mesma pergunta —
o que está acontecendo agora.

Este documento é a parte que a tela não cabe: **o que cada número mede**, **o
que o botão de encerrar promete e o que ele não promete**, e **o inventário das
threads que o `phxsqld` cria**.

---

## 1. O que ele acrescenta, e o que ele reusa

Nada aqui foi reescrito. O que já existia continua sendo a fonte:

| pergunta | quem responde | onde |
|---|---|---|
| quem está conectado agora | `ligacoes` / op `sessoes` | `src/ligacoes.rs` |
| o que chegou pela porta, cru | Profiler | `src/profiler.rs` |
| como está a MÁQUINA | monitor de sistema | `src/sistema.rs` |
| quanto o `.ndx` acertou no cache | contadores do cache de páginas | `store/src/ndx.rs` |
| o que já foi pedido, historicamente | log de acessos / op `estatisticas` | `src/acesso.rs` |

O que a telemetria acrescenta é o que nenhum dos cinco tinha:

1. **o tempo** — os números guardados numa série, para se ver a subida e não só
   o instante;
2. **o peso** — quanto de servidor cada atividade já gastou, que é o que ordena
   as bolhas;
3. **a espera** — quanto tempo se passa na fila da trava de dados, que é o
   gargalo declarado deste servidor e não era medido em lugar nenhum;
4. **o encerrar** — parar uma operação longa sem derrubar a conexão e sem
   arriscar o arquivo;
5. **as threads** — um registro central com a finalidade de cada uma **escrita**.

---

## 2. As séries, e por que estas

Cinco faixas. A escolha não é decorativa: cada uma responde uma pergunta que
alguém faz quando o servidor está lento, e nenhuma delas tinha resposta.

### 2.1 Esperas — atividades por estado (área empilhada)

É o equivalente do *Waits* do SQL Check. Aqui a espera tem **um dono só**: a
trava única de dados (`Mutex<Instancia>`), por onde passam toda leitura e toda
escrita, uma de cada vez.

A faixa empilha a contagem de atividades em cada estado, a cada segundo:

| estado | o que quer dizer |
|---|---|
| `executando` | tem pedido e está com a trava de dados na mão |
| `esperando` | tem pedido e está **parada na fila** da trava |
| `encerrando` | foi marcada para encerrar e ainda não chegou no ponto seguro |
| `ociosa` | conectada, sem pedido nenhum |

O número em destaque é `espera_ms_s`: **quantos milissegundos de cada segundo o
servidor passou na fila**. Meio segundo por segundo quer dizer que, na média,
há sempre alguém parado esperando — e é um dos gatilhos de *stress*.

A medida vem de um lugar só. As 50 tomadas de trava do `servidor.rs` passaram a
chamar `travar_dados()`, e é lá dentro que o cronômetro está. Medir em cada uma
seria copiar a mesma conta 50 vezes, e a que alguém esquecesse viraria o buraco
na série — a mesma razão pela qual o portão de permissão é um só.

### 2.2 Leitura e escrita físicas — **deste processo**

Sai de `/proc/self/io`, campos `read_bytes` e `write_bytes`. São os bytes que
foram ao **dispositivo**, e não os que passaram pelo `read()` — o cache do
sistema pode ter servido esses de graça, e num painel de banco de dados só a
leitura física é notícia.

É diferente do `/proc/diskstats` que o monitor de sistema usa: aquele mede a
**máquina inteira**, este mede o **processo**. Num servidor compartilhado os
dois contam histórias diferentes, e a que interessa aqui é a nossa.

### 2.3 CPU — processo e máquina

Duas linhas. A do processo sai de `/proc/self/stat` (`utime + stime`), e passa
de 100% quando várias threads estão ocupadas — 100% é **um núcleo**, e isso é o
certo num servidor com uma thread por conexão. A da máquina é a mesma leitura
que o painel de sistema já fazia (`sistema::jiffies_da_maquina`), e não uma
segunda: duas leituras com bases diferentes dariam dois percentuais para o
mesmo instante, e quem olhasse as duas telas ficaria sem saber em qual
acreditar.

### 2.4 Vazão — operações por segundo

Leituras, escritas e erros concluídos por segundo. A separação entre leitura e
escrita sai de `OPS_ESCRITA`, a mesma lista que o modo somente-leitura usa — e
não de uma segunda lista que pudesse discordar dela.

### 2.5 Cache de páginas do `.ndx`

Acertos e faltas por segundo, e o percentual acumulado. É o cache que comprou
2,40× na carga (`docs/DESEMPENHO.md` §2), e até aqui ninguém conseguia ver se
ele estava servindo.

**A granularidade é por OPERAÇÃO, e o painel diz isso.** Cada `.ndx` aberto
conta os próprios toques num `u64` comum, que não custa nada porque não é
atômico nem compartilhado; esse número sobe para o contador do processo **uma
vez, quando o arquivo fecha**. Somar cada toque direto num atômico global
custaria uma instrução sincronizada por toque — e são **10,86 toques de página
por linha inserida**, medidos, num caminho em que a linha inteira leva 7,5 µs.
Instrumentação que cobra do caminho quente é instrumentação que muda o que ela
mede. Como o servidor abre e fecha a tabela a cada operação, na prática o
número anda a cada operação, e uma carga de cinco mil linhas só aparece quando
ela termina.

### 2.6 O atraso, medido e mostrado

Três números ficam na barra do topo, porque **série congelada parece série
calma**:

- **última amostra** — o instante do último ponto da série;
- **atraso da amostra** — a distância desse instante até agora; acima de 3 s
  aparece em vermelho;
- **ida e volta** — quanto o último pedido da tela levou.

Os números medidos estão na secção 7.

---

## 3. As bolhas

### 3.1 O identificador é do DONO, não do pedido

`dados:17` é a **conexão** 17 da porta de dados; `web:a1b2c3d4` é a **sessão**
do navegador. A operação dentro dela troca; a bolha não.

Se o identificador fosse do pedido, a tela redesenharia bolhas novas duas vezes
por segundo e ninguém conseguiria clicar em nenhuma. Uma sessão web fechada sai
da lista depois de **um minuto** de silêncio — a aba não avisa que fechou, e sem
essa poda o painel viraria um cemitério de abas de ontem.

### 3.2 O tamanho é o peso, e o peso é TEMPO DE SERVIDOR

`peso_ms` = milissegundos em que a atividade esteve **executando** — somados
sobre todas as operações dela, mais o trecho corrente.

Não é linhas. Linha lida e linha gravada custam coisas diferentes, e uma tabela
larga custa mais que uma estreita: somar linhas compararia coisas que não se
comparam. Tempo de servidor é a moeda única, porque a trava de dados é uma só —
é exatamente o que a atividade tirou de todo mundo.

O acumulado entra junto com o corrente de propósito: uma conexão que fez
trezentas consultas de 50 ms pesou 15 segundos do servidor, e uma tela que só
mostrasse a consulta corrente diria que ela não fez nada.

**Quem espera na fila NÃO engorda**, e isso custou uma versão para aprender. A
primeira contava o relógio de parede do pedido — e o navegador mostrou o
resultado com toda a clareza: com uma soma de verificação segurando a trava,
as **oito conexões bloqueadas atrás dela apareceram exatamente do mesmo
tamanho da soma**. Oito bolhas idênticas, e nenhuma pista de qual era o
problema. O tamanho, que existe para ordenar, tinha parado de ordenar.

Contando só o tempo em execução, a soma cresce e as bloqueadas não — que é a
leitura certa: uma está comendo o servidor, sete estão esperando. Medido
depois do conserto, na mesma carga: a soma com **27,6 s** de peso e raio 74, as
sete bloqueadas entre **742 ms e 975 ms**, raio 26.

Os dois relógios ficam **lado a lado no descritivo**, porque os dois são
verdade e respondem perguntas diferentes:

- **operação dura há** — o relógio de parede, espera incluída. É quanto tempo
  quem pediu está esperando, e uma tela que dissesse «0 ms» de uma consulta que
  já bloqueou o cliente por meio minuto estaria mentindo para o outro lado;
- **desse tempo, trabalhando** e **desse tempo, na fila da trava** — a
  repartição. A diferença entre os dois *é* a contenção, sem ninguém ter de
  deduzir.

**O raio sai da raiz quadrada do peso.** O olho compara *área* de círculo, então
usar o peso direto no raio faria uma atividade duas vezes mais pesada parecer
quatro vezes maior — é o erro clássico do gráfico de bolha, e ele mente para o
lado do exagero.

### 3.3 A cor, e o que acompanha a cor

| cor | nível | quando | o que acompanha |
|---|---|---|---|
| azul | `normal` | tudo em paz | borda contínua |
| amarelo | `alto` | operação acima de 2 s de relógio, **ou** parada na fila da trava | borda tracejada, **▲** |
| vermelho | `stress` | **trabalhando** há mais de 5 s, **ou** segurando a trava enquanto há fila | borda pontilhada, **■** |
| rosa | `encerrando` | marcada para encerrar, ainda não chegou no ponto | traço longo, **✕** |

**O vermelho tem de apontar UMA atividade**, e essa regra também saiu do
navegador. A primeira versão pintava de vermelho toda atividade em curso
enquanto o servidor estivesse apertado — e o painel inteiro ficou vermelho de
uma vez. A cor, que existe para separar, deixou de separar qualquer coisa,
justamente na hora em que quem opera abre o painel para achar **qual** delas é
o problema.

Agora o vermelho é de quem está **com a trava na mão enquanto há gente na
fila** — o culpado —, ou de quem sozinho já passou de 5 s de trabalho. Quem
está na fila é vítima, e fica amarelo. O aperto do **servidor** continua dito,
em vermelho e com o motivo, na barra do topo — que é onde ele é do servidor e
não de ninguém em particular.

E há um caso que o painel também aprendeu a distinguir: uma operação pode
estar *executando* sem nunca ter pedido a trava — o próprio `telemetria`, o
`ping`, o `catalogo`. A bolha da tela que estava **olhando** o painel aparecia
em vermelho toda vez que havia fila, acusada de segurar uma trava que ela nem
pediu. O campo `com_trava` separa as duas coisas.

A bolha de quem está olhando leva o rótulo **«esta é a sua tela»**. Escondê-la
seria mentir sobre quem está conectado; deixá-la anônima faria o operador
procurar quem é `w·85a62fd` — e é ele mesmo.

**A cor nunca é o único sinal.** Cada bolha carrega o traço da borda, o glifo, o
`<title>` e um `aria-label` com o estado por extenso — quem não distingue as
três cores continua lendo o painel. As quatro passam de 4,5:1 sobre o painel nos
dois temas, **medido no navegador** com a fórmula da WCAG:

| cor | escuro | claro |
|---|---:|---:|
| azul (`--reg`) | 7,26:1 | 6,98:1 |
| âmbar (`--ambar`) | 11,84:1 | 4,61:1 |
| vermelho (`--vermelho`) | 6,32:1 | 6,72:1 |
| rosa (`--acao-marcar`) | 8,96:1 | 5,94:1 |
| texto (`--texto`) | 14,47:1 | 18,45:1 |

Elas saem das variáveis do tema e escurecem no claro pelo mesmo motivo do
vermelhão da marca.

**O nível é decidido no SERVIDOR**, e vai no campo `nivel` da resposta junto com
os limiares em `limiares`. Com a regra escrita na tela também, o dia em que um
dos dois mudasse a tela pintaria uma cor que o servidor não concorda — é a mesma
razão pela qual `sistema` manda o `livre_minimo_percentual` junto com o espaço
livre.

### 3.4 «Servidor em stress» vem com o motivo

`stress` é um adjetivo, e adjetivo não se conserta. O que se conserta é o
motivo, e ele vai no campo `stress_por_que`:

- CPU da máquina em 90% ou mais;
- 500 ms ou mais de cada segundo na fila da trava de dados;
- alguma atividade esperando a trava há mais de 5 s.

**O disco não entra**, e é decisão declarada: saber se ele está apertado custa
um `df`, que é um processo do sistema, e a tela pergunta de dois em dois
segundos. O aperto de disco já tem dono — o vigia de disco e o painel de
sistema — e ele avisa por e-mail, que é mais útil que uma cor.

---

## 4. Encerrar uma atividade — o que é e o que não é cancelável

### 4.1 Por que cooperativo

Rust não mata thread no meio, e ainda bem. Uma escrita interrompida entre o
slot do `.reg` e a chave do `.ndx` deixaria a tabela mentindo: o registro
existiria e o índice não o encontraria, ou o contrário. **Nunca comprometer o
dado** ganha de qualquer botão.

Então encerrar aqui é **marcar**, e a marca só é olhada em **ponto seguro** —
entre duas unidades de trabalho, nunca no meio de uma. O ponto de cancelamento
é `Atividade::siga()`: um `fetch_add` e um `load`, os dois `Relaxed`, num `Arc`
que o laço já tem na mão.

### 4.2 A marca morre com a operação que ela mirou

Cada operação tem um serial, e a marca guarda o serial que mirou. Sem isso,
mandar encerrar uma conexão parada mataria o **próximo** pedido dela — um que
ninguém pediu para matar, chegando talvez minutos depois. Marca que sobrevive
ao alvo é armadilha, e há teste que trava isso
(`a_marca_nao_atravessa_para_o_pedido_seguinte`).

### 4.3 A lista honesta

**Cancelável** — o laço consulta a marca e aborta, e o arquivo fica intacto
porque nada foi escrito ou porque o que foi escrito está completo:

| operação | fase cancelável | granularidade |
|---|---|---|
| `checksum` | a soma da tabela inteira | por linha |
| `exportar` | a **leitura** da tabela | por linha |
| `varrer` | a leitura das linhas da página | por linha |
| `inserir_lote` / `importar` / `carga` | a **conversão** das linhas, antes de a primeira ir ao disco | por linha |

**NÃO cancelável** — a operação termina, e a resposta diz isso em vez de
prometer o contrário:

| o que | por quê |
|---|---|
| `inserir_lote`, da gravação em diante | grava slot, índice e diário por linha; parar no meio deixaria a tabela com metade do lote e o índice com a outra metade |
| `inserir`, `atualizar`, `excluir`, `restaurar` de uma linha | são uma unidade só; não há «meio» onde parar |
| `reindexar` | reconstrói o `.ndx` inteiro; abandonar deixa o índice pela metade |
| `backup`, `conferir_backup` | copiam árvore de arquivos e conferem SHA-256 |
| `verificar` | o laço é do motor, e não do servidor |
| `juntar`, `unir`, `pivotar` | ainda não têm ponto de cancelamento — ficam de fora até terem |
| o `fsync` | é do sistema operacional; ninguém o interrompe |
| a montagem do formato de saída do `exportar` | já está tudo em memória; parar aí só jogaria trabalho fora sem soltar a trava mais cedo |

### 4.4 O que a resposta diz

`telemetria_encerrar` devolve um de **quatro** estados, e a tela mostra o que
ele diz:

- `encerrando` — está **dentro** do laço e aborta na próxima unidade de
  trabalho. É a promessa forte, a única sem ressalva. A tela mostra
  «encerrando…» e, na volta seguinte, a bolha some;
- `marcada` — a operação **tem** ponto de cancelamento mas não está nele neste
  instante. A marca fica posta, mirando aquela mesma operação, e vale para o
  primeiro ponto seguro que vier; se ela já tiver passado do último, termina
  normalmente;
- `nao_cancelavel` — a operação não tem ponto de cancelamento nenhum e **vai
  terminar**. A tela diz isso e **não** diz que matou;
- `ociosa` — não havia operação em curso; a resposta aponta o `encerrar_sessao`,
  que derruba a **conexão** fechando o soquete.

**O `marcada` existe por causa de um erro que o navegador mostrou.** Havia só
três estados, e o meio faltava: uma soma de verificação parada **na fila da
trava** tem `cancelavel:false` — o laço ainda não começou —, então a resposta
dizia «vai terminar»… e a operação **abortou** um instante depois, porque a
marca estava posta quando o laço começou. A resposta estava pessimista, que é
o lado seguro de errar, mas errada.

A separação que conserta isso são duas perguntas diferentes:

- *esta operação tem onde parar?* (`tem_ponto`, da lista `OPS_CANCELAVEIS`) —
  decide se o **botão** aparece;
- *ela está nesse ponto agora?* (`cancelavel`) — decide o que a **resposta**
  promete.

O botão segue `tem_ponto`. Seguir `cancelavel` fazia o botão **sumir e voltar**
conforme a operação entrava e saía da fila da trava — e botão que some sozinho
é tão ruim quanto botão que não cumpre. **Botão que não cumpre o que promete é
pior que botão nenhum** — e quem sabe qual dos quatro casos vale *agora* é o
servidor, não a tela.

Há teste que deriva a lista `OPS_CANCELAVEIS` do **texto do fonte**: ele acha
toda função `op_*` que abre uma fase cancelável e exige que ela esteja na
lista. Sem isso, a próxima operação a ganhar um ponto de cancelamento nasceria
com o botão desabilitado — e ninguém descobriria por leitura, porque nada
quebra: o botão simplesmente não aparece.

### 4.5 Encerrar deixa rastro

Todo `telemetria_encerrar` vai para o log de acessos com quem mandou, o quê e
quando. Derrubar o trabalho de outra pessoa é ato de administração, e ato de
administração deixa rastro.

### 4.6 A prova

`crates/phxsql-server/tests/telemetria.rs`, pelo soquete, com duas conexões e o
arquivo conferido depois: a soma de 200.000 linhas é encerrada, o cliente
recebe `CANCELADO` (código 6001), e a tabela reaberta devolve **a mesma soma e a
mesma contagem** de antes, além de passar no `verificar`. É a parte que separa
cancelamento de estrago.

A mesma prova foi refeita **pelo soquete, na tabela de 2.865.000 linhas**, com
o binário de release:

```text
ANTES     checksum=532f48958bde3ef5 linhas=2865000 (11050 ms)
ALVO      dados:5 · checksum · cancelavel=True · tem_ponto=True · com_trava=True
ENCERRAR  estado=encerrando quem=root
CLIENTE   operacao encerrada por root apos 1 unidade(s) de trabalho em checksum
          (somando a tabela); o que ja estava gravado continua gravado…
DEPOIS    checksum=532f48958bde3ef5 linhas=2865000 (10672 ms)
VERIFICAR True
BOLHA     saiu da lista: True
```

**E a prova tem de partir de um estado bom conhecido.** Na primeira tentativa
o `verificar` do fim acusou `CRC invalido na pagina 94990 de clientes.ndx` — e
a tentação era escrever que o cancelamento tinha estragado o índice. Não tinha:
a soma de verificação é **somente leitura** e não encosta no `.ndx`, e o
`checksum` de antes e o de depois deram o mesmo número no mesmo `.reg`. A
página quebrada já estava lá, de um `pkill` no servidor **durante uma carga de
inserção**, enquanto páginas sujas do índice estavam em memória. Um
`reindexar` devolveu o índice, o `verificar` passou, e a prova refeita do zero
deu o que está acima. Servidor morto no meio não prova nada sobre integridade
de arquivo — nem a favor nem contra.

Tire o `a.siga(1)?` do laço do `op_checksum` e o teste falha assim:

```
a soma terminou como se nada tivesse acontecido -- o laco nao consultou a marca:
{"ok":true,"op":"checksum","resultado":{...,"linhas":200000,...,"ms":1986}}
```

---

## 5. O inventário das threads

Toda thread do `phxsqld` passa por `Telemetria::subir` ou por
`registrar_fio`, e **a finalidade é obrigatória na assinatura**. Não é
formalidade: thread sem dono declarado é thread que ninguém acompanha — quando
ela morre, o serviço que ela prestava deixa de existir e nada avisa. Exigir a
frase no momento de subir é o que impede a próxima nascer anônima, e é o tipo
de coisa que não dá para acrescentar depois, porque depois ninguém lembra para
que ela era.

O nome também vai para o sistema operacional (`thread::Builder::name`), então
`top -H` deixa de mostrar quinze linhas chamadas `phxsqld`.

### 5.1 De serviço — vivem enquanto o servidor vive

| thread | finalidade | quando existe |
|---|---|---|
| `aceitador-dados` | a thread **principal**: fica no `accept` da porta de dados e entrega cada conexão nova a uma thread de atendimento | sempre |
| `amostrador` | tira, de segundo em segundo, a amostra das séries | sempre |
| `relogio-gravacao` | fecha a janela de durabilidade quando ninguém grava — sem ela a última venda do dia ficaria sem `fsync` a noite inteira | `durabilidade: por_lote` |
| `ouvinte-web` | aceita as conexões da interface web e entrega cada pedido a uma thread própria; **só aceita, nunca atende** | `web.ligado` |
| `vigia-disco` | chama o `df` de tempos em tempos e avisa quando o espaço aperta | `alertas.ligado` |
| `relogio-jobs` | vê quais jobs venceram a hora e os executa, com o poder do usuário de cada um | há job ligado |
| `vigia-jobs` | avisa o job **parado** — ligado, hora vencida e sem relógio que o rode | `alertas.email.avisar_jobs` |
| `aviso-job` | entrega **um** e-mail de job que falhou e sai | por falha |
| `backup-agendado` | confere de minuto em minuto se chegou a hora do backup | `backup.agendado` |
| `replica-<origem>` | puxa os eventos do diário de **uma** origem e os aplica aqui | papel réplica, uma por origem |
| `pulso-<nó>` | manda o pulso para **um** nó do cluster e escuta o dele | com bloco `cluster` |
| `arbitro-cluster` | conta os pulsos, apura a maioria e promove quando o master para de responder | com bloco `cluster` |
| `replica-cluster` | puxa do master **corrente**, que muda a cada promoção | com bloco `cluster` |

### 5.2 Efêmeras — uma por conexão, uma por pedido

| família | thread | vida |
|---|---|---|
| `atendimento` | `dados-<porta>` | uma conexão da porta de dados, do login até o fim |
| `web` | `web-<porta>` | **um** pedido HTTP (`Connection: close`) |

O registro guarda as últimas e descarta as mortas quando passa de 512 — as de
serviço nunca saem, porque elas são a lista que interessa.

### 5.3 Três achados

**1. A thread da web nasce sem teto.** A porta de dados recusa acima de
`conexoes_max` e registra a recusa no log; a porta web não conta nada — o laço
`for conexao in ouvinte.incoming()` cria uma thread por pedido, sem limite.
Uma enxurrada de pedidos HTTP vira uma enxurrada de threads. O registro agora
ao menos as **mostra** (e o painel dá a contagem viva); o teto é decisão de
configuração e ficou de fora desta rodada de propósito — mexer no
`config.json` é território de outro agente.

**2. A moldura da página rola de lado a 430 px, e não é a telemetria.** Medido
no navegador: a página tem 608 px de largura numa janela de 430, e os
elementos que passam da borda são os `.menu`/`.titulo` da barra de menus do
topo — o painel da telemetria termina em 408 px, dentro da janela. A checagem
foi feita **antes** de abrir a tela (`a moldura SEM a telemetria já rola de
lado? SIM`), então a origem é a moldura. Fica registrado para quem cuida do
desenho global; mexer no CSS global não é desta rodada.

**3. A thread do aviso de job por e-mail não tinha dono.** Ela era um
`thread::spawn` solto: quem a disparava não guardava nada, e se o relé de
e-mail estivesse fora do ar ela ficava pendurada no timeout sem ninguém saber
que existia. Continua sendo disparar-e-esquecer — é o certo, porque quem
dispara pode ser a tela e ela não pode esperar o relé —, mas agora ela aparece
no registro com nome e finalidade, e dá para ver quantas estão penduradas.

---

## 6. O custo: o portão vem ANTES do trabalho

`Telemetria::ligada()` é um `AtomicBool` lido com `Relaxed`. **Todo** ponto de
captura começa por ele:

```rust
pub fn contar_espera(&self, micros: u64) {
    if !self.ligada() { return; }
    self.espera_us.fetch_add(micros, Ordering::Relaxed);
}
```

É a lição do Profiler, aplicada antes de custar 7%: lá o ponto de captura fazia
dois `Json::analisar` do corpo inteiro, três `String` e um mutex, e **só então**
perguntava se estava ligado.

O que cada ponto custa, ligado:

| ponto | frequência | custo |
|---|---|---|
| `travar_dados` | por operação | dois `Instant::now()` + duas trocas de estado |
| `contar_pedido` | por operação | um `fetch_add` |
| `entrar` (atividade) | por operação | um `lock` + um `get` + um `Arc::clone` |
| `siga` (cancelamento) | **por linha** | um `fetch_add` + um `load`, `Relaxed` |
| cache do `.ndx` | por arquivo fechado | três `fetch_add` |
| amostragem | 1×/s, thread própria | quatro leituras de `/proc` |

Os números medidos estão na secção seguinte.

---

## 7. Os números medidos

Máquina: contêiner Linux, `/dev/vda`, `phxsqld` em `--release`. Tabela
`loja.clientes` com **2.865.000 linhas**, três colunas, um índice.

Cada carga rodou **cinco vezes desligada e cinco ligada, alternadas**
(A/B/A/B), e o valor é a **mediana** — uma máquina compartilhada tem vizinho, e
comparar «tudo desligado, depois tudo ligado» mediria o vizinho junto.

| carga | desligada | ligada | custo |
|---|---:|---:|---:|
| `checksum` de 2.865.000 linhas — **o pior caso**: uma chamada a `siga()` por linha | 10.163,8 ms | 10.395,2 ms | **+2,28%** |
| `varrer` de 50.000 linhas | 330,6 ms | 327,8 ms | −0,87% |
| `inserir` de 2.000 linhas, uma a uma — uma trava por linha | 410,2 ms | 419,2 ms | +2,19% |
| `inserir_lote` de 5.000 linhas | 68,7 ms | 69,6 ms | +1,37% |

O `varrer` deu **negativo**, e isso não quer dizer que a telemetria acelera
nada: quer dizer que o custo dela, nessa carga, é menor do que a variação da
máquina entre duas corridas. É o resultado honesto de uma medição que não
consegue resolver o número — e não um ganho.

**Isolando o custo por PEDIDO**, com 2.000 pedidos por corrida e nove
corridas alternadas:

| operação | desligada | ligada | por pedido |
|---|---:|---:|---:|
| `ping` — só o pedido, sem trava e sem linha | 90,75 µs | 93,30 µs | **+2,55 µs** (+2,81%) |
| `inserir` — pedido mais trava de dados | 207,72 µs | 210,90 µs | **+3,17 µs** (+1,53%) |

**Tentei atribuir esses 2,5 µs e não consegui.** Uma variante do servidor sem
o `comecou_pedido`/`terminou_pedido` — quatro `String` e quatro travas por
pedido a menos — mediu +2,30 µs contra os +2,55 µs da versão inteira. A
diferença (0,25 µs) é menor que a deriva do próprio valor de referência entre
duas corridas, que andou de 90,75 para 93,88 µs no mesmo par de medições.
Então o que dá para afirmar é o total: **da ordem de 2,5 µs por pedido**, e
nada sobre como ele se reparte. Diagnóstico plausível não é diagnóstico
medido.

### O número que estava errado, e por quê

A primeira medição desta tabela disse **+29,94%** no `inserir` e **+8,05%** no
`checksum`. Os dois estavam errados, por dois motivos diferentes, e ambos
valem como lição:

* o **+29,94%** saiu de cinco corridas de 300 inserções cada — 87 ms de
  relógio por corrida. Com 2.000 inserções e nove corridas, o mesmo teste deu
  +2,19%. Amostra pequena não mede o efeito: mede o vizinho;
* o **+8,05%** saiu de uma corrida em que eu rodei `cargo test --workspace`
  **em paralelo** com a medição, na mesma máquina. Refeita com a máquina
  quieta, deu +2,28%.

Ficam registrados de propósito: são a razão pela qual a tabela acima diz
quantas repetições, e por que a medição roda sozinha.

### 7.1 O atraso de ponta a ponta

O evento medido é o **começo de uma soma de verificação** numa conexão própria
da porta de dados. O cliente imprime o instante exato em que manda o pedido; o
navegador é observado até a bolha daquela operação aparecer. Cinco voltas:

| volta | 1 | 2 | 3 | 4 | 5 |
|---|---:|---:|---:|---:|---:|
| atraso | 1.669 ms | 1.137 ms | 1.172 ms | 1.167 ms | 1.161 ms |

**Mediana 1.167 ms**, mínimo 1.137, máximo 1.669.

E ele é o que a soma das partes prevê: a tela pergunta de dois em dois
segundos, então um evento que chega em instante qualquer espera, em média,
**meio período — 1.000 ms** —, mais a ida e volta do pedido (6 ms medidos) e o
desenho. O máximo de 1.669 ms é a volta que caiu logo depois de uma pergunta.

Para a **lista de atividades** é só isso: ela é lida ao vivo do registro, sem
passar por amostragem. Para as **séries** some-se o período do amostrador, 1 s
— e é por isso que o instante da última amostra e a distância dele até agora
ficam escritos na barra do topo.

Encolher o período da tela encurta o atraso e custa uma pergunta a mais por
segundo; o `telemetria` não toma a trava de dados, então ele responde mesmo com
o servidor travado — foi medido em 6 a 15 ms de ida e volta com uma soma de
verificação segurando a trava.

---

## 8. As operações

| op | o que faz | permissão |
|---|---|---|
| `telemetria` | o retrato: séries, atividades, threads, cache, servidor | administrador |
| `telemetria_ligar` | liga a coleta | administrador |
| `telemetria_desligar` | desliga e **descarta** a série e as atividades | administrador |
| `telemetria_encerrar` | encerra a operação em curso de uma atividade | administrador |

### 8.1 O portão próprio — a lição do `juntar`/`unir`

O portão do `despachar` confere o campo `"tabela"` do pedido, e caiu numa
armadilha antes: `juntar` e `unir` não têm esse campo, e escapavam.

Aqui é a mesma armadilha com o sinal trocado. Nenhuma das quatro operações tem
`"tabela"` **nem** `"database"`, então o portão geral só consegue perguntar
«este usuário pode administrar a base vazia?» — e a resposta disso cai na regra
`"*"` ou no nível. Um usuário com `bases: {"*": {"administrar": true}}` e nível
de leitor **passa** por ali.

E a telemetria mostra, de todo mundo, o login, o IP, a operação e a **tabela**
em que ela mexe. Quem vê isso vê o movimento de bases sobre as quais não tem
direito nenhum. Então as quatro perguntam por dentro o que o portão geral não
consegue perguntar: **é administrador deste servidor?** (`Usuario::e_admin`).

Provado no servidor de teste, com um usuário `leitor` de nível leitor e
`bases: {"*": {"ler": true, "administrar": true}}`:

```
sessoes                ok=True
telemetria             ok=False  leitor nao e administrador deste servidor…
telemetria_encerrar    ok=False  leitor nao e administrador deste servidor…
telemetria_ligar       ok=False  leitor nao e administrador deste servidor…
```

O `sessoes` continua passando **de propósito**: ele já se comportava assim, e
apertá-lo aqui tiraria o direito de alguém sem ninguém ter pedido. Guarda nova
entra pedida, não imposta — e a operação nova é onde ela cabe.

Sem cadastro de usuários, quem entrou pelo token de serviço continua podendo, é
assim que toda operação de administração já funciona.

### 8.2 Senha não passa por aqui

A telemetria não mostra o corpo de pedido nenhum — só o **nome** da operação, o
banco e a tabela. Não há por onde uma senha entrar: nem no descritivo da bolha,
nem no log do encerramento, nem no registro de threads.

---

## 9. O erro `CANCELADO` (6001)

Família nova, `6000 · execucao`. Não é recusa de acesso (quem pediu podia, e a
operação já tinha começado), não é erro do dado, do esquema nem do disco: é o
único erro do PhxSql que descreve uma **decisão de quem administra**, tomada
depois de o trabalho começar. Quem integra precisa distingui-lo de uma falha,
porque não há nada para consertar.

`adianta_repetir` é **falso**: repetir desfaria em silêncio a decisão que
acabou de ser tomada.

---

## 10. A tela

Arquivo próprio — `ui/telemetria.js` e `ui/telemetria.css` —, servidos como o
`ui/diagrama-er.js` já era. No `index.html` entram três linhas: o ícone, a
entrada do menu e a cola que monta a folha.

O estilo é **escopado em `.tlm`** e desfaz explicitamente as duas regras
globais que mordem componente novo: `input{width:100%}` e
`label{text-transform:uppercase}` — a segunda escreveria «BLUMENAU» onde o dado
é «Blumenau», que é mentira sobre o dado.

Três cuidados que só aparecem exercitando:

1. **Nada pisca.** O laço não redesenha o painel: atualiza os elementos que já
   existem, um `<g>` por atividade, achado pelo id. Redesenhar tudo faria o
   clique do operador cair no vazio a cada dois segundos e o cartão aberto
   fechar sozinho.
2. **O empacotamento é uma espiral gulosa**, escrita à mão, sem biblioteca: a
   maior vai ao centro e cada seguinte anda pela espiral até achar lugar que
   não encoste em nenhuma. Determinístico — mesma entrada, mesmo desenho.
3. **Pausa e retomada.** O botão para o relógio sem fechar a tela, e a barra
   continua mostrando o instante da última amostra, para ninguém confundir
   pausa com servidor parado.

O módulo **não fala com o servidor**: recebe uma função `api(op, params)` de
quem o chama. Foi assim que ele pôde ser exercitado no navegador com um retrato
inventado, sem servidor nenhum — e é assim que ele continua podendo.

### 10.1 O que só apareceu exercitando

Cinco defeitos, e nenhum deles aparecia lendo o código. É a lição do vídeo,
outra vez.

1. **O painel inteiro ficava vermelho** sob stress do servidor — a cor parou de
   separar (§3.3).
2. **Oito bolhas do mesmo tamanho**: o peso contava a espera, então a vítima
   crescia junto com o culpado (§3.2).
3. **A bolha de quem olhava a tela era acusada** de segurar a trava, sem nunca
   ter pedido nenhuma (§3.3).
4. **«0,00% de acerto» no cache** com zero toque de página. Uma soma de
   verificação varre o `.reg` de ponta a ponta e não encosta no índice — a
   tela dizia que o cache estava falhando enquanto ele nem tinha sido chamado.
   Agora ela diz **«sem toque de página ainda»**. Número honesto sabe dizer
   «ainda não sei».
5. **O relógio não parava** quando a tela saía da página. `folha()` avisa o
   módulo quando alguém troca de ferramenta, mas nem toda troca passa por ela:
   `abrirAdmin` e `abrirTabela` substituem o `#painel` por conta. E `abrirAdmin`
   é **assíncrona** — sob carga pesada ela demora. Exercitando com uma consulta
   longa segurando a trava, um `abrirAdmin("painel")` disparado lá no login só
   terminou depois de a Telemetria já estar montada, e **sobrescreveu o painel
   dela**. O relógio continuava batendo contra uma tela que não existia mais,
   pedindo telemetria de dois em dois segundos.

   O conserto é o mesmo remédio que o monitor da máquina já usava: **sem alvo,
   o relógio para sozinho**. Uma linha no começo da volta.

### 10.2 Quando o servidor não responde

O que está desenhado **continua na tela** — apagar tudo perderia justamente o
retrato do instante em que ele caiu, que é o que alguém vai querer olhar. Mas
ele passa a ser declarado **velho**: a barra do topo troca por «sem resposta do
servidor», diz **de quando é** o que se está vendo e há quanto tempo, conta as
tentativas sem resposta, e o desenho apaga.

A opacidade não é o sinal — a frase é. Painel congelado é idêntico a painel
calmo, e essa confusão é exatamente o que não pode acontecer num monitor.

Pausar também congela a tela, e por isso ganhou marca **própria**: «pausado por
você». Congelado por vontade de alguém e congelado porque o servidor caiu
param de atualizar do mesmo jeito; só um dos dois é notícia.
