# Recusar antes de ler o corpo vira «erro de rede» no navegador

- **Quando:** 2026-09-24, 02:46
- **Onde:** `docs/PHXZIP-WEB.md` §4, o teto de envio da web do PhxZip; medido
  com `testes-web/phxzip/servidor_falso.py` e o Chromium do Playwright

## O que aconteceu

O contrato manda conferir o teto no `Content-Length` **antes** de ler o corpo,
e responder `413 GRANDE_DEMAIS`. Um servidor que responde e fecha o soquete com
o corpo ainda chegando deixa dado não lido no buffer de recepção; o fechamento
vira RST, e o navegador — que ainda está **enviando** — perde a resposta.

## O que eu concluí primeiro, e estava errado

Escrevi no contrato que isso estava «medido no Chromium» **antes** de medir. A
medição veio depois e confirmou; mas por uns quinze minutos o documento citou
um número que ninguém tinha tirado. Número citado é número que não se mede — e
isso vale também para o número que eu *vou* medir.

## O que a medição disse

O mesmo `fetch` de 24 MiB contra um teto de 16 MiB, duas portas:

| servidor | o que a página recebeu |
|---|---|
| responde o `413` e **drena** o corpo antes de fechar | `HTTP 413 GRANDE_DEMAIS` |
| responde o `413` e fecha (`--nao-drenar`) | `TypeError: Failed to fetch` |

## A regra

Resposta antecipada só chega ao navegador se o servidor drenar o corpo antes de
fechar — até um teto de descarte, acima do qual fechar é o certo, e a tela diz
«a conexão caiu» em vez de fingir saber o motivo.

## Como está guardado hoje

No contrato (§4) e no caso `servidor-413-com-e-sem-dreno` do `exercitar.mjs`,
que reprova se o `413` com dreno não chegar. **Buraco:** o servidor de verdade
ainda não existe. Quem o escrever tem de reusar o `http.rs` do PhxSql (pedido
454), e ninguém mediu ainda o que **ele** faz com um corpo acima do teto.
