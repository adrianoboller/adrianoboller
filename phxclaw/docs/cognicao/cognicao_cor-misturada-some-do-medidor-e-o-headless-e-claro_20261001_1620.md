# A cor misturada sumiu do medidor, e o Chromium sem tema pedido já estava no claro

**Estado:** PENDENTE (a prova roda em `tests/desktop/qualificacao/qualificar.mjs`, sondas M1/M2 e
T1–T4 nos dois temas; promove com o commit que ligar o tema claro)

## O que aconteceu

Para ligar o tema claro, as 48 cores `rgba()` do `app.css`/`grades.css` viraram mistura de token
(`color-mix(in srgb,var(--painel) 85%,transparent)`). Duas coisas apareceram só rodando:

1. O `getComputedStyle` do Chromium devolve uma mistura como `color(srgb 0.039 0.067 0.133 / 0.85)`,
   não como `rgba(...)`. O `medir-pagina.js` só lia `rgba?\(` — a camada misturada **sumia** da
   conta do fundo, sem erro, e o contraste passava a ser medido contra o fundo errado. E o fundo
   de base estava cravado em `#010418`: no claro, todo contraste seria contra o escuro.
2. A primeira corrida do `qualificar.mjs` **antigo** (sem tema nenhum) saiu inteira no claro: o
   Chromium headless, sem `colorScheme` no contexto, responde `prefers-color-scheme: light`. O
   tema.js respeita o sistema — e o roteiro que «media o escuro» estava medindo o claro.

## O que eu concluí primeiro, e estava errado

Que trocar `rgba` por `color-mix` era só estilo: «a cor resolvida é a mesma». A cor é; a
**serialização** não é, e o medidor lia a serialização. E que um roteiro sem tema pedido mede o
tema padrão do produto — mede o do navegador que o roda.

## O que a medição disse

- `color-mix` → `color(srgb …)`: medido num `getComputedStyle` (Chromium do `/opt/pw-browsers`).
- Com o parser corrigido, a M2 do escuro acusou 4,42:1 no rodapé do splash — o brilho que era
  `rgba(19,73,101,.35)` (petróleo escuro) virara `--info` a 35% (azul claro). A troca de cor por
  token mudou a cor; o alfa voltou a 15%, que dá a mesma luminância.
- No claro: «ZERO TRUST» 4,49:1, «4» da hachura 4,24:1 e «0/4» 4,02:1 (o `--ambar` do PhxSql,
  `#a06a00`, dá 4,2:1 no papel; o `--aviso`, `#8a6a1f`, dá 4,37:1 sobre `--painel-2`).

## A regra

Medidor de cor lê **as duas** formas de cor computada (`rgb()` e `color(srgb …)`), e tira o fundo
de base da própria página. Roteiro de interface diz o tema que mede (`colorScheme` + a escolha
guardada), porque o padrão do navegador decide calado. Trocar uma cor literal por token é trocar
a cor: confere a luminância, não só o nome.
