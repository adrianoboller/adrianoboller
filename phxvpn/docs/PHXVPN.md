# phxvpn — desenho, provas e o que falta

Rodada de 23/09/2026. Código em `phxvpn/src/`, testes em `src/*` e
`tests/postgres_real.rs`.

## Estado

Contagem das caixas abaixo (`grep -c '^- \[x\]'` / `'^- \[ \]'`).

### Feito

- [x] Cliente PostgreSQL de fio v3 com SCRAM-SHA-256 e protocolo estendido (parâmetro nunca vira SQL)
- [x] Esquema `phx_*` com toda FK `ON DELETE RESTRICT` (não se apaga rede com membro — provado contra o PostgreSQL 16)
- [x] AC Ed25519 própria; certificado de servidor (`serverAuth`) e de membro (`clientAuth`)
- [x] Cofre da senha mestre: chaves da AC, dos servidores e `tls-crypt` seladas no banco; a senha mestre não é gravada
- [x] Instalação com os 12 campos (empresa, finalidade, responsável, e-mail, telefone, admin, senha admin, senha mestre, servidor nome/IP/DNS, certificado)
- [x] Criar rede (sub-rede /24 e porta próprias) e entrar na rede (IP fixo, perfil `.ovpn` com tudo embutido)
- [x] Tela web (instalar, login, redes com membros e IP, administração de usuários e servidores)
- [x] Linha de comando `phxvpn entrar` / `criar-rede`, senha só por ambiente ou terminal
- [x] Supervisor opcional que sobe um `openvpn` por rede (`--openvpn`)
- [x] P2P: aperto `Noise_IKpsk2_25519_ChaChaPoly_SHA256` conferido byte a byte contra o vetor oficial (cacophony), no MESMO motor Noise do PhxSql
- [x] Segurança C1: JSON com teto de aninhamento (128) — o corpo de 262.000 `[` não derruba mais o processo
- [x] P2P: transporte UDP (contador explícito, janela de 2.048 contra repetição, refazer aos 120 s, morrer aos 180 s / 2^60, par surdo refaz aos 15 s)
- [x] P2P: placa virtual TUN no Linux por FFI ao `ioctl` — sem `iproute2`
- [x] P2P: `phxvpn p2p chave` e `phxvpn p2p ligar` — **ping entre dois computadores pelo túnel, provado**
- [x] P2P: servidor intermediário (`phxvpn repasse`) e modos `direto` / `repasse` / `auto` — provado numa topologia de CGNAT
- [x] P2P: `p2p criar` / `p2p convidar` / `p2p entrar` — convite cifrado com a senha da rede, ficha de uso único, malha que se aprende pela lista de pares dentro do túnel
- [x] P2P no Windows: placa TAP-Windows6 em modo TUN só com APIs do sistema; `p2p placa` cria o adaptador pelo `tapctl.exe` do OpenVPN
- [x] Programa de mesa (`phxvpn mesa`, `phxvpnw.exe` sem console): janela no estilo Radmin — criar, entrar por convite, convidar, ligar/desligar, membros com estado
- [x] Programa de mesa: ping e chat por membro (dentro do túnel) e «lembrar a senha» (DPAPI no Windows)
- [x] Programa de mesa: ícone na bandeja (Windows, `phxvpnw.exe --bandeja`) e «abrir com o sistema» (registro `Run` no Windows, autostart XDG no Linux)
- [x] Servidor intermediário com contas de usuário e senha (`phxvpn repasse conta`, `--contas`)
- [x] Console `phxvpncmd` (ou `phxvpn cmd`), estilo prompt do MS-DOS: modos Painel, P2P e Ferramentas; lote por arquivo (`/entrada:`) e linha única (`/comando:`)
- [x] Segurança A1: revogação real — série no CN, reentrada revoga o perfil anterior, CRL Ed25519 no `crl-verify`, admin/dono remove membro
- [x] Segurança A2/A3: tentativas limitadas (login, IP, usuário+rede); PBKDF2 fora da trava com semáforo; hash fictício contra enumeração
- [x] Segurança A4 (mínimo): painel só escuta fora do loopback com `--aceito-sem-tls`; CLI/console recusam painel remoto sem `PHXVPN_ACEITO_SEM_TLS=1`
- [x] Segurança A5: `Host` conferido (DNS rebinding), POST só com JSON (CSRF), código de instalação de uso único
- [x] Segurança A6: cliente PG recusa senha em claro, exige o SCRAM provado antes do «autenticado», teto de iterações
- [x] Segurança M1, M2, M4, M5, M6, M7: teto de conexões e prazo total por pedido; segredo nasce 0600 e diretório 0700; senhas saem do ambiente; login só `[a-z0-9._-]`; cota de 3 redes por usuário; conexão do PG refeita e trava envenenada não derruba
- [x] USB pela rede (USB/IP, porta 3240): compartilhar e usar no Linux só com `std` + sysfs; usar no Windows pelo usbip-win2; interopera com o `usbip` de referência
- [x] **Túnel OpenVPN de verdade provado** (2.6.19, `prova-openvpn.sh`): PostgreSQL → painel → dois membros em netns, TLS 1.3/Ed25519, ping entre membros, removido barrado
- [x] Segurança M3: OpenVPN troca para `nobody` depois de abrir a placa; `tls-crypt-v2` com uma chave por membro e a série dentro — removido barrado ANTES do TLS
- [x] Arquivos que o OpenVPN relê (`crl.pem`, `ccd/`) gravados por troca atômica — nunca lidos pela metade
- [x] P2P: `mac1`/cookie contra inundação de INICIO — lixo recusado em 1,85 µs em vez de 198 µs (107×); sob carga, só com cookie e 5/s por origem
- [x] USB na janela do programa de mesa: compartilhar, ver o dos membros, usar e soltar — exercitado com duas janelas e o túnel P2P de verdade
- [x] Segurança no Windows: arquivos com chave e pastas de dados só do dono (DACL protegida, uma entrada, posta ANTES do segredo) — provado no essencial sob o Wine; a prova estrita está no `prova-windows.ps1` (passo 3b)
- [x] Serviço do sistema no Linux (`phxvpn servico instalar painel|repasse|p2p`), segredos como credencial CIFRADA do systemd, nunca texto puro
- [x] Pacotes por roteiro (`empacotar.sh`): `.deb`, `.msi` e `.zip` — instalados e removidos de verdade (`dpkg`, `msiexec` do Wine)
- [x] Serviço do Windows (SCM por FFI, reinício em 5 s, registro em arquivo), segredos em DPAPI da máquina num arquivo só de SYSTEM e Administradores
- [x] Interface responsiva (CSS grid + flexbox + container queries) na janela e no painel — rolagem lateral zero medida em 390, 820, 1280, 1920 e 3440 px
- [x] Segurança C2: sorteio falha fechado (descritor único; `BCryptGenRandom` no Windows) — nunca mais mistura previsível

### Falta

