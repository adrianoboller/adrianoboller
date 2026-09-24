# Lacunas do phxvpn contra o FONTE do OpenVPN 2.6.19 (e o que a 2.7 acrescentou)

Papel J, 24/09/2026. Não implementa nada; é a lista do que falta, medida onde
deu para medir.

- **Fonte lida:** `git clone --depth 1 --branch v2.6.19 https://github.com/OpenVPN/openvpn`
  — `doc/man-sections/*.rst` (**307 opções distintas**, contadas por
  `grep -o '^--[a-z0-9-]+' | sort -u`) e os módulos de `src/openvpn/` que
  decidem os casos duvidosos (`multi.c`, `mroute.c`, `dco.c`, `socket.c`,
  `options.c`). 2.7: `Changes.rst` do ramo `release/2.7` (2.7.7).
- **Lado phxvpn:** conferido no CÓDIGO (`phxvpn/src/*.rs`), não no
  `PHXVPN.md`. O modo servidor é o que `ovpn.rs` escreve no `servidor.conf` e
  no perfil `.ovpn`; o resto do OpenVPN está **no binário, mas não exposto**.
- Classes: **a** já temos · **b** existe pelo OpenVPN embutido, não exposto
  (ou só pelo padrão dele) · **c** falta e vale · **d** falta e NÃO vale.
- Prioridade pelo nicho (LAN virtual estilo Radmin + servidor de pequena
  empresa) e por segurança. Esforço P (≤1 dia), M (dias), G (semana+).

## 1. O que foi MEDIDO nesta rodada (laboratório em netns, root, OpenVPN 2.6.19)

Config do servidor = a do `ovpn.rs:79-105` (sem CRL/usuário/gerência, com
`status 1` para a granularidade). P2P = binário **recompilado do fonte atual**
(o `target/release` era das 03:15, o `p2p.rs` mudou às 09:54 — binário velho
mede o passado). **Uma corrida cada: n=1**; a bancada repete.

| # | Pergunta | Resultado |
|---|---|---|
| M1 | Servidor (tun, `client-to-client`): broadcast IPv4 de um membro chega ao outro? | unicast **5/5**; `10.77.1.255` **0/5**; `255.255.255.255` **0/5** — os 10 chegam à `tun0` do **servidor** e param ali. Multicast `224.0.0.251` **5/5** |
| M2 | P2P: o mesmo | unicast **5/5**; `10.78.0.255` **0/5**; `255.255.255.255` **0/5**; multicast **0/5** — a placa de A emite os 15, B recebe 0 |
| M3 | Membro que sai (SIGTERM) some da lista do servidor em quanto tempo? | sem `explicit-exit-notify`: **128,8 s**; com `explicit-exit-notify 1` no perfil: **9,2 s** (5 s são o «Delayed exit» do servidor, o resto a granularidade do `status`) |
| M4 | Servidor reinicia: em quanto tempo o membro volta? | `kill -KILL` (o que o `supervisor.rs:66-67` faz): **61 s** (espera o `ping-restart 60`); `SIGTERM` + `explicit-exit-notify 1` no servidor: **4 s** |
| M5 | Soquete `AF_INET` envia a destino IPv6? | **não** (`Address family … not supported`) — medido em Python; o nó P2P e o repasse abrem `0.0.0.0` (`comandos.rs:838`, `main.rs:563`) |

Explicação no fonte do M1/M2: em tun, o `mroute.c:167-171` marca só multicast
(`MROUTE_EXTRACT_MCAST`); o `multi.c:3474-3491` manda multicast a todos
(«for now, treat multicast as broadcast») e o resto só vai ao membro dono do
destino — broadcast IPv4 não tem dono. No P2P, `p2p.rs:719-726` descarta o
pacote cujo destino não é o IP de um par.

## 2. Matriz

