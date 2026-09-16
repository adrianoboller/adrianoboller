# Transporte P2P do pilar 2 (e-mail P2P) — pesquisa medida contra o nosso gargalo

**Papel:** J (pesquisador), subagente `pesquisa-rede`. **Data:** 16/09/2026.
**Item:** `P2-DESIGN` do `docs/pmo/BACKLOG.md`. **Estado:** pesquisa — **não é
plano, não há código, nada foi gravado, nada foi compilado.**

No mesmo contrato do `CASSANDRA.md` e do `P2P-DISTRIBUIDO.md`: toda afirmação
técnica traz a fonte primária (RFC, paper, especificação aberta ou fonte do
projeto), com URL; todo número diz **quem mediu e quando**; onde não achei
número publicado, está escrito «não achei número publicado» em vez de uma
estimativa vestida de medida.

---

## 0. Antes de propor: uma premissa desta casa CADUCOU

O `docs/P2P-DISTRIBUIDO.md` §5.2 — papel J, esta casa — **recusou** descoberta
por DHT para o PhxMail, com o motivo escrito: *«manter DNS fixo é decisão de
produto, não lacuna técnica»*. Aquela recusa era **correta para a premissa que
existia**, e a premissa está escrita no `docs/CORREIO-DNS.md`: um servermail por
empresa, num **IP fixo**, publicado como registro `A` no Cloudflare, e o cliente
conectando na porta 8000.

A ordem do dono de 16/09 (`docs/VISAO.md`, Pilar 2) **troca essa premissa**:
*«sem servidor central de entrega, os pares trocam mensagem direto»*. E o
contrato deste subagente acrescenta a quarta restrição: *o servidor de um usuário
pode estar atrás de NAT/firewall doméstico e **não pode exigir porta aberta de
entrada***.

**Recusa herdada de premissa trocada é recusa que ninguém mediu.** Então este
documento não herda a de §5.2 — remede, e o resultado bate em um ponto e diverge
em outro (§2.1 e §3).

### O choque que sai disso, e é do integrador

**`CORREIO-DNS.md` e «identidade sem domínio» não cabem juntos como estão.** O
endereço `usuario@empresa.phxmail.com.br` é um nome DNS resolvido por um provedor
central (Cloudflare); a identidade que a ordem pede é uma **chave Ed25519**. Os
dois podem conviver — o nome como apelido do par —, mas há um detalhe de formato
que decide antes: **o AAD do selo usa o endereço** (`CORREIO-FORMATO.md` §6,
pergunta 7), e isso fixa o endereço como identidade imutável. Trocar para o `id`
(ou para a chave) é barato **hoje**, porque não há mensagem gravada; depois de
haver, é migração de dado cifrado — o pior tipo. Não decido isso: é do dono e do
DBA, e entra cedo.

---

## 1. Resumo executivo — a recomendação em uma página

**A recomendação, em uma frase:** para a v1 do transporte, **não há DHT no
caminho crítico**; o desenho candidato é **pull sobre um par-relé voluntário,
com descoberta por convite assinado (fora de banda) e mDNS só na rede local** —
e as duas peças que faltam para isso funcionar **não são de rede**, são de
**formato** (assinatura por mensagem) e de **laço** (long-poll).

Os cinco números que sustentam a recomendação, todos medidos:

1. **O nosso gargalo de transporte não é a rede — é o sono do laço de pull.**
   Medido em `bancada/replicacao/resultados.json` (07/09/2026, v0.18.0): uma
   inserção leva **2.012 ms** para aparecer na réplica, e o próprio arquivo diz
   que isso *«inclui o sono do laço da réplica (2 s nesta bancada) MAIS o
   transporte»*. O transporte puro está medido à parte em
   `bancada/quorum/resultados.json` (07/09/2026 16:37): **`levar_ms` = 0,47 ms**
   (faixa 0,346–3,482, em `localhost`, declarado como **piso**). Ou seja: a rede
   responde por **0,023%** do atraso; o resto é o `sleep`. Qualquer receita de
   fora que prometa acelerar o **transporte** está mirando 0,02% do nosso
   problema. *(A divisão é aritmética minha sobre os dois números medidos, não
   uma terceira medição.)*
2. **Não existe long-poll no servidor.** Medido agora: `grep -rn "Condvar"
   crates/phxsql-server/src` devolve **zero**; a peça existe e é nossa, em
   `crates/phxsql-core/src/semaforo.rs` (`Mutex` + `Condvar`, escritos aqui). A
   proposta já estava no `docs/CASSANDRA.md` §6.2 e continua aberta. **É o item
   de maior valor ÷ custo de todo este documento.**
3. **O túnel cifrado já é agnóstico de soquete.** `fio::Canal::ler` é genérico
   sobre `BufRead` e `fio::Canal::escrever` sobre `Write`
   (`crates/phxsql-core/src/fio.rs:522` e `:593`) — **não** está preso a
   `TcpStream`. Consequência direta e não óbvia: **um par-relé que só empurra
   bytes não custa uma linha de criptografia nova**, e ele não consegue ler o que
   passa. É exatamente a propriedade que as salas do Scuttlebutt compram com
   duas camadas de cifra (§2.2.4), e aqui ela já está paga.
4. **A rede desta casa é 100% TCP, e isso é uma fronteira real.** Medido:
   **0** ocorrências de `UdpSocket` em 271 arquivos `.rs` / **194.328 linhas**;
   **109** de `TcpStream` e **48** de `TcpListener`. Todo mecanismo de descoberta
   e de travessia de NAT da literatura (Kademlia, STUN, mDNS, hole punching) é
   **UDP**. A boa notícia para a pétrea: a `std` tem tudo —
   `UdpSocket::join_multicast_v4`, `set_broadcast`, `set_multicast_ttl_v4`,
   `set_nonblocking` são estáveis **desde a 1.9.0**, e `bind`/`send_to`/
   `recv_from` desde a 1.0.0 (doc oficial da `std`). **Zero-deps passa.** O que
   não passa de graça é o **modelo de concorrência**: hoje é uma thread
   bloqueante por conexão (`servidor.rs:40584`), e um laço UDP com α consultas
   em voo é outro desenho.
5. **A anti-entropia que a literatura chama de sofisticada, nós já temos — e ela
   é mais forte que a deles no ponto que importa.** O `createHistoryStream` do
   Scuttlebutt («me dá tudo depois da sequência N» sobre um log append-only) é,
   peça por peça, o nosso `{"op":"replicar","desde":N}` sobre um `.reg` que
   **nunca reaproveita slot** (a ordem de digitação sagrada). Medido no nosso
   lado: **37.311 eventos/s** aplicados na réplica, **223 bytes por evento** com
   imagem da linha (44 sem), alcance de **2,7 s** para 105.001 linhas
   (`bancada/replicacao/resultados.json`, 07/09/2026).

