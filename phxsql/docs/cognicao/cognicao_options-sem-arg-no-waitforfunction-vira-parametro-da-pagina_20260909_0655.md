# `waitForFunction(fn, {timeout})` não passa timeout nenhum — vira o argumento da função

**09/09/2026, 06:55** — descoberto porque uma prova dupla que devia estourar
em 6 segundos estava estourando em 30, não procurando bug de assinatura
nenhum.

## 1. O que aconteceu

A prova dupla do `connect-src 'self'` (Bateria 3) espera "Testar a chave"
**falhar** quando o defeito está reposto — o `fetch` é barrado pela CSP e o
`#iaRecado` nunca vira `bom`. O código:

```js
await pg.waitForFunction(() => {
  const el = document.querySelector('#iaRecado .aviso');
  return !!el && el.classList.contains('bom');
}, { timeout: 6000 });
```

O veredito saiu certo (a prova detectou a falha), mas **cada corrida da
Bateria 3 levava a mais 24 segundos que o previsto** — 30,5 s numa prova que
devia estourar em 6.

## 2. O que eu concluí primeiro, e estava errado

**Achei que era o mesmo achado do endereço de loopback** (o da outra
cognição desta rodada) — que a chamada estava de algum jeito demorando para
falhar de verdade. Um script de diagnóstico com `console`/`response`/
`requestfailed` escutados mostrou o oposto: a violação de CSP aparece no
console **em menos de 100 ms** depois do clique. O `fetch` falha rápido. O
problema não estava na página — estava em quem esperava por ela.

## 3. O que a medição disse

A assinatura de `page.waitForFunction` no Playwright é
`waitForFunction(pageFunction, arg, options)` — **três** parâmetros, não
dois. Quando a função de página não recebe argumento nenhum, ainda assim o
`arg` ocupa a segunda posição. `{ timeout: 6000 }` passado como segundo
argumento não é lido como opção: é lido como o `arg` que seria entregue
à função de página (que aqui o ignora, porque não declara parâmetro
nenhum) — e o `options` de verdade fica `undefined`, caindo no padrão do
Playwright, que é **30.000 ms**.

A prova: o mesmo `waitForFunction` com `undefined` explícito no meio —

```js
await pg.waitForFunction(fn, undefined, { timeout: 6000 });
```

— estourou em **6.527 ms**, batendo com o timeout pedido.

Achei a MESMA forma errada em quatro lugares meus (`claude-apoio.mjs` duas
vezes, `claude-bateria.mjs` outras duas) e, ao procurar o padrão no
diretório inteiro para conferir se era só meu, achei a **mesma forma** em
`testes-web/apoio.mjs:42` — a função `entrar()` que toda bateria deste
diretório usa para logar:

```js
await page.waitForFunction(() => typeof est === 'object' && est.demo === false,
  { timeout: 15000 });
```

Ou seja: **toda bateria de frontend desta casa** (a `bateria.mjs` geral, as
provas de idioma, os vídeos de demonstração) está, hoje, esperando o modo
demonstração cair com o timeout PADRÃO de 30 s, não os 15/20 s que o código
lê. Isto não quebra nada que já passa — só torna mais generoso um limite que
o autor pensava estar apertando — mas é exatamente o tipo de coisa que
morde no dia em que alguém precisar que a bateria falhe RÁPIDO (ex.: um
`phxsqld` que nunca vai subir, numa esteira de CI com timeout geral curto).

## 4. A regra

**`page.waitForFunction(fn, options)` com uma função sem parâmetro é sempre
suspeito — falta o `arg` do meio.** Se a função de página não recebe nada,
escreva `waitForFunction(fn, undefined, options)` explicitamente. O sintoma
não é um erro: é um timeout maior que o pedido, silencioso, que só aparece
medindo quanto tempo a prova realmente levou.

## 5. Como está guardado hoje

Corrigido nos quatro pontos que este código introduziu
(`testes-web/claude-apoio.mjs`, `testes-web/claude-bateria.mjs`), com
comentário no primeiro apontando a assinatura certa.

**O que NÃO foi corrigido, e o buraco fica nomeado:** `testes-web/apoio.mjs`
linha 42 (usada por `entrar()`, portanto por toda bateria deste diretório) e
as ocorrências equivalentes em `prova-idiomas.mjs`, `prova-idiomas-telas.mjs`,
`video-demonstracao.mjs`, `video-gestao.mjs` e `capturas-cluster.mjs` — todas
fora do escopo desta frente (o pedido 231 é sobre a integração com a Claude,
não sobre auditar as demais baterias), e `apoio.mjs` é compartilhado por
scripts que esta rodada não rodou nem validou de novo. Corrigi-lo aqui sem
rodar a bateria geral inteira depois seria trocar uma suposição por outra.
Fica registrado para quem pegar o pedido de auditar `testes-web/*.mjs` -- ou
para a próxima vez que uma bateria de frontend "demorar mais que devia".
