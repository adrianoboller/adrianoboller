# Túnel que pinga não é túnel que se recupera

## O que aconteceu

Primeira prova de ponta a ponta do P2P do phxvpn (24/09/2026, dois `ip netns`
ligados por veth): ping 4/4, `tcpdump` sem texto claro no cabo, 660 Mbit/s,
senha errada com 100% de perda. Então reiniciei o nó B com a senha certa, e o
ping continuou em **100% de perda**.

## O que eu concluí primeiro, e estava errado

Que, depois de ping, cifra no cabo e senha errada barrada, o transporte estava
pronto. Os 34 testes unitários e de UDP real passavam. Nenhum deles reiniciava
um lado: todos nasciam com os dois nós juntos e morriam juntos.

## O que a medição disse

A sessão de A continuava válida de um lado só: confirmada e com menos de
120 s. A cifrava com as chaves velhas e o B novo descartava tudo, porque não
conhecia o índice. Sem regra para «mandei e não ouvi», A só refaria o aperto
aos 120 s. Com a regra do WireGuard (10 s + 5 s), A voltou sozinho no
`icmp_seq=31` com intervalo de 0,5 s, ou seja, ~15,5 s.

## A regra

Protocolo com estado se prova também **reiniciando um dos lados no meio**. O
caminho feliz com os dois nascendo juntos não exercita a perda de estado.

## Como está guardado hoje

`p2p::testes::par_surdo_dispara_aperto_novo` trava a regra, mas envelhece o
relógio à mão (subtrai `SURDO_APOS`). O reinício de verdade só se provou no
roteiro com `ip netns`, que **não é teste automatizado**: precisa de root e
de `iproute2`. Esse buraco está anotado.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `teste:par_surdo_dispara_aperto_novo`
- **Validado em:** 24/09/2026