**E o furo que a mudança de premissa abre, dito antes de qualquer elogio:** hoje
a autenticidade de um evento do diário vem do **canal** (Noise NX) e da lista
`replicas_autorizadas` (`REPLICACAO.md` §7). **No instante em que uma mensagem
passa por um par-relé, o canal deixa de cobrir o conteúdo** — o relé é o «outro
lado» de um dos canais. Sem **assinatura por mensagem**, um relé malicioso
insere, apaga ou reordena e nada acusa. O E2E do `correio-e2e.rs` protege o
*corpo* (ChaCha20-Poly1305 + AAD), mas **não** prova ao terceiro quem produziu a
linha nem em que ordem — é o que a assinatura Ed25519 por evento dá e o AEAD não
dá (não-repúdio; o mesmo argumento do `phxblockchain-melhorias-2026-09.md` §1.3).
**Isto é mudança de formato, e por isso entra cedo, antes de congelar o PSCH das
seis tabelas do `CORREIO-FORMATO.md`.**

**O que NÃO recomendo para a v1, com o número em §3:** Kademlia/DHT no caminho
crítico, STUN/ICE completo, TURN como serviço nosso, e gossip epidêmico.

---

## 2. A matriz de evidência

Legenda das duas fronteiras, que é o crivo desta casa:
**[dep]** = precisa de algo fora da `std` (barra sem o dono);
**[formato]** = mexe no formato em disco da caixa (é do DBA e entra cedo);
**[UDP]** = estreia soquete UDP no projeto (passa na `std`, mas é superfície de
rede nova); **[async]** = pede um modelo de concorrência que o servidor não tem.

### 2.1 A — Descoberta de pares sem servidor central

| Técnica | O que faz | Custo publicado | Crivo |
|---|---|---|---|
| **Kademlia (paper)** | ID de 160 bits, distância XOR, k-buckets, `FIND_NODE` recursivo com α consultas em paralelo; busca em `h − log k` passos, e `h ≈ O(log n)` | k=20 e α=3 no exemplo do paper; republicação **1 h**, republicação do publicador **24 h**, expiração **24 h** | **[UDP] [async]**, zero-deps **passa** |
| **Mainline DHT (BEP 5)** | O Kademlia em produção massiva: `ping`/`find_node`/`get_peers`/`announce_peer` em bencode sobre **UDP** | **k = 8**; nó compacto = **26 bytes** (20 de ID + 6 de contato); par compacto = **6 bytes**; `token` = `SHA1(IP‖segredo)`, segredo roda a cada **5 min**, token aceito até **10 min**; refresh de bucket a cada **15 min** | **[UDP] [async]** |
| **mDNS / DNS-SD** | Pergunta em multicast `224.0.0.251:5353` (ou `FF02::FB`); o serviço se anuncia como `<Instância>.<Serviço>.<Domínio>` com PTR/SRV/TXT | Mensagem pode ir até o MTU da interface, **nunca acima de 9.000 bytes**, e o RFC recomenda ficar **abaixo de 1.500**; TTL **120 s** para registro com nome de host, **75 min** para o resto; consulta contínua: intervalo ≥ **1 s** dobrando; TXT típico **≤ 200 bytes** | **[UDP]**, zero-deps **passa** — e é a única técnica desta linha que **não precisa de nenhum terceiro** |
| **Lista de bootstrap assinada** | Uma lista `host:porta + chave pública` embutida ou distribuída, assinada; o par novo entra por um contato conhecido | O Kademlia §2.2 exige **um** contato para ingressar; o BEP 5 sequer publica nós oficiais (*«Please do not automatically add router.bittorrent.com…»*) | **passa em tudo** — a assinatura é Ed25519, que já temos |
| **Convite do Scuttlebutt** | `domínio:porta:@chave.ed25519~segredo` — carrega **endereço + chave pública + segredo de uso único** numa string | uma string, fora de banda | **passa em tudo.** É bootstrap **autoautenticado**: não precisa de lista assinada porque o próprio convite traz a chave |
| **Anúncio local do Scuttlebutt** | Difusão UDP para `255.255.255.255:8008`, **um pacote por segundo**, com `net:IP:porta~shs:chave` | 1 pacote/s por par na LAN | **[UDP]**, passa |
| **Relés do Nostr** | Cliente abre WebSocket para N relés; `REQ` com filtros → `EVENT`s → `EOSE`. **Clientes nunca falam entre si**, e o NIP-01 não diz nada sobre relé falando com relé | — | **[dep]** (WebSocket sobre TLS), e **contradiz o pedido**: é servidor central, só que plural |

**O que cada uma resolve que a nossa abordagem de hoje não resolve, e o que já
temos a favor.** Hoje a descoberta é **uma linha de configuração** — `origens`
no `config.json` da réplica, `cluster.nos` no cluster, registro `A` no
Cloudflare para o servermail. Isso **não resolve** o par doméstico cujo IP muda:
o `REPLICACAO.md` §7 já registra a mesma dor com todas as letras — *«num
orquestrador o IP do vizinho muda a cada recriação. Lista por IP só é operável
com endereçamento fixo»*. Quem resolve isso é **qualquer** técnica que troque
«endereço fixo» por «chave fixa + endereço corrente» — e as três baratas são
mDNS (na LAN), convite autoautenticado (entre conhecidos) e DHT (entre
desconhecidos, com o custo de §3.1).

**O que já temos a favor:** a chave estática do fio já é o pino no estilo
`known_hosts` (`CIFRA-DO-FIO.md` §1), e `phxsqld --chave-do-fio` já imprime a
pública — **metade de um convite do Scuttlebutt já está escrita e em uso.**

**Bytes por consulta e saltos, com a conta feita à vista:** uma resposta de
`find_node` do BEP 5 com k=8 carrega 8 × 26 = **208 bytes** de nós, mais o
envelope bencode; uma busca em `n = 10^6` pares custa, pelo esboço de prova do
paper (§3, `h − log k`), da ordem de **20 − log₂ 8 ≈ 17 rodadas de α consultas**.
*Isso é aritmética a partir da especificação, não medição.* O número **medido**
que achei sobre latência de busca em DHT real é o do Jimenez, Osmani & Knutsson
(IEEE P2P 2011): estudos anteriores mediam buscas *«em segundos»*, na Mainline
DHT muitas já eram sub-segundo, e com as modificações deles a **mediana cai para
100–200 ms**. **Li o resumo, não o PDF** — `diva-portal.org` reiniciou a conexão
duas vezes nesta sessão —, então este número é **citado do abstract, não medido
por mim**.

### 2.2 B — Travessia de NAT

