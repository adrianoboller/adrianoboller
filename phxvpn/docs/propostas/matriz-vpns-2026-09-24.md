# Matriz de recursos: Radmin VPN × OpenVPN × VPN do Windows × VPN do Linux — contra o phxvpn

Papel J, 24/09/2026. Lido na fonte primária de cada produto, no mesmo dia.
Desempenho **fora** daqui, de propósito: nenhum número de vazão ou latência
nesta página. Onde a documentação oficial cala, a célula diz **não
documentado**. Não preenchi nada de memória: fórum e blog de terceiro não
contam como fonte.

## Como ler

- Cada célula traz o valor e o código da fonte entre colchetes, por exemplo `[O4]`. Os códigos estão
  em §3, com a URL e a frase citada.
- **ND** = não documentado nas fontes primárias lidas. Não quer dizer «não
  tem»: quer dizer que o fabricante não o escreve.
- Coluna **phxvpn**: é o que o próprio `phxvpn/docs/PHXVPN.md` declara
  (commit `81f6e12`), citado por linha como `[P:n]`. É **autodeclaração**:
  não refiz nenhuma prova nesta rodada.
- Versões lidas: OpenVPN **2.6.19** (tag `v2.6.19`, `doc/man-sections/`). A
  página de download da comunidade já distribui a **2.7.7** [O17], e onde a 2.7
  diverge ninguém leu ainda. strongSwan: docs `latest` (6.1). WireGuard: site e
  artigo técnico. Windows: Learn (2025–2026).

## 1. Matriz

