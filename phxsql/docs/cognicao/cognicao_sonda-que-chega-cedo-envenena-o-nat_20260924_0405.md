# A sonda que chega cedo envenena o NAT que a recebe

## O que aconteceu

Prova da perfuração de NAT do phxvpn (`phxvpn/provas/perfuracao/rodar.sh`,
24/09/2026): dois nós atrás de dois NATs Linux (`MASQUERADE`), repasse numa
rede «pública». Com o firewall de roteador doméstico na wan (`ctstate NEW ->
DROP`) a perfuração migrou em ~2 s e o repasse carregou **0** datagrama de
dados. Tirando **só** essa regra de firewall — NAT ainda mais aberto — a
perfuração **falhou**: 10 sondas de cada lado, nenhuma entrou, e o túnel
ficou no repasse.

## O que eu concluí primeiro, e estava errado

Que firewall a menos só podia ajudar: se o NAT sem filtro de entrada deixa
passar mais coisa, a perfuração que passava com filtro passaria sem. E que
`MASQUERADE` preserva a porta de origem, logo o mapeamento é independente do
destino e o NAT é «cone» — a condição clássica para o *hole punching* dar certo.

## O que a medição disse

O `/proc/net/nf_conntrack` dos dois NATs, gravado no fim do cenário
`sem-firewall`:

```text
NB: src=203.0.113.1 dst=203.0.113.2 sport=51820 dport=51820 [UNREPLIED]  <- sonda de A, chegou antes do furo de B
NB: src=192.168.2.2 dst=203.0.113.1 sport=51820 dport=51820 [UNREPLIED] src=203.0.113.1 dst=203.0.113.2 sport=51820 dport=3537
NA: src=203.0.113.2 dst=203.0.113.1 sport=3537 dport=51820 [UNREPLIED]  <- sonda de B, com a porta TROCADA
```

A sonda de A chegou ao NAT de B antes de B furar. Sem regra que a
descartasse, ela foi **aceita** no `INPUT` do roteador (nenhum processo
escuta, sai um ICMP) — e a entrada do conntrack foi **confirmada**. Quando B
mandou a sonda dele para A, o SNAT precisava da tupla de volta
`203.0.113.1:51820 -> 203.0.113.2:51820`, que já estava ocupada pela entrada
da sonda de A. O NAT então escolheu **outra porta** (3537): para aquele
destino, virou NAT simétrico. A sonda de B chegou ao NAT de A de uma porta que
A nunca contatou, e morreu. Com o `DROP` na wan, a sonda precoce morre antes de
ser confirmada no conntrack, a porta fica livre e o furo abre.

Números do `resultados.json` da mesma corrida: `cone` (com firewall) migrou,
ping 20/20, repasse 0 pacote de dados; `sem-firewall` não migrou, ping 20/20,
repasse 40 pacotes / 44.960 bytes de dados.

## A regra

**NAT sem filtro de entrada não é NAT mais aberto para perfuração: a primeira
sonda que chega aceita vira dona da porta.** Quem prova *hole punching* em
Linux prova com e sem o `DROP` de `NEW` na wan, e lê o conntrack — o veredito
«é cone» sai da tupla de volta, não da regra `MASQUERADE`.

## Como está guardado hoje

O cenário `sem-firewall` está no roteiro e **tem de reprovar** no critério
(a prova confere isso); o conntrack de cada NAT fica em
`/var/tmp/phx-perfuracao/<cenario>/conntrack-N{A,B}.txt`. O conserto **não
está feito**: a saída conhecida é mandar as primeiras sondas com TTL curto
(abrem o próprio NAT e morrem antes do NAT do outro), mas o TTL é do soquete
inteiro — baixar para uma sonda derrubaria o dado que outra thread mandasse
no mesmo instante — e o número de saltos até o NAT do outro não se conhece.
Anotado em `phxvpn/docs/PHXVPN.md` como limite; o túnel continua pelo repasse,
nunca pior que antes.