| Técnica | O que faz | Custo / taxa publicada | Crivo |
|---|---|---|---|
| **STUN (RFC 8489)** | O par manda um `Binding Request` a um servidor e recebe de volta o **seu próprio** endereço visto de fora (`XOR-MAPPED-ADDRESS`) | Cabeçalho de **20 bytes** + atributos; *magic cookie* `0x2112A442`; RTO inicial **≥ 500 ms**, `Rc` = **7**, `Rm` = **16**, transação falha em **39.500 ms** | **[UDP]**, zero-deps passa. Precisa de **um servidor STUN**, que é infraestrutura de terceiro ou nossa |
| **Hole punching UDP** | Os dois pares, por um intermediário, aprendem o endereço externo um do outro e **mandam ao mesmo tempo**; a saída de cada um abre o furo para a entrada do outro | **82%** dos NATs testados suportam (310 de 380; 68 fabricantes); **64%** para TCP (184 de 286); *hairpin* só **24%** UDP e **13%** TCP. A RFC 5128 §4 repete: *«mais de 80%»* UDP, *«pouco mais de 60%»* TCP, *«menos de 25%»* hairpin | **[UDP] [async]**. Precisa de um **intermediário** — que não precisa ser servidor (ver BEP 55) |
| **Hole punching entre pares (BEP 55)** | O intermediário é **outro par**: `rendezvous` (0x00) → o relé manda `connect` (0x01) aos dois, cada um com o endereço do outro → cada um conecta | Mensagem de **8 bytes** (IPv4): `msg_type` 1 + `addr_type` 1 + `addr` 4 + `port` 2, mais `err_code` 4 no erro | **[UDP] [async]**. **É a variante que casa com «sem servidor central»**: nenhum servidor, só um par que já fala com os dois |
| **ICE (RFC 8445)** | Junta candidatos (host, server-reflexive, peer-reflexive, relayed) e testa **todos os pares** com `Binding Request` | **N × M** verificações de conectividade; e o RFC é explícito: *«ICE não se destina à travessia de NAT do protocolo de sinalização, que se presume fornecido por outro mecanismo»* | **[UDP] [async]** e **pesado**: é a soma de STUN + TURN + um canal de sinalização que ainda não existe |
| **TURN (RFC 8656)** | Um relé **na Internet pública** aloca um endereço para o cliente e repassa tudo | Alocação padrão **600 s**, permissão **300 s**, canal **600 s**; o servidor *«tipicamente precisa de uma conexão de alta largura de banda»*; autenticação de credencial de longo prazo é **MUST**; *«melhor usar TURN só quando um caminho direto não pode ser encontrado»* | **é um servidor central com outro nome** — contradiz o pedido, e paga a banda de todo o tráfego |
| **PCP / NAT-PMP (RFC 6887 / 6886)** | O par **pede ao próprio roteador** que abra um mapeamento de entrada, com prazo | UDP, payload máximo **1.100 octetos**, opcode `MAP` com *Requested Lifetime* de 32 bits; exige que o CPE **suporte** PCP (negociação devolve versão 0 = NAT-PMP) | **[UDP]**, zero-deps passa. **É o mais barato quando funciona** — e não achei número publicado de qual fração dos roteadores domésticos o suporta hoje |
| **Relé por par voluntário** | Um par que **tem** porta aberta empresta o cano; os dois extremos abrem o túnel fim-a-fim **por dentro** dele | Salas do Scuttlebutt: o túnel *«está dentro das conexões externas, o que significa que é cifrado duas vezes»*; o administrador da sala **não lê** o conteúdo, mas vê *«endereços IP, carimbos de tempo, durações e largura de banda»* | **passa em tudo**, e é o único desta tabela que **não estreia UDP** |

**O que resolve que a nossa abordagem de hoje não resolve — e a metade que já
está resolvida.** O `REPLICACAO.md` §7 comprou uma propriedade de firewall que é
exatamente o que um par doméstico precisa: **quem procura é o cliente**; a
réplica só faz **saída** TCP 5000, e o source nunca precisa alcançá-la. Ou seja:
**o par atrás de NAT que quer RECEBER já sabe receber — desde que exista alguém
com porta aberta de quem puxar.** O que falta é só o caso «**ninguém** dos dois
tem porta aberta», e para ele há duas saídas medidas: furar o NAT (82% UDP,
2005) ou passar por um terceiro que tenha porta.

**E a honestidade sobre o 82%:** ele é de **2005**. Procurei número mais recente
e **não achei medição primária publicada**. O que achei foi: (a) blogs de
fornecedores de WebRTC citando 4%–30% de recurso a relé, **sem publicação
primária que eu pudesse conferir**; e (b) o texto da Tailscale sobre travessia de
NAT, cuja frase é literalmente uma **estimativa do autor** — *«eu estimaria que
você conseguiria conexão direta mais de 90% das vezes»* —, e estimativa não é
medida. **Fica registrado como premissa a medir (§5, P3).**

### 2.3 C — Gossip e anti-entropia para a caixa

| Técnica | O que faz | Custo publicado | Crivo |
|---|---|---|---|
| **Gossip do Cassandra** | A cada **1 s** (`intervalInMillis = 1000`), cada nó escolhe **um** par vivo ao acaso e troca 3 mensagens: `GossipDigestSyn` → `GossipDigestAck` → `GossipDigestAck2`; mais um par inalcançável com probabilidade `inalcançáveis/(vivos+1)` e uma semente com probabilidade `sementes/(vivos+inalcançáveis)` | 1 rodada/s/nó, 3 mensagens por rodada | **[async]**. O fonte aponta a CASSANDRA-150 como o motivo de um par por rodada |
| **Merkle anti-entropia (Dynamo §4.7)** | Uma árvore de hash por faixa de chaves; compara-se a raiz, e só se desce onde diverge | *«cada galho pode ser conferido independentemente sem exigir que os nós baixem a árvore inteira ou o conjunto de dados inteiro»*; a desvantagem declarada: *«muitas faixas mudam quando um nó entra ou sai, exigindo recalcular a(s) árvore(s)»*, e §6.2 chama isso de *«operação não trivial de fazer num sistema em produção»* | **[formato]** se persistida; efêmera não muda formato |
| **MerkleTree do Cassandra** | Árvore binária cheia de profundidade `hashdepth`; folhas cobrem faixas do particionador | `RECOMMENDED_DEPTH = Byte.MAX_VALUE − 1`; **o fonte não documenta complexidade** (o `differenceHelper` traz um `TODO` de otimização) | idem |
| **Feed append-only do Scuttlebutt** | Log por identidade, mensagem `n` referencia o id da `n−1`, sequência começa em 1; `createHistoryStream` com `sequence` devolve **só o que veio depois** | um pedido por feed, por par | **passa em tudo — e é o que já fazemos** |
| **Filtros do Nostr** | `REQ` com `since`/`until`/`limit`; o relé devolve o histórico e sinaliza `EOSE` | — | **[dep]** (WebSocket) |

