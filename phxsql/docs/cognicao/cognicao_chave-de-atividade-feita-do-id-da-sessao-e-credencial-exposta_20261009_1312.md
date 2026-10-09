# A chave de uma atividade feita do id da sessão é a credencial exposta em toda tela que a mostra

**Estado:** PENDENTE

## O que aconteceu

Exercitando a F9 do pedido 495 (`testes-web/prova-495-f9-ocorrencias.mjs`), a
ocorrência que nasceu de um `inserir` pela tela gravou
`"tarefa":"web:3f37…0bbf"` — 48 dígitos hex. Era o **id da sessão web**
(`http.rs`, `Sessoes::nova`: 24 bytes aleatórios), o mesmo valor que a página
manda no cabeçalho `X-Sessao` e que, sozinho, autentica cada clique.

A chave da atividade (`servico_web_01.rs`, `format!("web:{id_sessao}")`) não
mora só no `ocorrencias.log`: sai como `id` no `telemetria`, como `tarefa` no
`aquario_retrato` — que quem só tem `monitorar` (a TV) lê — e no `aquario.log`.

## O que eu concluí primeiro, e estava errado

Que era um campo a mais na ocorrência, e que bastava tirar o `tarefa` do corpo
que sobe à Claude (já estava fora pela lista de permissão). O corpo estava
limpo; o vazamento era **anterior** a ele, na chave da atividade, e alcançava
três telas que nada tinham a ver com a F9.

## O que a medição disse

No binário de antes do conserto, o teste
`o_id_da_sessao_nao_sai_na_ocorrencia_no_aquario_nem_na_telemetria` da prova
saiu VERMELHO nos três lugares: «na ocorrencia: true; no aquario_retrato: true;
na telemetria: true». Com a chave virando `web:` + 8 bytes do SHA-256 do id
(`resumo_da_sessao`), 17/17 verdes, e a ocorrência grava `web:51824b3238a5d846`.

## A regra

Identificador que se mostra não pode ser a credencial: quando a chave de
agrupamento precisa ser uma por sessão, use o resumo dela, nunca o id.

## Como está guardado hoje

Pela prova do navegador (`testes-web/prova-495-f9-ocorrencias.mjs`), que roda à
mão. **Não há teste em Rust** que trave a chave da atividade web: o buraco
fica aqui escrito. E o resumo de 8 bytes não protege sessão que já vazou antes
do conserto — essas morrem no próximo login ou na expiração.
