# Guarda de NOME de segredo pega o campo de uma regra, e só a tela mostrou

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxclaw-agent/tests/fluxo_cem.rs::fluxo_com_politica_passa_pelo_guarda_do_assistente`
serializa a `Politica` com toda regra ligada e passa o fluxo pelo guarda do assistente. Com o campo
de volta ao nome `credencial` (`#[serde(rename = "credencial")]`, `// REPOSTO`): o guarda recusa a
resposta inteira e o teste cai; com `credenciais`, passa. A irmã: `tests/desktop/textos_fora_da_fabrica.mjs`
deu 1 texto cravado («passo_1») com a caixa do assistente usando `.fp-campo`, e 0 com classe própria.

## O que aconteceu

O passo `politica` nasceu com `{"credencial": true}`. Os 11 testes em Rust passaram. Exercitando a
tela (`ui_fluxos_assistente.mjs`, servir real + Ollama falso), o assistente devolvia «nenhum fluxo
válido em 3 tentativas» para um fluxo que o motor aceitava: o assistente passa o fluxo inteiro por
`fluxos::variavel_parece_segredo`, que recusa qualquer CHAVE com nome de segredo — e `credencial`
está em `NOMES_DE_SEGREDO`. O valor era `true`.

## O que eu concluí primeiro, e estava errado

Que a resposta do modelo falso estava malformada (o servidor de roteiro devolvendo a resposta errada
para a 2ª chamada). O log das requisições mostrou três `POST /api/chat` com o roteiro certo: o
fluxo chegava, e era o guarda que o recusava. Os testes em Rust não pegaram porque o fluxo de teste
do assistente não tinha `politica`, e o da `politica` não passava pelo assistente — cada metade
provada sozinha.

## A regra

Campo novo num formato que uma guarda de NOME varre tem de passar pela guarda com o struct inteiro
serializado (todo campo ligado), não com um exemplo escrito à mão: o exemplo esquece o campo que
casa. E quem decide o nome do campo é a guarda compartilhada, não o contrário — afrouxá-la por um
campo seria mexer na guarda dos outros.

## Como está guardado hoje

`Politica::credenciais` (plural) com o motivo no cabeçalho do `fluxo_politica.rs`; o teste acima
serializa o struct, então campo novo com nome de segredo reprova sem ninguém lembrar dele. A caixa do
assistente tem classe própria (`.fluxos-descricao`), com o motivo no `fluxos.css`.