| Recurso | Radmin VPN | OpenVPN 2.6 (AS = Access Server) | VPN nativa do Windows | Linux: WireGuard / strongSwan / NetworkManager | phxvpn (autodeclarado) |
|---|---|---|---|---|---|
| **Modelo** | «rede local virtual» entre computadores; tenta direto e cai para Relay/TCP [R7][R3] | Ponto a ponto (`p2p`, o padrão) ou `server` (hub) [O23]; com `client-to-client` os clientes se veem [O11] | Cliente contra gateway: RRAS ou IKEv2 de terceiros [W1][W6][W7] | WG: pares por chave pública, sem papel de servidor no protocolo [L1]. strongSwan: cliente de acesso remoto ou gateway [S9]. NM: gerenciador, não é protocolo [N1] | Servidor (um OpenVPN por rede) **ou** P2P direto/repasse/auto [P:20][P:26] |
| **Depende de servidor ou conta do fornecedor?** | Servidores do fabricante: sim, «its servers belong to Famatech» [R8]. Conta de usuário: ND | Não: o servidor é seu. O AS é auto-hospedado [O19] | Não: o gateway é seu (RRAS) ou de terceiros [W6] | Não [L1] | Não; o intermediário é nosso [P:26] |
| **Criar rede / entrar com nome + senha** | **Sim**: botões «Create network» e «Join network», com nome e senha [R1] | Não: o cliente importa um perfil `.ovpn` [O18] | Não: em «Add VPN», escolhe-se servidor e protocolo [W8] | Não: troca de chaves públicas «como SSH» [L1] | **Sim**, no painel e no P2P [P:17][P:27] |
| **Convite por código ou link** | ND | ND (o perfil `.ovpn` faz o papel) [O18] | ND (a distribuição é por MDM/Intune com ProfileXML) [W6] | Fora do escopo, por desenho: «key distribution… out of scope» [L1] | Convite cifrado com a senha da rede, ficha de uso único [P:27] |
| **NAT / CGNAT** | Relay/TCP quando o direto falha [R3]. De quem é o relé: ND. Perfuração: ND | O servidor precisa ser alcançável; `--float` aceita troca de endereço [O14]. Perfuração: ND | NAT-T do IPsec; servidor atrás de NAT só com mudança no registro (Vista/2008) [W10]; cai para SSTP atrás de firewall ou proxy [W6] | WG: `PersistentKeepalive`, 25 s sugeridos, sem relé [L5]. strongSwan: NAT-T no núcleo do IKEv2 [S5] **e Mediation Extension**, perfuração mediada por servidor [S6] | Repasse próprio provado em CGNAT [P:26]; perfuração e «farol» **pendentes** [P:58] |
| **Protocolo e cifra** | «end-to-end 256-bit AES» [R8]; protocolo, modo e troca de chaves: ND | TLS no controle; dados em `AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305` [O1][O2] | IKEv2/IPsec com `GCMAES128/192/256` e DH `ECP256/384` configuráveis [W3]; SSTP, L2TP, PPTP [W1] | WG: Noise_IK, ChaCha20-Poly1305, Curve25519, BLAKE2s, HKDF [L2]. strongSwan: ESP padrão com AES-CBC/GCM [S4] | Noise_IKpsk2_25519_ChaChaPoly_SHA256, conferido contra vetor [P:21]; servidor com TLS 1.3/Ed25519 [P:41] |
| **Sigilo futuro (PFS)** | ND | Sim no modo TLS (DH) [O1]; o modo `--secret`, sem PFS, está **obsoleto** [O15] | `-PfsGroup` configurável; os 4 exemplos oficiais usam `PfsGroup None` [W3] | WG: chaves rotativas «for perfect forward secrecy» [L2]; troca aos 120 s e morte aos 180 s [L10]. strongSwan ESP: desde 6.0.2 o padrão inclui `none`, e o PFS fica **opcional** [S4] | Refaz o aperto aos 120 s e morre aos 180 s [P:23]. **Converge com o WG** [L10] |
| **Proteção contra repetição** | ND | Janela deslizante de 64 pacotes e 15 s [O3] | ND | WG: carimbo TAI64N no 1º pacote e janela da RFC 6479 [L3][L10]. strongSwan: `replay_window` 32 [S1] | Janela de 2.048 [P:23] |
| **Autenticação** | Nome e senha da rede [R1]; o resto é ND | Certificado de AC [O1]; usuário/senha por script [O7]; impressão digital sem AC [O24] | EAP-MSCHAPv2, EAP-TLS (TPM, cartão, Windows Hello), PEAP; L2TP com PSK [W2][W1] | WG: chave pública [L1]. strongSwan: EAP, PSK e certificado [N1] | Certificado Ed25519 próprio [P:14]; P2P por chave + PSK da senha [P:21] |
| **MFA / RADIUS / AD** | ND | Comunidade: `static-challenge` [O9] e plugins [O8]; RADIUS/LDAP **não** aparecem no manual. **AS**: LDAP/SAML/RADIUS [O19] e TOTP [O20] | RADIUS/NPS e MFA do Entra, acesso condicional [W6] | strongSwan: `eap-radius` [S3]. WG: fora do escopo [L1] | Não consta no Estado |
| **Revogar membro** | ND | CRL (`--crl-verify`) [O6]; metadados do `tls-crypt-v2` barram **antes do TLS** [O25] | RRAS barra certificado IKEv2 revogado **só** depois de `CertAuthFlags=4` no registro [W5] | strongSwan: CRL e OCSP ligados por padrão [S2]. WG: remover a chave do par é o modelo (implícito em [L1]); texto explícito: ND | CRL Ed25519, `ccd-exclusive`, `tls-crypt-v2` por membro [P:34][P:42]. P2P: rol **assinado** pendente [P:57] |
| **DoS no aperto** | ND | `tls-auth` é um «HMAC firewall» [O4]; `tls-crypt-v2 force-cookie`, mas o padrão continua `allow-noncookie` [O5] | ND | WG: não responde a quem não se autentica; mac1/mac2 e cookie sob carga [L3][L4]. strongSwan: `dos_protection yes`, cookies a partir de 30 IKE_SA meio-abertas [S1] | mac1/cookie: 107× medido [P:44]; repasse confere o HMAC **antes** do DH [P:86] |
| **Chat entre membros** | O EULA prevê mensagens «on our servers» [R9]; o recurso na tela: ND no help | ND | ND | ND | Chat por membro, dentro do túnel [P:30] |
| **Ping por membro e lista com estado** | Ping pelo menu de contexto; a rede mostra «its nodes» [R1][R2] | Interface de gerência no servidor [O10]; lista para o cliente: ND | ND | ND | Membros com estado e ping [P:29][P:30] |
| **USB pela rede** | ND | ND | ND | ND | USB/IP no Linux e no Windows [P:40][P:45]; prova com dispositivo real **pendente** [P:61] |
| **GUI / CLI / console** | Janela principal [R1]; CLI: ND | CLI `openvpn` (manual); OpenVPN GUI no Windows [O17]; Connect [O18]; AS com Admin Web UI [O19] | Configurações > VPN [W8]; PowerShell `Add-VpnConnection` [W9]; `Install-RemoteAccess` [W7] | WG: CLI «extremely limited» [L11]. NM: `nmcli` [N2] e diálogo de edição por plugin [N1] | Programa de mesa, painel web, CLI e `phxvpncmd` [P:18][P:19][P:29][P:33] |
| **Serviço do sistema / com o sistema / bandeja** | Serviço («Service not started») [R6]; auto-atualização [R7]; bandeja: ND | Windows: «system service daemon» [O17]; Connect antes do logon [O18] | Always On: conexão automática; túnel de dispositivo antes do logon [W6] | NM é daemon [N2]; WG: «no need to… manage daemons» [L1] | Serviço no Linux (systemd) e no Windows (SCM); bandeja no Windows [P:47][P:49][P:31]; bandeja num Windows real **não vista** [P:55] |
| **Plataformas** | Windows 7+, x86/x64/ARM64 [R4][R7] | Comunidade: Windows 10+ [O17]. Connect: Windows, macOS, Linux, iOS, Android [O18] | Só Windows; Always On «in all Windows editions» [W6] | WG: Windows, macOS, Linux, Android, iOS [L6]. strongSwan: Linux, Android, macOS; no Windows **não serve de cliente** com IP virtual [S8] | Linux e Windows [P:28]; Windows sem prova em máquina real [P:60] |
| **Código aberto / licença** | **Fechado**: engenharia reversa proibida [R9 §1.2]; usa Qt sob LGPL [R9 §8.1] | GPLv2 [O16]; o AS é por assinatura [O19] | Licença própria: ND nas páginas lidas | WG no kernel: GPLv2 [L7]. strongSwan: GPLv2 + licença comercial [S7]. NM: GPL-2.0+ / LGPL-2.1+ [N3] | `MIT OR Apache-2.0`, zero crate [P:658] |
| **Custo** | Grátis, «without ads or any paid features» [R7] | Comunidade: ND (licença livre). AS: 2 conexões grátis, depois assinatura [O19] | Incluso: Always On «in all Windows editions» [W6]; RRAS: ND | ND (licenças livres) | Decisão de produto: sobe ao dono |
| **Gerência central** | ND | Comunidade: só a interface de gerência local [O10]. AS: Admin Web UI e gestão de usuários [O19] | Intune, Configuration Manager, MDM e ProfileXML [W6] | WG: fora do escopo [L1]; strongSwan/NM: ND | Painel web + PostgreSQL [P:18][P:12] |
| **Limites documentados** | «does not limit the number of gamers» [R7]; membros por rede e redes por conta: ND | `--max-clients n` [O12]; AS grátis: 2 conexões [O19] | RRAS: ND. «Conexão de entrada» do Windows cliente: ND na documentação (só em fórum Q&A, fora da régua) | ND | 254 redes, 253 membros por rede [P:779]; 3 redes por usuário [P:39] |
| **Auditoria independente** | ND | QuarksLab/OSTIF, 2017, na 2.4.0: 1 alta (CVE-2017-7478), 1 média (CVE-2017-7479); revisão de M. Green [O21] | ND | WG: verificação formal com Tamarin e provas computacionais [L8]; auditoria de código: ND. strongSwan: ND | Só revisão adversária **interna** (2 críticos, 6 altos) [P:96] |
| **O que exige para instalar** | Adaptador próprio, «Famatech Radmin VPN Ethernet Adapter» [R5] | Windows: `ovpn-dco` (padrão), `tap-windows6` ou `wintun` [O13]. Linux: `ovpn-dco` ou o `tun` antigo [O26] | Nada: vem no sistema [W8] | WG: módulo do kernel desde a 5.6 [L9] ou `wireguard-go` [L11]. NM 1.16+: WG nativo, sem plugin [N1]; strongSwan pelo `NetworkManager-strongswan` [N1] | Linux: nada, TUN por `ioctl` [P:24]. Windows: TAP-Windows6 do instalador do OpenVPN [P:28] |
| *Pós-quântico / PSK opcional* (linha acrescentada) | ND | `tls-crypt`: pós-quântico «poor-man's» e sem PFS [O22] | ND | WG: slot de PSK para camada pós-quântica [L4]. strongSwan: ML-KEM (`mlkem1024`) nas propostas [S4] | PSK tirada da senha da rede (`psk2`) [P:21] |
| *Passa por TCP ou 443* (linha acrescentada) | Relay/TCP [R3] | `--proto tcp-client` / `tcp-server` [O27] | SSTP como queda [W6] | WG: **não** passa por TCP, «explicitly» [L4] | ND no Estado |
| *Jurisdição e registros* (linha acrescentada) | BVI; «We do not log, track, or collect» [R8] | não se aplica (é seu) | não se aplica | não se aplica | não se aplica (é seu) |