| Recurso (opções) | Classe | OpenVPN (arquivo:linha) | phxvpn (arquivo:linha) | Prior. | Esf. |
|---|---|---|---|---|---|
| Sub-rede por rede + IP fixo por membro (`server`, `topology subnet`, `client-config-dir`, `ifconfig-push`, `ccd-exclusive`) | a | server-options.rst:494, :118, :269, :114; vpn-network-options.rst:485 | ovpn.rs:85-89, :326-328 | — | — |
| `ifconfig-pool-persist` | d — o IP já é fixo pelo ccd | server-options.rst:242 | ovpn.rs:326 | — | — |
| Membros se enxergam (`client-to-client`) | a | server-options.rst (O11 da matriz anterior) | ovpn.rs:87 | — | — |
| **Broadcast IPv4 entre membros — P2P** (descoberta de jogo/LAN, NetBIOS) | **c** | (o OpenVPN também não faz em tun: mroute.c:167-171) | p2p.rs:719-726 descarta; **M2: 0/10** | **alta** | M |
| **Multicast entre membros — P2P** (mDNS, SSDP, lobby) | **c** | multi.c:3476-3479 (servidor faz: **M1 5/5**) | p2p.rs:719-726; **M2: 0/5** | **alta** | (junto) |
| Broadcast IPv4 entre membros — servidor | c | só com `dev tap` + `server-bridge` (server-options.rst:536; vpn-network-options.rst:120); DCO não faz TAP (dco.c:297) | ovpn.rs:84 `dev tun`; **M1: 0/10** | média | G |
| Multicast entre membros — servidor | a (padrão do OpenVPN) | multi.c:3476-3479 | **M1: 5/5** | — | — |
| **LAN da empresa atrás do servidor** (`push "route …"` + encaminhamento/NAT no host) | **c** | server-options.rst:440; vpn-network-options.rst:370 | ovpn.rs:79-105 sem `push`; nenhum `ip_forward`/NAT no fonte (grep) | **alta** | M |
| LAN atrás de um membro / site-to-site (`iroute` + `route` + `push route`) | c | server-options.rst:339-374 | ccd só leva `ifconfig-push` (ovpn.rs:326-328) | média | M |
| Idem no P2P (faixa por par, tipo `AllowedIPs`) | c | — | um IP por par, origem conferida: p2p.rs:6-10, :1161-1162 | baixa | G |
| Túnel total (`redirect-gateway def1` [+`ipv6`, `block-local`]) | c | vpn-network-options.rst:305-364 | ausente (ovpn.rs:343-363) | média | M |
| Kill switch / `block-outside-dns` / `block-ipv6` | d **hoje** — sem túnel total nem DNS empurrado não há o que vazar; **viram obrigatórios se o túnel total entrar** | windows-options.rst:13-24; vpn-network-options.rst:12-19 | — | (cond.) | M |
| Split tunnel | a por desenho — só a /24 vai ao túnel | vpn-network-options.rst:485 | ovpn.rs:85-86; p2p.rs:719 | — | — |
| DNS empurrado + nomes dos membros (`dns`, `dhcp-option`; 2.7: `dns-updown`, NRPT no Windows) | c | client-options.rst:175-224; vpn-network-options.rst:126; Changes.rst 2.7 «Client implementations for DNS options» | ausente nos dois modos | média | G (pede um DNS nosso) |
| IPv6 dentro do túnel (`server-ipv6`, `ifconfig-ipv6-push`) | c | server-options.rst:595, :311 | P2P só IPv4: p2p.rs:296-304, tun.rs:90; servidor sem `server-ipv6` | baixa | M |
| **Transporte IPv6 por fora — P2P e repasse** | **c (defeito)** — o `PHXVPN.md:150` promete «IPv6» no direto | — | `bind("0.0.0.0")` em comandos.rs:838 e main.rs:563; **M5** | **alta** (doc mente) | P |
| Transporte IPv6 por fora — servidor | a (padrão: bind dual-stack) | socket.c:3089; options.c:812 | ovpn.rs:82-83 (sem `local`) | — | — |
| `compress`/`comp-lzo` | d — VORACLE; padrão `allow-compression no`; a 2.7 **tirou** a compressão no envio | protocol-options.rst:8-30, :91-94; Changes.rst 2.7 «Compression on send has been removed» | não escrito (fica no padrão seguro) | — | — |
| `fragment` | d — desliga o DCO e o `mssfix` já cobre | dco.c:241; link-options.rst:27 | — | — | — |
| `mssfix` / MTU no servidor | a (padrão `1492 mtu`) | link-options.rst:128-142 | ovpn.rs (padrão) | — | — |
| **MTU do P2P pelo repasse/farol** | **c** | (referência: `mssfix`, link-options.rst:128) | placa 1420 (p2p.rs:52) + cab. 16 (transporte.rs:38) + etiqueta 16 + embrulho 36 (repasse.rs:275-279) + UDP/IPv4 28 = **1.516 > 1.500** — calculado das constantes, **não medido** na rede | média | P |
| `keepalive`/`ping-restart` | a | link-options.rst:65-99 | ovpn.rs:90; P2P p2p.rs:53, :60 | — | — |
| `persist-key`/`persist-tun` | a | generic-options.rst; vpn-network-options.rst | ovpn.rs:91-92, :353-354 | — | — |
| **`explicit-exit-notify` no perfil** (lista de membros honesta) | **c** | client-options.rst:231-254 | perfil sem a linha (ovpn.rs:343-363); lista lida do `status.log` (painel.rs:1139-1147); **M3: 128,8 → 9,2 s** | **alta** | P |
| **Reinício do servidor com aviso** (`explicit-exit-notify` no servidor + SIGTERM) | **c** | client-options.rst:245-249; multi.c:3866 | `Child::kill` = SIGKILL (supervisor.rs:66-67); **M4: 61 → 4 s** | média | P |
| `remote` múltiplos / `remote-random` / failover de servidor | c | client-options.rst:447, :511 | um `remote` só (ovpn.rs:349) | baixa | G |
| Queda UDP→TCP no modo servidor (`<connection>` + 2.7 multi-socket) | c | connection-profiles.rst; Changes.rst 2.7 «Multi-socket support for servers» | rede é UDP **ou** TCP (ovpn.rs:33-39); o P2P já cai e volta (PHXVPN.md, `auto`) | média | M (pede 2.7 no servidor) |
| `resolv-retry` / `connect-retry` | a | client-options.rst:521, :152 | ovpn.rs:351 | — | — |
| `float` / troca de endereço do membro | a | link-options.rst:13-26 (servidor: padrão) | P2P aprende pela sessão: p2p.rs:637 | — | — |
| `port-share` (OpenVPN TCP 443 dividindo porta com HTTPS) | c | server-options.rst:415-438 («Not implemented on Windows») | ausente | baixa | P |
| `status` + gerência (`management`, `client-kill`) | a | generic-options.rst:404; management-options.rst:6 | ovpn.rs:102-103; credencial.rs:486-493, :587 | — | — |
| **Log do OpenVPN sem teto** | **c** | log-options.rst:14-38 (`log`/`log-append`) | `openvpn.log` em append, `verb 3`, sem rotação (supervisor.rs:40-50) | média | P |
| Histórico de conexões (quem entrou/saiu, quando) — `client-connect`/`client-disconnect`/`learn-address` | b | script-options.rst:187, :220, :285 | só o «agora» do `status.log` (painel.rs:1139) | média | M |
| `max-clients` | a por construção — a /24 dá 253; padrão 1024 | server-options.rst:386; options.c:861 | ovpn.rs:86 | — | — |
| `connect-freq-initial` (reflexão) | a (padrão 100/10 s) | server-options.rst:192-209; options.c:862-863 | padrão | — | — |
| `connect-freq` | b | server-options.rst:168-190 (o próprio manual manda preferir `tls-crypt`) | `tls-crypt(-v2)` já na frente (ovpn.rs:111-122) | baixa | P |
| **`tls-crypt-v2 … force-cookie`** | **b** | tls-options.rst:511-516 (padrão `allow-noncookie`) | ovpn.rs:115 sem o parâmetro | média | P (+ prova com Connect/GUI) |
| `duplicate-cn` | a (desligado **de propósito**: CN leva a série, reentrada revoga) | server-options.rst:211-214 | pki/painel (série no CN) | — | — |
| `reneg-sec`/`reneg-bytes` | a (padrão 3600 s; P2P 120 s) | renegotiation.rst:7-40 | padrão; P2P PHXVPN.md «refazer aos 120 s» | — | — |
| `tls-version-min`, `data-ciphers`, `dh none`, `remote-cert-tls`, `verify-x509-name` | a | tls-options.rst:564, :166, :244, :636; protocol-options.rst:188 | ovpn.rs:98-101, :355-358 | — | — |
| `tls-groups` híbrido pós-quântico (X25519MLKEM768) | c | tls-options.rst:345; Changes.rst 2.7 «PQE support for WolfSSL» | ausente | baixa | P (depende do OpenSSL 3.5 nos dois lados — não medido) |
| MFA: `auth-user-pass-verify` + `static-challenge` + `auth-gen-token` | a | script-options.rst:63; client-options.rst (static-challenge); server-options.rst:11 | verificar.rs:47-50, :84-87 | — | — |
| `auth-gen-token-secret` (token sobrevive ao reinício) | b | server-options.rst:92-97 | ausente: reinício = código de novo para todos | baixa | P |
| `crl-verify`, `tls-crypt-v2-verify` | a | tls-options.rst:119, :518 | ovpn.rs:97, :115-117, :226-241 | — | — |
| `user`/`group` sem root | a | generic-options.rst:494 | ovpn.rs:318-323 | — | — |
| `mlock` (chave fora do swap) | c | generic-options.rst:268-280 | ausente | baixa | P |
| `chroot` | d — o OpenVPN relê `ccd/` e `crl.pem` a cada conexão; o `user` já tira o root | generic-options.rst:32 | ovpn.rs:312-317 | — | — |
| DCO (`ovpn-dco`) | b — a config é compatível (subnet, AEAD, sem compressão); some com `http-proxy` | dco.c:247, :392, :425; generic-options.rst:181 | ovpn.rs:77, :85; **não medido** | baixa | P (bancada) |
| `http-proxy` | a | proxy-options.rst:1 | ovpn.rs:42-62, :369-374 | — | — |
| `http-proxy-user-pass` / `socks-proxy` | c | proxy-options.rst:50, :85 | só proxy sem senha | baixa | P |
| Certificado em cartão/repositório do Windows (`pkcs11-*`, `cryptoapicert`) | c | pkcs11-options.rst; windows-options.rst:25 | pendência «certificado A1 da empresa» (PHXVPN.md, Falta) | baixa | G |
| `plugin` (RADIUS/LDAP) | **dono** (produto) — já listado | plugin-options.rst:7 | PHXVPN.md, Falta | — | — |
| `inactive` / `session-timeout` | d — o token de 12 h já força código novo | client-options.rst:256; link-options.rst:434 | verificar.rs:30 | — | — |
| `pull-filter` | d — o manual diz que não é medida de segurança; nós geramos os dois lados | client-options.rst:343-347 | — | — | — |
| `auth-nocache` | d — com token, a senha digitada «is never preserved» | client-options.rst:30-32 | verificar.rs:47-50 | — | — |
| `multihome` | c | server-options.rst:321-338 | sem `local` (ovpn.rs:82) | baixa | P |
| `secret` (chave estática), `peer-fingerprint` | d — obsoleto / temos AC | protocol-options.rst (`--secret` DEPRECATED); tls-options.rst:613 | pki.rs | — | — |
| `replay-persist`, `shaper`, `vlan-*`, `client-nat`, `bind-dev`/`mark`, `push-peer-info`, `mute-replay-warnings`, `stale-routes-check`, `max-routes-per-client` | d — fora do nicho ou só servem a TAP/iroute | link-options.rst:420; advanced-options.rst:52; server-options.rst:673, :608, :389; client-options.rst:129, :348; log-options.rst:40 | — | — | — |
| Driver do Windows (`windows-driver`; 2.7 tirou o wintun) | a — já decidido: TAP-Windows6, presente na 2.6 e na 2.7 | windows-options.rst:256; Changes.rst 2.7 «Support for wintun… removed» | tun_windows.rs:1-6 | — | — |
| 2.7: `PUSH_UPDATE` (mudar rota/DNS sem reconectar) | c | Changes.rst 2.7 «PUSH_UPDATE» | — | baixa | M |
| 2.7: `tls-crypt-v2-max-age` | d — a revogação por série já barra antes do TLS | Changes.rst 2.7 | ovpn.rs:226-241 | — | — |

