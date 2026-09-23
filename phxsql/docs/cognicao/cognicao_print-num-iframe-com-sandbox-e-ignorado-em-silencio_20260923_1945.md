# Cognição: `window.print()` num `iframe` com `sandbox` é ignorado em silêncio — e o sinal direto é `beforeprint`, não o relógio

**Descoberta:** 23/09/2026, 19:45 UTC. Papel H, pedidos 326 e 327.

## 1. O que aconteceu

O botão «⤓ Baixar em PDF» da capa do dossiê chama `window.print()`
(`docs/dossie/dossie-phxsql-0.18.html`, manipulador do `#btBaixar`). O
`docs/dossie/LEIA-ME.md` afirmava desde 07/09/2026, em duas seções e em dois
comentários do próprio HTML, que *«a caixa de impressão é do navegador, então
ela abre»* — e por isso o pedido 327 ficou 16 dias com a causa errada
disponível e pronta para ser consertada no lugar errado.

O visualizador de artefatos hospeda a página num `iframe` com `sandbox`.
**Sem a palavra `allow-modals`, o Chromium IGNORA a chamada** — sem exceção,
sem retorno diferente, só uma linha no console:

```
Ignored call to 'print()'. The document is sandboxed, and the 'allow-modals'
keyword is not set.
```

O botão parecia funcionar e não fazia nada.

## 2. O que eu concluí primeiro, e estava errado

**Duas conclusões erradas, e a segunda é minha.**

A primeira é a desta casa, e o pedido 327 já a tinha nomeado: *«`<a download>`
não funciona, então use `window.print()`»*. O «então» nunca foi medido — o
aviso do serviço falava de **entregar arquivo**, e imprimir é outro mecanismo.
Diagnóstico plausível sobreviveu escrito porque o conserto *parecia* ter
funcionado.

A minha veio em seguida, e é a que a medição matou: **«chamada engolida volta
na hora, então cronometro o `print()` e comparo»**. Parecia óbvio — em um
quadro que imprime, o `print()` bloqueia até a caixa fechar. Medido em modo sem
cabeça, ele volta na hora nos **três** embrulhos: 1,1 ms sem sandbox, 0,5 ms
sem `allow-modals`, 1,1 ms com ele. O relógio **não separa os casos**, e um
detector baseado nele reprovaria a página até num navegador que imprime.

## 3. O que a medição disse

`docs/dossie/prova-do-botao-de-baixar.mjs`, Chromium sem cabeça, o dossiê real
dentro de um `iframe`, três embrulhos com a **única** diferença sendo
`allow-modals` (`allow-popups` entra nos dois com sandbox de propósito —
variável a mais numa medição comparativa é bancada que compara trabalho
diferente):

| embrulho | `beforeprint` | `print()` voltou em | console acusou |
|---|---|---|---|
| sem `sandbox` | **1** | 1,1 ms | não |
| `sandbox` SEM `allow-modals` | **0** | 0,5 ms | **sim** |
| `sandbox` COM `allow-modals` | **1** | 1,1 ms | não |

`beforeprint` (e `afterprint`) separa os três com um bit; o relógio não separa
nenhum. E o console não serve de detector: a página não lê o próprio console.

**E a varredura que impediu o conserto no lugar errado.** Nos **20** `.html` de
`docs/`: `<a download>` de verdade, `href="data:"`, `blob:` de verdade,
`URL.createObjectURL`, `downloads.save`, `saveAs`, `msSaveBlob` — **zero em
todos os 20**. Quatro páginas casam o padrão do varredor do serviço
(`dossie-phxsql-0.18.html` pelos comentários; `pedidos-311-350.html`,
`pedidos-001-190.html` e `pedidos-351-410.html` pela **prosa dos próprios
pedidos 326, 327 e 380**). **Nenhuma oferece arquivo.** Declarar a capacidade
`downloads` não consertaria nada — não há o que mediar.

## 4. A regra

**Quando a plataforma engole uma chamada em silêncio, procure o EVENTO que a
chamada bem-sucedida dispara — não cronometre a chamada.** E, o corolário do
alcance: **conserto que não mede o mecanismo conserta o mecanismo errado** —
aqui, o aviso falava de *download* e o defeito era de *modal*.

## 5. Como está guardado hoje

- **O detector está na página**: o `#btBaixar` registra `beforeprint`, chama
  `print()`, e 250 ms depois revela o aviso `#semCaixa` se o evento não veio.
  O botão **diz que não imprimiu** em vez de parecer que funcionou.
- **A prova é real nos dois sentidos** e está versionada:
  `node docs/dossie/prova-do-botao-de-baixar.mjs` — VERDE (saída 0) com o
  conserto, VERMELHO (saída 1, caso do meio) com o defeito reposto. Roda em
  segundos e não chama `cargo`.
- **Os dois comentários errados do HTML foram corrigidos**, e a seção do
  `LEIA-ME.md` também — comentário que se declara resolvido é o motivo de
  ninguém olhar de novo.
- **Onde o buraco ficou, e ele é conhecido:** não se mede daqui quais palavras
  de `sandbox` o visualizador concede de verdade. Por isso o conserto **não
  adivinha o embrulho** — pergunta ao navegador. Se o visualizador conceder
  `allow-modals`, o aviso nunca aparece e o conserto não custa nada a ninguém.
- **A prova não está em nenhuma catraca automática.** O `portao-dos-geradores.py`
  roda geradores e a prova do leitor de pedidos; esta prova é `.mjs`, sobe um
  Chromium e fica **fora** dele. Papel que não está cumprindo aparece como não
  cumprindo: quem mexer no `#btBaixar` tem de lembrar de rodá-la.