- [ ] Programa de mesa: ver o ícone da bandeja num Windows real (no Wine ele é registrado, mas não aparece na área de trabalho virtual) e bandeja no Linux (pede D-Bus)
- [ ] Usar o certificado digital da empresa (A1/RSA) como AC — hoje ele é guardado só como identificação
- [ ] P2P: rol de membros ASSINADO (hoje a lista viaja cifrada entre membros, com confiança transitiva)
- [ ] P2P: descoberta — convite, broadcast na LAN e «farol» (membro alcançável que perfura NAT e faz relé)
- [ ] Segurança A4 (inteiro): TLS no próprio painel — choque com a pétrea de zero dependência; hoje, proxy com TLS na frente
- [ ] P2P no Windows: **prova numa máquina real** com OpenVPN (driver TAP e `netsh` — o roteiro `prova-windows.ps1` está pronto)
- [ ] USB: **prova com dispositivo real** (este contêiner não tem USB nem os módulos `usbip-host`/`vhci-hcd`)
- [ ] macOS, Android e iOS (OpenVPN e WireGuard têm; achado da validação de 24/09)
- [ ] Auditoria de segurança externa (OpenVPN teve em 2017, o WireGuard tem verificação formal; aqui só revisão interna)
- [ ] MFA / RADIUS / Active Directory no painel — **decisão de produto do dono** (Access Server, Windows e strongSwan têm)

## Portas e o controle de cada uma

| Porta | Quem abre | Controle |
|---|---|---|
| TCP 8470 | `phxvpn painel` | usuário + senha (PBKDF2), tentativas limitadas, `Host`/JSON/código de instalação |
| TCP 127.0.0.1:sorteada | `phxvpn mesa` | só a própria máquina + ficha da sessão (32 bytes por abertura) |
| UDP 51820 | nó P2P | chave do membro (Noise IK) + senha da rede (PSK) + ficha de convite para entrar |
| UDP 51821 | `phxvpn repasse` | **usuário + senha** com `--contas` (sem, fica aberto e avisa) |
| UDP 1195+ | OpenVPN (modo servidor) | certificado da AC + CRL + `ccd-exclusive` + `tls-crypt` |

### Contas do servidor intermediário

```text
servidor:  phxvpn repasse conta --usuario filial-a --contas contas.txt   (pede a senha)
           phxvpn repasse --contas contas.txt
nó:        phxvpn p2p criar ... --modo auto --repasse CHAVE@HOST:51821 --repasse-usuario filial-a
           phxvpn p2p ligar --rede ...        (pede a senha da rede e a do intermediário)
janela:    Ligar → «Servidor intermediário: usuário / senha» (só quando a rede o usa)
```

A senha nunca viaja nem fica no servidor: o nó deriva a credencial
`PBKDF2(senha, "phxvpn-repasse:"+usuário)` uma vez e prova por HMAC em cada
REGISTRO; o arquivo de contas guarda só a credencial (0600). A conferência
por HMAC vem **antes** do Diffie-Hellman — quem não tem conta não custa DH —
e os erros contam no mesmo limitador do painel, por usuário e por IP.

**Prova (24/09/2026, topologia de CGNAT, A e B só se alcançam pelo
intermediário):** sem conta 0/3; senha errada 0/3; senha certa **3/3**;
senha em claro no arquivo de contas: 0 ocorrências. RED: sem a conferência,
`contas_exigem_usuario_e_senha` reprova («sem conta registrou»).

## Segurança: a revisão de 23/09/2026 e o que fechou

Revisão adversária (papel SEC): 2 críticos, 6 altos, 7 médios, 7 baixos.
Fechados com prova, medida contra o painel rodando (24/09/2026):

| Achado | Ataque | Antes | Agora (medido) |
|---|---|---|---|
| C1 | 262.000 `[` num POST sem login | processo abortado | 400; JSON com teto de 128 níveis |
| C2 | esgotar descritores antes de sortear chave | bytes previsíveis | pânico, nunca byte previsível |
| A5 | site faz `fetch` `no-cors` para instalar | instalava | **415** |
| A5 | DNS rebinding (`Host: atacante.com`) | atendia | **421** |
| A5 | instalar sem o código do terminal | instalava | **403**; código vale uma vez |
| A2 | força bruta no login | sem limite | **429** a partir da 6ª falha (login ou IP), dobrando até 15 min |
| A2 | enumeração por tempo | 1 ms × 430 ms | existe 449–464 ms × não existe 440–447 ms |
| A2 | 10 logins paralelos param o painel | todas as rotas esperavam | `/api/estado` em 2,5–7,6 ms |
| A1 | notebook roubado; «sair» e «entrar» de novo | perfil velho voltava a valer | perfil velho **revoked** na CRL (OpenSSL) e sem `ccd/` |
| A6 | servidor PG falso pede senha em claro / pula a prova | entregava / aceitava | recusado; teto de iterações do SCRAM |
| M1 | slowloris (1 byte a cada 2 s) | thread presa por horas | **408** em 10 s de prazo total |

Defeito achado na própria medição: o hash fictício era calculado na primeira
tentativa de usuário inexistente, **dentro** da trava — o `/api/estado`
esperou 441 ms atrás de 10 logins. Agora é calculado ao ligar o painel.

RED: sem revogar na reentrada, o teste de ponta a ponta reprova («o ccd do
perfil velho tinha de sumir»); sem o `mac` do REGISTRO, o repasse desvia
tráfego. Limites que continuam: sessão já estabelecida com certificado
revogado só cai na próxima renegociação do OpenVPN (padrão 1 h); o painel
ainda é HTTP (A4 inteiro).

## Modos de uso

| Modo | Comando | Quando |
|---|---|---|
| **Servidor** | `phxvpn painel` + OpenVPN | Empresa com servidor próprio; cadastro no PostgreSQL; tudo passa pelo servidor |
| **P2P direto** | `phxvpn p2p ligar --modo direto` | Sem servidor nenhum: LAN, IP público, IPv6 ou NAT benigno |
| **P2P repasse** | `--modo repasse --repasse CHAVE@HOST:PORTA` | CGNAT dos dois lados: passa pelo nosso servidor intermediário, que só carrega pacote cifrado |
| **P2P auto** | `--modo auto --repasse …` | Tenta direto; sem resposta em 2 tentativas (~10 s), vai pelo intermediário |

Prova (24/09/2026, três `ip netns`; A e B só alcançam o intermediário, sem
rota entre si): direto 0/4 (esperado); repasse 4/4, 0,64 ms, 0 ocorrência do
texto enviado no fio do intermediário; auto 30/30, primeira resposta 10,5 s
depois de ligar. RED: sem conferir o `mac` do REGISTRO, um terceiro desvia o
tráfego de outro nó.

Na primeira medição do auto eu li o `icmp_seq=1` como «respondeu no primeiro
segundo». Não tinha respondido: o ping imprime a sequência original da
resposta que chega atrasada, e o nó guarda os pacotes na fila até o aperto
fechar. Medido com `ping -D` e o relógio de quando o nó foi ligado, são 10,5 s.

## Convite P2P (sem servidor)

```text
A:  phxvpn p2p criar --rede Matriz --ip 10.78.0.1/24
A:  phxvpn p2p convidar --rede Matriz --endereco 203.0.113.5:51820   -> phxvpn1.TWF0cml6.… (417 caracteres)
B:  phxvpn p2p entrar phxvpn1.TWF0cml6.…      (pede a senha da rede)
A, B:  phxvpn p2p ligar --rede Matriz
```