**Onde cada um diverge do que a nossa replicação por diário já faz.**

- **Scuttlebutt: convergência quase perfeita, com uma diferença que decide o
  desenho.** `createHistoryStream(sequence: N)` é o nosso
  `{"op":"replicar","desde":N}`; o log append-only por identidade é o nosso
  `.reg` que nunca reaproveita slot; a `rownum` é a `sequence`. **A divergência
  é a prova:** cada mensagem do feed deles é **assinada individualmente** e
  encadeada pelo hash da anterior, então **qualquer terceiro** verifica o feed
  inteiro sem confiar em quem o entregou. O nosso evento **não é assinado**: a
  confiança vem do canal e da lista de IPs. Enquanto o transporte for
  source→réplica dentro de um firewall, a nossa escolha é a mais barata e está
  certa. **Passando por relé, ela deixa de bastar.**
- **Cassandra: resolve um problema que não temos, e o fonte diz por quê.** O
  gossip existe para descobrir *membros* e detectar falha numa lista que muda
  sozinha. O nosso `docs/SPRINTS-CASSANDRA.md` §5.10 já registrou a recusa —
  *«copiar gossip seria trazer um subsistema»* — e o `CLUSTER.md` §2.2 mostra o
  substituto: pulso ponto a ponto entre nós **nomeados no `config.json`**, com
  eleição por maioria. **Para o correio P2P, a pergunta muda**: a lista de pares
  de um usuário **é** o caderno de confianças (`correio_confiancas`), que muda
  por decisão humana e é pequena. **O gossip continua não cabendo, pelo mesmo
  motivo de sempre**, e agora com uma razão a mais: gossip espalha estado para
  quem não pediu, e o anti-spam desta casa é *«sem confiança aceita, não há
  contato»* (`PHXMAIL-vs-SMTP.md` §2). Um protocolo que fala com quem não
  aceitou é um protocolo que fura a própria regra de anti-spam.
- **Merkle de anti-entropia: o nosso diário já é ordenado, e isso muda a conta.**
  A Merkle do Dynamo existe porque as réplicas guardam um **conjunto não
  ordenado** de chaves e é preciso achar *quais* divergem. O nosso diário é
  **uma sequência**, e «o que falta» responde-se com **um inteiro** (`desde:N`).
  Comparar árvore para descobrir o que falta numa sequência é pagar `O(n)` de
  construção para responder o que um `u64` responde. **A Merkle só volta a fazer
  sentido se um relé puder entregar eventos fora de ordem ou com buracos** — e
  aí a peça certa provavelmente não é a Merkle, é a **cadeia de hash por feed**
  do Scuttlebutt (cada evento carrega o hash do anterior), que detecta buraco e
  reordenação com 32 bytes por evento, sem árvore nenhuma. *Premissa a medir:
  §5, P4.*

### 2.4 D — Identidade sem domínio e confiança

| Técnica | O que faz | Rotação / perda de chave | Crivo |
|---|---|---|---|
| **Ed25519 como endereço (Scuttlebutt)** | A identidade **é** o par de chaves; o endereço é `@base64.ed25519` | **Não há rotação.** *«Se um usuário perde a chave secreta ou ela é roubada, ele precisará gerar uma nova identidade e dizer às pessoas para usarem a nova»* | **passa** — `ed25519.rs` já existe e está em uso |
| **Tox ID** | 32 B de chave pública + 4 B de *nospam* + 2 B de soma de verificação = **38 bytes** | A chave **do DHT** é efêmera e renova ao reiniciar; a **de longo prazo não tem rotação**, e chave comprometida = identidade nova | **passa**. A separação «chave de rede efêmera × identidade de longo prazo» é a ideia transferível |
| **Nostr** | A identidade é a pública secp256k1 (Schnorr, 32 bytes); o `id` do evento é o `sha256` da serialização | O NIP-01 não traz rotação | **[dep]** pela curva (secp256k1 não existe aqui; Ed25519 existe) |
| **Autocrypt / Delta Chat** | A chave viaja **no cabeçalho do próprio e-mail** (`Autocrypt: addr=…; prefer-encrypt=mutual; keydata=BASE64`, ≤ **10 KiB**); o par guarda `last_seen`, `autocrypt_timestamp`, `public_key`, `prefer_encrypt` e **a mensagem mais nova vence**; a chave passa entre aparelhos por *Setup Message* com código de **36 dígitos** em nove blocos de 4 | Degradação graciosa quando o par some | **conceitualmente passa** — é TOFU com estado por par, e o estado é uma tabela |
| **TOFU (o que já temos)** | O cliente aceita a estática do servidor na primeira conexão e a guarda; com pino, recusa a divergente | `CIFRA-DO-FIO.md` §1 já escreve o defeito: *«quem estiver no meio na PRIMEIRA conexão vence para sempre»* | **já em produção** |

**O que já temos a favor, e é mais do que parece.** A identidade por chave, sem
CA e sem certificado, **é o desenho desta casa desde o pedido 61**: Noise NX +
pino estilo `known_hosts`, Ed25519 e X25519 escritos aqui e conferidos contra
RFC 8032 e RFC 7748. O `P2P-DISTRIBUIDO.md` §5.2 já tinha registrado a
convergência com o S/Kademlia («ID = hash da chave pública»). **Não há nada a
importar nesta linha** — há uma coisa a **decidir**, e é de formato:

> **`correio_contas` tem hoje UMA chave pública (`publica`, X25519, 32 bytes).**
> Identidade que **assina** precisa de Ed25519; ECDH precisa de X25519. São
> curvas diferentes e as duas já existem aqui. Guardar **duas** colunas, ou
> derivar uma da outra, é decisão de DBA — e **entra antes de congelar o PSCH**.

**O que a literatura ensina e nós não temos:** o **Autocrypt resolve a
distribuição de chave sem servidor de chaves** pondo-a no próprio cabeçalho — e
o nosso equivalente natural é a chave viajar no **pedido de confiança**, que já
existe como canal (`correio_confiancas`). E a honestidade que o Autocrypt escreve
e que vale copiar **como texto**: *«Autocrypt Level 1 só defende contra ataques
passivos de coleta de dados… proteção contra adversários ativos é objetivo de
especificações futuras»*. É a mesma frase que a nossa §2 do `CIFRA-DO-FIO.md` já
escreve sobre `exigir: false`. Quem já diz isso sobre o fio tem de dizer sobre o
correio.

### 2.5 E — Transporte sem servidor central: o desenho candidato

**O que o par SEM porta aberta faz.** Ele faz o que a réplica já faz: **puxa**.
E a pergunta que sobra é «puxa de onde», com três respostas em ordem de custo:

