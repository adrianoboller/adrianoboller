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
- [x] Console `phxvpncmd` (ou `phxvpn cmd`), estilo prompt do MS-DOS: modos Painel, P2P e Ferramentas; lote por arquivo (`/entrada:`) e linha única (`/comando:`)
- [x] Segurança C2: sorteio falha fechado (descritor único; `BCryptGenRandom` no Windows) — nunca mais mistura previsível

### Falta

- [ ] Prova com o **túnel OpenVPN de verdade** — o binário `openvpn` não existe neste contêiner; o TLS foi provado com OpenSSL, o túnel não
- [ ] TLS no próprio painel (hoje HTTP; escuta 127.0.0.1 por padrão) — esbarra na pétrea de zero dependência
- [ ] Cliente de mesa tipo Radmin (ícone na bandeja, botão conectar) — hoje é o `.ovpn` + OpenVPN GUI/Connect
- [ ] Revogação por CRL (hoje: sair da rede apaga o `ccd/` e o `ccd-exclusive` barra)
- [ ] Usar o certificado digital da empresa (A1/RSA) como AC — hoje ele é guardado só como identificação
- [ ] Serviço do sistema (systemd / serviço do Windows) e pacote
- [ ] P2P: rol de membros assinado (Ed25519 da rede) e PSK da senha da rede
- [ ] P2P: descoberta — convite, broadcast na LAN e «farol» (membro alcançável que perfura NAT e faz relé)
- [ ] P2P: convite (`phxvpn p2p convidar`) e a tela
- [ ] P2P no Windows: TAP-Windows6 em modo TUN (CreateFileW + DeviceIoControl, adaptador próprio pelo `tapctl.exe` do OpenVPN) — ~250–350 linhas, estimado
- [ ] P2P: `mac1`/cookie contra inundação de INICIO (o WireGuard tem; aqui ainda não)
- [ ] Segurança A1: revogação real (série no CN, CRL Ed25519)
- [ ] Segurança A2/A3: limite de tentativas de login e de senha de rede; PBKDF2 fora do mutex; hash fictício contra enumeração
- [ ] Segurança A5: CSRF/DNS rebinding — exigir `Content-Type` JSON, conferir `Host`, código de instalação de uso único
- [ ] Segurança A6: cliente PG recusa senha em claro e exige o `SASLFinal` conferido
- [ ] Segurança M1–M7: teto de conexões e prazo por pedido, chave nascendo 0600, openvpn sem root e TLS 1.3, `--pg` fora do argumento, login só `[a-z0-9._-]`, cota de redes, reconexão do PG

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

## Comparativo

Legenda: **medido** = provado aqui; *citado* = documentação do fabricante,
não verificado por nós.

| | **phxvpn** | Radmin VPN | OpenVPN |
|---|---|---|---|
| Código | Nosso, `MIT OR Apache-2.0`, zero crate | Fechado (Famatech) | Aberto, GPLv2 |
| Topologia | Servidor **ou** P2P (direto / repasse / auto) | P2P com servidores do fabricante (*citado*) | Servidor central (hub) |
| Servidor próprio | Sim: painel + PostgreSQL, e o intermediário é nosso | Não: coordenação e relé são do fabricante (*citado*) | Sim |
| Sem servidor nenhum | Sim, no modo direto (LAN, IP público, IPv6) — **medido** | Não (*citado*) | Não (ponto a ponto só 1 par por processo) |
| CGNAT dos dois lados | Pelo nosso intermediário, cifrado de ponta a ponta — **medido** | Relé do fabricante (*citado*) | Pelo servidor |
| «Criar rede» / «entrar na rede» | Sim, nome + senha (painel); P2P por chave + senha | Sim, nome + senha | Não: arquivo de configuração por cliente |
| Cifra | Noise IKpsk2 X25519 + ChaCha20-Poly1305, **conferido contra vetor oficial**; modo servidor TLS 1.3 Ed25519 | AES-256 (*citado*) | TLS (OpenSSL) + AES-GCM/ChaCha |
| Senha da rede no protocolo | Vira PSK: sem ela o aperto não fecha — **medido** | Controle de acesso no servidor (*citado*) | Não tem |
| Cadastro | PostgreSQL (empresa, servidores, usuários, redes) | Nuvem do fabricante | Arquivos |
| Linux | Sim | Não (*citado*: só Windows) | Sim |
| Windows | Modo servidor: sim (OpenVPN). P2P: **ainda não** (TAP-Windows6, em escrita) | Sim | Sim |
| Vazão | 660 Mbit/s TCP pelo túnel P2P, uma corrida, contêiner — **medido** | Não medido por nós | Não medido por nós |
| O que ainda falta aqui | Painel com 6 achados altos de segurança; sem HTTPS no painel; sem cliente de mesa | — | — |

Números do Radmin e do OpenVPN não foram medidos aqui. A comparação justa de
vazão pede os três na mesma máquina e com o mesmo trabalho (regra da bancada).

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