O código é cifrado com uma chave tirada da senha da rede (XChaCha20-Poly1305,
nome da rede como dado associado): sem a senha não abre, e um byte trocado
derruba a etiqueta. Dentro vai quem convida, o IP reservado e uma **ficha de
uso único**; o nó de quem convidou admite a chave desconhecida que apresentar
a ficha, e a ficha morre. Depois, cada nó manda a lista de pares por dentro
do túnel, e a malha se completa sozinha.

**Prova (24/09/2026, quatro `ip netns` numa LAN virtual):** B e C entraram
por convites de A; **B pinga C** sem nunca ter recebido o endereço de C
(primeira resposta 5,6 s depois de ligar os três); senha errada: «senha da
rede errada, ou convite adulterado»; convite gerado com A **já ligado** admite
D; o mesmo código com outra chave (D2) é recusado; convites abertos depois do
uso: 0. Os três arquivos `.p2p` nascem 0600.

Três defeitos achados nessa prova, nenhum pelos testes que já existiam:
1. A lista de pares só ia no reenvio de 30 s: o terceiro membro ficava até
   30 s recusado pelo segundo. Agora quem aprende par novo avisa a malha no
   próximo segundo.
2. O nó ligado guardava a rede em memória e **não via convite feito depois**
   — e, ao regravar, o apagava. Agora os convites são do disco.
3. Consertado o 2, a regravação **ressuscitava a ficha já usada**; o segundo
   uso só foi barrado por acaso (IP ocupado). Agora o nó lembra as fichas que
   consumiu. RED: sem esse filtro, `ficha_de_convite_admite_uma_vez_e_nao_ressuscita`
   reprova.

Limites: a lista de pares não é assinada — um membro (que já tem a senha)
pode apresentar outros; e `convidar` com a rede ligada não trava o arquivo
contra gravação simultânea (janela de milissegundos).

## P2P no Windows

`src/tun_windows.rs`: o adaptador sai do registro (`ComponentId=tap0901`), abre
`\\.\Global\{GUID}.tap` com `FILE_FLAG_OVERLAPPED`, `CONFIG_TUN` (0x220028) e
`SET_MEDIA_STATUS` (0x220018) por `DeviceIoControl`, endereço e MTU pelo
`netsh`. Uma vez, como administrador: `phxvpn p2p placa` (usa o `tapctl.exe`
que o OpenVPN instalou). As partes puras (códigos de IOCTL, `CONFIG_TUN`,
UTF-16) moram em `src/tap.rs` para se testarem em qualquer sistema.

**O que está provado (24/09/2026):**

| Prova | Resultado |
|---|---|
| Link real para `x86_64-pc-windows-gnu` (MinGW) | `phxvpn.exe` 2,47 MB, `phxvpncmd.exe` 2,42 MB |
| DLLs importadas | só do Windows: ADVAPI32, KERNEL32, bcrypt, bcryptprimitives, WS2_32, USERENV, msvcrt, ntdll, api-ms-win-core-synch — **nenhuma nossa** |
| `phxvpncmd.exe … AUTOTESTE` sob Wine | 4/4 OK (vetores oficiais) |
| Suíte do phxvpn compilada para Windows, sob Wine | **56/56** (inclui P2P por UDP real, repasse e convite) |
| Suíte do `phxsql-core` compilada para Windows, sob Wine | **379/379** — o `BCryptGenRandom` do conserto C2 roda de verdade |
| `p2p placa` sem OpenVPN | erro dito: «tapctl.exe nao encontrado: instale o OpenVPN 2.6+…» |

**O que NÃO está provado:** o driver TAP, o `netsh` e o túnel num Windows
real — o Wine não tem o driver. O `prova-windows.ps1` faz essa prova em cinco
passos (autoteste, placa, convite, ligar, ping), como administrador.

## Programa de mesa

`phxvpn mesa` (no Windows também `phxvpnw.exe`, subsistema GUI: dois cliques,
sem console). A janela lista as redes P2P deste computador; cada rede tem a
lâmpada (ligada/desligada), **Convidar** e **Ligar/Desligar**, e embaixo os
membros com bolinha de conectado, IP (clique copia), chave e caminho
(direto/repasse). Redes e identidade ficam em `%APPDATA%\phxvpn` ou
`~/.config/phxvpn`.

**Desenho (papel J):** a tela é servida só em `127.0.0.1`, numa porta
sorteada, e aberta como janela de aplicativo (`--app=`) do Edge, que todo
Windows 10/11 tem, ou do Chromium no Linux; sem eles, o navegador padrão.
Morreram: janela Win32 nativa (só Windows, milhares de linhas) e GTK
(biblioteca externa, choca com a pétrea). **Guarda:** além das do `web.rs`,
toda rota pede a **ficha da sessão** — 32 bytes sorteados a cada abertura,
que chegam à janela no fragmento da URL (`#f=`), que não vai ao servidor nem
ao Referer, e somem da barra de endereço depois de lidos.

**Um motor só:** criar, convidar, entrar e ligar chamam o `comandos.rs` —
o mesmo de `phxvpn p2p …` e do `phxvpncmd`; e o transporte HTTP (tetos,
prazo, `Host`, JSON) saiu do painel para o `web.rs`, usado pelos dois.

**Prova (24/09/2026):** duas janelas, cada uma num `ip netns`, operadas no
Chromium: A cria «Matriz», gera o convite pela janela e liga; B cola o
código (senha errada: «senha da rede errada, ou convite adulterado»), entra
e liga; **B vê A conectado** (bolinha verde, «direto 192.0.2.1:51820»);
ping de B a A pelo túnel 3/3; **Desligar** pela janela apaga a placa
(`Device "phx0" does not exist`). O nó ganhou desligar de verdade: a placa é
lida com `poll` (Linux) ou `WaitForSingleObject` + `CancelIoEx` (Windows),
em fatias de 0,5 s. `phxvpnw.exe`: `Subsystem 2 (Windows GUI)`.

Defeito achado ao exercitar: o rodapé mostrava texto de terminal («convide
com p2p convidar» e o caminho do arquivo). A janela agora fala a língua dela.

### Ping, chat e senha lembrada

**Ping** e **Chat** aparecem em cada membro conectado. Os dois são mensagens de
controle **dentro do túnel cifrado**: o ping mede a ida e volta pelo próprio
túnel, sem socket bruto de ICMP; o chat é texto UTF-8 de até 1.000 bytes, com
uma caixa de 200 mensagens por rede e aviso de «não lida» no botão.

**Lembrar a senha** guarda o segredo **derivado** (a PSK da rede e a
credencial do intermediário), nunca a senha. No Windows vai pela DPAPI
(`CryptProtectData`): só o mesmo usuário do Windows abre. No Linux, XChaCha
com uma chave local 0600, e o limite fica dito: protege contra copiarem só o
arquivo, não contra quem entra na conta do usuário. «Esquecer senha» apaga.

**Prova (24/09/2026, duas janelas em `ip netns`, Chromium):** ping de B em A
**0,3 ms**; chat nos dois sentidos, com «Chat (1)» de não lida; A liga com
«lembrar», desliga e **religa sem diálogo de senha**; «Esquecer» apaga o
arquivo; 0 erro de console. O teste do `lembrar` compilado para Windows rodou
sob o Wine: a DPAPI executou.

