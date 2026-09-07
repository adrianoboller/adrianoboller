# Cognição: clicar no CENTRO geométrico de uma aba pode acertar o pino, não o rótulo

**Descoberta:** 07/09/2026 18:00 UTC, escrevendo `testes-web/casos/26-multitela-abas.mjs`
(a prova de clique trocando de aba dentro de uma região).

## 1. O que aconteceu

`page.click('.tira .tira-aba[data-i="0"]')` — clicar na aba "Painel" para
trocar o foco para ela — não trocava o foco nenhuma vez, mesmo com
`{force:true}`. Instrumentando o clique de verdade (capturando `ev.target` no
próprio navegador):

```
mousedown target=tira-aba data-i=0
CLICK target=SPAN.tira-pino
```

O Playwright clica no CENTRO da caixa delimitadora do elemento pedido. O botão
`.tira-aba` é um `flex` com três filhos lado a lado — `.rot` (o texto), o pino
e o `×` — e para um rótulo CURTO como "Painel" (o mais curto dos três nesta
prova), a largura total do botão é pequena o bastante para o centro geométrico
cair dentro do ícone do pino, não do texto. `ligarTira()` trata o clique no
pino ANTES de tratar o clique na aba (`ev.target.closest("[data-pino]")` vem
primeiro), então o clique "virava" um toggle de pino em vez de uma troca de
foco — sem erro nenhum, sem exceção, só o resultado errado.

Um segundo problema apareceu ao pinar as três abas de uma vez com
`document.querySelectorAll(...).forEach(b => b.click())`: só a PRIMEIRA
pinava. `alternarPino()` chama `desenhar()`, que reescreve o `innerHTML` da
tira inteira — os elementos 2 e 3, capturados pelo `querySelectorAll` ANTES do
primeiro clique, ficam órfãos do documento assim que o primeiro clique
redesenha. Um `.click()` sintético num nó órfão não borbulha para o ouvinte
delegado na `.tira` (que já não é mais ancestral dele), e o clique morre em
silêncio.

## 2. O que eu concluí primeiro, e estava errado

Concluí, ao ver o foco não mudar, que o defeito estava no PRODUTO — que o
`ligarTira()` tinha uma condição de corrida ou um `stopPropagation` capturando
o clique errado. Cheguei a ler `ligarTira()` de novo procurando o culpado
antes de testar `PhxTelas.focar()` diretamente (que funcionou de primeira,
`{foco:'painel'}`). Só a comparação entre "a API funciona" e "o clique não
funciona" apontou para o CAMINHO do evento, não para a lógica — e só a
instrumentação do `ev.target` mostrou que o alvo real do clique nunca foi o
que eu supunha.

Nenhuma das duas coisas é bug do produto: um ser humano mira no TEXTO que lê,
não no centro matemático de uma caixa com padding e ícones — a mesma
distinção, em miniatura, de "rótulo se estiliza, dado nunca" (aqui: "o teste
mira onde a PESSOA miraria, não onde a geometria do botão calcula").

## 3. O que a medição disse

- Clique no centro do botão "Painel" (100,75px de largura): `ev.target` =
  `SPAN.tira-pino`, foco não muda.
- Clique em `.tira-aba[data-i="0"] .rot` (o texto): foco muda corretamente.
- `forEach` síncrono sobre três `.tira-pino` capturados de uma vez: 1 de 3
  pinado. Três `page.click()` separados, um a um: 3 de 3 pinados.

## 4. A regra

**Um teste que clica numa aba com controles internos (pino, fechar) tem de
mirar no SUB-ELEMENTO do rótulo, nunca na caixa inteira do botão** — e um
teste que precisa clicar em N controles que se redesenham a cada clique tem
de fazer N chamadas de clique SEPARADAS, nunca um `forEach` síncrono sobre uma
lista de nós capturada de antemão.

## 5. Como está guardado hoje

- `testes-web/casos/26-multitela-abas.mjs`: todo clique de troca de aba mira
  `.rot`; o pino de cada uma das três abas é clicado num laço com
  `await` entre as chamadas, com o comentário explicando por quê.
- **O que fica de fora:** este documento não vira pétrea nem regra do
  `apoio.mjs` — é um cuidado LOCAL de quem escreve um novo caso contra a tira
  de abas, não uma regra geral de teste desta casa (outros componentes sem
  ícones internos não têm esse risco).
