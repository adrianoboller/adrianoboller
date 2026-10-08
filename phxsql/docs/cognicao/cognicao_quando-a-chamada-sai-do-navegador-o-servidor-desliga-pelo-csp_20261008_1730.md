# Quando a chamada sai do navegador, o servidor desliga pelo CSP — e «sessionStorage» não passa no ASVS

**Estado:** PENDENTE

## O que aconteceu

Pedido 339(a), refeito em 08/10/2026. O parecer externo pedia três coisas
para a chave da API da Claude: não persistir, desligamento administrativo e
aprovação do conteúdo enviado. Em 23/09 a chave tinha saído do `localStorage`
para o `sessionStorage`, e o pedido foi dado como fechado. Faltavam as outras
duas travas, e o repouso escolhido não passava na régua mais dura da OWASP.

## O que eu concluí primeiro, e estava errado

1. Que o `sessionStorage` era «a» resposta OWASP. A folha *HTML5 Security*
   aceita o `sessionStorage` só como o menos pior em relação ao
   `localStorage`. O *ASVS* 4.0.3, requisito 8.2.2 (nível 1), proíbe dado
   sensível em **qualquer** armazenamento do navegador. As duas fontes só
   concordam na memória.
2. Que o servidor não tinha como desligar uma integração cuja chamada nem
   passa por ele. Tem: a página é dele, e o `connect-src` do CSP também. Tirar
   a origem da Anthropic dali faz o **navegador** barrar o `fetch`, inclusive
   o de uma tela adulterada. O campo no `/saude` serve só para a tela explicar
   o porquê.

## O que a medição disse

`testes-web/prova-339-chave.mjs`: **12/12** verdes no binário novo, **9
vermelhos** no binário de `HEAD`. Os 2 verdes que sobram nos dois são os
controles. Com a integração desligada, um `fetch` direto feito de dentro da
página sai como «barrada», e a rota interceptada recebe 0 pedidos. No binário
velho o mesmo `fetch` sai, e a rota recebe 1.

## A regra

Quando a chamada sai do navegador, desligue pelo CSP da página que o servidor
serve, e não por um interruptor que só a tela lê. E segredo de terceiro na
tela fica em memória: o `sessionStorage` passa na folha e reprova no ASVS.

## Como está guardado hoje

- Rust: `crates/phxsql-server/src/http.rs::desligada_pelo_administrador_a_pagina_nao_alcanca_a_anthropic`
  e `crates/phxsql-server/src/config.rs::integracao_claude_e_lida_e_nasce_ligada`.
- Navegador: `testes-web/prova-339-chave.mjs`, que roda fora da bateria e por
  isso só pega quem a chamar. **Buraco:** ela não está no `bateria.mjs`. O caso
  `37-botoes-da-claude` cobre a aprovação e o repouso, mas não o servidor
  desligado.