Dois defeitos achados nessa prova:
1. O `desligar` tirava a rede da lista antes de o nó parar: a janela dizia
   «desligada» com a porta UDP presa, e religar dava «Address already in use».
   Com par conectado, parar leva 989 ms. Agora a rede fica como «desligando»
   até o fim.
2. A lista era redesenhada inteira a cada 2 s, o que pode engolir um clique.
   Agora só redesenha quando algo muda. Esse era o primeiro palpite para o
   defeito 1, e estava errado: está registrado na cognição.

### Bandeja e «abrir com o sistema»

**Bandeja (Windows):** `phxvpnw.exe` põe um ícone ao lado do relógio, pintado em
código (círculo no vermelhão da marca, sem arquivo de recurso); duplo clique
abre a janela, o botão direito mostra «Abrir phxvpn / Sair». FFI a `user32`,
`shell32`, `gdi32`. **Abrir com o sistema** (Configuração, na janela): no
Windows, o valor `phxvpn` em `HKCU\…\CurrentVersion\Run` →
`phxvpnw.exe --bandeja` (sobe só na bandeja); no Linux,
`~/.config/autostart/phxvpn.desktop`. Só a conta do usuário, nada que peça
administrador.

**Prova (24/09/2026):** sob o Wine, o teste liga, lê e apaga o valor no
registro do Windows; a estrutura `NOTIFYICONDATAW` tem os 976 bytes do
Windows; o `phxvpnw.exe --bandeja` rodando numa área de trabalho virtual
(Xvfb) fica vivo no laço de mensagens, e o rastro do próprio Wine
(`WINEDEBUG=+systray`) mostra `Shell_NotifyIconW cbSize=976`, `add_icon
id=0x1` e `show_icon id=0x1`. **Não visto:** o ícone não apareceu na barra da
área de trabalho virtual do Wine 9; a prova visual e o clique no menu ficam
para um Windows real. Linux: a janela liga e desliga o autostart (o arquivo
nasce e some), 0 erro de console. Bandeja no Linux: não há (pede D-Bus, que é
biblioteca de fora).

Defeito achado ao capturar: **o texto da janela estava todo sem acento**
(«Configuracao», «codigo», «intermediario») — a lei da casa é identificador sem
acento, **texto de interface com acento**. Corrigido na janela e nas frases do
servidor que aparecem no rodapé; as prévias foram todas refeitas.

Prévias de todas as telas em `docs/previa/` (janela: redes, criar, ligar com
conta, entrar, conectado, convite; painel: instalação e redes; console).

## Console `phxvpncmd`

No molde do `vpncmd` do SoftEther: escolhe-se o modo e cada modo tem seus
comandos, com parâmetros no jeito do DOS (`/nome:valor`, sem diferença de
caixa).

```text
phxvpncmd                                      menu: 1 Painel, 2 P2P, 3 Ferramentas
phxvpncmd /modo:painel /painel:http://host:8470
phxvpncmd /modo:ferramentas /comando:"AUTOTESTE"      uma linha e sai
phxvpncmd /modo:painel /entrada:rotina.txt            lote: o .bat do phxvpn
```

| Modo | Comandos |
|---|---|
| Painel | `CONECTAR`, `LOGIN`, `LOGOUT`, `ESTADO`, `REDES`, `MEMBROS`, `CRIARREDE`, `ENTRARREDE`, `SAIRREDE`, `USUARIOS`, `USUARIONOVO`, `SERVIDORES`, `SERVIDORNOVO`, `DESTRANCAR` |
| P2P | `CHAVE`, `LIGAR` (túnel em segundo plano, volta ao prompt), `PARES` |
| Ferramentas | `AUTOTESTE` (Noise contra vetor cacophony, SCRAM contra RFC 7677, cofre, certificado), `BANCADA`, `GERARCHAVE` |
| Todos | `MODO`, `AJUDA`/`?`, `CLS`, `VERSAO`, `SAIR` |

**Um motor só:** o console não tem regra própria. Cada comando chama
`comandos.rs`, o mesmo código do `phxvpn` de linha de comando, e as duas
portas diferem só em como leem as opções (`--nome valor` × `/nome:valor`).
O lote **para no primeiro erro** com arquivo e linha (`rotina.txt:2: ...`),
e o código de saída é 1: lote que segue depois de falha faz estrago.

**Limites (os mesmos do `phxsqlcmd`):** sem histórico, sem setas, e a senha
digitada aparece na tela. Esconder o eco pede o terminal em modo cru, que é
uma crate. Para não digitar senha: `PHXVPN_SENHA`, `PHXVPN_SENHA_REDE`,
`PHXVPN_SENHA_MESTRE`, `PHXVPN_SENHA_NOVA`.

**Prova (24/09/2026):** rotina de implantação por lote contra o painel real
com PostgreSQL (login, estado, usuário novo, rede nova com perfil em 0600,
usuários, redes, membros, servidor novo), código de saída 0; lote com erro
parou na linha 2 com código 1; `LIGAR` + `PARES` pelo console em topologia
de CGNAT pelo repasse, com ping 3/3. `BANCADA` nesta máquina: cifra a 2.582
Mbit/s num núcleo e aperto completo em 1,14 ms. Compila para Windows
(`cargo check --target x86_64-pc-windows-gnu`, 0 aviso); não rodado lá.

## Modo servidor com o OpenVPN de verdade (24/09/2026)

`sudo ./prova-openvpn.sh` refaz tudo numa máquina Linux: PostgreSQL
descartável com SCRAM → `phxvpn painel --openvpn` → instalar → criar rede →
dois membros em netns com o `openvpn` 2.6.19 → ping → remover um → reconectar.

| Medido | Resultado |
|---|---|
| Canal de controle | TLS 1.3, `TLS_AES_256_GCM_SHA384`, certificado Ed25519 da nossa AC, troca X25519 |
| Canal de dados | AES-256-GCM |
| Membro → servidor / membro → membro | 0,6 ms / 0,8 ms, 0% de perda |
| Processo do servidor | `UID set to nobody`, `GID set to nogroup` |
| Membro removido reconectando | recusado **antes do TLS**: `TLS CRYPT V2 VERIFY SCRIPT ERROR` |

**tls-crypt-v2.** Com a v1, todo membro tem a mesma chave, e quem sai continua
podendo fazer o servidor abrir um TLS (a CRL só o barra depois). Agora cada
membro leva a própria chave, embrulhada pela do servidor, com `serie:<hex>`
dentro. O `openvpn` pergunta ao `phxvpn ovpn-v2-verificar`, e a série revogada
não passa. O verificador falha fechado: sem lista, sem metadados ou com a
série revogada, recusa.

Hipóteses do J:
- **Gerar o embrulho com AES escrito aqui: morreu.** O núcleo já recusou AES
  em software, por vazar tempo.
- **Deixar o próprio `openvpn` gerar: venceu.** O modo servidor já o exige.

Sem `openvpn` no PATH, a rede nasce com a v1 e o painel avisa. A CRL continua
valendo como segunda barreira.

