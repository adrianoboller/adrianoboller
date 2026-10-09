# O rust-analyzer dobra até a linha da `}` — a reserva tem de dobrar igual

**Estado:** PENDENTE (a prova roda na árvore; falta o commit)

**Evidência:** `tests/desktop/ide_dobra.mjs`, checagem «a regiao do servidor vai ate a }»:
a função `longa` (linhas 3–14, `}` sozinha na 14) volta do `textDocument/foldingRange` com
`lineFoldingOnly` como `fim 14`. Lido no fonte dele (`to_proto::folding_range`): a linha final
só sai da região quando há texto depois do fechamento na mesma linha.

## O que aconteceu

A reserva por chaves (`dobras.rs`) nasceu deixando a linha da `}` sempre visível (fim 13). O
mesmo arquivo dobrava diferente conforme o servidor respondia ou não — trocar de fonte mudava
onde a dobra acabava.

## O que eu concluí primeiro, e estava errado

Que o rust-analyzer, como a dobra por indentação do VS Code, deixava a `}` de fora. Escrevi
isso no topo do módulo antes de medir. A medida deu 14.

## A regra

A reserva segue a convenção do servidor que ela substitui: região até a linha que fecha, menos
quando há texto depois do fechamento (`};`, `} else {`) — aí para na de cima, para não
esconder o `else`. Convenção de dobra se mede no servidor real, não se presume do editor.

## Como está guardado hoje

`crates/phxclaw-agent/src/dobras.rs` (`por_chaves`, com o motivo no topo do módulo) e o
unitário `chaves_dobram_ate_a_chave_que_fecha_menos_quando_ha_texto_depois`.