1. **Da rede local**, quando o par está na mesma LAN (mDNS/anúncio UDP). Custo:
   um soquete UDP, zero terceiros.
2. **De um par conhecido que TEM porta aberta** — o par-relé voluntário. É o
   modelo do Delta Chat (que **não atravessa NAT nenhum**: usa um relé burro,
   o servidor de e-mail, e é por isso que funciona no celular de todo mundo) e o
   das salas do Scuttlebutt (o relé é um par, e o túnel por dentro é cifrado
   fim-a-fim, então o relé não lê).
3. **Direto**, quando os dois têm endereço alcançável — ou porque um tem porta
   aberta, ou porque o roteador abriu por PCP, ou (opcional, depois) porque
   furaram o NAT com um terceiro par como intermediário (BEP 55).

**O custo em rodadas de rede por mensagem entregue, contado no nosso fonte.**
Contando o que a `replica::Cliente` faz hoje (`crates/phxsql-server/src/replica.rs`:
`cifrar` → `desafio` → `login` → `bancos`, depois `posicao` → `replicar`):

| fase | rodadas (RTT) | de onde sai a conta |
|---|---:|---|
| abrir TCP | 1 | handshake do TCP |
| aperto Noise (`cifrar`) | 1 | m1 = 32 B, m2 = 96 B (`CIFRA-DO-FIO.md` §6) |
| `desafio` + `login` | 2 | `replica.rs:209` e `:243` |
| `bancos` | 1 | `replica.rs:265` |
| **abrir a conexão** | **5** | soma |
| `posicao` + `replicar`, por rodada de sondagem | **2** | `replica.rs:302` e `:372` |

*Isto é contagem de chamadas no fonte, não medição no fio.* Com a conexão
mantida de pé, a entrega de **uma** mensagem custa **2 RTT** — e **o que domina
o relógio não são os 2 RTT (0,47 ms medidos de transporte), é o intervalo entre
rodadas**. Com `reconectar_em` de 10 s (o padrão dos exemplos), o atraso médio de
uma mensagem é **5 s**, e depois de uma falha de rede o recuo exponencial do
`Ritmo` chega ao teto de **60 s** (`replica.rs`, `Ritmo::TETO`, provado em
`rede_recua_dobrando_ate_o_teto`). **Long-poll troca esses 5 s por ~0 sem trocar
um byte do protocolo** — é o mesmo `replicar`, segurando a resposta até haver
evento ou até um prazo.

**O que o desenho candidato exige que ainda não temos**, em ordem de quem decide:

| falta | quem decide | fronteira |
|---|---|---|
| **Assinatura Ed25519 por mensagem/evento** — sem ela, relé não é seguro | C (DBA) + SEC | **[formato]**, 64 B/linha, **entra cedo** |
| **Chave Ed25519 do par em `correio_contas`** (hoje só X25519) | C (DBA) | **[formato]**, entra cedo |
| **Caixa de saída com estado por destino** (pendente/entregue/por-relé) — as seis tabelas propostas **não têm** | C (DBA) | **[formato]**, entra cedo |
| **Long-poll no `replicar`** (`Condvar` já existe em `semaforo.rs`) | B | sem formato, sem dep |
| **Papel «relé» e sua política** (quem aceita relear, teto de bytes, prazo de retenção) | dono + B | config, e **[formato]** se o relé guarda |
| **Soquete UDP** (só se entrar mDNS, PCP ou hole punching) | B | **[UDP]**, `std` cobre |

---

## 3. O que foi avaliado e RECUSADO, com o número

Esta é a seção que mais poupa tempo depois. Recusa aqui é «mediu-se e o número
disse não», nunca «não pensamos nisso».

1. **Kademlia/DHT no caminho crítico da v1 — RECUSADO, e por dois números, não
   por gosto.** (a) **O ganho não se aplica ao nosso tamanho**: a busca
   logarítmica paga o custo de k-buckets, republicação horária e refresh quando
   há **milhares** de pares; o caderno de confianças de um usuário é da ordem de
   **dezenas** — e o `P2P-DISTRIBUIDO.md` §5.1 já tinha feito essa conta para o
   cluster. (b) **O que ela entrega não é o que falta**: uma DHT devolve
   `IP:porta` de um par; um par doméstico atrás de NAT **continua inalcançável
   depois de encontrado**, então DHT sem hole punching e sem relé não entrega
   mensagem nenhuma. Ou seja, a DHT é **custo somado** ao problema real, não
   substituto dele. Fica registrada como item de pesquisa futura, com a premissa
   nomeada em §5 (P1).
2. **ICE completo (RFC 8445) — RECUSADO por custo declarado na própria RFC.**
   Ele exige (i) **N × M** verificações de conectividade, (ii) STUN, (iii) TURN,
   e (iv) um **canal de sinalização fora de banda que ele explicitamente não
   fornece** (*«ICE não se destina à travessia de NAT do protocolo de
   sinalização»*). Adotar ICE é adotar quatro subsistemas para resolver o caso
   dos 18% que o hole punching UDP não cobre (100% − 82%, Ford et al. 2005).
3. **TURN como serviço nosso (RFC 8656) — RECUSADO por contradizer o pedido.**
   A RFC descreve um relé **na Internet pública**, que *«tipicamente precisa de
   uma conexão de alta largura de banda»* e **MUST** autenticar por credencial
   de longo prazo. Um TURN nosso é um servidor central de entrega com outro
   nome — exatamente o que a ordem de 16/09 tirou do desenho. **O que substitui
   com a mesma função e sem o servidor é o par-relé voluntário** (§2.2, salas do
   Scuttlebutt), e o motivo técnico de a substituição funcionar é nosso e está
   medido: `fio::Canal` é genérico sobre `BufRead`/`Write` (`fio.rs:522`, `:593`),
   então o relé é um cano burro e **não lê**.
4. **Gossip epidêmico (estilo Cassandra) para a caixa — RECUSADO, e agora com
   dois motivos.** O primeiro já estava escrito (`SPRINTS-CASSANDRA.md` §5.10):
   *«copiar gossip seria trazer um subsistema»*, e a lista de pares aqui não muda
   sozinha. O segundo é novo e é de produto: **gossip fala com quem não pediu**,
   e a regra anti-spam do phxmail é *«sem confiança aceita, não há contato»*
   (`PHXMAIL-vs-SMTP.md` §2). Um transporte que espalha para vizinhos fura a
   regra que é a maior vantagem do produto.
5. **Merkle de anti-entropia para a caixa — RECUSADO por estrutura, com o número
   da alternativa.** A Merkle acha *quais* chaves divergem num conjunto **não
   ordenado**; o nosso diário é **ordenado**, e «o que falta» cabe num `u64`
   (`desde:N`). Construir a árvore é `O(n)` e o Dynamo declara o recálculo como
   *«operação não trivial num sistema em produção»* (§6.2). Trocar um inteiro por
   uma árvore é pagar `O(n)` para responder `O(1)`.
