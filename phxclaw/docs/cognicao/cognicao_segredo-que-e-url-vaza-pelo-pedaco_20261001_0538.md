# Segredo que é URL vaza pelo pedaço: o eco devolve só `chave=...`

**Estado:** PENDENTE (a evidência existe na árvore, falta o commit onde a prova roda)

**Evidência:** `crates/phxclaw-agent/tests/canais.rs::nenhum_provedor_devolve_segredo_ou_token_derivado_no_erro`,
RED antes do conserto (o Google Chat devolveu `chave=TOKEN-DE-CANAL-QUE-NAO-PODE-VAZAR` no erro) e
GREEN com `canais/http.rs::partes`, que passa a limpar cada valor da consulta quando o segredo é URL.
O mutante M10 (`partes(segredo)[..1]`, só a URL inteira) é a reposição do defeito.

## O que aconteceu

A saída do Google Chat é o webhook de entrada do espaço: a URL **inteira** é o segredo (`key` e
`token` na consulta), e ela mora no broker como um valor só. A `Credencial::com` tira o segredo de
todo erro pelo `scrub_text`, que troca o valor **exato**. O teste de eco — um servidor que devolve no
401 os cabeçalhos, a consulta e o corpo do pedido, rodado contra os 18 provedores HTTP num laço —
mostrou que o servidor não ecoa a URL, ecoa a **consulta**: `chave=<segredo>`. O valor exato nunca
aparece, e o pedaço passava.

## O que eu concluí primeiro, e estava errado

Que «todo erro sai da `Credencial::com`, então todo erro está limpo». Está limpo do **valor que o
broker guardou**; segredo composto (URL com credencial dentro) chega ao fio quebrado em partes, e o
que volta no erro é a parte. Os marcadores fixos do `scrub_text` (`token=`, `api_key=`) cobririam o
`token=` do Google por acaso — o teste usou `chave=` e mostrou que a cobertura era sorte do nome.

## O que a medição disse

1 de 18 provedores vazava no eco (Google Chat). Os outros 17, inclusive os 3 que derivam token
(Teams, Feishu, Reddit, unificados na `Credencial::com_derivado` no mesmo passo), já saíam limpos.

## A regra

Segredo que é URL se limpa pelo todo **e** por cada valor da consulta. E a prova de vazamento ecoa o
pedido **inteiro**, não só o cabeçalho de autorização: o que volta no erro é o que o outro lado viu.

## Como está guardado hoje

`canais/http.rs::limpar` (uma função para o segredo do broker e para o token derivado) chama
`partes`; o laço de eco reprova provedor HTTP novo que vaze, e conta os provedores (18) para o
próximo não ficar fora do laço calado.