## 2. Convergências e divergências

A régua de peso da casa (PG/MariaDB/MySQL/SQLite) não se aplica aqui. Conto
só fontes, sem peso.

**Convergem os quatro, e o phxvpn também:**

- **AEAD moderno no plano de dados**: AES-GCM ou ChaCha20-Poly1305 [O2][W3][L2][S4]. O phxvpn usa ChaCha20-Poly1305 [P:21].
- **Janela contra repetição** onde está documentada [O3][L10][S1]. A do phxvpn (2.048) é maior que as de OpenVPN (64) e strongSwan (32) e está dentro do desenho da RFC 6479 [L10].
- **Revogar por CRL** onde há AC [O6][W5][S2]. O phxvpn também usa CRL [P:34].
- **Temporizadores de aperto iguais aos do WireGuard**: 120 s / 180 s / 2^60 [L10] = [P:23]. É convergência, e ela está declarada.

**Divergem, e a divergência pesa no posicionamento:**

- **Criar/entrar por nome + senha** só existe no Radmin [R1] e no phxvpn. OpenVPN, Windows e Linux distribuem arquivo ou chave. **É o nicho.**
- **Relé**: o Radmin tem (Relay/TCP) [R3] e o dono do relé é ND. strongSwan tem **perfuração mediada** (Mediation Extension) [S6]. WG e OpenVPN não trazem relé nem perfuração documentados. O phxvpn tem relé próprio, e a perfuração está pendente [P:58].
- **DoS com cookie ligado por padrão**: WG (sob carga) [L3] e strongSwan (`dos_protection yes`) [S1]. O OpenVPN tem, mas o padrão do `tls-crypt-v2` é `allow-noncookie` [O5]. Windows e Radmin: ND.
- **PFS obrigatório × opcional**: WG sempre [L2]. strongSwan ESP opcional desde 6.0.2 [S4]. Os exemplos do Windows usam `PfsGroup None` [W3]. Radmin: ND.
- **PPTP/L2TP**: o RRAS deixará de aceitar conexões **de entrada**, e o cliente continua podendo sair por eles [W4].