6. **Relés no molde do Nostr — RECUSADO por dependência e por modelo.** O
   transporte é **WebSocket** (NIP-01), que sobre TLS é crate — e a pétrea
   zero-deps barra o meio, como o `CLAUDE.md` já registra para o TLS. E no
   modelo, *clientes nunca falam entre si*: é servidor central plural, não P2P.
7. **RC4/ofuscação, MD4/SHA-1, RSA-384 — permanecem RECUSADOS**, sem novidade:
   já estão em `P2P-DISTRIBUIDO.md` §6, e nada nesta rodada os reabre.
8. **Herdar a recusa de DHT do `P2P-DISTRIBUIDO.md` §5.2 — RECUSADO como
   método.** Aquela recusa dependia da premissa «um servermail por empresa em IP
   fixo publicado no DNS», e a ordem de 16/09 trocou a premissa. A recusa nova
   (item 1) é **outra**, com outro número: lá era «o produto quer IP auditável»,
   aqui é «a DHT não entrega o que falta». Recusa herdada é recusa que ninguém
   mediu.

---

## 4. Divergências justificadas por restrição nossa

A pergunta da casa: **onde esta lógica DIVERGE da de origem, e qual restrição
nossa causou a divergência?** Se a resposta fosse «em lugar nenhum», seria cópia.

| # | Origem | A nossa | A restrição que causou |
|---|---|---|---|
| 1 | **Delta Chat**: o relé é um servidor de e-mail (IMAP/SMTP), e o cliente **depende** de uma conta em um provedor | O relé é **um par PhxSql qualquer com porta aberta**, falando o nosso protocolo na porta de dados | **Zero-deps**: IMAP/SMTP+TLS é stack de terceiro; e a `std` não tem TLS. E já temos servidor: qualquer PhxSql é um |
| 2 | **Salas do Scuttlebutt**: o túnel é cifrado **duas vezes** (box stream dentro de box stream) | **Uma** camada de `fio` fim a fim por dentro de uma conexão de transporte que pode até ser em claro | `fio::Canal` é genérico sobre `BufRead`/`Write` (`fio.rs:522`,`:593`): a camada de fora existe para **transportar**, não para proteger — e cifrar duas vezes o que já está autenticado fim a fim é custo sem garantia nova. *(O metadado que o relé vê é o mesmo nos dois desenhos, e isso tem de ficar escrito, não escondido.)* |
| 3 | **Scuttlebutt**: cada mensagem do feed é assinada **e** encadeada pelo hash da anterior | O evento do diário **não é assinado hoje**; a proposta é **assinar**, mas manter o encadeamento **implícito** na `rownum`/`.reg`, sem hash do anterior por linha | **A ordem de digitação é sagrada e o `.reg` nunca reaproveita slot**: a sequência física já é o encadeamento. Pagar 32 bytes por linha para repetir o que o arquivo garante é pagar duas vezes — *a menos* que a premissa P4 (§5) mostre que o relé pode entregar com buraco |
| 4 | **BEP 5**: o `token` é `SHA1(IP ‖ segredo)`, para provar que quem anuncia é quem foi consultado | Se um dia houver anúncio, o análogo é o **desafio-resposta que já existe** (`desafio.rs`), com nonce fresco e HMAC-SHA256 | **Criptografia se confere contra vetor oficial** e SHA-1 está quebrado: o nosso primitivo já é mais forte, e o mecanismo (prova fresca amarrada a uma pergunta nossa) é o mesmo |
| 5 | **Kademlia**: o ID do nó é escolhido/derivado e a rede é aberta a desconhecidos | A identidade é a **chave Ed25519**, e o contato só existe com **confiança aceita** nos dois sentidos | A regra anti-spam do produto (*«sem confiança aceita, não há contato»*) faz o problema Sybil **encolher**: identidade barata não compra nada se ninguém aceita o pedido. **É o nosso equivalente de «encarecer a identidade», e ele é social, não computacional** — nenhum crypto puzzle entra |
| 6 | **Cassandra**: gossip a 1 par/s para descobrir membros | Pulso ponto a ponto entre pares **nomeados** (cluster) e caderno de confianças (correio) | A lista muda por **decisão humana** e é pequena; e gossip falaria com quem não aceitou o contato |
| 7 | **Autocrypt**: a chave viaja no cabeçalho de **toda** mensagem, e a mais nova vence | A chave viaja no **pedido de confiança** (uma vez, no canal que já existe), e trocar de chave é um pedido novo | Em Autocrypt não há aperto prévio; aqui **há**, e ele já autentica o par. Repetir a chave em toda mensagem seria pagar bytes por uma descoberta que o aperto já fez — e «a mais nova vence» é exatamente a porta pela qual um ativo troca a chave |
| 8 | **Nostr/Tox**: identidade sem rotação; perdeu a chave, perdeu a identidade | Idem **por enquanto**, e isso já está escrito como pergunta aberta do formato (`CORREIO-FORMATO.md` §6, pergunta 2: recuperação de senha) | Não é divergência ainda — é **a mesma limitação**, e ela precisa da palavra do dono antes de congelar o formato. Registrado para não passar por decisão |

---

## 5. As premissas que precisam de MEDIÇÃO antes de virar plano

Perguntas mensuráveis, com o que as mediria. **Medir a premissa do item vem
antes de propor o item** — inclusive quando o item é nosso.