**Contagem (linhas da matriz, por `awk` na coluna 2):** a **21** · b **5** · c **23** (uma delas
defeito) · d **11** · dono **1** — **61** linhas.

## 3. Top 10 (o que fazer, em ordem)

| # | Item | Por quê | Prior. | Esf. |
|---|---|---|---|---|
| 1 | **Broadcast e multicast no P2P** (repassar `x.x.x.255`, `255.255.255.255` e `224/4` a todos os pares da rede, com teto por segundo contra amplificação e a conferência de origem mantida) | É o coração do nicho Radmin (jogo e descoberta de LAN); **M2: 0/15** | alta | M |
| 2 | **IPv6 por fora no P2P e no repasse** — abrir `[::]` (dual-stack) ou dois soquetes — **ou** tirar «IPv6» do `PHXVPN.md:150` | A documentação promete o que o soquete não faz (**M5**) | alta | P |
| 3 | **`explicit-exit-notify 1` no perfil** | Lista de membros mente por ~2 min: **128,8 → 9,2 s** (M3) | alta | P |
| 4 | **LAN da empresa atrás do servidor**: `push "route <lan>"` por rede, escolhido no painel, + encaminhamento/NAT no host | É o uso nº 1 de VPN de pequena empresa (ERP, impressora, pasta da matriz) | alta | M |
| 5 | **Reinício do servidor com aviso**: `SIGTERM` + `explicit-exit-notify 1` no servidor | Hoje todo membro fica 61 s no escuro a cada troca de MFA (**M4: 61 → 4 s**) | média | P |
| 6 | **MTU do P2P pelo repasse/farol** (placa menor, ou MSS ajustado quando a via é o repasse) | 1.516 B > 1.500 — fragmenta, e CGNAT costuma jogar fragmento fora. **Raciocinado, não medido**: decide um `ping -M do -s 1392` pelo repasse em netns | média | P |
| 7 | **Log do OpenVPN com teto** (rotação ou `log` + troca) | `openvpn.log` cresce para sempre em `verb 3` | média | P |
| 8 | **`tls-crypt-v2 … force-cookie`** | Tira a exaustão de estado do aperto; o padrão ainda é `allow-noncookie`. Só depois de provar Connect 3 e GUI 2.6 (os dois têm de mandar o cookie) | média | P |
| 9 | **Site-to-site (`iroute`)** — LAN atrás de um membro, no mesmo `ccd` que já existe | Filial com roteador; o ccd e o `client-to-client` já estão lá | média | M |
| 10 | **Histórico de conexões** (`client-connect`/`client-disconnect` → PostgreSQL) | Auditoria «quem entrou quando» que o `status.log` não guarda | média | M |