**O aviso `CRL: cannot read CRL from file`.** Aparece a cada releitura da CRL,
seguido de `loaded 1 CRLs`. Três hipóteses, medidas num servidor isolado:
- **H1, leitura no meio da gravação: morreu** — o aviso sai mesmo só com
  `touch`;
- **H3, a nossa codificação: morreu** — o aviso sai igual com a CRL regravada
  pelo `openssl crl`;
- **H2, peculiaridade do OpenVPN ao recarregar: venceu.** É inofensiva: a
  revogação vale (`certificate revoked` no registro).

H1 caiu como causa, mas mostrou um risco real: o painel regravava o `crl.pem`
truncando o próprio arquivo, e um `openvpn` que o lesse pela metade ficaria
sem CRL. Agora o painel grava num temporário e renomeia; no Windows, que não
renomeia por cima de arquivo aberto, tenta de novo por até 1 s e, se não
der, dá erro. O teste falha com a gravação antiga (RED).

## P2P: `mac1` e cookie contra inundação (24/09/2026)

**Premissa medida antes de desenhar** (`BANCADA`, release, um núcleo):

| | Custo |
|---|---|
| Aperto IKpsk2 completo | 1,17 ms |
| INICIO de lixo recusado no X25519 (antes) | **198 µs** |
| INICIO de lixo recusado no `mac1` (agora) | **1,85 µs** |

Antes, cerca de 5.000 INICIOs de lixo por segundo (uns 6 Mbit/s) ocupavam um
núcleo inteiro, e no **mesmo fio** que carrega o tráfego do túnel. Agora o
lixo morre num HMAC.

**O desenho segue o do WireGuard (§5.4.4):**
- **`mac1`**: HMAC com chave derivada da **pública** do receptor. Quem não a
  conhece não chega ao X25519.
- **"Sob carga"**: acima de 20 INICIOs válidos por segundo, o receptor passa a
  exigir o **`mac2`**, feito com um cookie amarrado ao IP:porta de quem manda.
  O segredo do cookie gira a cada 120 s.
- **Resposta de cookie**: custa um HMAC e uma XChaCha. Quem forjou o IP não a
  recebe; quem a recebe prova o endereço.
- **Limite por origem**: com o cookie provado, cada origem abre no máximo 5
  INICIOs por segundo.

**Onde diverge do WireGuard, e por quê:**
- **HMAC-SHA256 truncado em 16 bytes, em vez de BLAKE2s**: o núcleo já tem o
  SHA-256 conferido contra a FIPS.
- **Só o INICIO leva os macs**: a RESPOSTA já morre na busca do índice
  pendente, antes de qualquer X25519.
- **Pelo repasse, o cookie se amarra à chave declarada**: ali o IP é sempre o
  do repasse.

**Provas:**
- `lixo_sem_mac1_morre_antes_do_x25519`: o contador colocado antes do X25519
  não anda.
- `sob_carga_exige_cookie_e_o_par_legitimo_passa`:
  - 40 INICIOs de um atacante: exatamente 20 chegam ao X25519, e o resto
    recebe cookie;
  - o par legítimo recebe o cookie, repete com o `mac2` e abre a sessão;
  - o dado dele atravessa.
- **RED dos dois**: sem a conferência do `mac1`, "lixo chegou ao X25519"; sem
  o regime de carga, "sob carga o X25519 tinha de parar".
- **Formato**: o INICIO cresceu 32 bytes. Nó antigo e nó novo não fecham
  aperto entre si; sem dado em produção, a mudança de formato entra agora.

## Windows: só o dono lê os segredos (24/09/2026)

`acl.rs` é o equivalente do 0600/0700 do Linux. A DACL fica com **uma**
entrada (acesso total ao usuário do processo), **protegida** para não herdar
nada da pasta de cima, e é posta **antes** de o segredo entrar no arquivo.
Onde vale:
- `gravar_secreto`: chave P2P, arquivo da rede, senha lembrada;
- `gravar` do painel: chave do servidor e `tls-crypt`;
- pastas de dados do painel e da janela; nelas a entrada é herdável.

**O que o Wine esconde, medido pelo rastro do servidor dele
(`WINEDEBUG=+server`):**
- **`SetNamedSecurityInfoW` lê a ACL e nunca a grava — e devolve sucesso.**
  Trocado por `SetFileSecurityW`, que chega ao `set_security_object` nos dois
  sistemas.
- **O Wine guarda a ACL como modo Unix e a sintetiza na leitura**: SYSTEM +
  dono, sem a marca de protegida. Então, sob o Wine, prova-se o essencial:
  antes, `S-1-1-0` (Todos) lia; depois, só o dono. RED: sem a chamada, Todos
  continua. A prova estrita (uma entrada, protegida) roda no Windows real,
  no passo 3b do `prova-windows.ps1`.
- **O Wine não implementa herança**: arquivo novo nasce do `umask`. Por isso
  cada segredo recebe a própria ACL, sem depender da herança da pasta.

## Interface responsiva e vídeo para investidor (24/09/2026)

**Interface.** A janela e o painel foram refeitos em **CSS grid + flexbox**,
com **container queries** dentro de cada cartão:
- **o esqueleto** da janela é uma grade de três faixas: cabeçalho, conteúdo e
  rodapé;
- **as redes** ficam em grade `auto-fit`: uma coluna no celular, várias no
  monitor, e cada cartão com teto de 900 px, para a largura extra virar mais
  cartão e não linha mais comprida;
- **um resumo** de indicadores em grade, contado dos mesmos dados da lista,
  vira coluna lateral a partir de 1100 px;
- **cada membro** é uma linha de grade, que empilha o caminho quando o
  **cartão** (e não a janela) é estreito;
- **no celular**, os diálogos viram folha de tela cheia;
- **no painel**, as redes ficam em grade e a tabela larga rola por dentro,
  sem empurrar a página.

**Medido** (prévias 18 e 19): rolagem lateral **zero** em 390, 820, 1280,
1920 e 3440 px, nas três telas do painel e na janela. Colunas da grade da
janela: 1 → 1 → 2 → 3 → 7. Nenhum erro de página.

**Vídeo** (`docs/video/phxvpn-investidor.mp4`, 3 min, 3,4 MB): refeito do
zero por `sudo provas/video/gravar.sh`.
- **O que se vê:** três computadores em `ip netns` (Matriz, Filial, Casa do
  diretor), cada um com a sua janela e o túnel P2P **de verdade**. A Matriz
  cria a rede e convida; a Filial entra e liga, lembrando a senha; os dois
  fazem ping e chat; a Filial usa o pendrive da Matriz; a Casa entra e já
  enxerga a Filial, porque a malha se forma sozinha.
- **Como é montado:** cada janela é gravada contínua, com as cenas marcadas;
  o `ffmpeg` corta as cenas e as intercala na ordem da história.
- **Os cartões** (abertura, console, responsividade, números) leem cada
  número de um arquivo medido — bancada, autoteste, este documento e o
  `CIFRA-DO-FIO.md` do PhxSql. O percentual de estado sai da contagem das
  caixas acima.
- **Limite dito:** o USB do vídeo usa um sysfs de mentira, porque não há
  pendrive no contêiner. O protocolo e o túnel são os de verdade; a troca de
  driver que o kernel faria, o roteiro faz.

