# Túnel total é saída de INTERNET, não «tudo o que o servidor alcança»

## O que aconteceu

Frente do túnel total do phxvpn: com `redirect-gateway def1` o membro manda
toda a internet ao servidor, e o servidor precisa de NAT de saída. A tabela
de guarda das rotas (`rotas.rs`, cognição de 11:00) já descartava o espaço da
VPN; faltava decidir o que a rede com túnel total pode alcançar.

## O que eu concluí primeiro, e estava errado

1. **«Túnel total = `accept` e `masquerade` de tudo o que sai da /24 da
   rede, e a guarda cuida do resto.»** A guarda só cobre o espaço da VPN e as
   filiais. A LAN onde o servidor mora (e a LAN que OUTRA rede recebeu por
   rota) fica fora dela — e o NAT faz a LAN responder ao servidor. Quem pediu
   só internet ganharia a LAN da empresa, que o painel só dá por rota e só
   pelo admin.
2. **«A guarda de FORWARD também protege o resolvedor da rede.»** O
   `10.77.B.1` (onde o resolvedor da rede B escuta) é endereço LOCAL do host:
   o pacote do membro da rede A até ele é entregue em INPUT, nunca passa pelo
   FORWARD. Quem guarda o resolvedor é a conferência da ORIGEM no próprio
   soquete (o OpenVPN descarta origem que não é a do membro).

## O que a medição disse

`phxvpn/provas/tunel-total/rodar.sh`, openvpn 2.6.19 em netns, n=1 por motor:

| Caso | nft | iptables |
|---|---|---|
| Túnel total com a exceção da faixa privada: ana → LAN do servidor (ping; TCP) | **0/3**; falhou | **0/3**; falhou |
| RED, túnel «ingênuo» (accept + masquerade sem exceção) | **3/3**; a LAN viu 192.168.10.1 | **3/3**; idem |
| O site externo vê a ana como | 192.0.2.1 (o servidor) | 192.0.2.1 |
| xavier (rede Outra) → `10.77.1.1`: ping; pergunta DNS | 3/3; sem resposta | 3/3; sem resposta |

E a rota de volta (sem NAT) da mesma rede continua mostrando o IP do membro:
o NAT do túnel total é só para `daddr !=` faixa privada.

## A regra

Ao abrir uma saída larga (0/0), escreva o que ela NÃO alcança antes do
`accept` — faixa privada e link-local —, e confira quem chega ao host por
INPUT: guarda de encaminhamento não vê endereço local.

## Como está guardado hoje

- `phxvpn/src/rotas.rs`: `fora_do_tunel_total` antes do `accept` e na negação
  do `masquerade` (`teste:tunel_total_sai_com_nat_so_para_a_internet`).
- `phxvpn/src/dns.rs`: `origem_aceita` no laço do resolvedor
  (`teste:resolvedor_responde_repassa_e_ignora_quem_e_de_fora`).
- RED das duas guardas em `phxvpn/provas/tunel-total/resultados.json` →
  `red_das_guardas` (17/17 reprovam com a guarda tirada).
- O buraco: um host com `policy drop` no INPUT cala o resolvedor; o painel não
  abre regra de entrada (dito no `PHXVPN.md`).

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/tunel-total/resultados.json`, `phxvpn/src/rotas.rs`, `phxvpn/src/dns.rs`
  (os testes citados moram em `phxvpn/src/`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