Fora do top, mas registrado: túnel total (média, M) **só entra com o pacote
inteiro** — `block-outside-dns`, `block-ipv6`/`ipv6`, `block-local` e NAT no
servidor —, senão entrega vazamento de DNS/IPv6 no Windows; queda UDP→TCP do
servidor espera a 2.7 no servidor (multi-socket).

## 4. Hipóteses que morreram (com o número)

- **«Falta `duplicate-cn`.»** Morreu: desligado é o certo aqui — o CN leva a
  série e reentrada revoga; ligar deixaria perfil roubado conviver calado
  (server-options.rst:211-214).
- **«Falta `compress`.»** Morreu: VORACLE (protocol-options.rst:91-94) e a 2.7
  removeu a compressão no envio. O padrão `allow-compression no` já nos
  protege sem escrever nada.
- **«Falta `fragment` para MTU.»** Morreu: desliga o DCO (dco.c:241) e o
  `mssfix 1492 mtu` padrão já cobre o modo servidor.
- **«Falta `auth-nocache` no perfil de MFA.»** Morreu: com `auth-gen-token` a
  senha digitada «is never preserved once an authentication token has been
  set» (client-options.rst:30-32).
- **«Falta kill switch / `block-outside-dns`.»** Morreu **hoje**: sem túnel
  total e sem DNS empurrado não há vazamento a bloquear. Renasce junto do
  túnel total, e aí é obrigatório.
