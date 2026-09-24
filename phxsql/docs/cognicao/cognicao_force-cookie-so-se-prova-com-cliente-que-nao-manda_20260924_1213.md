# `force-cookie` só se prova com um cliente que NÃO manda o cookie

## O que aconteceu

Item 8 das lacunas do phxvpn: ligar `tls-crypt-v2 … force-cookie` nas redes
v2 em UDP (`phxvpn/src/ovpn.rs`, `Rede::cookie`). O cliente 2.6.19 entra com e
sem a opção — então conectar com ele não prova nada sobre a opção. O RED
precisava de um cliente sem suporte ao cookie: o **2.5.11 compilado do fonte**
(`git clone --branch v2.5.11`, `./configure --disable-lzo --disable-lz4
--disable-plugins`, 1 min de `make`).

## O que eu concluí primeiro, e estava errado

Li `proto.ta_pid_send.init(EARLY_NEG_START)` na linha 4132 do
`openvpn/ssl/proto.hpp` do OpenVPN 3 core e tomei como o lado do **cliente**.
Era o lado do **servidor** (a detecção de v2 no `decapsulate`). O do cliente é
outro, na linha 4780 (`reset()`, caso `TLS_CRYPT_V2`, fora do `is_server()`).
Se eu tivesse parado na primeira, teria concluído certo por acaso — e a
conclusão sem a linha certa não se defende.

E o segundo erro, do mesmo dia: com o Connect ≥ 3.8 provado, **liguei a opção
sempre**. O suporte do cliente mais novo não decide nada — decide quem está
instalado: o 2.5 ainda é o pacote do Ubuntu 22.04 (2.5.11) e do EPEL 9
(2.5.11), e o EPEL 8 está no 2.4.12. O integrador desfez pela lei «guarda nova
entra pedida»: virou opção por rede, desligada por padrão (`src/cookie.rs`).

## O que a medição disse

`provas/operacao/openvpn.sh`, netns, n=1 (`resultados.json`, seção `openvpn`):

| Servidor | Cliente 2.6.19 | Cliente 2.5.11 |
|---|---|---|
| com `force-cookie` (ligado pelo admin) | entra | **não entra** em 25 s |
| sem (rede recém-criada, e o binário de antes) | entra | entra |

No fonte: o servidor só olha a opção no `mudp.c:122` (UDP; `do_pre_decrypt_check`); o OpenVPN 3
core manda `EARLY_NEG_START` e reenvia a WKc desde o commit `2ff291e7`
(16/11/2022), primeira etiqueta que o contém: `release/3.8`.

## A regra

Guarda que só muda o comportamento para o cliente velho se prova com o
cliente velho (compile-o do fonte se não houver pacote) — e entra pedida,
medida contra a versão que as LTS empacotam, nunca contra a mais nova.

## Como está guardado hoje

`provas/operacao/openvpn.sh` (os dois sentidos no mesmo roteiro, com
`PHXVPN_OVPN25`), `teste:force_cookie_so_quando_a_rede_pede_e_so_em_udp` e
`teste:force_cookie_nasce_desligado_e_o_admin_liga_por_rede`. O buraco: o OpenVPN
Connect é fechado — o que se conferiu foi o core que ele usa, não o
aplicativo; versão do Connect anterior ao core 3.8 fica de fora e não foi
medida.

## Estado

- **Estado:** FRUTÍFERO
- **Evidência:** `phxvpn/provas/operacao/resultados.json`, `phxvpn/provas/operacao/openvpn.sh`, `teste:force_cookie_so_quando_a_rede_pede_e_so_em_udp`, `teste:force_cookie_nasce_desligado_e_o_admin_liga_por_rede`
- **Validado em:** 24/09/2026
