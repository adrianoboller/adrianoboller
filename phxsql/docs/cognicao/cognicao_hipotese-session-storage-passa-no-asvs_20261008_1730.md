# Hipótese morta: o `sessionStorage` é «a» resposta OWASP para a chave da API da Claude

**Estado:** INFRUTÍFERO

**Causa:** Foi lida uma só folha da OWASP (a comparativa) e não o requisito do ASVS; o «menos pior» foi tomado como «aceito».

**Prevenção:** Segredo de terceiro na tela fica em memória, nunca no armazenamento do navegador; a chamada que sai do navegador se desliga pelo CSP servido pelo servidor.

## 1. O que aconteceu

Pedido 339(a): em 23/09/2026 a chave saiu do `localStorage` para o `sessionStorage` e o pedido foi dado como fechado. Origem: `cognicao_quando-a-chamada-sai-do-navegador-o-servidor-desliga-pelo-csp_20261008_1730.md`.

## 2. O que eu concluí primeiro, e estava errado

Que o `sessionStorage` era a resposta que a OWASP aceita.

## 3. O que a medição disse

A folha *HTML5 Security* só o aceita como o menos pior em relação ao `localStorage`; o *ASVS* 4.0.3, requisito 8.2.2 (nível 1), proíbe dado sensível em qualquer armazenamento do navegador. As duas fontes só concordam na memória. Controle: `testes-web/prova-339-chave.mjs` deu 12/12 no binário novo e 9 vermelhos no de `HEAD`.

## 4. A regra

Ao escolher onde guardar segredo no navegador, leia o requisito do ASVS (nível 1) além da folha comparativa, e trate «menos pior» como reprovação.

## 5. Como está guardado hoje

`crates/phxsql-server/src/http.rs::desligada_pelo_administrador_a_pagina_nao_alcanca_a_anthropic` e `testes-web/prova-339-chave.mjs` (esta última ainda fora do `bateria.mjs`). O aprendizado vivo continua no arquivo do CSP, PENDENTE.