- **«O modo servidor não repassa multicast.»** Morreu medido: repassa, **5/5**
  (M1) — só o broadcast IPv4 fica, **0/10**.
- **«O servidor não aceita cliente por IPv6.»** Morreu: bind dual-stack é o
  padrão (socket.c:3089, options.c:812). O IPv4-only é do **P2P** (M5).
- **«A 2.7 quebra o P2P no Windows (tirou o wintun).»** Morreu: o
  `tun_windows.rs:1-6` já usa o TAP-Windows6, que a 2.7 mantém como queda.
- **«`connect-freq` falta como anti-DoS.»** Morreu como prioridade: o próprio
  manual manda preferir `tls-crypt` (server-options.rst:188-190), que já está
  na frente, e o `connect-freq-initial` 100/10 s é padrão (options.c:862-863).

## 5. Lacunas desta pesquisa

- M1–M4 são **n=1**; M6 (MTU pelo repasse) **não foi medido**; DCO **não foi
  medido** (o módulo não foi carregado no laboratório).
- O `force-cookie` (item 8) depende de o OpenVPN Connect 3 mandar o cookie —
  **não conferido** no fonte do Connect.
- A 2.7 foi lida **só** no `Changes.rst`, não no manual nem no fonte.
- Broadcast no modo servidor por TAP (`server-bridge`): o custo real é o
  cliente — o suporte a TAP no OpenVPN Connect **não foi conferido** no fonte
  dele; por isso ficou em média/G e fora do top.