| # | Pergunta mensurável | O que a mediria | Por que ela trava o plano |
|---|---|---|---|
| **P1** | **Quantos pares um usuário do phxmail tem de fato?** Se a mediana for dezenas, a DHT nunca entra; se for milhares, a conta de §3.1 muda | Contar `correio_confiancas` com estado «aceita» por conta, num piloto. **Hoje não há dado: o protótipo é em memória** | É a premissa única que sustenta a recusa 1 |
| **P2** | **O long-poll tira mesmo os ~2 s do atraso?** Medir `atraso_ms` com `reconectar_em` de 10 s contra `replicar` segurando a resposta | Repetir `bancada/replicacao/medir.py` com o `replicar` em long-poll e comparar contra os **2.012 ms** já medidos em 07/09/2026. O alvo aritmético é o `levar_ms` de **0,47 ms** | É o item de maior valor ÷ custo, e **ninguém mediu o ganho, só a perda** |
| **P3** | **Qual a taxa de sucesso de hole punching HOJE, na rede dos nossos usuários?** Os 82%/64% são de 2005 e não achei medição primária mais recente | Uma sonda: dois processos em redes domésticas diferentes, um terceiro par como intermediário (molde do BEP 55), contando sucesso/fracasso por operadora. **Sem esse número, o custo do hole punching é palpite** | Decide se a v2 investe em furar NAT ou fica só no relé |
| **P4** | **Um relé pode entregar eventos com buraco ou fora de ordem?** Se a resposta é não (o relé é um cano FIFO por par), a `rownum` basta; se é sim, precisa de hash do anterior por evento | Um teste de soquete com relé que **atrasa e reordena de propósito**, e a conferência de que o receptor detecta. Prova real nos dois sentidos: com o defeito reposto, cai | Decide a divergência #3 (§4), e é **[formato]** — entra antes de congelar o PSCH |
| **P5** | **Quanto custa a assinatura Ed25519 por mensagem no nosso caminho quente?** O `ed25519.rs` já existe; o custo por assinar/verificar **não está medido por mensagem** | Um `--example custo-da-assinatura` no molde do `onde-doi`, medindo assinar e verificar contra o custo de gravar a linha (**0,206 ms** de `gravar_ms` medido em `bancada/quorum`) | Se a assinatura custar mais que a gravação, o desenho muda (assinar por lote, não por linha) |
| **P6** | **Quantos bytes uma caixa real troca por rodada de sondagem?** Hoje sabemos **223 B/evento** com imagem; não sabemos o tamanho do `posicao` nem o custo de uma rodada **vazia** | Contar no `acessos.log`/telemetria uma rodada sem evento. Uma sondagem a cada 10 s, 24 h, são **8.640 rodadas vazias/dia/par** — o número importa para um par doméstico | Decide `reconectar_em` padrão e, de novo, o valor do long-poll |
| **P7** | **O modelo de thread bloqueante aguenta N pares?** Hoje é uma thread por conexão (`servidor.rs:40584`) | A bancada de conexões que já existe (`bancada/conexoes`), com N crescendo até o ponto em que a memória ou o escalonador doem | Decide se o par-relé precisa de outro modelo de concorrência, e **[async]** é a fronteira mais cara deste documento |
| **P8** | **Qual fração dos roteadores domésticos abre porta por PCP/NAT-PMP?** Não achei número publicado | Uma sonda `MAP` (RFC 6887, UDP) contra o roteador de cada implantação piloto | PCP é o caminho mais barato quando funciona, e **zero** se não funciona |

---

## 6. Fontes — todas primárias, com URL

**RFCs**

- RFC 5128 — *State of Peer-to-Peer Communication across NATs* (§3.3 hole
  punching, §4 as taxas, §5.1 relé como recurso final):
  https://datatracker.ietf.org/doc/html/rfc5128
- RFC 8489 — *Session Traversal Utilities for NAT (STUN)* (§5 cabeçalho de 20 B
  e magic cookie, §6.2.1 RTO/Rc/Rm, §6.3.1.2 XOR-MAPPED-ADDRESS, §1 «não é uma
  solução de travessia por si só»): https://datatracker.ietf.org/doc/html/rfc8489
- RFC 8445 — *Interactive Connectivity Establishment (ICE)* (tipos de candidato,
  verificações de conectividade, a exigência de canal de sinalização externo):
  https://datatracker.ietf.org/doc/html/rfc8445
- RFC 8656 — *Traversal Using Relays around NAT (TURN)* (relé na Internet
  pública, prazos de alocação/permissão/canal, autenticação obrigatória):
  https://datatracker.ietf.org/doc/html/rfc8656
- RFC 6887 — *Port Control Protocol (PCP)* (opcode `MAP`, payload ≤ 1.100
  octetos, negociação com NAT-PMP): https://datatracker.ietf.org/doc/html/rfc6887
- RFC 6886 — *NAT Port Mapping Protocol (NAT-PMP)*:
  https://datatracker.ietf.org/doc/html/rfc6886
- RFC 6762 — *Multicast DNS* (§5.1/§5.2 consulta única e contínua, §10 TTLs de
  120 s e 75 min, §17 tamanho máximo e o teto de 9.000 bytes):
  https://datatracker.ietf.org/doc/html/rfc6762
- RFC 6763 — *DNS-Based Service Discovery* (§4.1 `Instância.Serviço.Domínio`,
  §5 PTR/SRV/TXT, §6.2 os tamanhos de TXT):
  https://datatracker.ietf.org/doc/html/rfc6763
- RFC 8032 (Ed25519), RFC 7748 (X25519), RFC 8439 (ChaCha20-Poly1305), RFC 5869
  (HKDF) — os quatro já implementados e conferidos aqui; ver
  `docs/CIFRA-DO-FIO.md` §0.
- RFC 7435 — *Opportunistic Security*, citada pelo Autocrypt como a postura dele:
  https://datatracker.ietf.org/doc/html/rfc7435

**Papers**

- Ford, B.; Srisuresh, P.; Kegel, D. — *Peer-to-Peer Communication Across
  Network Address Translators*, USENIX ATC 2005. **82% UDP (310/380), 64% TCP
  (184/286), hairpin 24% UDP / 13% TCP, 68 fabricantes**, com a ressalva dos
  próprios autores de que a amostra é auto-selecionada:
  https://bford.info/pub/net/p2pnat/
- Maymounkov, P.; Mazières, D. — *Kademlia: A Peer-to-peer Information System
  Based on the XOR Metric*, IPTPS 2002:
  https://www.scs.stanford.edu/~dm/home/papers/kpos.pdf
- DeCandia, G. et al. — *Dynamo: Amazon's Highly Available Key-value Store*,
  SOSP 2007 (§4.7 Merkle para anti-entropia; §6.2 o recálculo «não trivial»).
  Texto extraído do PDF oficial nesta sessão:
  https://www.allthingsdistributed.com/files/amazon-dynamo-sosp2007.pdf
- Jimenez, R.; Osmani, F.; Knutsson, B. — *Sub-Second Lookups on a Large-Scale
  Kademlia-Based Overlay*, IEEE P2P 2011, p. 82–91. **Resumo lido; o PDF
  completo não abriu nesta sessão** (conexão reiniciada duas vezes contra
  `diva-portal.org`), então o «mediana de 100–200 ms» é **citado do abstract,
  não medido por mim**: https://ieeexplore.ieee.org/document/6038665/ e
  https://www.diva-portal.org/smash/get/diva2:436670/FULLTEXT01.pdf

**Especificações abertas e fonte de projeto**

- BEP 5 — *DHT Protocol* (UDP, k = 8, nó compacto de 26 B, par compacto de 6 B,
  `token` com segredo de 5 min aceito por 10 min, refresh de 15 min):
  https://www.bittorrent.org/beps/bep_0005.html
- BEP 55 — *Holepunch extension* (rendezvous/connect/error; **o intermediário é
  um PAR**): https://www.bittorrent.org/beps/bep_0055.html
