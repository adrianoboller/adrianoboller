# O `ifconfig-ipv6` que o manual manda empurrar derruba o cliente sem IPv6

## O que aconteceu

Frente do túnel total do phxvpn (modo servidor). O manual do OpenVPN 2.6.19
(vpn-network-options.rst:12-45) dá a receita contra o vazamento de IPv6 de
quem só tem túnel IPv4: empurrar `ifconfig-ipv6` (um endereço qualquer),
`redirect-gateway ipv6` e `block-ipv6`. O IPv6 entra no túnel e morre ali com
«sem rota», em vez de sair pela placa de fora.

## O que eu concluí primeiro, e estava errado

1. **«O pacote do manual é inofensivo para quem não tem IPv6: sem IPv6, nada
   acontece.»** Parecia o caso neutro — quem não tem IPv6 não tem o que
   vazar, então as linhas seriam ignoradas.
2. **«Dá para empurrar só `redirect-gateway ipv6` + `block-ipv6`, sem o
   `ifconfig-ipv6`.»** O `block-ipv6` só age no pacote que CHEGA à placa
   (forward.c:1692); sem IPv6 na placa as rotas IPv6 não se instalam — e aí
   o IPv6 vaza em quem tem IPv6, que é justamente quem importa.

## O que a medição disse

Laboratório em netns, openvpn 2.6.19, kernel com `ipv6.disable=1` (o deste
contêiner), servidor empurrando o pacote:

| Perfil do cliente | Resultado |
|---|---|
| sem nada | «Linux can't add IPv6 to interface tun0», «Exiting due to fatal error» (tun.c:1126, `M_FATAL`) — **o cliente morre** |
| com `pull-filter ignore "ifconfig-ipv6"` | «Pushed option removed by filter», as rotas IPv6 falham («ERROR: Linux route add command failed», não fatal) e **conecta** |
| `block-outside-dns` empurrado a um Linux | «Options error: Unrecognized option … block-outside-dns» e **segue** (no perfil seria fatal) |

`phxvpn/provas/tunel-total/resultados.json` → `ipv6_kernel_sem_ipv6` e
`com_tunel_total.log_*`, nos motores nft e iptables.

## A regra

Opção empurrada que CONFIGURA a placa (endereço, não rota) é fatal no cliente
que não a suporta — antes de empurrar uma receita do manual, prove-a num
cliente sem o recurso; e dê ao cliente o jeito de recusar pelo próprio
perfil (`pull-filter`), porque o servidor não tem `push` condicional.

## Como está guardado hoje

- `phxvpn/src/saida.rs`: o túnel total empurra o pacote inteiro
  (`teste:tunel_total_leva_o_pacote_inteiro`), e o perfil de quem marca «sem
  IPv6» leva o `pull-filter` (`teste:perfil_do_cliente_so_leva_o_que_foi_pedido`).
- O buraco: quem não marca e tem o IPv6 desligado descobre pelo log do
  OpenVPN. O painel não sabe do kernel do membro; a tela e o `PHXVPN.md`
  dizem o que marcar.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/tunel-total/resultados.json`, `phxvpn/src/saida.rs`, `phxvpn/provas/tunel-total/rodar.sh`
  (os testes citados moram em `phxvpn/src/saida.rs`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
