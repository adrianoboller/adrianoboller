# MCP por HTTP (N8n/Claude Code) e avisos por e-mail + REST — medido

Pesquisa do papel J (11/09/2026), pedida pelo orquestrador. Dois assuntos,
cada um com o que a especificação pede, o que já existe aqui (arquivo e
linha) e o veredito medido contra o nosso código — nenhum número abaixo é
citado de memória; o comando que o gerou está ao lado. **Nenhuma linha de
código de produção foi escrita nesta rodada**, e nada foi commitado.

---

## Parte 1 — MCP por HTTP, para N8n e Claude Code

### 1.1 O que a spec pede

MCP fala **JSON-RPC 2.0**: `initialize` → `notifications/initialized` →
`tools/list` / `tools/call` (e, fora do que este projeto fala hoje,
`resources/list` / `resources/read`, `prompts/*`). Dois transportes
padronizados:

- **stdio** — o cliente sobe o servidor como subprocesso e fala por
  `stdin`/`stdout`, uma mensagem JSON por linha, sem quebra de linha
  embutida.
- **Streamable HTTP** (substituiu o HTTP+SSE de 2024-11-05) — um **único**
  caminho HTTP que aceita `POST` e `GET`. Pontos medidos na especificação
  oficial (fonte: [modelcontextprotocol.io/specification/2025-06-18/basic/transports](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)):
  - Toda mensagem do cliente é um `POST` novo nesse endpoint.
  - O `POST` de uma *notificação* ou *resposta* recebe `202 Accepted` sem
    corpo.
  - O `POST` de um *pedido* pode ser respondido de **duas** formas, e o
    cliente é obrigado a suportar as duas: `Content-Type: application/json`
    (uma resposta só, fecha ali) **ou** `Content-Type: text/event-stream`
    (abre um SSE que eventualmente carrega a resposta, e pode mandar
    notificações do servidor antes dela).
  - Um `GET` nesse mesmo endpoint é **opcional** para o servidor: ele "MUST
    either return `text/event-stream` ... or else return `405 Method Not
    Allowed`".
  - Sessão: o servidor **pode** devolver `Mcp-Session-Id` no `initialize`;
    se devolver, o cliente é obrigado a ecoar esse cabeçalho em todo pedido
    seguinte, e um `Mcp-Session-Id` desconhecido vira `404`.
  - Todo pedido HTTP subsequente carrega `MCP-Protocol-Version: <versão
    negociada>`.
  - Segurança exigida pela própria spec: **validar o cabeçalho `Origin`**
    (contra *DNS rebinding*), **preferir `127.0.0.1` a `0.0.0.0`** quando
    local, e **autenticar** toda conexão.
