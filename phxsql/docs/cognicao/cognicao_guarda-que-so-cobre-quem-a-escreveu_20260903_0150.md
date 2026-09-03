# A guarda existia, tinha o comentário certo, e cobria só quem a escreveu

- **Quando:** 2026-09-03, 01:50
- **Onde:** `ui/index.html` — `abrirAdmin`, `folha`, `montarArvore`; e o caso
  `telemetria` da bateria de frontend
- **Custo:** a sprint SP000056 existia porque o caso `telemetria` reprovava em
  ~metade das rodadas e trocava de tema entre elas. Quatro diagnósticos
  errados em sequência, uma tarde, e a decisão do dono de **reescrever o
  módulo** em vez de continuar caçando. O módulo não tinha defeito nenhum.

## O que aconteceu

O caso `telemetria` estourava com `page.click: Timeout` em
`.tlm-threads summary`. O `clicarOuExplicar`, que a própria sprint tinha
entregado, disse o que «timeout» nunca diria: **`achou: false`** — o elemento
não estava coberto por outro nem parado nem desabilitado; **não existia**.

E não existir era impossível de ler no código: o `<details class="tlm-threads">`
e o `#tlmThreads` saem do **mesmo** template literal, então um sem o outro não
se explica desenhando. A explicação só apareceu com um `MutationObserver`
plantado antes do `DOMContentLoaded`:

```
NASCEU .tlm   em #painel
SUMIU  .tlm   em #painel     ← 37 ms depois (e 104 ms na outra reprovação)
```

E o `#painel` ficava com `<div class="kpis">…bancos…registros…` — o corpo do
**Painel** — enquanto o `#titulo` continuava dizendo **«Telemetria»**.

A causa: `montarArvore()` termina disparando o clique no nó Painel, e esse
clique roda `Promise.resolve(abrirAdmin("painel"))` que **ninguém segura**.
`abrirApp()` devolve, a árvore aparece, e o `abrirAdmin` ainda está no
`await vPainel()`. Quando ele volta, escreve `p.innerHTML` por cima de quem
tiver chegado no meio-tempo.

O desconfortante é que a guarda para isso **já existia**, com um comentário
de vinte linhas explicando o defeito exato — «título de uma tela e corpo da
outra» — e admitindo com honestidade rara que **não tinha prova real**. Ela
não tinha prova real porque não cobria o caso que descrevia: o contador
`admGeracao` era privado do `abrirAdmin`, então defendia `abrirAdmin` de
`abrirAdmin` e de mais ninguém. Toda tela que pinta por `folha()` — a
telemetria, o profiler, o backup, as **Configurações**, umas cinquenta —
passava por fora da catraca.

O `TESTES.md` §9.8 já tinha registrado o estrago com outra vítima («título de
Configurações, corpo do Painel»), e o §5.6 o deixou em «anotado, e não
consertado». As Configurações pintam por `folha()`. **A guarda que entrou
depois nunca fechou o item que a motivou.**

## O que eu concluí primeiro, e estava errado

Três coisas, e as três eram plausíveis:

1. **«É o tema.»** A sprint descrevia «troca de tema entre as rodadas», e eu
   fui atrás de estado que sobrevivesse entre casos — `localStorage`, o
   `lembrar: "tlmthreads"` da grade, um relógio que não morre. **Errado:** o
   tema não tem nada com isso. As reprovações se distribuíam por escuro e
   claro porque o que sorteava era a viagem do `page.evaluate`, e o tema é só
   quem estava na vez.
2. **«É o gestor de threads»**, que era o que a sprint mandava reescrever. Ele
   é um painel vivo montado sobre `phx-grid`, com duas armadilhas já pagas
   (grade em `display:none` mede largura zero; com `fonte`, ordenar é da
   fonte). **Errado:** o módulo nunca aparece na falha. Reescrevê-lo teria
   custado uma frente inteira e comprado zero.
