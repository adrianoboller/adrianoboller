# Componente de fora fala português cru, e a régua vê o que está escondido

**Estado:** PENDENTE (a prova roda na árvore; falta o commit onde ela roda)

**Evidência:** `tests/desktop/textos_fora_da_fabrica.mjs` (35 no teto 35, com quatro estados novos de
grade e quatro da Configuração), `tests/desktop/ui_navegacao.mjs` (58/58) e `tests/desktop/ui_config.mjs`
(21/21). Prova contrária: 30 mutantes, cada um desfazendo uma mudança numa cópia da UI; os 30
reprovam na checagem que os vigia.

## O que aconteceu

As listagens do Command Center viraram grade com o phx-grid 0.78.1 (do próprio dono, JS puro). Ele
traz uma fábrica de idiomas (`definirTextos`) com **15** chaves — e escreve cru, no fonte, todo o
resto: o menu de filtro («Classificar de A a Z», «(Selecionar Tudo)», «OK», «Cancelar»), os títulos
(«desagrupar», «filtrar», «alternar agregador»), o «Buscar…» da linha de filtro, o «SUM» do rodapé, as
zonas do cubo e a frase «110 linhas em 1 nível de grupo». Ligada a grade, a catraca foi de 35 para
**49**.

## O que eu concluí primeiro, e estava errado

Que esconder com CSS bastava (`.phx-mostrando{display:none}`). Não basta: o coletor da catraca anda o
DOM, não a tela, e a frase escondida continuava lá — em português, com os números dentro. O certo foi
**esvaziá-la** (a contagem certa já está no canto do rodapé, pela fábrica). E que «Σ» era ícone: para a
régua, `Σ` é **letra** (grega maiúscula); o símbolo de somatório é `∑` (U+2211), que não é.

## O que a medição disse

- Texto cru do componente se resolve pela **classe** do elemento (`LOCAIS` em `assets/grades.js`),
  nunca comparando a frase: 35 entradas, reaplicadas por um `MutationObserver` a cada render (o
  phx-grid refaz o DOM por `innerHTML`). 49 → 35, com a régua medindo **mais** estados.
- Rótulo e dado no mesmo nó de texto enganam a régua: «Produto: OpenClaw» virava «: OpenClaw» depois
  de tirar a chave. O valor do grupo passou a ir num `<span>` próprio (`formatoGrupo`).
- A paginação do phx-grid conta a **linha de grupo como registro**: 110 papéis em 11 grupos davam
  «Página 1 de 5 (121 registros)». Nas listas que cabem numa página, a paginação some; o total certo
  fica no rodapé. Defeito do componente, para o dono.
- Sob a CSP do Tauri (`style-src 'self'`), o phx-grid perde **37** `style=""` (recuo de grupo aninhado,
  recuo de linha do cubo) e 2 `<style>`: cosmético, medido por `securitypolicyviolation`; script
  bloqueado, zero.
- Exercitando, dois defeitos de encaixe que ler o código não mostrava: sem a paginação, o seletor de
  colunas ficou sozinho à esquerda e o menu dele (que abre para a esquerda) saiu **cortado** pela borda
  da grade; e a coluna mínima de 360px da tela de Tarefas empurrou o detalhe para fora da tela do
  celular (o `pwa_ponte.mjs` pegou num Pixel 7).

- Na primeira rodada, 6 dos 30 mutantes só derrubavam o roteiro (clique que espera 30 s,
  `undefined.slice`): placar vermelho pela exceção, não pela checagem — reescritas para reprovar no
  ponto (download com prazo, clique com prazo, campo ausente vira falso). E 1 **passou**: «voltar ao
  padrão» mandando o valor do padrão em vez de `null` é invisível numa chave cujo padrão é `null`; a
  prova passou a usar também uma chave de padrão `true`.

## A regra

Componente de fora entra pela fábrica em dois degraus: o que ele já lê por chave (`definirTextos`) e o
resto **pela estrutura** (classe/atributo → chave). A régua conta o DOM, não a tela: o que não se
mostra, esvazia-se — não se esconde.

## Como está guardado hoje

`assets/grades.js` (`textosDoPhxGrid`, `LOCAIS`, `vigiar`); a catraca visita menu de filtro aberto,
seletor de colunas, agrupamento por estado, o cubo e a Configuração carregada, com diff, 409 e 422.