- **Batching JSON-RPC** (lote como `[ {...}, {...} ]`) existiu na revisão
  2025-03-26 e foi **removido** na 2025-06-18 — "no compelling use case for
  batching" (fonte: [PR #416](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/416),
  confirmado no [changelog oficial](https://modelcontextprotocol.io/specification/2025-06-18/changelog)).
  A mudança **não é compatível para trás**.
- **N8n**, o alvo de integração citado na tarefa: o nó **MCP Client Tool**
  fala o transporte **Streamable HTTP** (recomendado) ou SSE (legado), e
  aceita autenticação **None, Bearer, Header genérico, Multi-header, ou
  OAuth2** (fonte: [docs.n8n.io/.../n8n-nodes-langchain.toolmcp](https://docs.n8n.io/integrations/builtin/cluster-nodes/sub-nodes/n8n-nodes-langchain.toolmcp)).
  Ou seja: **N8n não exige OAuth** — um cabeçalho `Authorization: Bearer
  <token>` estático já basta do lado do cliente.

### 1.2 O que já existe aqui, medido

O MCP **já está implementado** neste repositório — o pedido 6 da lista de
pendências está fechado — só que **por stdio**. A pergunta desta pesquisa é
sobre expor o mesmo servidor por HTTP, para um cliente de rede como N8n.

**A ponte MCP** — `crates/phxsql-server/src/mcp.rs` (735 linhas, medido com
`wc -l`):

| peça | onde |
|---|---|
| `struct Ponte<E: Executor>` | `mcp.rs:90` |
| `Ponte::atender(&self, linha: &str) -> Option<String>` | `mcp.rs:135` — recebe uma **linha de texto**, devolve uma **linha de texto** ou `None` (notificação). Não conhece soquete, `stdin` nem HTTP. |
| `tools_call` (traduz `tools/call` → `{"op": ...}` do PhxSql) | `mcp.rs:222` |
| `servir()` (o transporte, hoje só stdio) | `mcp.rs:368` |
| catálogo de ferramentas (fonte única com o `/help` do `phxsqlcmd` e o `tools/list`) | `crates/phxsql-server/src/catalogo.rs` |
| `ExecutorLocal` (o que chama `despachar`, os quatro portões) | `servidor.rs:20907` |
| flag de linha de comando `phxsqld --mcp [--usuario u] [--escrita]` | `crates/phxsql-server/src/main.rs:36`, `:121`, `:141` |

Contagem agora (`grep -c "ferramenta_mcp: true" catalogo.rs` /
`grep -c "ferramenta_mcp: false" catalogo.rs`): **13 operações** viram
ferramenta MCP hoje, de **137 operações** no catálogo inteiro — 8 são
somente-leitura e 2 escrevem (`phx_inserir`, `phx_atualizar`); com a ponte
padrão (`Ponte::nova`, somente leitura) só as 8 aparecem. Testes: **14** em
`mcp.rs` + **5** de processo em `crates/phxsql-server/tests/mcp_stdio.rs`
(`grep -c "#\[test\]"`) — **19** no total, contra os "quinze" que o
`docs/MCP.md` ainda cita: o número do documento envelheceu, e registro aqui
para quem revisar aquele arquivo.

**O webservice HTTP que já existe** — `crates/phxsql-server/src/http.rs`
(921 linhas) + as rotas em `servidor.rs`:

| rota | onde |
|---|---|
| roteador (`match (metodo, caminho)`) | `servidor.rs:6909` |
| `GET /`, `GET /saude`, `GET /idiomas`, **`POST /api`** | `servidor.rs:6910-7022`; o `/api` está em `servidor.rs:7011` |
| `fn api_http(...)` — "o mesmo protocolo da porta 5000, um pedido por vez" | `servidor.rs:7681` |
| sessão por cabeçalho (`X-Sessao`, não por conexão) | `servidor.rs:7684-7690`, struct `Sessoes` em `http.rs:434` |
| modelo de conexão: `TcpListener` + **thread por conexão**, `Connection: close` | `servidor.rs:1413` (bind), `http.rs:282` (`Connection: close`) |

**O JSON escrito à mão** — `crates/phxsql-core/src/json.rs` (992 linhas):
`Json::analisar`, `Json::escrever`/`escrever_identado`, `Json::objeto`,
`Json::texto_de`, e o detalhe que importa adiante — `campo(&self, nome)`
(json.rs:53-58) só enxerga `Json::Objeto`; para qualquer outra variante
(incluindo `Json::Lista`) devolve `None`.

### 1.3 O veredito: dá para servir MCP zero-deps reusando o HTTP + JSON que já existem?

**Sim, e a maior parte já está pronta.** Três fatos medidos sustentam isso:

1. `Ponte::atender` já é **agnóstica de transporte** (`mcp.rs:135`): ela
   recebe `&str`, devolve `Option<String>`. Trocar "linha do stdin" por
   "corpo do POST" não pede mudar uma vírgula dela.
2. O `/api` (`servidor.rs:7681`) **já resolve exatamente o perfil que a
   spec chama de "resposta única em `application/json`"**: um pedido HTTP,
   uma resposta HTTP, sessão carregada num cabeçalho (`X-Sessao` hoje,
   `Mcp-Session-Id` seria o equivalente MCP) em vez de amarrada à conexão
   TCP. É o desenho que a Streamable HTTP pede quando o servidor decide
   **nunca** abrir SSE — e a própria spec permite essa escolha por inteiro
   (ver 1.1: `POST` responde `application/json` sempre; `GET` responde
   `405` sempre).
3. Nenhuma crate nova entra: tudo que falta é uma rota a mais no `match` de
   `servidor.rs:6909` (`("POST", "/mcp")`) que leia o corpo como uma linha,
   chame `ponte.atender(&corpo)`, e escreva o resultado com
   `http::responder_json` — o mesmo padrão que `POST /api` já usa.

**Veredito: sim, dá — no perfil "resposta única", sem SSE.** É o suficiente
para N8n (o nó MCP Client Tool consome Streamable HTTP; a spec obriga o
*cliente* a suportar o modo `application/json`) e para qualquer cliente
Claude Code que fale HTTP em vez de subir processo.

### 1.4 Onde o desenho divergiria da spec, e a restrição nossa em cada uma

| # | divergência do desenho aqui | o que a spec pede/assume | restrição nossa que causa a divergência |
|---|---|---|---|
| 1 | HTTP em texto claro, sem TLS | Segurança de rede geralmente assumida por HTTPS; a spec não impõe TLS no texto do protocolo, mas trata a rede como hostil (por isso exige validar `Origin`) | **Zero dependências externas** — a `std` não fala TLS, e a casa já tomou essa mesma decisão para `/api` hoje e para o SMTP de `email.rs` ("serve para um relé que você controla, não para entregar direto"). Nada novo: mesma linha da cláusula "TLS na conexão" do `CLAUDE.md`. |
| 2 | Nunca abrir `text/event-stream`; `POST` sempre responde `application/json` de uma vez; `GET /mcp` responde `405` | A spec permite SSE para notificação/streaming do servidor, e o cliente é obrigado a aceitar os dois modos | **Um pedido por conexão, thread por conexão, `Connection: close`** (`servidor.rs:1413`, `http.rs:282`) — arquitetura decidida para não trazer um runtime assíncrono (a mesma recusa que fechou o item 106 da lista de pendências: o MULTILINK trazia `tokio` obrigatório mesmo sem feature nenhuma, e foi recusado). A spec **permite esse perfil por inteiro** (não é gambiarra, é o modo documentado), e como o protocolo interno do PhxSql já é pergunta-resposta — nunca envia notificação por iniciativa própria — não se perde nenhuma capacidade que já existisse. |
| 3 | Autenticação por **token/login+desafio do próprio PhxSql**, carimbado num cabeçalho (Bearer ou próprio), não por OAuth 2.1 | A revisão 2025-06-18 classifica servidor MCP remoto como *OAuth Resource Server*, com metadata de recurso protegido e *Resource Indicators* (RFC 8707) | **Zero dependências** (OAuth/JWT não é `std`) **+ "o portão continua sendo um"** (`mcp.rs:11-22`) — abrir um segundo sistema de autenticação paralelo ao login/token existente seria exatamente o "segundo caminho até o dado" que o próprio módulo já recusa por princípio. **Medido que isso não impede o alvo**: o nó MCP Client Tool do N8n aceita Bearer/Header genérico sem OAuth (fonte em 1.1) — o mesmo campo fixo que `Ponte::com_campo_fixo("token", ...)` já carimba (`mcp.rs:117-120`) cobre o caso. |
| 4 | `GET /mcp` sem SSE — servidor nunca inicia mensagem por conta própria | A spec permite o cliente abrir um `GET` para ouvir notificações do servidor | Mesma restrição do item 2: sem uma tarefa de fundo por sessão (que pediria ou threads paradas segurando conexão, ou um runtime assíncrono), não há como manter um `GET` de longa duração fora do modelo "aceita → lê um pedido → responde → fecha". |

### 1.5 Achado à parte — não é divergência por restrição, é lacuna medida

**Um cliente 2025-03-26 que mande lote trava a ponte, hoje, mesmo por
stdio.** `mcp.rs:56` (`VERSOES_ACEITAS`) inclui `"2025-03-26"` e
`"2024-11-05"` — revisões em que o **lote JSON-RPC** (`[ {...}, {...} ]`)
era válido (ver 1.1). Mas `atender` (`mcp.rs:135`) faz
`Json::analisar(linha)` e em seguida `pedido.campo("id")`; `campo()`
(`json.rs:53-58`) só enxerga `Json::Objeto` e devolve `None` para
`Json::Lista`. O resultado: um pedido em lote cai no `let id = id?;`
(`mcp.rs:154`) como se fosse **notificação**, e a ponte devolve `None` —
**nenhuma resposta sai**, e um cliente que negociou 2025-03-26 e mandou um
lote fica esperando para sempre. Não é uma escolha registrada em lugar
nenhum: é uma revisão anunciada como aceita cujo recurso a ponte não fala.
Fica registrado com arquivo e linha; não é código de produção mudar isso
aqui.

### 1.6 A fronteira: leitura (fase 1) × escrita guardada (fase 2)

**Fase 1 — já é o padrão hoje.** `Ponte::nova` (`mcp.rs:101`) nasce
somente-leitura; liberar escrita é `com_escrita(true)` (`mcp.rs:111`), uma
decisão com nome. Sobre HTTP a mesma regra vale sem mudar nada: a rota
`POST /mcp` nasceria com uma `Ponte` somente-leitura por padrão, do mesmo
jeito que `phxsqld --mcp` sem `--escrita` hoje.

**Fase 2 — o gancho já existe, só não está exigido para esta ponte.** As
duas ferramentas de escrita hoje são `phx_inserir` e `phx_atualizar`
(`excluir` tem `ferramenta_mcp: false`, `catalogo.rs:863` — a régua
primordial de integridade nem chega perto do MCP). `phx_atualizar` já
carrega um parâmetro **opcional** `"versao"` — "a versão lida; quem manda
ganha a recusa por escrita concorrente" (`catalogo.rs:832-836`) — que é
exatamente o mecanismo da janela de conflito de escrita (pedido 123,
`CLAUDE.md`), hoje **pedido, não imposto**, para não quebrar cliente antigo.

Do outro lado da ponte MCP não existe "cliente antigo" — ela nasceu depois
da guarda. A proposta medida para a fase 2 (não construída aqui): quando
`com_escrita(true)` estiver ligado **na ponte MCP especificamente**, tornar
`"versao"` **obrigatório** nas chamadas de escrita — reaproveitando a mesma
recusa que já existe para argumento obrigatório faltando
(`tools_call`, `mcp.rs:249-259`, "falta o argumento ...") — em vez de mudar
a regra geral do protocolo (que continua opcional para a porta 5000 e para
o `/api`, por causa exatamente da mesma regra de não quebrar cliente
antigo). Isso dá ao modelo de linguagem uma escrita sempre lida-antes-de-
gravar, sem tocar no comportamento que os outros clientes já têm.

---

## Parte 2 — Aviso de problema por e-mail e por API REST

### 2.1 O que existe, medido

Hoje há **quatro pontos** que disparam e-mail de aviso — todos passando
pelo **mesmo** cliente SMTP (`crate::email::enviar`,
`crates/phxsql-server/src/email.rs`, 392 linhas, sem TLS por decisão
documentada no próprio arquivo), cada um com a própria condição de
ligar/desligar e a própria janela de silêncio:

| # | o quê | onde dispara | condição de ligar | janela de silêncio |
|---|---|---|---|---|
| 1 | violação grave de segurança (IP bloqueado) | `avisar_violacao_por_email`, `servidor.rs:1670` | `alertas.email.ligado && alertas.email.avisar_seguranca` | `alertas.repetir_horas`, por IP (`avisos_de_seguranca`) |
| 2 | espaço em disco apertado | `ligar_vigia_de_disco`/`conferir_disco`, `servidor.rs:19342`/`19386` | `alertas.ligado` (liga o vigia) **e** `alertas.email.ligado` (manda e-mail; sem e-mail, só grava no painel) | `alertas.repetir_horas`, por caminho (`avisados`) |
| 3 | falha de job agendado | `avisar_sobre_a_corrida`, `servidor.rs:6539` | `alertas.email.ligado && alertas.email.avisar_jobs` | `alertas.repetir_horas`, por job (`avisos_de_jobs`) |
| 4 | promoção/degradação do cluster | `avisos_do_cluster`, `servidor.rs:3113` | `estado.config.email.ligado` — **config PRÓPRIA do cluster**, não `alertas.email` | `cluster.avisar_cada_min`, medido por `ultimo_email_ms` (`cluster.rs:197`) |

**Achado estrutural**: o destino do e-mail está em **dois** blocos de
configuração independentes, não um só — `Alertas.email`
(`config.rs:700`/`716`, tipo `Email` de `config.rs:834`) e `Cluster.email`
(`config.rs:358`/`376`, mesmo tipo `Email`). Um servidor pode ter
`alertas.email` configurado e `cluster.email` vazio (ou o inverso): não há
"o e-mail do PhxSql", há dois, cada um do tamanho do bloco que o motivou.

**Nenhum dos quatro sai por REST hoje.** `grep -rn "webhook" crates/` não
acha nada — zero ocorrências. O único precedente de "evento consultável
pela API" é `op:"acessos"` (`catalogo.rs:1661`, alimentado por
`fn anotar`, `servidor.rs:1765`) e `op:"jobs"` com `"historico":true`
(`catalogo.rs:1980-1991`, histórico só **de jobs**, nada de disco,
segurança ou cluster). A promoção do cluster, aliás, já é registrada em
`acessos` (`servidor.rs:3096-3103`, `op: "cluster_promocao"`) **além** do
e-mail — mas os outros três avisos (disco, job, segurança) não deixam
rastro **estruturado** nenhum além do `eprintln!` (efêmero, some com o
contêiner) e do próprio e-mail (que sai da casa e não volta).

### 2.2 O que falta para "aviso de problema sai por e-mail E por `/avisos` REST"

1. **Não existe rota nem `op` de leitura para os avisos.** Nem `/avisos`
   HTTP, nem `op:"avisos"` no protocolo de porta 5000 — o único jeito de
   saber que um alerta disparou hoje é ter visto o `stderr` no instante, ou
   ter recebido o e-mail.
2. **Não existe um tipo/estrutura comum para "aviso".** Cada um dos quatro
   pontos monta `assunto`/`corpo` como `String` solta, inline, na função
   que o dispara — não há um `struct Aviso { quando_ms, categoria, texto,
   enviado_por_email }` nem equivalente.
3. **Não existe histórico persistido para três dos quatro** — disco, job e
   segurança guardam só a marca de "já avisei, não repita" (os mapas
   `avisados`/`avisos_de_jobs`/`avisos_de_seguranca`), que serve para a
   janela de silêncio e **não** para responder "quais avisos dispararam
   esta semana". Job tem histórico (`jobs.log`), mas por **corrida**, não
   por aviso — teria de cruzar corrida-falhou com o que de fato foi
   avisado.
4. **O padrão para persistir já existe na casa** (não seria um mecanismo
   novo): `acesso.rs` — `struct Acesso` + `fn anotar` — já é exatamente
   "acrescenta um evento a um log append-only e deixa a op `acessos`
   reler". Estender esse MESMO padrão (ou abrir uma segunda lista igual,
   dedicada a avisos) é o caminho medido de menor distância — nenhuma peça
   nova, só o mesmo desenho aplicado aos quatro pontos que hoje só
   `eprintln!`.
5. **A configuração duplicada (`alertas.email` × `cluster.email`) precisa
   de uma decisão antes de existir UM `/avisos`**: a rota unificada
   responde por avisos de **duas** fontes de configuração distintas, ou o
   DBA decide fundir os dois blocos (mudança de formato — "entra cedo",
   pela própria regra do projeto).
6. **Nenhuma checagem própria de permissão foi pensada para `/avisos`.**
   Pelo padrão já usado em `jobs`/`acessos`, a leitura exigiria
   `administrar` — e precisaria da mesma disciplina que a cláusula do
   "portão único" já cobra: **um** só lugar decide quem lê avisos, não um
   por categoria.

Nada disso foi implementado nesta rodada — é o levantamento do que a
próxima rodada de código precisaria decidir antes de escrever a primeira
linha, na mesma régua da Parte 1.