## 3. Fontes (código → URL → frase citada)

### Radmin VPN (versão do instalador em 24/09/2026: `Radmin_VPN_2.1.4951.1.exe`)

| Código | URL | Frase |
|---|---|---|
| R1 | https://www.radmin-vpn.com/help/ | «Press "Create network" button. Set Network name and Password.» / «press "Join network" button. Enter Network name and Password» / «The network created earlier and its nodes will be shown» |
| R2 | https://www.radmin-vpn.com/help/ | «You can check the connection by calling the "ping" command from the context menu of the remote computer.» |
| R3 | https://www.radmin-vpn.com/help/ | «A Relay/TCP connection is established when a direct connection cannot be established.» |
| R4 | https://www.radmin-vpn.com/help/ | «OS: Windows 7 and above» / «Processor: 1 GHz (х86 or x64 or ARM64 architecture)» |
| R5 | https://www.radmin-vpn.com/help/ | «find Adapter Description : Famatech Radmin VPN Ethernet Adapter» / «Trusted Network in 26.0.0.0/8 row» |
| R6 | https://www.radmin-vpn.com/help/ | «Radmin VPN shows "Service not started" status — Click the round button in Radmin VPN to start the service.» |
| R7 | https://www.radmin-vpn.com/ | «software product to create virtual local networks» / «completely free software, without ads or any paid features» / «can install its updates automatically» / «does not limit the number of gamers» / «Free Radmin VPN is trusted by 60 million users» / «Compatible with Windows 11, 10, 8, 7» |
| R10 | https://www.radmin-vpn.com/about/ | «Since launching Radmin VPN in 2016, Famatech has continued to develop and improve this program.» |
| R8 | https://www.radmin-vpn.com/security/ | «Radmin VPN uses end-to-end 256-bit AES encryption» / «Radmin VPN product and its servers belong to Famatech Corp. which is a BVI company» / «We do not log, track, or collect any of your traffic» |
| R9 | https://www.radmin-vpn.com/eula/ | §1.2 «the source code for the Product is proprietary» / §2.6 «the messages (including text, links, GIFs, emoji, photos)… store your messages on our servers» / §8.1 «Qt GUI Toolkit under the terms of GNU Lesser General Public License version 2.1» |