- *Scuttlebutt Protocol Guide* (identidade Ed25519 `@…ed25519` e a ausência de
  rotação; difusão UDP em `255.255.255.255:8008` **um pacote por segundo**;
  convite `domínio:porta:@chave~segredo`; feed append-only com `sequence` e hash
  do anterior; `createHistoryStream`; aperto secreto de 4 passos):
  https://ssbc.github.io/scuttlebutt-protocol-guide/
- *Scuttlebutt Rooms 2* — conexão tunelada (o túnel *«é cifrado duas vezes»*; o
  administrador da sala vê IP, carimbo, duração e banda, mas **não o conteúdo**):
  https://github.com/ssbc/rooms2/blob/master/docs/Participation/Tunneled%20connection.md
- Nostr **NIP-01** (relés WebSocket, `REQ`/`EVENT`/`EOSE`, filtros
  `since`/`until`/`limit`, `id` = `sha256` da serialização, pública secp256k1):
  https://github.com/nostr-protocol/nips/blob/master/01.md
- *Autocrypt Level 1* (cabeçalho `Autocrypt:` com `addr`/`prefer-encrypt`/
  `keydata`, ≤ 10 KiB; estado por par; Setup Message de 36 dígitos; **«só defende
  contra ataques passivos»**): https://docs.autocrypt.org/level1.html
- *Delta Chat* — IMAP/SMTP sobre conta de e-mail existente, **sem servidor Delta
  Chat**; relés *chatmail* «baratos e burros»; Secure-Join por QR/convite; sem
  número de telefone: https://delta.chat/en/help
- Apache Cassandra 5.x — `src/java/org/apache/cassandra/gms/Gossiper.java`
  (`intervalInMillis = 1000`, um par vivo por rodada, `GossipDigestSyn` →
  `Ack` → `Ack2`, CASSANDRA-150) e
  `src/java/org/apache/cassandra/utils/MerkleTree.java`
  (`RECOMMENDED_DEPTH = Byte.MAX_VALUE − 1`, sem complexidade documentada):
  https://github.com/apache/cassandra/blob/trunk/src/java/org/apache/cassandra/gms/Gossiper.java
  e
  https://github.com/apache/cassandra/blob/trunk/src/java/org/apache/cassandra/utils/MerkleTree.java
- *Tox protocol specification* (Tox ID = 32 B + 4 B nospam + 2 B checksum; DHT
  Kademlia com k = 16 e 256 buckets; chave de DHT efêmera × identidade de longo
  prazo sem rotação): https://toktok.ltd/spec.html
- `std::net::UdpSocket` — documentação oficial da biblioteca padrão do Rust:
  `bind`/`send_to`/`recv_from` desde **1.0.0**, `set_read_timeout` desde
  **1.4.0**, `connect`/`set_broadcast`/`join_multicast_v4`/`leave_multicast_v4`/
  `set_multicast_ttl_v4`/`set_ttl`/`set_nonblocking` desde **1.9.0**:
  https://doc.rust-lang.org/std/net/struct.UdpSocket.html

**Não achei número publicado** (registrado para ninguém inventar depois):
taxa de sucesso de hole punching medida depois de 2005 em publicação primária;
fração de roteadores domésticos com PCP/NAT-PMP; percentual de sessões WebRTC
que caem em relé em publicação primária (só achei blogs de fornecedor); e o
número da Tailscale, que é **estimativa declarada pelo próprio autor**
(*«I'd estimate»*), não medição.

**Medido nesta sessão, no nosso repositório** (comando ao lado, para se refazer):

| número | valor | comando |
|---|---:|---|
| ocorrências de `UdpSocket` | **0** | `grep -r 'UdpSocket' crates/ --include=*.rs \| wc -l` |
| ocorrências de `TcpStream` | **109** | idem com `TcpStream` |
| ocorrências de `TcpListener` | **48** | idem com `TcpListener` |
| arquivos `.rs` | **271** | `find crates -name '*.rs' \| wc -l` |
| linhas `.rs` | **194.328** | `find crates -name '*.rs' \| xargs cat \| wc -l` |
| `Condvar` em `phxsql-server/src` | **0** | `grep -rn 'Condvar' crates/phxsql-server/src` |
| `Condvar` em `phxsql-core` | `semaforo.rs` | `grep -rn 'Condvar' crates/phxsql-core/src` |

**Medido antes, pela casa** (com a data, porque juntar corridas de dias
diferentes publica um retrato que nunca existiu):

- `bancada/replicacao/resultados.json` — **07/09/2026**, v0.18.0:
  `replica_eventos_s` 37.311; `atraso_ms` de 1 inserção **2.012** (inclui o sono
  de 2 s do laço); `com_imagem_bytes_por_evento` **223**;
  `sem_imagem_bytes_por_evento` **44**; `alcance_s` **2,7** para 105.001 linhas.
- `bancada/quorum/resultados.json` — **07/09/2026 16:37**: `levar_ms` **0,47**
  (faixa 0,346–3,482), `gravar_ms` **0,206**; tudo em `localhost`, declarado
  pelo próprio arquivo como **piso**.
- `docs/CIFRA-DO-FIO.md` §6 — aperto de 1 ida-e-volta, m1 = **32 B**,
  m2 = **96 B**; sobrecusto de registro: **+17 B fixos** mais Base64 — um `ping`
  de 52 B vira **93 B** no fio (**+78,8%**).

---

## 7. Dispensa registrada dos outros papéis, nesta frente

- **B (engenheiro)** — **dispensado**: nenhuma linha de Rust, nenhum `cargo`,
  por instrução da tarefa. O que virar código sai de outra rodada.
- **C (DBA)** — **NÃO dispensado; convocado por nome.** Quatro itens deste
  documento são formato em disco e **entram antes de congelar o PSCH das seis
  tabelas do `CORREIO-FORMATO.md`**: a assinatura Ed25519 por mensagem, a chave
  Ed25519 em `correio_contas`, a caixa de saída com estado por destino, e a
  decisão «AAD por endereço × por `id`/chave» que a identidade sem domínio
  reabre.
- **SEC (segurança)** — **convocado**: o relé muda o modelo de ameaça (o que o
  relé vê, o que ele pode inserir, e o que a assinatura por mensagem fecha).
- **F (prova real)** — **dispensado agora, obrigatório quando houver código**:
  P4 e P5 do §5 só valem com o defeito reposto derrubando o teste.
- **A (orquestrador)** — **convocado na integração**: o choque
  `CORREIO-DNS.md` × «identidade sem domínio» (§0) é encontro de frentes, e é
  exatamente o tipo de defeito que nenhuma frente sozinha vê.
- **D, E, G, H, I** — **dispensados**: nenhum ambiente sujo, nenhuma tela muda,
  nenhuma catraca nova, nenhum commit (quem comita é o integrador).
