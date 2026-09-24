# A lista de pares já perfura NAT cone — o farol só é indispensável no simétrico

## O que aconteceu

A prova do farol do phxvpn (`phxvpn/provas/farol/`) tinha como controle
«sem o farol, B e C não se falam». Com os dois atrás de NAT cone (MASQUERADE
com o firewall de roteador doméstico: da wan só entra resposta), o controle
**reprovou**: B pingou C em 1,0 s sem farol nenhum, e o farol, quando havia,
carregou **0** pacote de dados.

## O que eu concluí primeiro, e estava errado

1. **Que o farol estava vazando para o caminho direto** (algum pacote do
   farol abrindo o furo). Não: o mesmo resultado sem marcar o farol.
2. **Que o firewall «só entra resposta» impediria qualquer direto sem
   mediador.** Errado: impede o PRIMEIRO pacote de cada lado, não o segundo.

## O que a medição disse

- Cenário `cone` e `sem-farol-cone`: 1º ping em **1,01 s**, 20/20, 20
  pacotes (21.760 B) saindo direto do NAT de B para o de C, farol 0.
- O mecanismo: o A (alcançável) aprende o endereço público de B e de C pelo
  aperto direto e o ensina pela lista de pares. No `auto`, B e C tentam o
  direto (2 tentativas de 5 s) **ao mesmo tempo**: o INICIO de B abre o
  conntrack no NAT de B; o de C chega nele como «resposta» e entra.
- NAT de C simétrico (`--random`): sem farol **0/20**; com farol 20/20 pelo
  relé (40 pacotes, 44.960 B no farol).
- A perfuração **apresentada pelo farol** (tipos 10/11) só apareceu quando o
  direto foi bloqueado até a lista desistir (cenário `cone-libera`): fura na
  2ª rodada, 84,1 s depois do início.

## A regra

Numa malha com um membro alcançável, conte a lista de pares como perfurador:
controle negativo de NAT se faz com NAT **simétrico**, não com cone — com
cone, o direto nasce da sincronia dos INICIOs, com ou sem mediador.

## Como está guardado hoje

- `phxvpn/provas/farol/rodar.sh` tem os dois controles: `sem-farol`
  (simétrico, exige 0/20) e `sem-farol-cone` (só mede, sem esperado).
- O `PHXVPN.md` («P2P: farol») registra que o pedido esperava o contrário.
- **O buraco:** a perfuração por sincronia é acaso de tempo — se os dois
  lados ligam com mais de ~10 s de diferença, a lista desiste e o par fica
  no intermediário até a rodada de 60 s da perfuração mediada. Não medido.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/farol/resultados.json`, `phxvpn/provas/farol/rodar.sh`
- **Validado em:** 24/09/2026
