# IPv6 por fora: `[::]` de pilha dupla derrubaria o nó em máquina sem IPv6

## O que aconteceu

O `PHXVPN.md` prometia IPv6 no P2P direto, e o nó e o repasse só abriam
`0.0.0.0` (`comandos.rs`, `main.rs`). A tarefa pedia «netns só com IPv6 entre
dois nós → ping pelo túnel». Antes de escrever a prova, `python3 -c
"socket.socket(AF_INET6, SOCK_DGRAM)"` deu `EAFNOSUPPORT` — no hospedeiro e
dentro de um netns novo. `/proc/cmdline` traz `ipv6.disable=1`: o kernel tem
`CONFIG_IPV6=y`, mas o IPv6 está desligado no arranque desta máquina.

## O que eu concluí primeiro, e estava errado

Que a saída era a do OpenVPN (`socket.c:3089`): um soquete `[::]` com
`IPV6_V6ONLY=0`, desmapeando `::ffff:a.b.c.d` onde se compara endereço. Parecia
o menor conserto — um `bind` só. Nesta máquina, esse `bind` falha, e o nó
inteiro não sobe: o conserto do IPv6 teria quebrado o IPv4, que é o caminho
provado por todas as outras provas em netns daqui.

## O que a medição disse

- `ipv6.disable=1` → `socket(AF_INET6)` = `EAFNOSUPPORT` (errno 97), até em netns.
- Com dois soquetes, o `phxvpn repasse` aqui imprime `sem IPv6 na porta UDP
  51899 … segue so no IPv4` e fica no ar; `provas/perfuracao` cone e
  simétrico conferem com o binário novo.
- O Windows nasce com `IPV6_V6ONLY=1`: lá pilha dupla pediria FFI antes do
  `bind`; dois soquetes saem da `std`. No Linux é o contrário
  (`bindv6only=0`), e o IPv6 só-v6 pede um `setsockopt` antes do `bind`.
- A prova IPv6 de verdade **não roda aqui**; os dois testes por `::1` ficaram
  `#[ignore]` com o motivo escrito.

## A regra

Antes de escolher o desenho de um recurso do sistema operacional, meça se a
máquina da prova TEM o recurso — e escolha o desenho que degrada para o de
antes quando ela não tem, em vez do que troca o de antes pelo novo.

## Como está guardado hoje

- `phxvpn/src/soquete.rs` (topo: os três motivos), `udp_v6_ao_lado` devolve
  aviso em vez de erro; teste `ipv6_ao_lado_do_ipv4_na_mesma_porta_ou_aviso`.
- `phxvpn/docs/PHXVPN.md`, «Ciclo do OpenVPN e IPv6 por fora».
- **Buraco que fica:** tráfego IPv6 real não foi provado. Precisa de uma
  máquina sem `ipv6.disable=1` e `cargo test -- --ignored`.

## Estado

- **Estado:** PENDENTE
- **Evidência:** `phxvpn/src/soquete.rs` (o «sem IPv6 vira aviso» conferido; o caminho IPv6 em si ainda sem prova)