## Serviço do sistema e pacotes (24/09/2026)

**Serviço (Linux, systemd).** `phxvpn servico instalar painel|repasse|p2p`
escreve a unidade e a liga; `--mostrar` só a imprime.

**O choque com pétrea, resolvido sem subir ao dono.** Serviço não tem quem
digite senha, e "senha nunca em texto puro, nem em arquivo" proíbe o `.env`
de costume. A saída é o mecanismo do próprio systemd:
- `systemd-creds encrypt` guarda o segredo **cifrado** em `/etc/phxvpn/*.cred`
  (chave do host, ou TPM quando há), recebendo-o pela entrada padrão, e não
  pela linha de comando;
- o systemd só o decifra em memória, para o processo (`LoadCredentialEncrypted`).

O que cada serviço recebe:
- **painel**: a conexão do PostgreSQL e, se pedida, a senha mestre;
- **nó P2P**: a PSK **derivada** (não a senha), lida do nome gravado no
  arquivo da rede — um nome digitado com outra caixa daria um túnel que nunca
  fecha, sem aviso.

| Serviço | Roda como | Endurecimento |
|---|---|---|
| painel | root (o `openvpn` cria a placa e desce a `nobody`) | só `/dev/net/tun`, capacidades mínimas, `ProtectSystem=strict` |
| repasse | `DynamicUser`, **sem root** | nenhuma capacidade, sem dispositivos |
| p2p | root só com `CAP_NET_ADMIN` | só `/dev/net/tun` |

**Provas:**
- as três unidades passam no `systemd-analyze verify` sem aviso nenhum, e
  nenhuma contém segredo;
- a credencial cifrada (223 bytes) não contém o texto da senha;
- o painel sobe **só** com a credencial (sem `PHXVPN_PG`) e responde
  `/api/estado`; sem credencial e sem variável, recusa com a frase certa.
- **Não provado aqui**: o serviço rodando sob o systemd (o contêiner não tem
  systemd como PID 1).

**Pacotes** (`./empacotar.sh`, nunca à mão):

| Pacote | Tamanho | Prova |
|---|---|---|
| `phxvpn_0.1.0_amd64.deb` | 809 KB | `dpkg -i` → `phxvpn versao` e `AUTOTESTE` ok → `dpkg -r` limpo |
| `phxvpn-0.1.0-x64.msi` | 3,8 MB | `msiexec /i` do Wine → `phxvpn.exe versao` ok → `msiexec /x` limpo |
| `phxvpn-0.1.0-windows-x64.zip` | 3,6 MB | — |

O MSI não põe o phxvpn no PATH: o `wixl` não conhece a tabela `Environment`.
O `UpgradeCode` é fixo, e é por ele que o Windows reconhece a versão nova
como atualização.

**Windows.** O mesmo comando (`phxvpn servico instalar painel|repasse|p2p`)
fala com o gerenciador de serviços por FFI (`advapi32`):
- o serviço roda `phxvpn.exe servico-rodar <unidade> …` como SYSTEM;
- se cair, religa em 5 s;
- a saída vai para `%ProgramData%\phxvpn\<unidade>.log`, porque serviço não
  tem console;
- um pânico para o serviço **avisando** o gerenciador de serviços.

O equivalente do `systemd-creds`:
- o segredo vai selado pela **DPAPI da máquina**;
- num arquivo que só SYSTEM e Administradores leem (`acl::so_do_sistema`);
- a DPAPI saiu do `lembrar.rs` para `dpapi.rs`, um motor só para os dois
  escopos.

**Provado sob o Wine:**
- **repasse**: `RUNNING`, escutando em 1 s, registro com a chave; remover
  fecha a porta em 1 s;
- **painel** (conexão do PostgreSQL **só** na credencial DPAPI): no ar em 8 s,
  responde `/api/estado`; a senha não aparece no registro nem na credencial;
  remover apaga a credencial.

**O que o Wine faz e o Windows não:** quando o último processo de usuário
sai, ele derruba os serviços. Na prova, um processo à toa segura o Wine,
como uma máquina ligada seguraria o Windows.

## USB pela rede (24/09/2026)

Pedido do dono: «as USB podem ser compartilhadas?». Decisão dele no mesmo dia:
o Windows **pode quebrar a pétrea** de zero dependências para isso.

**Hipóteses (papel J), antes de medir:**

| | Hipótese | Resultado |
|---|---|---|
| H1 | USB/IP do kernel Linux; parte de usuário escrita aqui; Windows pelo usbip-win2 | **venceu** |
| H2 | Encaminhar URBs em espaço de usuário (libusb) | morreu: biblioteca de fora nos dois lados E, do lado de quem usa, ainda precisaria de driver para o sistema ver o dispositivo |
| H3 | Produto pronto (VirtualHere e similares) | morreu: licença paga por servidor e binário fechado |

O que H1 comprou: **no Linux, nenhuma dependência nova.** A parte de usuário do
USB/IP é pequena (duas mensagens de combinado) e o resto é o kernel; falamos
com ele pelo sysfs, como o `usbip`/`usbipd` de referência. A exceção autorizada
ficou só no Windows, e só do lado de quem **usa**: o usbip-win2 não tem o lado
servidor.

