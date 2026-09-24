# Fragmento em netns passa — até a regra ficar antes do *defrag*

## O que aconteceu

Item 6 das lacunas do phxvpn: o P2P pelo repasse e pelo farol punha 1.516 B
no fio (placa 1.420 + cabeçalho 16 + etiqueta 16 + embrulho 36 + UDP/IPv4 28).
A lista dizia «calculado, não medido». Medido em netns
(`phxvpn/provas/operacao/mtu.sh`).

## O que eu concluí primeiro, e estava errado

«Se fragmenta, o ping cheio falha.» Em netns ele **passa**: 5/5 pelo repasse
e pelo farol com a placa em 1.420 — o kernel fragmenta na saída e o NAT
(conntrack) remonta. O defeito só aparece quando o fragmento morre no
caminho, que é o que CGNAT e firewall de operadora fazem — e para simular
isso a regra `ip frag-off & 0x3fff != 0 drop` tem de ficar em `prerouting`
com prioridade **−450**, antes do *defrag* do conntrack (−400). Em `raw`
(−300) ela nunca vê fragmento: ele já virou pacote inteiro.

## O que a medição disse

`provas/operacao/resultados.json`, seções `mtu-antes` e `mtu-depois`, n=1:
com fragmento descartado, TCP pelo relé **0,0 Mbit/s** (placa 1.420) contra
**641,7** (repasse) e **46,4** (farol) com a placa em 1.384; fragmentos no
fio para 5 pings cheios 80/120 → 0/0. Custo no direto: faixas de três
corridas se cruzam (1.420: 636,5–843,6; 1.384: 742,3–849,9 Mbit/s).

## A regra

Para provar o que acontece quando o caminho descarta fragmento, descarte-o
antes do *defrag* (nft prerouting −450) — senão a prova passa por engano.

## Como está guardado hoje

`p2p::MTU` = 1.384 com o motivo, `teste:pacote_cheio_pelo_rele_cabe_no_fio`
(RED: 1.536 B com 1.420) e o roteiro `mtu.sh`, que mede os dois lados com
os dois binários. O buraco: a prova não roda IPv6 por fora (o kernel daqui
arranca com `ipv6.disable=1`); o 1.384 cobre o IPv6 pela conta, não pela
medida.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/operacao/resultados.json`, `phxvpn/provas/operacao/mtu.sh`, `teste:pacote_cheio_pelo_rele_cabe_no_fio`
- **Validado em:** 24/09/2026
