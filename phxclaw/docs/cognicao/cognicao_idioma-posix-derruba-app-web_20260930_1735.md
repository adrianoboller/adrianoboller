# O navegador do agente dizia que o idioma era «en-US@posix», e isso derrubava app web

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxclaw-browser/tests/navegador.rs`, teste
`idioma_da_pagina_e_bcp47_valido`: sem o `--lang` ele falha com «invalido: en-US@posix»;
com o `--lang`, passa. E o `tests/flutter.rs` do `phxclaw-ui-ir`, que antes nunca via o app
subir, passa (medido em 30/09).

## O que aconteceu

O build web do Flutter gerado carregava tudo (wasm, `main.dart.js`, fontes) e nunca criava
a vista. Nenhuma requisição foi bloqueada e nenhum erro apareceu, porque o navegador do
PhxClaw não captura o console.

## O que eu concluí primeiro, e estava errado

Cinco hipóteses, e cada uma morreu medida: lentidão (60 s sem subir), service worker
(sem ele, igual), servidor (com o do Python, igual), WebGL (disponível) e aba oculta
(visível, `requestAnimationFrame` rodando). No meio disso, um erro de instrumento: o
`--timeout` do Chromium **interrompe** o carregamento antes de gravar o DOM, então as
medições feitas com ele não valiam.

## O que a medição disse

Uma página na mesma origem instalou a captura de erros antes do bootstrap: `RangeError:
Incorrect locale information provided`. O `navigator.language` era `en-US@posix`, que o
Chromium deduz de `LANG=C`. `Intl.Locale` recusa essa etiqueta, e o Flutter lê o idioma
na largada.

## A regra

Quando a página não diz nada, **instale o ouvido antes de perguntar**: capturar o erro
na largada vale mais que cinco hipóteses sobre o motivo. E o navegador do agente declara
o idioma (`--lang`), porque ambiente de servidor não tem idioma de usuário.

## Como está guardado hoje

`idioma_do_sistema` no `phxclaw-browser` (BCP 47 do `LANG`, senão pt-BR) e os dois testes.
