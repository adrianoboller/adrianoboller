# Cognição: difusão numa placa TUN — o Linux entrega, o TAP-Windows6 não origina

Frente da difusão do phxvpn (`phxvpn/src/difusao.rs`), 24/09/2026.

## O que aconteceu

O P2P passou a replicar broadcast e multicast da placa a todos os pares. Antes
de escrever a prova, duas perguntas de sistema operacional decidiam se a
replicação serviria para alguma coisa: (1) o kernel de quem RECEBE entrega a
um soquete o pacote para `10.78.0.255` escrito numa placa TUN, que é
ponto-a-ponto e não tem `IFF_BROADCAST`? (2) no Windows, o TAP-Windows6 em
modo TUN entrega ao programa o broadcast que uma aplicação manda?

## O que eu concluí primeiro, e estava errado

Concluí que (1) **não**: sem `IFF_BROADCAST`, o `SIOCSIFNETMASK` não calcula
endereço de broadcast, então `10.78.0.255` seria unicast para um host
inexistente e o kernel tentaria encaminhar (e descartaria). Planejei mexer no
`tun.rs` para pôr o broadcast por `SIOCSIFBRDADDR`, ou reescrever o destino
para `255.255.255.255` na entrada (com as duas somas de verificação).

## O que a medição disse

- (1) Medido em `netns` (kernel 6.18, placa TUN com `IFF_NO_PI`, pacote UDP
  escrito à mão no descritor): `10.78.0.255` **chegou** ao soquete, com e sem
  `broadcast` explícito. O `fib_add_ifaddr` põe a rota `broadcast` do prefixo
  (`ip route show table local` a mostra) para qualquer prefixo < 31,
  independente do `IFF_BROADCAST`. Depois, pela placa do próprio phxvpn
  (endereço e máscara por `ioctl`, sem `iproute2`): 20/20 em cada receptor
  (`phxvpn/provas/broadcast/resultados.json`). O `tun.rs` não precisou mudar.
- (1b) Para a aplicação ORIGINAR pela placa, basta o `bind` no IP dela: para
  multicast e `255.255.255.255`, o Linux escolheu a interface de saída pela
  origem, mesmo com a rota padrão em outra placa (medido na prova; a função
  do kernel que faz isso não foi conferida no fonte).
- (2) Lido no fonte do TAP-Windows6 (`src/txpath.c`, ramo `m_tun`, IPv4):
  «Only accept directed packets, not broadcasts» — o quadro só vai ao
  programa se o cabeçalho Ethernet inteiro for o do par ponto-a-ponto.
  Broadcast e multicast morrem no driver. O caminho de volta (`rxpath.c`)
  põe o cabeçalho dirigido ao adaptador, então RECEBER deve funcionar.
  **Não rodou num Windows**.

## A regra

Antes de mudar a placa para «consertar» a entrega, escreva o pacote no
descritor e conte o que chega ao soquete — o kernel sabe mais do que o
`IFF_*` sugere. E no Windows, o modo TUN do TAP é unicast para quem ORIGINA:
difusão originada no Windows pede o TAP em modo Ethernet.

## Como está guardado hoje

- `phxvpn/src/difusao.rs` (cabeçalho, seção Windows) e `phxvpn/docs/PHXVPN.md`
  «P2P: difusão» registram as duas respostas.
- `phxvpn/provas/broadcast/rodar.sh` prova a entrega no Linux a cada corrida
  (quatro destinos, dois receptores) e entrou no `validar-tudo.sh`.
- **Buraco que fica:** Windows não origina difusão, e nem a recepção foi
  provada numa máquina real; está em «Falta» no `PHXVPN.md`.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/broadcast/resultados.json`, `phxvpn/src/difusao.rs`, `phxvpn/provas/broadcast/rodar.sh`
  (os testes `difusao_chega_aos_dois_pares_e_nao_faz_laco` e `desligada_nao_sai_nem_entra` moram em `difusao.rs`; o `classificar.py` só procura `teste:` dentro de `phxsql/`)
- **Validado em:** 24/09/2026
