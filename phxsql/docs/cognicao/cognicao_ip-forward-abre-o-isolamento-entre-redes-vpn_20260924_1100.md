# Ligar o `ip_forward` para a LAN da empresa abre a rede A para a rede B

## O que aconteceu

Frente das redes alcançáveis do phxvpn (lacunas do OpenVPN, itens 4 e 9): dar
aos membros acesso à LAN atrás do servidor pede `push "route"` e o host do
servidor encaminhando (`ip_forward`) com NAT. O `ovpn.rs` promete isolamento
entre redes «sem regra de firewall»: uma instância de OpenVPN e uma placa
(`tun0`, `tun1`…) por rede.

## O que eu concluí primeiro, e estava errado

1. **«Basta `ip_forward=1` e um `masquerade` da /24 da VPN para a LAN.»** O
   pedido dizia «encaminhar e fazer NAT», e o NAT sozinho resolve o caminho
   membro → LAN. Mas o `ip_forward` é do HOST inteiro: com ele aceso o kernel
   passa a rotear entre `tun0` e `tun1`, e a promessa do `ovpn.rs` só valia
   porque o encaminhamento estava desligado — não por ser «processo e placa
   diferentes».
2. **«Sem rota configurada, o isolamento continua valendo.»** Vale só no host
   com `ip_forward=0`. Num host que já encaminha (Docker, roteador, firewalld
   com masquerade), a rede A já fala com a rede B hoje, sem nenhuma rota do
   phxvpn — o RED abaixo é exatamente esse host.

## O que a medição disse

`phxvpn/provas/rotas/rodar.sh`, openvpn 2.6.19 real em netns, n=1 por motor:

| Caso | nft | iptables |
|---|---|---|
| X (rede Outra) → A (Matriz), rota manual, tabela de guarda no lugar | **0/3** | **0/3** |
| O mesmo, RED: tabela de guarda apagada, `ip_forward=1` | **3/3** | **3/3** |
| A → LAN 192.168.10.5 sem rota (mesmo com rota manual no membro) | 0/3 | 0/3 |
| A → LAN com a rota (NAT): ping / TCP, IP visto pela LAN | 3/3 / 192.168.10.1 | 3/3 / 192.168.10.1 |
| Regras nossas antes → com rota → depois de remover | 0 → 6 → 0 | 0 → 6 → 0 |
| `ip_forward` antes → com rota → depois | 0 → 1 → 0 | 0 → 1 → 0 |

E um resíduo que não é regra nossa: o `iptables-nft` deixa as tabelas
`filter`/`nat` vazias que ele mesmo criou (**4 handles** no `nft list
ruleset`, **0 regras** no `iptables-save`). Por isso a prova compara regras no
motor iptables e handles no nft.

## A regra

Quem acende o encaminhamento do host acende JUNTO uma guarda que só deixa
passar o par (origem da rede N, LAN da rede N) e descarta o resto do espaço da
VPN — antes de escrever o `1` no `ip_forward`, e apagando os dois juntos.

## Como está guardado hoje

- `phxvpn/src/rotas.rs`: `script_nft` / `script_iptables` põem os `drop` do
  `10.77.0.0/16` e das filiais depois dos `accept`; `aplicar` grava as regras
  antes do `ip_forward` e devolve o valor antigo quando a última rota sai.
- `teste:nft_guarda_o_isolamento_entre_redes_e_so_mascara_o_nat` (reprova com
  os `drop` tirados — RED em `provas/rotas/resultados.json` →
  `red_das_guardas`).
- **Buraco que fica:** host que JÁ encaminha, sem nenhuma rota do phxvpn, não
  tem a tabela de guarda — a rede A alcança a rede B nele hoje. Pôr a guarda
  sempre seria regra nova imposta a toda instalação: vai à mesa, não entrou.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/rotas/resultados.json`, `phxvpn/src/rotas.rs`, `phxvpn/provas/rotas/rodar.sh`
  (o teste citado mora em `phxvpn/src/rotas.rs`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