### OpenVPN (manual da 2.6.19: `https://github.com/OpenVPN/openvpn/tree/v2.6.19/doc/man-sections/`)

| Código | Arquivo / URL | Frase |
|---|---|---|
| O1 | `tls-options.rst` | «control channel that provides all of the security features of TLS, including certificate-based authentication and Diffie Hellman forward secrecy» |
| O2 | `protocol-options.rst` | «defaults to AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305 when Chacha20-Poly1305 is available» |
| O3 | `link-options.rst` (`--replay-window`) | «By default n is 64 (the IPSec default) and t is 15 seconds.» |
| O4 | `tls-options.rst` (`--tls-auth`) | «enables a kind of "HMAC firewall" on OpenVPN's TCP/UDP port» |
| O5 | `tls-options.rst` (`--tls-crypt-v2`) | «cookie based stateless three way handshake that avoids replay attacks and state exhaustion… The default is (currently) allow-noncookie» |
| O6 | `tls-options.rst` (`--crl-verify`) | «Check peer certificate against a Certificate Revocation List.» |
| O7 | `script-options.rst` | «Require the client to provide a username/password (possibly in addition to a client certificate)» |
| O8 | `plugin-options.rst` | «Multiple plugin modules may be loaded into one…» (o manual não nomeia RADIUS nem LDAP) |
| O9 | `client-options.rst` | «Enable static challenge/response protocol» |
| O10 | `management-options.rst` | «Enable a management server on a socket-name Unix socket… or on a designated TCP port» |
| O11 | `server-options.rst` | «each client will "see" the other clients which are currently connected» |
| O12 | `server-options.rst` | «Limit server to a maximum of n concurrent clients.» |
| O13 | `windows-options.rst` | «Values are ovpn-dco (default), tap-windows6 and wintun.» |
| O14 | `link-options.rst` (`--float`) | «tells OpenVPN to accept authenticated packets from any address» |
| O15 | `protocol-options.rst` (`--secret`) | «DEPRECATED Enable Static Key encryption mode (non-TLS)» |
| O16 | `COPYING` (tag v2.6.19) | «OpenVPN is distributed under the GPL license version 2» |
| O17 | https://openvpn.net/community/ | «installers are available with OpenVPN GUI client software and a system service daemon» / «Supported on all Windows platforms from Windows 10 onward» / «Download OpenVPN 2.7.7…» / «Tunnelblick — An open source macOS OpenVPN client» |
| O18 | https://openvpn.net/client/ | abas Windows/macOS/Linux/iOS/Android / «Pre-Login Connect support» / «a file that contains the specifics… called a profile and has an .ovpn file extension» |
| O19 | https://openvpn.net/as-docs/limitations-of-two-free-connections.html | **[AS]** «Two free VPN connections.» / «access to all Access Server features, including the Admin Web UI, user management, authentication methods (LDAP< RADIUS, SAML)» |
| O20 | https://openvpn.net/as-docs/tutorials/tutorial--turn-on-mfa.html | **[AS]** «Tutorial: Turn On TOTP Multi-Factor Authentication» |
| O21 | https://ostif.org/the-openvpn-2-4-0-audit-by-ostif-and-quarkslab-results/ | «OpenVPN 2.4.0, the NDIS6 TAP Driver for Windows, the Windows GUI, and Linux versions were evaluated» / «1 Critical/High Vulnerability CVE-2017-7478 / 1 Medium Vulnerability CVE-2017-7479» / «another security review performed by Dr Matthew Green» |
| O22 | `tls-options.rst` (`--tls-crypt`) | «provides "poor-man's" post-quantum security… (i.e. no forward secrecy)» / «All peers use the same --tls-crypt pre-shared group key» |
| O23 | `link-options.rst` (`--mode`) | «By default, OpenVPN runs in point-to-point mode (p2p). OpenVPN 2.0 introduces a new mode (server)» |
| O24 | `tls-options.rst` (`--peer-fingerprint`) | «allows the --peer-fingerprint to be used as alternative to a PKI» |
| O25 | `tls-options.rst` (`--tls-crypt-v2-verify`) | «allows server administrators to reject client connections, before exposing the TLS…» |
| O26 | `generic-options.rst` (`--disable-dco`) | «On Linux don't use the ovpn-dco device driver, but rather rely on the legacy tun module.» |
| O27 | `link-options.rst` (`--proto`) | «p can be udp, tcp-client, or tcp-server» |

