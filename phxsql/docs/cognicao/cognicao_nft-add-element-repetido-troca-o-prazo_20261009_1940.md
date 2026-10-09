# `nft add element` repetido troca o prazo do elemento

**Estado:** PENDENTE

## O que aconteceu

Pedido 766, P10/P11: o bloqueio escalonado volta ao firewall com o prazo novo
pelo mesmo `bloquear` (`add element … { {ip} timeout {segundos}s }`). Ao
documentar o exemplo do conjunto com prazo no kernel (`docs/SEGURANCA.md` §5),
a frase precisava dizer o que acontece com o IP que já está no conjunto.

## O que eu concluí primeiro, e estava errado

Escrevi, sem medir, que o `add` de um elemento que já existe não troca o prazo
dele, e que quem quisesse o prazo novo no kernel teria de pôr `delete` antes.

## O que a medição disse

Num namespace de rede próprio (`unshare --net`), nftables 1.0.9, kernel 6.18:
`add element … { 203.0.113.9 timeout 60s }` e depois o mesmo com `600s` → o
segundo sai com código 0 e o conjunto mostra `timeout 10m expires 9m59s996ms`.
O prazo foi trocado.

## A regra

Comportamento de programa de fora se mede no programa, na versão que roda,
antes de virar frase de documento.

## Como está guardado hoje

Na frase do `docs/SEGURANCA.md` §5, com a versão medida e o par de comandos
para conferir em outra, e no
`blacklist::tests::o_nft_de_verdade_recusa_a_injecao_e_reconcilia`, que agora
faz o segundo `add` (120 s e depois 600 s) e confere `timeout 10m` no conjunto.
**O limite:** a prova roda com o `nft` desta máquina; outra versão do nftables
só se confere rodando o mesmo teste nela (ele se declara NÃO MEDIDO sem `nft`
ou sem `unshare --net`).