3. **«É o `folha()` de outra tela chegando por cima.»** Certo no destino,
   errado no mecanismo — e eu instrumentei o `folha` primeiro, gastando duas
   sondas. O log saiu com **uma única** chamada, a da telemetria. Quem
   escrevia não era o `folha`: era o `p.innerHTML` do `abrirAdmin`, que não
   passa por lá e por isso não repõe o `#titulo` — e é daí que sai a assinatura
   do defeito, título de uma tela e corpo da outra.

## O que a medição disse

| | antes | depois |
|---|---:|---:|
| caso `telemetria`, execuções isoladas | **4 de 40** reprovaram | **0 de 60** |
| caso `telemetria`, com a máquina carregada | **5 de 14** | **0 de 24** (12 rodadas) |
| bateria inteira, dois temas | — | **36/36**, e 3 rodadas seguidas |

A janela da corrida, medida em 12 logins: **32 ms de mediana** (min 29, máx
35) entre o `#arvore .no` aparecer — que é o sinal por onde a bateria dizia
«entrei» — e o Painel pintar. A viagem do `page.evaluate` seguinte cai dentro
ou fora dessa janela conforme o humor da máquina, e é isso, e nada mais, que
decidia se o caso passava. Com carga a janela cresce, e por isso a taxa subiu
de 10% para 36% quando havia outra frente compilando ao lado.

O ganho de 0,9^60 = **0,18%** é a probabilidade de ver 60 execuções limpas
seguidas se a taxa de 10% tivesse continuado. Não é prova; é o que sobra
depois de a prova real existir.

## A regra

**Guarda nova nasce com a pergunta «quem NÃO passa por aqui?».** O contador do
painel estava certo e era barato, e defendia um caminho de dois. A pergunta
que faltou é a mesma que esta casa já escreveu para o portão de permissão —
*«quando o portão passar a olhar um campo novo, procure quem não tem esse
campo»* — e ela vale igual para um portão que olha o caminho certo e ignora os
outros quarenta e nove.

E o corolário, que é o que fecha o ciclo: **guarda sem prova real não é guarda,
é intenção.** O comentário dizia, com todas as letras, que a sonda passava com
o defeito reposto. Isso não era um detalhe a resolver depois — era o aviso de
que a guarda não alcançava o que dizia alcançar, e ele ficou escrito por meses
sem ninguém ler assim.

## Como está guardado hoje

- A catraca virou do **painel** e não do `abrirAdmin`: `tomarPainel()` /
  `aindaNoPainel()`. Quem pinta toma a posse; quem pinta depois de um `await`
  confere se ainda a tem. **`folha()` toma no lugar das ~50 telas que passam
  por ela** — uma linha, e não uma conferência espalhada por cinquenta
  funções, onde a esquecida viraria a porta dos fundos.
- `desenharAba()` ganhou a mesma posse, porque as cinco abas também escrevem
  o painel depois de um `await`.
- `montarArvore()` **espera** a primeira tela pintar em vez de disparar um
  clique e ir embora, e `abrirApp()` marca `#app[data-pronto="1"]` quando
  acaba de verdade. O `entrar()` da bateria espera por essa marca: ninguém
  clica no menu 30 ms depois de a tela abrir, e o teste deixou de medir uma
  corrida que a pessoa não corre.
- **A prova real:** `testes-web/casos/18-tela-atropelada.mjs`. Ela não torce
  por *timing* — **segura a resposta da op `painel` no fio** até a segunda
  tela estar pintada, e só então solta. Com o `tomarPainel()` do `folha`
  comentado ela **reprova nos dois temas**, dizendo `titulo="Telemetria",
  kpis do Painel no corpo=true`. Cobre as duas vítimas: a telemetria e as
  **Configurações**, que é a do §9.8 do `TESTES.md`.
- O que **não** está guardado, e fica escrito: uma tela que faz
  `await api(...)` e **só então** chama `folha()` — o profiler é uma — pinta
  por cima de quem chegou no meio-tempo. Título e corpo saem coerentes, então
  não é a mesma mentira; é a tela que você pediu chegando atrasada e ganhando
  de quem você pediu depois. Sem prova real e sem guarda. Está no
  `PENDENCIAS.md`.