### VPN nativa do Windows

| Código | URL | Frase |
|---|---|---|
| W1 | https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/vpn/vpn-connection-type | «IKEv2… L2TP… PPTP… SSTP» / «Automatic… attempts from most secure to least secure» |
| W2 | https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/vpn/vpn-authentication | «older and less-secure password-based authentication methods (which should be avoided)» / «EAP-TLS… Smart card certificates, Windows Hello for Business certificate» |
| W3 | https://learn.microsoft.com/en-us/powershell/module/vpnclient/set-vpnconnectionipsecconfiguration | «Accepted values: DES, DES3, AES128, AES192, AES256, GCMAES128, GCMAES192, GCMAES256, None» / exemplos «-PfsGroup None -DHGroup ECP384» |
| W4 | https://techcommunity.microsoft.com/blog/windowsservernewsandbestpractices/pptp-and-l2tp-deprecation-a-new-era-of-secure-connectivity/4263956 (08/10/2024) | «Windows RRAS Server (VPN Server) will not accept any incoming VPN connections based on these protocols» / «still remain available if you want to make outgoing VPN connections» |
| W5 | https://learn.microsoft.com/en-us/windows-server/remote/remote-access/how-to-always-on-vpn-block-clients-revoked-certificates | «configure RRAS server to block VPN clients that use a revoked IKEv2 certificate» / «reg add …\Ikev2 /v CertAuthFlags … /d "4"» |
| W6 | https://learn.microsoft.com/en-us/windows-server/remote/remote-access/vpn/always-on-vpn/ | «Always On VPN is available in all Windows editions» / «NPS extension for Microsoft Entra multifactor authentication» / «Fall back to SSTP from IKEv2… behind firewalls or proxy servers» / «Intune… any third-party mobile device management (MDM) tool» |
| W7 | https://learn.microsoft.com/en-us/windows-server/remote/remote-access/remote-access | «The VPN service uses the connectivity of the internet and a combination of tunneling and data encryption technologies» / `Install-RemoteAccess -VpnType RoutingOnly` |
| W8 | https://support.microsoft.com/en-us/windows/connect-to-a-vpn-in-windows-3d29aeb1-f497-f6b7-7633-115722c1009c | «select Network & internet > VPN > Add VPN» / «Select a VPN client and tunneling protocol» |
| W9 | https://learn.microsoft.com/en-us/powershell/module/vpnclient/add-vpnconnection | `Add-VpnConnection … -TunnelType "L2tp" … -L2tpPsk` |
| W10 | https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/configure-l2tp-ipsec-server-behind-nat-t-device | «By default, Windows Vista and Windows Server 2008 don't support… NAT-T security associations to servers that are located behind a NAT device» |

### Linux

