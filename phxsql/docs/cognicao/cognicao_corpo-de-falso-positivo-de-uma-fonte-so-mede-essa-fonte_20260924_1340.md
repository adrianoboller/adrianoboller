# Corpo de falso positivo tirado de uma fonte só mede essa fonte

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-sql/src/sintaxe.rs::comando_empilhado_nao_acusa_o_legitimo`; `crates/phxsql-sql/src/sintaxe.rs::comando_empilhado_acha_o_segundo_comando`; `bancada/seguranca/495/prova_215.py`. Validada pelo integrador, que não é o autor do conserto, em 24/09/2026, na árvore exata. Com o defeito ORIGINAL reposto, o teste do legítimo cai (`nao devia acusar`, `sintaxe.rs:2079`). Com a 1a versão do conserto, o do ataque cai (`devia acusar`, `sintaxe.rs:2035`). Com o conserto final, os dois passam (2/2).

A promoção a FRUTÍFERO fica para o integrador validar (pétrea de 24/09/2026:
PENDENTE não vira FRUTÍFERO sem evidência validada por quem não é o autor do
conserto). O que já foi medido nesta rodada, pedido 501 (fatia F0 do 495):

- 1a versão do conserto (`houve_ponto_e_virgula` só na posicao IMEDIATA
  depois do comando aceito) fechou os 3 textos legítimos, mas o integrador
  achou um furo por prova real: uma sobra que NÃO começa com `;` —
  `LIMIT 0, 2; DROP TABLE t`, `RETURNING x; DROP TABLE t` — escondia um `;`
  de verdade mais à frente, e a versão imediata devolvia `false`. Vermelho
  reproduzido dos dois lados: a 1a versão falha nos 3 ataques novos, e o
  defeito original (215, sem nenhum conserto) falha nos 3 legítimos.
- 2a versão (a de agora): varre `p.s[p.i..]` inteiro atrás do primeiro `;`,
  e confere se sobra símbolo NÃO-`;` depois dele — em vez de olhar só o
  símbolo na posição corrente. `cargo test -p phxsql-sql comando_empilhado`
  verde nas duas listas (legítimo e ataque, 7 textos de ataque agora,
  incluindo os 3 achados pelo integrador).
- `bancada/seguranca/495/prova_215.py`, pelo soquete, binário
  `target/release/phxsqld` recompilado: os 3 pedidos legítimos originais, 2
  rodadas (6 pedidos), `bloqueios: []` — antes bloqueava 127.0.0.1 por 60 min
  no primeiro. Refeito depois da 2a versão do conserto (mesmo resultado).

## O que aconteceu

O pedido 215 (contar injeção de SQL contra o IP) passou na prova de falso
positivo dele (5b), e nasceu desligado de fábrica. O papel J, no 495,
extraiu o SQL legítimo do repositório inteiro, 1.186 textos das crates, das
bancadas e da tela, e ligou o interruptor. O resultado está em
`bancada/seguranca/495/prova_215.py`: cinco pedidos legítimos pelo soquete
(`LIMIT 0, 2`, `RETURNING`, `EXCEPT`) **bloquearam 127.0.0.1 por 60 minutos**,
com o firewall disparado.

## O que eu concluí primeiro, e estava errado

Que o 5b tinha provado o falso positivo. Tinha provado só o corpo que usou.
Esse corpo não tinha nenhum comando com sobra depois do fim, e o
`comando_empilhado` não confere se houve `;`: ele acusa qualquer sobra que o
analisador não consome.

## O que a medição disse

- No corpo legítimo, o `comando_empilhado` acusa **17** textos, e **11** deles
  são um comando só.
- Um detector que analisa o léxico do motor acusa **1** texto em 1.186. Um que
  recorta o texto cru acusa **107**.

## A regra

O corpo de falso positivo sai do repositório inteiro, por um extrator (o
`extrair_legitimo.py`), e não de uma fonte escolhida à mão.

## Como está guardado hoje

No teste `comando_empilhado_nao_acusa_o_legitimo` (`sintaxe.rs`) e na prova
pelo soquete `prova_215.py`. O 215 não acusa mais o comando único com sobra.

## Um efeito colateral que a prova ensinou

`extrair_legitimo.py` dedupica por TEXTO, e varre Rust antes de Python: os
mesmos 3 textos que o pedido 501 pôs no teste Rust passaram a "roubar" a
origem deles em `legitimo.jsonl` (a linha vira `sintaxe.rs:2054`, não mais
`sondar.py:251`). `prova_215.py` filtrava por
`origem.startswith("bancada/gaps-sql/sondar.py:")`, e passou a achar zero
linhas — silenciosamente, sem erro. Corrigido pondo os 3 textos DIRETO no
script (sem depender do `legitimo.jsonl`), porque o que a prova precisa é do
TEXTO, não de onde o extrator disse que ele veio. **Corpo de teste que reusa
o texto exato de um teste unitário vira frágil a dedup de outro extrator** — é
o mesmo aprendizado desta cognição, por outro caminho: quem gera um corpo por
extração tem de saber que outra frente pode mudar o que o extrator acha.
