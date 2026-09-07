# Cognição: abrir uma tela pela barra deixava o RÓTULO certo e o ENDEREÇO velho

**Descoberta:** 07/09/2026, entre 17:15 e 18:00 UTC, exercitando o pedido 139
(abrir três telas na mesma região do multitela pelo caminho real — Ctrl+clique
na barra de ferramentas).

## 1. O que aconteceu

Ctrl-clicar "Telemetria" na barra deveria abrir uma aba nova, endereçável
(`chave:"telemetria"`), porque a Telemetria é uma das telas do `CATALOGO` do
multitela — pensada de propósito para reabrir por URL e sobreviver a um pino.
Uma sonda (`page.evaluate(() => PhxTelas._W.foco.chave)`) mediu o oposto:

```
apos ctrl+clique em Telemetria: { chave: 'painel', abas: 2,
  rotAbas: [ 'Telemetria', 'Telemetria' ] }
```

O RÓTULO da aba nova estava certo ("Telemetria"); o CHAVE por dentro ainda era
`"painel"` — herdado do `criarAba(r,"painel",{})` que `PhxTelas.novaAba()` faz
por baixo. Pinar essa aba pinava "Painel", em silêncio: a tela nunca mentia
sobre o que mostrava, só sobre o que ela ERA para o mecanismo de persistência.

A causa: `disparar()` (`index.html`) chamava `PhxTelas.novaAba()` — que pinta
"painel" de forma ASSÍNCRONA (`Promise.resolve().then(() => abrirAdmin("painel"))`)
— encadeado com `f.faz()` (`telaTelemetria`), sem esperar um pelo outro. Os
dois brigam pelo mesmo `Promise.resolve().then()`. Quando `folha()` (dentro de
`telaTelemetria`) tenta `marcar(null)` para desenderecar a aba nova — o
comportamento certo de uma "folha avulsa" sem `PhxTelas.abrir()` por trás —, a
guarda `W.abrindo > 0`, pensada só para o `PhxTelas.abrir()` não desfazer o
PRÓPRIO `chave` que acabou de pôr, ainda estava de pé por causa do
`abrirAdmin("painel")` pendente de `novaAba()`, e suprimia o `marcar(null)`
ALHEIO.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o defeito era só do Ctrl+clique na barra, e conserto só nos QUATRO
botões com par no catálogo (`fer_query`, `fer_telemetria`, `fer_profiler`,
`fer_diagrama` ganharam `tela:` e passaram a abrir por `PhxTelas.abrir(...)`).

Medindo o estado logo após o LOGIN — antes de eu tocar em qualquer botão —,
achei o mesmo defeito na aba PADRÃO, a que todo mundo abre com:

```
tab0 logo apos login: { chave: null, rot: 'Painel', guardado: null }
```

A causa raiz é a MESMA, num lugar diferente: `montarArvore()` termina clicando
sozinha no Painel com um `abrirAdmin("painel")` CRU (sem guarda nenhuma), e
`folha()` desendereça a aba. Isso sempre esteve assim — só nunca tinha
aparecido como defeito porque, ANTES desta rodada, ninguém tentava pinar o
Painel (ele nunca teve chave para começar, então pinar era sempre um no-op
silencioso). Meu primeiro conserto (só a barra) deixou a raiz maior intacta:
Alt+1, o item de menu "Painel" e a própria arvore (`irPara`, o clique em
`[data-admin]`) continuariam produzindo o mesmo Painel sem endereço.

E uma TERCEIRA camada, só visível depois de consertar a segunda: com o Painel
agora pinável de verdade, `restaurar()` (que roda a cada `reload`) tentava
recriar UMA aba "painel" pinada por cima da aba "painel" que `iniciar()` já
cria por padrão — porque `restaurar()` sempre pede `nova:true`, e o
desvio-de-duplicata do `abrir()` só age quando o achado NÃO é a aba com foco
(e a aba padrão, recém-criada, É o foco). Restaurar com o Painel pinado
sozinho devolvia `["Painel","Painel"]` em vez de `["Painel"]`.

Três hipóteses, cada uma medida ANTES de eu me convencer de que o conserto
anterior bastava — e as três nasceram da mesma raiz: código que pinta uma tela
do catálogo SEM passar por `PhxTelas.abrir()`.

## 3. O que a medição disse

- Antes do conserto: Ctrl+clique em Telemetria → `chave:'painel'` (deveria ser
  `'telemetria'`).
- Antes do conserto (2ª camada): login → `chave:null` na aba padrão (deveria
  ser `'painel'`).
- Antes do conserto (3ª camada): reload com só o Painel pinado →
  `["Painel","Painel"]` (deveria ser `["Painel"]`).
- Depois dos três consertos: os três casos batem com o catálogo, e a bateria
  inteira (26 casos, os dois temas) continua verde —
  `testes-web/casos/26-multitela-abas.mjs` prova as três telas endereçadas
  corretamente, o pino sobrevivendo ao fechar+recarregar.

## 4. A regra

**Toda tela do catálogo do multitela tem de nascer POR `PhxTelas.abrir()`,
mesmo quando quem a abre é a barra, o menu, a árvore ou o clique automático
do login — nunca por `abrirAdmin()`/`f.faz()` cru.** Um atalho que pinta a
tela certa mas chega por um caminho diferente do catálogo entrega o RÓTULO
certo e MENTE sobre o ENDEREÇO — e essa mentira só aparece no dia em que
alguém tenta pinar.

E o corolário do §2: **quando um conserto pontual funciona, pergunte se a
MESMA função (aqui, `abrirAdmin` cru dentro de um `Promise.resolve().then()`
sem `PhxTelas.abrir`) tem outro chamador** antes de dar a bateria por encerrada
— achar os outros DOIS chamadores exigiu medir de novo, não ler o primeiro
conserto com mais atenção.

## 5. Como está guardado hoje

- `crates/phxsql-server/ui/index.html`: `admGo(qual, novaAba)` — o ponto único
  que decide entre `PhxTelas.abrir()` (painel/usuários) e `abrirAdmin()` cru
  (acessos/bloqueios/idiomas, que não têm par no catálogo). Chamado por
  `montarArvore()` (clique automático do login), pelo clique em
  `[data-admin]` da árvore, e por `irPara()` (Alt+1, itens de menu).
  `montarFerramentas()`/`disparar()` ganhou o mesmo tratamento para as quatro
  ferramentas com `tela:` no próprio objeto `FERRAMENTAS`.
- `crates/phxsql-server/ui/multitela.js`, `restaurar()`: antes de pedir
  `abrir(...,{nova:true})`, confere se a região já tem uma aba com a MESMA
  chave e, se tiver, só pina — não clona.
- `testes-web/casos/26-multitela-abas.mjs`: prova as três camadas juntas —
  chave certo ao abrir pela barra, clique/teclado trocando, fechar, pinar e
  recarregar sem duplicar.
- **O que fica de fora, nomeado:** o mesmo padrão pode existir noutros pontos
  de entrada que este exercício não percorreu (por exemplo, algum atalho que
  chame `abrirTabela`/`abrirConsulta` fora do `PhxTelas.abrir`). Os quatro
  pontos medidos (barra, árvore, `irPara`, `restaurar`) estão consertados; um
  quinto não medido não está — e não está registrado como "provavelmente
  também está certo", porque não foi provado.
