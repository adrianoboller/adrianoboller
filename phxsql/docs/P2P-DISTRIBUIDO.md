# P2P e base de dados distribuída — do Kademlia ao eMule, medidos contra o crivo

Documento de **pesquisa aprofundada** (papel J), no mesmo contrato do
`CASSANDRA.md`: toda afirmação técnica traz a fonte (paper, seção, RFC ou
BEP); toda comparação com o PhxSql diz onde a pétrea barra e onde cabe; nada
aqui foi medido em bancada própria — é leitura de fonte primária, marcada como
tal, nunca "parece que funciona assim".

**As duas âncoras do dono para esta frente:** *«segurança em primeiro
lugar»* e *«base de dados distribuída»*. O documento está organizado para
responder às duas: a segurança tem seção própria (§4) com ataque e defesa
lado a lado, e a "base de dados distribuída" é respondida com uma fronteira
clara (§5.1) — o algoritmo que resolve *descoberta e roteamento distribuído*
não é o mesmo algoritmo que resolve *ausência de perda de dado com
integridade referencial*, e confundir os dois é o erro mais comum da
literatura popular sobre DHT.

**Zero código Rust, zero `cargo`, zero commit.** Este `.md` é o entregável;
quem integra e compila é o orquestrador, e o motor está com outra frente
compilando o P0 no momento desta pesquisa — o pipeline é uma compilação por
vez.

---

## 0. A trava jurídica — leia antes de tudo, porque ela decide o que pode entrar

O licenciamento do PhxSql é **`MIT OR Apache-2.0`** — permissivo, o mesmo par
usado nas outras leituras técnicas desta casa (o `CASSANDRA.md` lê Apache
2.0 à vontade, porque a licença deles é compatível). **O eMule e o aMule são
GPL** (a árvore do aMule cita GPLv2, herdada do eMule original), e GPL é
**copyleft**: qualquer código derivado dele — traduzido, portado,
"reescrito olhando o `.cpp`" — arrastaria a obrigação de licenciar o
resultado sob GPL. Isso contaminaria o `MIT OR Apache-2.0` do projeto
inteiro, e é inegociável na direção oposta a como a casa decidiu (pedido do
dono, ordem de 04/09/2026 sobre o Cassandra: *"inspiração, não cópia"* — aqui
o mesmo princípio vem com um alarme extra, porque a licença de origem não
perdoa contato).

**A régua que este documento seguiu, do início ao fim:**

| Fonte | Licença | Como foi lida |
|---|---|---|
| Paper do Kademlia (Maymounkov & Mazières, IPTPS 2002) | acadêmica, sem copyleft | **texto completo lido**, é a base técnica de tudo |
| Paper do S/Kademlia (Baumgart & Mies, 2007) | acadêmica, sem copyleft | **texto completo lido** |
| BEP 5 — Mainline DHT do BitTorrent | especificação aberta (bittorrent.org) | **especificação lida por inteiro** |
| Especificação Kad-DHT do libp2p/IPFS | especificação aberta (`specs.ipfs.tech`, licença do próprio projeto de specs) | **especificação lida** |
| Draft R5N do GNUnet (IETF) | especificação aberta | **draft lido** |
| Wiki do eMule/aMule (`wiki.amule.org`, `wiki.emule-web.de`) | documentação de **como o protocolo funciona**, não o `.cpp` | **lida para entender o desenho**, nunca para copiar trecho de código |
| Fonte C++ do eMule/aMule (`.cpp`/`.h`) | **GPLv2** | **NÃO lido, e não vai ser lido nesta frente** |

A régua concreta: **o algoritmo** — distância XOR, k-buckets, o formato de
uma mensagem `FIND_NODE`, a fórmula do modificador de crédito — é fato de
engenharia descrito em paper ou em documentação de protocolo, do mesmo jeito
que "o SHA-256 processa blocos de 512 bits" é fato e não é cópia de nenhum
`sha256.c`. Nada neste documento cita um número de linha de arquivo `.cpp`
do eMule/aMule, e nenhuma proposta de código na §5 é uma tradução de função
alheia — cada uma nomeia o paper/RFC de origem e a peça que já existe aqui em
Rust puro para reaproveitar (Ed25519, X25519+Noise, SHA-256, HMAC), do mesmo
jeito que a casa já fez com a norma dessas quatro.

---

## 1. Kademlia — a "base de dados distribuída" do título

Fonte primária única desta seção, salvo indicação contrária: **Maymounkov,
P.; Mazières, D. — "Kademlia: A Peer-to-peer Information System Based on the
XOR Metric", IPTPS 2002** (texto lido por inteiro, PDF do próprio site dos
autores, `kademlia.scs.cs.nyu.edu`).

### 1.1 O espaço de identificadores e a distância XOR

Toda máquina participante recebe um **ID de nó de 160 bits** (o paper propõe
o SHA-1 de algo maior, mas simplifica assumindo escolha aleatória). Chaves
também são identificadores de 160 bits. A distância entre dois
identificadores `x` e `y` é o **XOR bit a bit, lido como inteiro**:
`d(x,y) = x ⊕ y` (§2 do paper).

