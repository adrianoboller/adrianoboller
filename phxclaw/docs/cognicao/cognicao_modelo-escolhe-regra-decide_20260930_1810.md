# Print de tela: o modelo só escolhe, a tela confirma e a regra decide

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxclaw-ui-ir/tests/imagem.rs`: sem a confirmação pelo OCR, o
teste do intruso falha; com ela, passa. `crates/phxclaw-agent/tests/visao.rs`
(`PHXCLAW_PROVA_VISAO=1`): o print do pedido vira o mestre/detalhe certo em 92 s (30/09).

## O que aconteceu

Três montagens foram medidas no mesmo print, com a resposta certa conhecida:

- moondream: 0 rótulos, e uma descrição inventada;
- OCR + qwen2.5:3b escrevendo o SQL: colunas inventadas (`nome_cliente`, `data_hora`);
- OCR + qwen2.5:3b escolhendo linhas: 4/8 e 2/6, com intrusos.

A que ficou: qwen2.5vl:3b, com cada rótulo confirmado contra uma linha do OCR, e nome,
tipo e obrigatório por regra fixa.

## O que eu concluí primeiro, e estava errado

Que o teste do agente provava a leitura. Ele passou com a coluna `selecione_produto`,
com as colunas da grade na tabela mestre e com o símbolo «R$» virando a coluna `r`.
O teste só cobrava o que devia **estar** no SQL, não o que **não** podia estar.

## O que a medição disse

A resposta crua do modelo explicou dois desses defeitos: ele listava a grade junto dos
campos, e degenerava repetindo «R$» até o teto de tokens, sem fechar o JSON (421 s). O
teto de 600 tokens e a leitura tolerante das aspas trouxeram o tempo para 92 s.

## A regra

Modelo na borda **escolhe**; o que ele escolhe só vale se estiver escrito na entrada; e
o que dá forma ao resultado é regra. Teste de extração cobra os intrusos, não só os
acertos.

## Como está guardado hoje

`confirmar`, `sem_itens` e `texto_de_exemplo` no `phxclaw_ui_ir::imagem`, com os testes
feitos das respostas reais medidas; o teste do agente cobra os quatro intrusos.
