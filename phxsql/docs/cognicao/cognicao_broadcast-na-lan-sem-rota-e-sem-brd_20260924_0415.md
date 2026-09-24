# Broadcast na LAN: o limitado não sai sem rota, e o `ifa_broadaddr` mente sem `brd`

## 1. O que aconteceu

A descoberta na LAN do phxvpn (`phxvpn/src/descoberta.rs`) passou em todos os
testes unitários — o anúncio era mandado direto ao soquete do outro, no lugar
do broadcast — e **falhou na prova em `netns`**: dois membros na mesma bridge,
zero anúncios no `tcpdump`, nenhum ping. Dois defeitos em fila, os dois do
sistema operacional, nenhum visível por teste unitário.

## 2. O que eu concluí primeiro, e estava errado

Primeiro: «`255.255.255.255` com `SO_BROADCAST` chega a todo mundo do
segmento». Não chega: sem rota padrão, o Linux devolve `ENETUNREACH`. Troquei
pelo broadcast dirigido lido do `getifaddrs` (`ifa_broadaddr`) e concluí
«agora vai». Também não foi: o `strace` mostrou o anúncio indo para
`192.168.77.4` — o **próprio endereço** da interface.

## 3. O que a medição disse

- `ip addr add 192.168.77.4/24 dev eth0` **sem `brd`**: o `getifaddrs` devolve
  em `ifa_broadaddr` o endereço da interface, não o `.255`. O `sendto` para
  `192.168.77.255`, calculado à mão, chega ao vizinho mesmo assim.
- `sendto(255.255.255.255)` num netns só com a bridge e sem rota padrão:
  `-1 ENETUNREACH (Network is unreachable)` (medido com Python e com o
  `strace` do phxvpn).
- Consertado (endereço `|` `!máscara`), E→D pelo túnel em **459 ms**, dois
  anúncios de 200 B no fio, nome da rede em claro 0.

E um terceiro, do mesmo dia e do mesmo roteiro, este nosso: o rol assinado ia
**depois** da lista de pares no mesmo lote, e a lista, em rede assinada, só
ensina endereço de quem já está no rol — então o endereço do membro novo era
jogado fora e só voltava no reenvio de 30 s. Medido B→C em **30,2 s**; com o
rol na frente, **11 ms**.

## 4. A regra

Broadcast dirigido se **calcula** (endereço `|` `!máscara`), não se lê do
`ifa_broadaddr`; o limitado vai junto, nunca sozinho. E quando duas mensagens
de um lote dependem uma da outra, a ordem de envio é parte do protocolo —
ponha a que dá permissão antes da que usa a permissão.

## 5. Como está guardado hoje

- O cálculo está em `descoberta::broadcasts_das_interfaces`, com o motivo no
  comentário; o teste `destinos_tem_o_limitado_e_nao_tem_loopback` só confere
  a lista, **não** o valor do broadcast — quem guarda o valor é a prova
  `phxvpn/provas/rol-descoberta/rodar.sh` (b), que configura a interface sem
  `brd` de propósito.
- A ordem rol→lista está em `No::rol_para`, com o número no comentário; o
  teste unitário não a trava (ele bombeia até convergir) — quem a mede é a
  prova (a), `a_bc_ms` no `resultados.json`.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/rol-descoberta/resultados.json`, `commit:aba3610`
- **Validado em:** 24/09/2026
