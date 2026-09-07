# Cognição: Ctrl+Tab é atalho reservado do navegador, não da página

**Descoberta:** 07/09/2026 17:30 UTC, implementando o teclado do pedido 139
(trocar de aba dentro de uma região do multitela).

## 1. O que aconteceu

O pedido pedia, literalmente, «teclado (Ctrl+Tab se não conflitar)» para
trocar entre as abas de uma região. Antes de ligar `document.addEventListener`
a Ctrl+Tab, fui procurar se algum atalho parecido já existia na casa —
`crates/phxsql-server/ui/index.html`, `ligarLateral()` — e achei um comentário
que já respondia à pergunta sem eu precisar medir nada:

> «Ctrl+\ recolhe e reabre. Alt+letra já é do menu e Ctrl+B já é o backup;
> esta sobra, e nenhum navegador a usa.»

Ctrl+Tab e Ctrl+Shift+Tab são o atalho que **todo** navegador de mesa (Chrome,
Edge, Firefox, Safari) usa para trocar a **própria** aba do navegador. O
evento é interceptado na moldura do navegador, antes de chegar ao processo que
roda a página — não há `preventDefault()` que alcance isso, porque a página
nunca recebe o evento para começar. É o mesmo motivo, na mesma família, de
Ctrl+W (fechar aba) e Ctrl+N (nova janela) serem irrecuperáveis por JavaScript.

## 2. O que eu concluí primeiro, e estava errado

Nada — desta vez o erro NÃO foi meu. A pétrea de teste desta casa (F) diz
«prova real nos dois sentidos: o teste falha com o defeito e passa com o
conserto», e cheguei a cogitar simular isso com o Playwright: abrir duas
`page` no mesmo `context` (duas abas de verdade) e mandar Ctrl+Tab para uma
delas, esperando ver o Chromium trocar para a outra.

Não fiz — e o motivo é o achado que vale registrar: o Playwright injeta
teclado por CDP (`Input.dispatchKeyEvent`), que entrega o evento **direto no
processo de render** da página alvo, contornando exatamente a camada da
moldura do navegador que intercepta Ctrl+Tab num uso real. Uma prova
automatizada dessas mediria o Playwright, não o navegador — o mesmo defeito
de método já pago aqui com «medidor que mede a coisa errada é pior que
medidor que não roda» (`servidor.mjs`). A fonte confiável aqui não é uma
bancada: é o próprio contrato de teclado do navegador, documentado e estável
há mais de uma década, do mesmo jeito que «o navegador não vê o arrasto de uma
janela do sistema por cima de outra» já é aceito sem bancada em
`docs/MULTITELA.md`.

## 3. O que a medição disse

Não há número aqui — é a exceção que confirma a regra «número citado é número
que não se mede»: quando a resposta é uma garantia de PLATAFORMA (não um
comportamento desta casa), citar a garantia é medir; inventar uma bancada para
reprová-la seria medir o instrumento errado.

## 4. A regra

**Um atalho de teclado que troca de ABA nunca pode usar Ctrl+Tab,
Ctrl+Shift+Tab, Ctrl+PageUp/PageDown nem Alt+Tab — os quatro são reservados
pelo navegador ou pelo sistema operacional, nessa ordem de blindagem.** Antes
de ligar um atalho novo, procure primeiro se a casa já decidiu um caso
parecido (aqui, o comentário do Ctrl+\ já respondia tudo) em vez de descobrir
de novo por tentativa.

## 5. Como está guardado hoje

- `crates/phxsql-server/ui/multitela.js`, `cicloAba()`: Alt+→/Alt+← trocam de
  aba na região com foco, com o motivo escrito ao lado.
- `crates/phxsql-server/ui/index.html`, `ligarAtalhos()`: o par de teclas,
  comentado com a mesma referência ao Ctrl+\ que já existia.
- `docs/MULTITELA.md` e a tela «Sobre o modo multitela» (`tela.mt_nao_faz_ctrltab`
  / `_ctrltab2`, seis idiomas): dizem por que não é Ctrl+Tab, em vez de
  simplesmente não linkar o atalho pedido e deixar quem usa descobrir sozinho.
- **O que fica sem prova automatizada, e por quê:** que um navegador REAL, com
  mais de uma aba aberta, de fato ignora `preventDefault` em Ctrl+Tab. Isso é
  garantia de plataforma, não comportamento desta casa — não há bancada
  daqui que a meça sem medir o instrumento errado.
