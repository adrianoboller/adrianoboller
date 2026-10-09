# Live Share = terminal compartilhado (proposta, NÃO implementada)

Gap `live_share` do VS Code. Decisão do dono, 09/10/2026 (item 6): **terminal compartilhado —
um anfitrião, N convidados, cursor do anfitrião; edição simultânea fica declarada como limite.**
Este documento levanta o que existe e desenha o mínimo. Não há código novo.

## 1. O que já existe (`crates/phxclaw-agent/src/ide.rs`, `crates/phxclaw-terminal`)

| Peça | Onde | Serve ao compartilhamento? |
|---|---|---|
| `GET /v1/ide/terminal` (websocket) | `ide.rs` `terminal`/`sessao` | É a sessão do anfitrião. Mensagens `auth`, `abrir`, `tecla`, `colar`, `texto`, `redimensionar`, `rolar`, `fechar`; devolve `grade` por diferença |
| Token na PRIMEIRA mensagem, prazo de 10 s | `PRAZO_DO_TOKEN` | Padrão a reaproveitar (navegador não põe cabeçalho em websocket; token fora da URL e dos logs) |
| Só o Helix abre (`programa != "helix"` recusa) | `abrir` | Mantém: o convidado nunca recebe um bash |
| Sandbox obrigatório (`processo::terminal_no_bwrap`) | `abrir` | Mantém: projeto em `/work`, pasta do agente mascarada; sem bwrap, recusa |
| Uma sessão por usuário (`vivas()`, chave = SHA-256 do token) | `ide.rs` | **Atrapalha**: conexão nova do mesmo token fecha a anterior. O convidado precisa de chave própria, senão derrubaria o anfitrião |
| `Terminal::abrir(.., ao_mudar)` com UM `ao_ouvinte` | `phxclaw-terminal` | A saída vai a um só `mpsc`; compartilhar exige **fan-out** (um laço que reenvia cada `Atualizacao` a N destinatários) |
| `Terminal::grade()` (retrato completo) | `phxclaw-terminal` | É o que o convidado que entra no meio recebe antes das diferenças |
| `Terminal::{tecla,colar,escrever,redimensionar,rolar}` | `phxclaw-terminal` | Entradas que o convidado **não** poderá chamar |
| Auth por Bearer ÚNICO (`api::auth` compara com `s.token`) | `api.rs` | Não há identidade por usuário. O convidado **não pode** usar o token da API |
| Tunel remoto de terminal (`/v1/tunel/terminal`, pedido-resposta) | `tunel.rs` | Outro mecanismo (linha a linha pela ponte); fora do escopo |
| `PHXCLAW_API_TOKEN` injetado no ambiente do `hx` (completação por IA) | `abrir` | **Risco direto** (ver §4, R1) |

Lacuna medida no código: não há sessão com mais de um espectador, nem token de sessão, nem
expiração de sessão; a única forma de «ver o terminal do outro» hoje é ter o Bearer da API.

## 2. Desenho mínimo

Um anfitrião, N convidados, **somente leitura**, token por sessão, expiração.

**Anfitrião** (autenticado pelo Bearer da API, como hoje) ganha uma mensagem nova no mesmo
websocket: `{"op":"compartilhar","expira_em_s":3600,"max_convidados":4}`. A resposta é
`{"ev":"compartilhado","sessao":"<uuid v7>","token":"<segredo>","expira_em":"<RFC3339>"}`. O
token é mostrado **uma vez** e não é gravado em disco. `{"op":"parar_compartilhar"}`, o `fechar`
do terminal e a queda do websocket encerram a sessão e derrubam todos os convidados.

**Convidado** conecta em `GET /v1/ide/compartilhado` (websocket), manda
`{"op":"entrar","sessao":"…","token":"…"}` na primeira mensagem (mesmo prazo de 10 s) e recebe
`{"ev":"grade",…}` — primeiro o retrato (`Terminal::grade()`), depois as diferenças. **Qualquer
outra mensagem do convidado é ignorada e conta** (§3): o convidado não tem `tecla`, `colar`,
`texto`, `redimensionar` nem `rolar`. «Cursor do anfitrião» é de graça: a grade já traz o cursor
do `hx` do anfitrião; o convidado vê o que o anfitrião vê, no tamanho do anfitrião.

**Estado** (em memória, como `vivas()`): `HashMap<sessao, Compartilho { digest_do_token,
expira_em, convidados: Vec<Sender>, max }>`. O token é guardado como **SHA-256** (a base já tem
`sha2`) e comparado em tempo constante. Um laço de fan-out por sessão lê o `Atualizacao` do
terminal e o reenvia a cada convidado; convidado lento (canal cheio) é desconectado, nunca
segura o anfitrião.

**Reuso, sem segundo motor** (lei «função não se duplica»): a abertura do `hx`, o bwrap, o
`Pedido` do anfitrião e a serialização da grade ficam onde estão; o convidado só **consome** o
que `sessao()` já produz.

