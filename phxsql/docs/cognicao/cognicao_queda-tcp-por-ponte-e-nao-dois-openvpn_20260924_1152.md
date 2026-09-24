# Queda UDP→TCP no servidor 2.6: uma ponte TCP→UDP, e não dois `openvpn`

## O que aconteceu

Frente do alcance do modo servidor do phxvpn: o membro cuja rede bloqueia UDP
tem de cair para TCP/443 sozinho. O servidor 2.6 escuta num protocolo só; o
multi-soquete é da 2.7 (`Changes.rst` «Multi-socket support for servers»).
O pedido chegou com o desenho pronto: dois processos (um UDP, um TCP) na
mesma rede, com pools que não colidam e o mesmo `ccd`/CRL.

## O que eu concluí primeiro, e estava errado

«Dois processos é o único caminho na 2.6.» Era a receita conhecida, e ela
cobrava três coisas que esta casa promete:

1. **IP fixo** — cada processo tem a sua placa e a sua sub-rede; o membro
   que caísse para o TCP ganharia outro IP, e o `ccd/` fixa um só.
2. **LAN entre membros** — o `client-to-client` não atravessa de um
   processo ao outro: o membro no TCP não veria o do UDP.
3. **Isolamento** — o kernel teria de encaminhar entre as duas placas:
   `ip_forward` e a guarda de `rotas.rs` aberta para uma faixa nova (ver
   `cognicao_ip-forward-abre-o-isolamento-entre-redes-vpn_20260924_1100.md`).

## O que a medição disse

Hipótese nova: o pacote do OpenVPN é o mesmo nos dois transportes, só o
enquadramento muda (2 bytes de tamanho no TCP). Uma ponte que tira o quadro
e entrega o datagrama ao `openvpn` UDP por `127.0.0.1` deveria bastar.

- Laboratório (ponte em Python, 2.6.19): cliente `tcp-client` × servidor
  `udp` conectou, **sem aviso de incompatibilidade** no log dos dois lados.
- Prova com o painel (`phxvpn/provas/servidor-alcance/rodar.sh`, ponte em
  Rust no supervisor): com o UDP da ana bloqueado ela entra pelo TCP com o
  **mesmo IP do `ccd` (10.77.1.3)** e pinga o admin que está no UDP **5/5**
  — as três promessas de pé, com zero regra de firewall nova.
- O custo que apareceu: para o OpenVPN todo cliente da ponte vem do
  loopback, e o limitador do autenticador conta por `untrusted_ip`. Com
  `127.0.0.1` para todos, um atacante pela 443 travaria o código de todo
  membro que caiu no TCP. Cada IP de fora ganha o seu `127.x.y.z` (SHA-256
  com chave do processo); o `status.log` mostrou a ana como
  `127.65.233.200:43395`, e a linha `queda-tcp:` do `openvpn.log` liga esse
  endereço ao IP de fora.

## A regra

Antes de aceitar a receita de «dois processos» (ou de qualquer duplicação de
servidor), pergunte o que cada promessa da casa vira do outro lado — e
procure a peça que só muda o enquadramento.

## Como está guardado hoje

- `phxvpn/src/queda_tcp.rs` (a ponte), `phxvpn/src/supervisor.rs`
  (`garantir_ponte`: sobe, troca e derruba com o OpenVPN da rede).
- `teste:quadro_tcp_vira_datagrama_e_volta` reprova se a ponte falar pelo
  `127.0.0.1` de todos; `teste:ponte_segue_o_arquivo_da_pasta` reprova se a
  ponte velha não largar a porta antes da nova — RED em
  `phxvpn/provas/servidor-alcance/resultados.json` → `red_das_guardas`.
- O que fica em aberto: a origem real não chega ao `client-connect` (o
  histórico de conexões verá `127.x.y.z`); a correspondência está só no
  `openvpn.log`. E o membro que some pela ponte fica na lista até o
  `ping-restart` do servidor (o TCP fechado não vira `explicit-exit-notify`).

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/servidor-alcance/resultados.json`, `phxvpn/src/queda_tcp.rs`, `phxvpn/provas/servidor-alcance/rodar.sh`
  (os testes citados moram em `phxvpn/src/`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