| Código | URL | Frase |
|---|---|---|
| L1 | https://www.wireguard.com/ | «configure it with your private key and your peers' public keys» / «All issues of key distribution and pushed configurations are out of scope of WireGuard» / «exactly like exchanging SSH keys» / «no need to manage connections… manage daemons» |
| L2 | https://www.wireguard.com/protocol/ | «ChaCha20… Poly1305… Curve25519… BLAKE2s… HKDF» / «Noise_IK handshake» / «rotating keys for perfect forward secrecy» |
| L3 | https://www.wireguard.com/protocol/ | «the server does not even respond at all to an unauthorized client» / «we include a TAI64N timestamp in the first message» / «respond with a cookie reply packet» |
| L4 | https://www.wireguard.com/known-limitations/ | «explicitly does not support tunneling over TCP» / «abuse-resistant, by virtue of its use of mac1 and mac2» / «the pre-shared key parameter can be used to add a layer of post-quantum secrecy» |
| L5 | https://www.wireguard.com/quickstart/ | «A sensible interval that works with a wide variety of firewalls is 25 seconds.» |
| L6 | https://www.wireguard.com/install/ | «Windows [10, 11, 2016, 2019, 2022, 2025]», «macOS», «Ubuntu [module & tools]», «Android», «iOS» |
| L7 | https://www.wireguard.com/ | «The kernel components are released under the GPLv2» |
| L8 | https://www.wireguard.com/formal-verification/ | «formally verified in the symbolic model using Tamarin» / «Computational Proof of Protocol in eCK Model» |
| L9 | https://lists.zx2c4.com/pipermail/wireguard/2020-March/005206.html | «[ANNOUNCE] WireGuard 1.0.0 for Linux 5.6 Released» (30/03/2020) |
| L10 | https://www.wireguard.com/papers/wireguard.pdf | «Rekey-After-Messages 2^60… Rekey-After-Time 120 seconds, Reject-After-Time 180 seconds» / «RFC6479, which uses a larger bitmap» |
| L11 | https://www.wireguard.com/xplatform/ | «wireguard-go is quite functional» / «extremely limited command line interface» |
| S1 | https://docs.strongswan.org/docs/latest/config/strongswanConf.html | `cookie_threshold 30` «Number of half-open IKE_SAs… that activate the cookie mechanism» / `dos_protection yes` / `replay_window 32` |
| S2 | idem (`charon.plugins.revocation`) | `enable_crl yes` «Whether CRL validation should be enabled» / `enable_ocsp yes` |
| S3 | https://docs.strongswan.org/docs/latest/plugins/eap-radius.html | «redirects the EAP conversation with a client to a RADIUS backend server» |
| S4 | https://docs.strongswan.org/docs/latest/config/proposals.html | «aes128gcm16-aes192gcm16-aes256gcm16-noesn» / «Since 6.0.2… as well as none to make PFS optional» / «ML-KEM… (mlkem1024)» |
| S5 | https://docs.strongswan.org/docs/latest/features/natTraversal.html | «The IKEv2 protocol includes NAT Traversal (NAT-T) in the core» |
| S6 | https://docs.strongswan.org/docs/latest/swanctl/swanctlConf.html | `<conn>.mediation` «used to mediate other connections using the IKEv2 Mediation Extension» (desde 5.5.2) |
| S7 | https://www.strongswan.org/license.html | «distributed under the GPLv2 license» / «Commercial License» |
| S8 | https://docs.strongswan.org/docs/latest/os/windows.html · …/os/macos.html · …/os/androidVpnClient.html | «not usable as client for this particular scenario» / «strongSwan can be installed via Homebrew» / «installed directly from Google Play» |
| S9 | https://docs.strongswan.org/docs/latest/features/networkManager.html | «configure roadwarrior clients… supports connections using the IKEv2 protocol only» |
| N1 | https://networkmanager.dev/docs/vpn/ | «NetworkManager 1.16.0+ supports WireGuard natively and requires no plugin» / «NetworkManager-strongswan — …support for EAP, PSK and certificate authentication» / «NetworkManager-sstp — Unmaintained» |
| N2 | https://networkmanager.dev/docs/api/latest/nmcli.html · …/NetworkManager.html | «nmcli — command-line tool for controlling NetworkManager» / «NetworkManager — network management daemon» |
| N3 | https://gitlab.freedesktop.org/NetworkManager/NetworkManager/-/raw/main/README.md | «NetworkManager is free software under GPL-2.0-or-later and LGPL-2.1-or-later.» |

## 4. Lacunas