## 3. Controles de segurança exigidos

1. **Somente leitura por construção**, não por filtro: a rota do convidado nem tem o
   `match Pedido` do anfitrião; o enum dela só aceita `entrar`/`sair`.
2. **Token por sessão**, 32 bytes aleatórios (`getrandom`, já no Cargo.lock), comparado por
   digest; **nunca** o Bearer da API, nunca na URL, nunca em log nem no ledger (entra no
   ledger só `sessao_id`, quando abriu/fechou e quantos convidados).
3. **Expiração dura**: padrão 1 h, teto 8 h; checada a cada tick (o laço de 250 ms já existe) e
   também na entrada; sessão expirada derruba os convidados e esquece o token.
4. **Teto de convidados** (padrão 4) e **teto de tentativas** de `entrar` com token errado por
   IP/minuto (a API já tem o balde de fichas `Limite`, reaproveitar).
5. **Anfitrião manda**: ele vê quantos convidados há e pode revogar um (`revogar`) ou todos.
6. **Opt-in**: `ide.compartilhar` desligado por padrão (chave no catálogo do `config.json`,
   pelo `phxclaw-config-runtime`), porque compartilhar a tela é decisão do operador.
7. **Prova de que o convidado não escreve**: teste que manda `tecla`/`colar`/`texto` pelo
   websocket do convidado e confere, lendo a grade do anfitrião, que nada mudou — e que falha
   se a rota passar a aceitar entrada (RED medido).

## 4. Riscos

| # | Risco | Mitigação |
|---|---|---|
| R1 | **O `hx` do anfitrião tem `PHXCLAW_API_TOKEN` no ambiente** (`abrir` o injeta para a completação por IA). Se o anfitrião rodar `:sh echo $PHXCLAW_API_TOKEN`, a grade mostra o segredo **a todos os convidados** | Antes de compartilhar, o terminal compartilhado abre o `hx` **sem** esse token (a completação por IA fica indisponível enquanto compartilha) ou com um token de escopo só `completar`. Decidir e provar com teste; é pré-requisito |
| R2 | A grade é o que está na tela: segredos exibidos pelo anfitrião (arquivo `.env`, saída de comando) vão aos convidados | Aviso explícito ao compartilhar + indicador permanente «compartilhado com N» no terminal do anfitrião. Redigir por varredura da grade com o motor `phxclaw_types::segredo` é possível (analisar, não recortar), mas é defesa parcial: o aviso é o controle principal |
| R3 | Token vazado (print, chat) dá acesso à tela até expirar | Expiração curta por padrão, revogação, token mostrado uma vez, teto de convidados; sem token longevo |
| R4 | Força bruta do token | 32 bytes aleatórios (inviável) + balde de tentativas + prazo de 10 s para a primeira mensagem |
| R5 | Convidado lento ou malicioso retém memória do anfitrião | Canal limitado por convidado; estourou, desconecta |
| R6 | Compartilhar pela rede aberta: a API fala HTTP/WS sem TLS | Mesmo limite da API: só atrás de TLS/ponte (`INSTALACAO_DOCKER_K8S.md`); o anfitrião recusa compartilhar se `api.host` não for loopback e não houver terminação TLS declarada |
| R7 | Dois usuários reais, uma identidade: o Bearer da API é único, não há conta por pessoa; a auditoria diz «um convidado entrou», não «quem» | Aceito e declarado: o convidado é identificado por um rótulo que o anfitrião dá ao criar o convite |
| R8 | A chave por usuário de `vivas()` (hash do token) faria o convidado derrubar o anfitrião se reaproveitasse a rota | Rota e mapa próprios do convidado; teste de que entrar não fecha a sessão do anfitrião |
| R9 | No contêiner endurecido o terminal do IDE nem abre (sem bwrap) | O recurso herda o limite do IDE web; documentado em `INSTALACAO_DOCKER_K8S.md` |

## 5. Limites declarados (ordem do dono)

- **Sem edição simultânea** (CRDT/OT): um só cursor, o do anfitrião. O convidado não digita.
- Sem voz, sem chat, sem servidor de linguagem compartilhado, sem compartilhar o *servidor*
  (portas) do projeto.
- Um terminal por sessão (o Helix do anfitrião); sem multiplexar vários.

## 6. Estimativa e ordem de entrega

Pequena, porque a máquina de grade, sandbox e auth de primeira mensagem já existem: (1) fan-out
no `phxclaw-terminal`/`ide.rs`, (2) estado de compartilhamento com digest e expiração, (3) rota
do convidado só de leitura, (4) opt-in no catálogo de configuração, (5) botão e indicador na
tela do IDE (texto pela fábrica de idiomas). Pré-requisito: resolver **R1** primeiro.
Nada disto toca o formato de nenhum arquivo em disco: o estado é só memória.

**Estado do gap:** continua `nao` até existir código e teste; esta proposta só reduz o risco do
desenho.
