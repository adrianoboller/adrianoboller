# Cognição: trocar `prompt()`/`confirm()` nativo por diálogo próprio tira o auto-descarte do Playwright — e o `FORA` do passeio só olhava os MENUS, nunca a barra

## 1. O que aconteceu

O achado 4 da revisão de tela pedia para `restaurarLinha()`, `backupAgora()`
e `conferirBackup()` (`crates/phxsql-server/ui/index.html`) trocarem o
`prompt()` nativo do navegador por um diálogo com a cara da marca
(`perguntarTexto`, no molde do `dialogoExcluir`). Depois do conserto,
`node testes-web/bateria.mjs --caso passeio` — que clica **todos** os itens
dos nove menus e **todos** os botões da barra de ferramentas — passou a
FALHAR com um timeout genérico: `locator.click` esperando 30 s por um botão
qualquer da barra, coberto por `<div class="sobre">…</div> intercepts
pointer events`.

## 2. O que eu concluí primeiro, e estava errado

Sabia, pelo comentário que já existia no topo de `02-passeio.mjs`, que
diálogos NATIVOS (`confirm`/`prompt`) são descartados pelo Playwright por
padrão quando ninguém os escuta — e por isso o passeio sempre pôde clicar
em "Backup agora…" sem que nada acontecesse de verdade. Concluí, com essa
lembrança, que bastava acrescentar as três entradas ("Backup",
"Backup agora…", "Conferir um backup…") ao `FORA` — o `Map` que já existia
para excluir "Sair" e "Soltar esta tela numa janela" pelo mesmo motivo de
diálogo bloqueante — e o passeio voltaria a passar.

Estava incompleto: acrescentei as três ao `FORA`, rodei o `passeio`, e ele
continuou falhando NO MESMO PONTO. O `FORA.has(...)` só é consultado no
laço dos MENUS (`for (const menu of menus) { … if (FORA.has(item.rot))
continue; … }`) — o laço da BARRA DE FERRAMENTAS (`#ferramentas .fer`,
umas linhas abaixo) clica todo item sem checar o `FORA` nenhuma vez. "Backup"
é um item da barra (`FERRAMENTAS`, não `MENUS`), então continuava sendo
clicado, o meu diálogo abria, e ninguém o fechava — o próximo item da barra
batia nele.

## 3. O que a medição disse

Antes desta rodada, NENHUM item da barra de ferramentas abria um diálogo
que ficasse por cima da página depois do clique — então o laço da barra
nunca precisou de exclusão nenhuma, e a lacuna (checar `FORA` só nos menus)
nunca apareceu. `perguntarTexto` foi o primeiro item da barra a introduzir
esse comportamento. Medido depois do segundo conserto (`FORA.has(soRotulo)`
também no laço da barra): `passeio` volta a passar, com **114 telas
percorridas** (o mesmo número de antes — nenhuma cobertura real se perdeu,
só os três itens que já eram inertes por trás do `prompt()` descartado, e
que agora ficam de fora nomeados em vez de inertes calados).

## 4. A regra

**Um denylist de "diálogo bloqueante" que cobre só uma das formas de
navegação (menu OU barra) protege só metade da tela. Quando um componente
novo pode abrir um diálogo que fica por cima da página, ele precisa entrar
no `FORA` de TODO laço que clica coisas cegamente — não só do que o
motivou.**

## 5. Como está guardado hoje

`testes-web/casos/02-passeio.mjs` — as três entradas no `FORA`, com o
motivo escrito, e o `if (FORA.has(soRotulo)) continue;` acrescentado ao
laço da barra, com um comentário nomeando por que ele nunca precisou disso
antes. O que fica como alcance para quem ler isto depois: **se um dia a
barra de ferramentas ganhar OUTRO tipo de gesto cego** — um `dblclick`, um
`hover` que abre popover — esse gesto também vai precisar da própria
checagem de `FORA`, porque o denylist de hoje só cobre `click`. Este
arquivo não tenta prever esse caso; só nomeia que ele não está coberto.