1. **Radmin é o mais opaco dos quatro.** Protocolo, modo do AES, troca de chaves, PFS, repetição, DoS, revogação, dono do relé, limite de membros e auditoria: todos ND. A única fonte sobre o chat é o EULA §2.6, e ela diz que as mensagens ficam **nos servidores da Famatech**. Fechar essas lacunas pede observar o binário no fio (`tcpdump` numa rede Radmin). Isso é bancada, não documentação, e o §1.2 do EULA proíbe engenharia reversa: medir o fio é permitido, abrir o binário não.
2. **Windows como «servidor» de conexão de entrada**: o limite de conexões simultâneas **não** está na documentação oficial. Só aparece em fórum, que fica fora da régua.
3. **OpenVPN 2.7** é o que a página da comunidade distribui hoje [O17]. Ninguém leu o manual da 2.7. O driver padrão no Windows [O13] pode ter mudado, e isso toca a decisão TAP-Windows6 do phxvpn.
4. **Auditoria de código do WireGuard e do strongSwan**: o site do WG cita só a verificação formal. Uma auditoria de implementação publicada, se existe, não foi achada em fonte primária.
5. **Relé do Radmin**: se é da Famatech ou de outro par não está escrito. A comparação «o relé do phxvpn é seu, o do Radmin é deles» continua sendo inferência, apoiada em [R8].

## 5. Recomendação (papel J)

Hipóteses escritas antes da leitura:
- **H1**: o phxvpn compete em **segurança** com OpenVPN e WG.
- **H2**: o phxvpn compete em **uso** com o Radmin, e a segurança é o diferencial.

**H1 morre como posicionamento.** Não é por falta de mérito: na criptografia
o phxvpn **converge** com o WireGuard (Noise IK, ChaCha20-Poly1305,
120/180 s), e convergir não diferencia. Os dois maduros ainda têm o que ele
não tem: auditoria independente ou verificação formal [O21][L8], MFA/RADIUS
[O19][S3][W6] e as cinco plataformas [L6][O18].

**H2 sobrevive.** Só Radmin e phxvpn documentam «criar rede / entrar com nome
e senha» [R1]. Contra o Radmin, o phxvpn tem com fonte o que o Radmin deixa
em ND: protocolo nomeado e conferido contra vetor, PFS, janela de repetição,
cookie contra DoS, revogação, código aberto, servidor e relé próprios, Linux.
O Radmin ganha em plataforma Windows provada (7+, ARM64), em maturidade
(no mercado desde 2016 [R10]; «trusted by 60 million users» [R7]) e em ter relé funcionando **e** caminho
direto através de NAT sem servidor do cliente.

Consequências para o plano, sem código:

1. **Corrigir o «Comparativo» do `PHXVPN.md`.** Ele se desmente em três
   pontos: diz «sem cliente de mesa», «P2P Windows: ainda não» e «6 achados
   altos», enquanto o Estado dá os três como feitos [P:29][P:28][P:35–39]. E a
   célula «Senha da rede no protocolo — OpenVPN: Não tem» está **errada** pela
   fonte: o `tls-crypt` é uma chave de grupo pré-compartilhada que barra o
   aperto [O22]. É o análogo, e o próprio phxvpn o usa no modo servidor.
2. **A perfuração P2P [P:58] é o que falta para empatar com o Radmin no NAT.**
   A referência mais próxima em fonte aberta é a **IKEv2 Mediation Extension
   do strongSwan** [S6]: servidor que só apresenta os pares, e os pares
   perfuram. Pede leitura do fonte antes de virar plano, pela lei da
   inspiração.
3. **MFA** fica como lacuna registrada, não como item. Os três maduros
   corporativos têm (AS, Windows, strongSwan). O Radmin, que é o concorrente
   de posicionamento, não documenta. Se entrar, é decisão de **produto**:
   sobe ao dono.
4. **Proposta recusada com fonte**: «relé do fornecedor, estilo Radmin». O
   phxvpn já decidiu pelo relé próprio [P:26], e a única vantagem documentada
   do modelo Radmin é não exigir servidor do cliente. O custo, também
   documentado, é que as mensagens passam pelos servidores do fornecedor [R9
   §2.6].

Nada aqui sobe ao dono, exceto o item 3 (produto).
