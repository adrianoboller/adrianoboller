# A latência de chegada misturada com a janela da queda

**Estado:** PENDENTE

## O que aconteceu

Pedido 678, `bancada/caixa-offline/medir.py`, no ensaio de 20 caixas com 10 s
por fase (08/10/2026, 08:16). A chegada ao central das vendas cometidas
**antes** da queda saiu com mediana de **613 ms** e p95 de **10.719 ms**.

## O que eu concluí primeiro, e estava errado

Que o central se atrasava com 20 origens chegando juntas: a trava global
segurando a aplicação e a fila crescendo. Era a hipótese que o próprio parecer
do 325 (lacuna L1) deixava pronta para ser confirmada.

## O que a medição disse

O p95 vinha de outra coisa. As vendas cometidas no último segundo antes do
`SIGKILL` ainda não tinham sido puxadas, e só chegaram depois do religar, ~20 s
depois. Elas entravam no «antes» pela fase do commit. Separadas pela hora da
chegada (chegou depois da morte → `antes_pega_pela_queda`), nas 5 voltas:

- o p95 do «antes» ficou em 1.015–1.025 ms;
- as pegas pela queda foram 32–48 por volta, todas entre 20,2 e 22,6 s.

A trava aparece em outro lugar: no `varrer` do central, que vai de 1–3 ms
(ocioso) para 251–595 ms no alcance.

## A regra

Latência de quem atravessa uma queda se classifica pela hora da **chegada**,
não só pela hora da origem. A janela da queda é um número à parte.

## Como está guardado hoje

No `medir.py` (`t_morte` e a fase `antes_pega_pela_queda`), com o motivo no
comentário. Nenhuma guarda impede outra bancada de queda de repetir a mistura.