Por que XOR e não diferença numérica (como o Chord) ou métrica de anel: XOR é
**simétrica** (`d(x,y) = d(y,x)`) e **unidirecional** — para um ponto fixo
`x` e uma distância `Δ`, existe **exatamente um** `y` tal que `d(x,y) = Δ`.
A simetria é o que faz um nó aprender rota útil **a partir de qualquer
mensagem que recebe**, não só das que ele mesmo envia — o Chord, por ter
métrica assimétrica, precisa de um protocolo de estabilização à parte para
isso (§1, §2 do paper, citando o motivo pelo qual "Kademlia minimiza o
número de mensagens de configuração"). A unidirecionalidade garante que
buscas pela mesma chave, de origens diferentes, **convergem pelo mesmo
caminho** — o que torna cache ao longo do caminho eficaz.

### 1.2 k-buckets: a estrutura de roteamento

Cada nó mantém, para cada `0 ≤ i < 160`, uma lista de triplas
`⟨IP, porta UDP, ID⟩` para nós à distância entre `2^i` e `2^(i+1)` de si
mesmo — o **k-bucket** de índice `i` (§2.1). `k` é um parâmetro de
replicação do sistema inteiro; o paper usa `k = 20` como exemplo, escolhido
para que a chance de **k nós falharem todos dentro de uma hora seja
desprezível**.

A regra de atualização de um k-bucket, exata (§2.1):

1. Chegou mensagem (pedido ou resposta) de um nó → atualiza o bucket certo.
2. Nó já está na lista → move para o **fim** (mais recentemente visto).
3. Nó não está e o bucket **não está cheio** → insere no fim.
4. Nó não está e o bucket **está cheio** → faz **PING no mais antigo**
   (cabeça da lista). Se não responder, é **descartado** e o novo entra; se
   responder, o antigo vai para o fim e o **novo é que é descartado**.

**Isto não é LRU comum** — é LRU com **viés deliberado pelo nó mais
antigo**, e o paper justifica com dado medido de outro sistema (Gnutella,
Saroiu et al.): quanto mais tempo um nó já está de pé, maior a chance
estatística de continuar de pé na próxima hora (Figura 1 do paper). E o
efeito colateral é a primeira defesa de segurança do próprio desenho
**antes** de qualquer extensão de segurança existir: *"não dá para descartar
o estado de roteamento de um nó inundando o sistema com nós novos.
Kademlia só insere os novos quando os antigos saem"* (§2.1, "second benefit
of k-buckets"). Isso volta no §4.2 — é o motivo pelo qual o Kademlia puro já
nasce parcialmente resistente a Eclipse, e por que o S/Kademlia decide não
mexer nessa regra.

### 1.3 As quatro RPCs

`PING`, `STORE`, `FIND_NODE`, `FIND_VALUE` (§2.2).

- **PING** — verifica se o nó está ativo.
- **STORE** — instrui o destinatário a guardar um par `⟨chave,valor⟩`.
- **FIND_NODE** — recebe um ID de 160 bits; o destinatário devolve as
  triplas dos **k** nós mais próximos daquele ID que ele conhece (podem vir
  de vários buckets, se o mais próximo não tiver k entradas).
- **FIND_VALUE** — comporta-se como `FIND_NODE`, **exceto** que, se o
  destinatário já recebeu um `STORE` para aquela chave, devolve o valor
  direto em vez das triplas.

Toda RPC ecoa um **ID de requisição aleatório de 160 bits**, que dá
resistência **básica** contra falsificação de endereço (§2.2) — não é
autenticação, é só amarrar pergunta e resposta.

### 1.4 A busca (node lookup): o coração do algoritmo

Localizar os `k` nós mais próximos de um ID-alvo é a operação sobre a qual
tudo mais se constrói. O algoritmo (§2.2, "the most important procedure"):

1. O iniciador escolhe **α** nós do seu k-bucket não-vazio mais próximo do
   alvo (`α` é um parâmetro de **concorrência**, exemplo dado: `α = 3`).
2. Manda `FIND_NODE` **em paralelo e assíncrono** para esses α nós.
3. Passo recursivo: à medida que as respostas chegam, reenvia `FIND_NODE`
   para os α nós **ainda não consultados** dentre os k mais próximos já
   conhecidos. A recursão **pode começar antes de todas as α respostas
   anteriores voltarem**.
4. Se uma rodada não trouxer nó mais próximo que o já visto, reenvia para
   **todos** os k mais próximos ainda não consultados.
5. Termina quando o iniciador consultou e obteve resposta dos k nós mais
   próximos que já viu.

O ganho de projeto sobre um Chord com `α = 1`: nós que não respondem **não
seguram** a busca com timeout — a concorrência assíncrona absorve a falha
sem atraso imposto ao usuário (é a frase de abertura do próprio abstract do
paper: *"tolerate node failures without imposing timeout delays on users"*).

**O tempo é logarítmico e o paper prova isso** (§3, esboço de prova): com a
invariante "todo k-bucket tem pelo menos um contato, se existir nó na
faixa", a busca leva `h - log k` passos, onde `h` é a "profundidade" do nó
mais próximo do alvo — e a profundidade típica de um sistema com `n` nós é
`O(log n)`.

### 1.5 Armazenar, republicar, expirar — a parte de "base de dados"

- **STORE**: o publicador localiza os k nós mais próximos da chave e manda
  `STORE` a cada um.
- **Republicação horária**: cada nó **republica** todo `⟨chave,valor⟩` que
  guarda **a cada hora** — isso garante persistência com alta probabilidade
  mesmo com nós entrando e saindo (o próprio paper mostra, na prova, que a
  chance de o par sobreviver a uma hora é `1 - 2^(-k)`).
- **Republicação do publicador original a cada 24h**, e **expiração aos 24h
  contados da publicação original** se ninguém republicar — limite explícito
  para conter informação obsoleta (§2.2).
- **Consistência por transferência**: se um nó `w` percebe um nó novo `u`
  mais próximo de alguma das suas chaves, `w` **replica** o par para `u`
  **sem apagar o seu próprio** (§2.2) — é replicação por convergência, sem
  coordenador.
- **Cache ao longo do caminho**: por causa da unidirecionalidade da métrica,
  buscas futuras tendem a bater em um nó que cacheou o par antes de chegar
  ao dono "de direito"; o tempo de expiração do cache é **inversamente
  proporcional ao número de nós entre o nó atual e o dono mais próximo**,
  para evitar "over-caching" de chaves populares (§2.2).
- **Refresh de bucket**: se um bucket não foi tocado por lookup em uma hora,
  o nó escolhe um ID aleatório na faixa do bucket e faz uma busca — mantém
  o roteamento quente mesmo sem tráfego orgânico.

**A base de dados distribuída aqui é isto, e nada mais**: um mapa
`⟨chave,valor⟩` eventualmente consistente, sem transação, sem esquema, sem
unicidade imposta (`STORE` não confere nada antes de gravar — quem manda por
último "ganha" o slot local daquele nó, e cada nó guarda a sua própria
cópia), e sem garantia de durabilidade além de "sobrevive enquanto pelo
menos um dos k nós mais próximos estiver de pé e alguém republicar". Guarde
isso — é o argumento central do §5.1.

### 1.6 Ingresso na rede

Um nó novo `u` precisa de um contato `w` já participante. Insere `w` no
bucket certo, faz um lookup pelo **próprio ID**, e então dá refresh em todos
os buckets mais distantes que o vizinho mais próximo — isso popula os
próprios buckets de `u` **e** insere `u` nos buckets alheios, como
subproduto do tráfego (§2.2). Não há registro central, não há lista fixa de
sementes além de um único contato inicial (na prática, implementações usam
uma lista de *bootstrap nodes* conhecidos — o BEP 5, §3.2, formaliza isso).

---

## 2. eMule / eDonkey2000 — o que a casa pode aprender sem tocar no `.cpp`

Esta seção descreve **comportamento e formato de protocolo**, lidos em
documentação técnica (wikis oficiais do projeto e um artigo acadêmico sobre
o protocolo) — nunca no fonte GPL. Onde a fonte é um wiki de usuário e não
uma especificação formal, o texto diz isso.

### 2.1 eDonkey2000 — o antecessor centralizado, para contraste

O eDonkey2000 original **depende de servidores** para descoberta: o cliente
se conecta a um servidor conhecido, que indexa quem tem qual arquivo; a
busca por fonte é uma pergunta ao servidor, não uma busca distribuída. O
Kad (a seguir) nasceu **dentro do eMule** como alternativa **sem** esse
servidor central — é uma aplicação prática do Kademlia, junto com o
BitTorrent e o OverNet, citada nominalmente como tal já na introdução do
paper do S/Kademlia (§1: *"All widely deployed structured overlay networks
used in the Internet today (i.e. BitTorrent, OverNet and eMule) are based
on the Kademlia protocol"*).

O identificador de conteúdo do eDonkey2000/eMule é o **hash ed2k**: o
arquivo é dividido em blocos de **9.500 KiB (9.728.000 bytes)**; cada bloco
recebe um hash **MD4** de 128 bits; se houver mais de um bloco, o hash
final do arquivo é o MD4 da **concatenação dos hashes de bloco**; um único
bloco usa o próprio MD4 dele sem outra etapa (confirmado em múltiplas
fontes independentes de especificação do formato ed2k, incluindo a
implementação de referência do RHash e a página da AniDB sobre o
algoritmo). **MD4 está quebrado como função de hash criptográfico desde os
anos 1990** (colisões práticas conhecidas) — o ed2k hash serve para
**identificar** e **deduplicar** arquivos entre pares que confiam uns nos
outros, não para prova de integridade contra um adversário ativo que queira
forjar uma colisão. É o ponto de contraste mais direto com esta casa: o
PhxSql já não usa MD4 nem SHA-1 para nada que precise resistir a
adversário — SHA-256 para o hash do bloco do ledger (`ledger.rs`), PBKDF2-
HMAC-SHA256 para senha, HMAC-SHA256 no desafio de login. Não há nada a
"trazer" do ed2k hash — ele é o exemplo do que **não** copiar.

### 2.2 Secure User Identification — desafio-resposta com RSA

Fonte: **página "Secure User Identification" do wiki oficial do aMule**
(`wiki.amule.org/wiki/Secure_User_Identification`), que descreve o
comportamento do protocolo (não código).

O problema que resolve: impedir que um cliente **falsifique a identidade**
de outro para roubar o crédito acumulado dele (§2.3) — sem isso, bastaria
anunciar o mesmo "user hash" de outro cliente popular.

O mecanismo, tal como descrito:

1. No primeiro início, o cliente gera um **par de chaves RSA de 384 bits**,
   guardado em `cryptkey.dat` — perder o arquivo perde todo o crédito
   acumulado, porque a identidade **é** aquela chave.
2. Ao conectar, cada lado manda a **chave pública** e um **número
   aleatório**.
3. Cada lado assina, com a **própria chave privada**, uma mensagem que
   amarra a chave pública do outro e o número aleatório recebido.
4. O outro lado confere a assinatura contra a chave pública anunciada.

**384 bits de RSA é fraco para os padrões de hoje** — quebrável por força
computacional muito menor que a de uma assinatura Ed25519 de 256 bits de
segurança equivalente, e o próprio wiki não discute isso como limitação (o
protocolo é de 2004). O ponto qualitativo que interessa, e que **converge**
com o que esta casa já decidiu: **a identidade é a posse de uma chave
privada, provada por desafio, e não um segredo compartilhado guardado nos
dois lados** — exatamente a diferença que o comentário do `ed25519.rs` já
registra (*"a chave prova que você TEM alguma coisa"*, diferente de um
desafio-resposta onde o servidor guarda o que a prova usa). O PhxSql **já
está à frente** aqui em três eixos: (a) Ed25519 de 256 bits contra RSA-384;
(b) a chave nunca precisa estar no `config.json` do servidor, como o
comentário do `ed25519.rs` já explica; (c) o canal inteiro já é cifrado e
autenticado por Noise_NX (`fio.rs`, `docs/CIFRA-DO-FIO.md`) — o eMule
autentica identidade **sobre um canal em claro** (a cifra RC4 do §2.5 não é
autenticação, é ofuscação).

### 2.3 AICH — a árvore de hash, e por que ela importa para o ledger

Fonte: página **"AICH" do wiki oficial do aMule**
(`wiki.amule.org/wiki/AICH`).

O problema: um único hash MD4 por arquivo (ou até por bloco de 9,5 MiB) só
diz "corrompeu em algum lugar deste bloco inteiro" — obrigando a rebaixar
ou redescarregar o bloco todo diante de um único bit errado.

A solução: cada bloco de 9,5 MiB é dividido em **53 partes** (52 de 180 KiB
mais uma de 140 KiB), cada parte ganha um **hash SHA-1** (o "Block Hash"),
e os hashes de blocos são combinados **dois a dois, subindo em árvore**
— cada nó interno é um **"Verifying Hash"**, e o topo é o **"Root Hash"**.
Isto é uma **árvore de Merkle** na prática, ainda que o eMule não use esse
nome. A vantagem prática direta: dado o Root Hash confiável (normalmente
vindo dentro do link `ed2k://`), o cliente pode pedir só a trilha de
Verifying Hashes necessária para confirmar **uma parte específica de 180
KiB**, sem precisar da árvore inteira nem do arquivo inteiro — corrupção se
localiza e se corrige **em pedaço**, não em bloco inteiro.

**O ponto frágil, e ele é o achado de segurança mais direto desta seção**:
quando o link `ed2k://` **não** trouxe o Root Hash (existem casos onde o
link é só o hash do arquivo, sem o AICH), o wiki descreve um mecanismo de
**confiança por consenso**: o cliente aceita um Root Hash desconhecido só
se **pelo menos 10 outros clientes mandarem o mesmo Root Hash, e isso for
92% ou mais dos Root Hashes recebidos** — e mesmo assim só vale **para a
sessão corrente**, nunca é persistido. É um quórum informal contra
poluição, mas é um quórum sobre **identidades que ninguém confere** — é
precisamente o desenho que um ataque Sybil (§4.1) atropela: gerar mais de
10 identidades baratas that concordam entre si custa pouco, e o eMule/Kad
já teve isso medido na prática (§4.4). SHA-1 também está formalmente
quebrado para resistência a colisão desde 2005 (ataques teóricos) e na
prática desde 2017 (SHAttered) — mais um motivo para não portar o algoritmo
como está, só a **forma**.

**A forma É reaproveitável, e ela aponta direto para o `ledger.rs` desta
casa.** Hoje `verificar_cadeia` (`crates/phxsql-store/src/ledger.rs:326`)
percorre a cadeia **inteira**, recalculando o hash de **cada** linha e
conferindo a ligação — é `O(N)` no número de blocos, exatamente como
verificar um arquivo inteiro contra um hash único no ICH antigo do eMule.
Uma árvore de hash à la AICH sobre **faixas de altura** do ledger — por
exemplo, folhas de 1.024 alturas, combinadas em árvore até uma raiz —
permitiria a um verificador externo (ou a uma réplica, ou a um auditor)
confirmar que um **segmento específico** da cadeia bate com uma raiz
publicada, em `O(log N)` hashes, sem reprocessar as alturas anteriores.
Isto volta com desenho concreto no §5.3, porque é exatamente a peça que
falta à proposta do DBA em `docs/propostas/dba-bases-2026-09.md §2.1`
("a cadeia não tem âncora fora de si") — e o SHA-256 já escrito à mão nesta
casa (`crates/phxsql-core/src/hash.rs`) é superior ao SHA-1 do AICH sem
custo de licença nem de dependência nova.

### 2.4 O sistema de crédito — fórmula e a fraqueza estrutural

Fórmula, tal como documentada de forma consistente por múltiplas fontes de
comunidade sobre o "Official Credit System" do eMule (a wiki oficial trata
o tema em `CreditSystems`; a fórmula abaixo é a mesma citada de forma
convergente por documentação de mods derivados do cliente oficial):

```
razao_1 = (total_baixado_do_par × 2.0) / total_enviado_ao_par
razao_2 = sqrt(total_baixado_do_par_em_MiB + 2)
modificador = min(razao_1, razao_2), sujeito a min 1.0 e max 10.0
```

Casos de borda: se o total baixado é menor que 1 MiB, o modificador é fixo
em 1.0 (ninguém ganha prioridade por reciprocidade insignificante); se não
há histórico de envio, o cálculo parte de 10.0 (prioridade máxima para
quem nunca recebeu nada do par).

**A fraqueza estrutural, visível na própria descrição do mecanismo**: os
contadores de bytes trocados são **contados e guardados por cada lado,
localmente** e amarrados à identidade pela assinatura RSA do §2.2 — a
assinatura prova **quem** está falando, não **quanto de fato foi
transferido**. Não há terceiro confirmando a contagem de bytes; um cliente
modificado que mentisse sobre o próprio histórico de download/upload
mudaria o modificador que ele oferece ao par, sem que o par tenha como
auditar a alegação além de observar o próprio tráfego direto com aquele
cliente. Isto não é uma "descoberta" desta pesquisa — é uma consequência
direta e visível da própria descrição do mecanismo, e por isso está descrita
como **inferência sobre o desenho**, não como exploração medida (nenhuma
fonte consultada citou um número de fraude de crédito medido em produção).
**Nada disso serve de peça para o PhxSql**: um sistema de crédito por
reciprocidade não tem aplicação num motor de dados corporativo — é citado
aqui só porque a pergunta do dono cobre "o que eles já sofreram", e a
resposta honesta é "o crédito é auto-declarado e assinado, não auditado por
terceiro".

### 2.5 Ofuscação de protocolo — RC4, e o que ela explicitamente NÃO promete

Fonte: página **"Protocol obfuscation" do wiki oficial do eMule**
(`wiki.emule-web.de/Protocol_obfuscation`).

O desenho: a chave RC4 de envio é `MD5(user_hash || valor_mágico_34 ||
parte_aleatória)`; a de recepção troca o valor mágico para `203`; o lado
que recebe inverte os papéis. Os primeiros **1024 bytes** da saída do RC4
são **descartados** antes de usar o restante como keystream — mitigação
conhecida contra o viés estatístico inicial do RC4. O aperto de mão começa
com bytes semi-aleatórios em claro, seguidos de payload cifrado negociando
o método.

**O próprio wiki é explícito sobre o limite, e essa frase é o achado mais
citável desta subseção**: a ofuscação dá apenas *"proteção muito limitada
contra escuta passiva"*, admite que *"ainda é possível detectar o protocolo
eMule"*, e — a frase decisiva — **"não aumenta o anonimato nem esconde quais
arquivos são compartilhados"**. RC4 em si está formalmente **quebrado**
como cifra de fluxo há mais de uma década (vieses estatísticos exploráveis,
RFC 7465 proíbe seu uso em TLS desde 2015). Isto não é confidencialidade —
é ofuscação de assinatura de tráfego, um objetivo diferente e mais fraco.

**O contraste com esta casa é direto e favorável**: o PhxSql/PhxMail já não
"ofusca" — o `fio.rs` implementa `Noise_NX_25519_ChaChaPoly_SHA256`
(`docs/CIFRA-DO-FIO.md`), que dá **confidencialidade e autenticação
criptográfica reais** com primitivas modernas (X25519, ChaCha20-Poly1305,
SHA-256), todas conferidas contra vetor oficial (RFC 7748, RFC 8439, FIPS
180-4). Não há nada a copiar do RC4 do eMule — ele é, de novo, exemplo do
que a pétrea de zero dependências já resolveu melhor com peça própria.

---

## 3. Sistemas similares — o que cada um contribui de fato

### 3.1 BitTorrent Mainline DHT (BEP 5)

Fonte: **especificação BEP 5, bittorrent.org** (lida por inteiro).

É a aplicação mais próxima do paper original ainda em produção massiva
hoje. Diferenças concretas em relação ao Kademlia "de livro":

| | Kademlia (paper) | Mainline DHT (BEP 5) |
|---|---|---|
| Espaço de ID | 160 bits, genérico | 160 bits, casado com o infohash do torrent |
| Tamanho do k-bucket | `k` genérico (exemplo 20) | **8** por bucket |
| Estado do contato | não formalizado | **good** (respondeu em 15 min), **questionable**, **bad** (falhou repetido) |
| Refresh | 1 hora sem lookup na faixa | **15 minutos** |
| RPCs | `PING/STORE/FIND_NODE/FIND_VALUE` | `ping/find_node/get_peers/announce_peer` — troca `STORE` genérico por um par especializado em anunciar "quem baixa este torrent" |
| Proteção contra anúncio de terceiro | nenhuma no paper original | **token**: `get_peers` devolve um token opaco (`SHA1(IP || segredo rotativo)` na implementação de referência); `announce_peer` só é aceito se vier com um token emitido recentemente (~10 min) pelo mesmo nó consultado |

O `token` do BEP 5 é a peça de segurança mais concreta e barata desta
seção: impede que um nó qualquer anuncie "estou compartilhando o arquivo X"
em nome de um IP que nunca pediu nada — sem ele, qualquer um poderia
envenenar a lista de fontes de um torrent popular anunciando falsamente
por outros IPs. **Isto é diretamente análogo, em espírito, ao nonce do
`desafio.rs` desta casa** (provar que a resposta é fresca e amarrada a uma
pergunta recente do próprio verificador) — mas resolvendo um problema
diferente (impedir anúncio de terceiro numa DHT aberta, não autenticar
login).

### 3.2 IPFS / libp2p — Kademlia com endereçamento por conteúdo

Fonte: **especificação Kad-DHT do IPFS** (`specs.ipfs.tech/routing/kad-dht/`,
lida por inteiro) e documentação do libp2p.

Diferenças em relação ao Kademlia clássico:

- **Espaço de 256 bits**, via **SHA-256** do Peer ID — não 160 bits/SHA-1.
  Ponto de convergência com esta casa: SHA-256 já é o hash primário do
  PhxSql inteiro.
- **`k = 20`** nós mais próximos por chave, igual ao exemplo do paper
  original.
- Além de `FIND_NODE`, `GET_VALUE`/`PUT_VALUE` (registros genéricos,
  usados por exemplo para `/pk/` — chaves públicas grandes — e `/ipns/` —
  nomeação mutável), há um par **específico para conteúdo**:
  `ADD_PROVIDER`/`GET_PROVIDERS`. Um nó que possui um bloco de dado
  identificado por um CID (hash do conteúdo) manda `ADD_PROVIDER` aos k
  nós mais próximos **do CID**, anunciando "eu tenho este conteúdo". A
  especificação **exige** duas checagens antes de um servidor da DHT
  aceitar o registro: validar a chave e **conferir que quem mandou é quem
  diz ser** (o Peer ID do remetente bate com o anunciado).
- **Expiração em 48 horas** sem republicação — mais que o dobro das 24h do
  paper original.
- **Modo cliente/servidor explícito**: um nó pode participar só como
  cliente (consulta, não guarda nada de terceiro) — separação que o paper
  original de 2002 não previa, e que reduz a superfície de quem pode ser
  usado para poluir a tabela de outros.
- **Filtro de diversidade de IP**, citado explicitamente como mitigação
  contra Sybil: limitar quantos nós de um mesmo bloco de IP entram na
  mesma vizinhança da tabela de roteamento de alguém.

O **CID (Content Identifier)** do IPFS é, na essência, o mesmo conceito que
o `hash` do modo ledger desta casa: um identificador **derivado do
conteúdo** e não escolhido por quem publica — e por isso a peça mais
transferível para o PhxBlockchain é conceitual, não de protocolo: qualquer
coisa endereçada por hash do próprio conteúdo herda automaticamente
detecção de adulteração (mudar o conteúdo muda o endereço).

### 3.3 Bitswap — o par de exchange do IPFS, não uma DHT

Fonte: documentação e especificação do Bitswap (`docs.ipfs.tech`,
`specs.ipfs.tech/bitswap-protocol/`).

Bitswap **não** é uma DHT — é o protocolo de **troca de blocos** entre pares
que já sabem uns dos outros (a DHT resolve "quem tem", o Bitswap resolve
"me dá"). Mecanismo: cada nó mantém uma **want-list** (lista de CIDs
desejados) e a anuncia aos pares conectados; ao receber um bloco, cada nó
confere se algum peer quer aquele bloco e o encaminha. Um pedido em duas
fases — `want-have` (pergunta "você tem?", resposta leve) seguido de
`want-block` só para quem confirmou ter — evita puxar o bloco inteiro de um
peer que não o possui. **Sem aplicação direta ao PhxSql/PhxMail** — é
resolvendo um problema de distribuição em massa de blocos imutáveis entre
desconhecidos, que não é o modelo de nenhum dos três produtos da casa.
Citado por completude, porque a pergunta do dono nomeou "IPFS/libp2p" como
alvo de estudo.

### 3.4 S/Kademlia — a versão "com segurança em primeiro lugar"

Ver §4 — o S/Kademlia é tratado por inteiro na seção de segurança, porque
ele **é** uma resposta de defesa a ataques específicos, e separar os dois
fragmentaria o argumento.

### 3.5 GNUnet / R5N

Fonte: **draft R5N do GNUnet**, `draft-schanzen-r5n` (IETF, versão lida via
`lsd.gnunet.org/lsd0004/`).

R5N ("randomized recursive routing for restricted-route networks") resolve
um problema que nem o Kademlia nem o S/Kademlia endereçam: redes onde
**nem todo par de nós consegue se falar diretamente** (redes com NAT
simétrico severo, roteamento restrito, ou more geralmente topologias que
não são um grafo completo). A solução é híbrida: os primeiros
`log2(tamanho estimado da rede)` saltos são **roteamento aleatório**
(escapa de mínimos locais impostos pela topologia real), e só depois entra
o roteamento por distância XOR determinístico, como no Kademlia clássico.

**O ponto mais relevante para a §4 desta pesquisa é uma postura de
segurança explícita e honesta**: o próprio draft assume que **uma fração
dos nós é maliciosa por padrão** ("the network is open and thus a fraction
of the participating peers is malicious"), e admite abertamente que
**Sybil é possível e não é impedido por votação ou reputação** — a defesa
do R5N não é impedir múltiplas identidades, é assumir que **nós honestos
mantêm conexões diretas suficientes entre si** para que o roteamento
aleatório encontre caminho por fora do cerco. Não há peça de código
reaproveitável aqui (é um design de roteamento sobre restrição de rede que
o PhxSql não tem — os servidores do cluster conversam diretamente uns com
os outros, listados em configuração), mas a **postura** — nomear a fração
adversária assumida em vez de fingir que "a rede é confiável por padrão" —
é o mesmo espírito que move a §4 inteira.

---

## 4. Segurança em primeiro lugar — ataques medidos × defesas com paper

### 4.1 Sybil — a impossibilidade que baliza tudo o mais

Fonte: **Douceur, J. R. — "The Sybil Attack", IPTPS 2002**.

O resultado central do paper, e ele é uma prova de impossibilidade, não uma
sugestão: **sem uma autoridade logicamente centralizada certificando
identidades, um atacante com recursos suficientes sempre consegue forjar
identidades bastantes para dominar um sistema**, exceto sob premissas
extremas e irrealistas de paridade de recursos e coordenação entre
participantes honestos. Ou seja: **não existe defesa puramente algorítmica
e descentralizada que elimine Sybil** — só existem defesas que **encarecem**
criar identidade (prova de trabalho, custo de bandwidth, verificação
externa como número de telefone) ou que **limitam o dano** de uma
identidade forjada depois que ela já existe (redundância, roteamento
disjunto, votação por peso de reputação).

Isto é a régua contra a qual toda "defesa contra Sybil" desta seção precisa
ser lida: nenhuma delas **impede**, todas **encarecem** ou **contêm**.

### 4.2 Eclipse — isolar um nó cercando-o de adversários

Descrito no §3.2 do paper do S/Kademlia, citando Singh, Ngan, Druschel &
Wallach ("Eclipse attacks on overlay networks: Threats and defenses",
INFOCOM 2006) como referência original: o atacante posiciona nós
adversários de forma que **todo** o tráfego de/para uma vítima passe por
pelo menos um deles — na prática, "sequestrando" a visão de mundo daquele
nó sobre a rede inteira.

O S/Kademlia observa (§3.2) que o Kademlia **puro já dificulta parcialmente**
isto, por duas propriedades que já existem sem extensão nenhuma: (a) um nó
não escolhe o próprio ID livremente contra um alvo específico, se o ID vier
de hash de chave pública; e (b) a regra de k-bucket do §1.2 desta pesquisa
**favorece nós de vida longa e só insere novos quando os antigos saem** —
inundar a vizinhança de alguém com nós novos não desloca ninguém que já
esteja lá e responda a PING.

### 4.3 Envenenamento de roteamento adversarial, e a defesa medida do S/Kademlia

Fonte: **Baumgart & Mies, "S/Kademlia", 2007** (paper lido por inteiro,
§3.2 e §4).

O ataque: um nó adversário, ao responder `FIND_NODE`, devolve só nós
colaboradores (também adversários) "mais próximos" do alvo — dirigindo a
busca inteira para dentro de uma sub-rede de cúmplices, sem que o alvo real
seja encontrado.

A defesa proposta, **medida por simulação** no próprio paper (framework
OverSim, N = 10.000 nós):

- **Buscas por caminhos disjuntos**: em vez de uma única busca convergente,
  o iniciador espalha os k nós mais próximos conhecidos em **d** buckets de
  busca independentes e roda **d buscas paralelas**, cada nó usado **uma
  única vez** em todo o processo, garantindo que os caminhos não se
  cruzem. A busca é bem-sucedida se **ao menos um** dos d caminhos não
  tocar nenhum nó adversário.
- **Fórmula da probabilidade de sucesso**, dada a fração `m` de nós
  adversários, `d` caminhos disjuntos e a distribuição `(h_i)` de
  comprimento de caminho:
  `P_K = Σ h_i · (1 - (1 - (1-m)^i)^d)`.
- **Resultado medido por simulação**: com `d = 8` caminhos e bucket
  `k = 16`, **mesmo com 20% dos nós da rede adversários, 99% das buscas
  ainda tiveram sucesso**. Com `d = 1` (Kademlia puro), a taxa de sucesso
  cai visivelmente mais rápido conforme a fração adversária cresce (Figura
  4 do paper). Os autores recomendam `d = 4..8` com `k = 8..16` como ponto
  de equilíbrio entre resiliência e overhead de comunicação (que cresce
  linearmente com `d`).

**Geração segura de ID — a peça mais transferível de todo o documento**
(§4.1 do paper): o S/Kademlia recomenda o ID do nó ser o **hash da chave
pública** (não hash de IP:porta, que muda com DHCP e não impede múltiplas
identidades atrás de um NAT), e introduz dois **quebra-cabeças
criptográficos** (crypto puzzles) para encarecer gerar identidades:

- **Puzzle estático**: gerar um par de chaves tal que os primeiros `c1`
  bits de `H(H(chave_pública))` sejam zero — encarece escolher o ID
  livremente (defesa direta contra Eclipse: não dá para mirar um ID
  específico sem repetir o puzzle).
- **Puzzle dinâmico**: junto com o ID já fixado, encontrar um `X` tal que
  os primeiros `c2` bits de `H(ID ⊕ X)` sejam zero — encarece gerar
  **muitas** identidades em volume (defesa direta contra Sybil, no espírito
  de "não impede, encarece" do §4.1).
- Mensagens carregam **assinatura fraca** (cobre só IP/porta/timestamp, para
  `PING`/`FIND_NODE`, onde integridade total do corpo é dispensável) ou
  **assinatura forte** (cobre a mensagem inteira, contra homem-no-meio, com
  nonce contra replay).
- **Lista de "siblings"** de tamanho `η·s` (o paper prova, reaproveitando
  resultado de Gai & Viennot sobre a DHT Broose, que `η ≥ 5` basta com alta
  probabilidade) — resolve replicação segura de dado por maioria mesmo sob
  nós adversários na vizinhança imediata da chave.

### 4.4 Não é só teoria — Sybil e Eclipse já aconteceram nas redes reais desta família

Duas medições em produção, contra redes que são literalmente as citadas
pelo dono como objeto de estudo:

- **Kad (a DHT do próprio eMule), medida em produção por Steiner,
  En-Najjary & Biersack** ("Poisoning the Kad network"; "eDonkey & eMule's
  Kad: Measurements & Attacks", *Fundamenta Informaticae* 109, 2011). O
  achado central, segundo o resumo da própria pesquisa disponível: **montar
  um ataque Sybil em Kad é fácil e barato**, e permite comprometer a
  privacidade dos usuários, corromper o funcionamento correto da busca por
  chave e montar negação de serviço distribuída com pouquíssimo recurso —
  os autores chegaram a propor amarrar o ID Kad à posse de um número de
  telefone celular como mitigação de custo de identidade (o mesmo princípio
  do §4.1: não impedir, encarecer). É a prova de que a rede de Kad do
  próprio eMule — sem nenhuma das extensões do S/Kademlia — já sofreu isso
  fora do laboratório, não só em simulação.
- **IPFS, ataque de Eclipse de ponta a ponta, CVE-2020-10937**
  ("Total Eclipse of the Heart — Disrupting the InterPlanetary File
  System", USENIX Security 2022; Prünster et al.). Os pesquisadores
  exploraram uma falha conceitual numa biblioteca central do libp2p e
  demonstraram, de ponta a ponta contra a rede pública do IPFS v0.4.23,
  que **qualquer nó pode ser isolado (eclipsado) com esforço moderado**: a
  tabela de roteamento de um nó-alvo é totalmente envenenada em **minutos**,
  e o cerco completo se fecha em **menos de uma hora**, sem precisar de
  ataque de negação de serviço adicional — usando uma lista pré-gerada de
  29 TB de Peer IDs para vencer o sistema de reputação do libp2p. A
  divulgação responsável levou a mitigações reais (o **filtro de
  diversidade de IP** citado no §3.2 desta pesquisa nasceu, em parte, desse
  episódio).

**A lição que as duas medições ensinam junto**: nenhuma das duas redes
tinha, na época do ataque, a defesa mais barata e mais antiga do próprio
paper original do Kademlia — **ID amarrado a algo que custa** (o S/Kademlia
propõe chave pública com puzzle; Steiner et al. chegaram a propor número de
telefone). Um ID de nó **livremente escolhido, sem custo nenhum para gerar**
é a raiz comum dos dois incidentes.

### 4.5 O que esta casa já tem escrito, e como ele se encaixa

| Peça que a defesa de DHT pede | Onde o PhxSql já tem o equivalente, escrito à mão |
|---|---|
| ID de nó = hash de chave pública, não escolhido livremente | `ed25519.rs` já gera assinatura a partir de chave pública/privada; um ID assim derivado (`SHA-256(chave_pública)`) é trivial de compor com peças existentes |
| Autenticação de mensagem (assinatura forte contra homem-no-meio) | `ed25519.rs` (assinatura) e o canal inteiro do `fio.rs` (Noise_NX, que já autentica o servidor pela chave estática) |
| Canal confidencial (o que RC4/ofuscação do eMule explicitamente NÃO dá) | `fio.rs`: ChaCha20-Poly1305 sobre X25519, RFC 8439/7748, já conferido contra vetor |
| Hash resistente a colisão para qualquer estrutura tipo Merkle/AICH | `hash.rs` (SHA-256), já usado no `ledger.rs`; muito acima do SHA-1 do AICH e do MD4 do ed2k |
| Desafio-resposta amarrado a nonce fresco | `desafio.rs`, já em produção para login |
| Quebra-cabeça de custo computacional (crypto puzzle) | **não existe** — seria a única peça genuinamente nova, e só faria sentido se e quando existir uma superfície de auto-cadastro de nó desconhecido (§5.1 explica por que essa superfície **não existe hoje** no PhxSql) |

A conclusão desta seção, antes de aplicar ao crivo: **a casa já tem, escrito
em Rust puro e conferido contra vetor, praticamente todo o material-base que
o S/Kademlia pede de uma implementação "com segurança em primeiro lugar"** —
falta só o quebra-cabeça de custo de identidade, e ele só teria função onde
houver ingresso de nó não previamente cadastrado, que é exatamente a
fronteira tratada a seguir.

---

## 5. Aplicação contra o crivo — o que cabe, e onde a pétrea barra

### 5.1 PhxSql — "base de dados distribuída" não é sinônimo de DHT

A resposta direta à âncora do dono, dita sem rodeio: **o PhxSql já é uma
base de dados distribuída** — replicação Source→Replica com quatro
servidores medidos (`docs/REPLICACAO.md`) e cluster com eleição e promoção
automática (`docs/CLUSTER.md §2`) — mas é distribuída num **modelo
diferente** do que uma DHT resolve, e os dois modelos não competem pelo
mesmo lugar:

| | DHT (Kademlia e família) | PhxSql hoje |
|---|---|---|
| Coordenação | **sem líder**, consistência eventual | **líder único**, um master por vez, réplicas seguem (`CLUSTER.md`) |
| Verdade sobre um dado | quem tem a cópia mais recente que alguém perguntou por acaso | **uma** linha em **um** lugar, com rowid próprio, `.reg`/`.ndx` |
| Integridade referencial | não existe — não há noção de "pai" e "filho" numa DHT | é a regra primordial da casa: *"nunca se mata o pai que tem filhos"*, imposta na gravação |
| Conflito de escrita | resolve depois, por convergência ou timestamp (como o próprio `CASSANDRA.md §4.6` já documentou para o Cassandra) | recusa **na hora**, com `conferir_versao` e a janela de conflito (`CLAUDE.md`) |
| Escrita aceita mesmo sem quórum alcançável | sim, por natureza | é justamente o que o pedido do quórum (`docs/REPLICACAO.md §19`, a rodada do quórum de 07/09) tratou como **decisão de produto**, não escolha técnica |

**Onde uma DHT ajudaria de verdade, sendo honesto sobre o tamanho do
ganho**: hoje o cluster do PhxSql tem a lista de nós **escrita no
`config.json`**, um bloco pequeno e explícito (`docs/CLUSTER.md §2.1`) —
"sem o bloco, nada muda". Isso é **exatamente** o cenário em que um
protocolo de descoberta tipo Kademlia **não compensa o custo**: a lista tem
poucos nós (quatro na bancada medida), muda por decisão humana explícita
(`cluster_no_acrescentar`/`cluster_no_remover`, pedido 217), e o ganho de
uma DHT — logaritmo do número de saltos para achar um nó entre milhares —
não existe quando o número de nós é dezenas, não milhares. **Medir a
premissa antes do item**, como a pétrea manda: a pergunta que decidiria se
vale a pena é *"o PhxSql algum dia terá centenas ou milhares de nós de
cluster descobrindo-se sozinhos, sem lista explícita?"* — e a resposta hoje,
pelo desenho documentado, é **não**: o cluster é pensado para um número
pequeno de servidores conhecidos de uma organização, não uma rede aberta
de participantes anônimos. Onde a DHT **poderia** um dia caber, se essa
premissa mudar, é como **camada de descoberta de rede** — "quem são os
outros nós e qual o endereço deles agora" — nunca como o **armazém
relacional**: uma DHT não tem chave estrangeira, não tem unicidade
conferida na gravação, não tem transação, e aceitar essas garantias
"depois, por reconciliação" é o oposto do que a regra primordial da
integridade exige ("recusar cedo custa um erro lido... recusar tarde custa
um banco inteiro modelado errado", `CLAUDE.md`).

**Recusa medida, com o número que a sustenta**: implementar Kademlia dentro
do PhxSql hoje seria a mesma classe de erro que o `docs/propostas/
dba-bases-2026-09.md` já registrou para o Paxos do Cassandra — *"resolve
compare-and-set num cluster sem líder; nós somos single-leader... zero
convergência dos motores de referência"*. O mesmo raciocínio vale aqui:
zero dos quatro motores de referência da régua ponderada da casa
(PostgreSQL, MariaDB, MySQL, SQLite) usa DHT para nada — é tecnologia de
outra família de sistema (compartilhamento de arquivo/mensageria entre
participantes anônimos), não de banco relacional transacional.

### 5.2 PhxMail — o que já convergiu, e o que o Kademlia ensinaria se a premissa mudasse

O desenho de hoje (`docs/CORREIO-DNS.md`, `docs/CORREIO-SEGURANCA.md`): um
servidor de e-mail por empresa, em **IP fixo**, publicado como registro `A`
no Cloudflare sob `<empresa>.phxmail.com.br`; o cliente resolve o nome, abre
a porta 8000 e faz aperto Noise; a **confiança do canal vem da fixação da
chave pública do servidor, nunca do DNS** — o próprio documento já diz isso
("DNS não autentica ninguém").

Comparando eixo a eixo com o que esta pesquisa levantou:

- **Descoberta de nó.** O Kademlia resolveria "ache o servidor de correio
  da empresa X" com uma busca distribuída por ID; hoje o PhxMail resolve
  isso com **um registro DNS fixo por empresa**. A DHT ganharia em não
  depender de nenhum provedor de DNS central — mas **perderia** em algo que
  o negócio de correio corporativo valoriza mais que descentralização pura:
  **um IP estável e auditável**, que um cliente de e-mail, um firewall
  corporativo ou uma política de compliance consegue fixar em allowlist.
  Um servidor de correio "flutuando" dentro de uma DHT aberta, sujeito a
  ser temporariamente cercado por Eclipse (§4.2, e **medido de verdade**
  contra o IPFS no §4.4) é um risco de disponibilidade que um e-mail
  corporativo não deveria correr. **Recusa, com o motivo escrito**:
  manter DNS fixo é decisão de produto, não lacuna técnica.
- **Identidade do nó.** Aqui sim há uma peça diretamente aproveitável, e é
  a mesma do §4.5: o S/Kademlia recomenda **ID = hash da chave pública**.
  O PhxMail **já faz exatamente isso em espírito** — a confiança não vem do
  nome DNS, vem da chave estática pinada do Noise (`CORREIO-SEGURANCA.md`,
  o par X25519). Não há nada a importar aqui; é uma **confirmação de que a
  escolha já feita converge** com o que a literatura de segurança de DHT
  recomenda para o mesmo problema (amarrar identidade a posse de chave, não
  a endereço de rede).
- **Autenticação entre pares.** A Secure User Identification do eMule
  (§2.2, RSA-384, desafio-resposta sobre canal em claro) é estruturalmente
  **mais fraca** que o que o PhxMail já tem: Noise_NX já autentica o
  servidor **dentro** de um canal cifrado, com uma curva mais forte
  (X25519/256 bits contra RSA-384). Não há ganho em "trazer" a Secure
  Identification — o PhxMail já resolveu o mesmo problema com peça
  superior.
- **Onde a DHT teria função real, se um dia a premissa mudar**: **não**
  entre empresas diferentes (aí o modelo fixo por tenant é o certo, pelos
  motivos acima), mas **dentro da própria frota de servidores de um mesmo
  operador**, se um dia o PhxMail precisar de múltiplos servidores por
  empresa se descobrindo automaticamente (hoje não é o desenho — é um
  servidor por empresa). Fica registrado como item de pesquisa futura, não
  como recomendação de implementar agora — de novo, "medir a premissa antes
  do item": hoje não existe múltiplo servidor por tenant documentado.

### 5.3 PhxBlockchain (o modo ledger encadeado do PhxSql, `ledger.rs`) — a peça com o ganho mais concreto

Esta é, das três aplicações, a que tem **transferência direta e de baixo
risco**, porque o modo ledger já foi desenhado com o mesmo princípio de
fundo do AICH e do CID do IPFS: **hash derivado do conteúdo, encadeado**.
O comentário do próprio `ledger.rs` já registra isso — o SHA-256 do
conteúdo canônico da linha, nunca dos bytes crus do `.reg` (`ledger.rs`,
"redige ANALISANDO, nunca recortando").

**Onde a lacuna está, medida no próprio código**: `verificar_cadeia`
(`crates/phxsql-store/src/ledger.rs:326` em diante) percorre **cada linha
da cadeia, em ordem, do gênesis até o topo**, recalculando o hash de cada
uma — é exatamente o modelo do **ICH antigo** do eMule (hash único cobrindo
tudo, sem estrutura de árvore), o que o **AICH substituiu** (§2.3) por um
motivo preciso: verificar **um segmento** sem reprocessar **tudo**.

**A proposta concreta, no espírito de "inspiração, não cópia" (a forma da
árvore de Merkle é conhecimento público, não código GPL de ninguém)**:

- Agrupar as alturas do ledger em **faixas fixas** (por exemplo, 1.024
  alturas por folha — o número exato é decisão de medição, não desta
  pesquisa).
- Cada folha guarda o SHA-256 do **último hash de bloco daquela faixa**
  (que já encadeia todos os anteriores dela, por construção — o ledger já é
  uma cadeia, então a folha não precisa nem recalcular nada novo, só
  **apontar** para o hash que a cadeia já produziu na borda da faixa).
  Combinando folhas duas a duas sobe-se a uma raiz, exatamente como o AICH
  combina Block Hashes em Verifying Hashes.
- Um verificador externo — **ou a réplica que já registra `(altura, hash)`
  recebido pelo fio**, exatamente a ideia de "testemunha por réplica" que
  `docs/propostas/dba-bases-2026-09.md §2.1` já propôs contra o problema
  "a cadeia não tem âncora fora de si" — passaria a poder confirmar **uma
  faixa específica** contra a raiz publicada, sem precisar da cadeia
  inteira. Isto **não substitui** a testemunha por réplica proposta pelo
  DBA (aquela ainda é a âncora **externa ao arquivo**, contra um atacante
  que reescreve o arquivo inteiro e recalcula tudo); **complementa** ela,
  dando à própria réplica um jeito **barato** de conferir um segmento
  recebido sem reprocessar a cadeia toda a cada pulso — que é justamente
  o tipo de custo que esta casa já mediu como caro em outros lugares do
  motor (o `.ndx` inteiro sendo reconstruído por completo, por exemplo).
- **Onde a pétrea barra, dita cedo**: isto é **mudança de formato em disco**
  se a árvore for persistida (novas colunas ou uma tabela auxiliar de
  raízes por faixa) — entra na régua de "mudança de formato entra cedo",
  decisão do DBA/dono, não implementação direta desta pesquisa. Se a árvore
  for **recalculada sob demanda** a partir do que já existe (percorrendo o
  índice `porAltura` uma vez para montar as folhas), não muda formato
  nenhum — é só um modo de leitura auxiliar sobre o que já está gravado.
  A escolha entre as duas é do DBA, com o custo de cada uma medido antes.
- **Propagação por gossip** (a peça que o Bitcoin usa para propagar blocos
  entre pares, citada pelo dono): **não se aplica hoje**, porque o modo
  ledger do PhxSql não tem uma rede de pares anônimos propagando blocos —
  ele vive **dentro** de uma tabela comum, replicada pelo mecanismo
  Source→Replica já existente e medido (`docs/REPLICACAO.md`). Gossip
  resolve "como um bloco chega a milhares de nós que não se conhecem
  antecipadamente"; o PhxSql já sabe exatamente quem são as réplicas
  (config explícita), então o problema que o gossip resolve **não existe**
  aqui — é o mesmo argumento do §5.1 aplicado de novo.

---

## 6. O que esta pesquisa recusa, medido ou com o motivo escrito

Registro explícito, no molde de `docs/propostas/dba-bases-2026-09.md`
Tier 3 — recusa não é "não pensamos nisso", é "pensamos e o número/motivo
diz não":

- **Implementar Kademlia/uma DHT como camada de descoberta de cluster hoje
  — RECUSADO por premissa não verificada.** O ganho de uma DHT só aparece
  quando o número de nós é grande o bastante para que busca logarítmica
  valha o custo de manter k-buckets, republicação e refresh. O cluster do
  PhxSql tem a lista de nós em configuração explícita, pequena, mudada por
  decisão humana (`docs/CLUSTER.md §2.1`, §2.7). Não há medição de que essa
  premissa (poucos nós, config explícita) vá mudar — e "medir a premissa
  antes do item" é a lei que barra implementar isto sem essa medição
  primeiro.
- **Usar a DHT como armazém de dado relacional (qualquer parte do PhxSql
  em si) — RECUSADO por incompatibilidade estrutural, não por medição.**
  Uma DHT não tem chave estrangeira, não confere unicidade na gravação, não
  tem transação nem `ROLLBACK`, e resolve conflito por convergência
  eventual — o oposto direto da regra primordial da integridade ("nunca se
  mata o pai que tem filhos") e do que o `docs/ACID.md` já mede que o
  PhxSql garante hoje (leitura confirmada, escrita serializada por linha).
  Isto não é "ainda não medimos"; é incompatibilidade de modelo, do mesmo
  jeito que o Paxos do Cassandra foi recusado por não haver convergência
  dos quatro motores de referência.
- **Portar qualquer trecho de código do eMule/aMule (GPL) — RECUSADO por
  licença, sem exceção.** Coberto por inteiro no §0. Toda peça descrita
  neste documento veio de paper acadêmico, especificação aberta (BEP,
  IETF draft, spec do libp2p/IPFS) ou descrição de comportamento em wiki —
  nunca do `.cpp`/`.h`.
- **RSA-384 ou qualquer coisa no molde da Secure User Identification —
  RECUSADO por já estarmos à frente.** §2.2 e §4.5: Ed25519 + Noise_NX já
  entregam identidade amarrada a chave e canal autenticado, superior em
  tamanho de chave e em modelo de ameaça (o eMule autentica sobre canal em
  claro; o PhxMail autentica dentro de canal cifrado).
- **MD4 ou SHA-1 para qualquer hash novo — RECUSADO, sempre.** SHA-256 já é
  o padrão da casa (`hash.rs`), formalmente mais forte que os dois, e sem
  custo de dependência nova — trazer um hash mais fraco "porque é o que o
  eMule usa" seria regressão, não modernização.
- **RC4 ou "ofuscação" de protocolo em vez de cifra — RECUSADO.** O próprio
  wiki do eMule admite que ofuscação não é confidencialidade (§2.5). O
  PhxSql/PhxMail já tem confidencialidade real via Noise/ChaCha20-Poly1305;
  trocar por ofuscação seria trocar uma garantia forte por uma fraca.
  Quebra-cabeça de custo computacional (crypto puzzle, §4.3) — **NÃO
  recusado, mas adiado, com a premissa nomeada**: só faz sentido no dia em
  que existir superfície de auto-cadastro de nó não previamente conhecido
  (nem o cluster, nem o PhxMail entre tenants, nem o ledger têm essa
  superfície hoje). Fica registrado aqui para não ser esquecido, não para
  ser implementado agora.
- **Gossip de propagação de bloco (estilo Bitcoin) para o modo ledger —
  RECUSADO hoje, mesmo motivo do cluster.** O ledger vive dentro da
  replicação Source→Replica já existente, com réplicas conhecidas por
  configuração — o problema que o gossip resolve (propagar para uma rede
  aberta de desconhecidos) não existe no desenho atual.

---

## 7. Fontes

**Papers acadêmicos, lidos por inteiro:**

- Maymounkov, P.; Mazières, D. — *"Kademlia: A Peer-to-peer Information
  System Based on the XOR Metric"*, IPTPS 2002.
  `https://www.scs.stanford.edu/~dm/home/papers/kpos.pdf`
- Baumgart, I.; Mies, S. — *"S/Kademlia: A Practicable Approach Towards
  Secure Key-Based Routing"*, 2007.
  `https://telematics.tm.kit.edu/publications/Files/267/SKademlia_2007.pdf`
- Douceur, J. R. — *"The Sybil Attack"*, IPTPS 2002 (resumo/abstract e
  citação lidos via Microsoft Research e via referência cruzada no
  S/Kademlia §3.2/§6).
- Singh, A.; Ngan, T.-W.; Druschel, P.; Wallach, D. — *"Eclipse attacks on
  overlay networks: Threats and defenses"*, INFOCOM 2006 (citado e descrito
  via S/Kademlia §3.2 e §6, referência [15]).
- Steiner, M.; En-Najjary, T.; Biersack, E. — *"Poisoning the Kad network"*
  e *"eDonkey & eMule's Kad: Measurements & Attacks"*, Fundamenta
  Informaticae 109 (2011) — achados lidos via resumo/abstract das
  publicações (ResearchGate/ACM Digital Library, ver §4.4).
  `https://eprints.cs.univie.ac.at/5481/1/34_eDonkey.pdf`
- Prünster, B. et al. — *"Total Eclipse of the Heart — Disrupting the
  InterPlanetary File System"*, USENIX Security 2022, CVE-2020-10937 —
  achados lidos via resumo/abstract (arXiv 2011.00874, USENIX).

**Especificações abertas, lidas por inteiro:**

- BEP 5 — *Mainline DHT Protocol*, bittorrent.org.
  `https://www.bittorrent.org/beps/bep_0005.html`
- Kad-DHT — especificação de roteamento do IPFS.
  `https://specs.ipfs.tech/routing/kad-dht/`
- Bitswap — especificação de protocolo do IPFS.
  `https://specs.ipfs.tech/bitswap-protocol/`, `https://docs.ipfs.tech/concepts/bitswap/`
- R5N — *"The R5N Distributed Hash Table"*, draft IETF do GNUnet.
  `https://lsd.gnunet.org/lsd0004/`, `https://datatracker.ietf.org/doc/draft-schanzen-r5n/`

**Documentação de comportamento de protocolo (wiki oficial, NUNCA código):**

- *Secure User Identification*, aMule Project Wiki.
  `https://wiki.amule.org/wiki/Secure_User_Identification`
- *AICH*, aMule Project Wiki.
  `https://wiki.amule.org/wiki/AICH`
- *Protocol obfuscation*, eMule Wiki.
  `https://wiki.emule-web.de/Protocol_obfuscation`
- *CreditSystems*, eMule Wiki — fórmula do modificador de crédito
  (referência de comunidade, convergente entre múltiplas fontes
  independentes sobre o "Official Credit System").
  `https://wiki.emule-web.de/CreditSystems`
- Especificação do hash ed2k (MD4 por bloco de 9.500 KiB), conferida em
  múltiplas fontes independentes de implementação de referência (RHash,
  AniDB).
  `https://github.com/rhash/RHash/blob/master/librhash/ed2k.c`,
  `https://wiki.anidb.net/Ed2k-hash`

**Documentos internos citados:**

`phxsql/docs/CASSANDRA.md` (molde de tom e de contrato de citação),
`phxsql/docs/CLUSTER.md`, `phxsql/docs/REPLICACAO.md`,
`phxsql/docs/ACID.md`, `phxsql/docs/SOMBRA.md`,
`phxsql/docs/CORREIO-DNS.md`, `phxsql/docs/CORREIO-SEGURANCA.md`,
`phxsql/docs/CIFRA-DO-FIO.md`,
`phxsql/docs/propostas/dba-bases-2026-09.md` §2.1,
`phxsql/crates/phxsql-store/src/ledger.rs`,
`phxsql/crates/phxsql-core/src/ed25519.rs`,
`phxsql/crates/phxsql-core/src/x25519.rs`,
`phxsql/crates/phxsql-core/src/fio.rs`,
`phxsql/crates/phxsql-core/src/desafio.rs`,
`phxsql/crates/phxsql-core/src/hash.rs`.

---

## Dispensa registrada dos outros papéis, nesta frente

Esta é uma frente de **pesquisa e leitura** — nenhum código, nenhuma
compilação, nenhum commit, por instrução explícita do orquestrador (o motor
está com o P0 compilando, uma compilação por vez no pipeline).

- **B — engenheiro**: **dispensado**. Nenhuma linha de Rust foi escrita;
  toda proposta do §5 fica registrada como desenho para uma rodada futura,
  com o crivo já aplicado.
- **C — DBA**: **não dispensado, convocado por citação**: a proposta do
  §5.3 (árvore de hash por faixa de altura sobre o ledger) é dele decidir
  se persiste em disco (mudança de formato, entra cedo) ou fica como leitura
  auxiliar sob demanda (sem mudança de formato) — esta pesquisa nomeia a
  bifurcação, não decide por ele.
- **D — zelador**: **dispensado**. Nenhum ambiente foi sujado; os PDFs
  buscados ficaram no diretório de scratchpad da sessão, fora do
  repositório.
- **E — designer**: **dispensado**. Nenhuma tela muda com este documento.
- **F — prova real**: **dispensado nesta frente, e é decisão, não
  esquecimento**: não há código para provar nos dois sentidos — é
  literatura. Se o §5.3 virar código numa rodada futura, **aí** o papel F
  entra, com o defeito reposto de praxe (uma faixa adulterada tem de
  derrubar a verificação; uma faixa íntegra tem de passar).
- **G — QA**: **dispensado**. Nenhuma catraca nova; nenhum comportamento de
  motor mudou.
- **H — documentação**: **é este próprio documento** — o papel H aqui é o
  registro da pesquisa, no molde do `CASSANDRA.md`. Fica para o orquestrador
  decidir se o `docs/TECNOLOGIAS.md` ganha uma linha nesta rodada apontando
  para este arquivo, e se as três páginas do dossiê citadas no
  `CLAUDE.md` do projeto precisam de atualização — nenhuma delas é gerada a
  partir deste documento hoje.
- **I — versionador**: **dispensado por instrução explícita** — "não
  comite" veio na ordem da tarefa; quem integra e commita é o orquestrador.