| | Linux | Windows |
|---|---|---|
| Compartilhar um dispositivo | sim (root, `modprobe usbip-host`) | não (o usbip-win2 não serve) |
| Usar o de um membro | sim (root, `modprobe vhci-hcd`) | sim, com o [usbip-win2](https://github.com/vadimgrn/usbip-win2/releases) instalado |
| Ver o que um membro oferece | sim | sim (código nosso, sem o usbip-win2) |

```text
phxvpn usb listar
phxvpn usb compartilhar 1-1.2 --rede Matriz     # sai deste computador
phxvpn usb remotos 10.78.0.1                    # no outro membro
phxvpn usb usar 10.78.0.1 1-1.2                 # aparece como espetado aqui
phxvpn usb portas / soltar N / parar 1-1.2 --rede Matriz
```

O servidor sobe sozinho com o `p2p ligar` (e com o «Ligar» da janela). Sem
rede P2P (modo OpenVPN), roda avulso: `usb servir --ip IP --interface tun0
--permitir 10.8.0.0/24 --busid 1-1`.

**Na janela** (botão **USB** de cada rede, prévias 15–17): "Deste computador"
(Compartilhar / Parar), "Dos membros" (Ver USB → Usar) e "Em uso aqui"
(Soltar). As três portas (janela, console e linha de comando) chamam as mesmas
funções do `usb.rs`; só a formatação muda.

Exercitado com `provas/usb-janela/rodar.sh`:
- duas janelas no Chromium, cada uma num `ip netns`, com o túnel P2P de
  verdade e um sysfs de mentira em cada lado;
- **o que o kernel faria, o roteiro faz à mão:** trocar o driver depois do
  `bind` e marcar a porta usada.

O que se mediu:
- A compartilhou, e o sysfs recebeu `unbind` do `usb-storage`,
  `add 1-1` e `bind`;
- B viu o pendrive de A **pelo túnel**, clicou Usar, e então:
  - A entregou o soquete ao `usbip-host`;
  - B escreveu `0 11 65541 3` no `attach`;
  - depois, Soltar escreveu `0` no `detach`;
- nenhum erro no console das duas páginas.

Dois defeitos achados só exercitando:
- quem só compartilha via o erro cru "vhci-hcd não carregado" em "Em uso
  aqui". Não ter o módulo é o normal nesse caso; agora a tela diz como usar;
- as mensagens do motor iam sem acento para a tela.

**Segurança.** O USB/IP não tem senha nem cifra; quem alcança a porta lê o
pendrive. Três travas, cada uma provada:

1. **Presa à placa da rede** (`SO_BINDTODEVICE`). Escutar só no IP virtual
   **não basta**, e isso está medido: em netns, um servidor sem a trava no
   `10.99.0.1:3240` foi alcançado **pela LAN** (modelo de host fraco do Linux).
   Com a trava, a LAN recebe `Connection refused` e a placa da rede é
   atendida. Sem `--interface`, o `usb servir` recusa subir.
2. **Só membro**: o IP de quem conecta tem de ser de um par da rede (relido a
   cada conexão). Estranho não recebe nem um byte. No P2P, a origem dentro do
   túnel já foi conferida contra a chave de quem mandou.
3. **Só o compartilhado nesta rede**: preso ao `usbip-host` não basta; o busid
   tem de estar no `usb` do arquivo da rede. E o busid que vem da rede é
   validado antes de virar caminho no sysfs (`../1-1` é recusado).

**Provas (24/09/2026):**

- 8 testes contra um sysfs de mentira: 312 bytes nos deslocamentos da norma,
  lista sem hub, só o compartilhado, estranho sem resposta, soquete entregue
  ao `usbip-host`, recusa de em uso/torto, `bind`/`unbind` com as mesmas
  escritas do `usbip`, porta livre do hub certo (`hs`/`ss`) no `vhci_hcd`.
- **Interoperação**: o `usbip list -r` de referência (usbip-utils 2.0) lê o
  nosso servidor e mostra fabricante, produto e a interface «Mass Storage /
  SCSI / Bulk-Only» (`PHXVPN_USBIP=… cargo test o_usbip_de_referencia`).
- **RED**: tirar o byte de preenchimento da interface derruba o teste de
  interoperação; abrir a admissão derruba o do estranho.
- `phxvpn.exe usb remotos` (Windows, sob o Wine) lê o servidor Linux.
- **Não provado**: anexar um dispositivo real. Este contêiner não tem USB nem
  os módulos do kernel; o `usar`/`compartilhar` com hardware fica para uma
  máquina de verdade.

## Comparativo (revisto em 24/09/2026)

Resumo; cada célula dos concorrentes, com a fonte primária e a frase citada,
está em [`propostas/matriz-vpns-2026-09-24.md`](propostas/matriz-vpns-2026-09-24.md)
(108 células; 62 com fonte, 26 «não documentado»). **medido** = provado
aqui; os demais são documentação do fabricante.

| | **phxvpn** | Radmin VPN | OpenVPN 2.6 | VPN do Windows | VPN do Linux (WireGuard / strongSwan) |
|---|---|---|---|---|---|
| Criar/entrar com nome + senha | **Sim** (P2P e painel) — medido | Sim | Não: arquivo por cliente | Não: perfil por conexão | Não: chave/arquivo por par |
| Sem servidor nenhum | **Sim** (modo direto) — medido | Não: servidores do fabricante | Não (ponto a ponto é 1 par por processo) | Não | WireGuard sim, por par configurado à mão |
| CGNAT dos dois lados | Relé **próprio**, cifrado ponta a ponta — medido | Relé **do fabricante** | Pelo servidor | Pelo servidor | Pelo servidor; strongSwan tem mediação IKEv2 |
| Perfuração de NAT (hole punching) | **Ainda não** (usa o relé) | Sim, e cai para relé | Não | Não | Não no WireGuard |
| Cifra documentada | Noise IKpsk2 + ChaCha20-Poly1305, vetor oficial — medido | só «AES 256-bit» | TLS + AES-GCM/ChaCha | IKEv2/SSTP/L2TP | Noise IK (WG) / IKEv2 (strongSwan) |
| Barreira antes do aperto | PSK da rede + mac1/cookie — medido | não documentado | `tls-crypt`/`tls-crypt-v2` (chave de grupo) | cookies do IKEv2 | cookies (WG) / IKEv2 |
| Revogar um membro | CRL + barreira pré-TLS (v2) — medido | não documentado | CRL | certificado/AD | remover a chave |
| Chat e ping por membro | **Sim** — medido | Sim | Não | Não | Não |
| USB pela rede | **Sim** (USB/IP) — medido no protocolo | Não | Não | Não | Não (usbip à parte) |
| Serviço / inicia com o sistema | systemd e SCM — medido | Sim | Sim | Embutido | Embutido |
| Linux / Windows | Sim / Sim (Windows real a provar) | Não / Sim | Sim / Sim | — / Sim | Sim / WireGuard sim |
| macOS, Android, iOS | **Não** | Não | Sim | — | WireGuard sim |
| MFA / RADIUS / AD | **Não** | Não | Access Server / plugins | Sim | strongSwan sim |
| Auditoria externa | **Não** (revisão interna) | não documentada | Sim (2017) | Microsoft | WG: verificação formal |
| Código | Aberto, zero crate | Fechado | Aberto (GPLv2) | Fechado | Aberto |

**Bancada comparativa** (`bancada/comparativo/medir.sh`, 2026-09-24, 5 corridas,
4 nucleos, 6.18.44-fc-v37; mesma topologia de dois netns, `iperf3` TCP de 5 s; tudo em espaço
de usuário, porque o kernel daqui não tem o módulo do WireGuard nem o DCO do
OpenVPN):

| Túnel | Conectar | Mbit/s mín – mediana – máx | Ping |
|---|---|---|---|
| **phxvpn** (ChaCha20-Poly1305) | 0,01 s | 624 – **651** – 664 | 0,498 ms |
| WireGuard-go (ChaCha20-Poly1305) | 0,01 s | 655 – **662** – 731 | 0,739 ms |
| OpenVPN AES-256-GCM | 2,33 s | 645 – **670** – 693 | 0,387 ms |
| OpenVPN ChaCha20-Poly1305 | 2,33 s | 426 – **477** – 514 | 0,515 ms |

Pela regra das faixas (só há vencedor quando elas não se cruzam):
- **Com a mesma cifra, o phxvpn vence o OpenVPN**: as faixas estão separadas.
- **Contra o WireGuard-go e o OpenVPN com AES, é empate dentro do ruído**: as
  faixas se cruzam.
- **Conectar**: o P2P fecha no primeiro pacote; o OpenVPN leva 2,3 s no
  aperto TLS.
- Radmin e a VPN do Windows não rodam aqui: não medidos.

**Uma conclusão minha que caiu:** com a primeira corrida de 3, eu escrevi aqui
que o phxvpn ficava «~6% atrás do WireGuard» e que «o AES vence os dois».
A segunda corrida de 3 já cruzou as faixas, e a de 5 confirmou o empate. Com
três amostras, a diferença estava dentro do ruído — a mesma lição do pedido
155 do PhxSql (vencedor declarado dentro do ruído).

## Modo P2P (decisão do papel J, 23/09/2026)

Vence o **plano de dados próprio no estilo WireGuard**: UDP + Noise IKpsk2 +
TUN por FFI. Morreram, com o motivo:

- **OpenVPN ponto a ponto** — um processo e uma interface por PAR: 253 membros
  dão 31.878 túneis, sem a /24 única que imita a LAN, sem descoberta e sem
  perfuração de NAT.
- **WireGuard do sistema** — o kernel é dono da porta UDP, então a perfuração
  de NAT teria de sair de outra porta e não serve.
- **DHT pública** — depende de nós de terceiros e vaza quem é membro de quê.

Medido pela pesquisa: a cifra do núcleo faz 2.525–2.661 Mbit/s num núcleo
(pacote de 1.420 B, só a cifra); X25519 custa 86,4 µs, e o aperto, cerca de
0,35 ms (conta feita sobre o número medido).

Onde diverge do WireGuard, e por qual restrição nossa: SHA-256 no lugar do
BLAKE2s (só primitiva já conferida no núcleo); admissão por rol assinado (não
há servidor para distribuir a lista); PSK tirada da senha da rede (o produto
é «rede com senha»); farol é um membro, não um servidor nosso.

**Limite físico, com fonte (RFC 5128 §3.3.1 e §5.1; RFC 8445 §2):** sem
servidor nenhum, o P2P conecta na mesma LAN, com um membro alcançável (IP
público, IPv6 ou UPnP/PCP) ou com NATs benignos dos dois lados. **Dois lados
atrás de CGNAT/NAT simétrico, sem membro alcançável, não conectam sem um
terceiro que repasse** — é por isso que Tailscale (DERP), ZeroTier (roots) e
Nebula (lighthouse) têm um.

**Decidido pelo dono (23/09/2026):** (1) Windows usa o driver que o OpenVPN já
instala; (2) CGNAT dos dois lados passa por repasse num servidor nosso, que
só carrega pacote cifrado de ponta a ponta.

**Windows (papel J, depois da decisão):** vence o **TAP-Windows6 em modo TUN**
— o único driver presente no instalador do OpenVPN 2.6 e no 2.7, em toda
arquitetura. O Wintun morreu (saiu do OpenVPN 2.7, não existe em arm64 e,
na 0.8.1, só abre para SYSTEM); o ovpn-dco-win também (é acoplado ao
protocolo do OpenVPN). Consequência de produto: exigir OpenVPN 2.6+ com o TAP
marcado (vem por padrão) e criar o adaptador do phxvpn uma vez, com
elevação, pelo `tapctl.exe` do próprio OpenVPN — o TAP é exclusivo, não se
divide com o openvpn. Nada disso foi rodado num Windows ainda.

### Prova P2P de ponta a ponta (23/09/2026)

Dois espaços de rede Linux isolados (`ip netns`), ligados por um cabo
virtual que faz o papel da internet; um `phxvpn p2p ligar` em cada um.

| Prova | Resultado |
|---|---|
| `ping` pelo túnel | 4/4, 0,68 ms médio; B não sabia o endereço de A e o aprendeu pelo aperto |
| O que passa no cabo (`tcpdump`) | 6 pacotes UDP, **0** ocorrências do texto enviado; dentro da placa de B, 2 de 2 |
| Vazão TCP pelo túnel | **660 Mbit/s** (247,6 MB em 3 s, release, uma corrida, dois núcleos virtuais) |
| Senha da rede errada | 3/3 pacotes perdidos |
| B reinicia (perde as chaves) | A se recupera sozinho em ~15,5 s — **defeito achado aqui**: antes esperaria 120 s |
| RED das guardas | sem conferir a origem, o par forja IP de outro; sem o carimbo, INICIO gravado abre sessão |

## Decisões, com a hipótese que morreu

**Uma instância de OpenVPN por rede** (porta `1194+N`, sub-rede `10.77.N.0/24`).
Hipótese rival: uma instância só, isolando redes por regra de firewall. Morreu
porque o isolamento passaria a depender de `iptables`/`netsh` escrito certo em
cada sistema; com processos separados, rede A e rede B não têm interface em
comum. `client-to-client` dentro da rede faz ela parecer LAN — o comportamento
do Radmin.

**Ed25519, não RSA.** O OpenVPN 2.6 com OpenSSL 1.1.1+ aceita, e o Ed25519 já
está conferido contra a RFC 8032 no `phxsql-core`. RSA pediria aritmética de
2048 bits e geração de primos — mais código de risco para o mesmo serviço.
Preço: cliente com OpenSSL anterior a 1.1.1 não conecta.

**`ccd-exclusive`.** Só conecta quem tem arquivo em `ccd/`, que o painel
escreve na entrada e apaga na saída. Tirar alguém da rede não depende de CRL.

**Senha mestre separada da senha admin, e recusada se igual.** Quem descobre a
de login não destranca o cofre. O painel reiniciado nasce **trancado** até o
admin informar a senha mestre (tela ou `PHXVPN_SENHA_MESTRE`).

**Chave privada do membro não fica no servidor.** Nasce no «entrar», vai no
perfil e some; baixar de novo emite par novo e mantém o IP.

## Provas (23/09/2026)

| Prova | Resultado |
|---|---|
| SCRAM contra o vetor da RFC 7677 §3 | prova e assinatura do servidor idênticas; assinatura torta recusada |
| PostgreSQL 16 real (`tests/postgres_real.rs`) | 3/3: senha errada → `28P01`; `'; DROP TABLE` volta como texto; ciclo instalar → criar → entrar → reiniciar → destrancar → sair |
| `openssl verify -purpose sslclient` no membro | OK |
| `openssl verify -purpose sslserver` no servidor | OK |
| `openssl verify -purpose sslserver` no **membro** | recusado — é a guarda do `remote-cert-tls server` |
| **RED:** a mesma verificação com a EKU removida do código | membro **passa** como servidor → a guarda é a EKU, provada nos dois sentidos |
| `s_server`/`s_client` com certificado mútuo | TLSv1.3, `Peer signature type: Ed25519`, `Verify return code: 0` |
| Reinício do painel | cofre trancado; senha mestre errada recusada; certa destranca e reescreve `ccd/` |
| Tela no Chromium (1280 e 390 px) | instalar → login → criar rede → download `phxvpn-Matriz.ovpn`; 0 erro de console; sem rolagem lateral |
| `clippy --all-targets` | 0 aviso |

Rodar o teste real: `PHXVPN_PG_TESTE="host=… port=… user=… password=… dbname=postgres" cargo test`.
Sem a variável, os três dizem «NAO RODOU» em vez de passar calados.

## Limites que valem saber antes de usar

- **Painel em HTTP.** Senha de login e perfil (com a chave privada) passam em
  claro. Deixe em `127.0.0.1` (padrão), atrás de proxy com TLS, ou acesse pela
  própria VPN. O `phxvpn` avisa ao escutar fora de 127.x.
- **Chave do servidor em disco.** O OpenVPN lê `servidor.key` e `tls-crypt.key`
  sem pedir senha; ficam com permissão 0600.
- **Limites de tamanho:** 254 redes por instalação, 253 membros por rede.

![Instalação](tela-instalar.png)
![Redes](tela-redes.png)
