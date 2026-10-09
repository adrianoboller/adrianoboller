# Estudo do material do dono — 68 projetos Rust e 15 alternativas ao n8n

Papel J, 09/10/2026. Ordem do dono do mesmo dia: «Material a ser estudado e integrado». Material:
`projetos_rust_para_phoenix_2026-10-09.md` (68 repositórios, 8 áreas, P0/P1/P2), o mesmo em
`catalogo_rust_phoenix_2026-10-09.json` (os 68 ids batem 1 a 1 com o `.md`, conferido por script) e
`alternativas_n8n_github_2026-10-09.md` (15 alternativas + Flowise). O material mira o **Phoenix**
(projeto do dono: núcleo Rust, Studio React/Tauri, PostgreSQL, UUIDv7, PHX JSON/Phoenix IR, linguagem
própria, cena/timeline/mídia); este documento separa o que é do **PhxClaw** do que é do Phoenix.

Nada foi compilado (disco ~1,7 GB livre, portões do integrador rodando). **Medido**: `Cargo.lock` e
`Cargo.toml` da árvore de `phxclaw/v070-nativo` (commit `211b9cd6`, 849 pacotes); `LICENSE` e data do
`HEAD` de 16 repositórios por clone raso e esparso (48 MB, apagados ao fim); documentação oficial
lida nas páginas citadas. **Raciocinado**: o custo em pacotes de cada crate nova (método em §2) e
tudo o que diz «raciocinado».

## 0. Decisão, em uma tela

| Parte | Resultado medido |
|---|---|
| 68 projetos Rust | **11 já em uso** · **9 úteis ao PhxClaw** (6 com dependência nova, 3 só arquitetura) · **23 do Phoenix** · **25 recusados com número** |
| 15 alternativas ao n8n (+ Flowise) | **10 ids novos**: **4 ampliam a fonte `n8n`** (estão na doc do próprio n8n e a lista de 59 não os tinha) + **6 abrem a fonte nova `prefect`** (execução durável/orquestração). Temporal, Trigger.dev, Kestra, Windmill, Node-RED, Activepieces, Huginn, Dify e Langflow ficam como **corroboração**; IronFlow, Automatisch, Sim e Flowise **recusados** |
| Efeito na conta | n8n **76,3% → 71,4%** no agente (45/59 → 45/63); `prefect` nasce em **68,2%** (15/22); soma das fontes **92,4% → 89,9%** (304/329 → 319/355); somada à proposta de 09/10 (fontes 8–12), **80,1% → 78,8%** (330/412 → 345/438) |

**As três decisões:**

1. **A SP000035 se confirma: expressão por caminho JSON, sem linguagem embutida.** Rhai custa só 9
   pacotes, mas o que os fluxos pedem de cálculo os quatro motores maduros resolvem com **nós
   declarativos** (n8n: 6 nós especializados documentados; Node-RED: change/sort/split/batch;
   Activepieces: 5 ajudantes; Dify: `list_operator`) — vira o id `transformacao_declarativa`. Código
   arbitrário já tem nó: `execucao_codigo` (shell/python no bwrap) está `agente`. Uma segunda
   linguagem seria a segunda sandbox e a segunda política. Rune (18), Boa (45), rquickjs (motor C),
   deno_core (56 + V8) e mlua (Lua em C) recusados.
2. **WASM soma, não substitui o bwrap — e só quando um plugin precisar rodar fora do Linux.** O bwrap
   isola processo (shell, git, `python_project`, plugins); WASM não roda bash nem git. Trocar
   deixaria dois sandboxes (fere «um sandbox só», `git.rs:5-8`). Custo medido: wasmtime **37**
   pacotes novos (só `runtime+cranelift`), **18** com o interpretador `pulley`, **68** no padrão;
   extism **113** (recusado: segundo manifesto ao lado do `PluginManifest`). O portão é o id
   `sandbox_multiplataforma` (reserva 13, zeroclaw): quando ele abrir, a escolha é wasmtime contra o
   isolamento nativo do Windows, com a bancada.
3. **Cada motor que o material sugere para o «PhoenixClaw» já existe no PhxClaw com uma decisão
   medida** — gateway de modelos (`phxclaw-llm`, 4 provedores), MCP (`mcp.rs`, 981 linhas), grafo
   (`phxclaw-task-graph`), BM25 (`bm25.rs`), política (`regras.rs` + capacidades), validador
   (`esquema.rs`), git no bwrap. rust-genai (12), rig (9), tantivy (42), petgraph (2), cedar (35),
   jsonschema (19), gitoxide (82), nats (10 + servidor) recusados por «função e comando vêm do mesmo
   motor». **Exceção que entra:** `rmcp` como **dependência de teste** (6 pacotes, 0 no binário),
   oráculo de conformidade do `mcp.rs`; substituir o `mcp.rs` se decide pelo resultado desses testes
   na onda dos ids `elicitacao_mcp`/`roots_mcp`. Recomendação ao Phoenix (§5): consumir o PhxClaw
   (API, MCP, A2A) em vez de remontar esses motores — **isso é escopo de produto e sobe ao dono** (§7).

## 1. Hipóteses, escritas antes de medir

Escritas depois de ler o material e antes de abrir o `Cargo.lock`, o índice do crates.io e as docs.

| H | Hipótese | Rival | Régua |
|---|---|---|---|
| H1 | A maioria dos 68 é dependência nova útil ao PhxClaw | a maioria já existe aqui ou é do Phoenix | contagem por classe no `Cargo.lock` |
| H2 | wasmtime **substitui** o bwrap nos plugins | **soma**, e só fora do Linux | o que o bwrap isola hoje + pacotes |
| H3 | Rhai (ou JS) entra nas expressões dos fluxos | **nós declarativos** de transformação | o que n8n/Node-RED/Activepieces/Dify documentam + custo |
| H4 | `rmcp` substitui o `mcp.rs` agora | `rmcp` como oráculo de conformidade (teste) | pacotes + o que falta no `mcp.rs` |
| H5 | petgraph no lugar do grafo da casa | o `phxclaw-task-graph` fica | o que cada um faz |
| H6 | Cedar como motor do RBAC do n8n (`projetos_rbac`, decisão 2 do dono) | tabela papel → capacidades no portão que existe | pacotes + número de políticas |
| H7 | as alternativas ampliam a fonte `n8n` | fonte nova de execução durável · ou só arquitetura | onde cada capacidade está documentada |
| H8 | Temporal é a fonte da execução durável | Prefect | quantos dos candidatos cada um documenta |
| H9 | a proveniência por item do NiFi vira id | é o `pairedItem` recusado com outro nome | decisão SP000035 + ledger |
| H10 | recuperar disparos perdidos (catch-up/backfill) vira id | já existe política fixa | `agenda.rs` |

### Veredito, com o número

- **H1 morreu.** 11 em uso, 9 úteis, 23 do Phoenix, 25 recusados. Só **6** pediriam crate nova, e
  duas delas são só de teste.
- **H2 morreu; a rival vive condicionada.** O `phxclaw-sandbox` (506 linhas) só conhece bwrap
  (`BackendUnavailable` fora dele); o mesmo bwrap roda shell, git e plugins. Mesmo o z8run, que é o
  modelo de plugin WASM do material, roda módulo **sem nenhum import de host** («no filesystem,
  network or clock access», `crates/z8run-runtime/src/sandbox.rs:3-11`): computação pura, com fuel,
  época e `StoreLimits`. Isso não substitui um plugin que precisa de processo.
- **H3 morreu.** A doc do n8n lista seis nós de transformação estrutural (Aggregate, Limit, Remove
  Duplicates, Sort, Split Out, Summarize) como o caminho sem código; o Node-RED tem `15-change`,
  `18-sort`, `17-split`, `19-batch`; o Activepieces tem `data-mapper`, `text-helper`, `math-helper`,
  `date-helper`, `data-summarizer` no núcleo; o Dify tem `list_operator`. Quatro motores convergindo
  no **meio** sem linguagem. O custo da Rhai (+9) não é o problema: é a segunda política.
  **Reabre** só se um fluxo real pedir um cálculo que nem os nós declarativos nem o passo de código
  no bwrap resolvam — raciocinado; hoje não há fluxo de usuário gravado para contar.
- **H4 morreu como substituição imediata.** `rmcp` 3.5.1 com cliente, servidor, stdio, processo
  filho e Streamable HTTP: **6** pacotes novos. Barato; mas o `mcp.rs` funciona e tem decisões nossas
  (servidor declarado só pelo operador, capacidade `mcp.<servidor>`, mesmo `Agent::call_tool`). O
  que falta nele são ids (`elicitacao_mcp`, `roots_mcp`, `nao` desde 09/10). Entra como teste agora.
- **H5 morreu.** petgraph custaria só 2 pacotes, mas responderia «qual a ordem / há ciclo» ao lado
  do `phxclaw-task-graph` (710 linhas: ordem topológica, ciclo, `RetryPolicy`, aprovação, diário em
  Postgres). Dois motores para a mesma pergunta.
- **H6 morreu.** Cedar: 35 pacotes (gerador `lalrpop` incluso) e uma segunda linguagem de política
  ao lado da `regras.rs` e das capacidades. Os papéis do n8n são fixos (dono/admin/membro e papéis de
  projeto): tabela, não linguagem. Reabre se o dono pedir controle por **atributo** (ABAC).
- **H7 morreu nos dois extremos.** «Só arquitetura» morreu: 10 capacidades com doc oficial.
  «Ampliar o n8n com tudo» morreu: 6 das 10 **não estão** na doc do n8n, e pô-las ali seria mentir
  a origem. Vivem as duas metades: 4 ampliam o `n8n`, 6 abrem fonte nova.
- **H8 morreu.** Dos 7 candidatos de execução durável (sobreposição de agenda, idempotência do
  disparo, recuperação de disparo perdido, concorrência por chave, limite de taxa, cache de passo,
  desfazer), Temporal documenta **3**; Prefect documenta **6** (todos menos a recuperação). Temporal
  fica como corroboração de dois ids.
- **H9 morreu.** A proveniência do NiFi (`user-guide.adoc:2894`, «Replaying a FlowFile» `:2981`)
  é a ligação item → origem; a SP000035 já recusou o `pairedItem` porque cada chamada pelo portão
  deixa evidência própria no ledger. Reabre pela mesma porta: um `juntar` por campo que peça.
  A contrapressão (`:1370`, padrão 10.000 objetos / 1 GB) vira aceite do `fila_workers` (onda 4).
- **H10 morreu.** `agenda.rs:127-128`: «Disparo atrasado (processo parado) dispara UMA vez, nao uma
  por janela perdida» — é o `LAST` do Kestra, escrito e testado
  (`dispara_uma_vez_mesmo_atrasado_e_persiste`). Kestra recupera `ALL` por padrão; Temporal tem janela
  de 1 ano e backfill. Divergência registrada; vira id se um operador pedir backfill.

## 2. Como se mediu

- **Já em uso**: nome no `Cargo.lock` + quem depende (dependência reversa lida do próprio lockfile) +
  quantos `Cargo.toml` declaram direto.
- **Custo de crate nova** (raciocinado, não `cargo`): pacotes alcançados pelo índice esparso do
  crates.io (`index.crates.io`; deps normais e de build; opcionais só se uma feature ativa as liga;
  alvo Linux; versão máxima compatível), menos os nomes que já estão no `Cargo.lock`. **Validado**
  contra o que já está no lock: `axum` 0 novos de 47, `reqwest` 0 de 71, `tauri` 1 de 267. O tamanho
  do binário **não foi medido**; quem decide é `cargo build --release` + `ls -l` na bancada.
- **Licença e atividade**: `LICENSE` lido na raiz por clone `--depth 1 --filter=blob:none --sparse`;
  `HEAD` = data do último commit do ramo padrão em 09/10/2026.

## 3. Os 68, um por um

Coluna **novos** = pacotes que entrariam no `Cargo.lock` (849 hoje). «—» = não é crate (produto,
ferramenta ou referência).

| id | projeto | P | classe | o que é aqui / o número |
|---|---|---|---|---|
| R001 | tokio | P0 | **já em uso** | 1.53.1; 15 `Cargo.toml` diretos |
| R002 | axum | P0 | **já em uso** | 0.8.9; 4 diretos (`phxclaw`, `agent`, `api-gateway`, `sdk`) |
| R003 | serde | P0 | **já em uso** | 1.0.229; 108 diretos |
| R004 | serde_json | P0 | **já em uso** | 1.0.151; 115 diretos |
| R005 | uuid | P0 | **já em uso** | 1.26.1 com `v7`; 101 diretos; 149 chamadas `now_v7`/`new_v7` |
| R006 | jsonschema | P0 | recusar | +19 (com ou sem as features padrão); `esquema.rs` próprio cobre as 7 palavras medidas (`tests/guardas.rs`) e nomeia o que não conferiu |
| R007 | petgraph | P0 | recusar | +2; `phxclaw-task-graph` já responde ordem e ciclo (H5) |
| R008 | sqlx | P0 | Phoenix | +11 (só `postgres`); aqui `postgres` 0.19.14 síncrono em 10 crates — um segundo driver seria dois motores |
| R009 | sqlparser | P1 | Phoenix | +7; o `PostgresTool` separa leitura por `BEGIN READ ONLY` (`sistema.rs:1336-1339`): quem recusa a escrita é o banco, não um analisador |
| R010 | wasmtime | P0 | **útil, condicional** | +37 (`runtime+cranelift`), +18 (`pulley`), +43 (com component model), +68 (padrão). Capacidade: plugin isolado fora do Linux. Portão: `sandbox_multiplataforma` (H2) |
| R011 | wit-bindgen | P0 | Phoenix | 0.57.1 já aparece no lock, mas **só** via `wasip2 ← getrandom` (alvo `wasm32-wasip2`); nenhum uso |
| R012 | wasm-tools | P0 | Phoenix | `wasmparser` +1 se o wasmtime entrar |
| R013 | extism | P1 | recusar | +113; segundo manifesto/registro ao lado do `PluginManifest` assinado |
| R014 | rhai | P0 | recusar (PhxClaw) | +9; H3. No Phoenix é candidato da camada de regras |
| R015 | rune | P1 | recusar | +18; H3 |
| R016 | boa | P1 | recusar | +45; JS nas expressões já recusado (`TECNOLOGIAS.md` §5) |
| R017 | deno | P1 | recusar | `deno_core` +56, mais V8 |
| R018 | rquickjs | P1 | recusar | +4, mas o motor é QuickJS-NG em C; H3 |
| R019 | pest | P1 | Phoenix | +2 (+5 com `pest_derive`); gramática própria do Phoenix |
| R020 | chumsky | P2 | Phoenix | +6; GitHub arquivado (02/04/2026), segue no Codeberg |
| R021 | rowan | P1 | Phoenix | +3; árvore de edição do Studio |
| R022 | oxc | P1 | Phoenix | `oxc_parser` +28; aqui o código se analisa por `tree-sitter` 0.27.0 (8 gramáticas) |
| R023 | swc | P1 | Phoenix | `swc_core` é fachada (+3 sem features); conversão de código |
| R024 | RustPython | P2 | recusar | `rustpython-vm` +91; o `python_project` roda o Python real no bwrap |
| R025 | ruff | P1 | **já em uso** | como binário: `python.rs:1,22,35` (pytest, ruff, mypy no venv) |
| R026 | c2rust | P1 | Phoenix | migração C → Rust; a regra `c2rust_transpiler.md` do skyfireitdiy/Jarvis já foi indicada como skill (09/10) |
| R027 | tauri | P0 | **já em uso** | 2.12.0 em `apps/phxclaw-desktop/src-tauri` |
| R028 | wry | P1 | **já em uso** | 0.57.0, transitivo por `tauri-runtime-wry` |
| R029 | egui | P1 | Phoenix | +25; a tela do PhxClaw é web sem framework |
| R030 | iced | P2 | Phoenix | +89 |
| R031 | slint | P2 | Phoenix | +162; licença royalty-free proíbe expor a API do Slint (FAQ) — sobe ao dono **se** o Phoenix quiser |
| R032 | floem | P2 | Phoenix | referência de reatividade |
| R033 | dioxus | P2 | Phoenix | +50 |
| R034 | wgpu | P1 | Phoenix | +20 |
| R035 | vello | P1 | Phoenix | +35 |
| R036 | bevy | P1 | Phoenix | +225 |
| R037 | ruffle | P0 | Phoenix | referência Flash/timeline; não publicado como crate |
| R038 | notify | P0 | recusar | +4; a pasta observada varre a cada 5 s (`main.rs` do app, laço do `Observador`) e exige arquivo estável — duas leituras de qualquer jeito; o próprio material manda «reconciliar pelo estado» |
| R039 | enigo | P1 | **já em uso** | 0.6.1 em `phxclaw-system-automation` |
| R040 | xcap | P1 | **já em uso** | 0.9.8 em `phxclaw-system-automation` |
| R041 | Symphonia | P1 | Phoenix | +18; MPL-2.0 (cópia fraca, por arquivo) |
| R042 | rust-genai | P0 | recusar | +12; `phxclaw-llm` (Anthropic, Gemini, Ollama, OpenAI; 2.067 linhas) |
| R043 | rig | P0 | recusar | +9; já «arquitetura» em `novas-fontes-2026-10.md` §3.1 |
| R044 | rmcp | P0 | **útil (teste)** | +6 com todas as features de transporte; dev-dependency, 0 no binário; H4. Licença em transição MIT → Apache-2.0 (as duas permissivas; o PhxClaw é Apache-2.0) |
| R045 | candle | P1 | **útil, condicional** | `candle-core` +47; onda C6 de 09/10 (VM do dono, custo por uso) |
| R046 | burn | P2 | **útil, condicional** | +45; treino só na VM (decisão 8 do dono), nunca no binário |
| R047 | mistral.rs | P1 | recusar | +211; segundo motor de modelo ao lado do Ollama |
| R048 | kalosm | P2 | recusar | +42; o backend Fusor «não pronto para produção» (README) |
| R049 | ort | P1 | recusar | já recusado em 09/10: série `2.0.0-rc`, baixa o ONNX Runtime C++ no build; a voz usa o sherpa-onnx por processo |
| R050 | ocrs | P2 | recusar | +18; OCR aqui é o tesseract, «um motor so» (`visao.rs:81`); ocrs só latim e «early preview» |
| R051 | whisper-rs | P2 | recusar | arquivado (30/07/2025); C++ |
| R052 | tantivy | P1 | recusar | +42; `bm25.rs` próprio, e a fusão com embedding perdeu 3/8 contra 6/8 (`documentos.rs:13-16`) |
| R053 | zeroclaw | P1 | **útil (arquitetura)** | 0 pacotes; reserva 13 de 09/10 (6 ids) |
| R054 | windmill | P1 | **útil (arquitetura)** | 0 pacotes; AGPL no backend — só leitura. A especificação OpenFlow é Apache-2.0 (`LICENSE`) e documenta `concurrency_key`, `cache_ttl`, `priority` (`openflow.openapi.yaml:69-112`) |
| R055 | z8run | P2 | **útil (arquitetura)** | 0 pacotes; desenho do sandbox WASM (fuel + época + `StoreLimits`, sem import de host) e portas tipadas, para a onda 5 |
| R056 | ironflow | P2 | recusar | 0 ids novos (§4.2); fluxo em Lua (`mlua` +6, Lua 5.4 em C vendorizado) seria a segunda forma de definir fluxo |
| R057 | datafusion | P2 | Phoenix | +94 |
| R058 | opendal | P1 | Phoenix | +12; o PhxClaw grava em disco local |
| R059 | nats | P1 | recusar | +10 e um servidor para operar; o outbox em Postgres existe (`PostgresOutbox.claim_batch`) e a fila é onda 4 com bancada |
| R060 | cedar | P1 | recusar | +35; H6 |
| R061 | gitoxide | P1 | recusar | `gix` +82; o git roda no MESMO bwrap do shell e o modelo nunca monta a linha (`git.rs:1-12`); `gix` em processo sairia do sandbox |
| R062 | tracing | P0 | **útil, condicional** | 0.1.44 já está na árvore (transitivo por axum/h2/hyper), sem uso direto; `tracing-subscriber` +6, `tracing-opentelemetry` +7, `opentelemetry-otlp` +7. Entra com o id `telemetria_otel` (goose, `nao`) |
| R063 | clap | P0 | recusar | +10; a CLI tem o `ajuda.rs` (711 linhas) como fonte única que o `gerar_guia_operador.py` lê |
| R064 | ratatui | P1 | Phoenix | +21; o terminal do PhxClaw é o `alacritty_terminal` do IDE |
| R065 | loom | P1 | recusar | +9 (dev); pede trocar as primitivas por `cfg(loom)` e não modela o tokio |
| R066 | proptest | P1 | **útil (teste)** | +6 (dev); propriedades do JSON canônico da assinatura de fluxo, do caminho `{{a.b[0]}}` e do `esquema.rs` |
| R067 | nextest | P1 | recusar | ferramenta; raciocinado: o gargalo desta máquina é disco, não tempo de teste, e o `tools/suite.sh` grava a saída que o dossiê lê |
| R068 | cargo-deny | P1 | **já em uso** | nos dois fluxos de CI (`phxclaw-release-qualification.yml:44`, `-candidate.yml:56`) e no `qualify_release.py:162` — **sem `deny.toml`** (achado §8) |

## 4. As 15 alternativas ao n8n

### 4.1 Tabela

| Projeto | Licença (lida no `LICENSE`) | HEAD | Traz para o motor | Decisão |
|---|---|---|---|---|
| Prefect | Apache-2.0 | 2026-10-07 | 6 dos 7 candidatos de execução durável (§4.2) | **fonte nova `prefect`** |
| Temporal | MIT | 2026-10-09 | sobreposição de agenda (6 políticas, padrão `Skip`), política de id do fluxo, catch-up de 1 ano e backfill | corroboração (H8: 3 de 7) |
| Trigger.dev | Apache-2.0 | 2026-10-08 | `idempotencyKey` (escopos `run`/`attempt`/`global`, TTL 30 dias), `concurrencyKey` por inquilino | corroboração |
| Kestra | Apache-2.0 | 2026-10-08 | `concurrency.behavior` QUEUE/CANCEL/FAIL; `recoverMissedSchedules` ALL/NONE/LAST; backfill | corroboração; H10 |
| Node-RED | Apache-2.0 | 2026-10-08 | nós change/sort/split/batch/range; `delay` limita taxa; contexto persistente (`localfilesystem.js`) | corroboração |
| Activepieces | MIT fora de `packages/ee` e `server/api/src/app/ee` | 2026-10-09 | ajudantes de dados, `tables`, `store`; 736 peças da comunidade | corroboração |
| Huginn | MIT | 2026-10-04 | `DeDuplicationAgent` (`property`, `lookback`) | corroboração |
| Dify | Apache-2.0 **modificada** (multi-inquilino e logo exigem licença comercial) | 2026-10-09 | `parameter_extractor`, `list_operator`, `iteration` | **só arquitetura** |
| Langflow | MIT | 2026-10-06 | componente de saída estruturada, fluxo como servidor MCP (`Agents/mcp-server.mdx`), tipos (`Develop/data-types.mdx`) | corroboração |
| Windmill | AGPL-3.0 + proprietário; OpenFlow e clientes Apache-2.0 | 2026-10-09 | `concurrency_key` + janela, `cache_ttl`, prioridade; 1.425 pacotes no lock deles | **só arquitetura** (R054) |
| NiFi | Apache-2.0 | 2026-10-08 | proveniência + replay por item; contrapressão | H9 morreu; contrapressão → aceite do `fila_workers` |
| z8run | MIT ou Apache-2.0 | 2026-09-19 (`v0.3.0-rc.1`) | portas tipadas; WASM com fuel/época/`StoreLimits` (wasmtime 48); 491 pacotes | arquitetura (onda 5 e H2) |
| IronFlow | MIT | 2026-09-28 | fluxo em Lua; nada que os 59 + 10 não cubram | recusar (0 ids) |
| Automatisch | AGPL-3.0 + `.ee.` comercial | **2026-01-15** | nada além do n8n; 9 meses sem commit | recusar |
| Sim | Apache-2.0 | 2026-10-08 | nada além do n8n + fontes 8–12 | recusar (0 ids) |
| Flowise | Apache-2.0 fora de `enterprise/` | 2026-08-13 | **arquivado em 13/08/2026**, fim de ciclo 31/08 (discussão #6727) | recusar |

O material lista 15 + Flowise; a ordem do orquestrador nomeou 14 e não citou Sim nem Prefect. Os 16
foram medidos.

### 4.2 Lista FECHADA dos 10 ids novos

Nenhum colide com os 191 de hoje nem com os 56 propostos em 09/10 (conferido por script).

**Ampliação da fonte `n8n` (59 → 63)** — estão na doc do próprio n8n:

| id | o que é | onde (doc oficial) | corroboração | estado hoje | divergência nossa e a restrição |
|---|---|---|---|---|---|
| `transformacao_declarativa` | nós do motor que ordenam, limitam, agregam, resumem, desdobram e definem campos dos itens | docs.n8n.io/data/transforming-data («Aggregate, Limit, Remove Duplicates, Sort, Split Out, Summarize») | Node-RED `15-change`, `18-sort`, `17-split`, `19-batch`; Activepieces `data-mapper`, `*-helper`; Dify `list_operator` | parcial | campo por **caminho JSON**, nunca expressão avaliada (sem segunda sandbox); são nós do motor como `se`/`juntar`, sem portão de ferramenta porque não tocam nada fora dos itens |
| `deduplicacao_entre_execucoes` | descarta item já visto em execução anterior, por valor novo, maior ou data posterior | `n8n-nodes-base.removeduplicates` («Remove Items Processed in Previous Executions»; escopo nó/fluxo; histórico padrão 10.000) | Huginn `de_duplication_agent.rb` | nao | o histórico guarda o **sha256** do valor, não o valor: dado pessoal e segredo não moram no histórico (`redacao_dados_execucao`) |
| `tabelas_dados` | tabela embutida que o fluxo lê e grava entre execuções (marcador de execução, tabela de consulta) | docs.n8n.io/data/data-tables (200 MiB padrão; a doc do static data a recomenda no lugar dele) | Activepieces `tables`, `store`; Node-RED contexto persistente | parcial | passa pelo portão com capacidade própria e teto; segredo recusado na gravação, como já acontece nas `variaveis` |
| `extracao_estruturada` | passo de modelo cuja saída obedece a um esquema; classificação = esquema com `enum` que vira porta por categoria | `information-extractor` (3 modos de esquema) e `text-classifier` («Other» opcional, várias classes) | Dify `parameter_extractor`; Langflow `Components/parser.mdx` | parcial | o esquema se confere pelo **mesmo** `esquema.rs` dos argumentos de ferramenta (um validador só); saída fora do esquema é erro do passo e cai no `ao_errar`, nunca passa calada |

**Fonte nova `prefect`** — Prefect 3, Apache-2.0, doc lida no clone (`docs/v3`, `HEAD` `bc5fb57`):

| id | o que é | onde (doc oficial) | corroboração | estado hoje | divergência nossa e a restrição |
|---|---|---|---|---|---|
| `politica_sobreposicao` | o que fazer quando o disparo chega com a execução anterior ainda rodando: enfileirar, pular, cancelar o novo | `concepts/deployments.mdx:337-340` (`collision_strategy` ENQUEUE/CANCEL_NEW) | Temporal Schedule «Overlap Policy»; Kestra `concurrency.behavior` | parcial | o padrão continua o de hoje (o excedente espera a vaga global, `fluxos.rs:2002`): guarda nova entra **pedida**, não imposta (o Temporal impõe `Skip`) |
| `chave_idempotencia_disparo` | o mesmo disparo repetido devolve a execução que já existe | `advanced/generate-custom-sdk.mdx:85,95` (`idempotency_key`) | Trigger.dev `idempotency.mdx` (TTL 30 dias); Temporal «Workflow Id Conflict Policy» `Use Existing` | nao | devolve a execução existente (o `Use Existing` do Temporal), e o descarte é **o mesmo** da caixa do canal de webhook, que já descarta `id` repetido (`canais/webhook.rs:7`) — não uma segunda deduplicação (achado §8) |
| `concorrencia_por_chave` | vagas por chave calculada da entrada (inquilino, cliente) | `concepts/global-concurrency-limits.mdx`, `tag-based-concurrency-limits.mdx` | Trigger.dev `concurrencyKey`/`perKey`; Windmill OpenFlow `concurrency_key` | nao | a chave é caminho JSON sobre a entrada; o semáforo é o mesmo do `concorrencia_limite`, partido por chave |
| `limite_taxa` | quantos disparos ou chamadas por segundo, independente da duração | `global-concurrency-limits.mdx:18-28`, `:82-86` (`rate_limit`, `slot_decay_per_second`) | Node-RED `89-delay` («limits the rate at which they can pass») | nao | por passo e por `por_item`; a espera conta dentro do `teto_ms` (um limite de taxa não pode estourar o teto sem dizer) |
| `cache_resultado_passo` | passo com a mesma definição e a mesma entrada devolve o resultado guardado, dentro de um prazo | `concepts/caching.mdx` (políticas DEFAULT/INPUTS/TASK_SOURCE; `cache_expiration`) | Windmill OpenFlow `cache_ttl` | parcial (`pin` é manual; o reaproveitamento só vale na retomada da mesma execução) | **só** passo cuja capacidade não escreve (a lista única `CAPACIDADES_QUE_ESCREVEM` do `checkpoint.rs`): efeito externo nunca sai do cache; o acerto vai ao ledger como `reaproveitado` |
| `compensacao_rollback` | quando o fluxo falha, desfazer o que os passos anteriores fizeram | `advanced/transactions.mdx:7`, `:47`, `:81-110` (`on_rollback`, `on_commit`, ciclo de vida) | — (Temporal: só padrão de amostra, não recurso) | parcial | para arquivo, o desfazer **é** o `checkpoint.rs` (ponto antes de cada escrita, nascido no portão); para efeito fora da pasta, um passo `ao_desfazer` declarado, que roda pelo `Agent::call_tool` — nunca um gancho de código livre |

Ids que a Prefect **também** tem (para a conta por fonte sair certa), lidos nos títulos de
`docs/v3/concepts` e `advanced`: `workflows`, `subfluxo`, `tentativas_por_no`, `timeout_execucao`,
`retomar_execucao`, `cron`, `gatilhos_evento`, `espera_wait`, `aprovacao_humana`,
`variaveis_globais`, `credenciais_cifradas`, `concorrencia_limite`, `fila_workers`,
`historico_execucoes`, `api_servidor`, `execucao_codigo`.

Onda sugerida dentro da SP000035: **4** (escala e gestão) recebe `politica_sobreposicao`,
`chave_idempotencia_disparo`, `concorrencia_por_chave`, `limite_taxa`; uma **4b** (dados) recebe
`transformacao_declarativa`, `deduplicacao_entre_execucoes`, `extracao_estruturada`,
`cache_resultado_passo`, `compensacao_rollback`; `tabelas_dados` é item de produto (decisão 2 do dono
já o põe no escopo). Para a **onda 5** (editor), duas entradas de arquitetura sem id: portas
tipadas (z8run, Langflow `data-types.mdx`) e a validação de conexão na leitura; o editor continua
SVG próprio (xyflow recusado, R20).

### 4.3 Bloco para o `fontes.json` (NÃO editado nesta rodada)

Acrescentar ao fim de `"n8n".ids`:

```json
["transformacao_declarativa", "deduplicacao_entre_execucoes", "tabelas_dados", "extracao_estruturada"]
```

Fonte nova:

```json
{
 "prefect": {
  "repositorio": "https://github.com/PrefectHQ/prefect",
  "doc": "https://docs.prefect.io/v3",
  "lido_em": "09/10/2026",
  "licenca": "Apache-2.0. Python: so arquitetura e documentacao. Lido no clone: prefect bc5fb57 (docs/v3/concepts e docs/v3/advanced). Corroborado por Temporal (MIT), Trigger.dev, Kestra, Node-RED (Apache-2.0) e pela especificacao OpenFlow do Windmill (Apache-2.0)",
  "ids": [
   "workflows", "subfluxo", "tentativas_por_no", "timeout_execucao", "retomar_execucao", "cron",
   "gatilhos_evento", "espera_wait", "aprovacao_humana", "variaveis_globais", "credenciais_cifradas",
   "concorrencia_limite", "fila_workers", "historico_execucoes", "api_servidor", "execucao_codigo",
   "politica_sobreposicao", "chave_idempotencia_disparo", "concorrencia_por_chave", "limite_taxa",
   "cache_resultado_passo", "compensacao_rollback"
  ]
 }
}
```

E o bloco do `phxclaw.json` — vai **junto**, senão o `gerar_absorcao.py` para em id sem estado:

```json
{
 "transformacao_declarativa": {"estado": "parcial", "evidencia": "fluxos.rs: `se` filtra itens por porta, `juntar` e `lote` existem; sem ordenar, limitar, agregar, resumir, desdobrar nem definir campos (enum Tipo, fluxos.rs:583)"},
 "deduplicacao_entre_execucoes": {"estado": "nao", "evidencia": "nenhum historico de itens entre execucoes em fluxos.rs"},
 "tabelas_dados": {"estado": "parcial", "evidencia": "banco_postgres (sistema.rs:1336) grava num Postgres do operador; sem tabela embutida nem no de tabela no fluxo; variaveis sao so leitura"},
 "extracao_estruturada": {"estado": "parcial", "evidencia": "passo `tarefa` cujo texto e JSON vira itens (fluxos.rs Saida); sem esquema imposto a saida nem porta por categoria; esquema.rs valida so argumento de ferramenta"},
 "politica_sobreposicao": {"estado": "parcial", "evidencia": "concorrencia_limite faz o excedente esperar a vaga global (fluxos.rs:2002) = ENQUEUE; agenda.rs/api.rs disparar_agenda nao olha se o last_task ainda roda; sem pular/cancelar por agenda"},
 "chave_idempotencia_disparo": {"estado": "nao", "evidencia": "gatilhos.rs:770-784 confere assinatura e janela de 300 s, sem id: a mesma requisicao reenviada dentro da janela dispara de novo; o canal de webhook descarta id repetido (canais/webhook.rs:7)"},
 "concorrencia_por_chave": {"estado": "nao", "evidencia": "limite so por instancia (fluxos.rs:2002)"},
 "limite_taxa": {"estado": "nao", "evidencia": "nenhum limite por segundo em fluxos.rs nem nos gatilhos"},
 "cache_resultado_passo": {"estado": "parcial", "evidencia": "Passo.pin manual e saidas reaproveitadas so na retomada da mesma execucao (retomar); sem cache por definicao+entrada com prazo"},
 "compensacao_rollback": {"estado": "parcial", "evidencia": "checkpoint.rs cria ponto antes de cada escrita pelo portao e restaurar e ferramenta; fluxo que falha nao desfaz os passos anteriores nem roda passo de desfazer"}
}
```

## 5. O que serve ao Phoenix (projeto do dono) — recomendação para quando o repositório abrir

Nada disto entra no PhxClaw. É o que a medição de hoje ensina ao Phoenix.

1. **Não remontar o que o PhxClaw já é.** O material propõe para o «PhoenixClaw» rust-genai/rig,
   rmcp, tantivy, petgraph, nats, cedar. O PhxClaw já tem gateway de modelos, MCP nos dois sentidos,
   grafo com retomada, BM25, política e fluxo (`fluxos.rs`, 3.770 linhas, n8n em 76,3%). O caminho
   que respeita «função do mesmo motor» entre os dois projetos é o Phoenix **falar com o PhxClaw**
   (API `/v1`, MCP, e o A2A da onda C5) em vez de embutir um segundo gateway. **Sobe ao dono** (§7).
2. **Núcleo e contratos.** tokio, serde, serde_json e uuid v7 são o que o PhxClaw já roda (149
   chamadas v7). Para o PHX JSON de plugins de **terceiros**, o Phoenix precisa do JSON Schema
   inteiro (`$ref`, `oneOf`, `format`): aí o `jsonschema` (+19) faz sentido — o subconjunto do
   `esquema.rs` serve ao PhxClaw porque os esquemas são da casa. Driver: um só; o PhxClaw usa
   `postgres` síncrono, o `sqlx` (+11) é a escolha assíncrona — se o Phoenix for consumir crates do
   PhxClaw como biblioteca, a escolha tem de ser a mesma nos dois.
3. **Plugins.** wasmtime atrás de interface própria; mínimo medido +37 pacotes (`runtime+cranelift`),
   +18 com `pulley` (interpretador, sem JIT — útil onde JIT é proibido). Referência de desenho: o
   `sandbox.rs` do z8run (fuel por chamada, prazo por época, `StoreLimits`, zero import de host).
   Extism (+113) recusado pelo mesmo motivo daqui: segundo manifesto. wit-bindgen/wasm-tools seguem.
4. **Linguagem.** Rhai (+9) é o candidato barato para regras e eventos; Pest (+2/+5) para a gramática
   própria; Rowan (+3) para a árvore de edição. A lição dos fluxos daqui vale lá: quatro motores
   maduros preferem **nó declarativo** a linguagem para transformar dados — a linguagem fica para
   comportamento, não para mapear campo.
5. **Studio e mídia.** Tauri: o PhxClaw já tem `phxclaw-desktop` em Tauri 2.12.0 (experiência
   reaproveitável). wgpu (+20), vello (+35), bevy (+225), Symphonia (MPL-2.0, +18), Ruffle
   (referência). **Slint**: a licença royalty-free proíbe expor a API do Slint — num construtor de
   aplicações isso é decisão de licença do dono antes de qualquer protótipo.
6. **Dados e operação.** datafusion (+94), opendal (+12), sqlparser (+7), ratatui (+21), c2rust.
   Windmill e Automatisch são AGPL: só leitura de arquitetura; a especificação OpenFlow do Windmill
   é Apache-2.0 e pode ser lida como referência de formato de fluxo.

## 6. Hipóteses que morreram, com o número

| Hipótese | O número que a matou |
|---|---|
| H1 — a maioria dos 68 é dependência nova útil | 11 em uso, 23 do Phoenix, 25 recusados; só 6 pediriam crate (2 de teste) |
| H2 — wasmtime substitui o bwrap | o bwrap isola shell, git, Python e plugins; WASM não roda nenhum dos três primeiros; +37 a +68 pacotes para ganhar só fora do Linux |
| H3 — Rhai/JS nas expressões | n8n (6 nós), Node-RED (4), Activepieces (5), Dify (1) resolvem com nó declarativo; código já tem nó no bwrap |
| H4 — rmcp substitui o mcp.rs já | o que falta são 2 ids de protocolo, não um motor; entra como teste (+6, 0 no binário) |
| H5 — petgraph | +2, mas responderia a pergunta do `phxclaw-task-graph` (710 linhas) |
| H6 — Cedar para o RBAC | +35 e uma segunda linguagem de política para papéis fixos |
| H7 — só arquitetura / tudo no n8n | 10 ids com doc; 6 deles fora da doc do n8n |
| H8 — Temporal como fonte | 3 de 7 contra 6 de 7 da Prefect |
| H9 — proveniência do NiFi como id | é o `pairedItem` recusado na SP000035; o ledger já liga chamada → evidência |
| H10 — recuperar disparo perdido como id | `agenda.rs:127-128` já decide «uma vez» (o `LAST` do Kestra), com teste |
| extism | +113 e um segundo manifesto de plugin |
| mistral.rs | +211; segundo motor de modelo ao lado do Ollama |
| gitoxide | +82; git em processo sairia do bwrap |
| notify | +4 para tirar até 5 s de latência de um gatilho que exige arquivo estável |
| IronFlow, Sim | 0 ids novos |
| Automatisch | 9 meses sem commit, AGPL, nada novo |
| Flowise | arquivado em 13/08/2026 |

## 7. O que sobe ao dono — e só isto

1. **Escopo de produto (os dois projetos):** o Phoenix consome o PhxClaw como motor de agente e
   fluxo (API/MCP/A2A), ou monta o seu? A pesquisa recomenda consumir (§5.1); a decisão é de produto.
2. **Escopo da versão:** os 10 ids levam o n8n de 76,3% para 71,4% e a soma de 92,4% para 89,9%.
   Sugestão pela regra de escopo congelado: nascem ⏸ («depois da versão»), **menos**
   `chave_idempotencia_disparo`, que fecha uma garantia que não vale hoje (§8, primeiro achado).
3. **Licença (só do Phoenix, só se ele quiser Slint):** termos royalty-free do Slint.

Não sobe: o comportamento que as fontes documentam (decidido acima), as recusas com número, e a
entrada do `rmcp` e do `proptest` como dependências de teste — essas pedem a decisão de crate nova
do integrador, pela regra da casa, com o número de §3.

## 8. Achados colaterais

- **Réplica do gatilho de webhook dentro de 300 s dispara de novo.** `gatilhos.rs:770-784` confere
  assinatura e carimbo, sem id nem memória do que já viu; o irmão `canais/webhook.rs:7` descarta `id`
  repetido pela caixa. Lido no código, **não provado por soquete**: a prova é reenviar a mesma
  requisição assinada duas vezes e contar dois fluxos. O conserto vem pelo mesmo motor da caixa (lei
  «conserto entra no caminho que o motivou, e o caminho irmão fica»).
- **`cargo deny check` sem `deny.toml`.** Os dois fluxos de CI instalam o cargo-deny e o
  `qualify_release.py:162` o roda; não há `deny.toml` na árvore. A doc diz «Licenses not in this
  list are denied by default» — sem lista, toda licença cai. Raciocinado, **não rodado**: o integrador
  confere com `cargo deny check licenses` numa máquina com o binário.
- **`wit-bindgen` 0.57.1 está no `Cargo.lock` sem nenhum uso**: chega por `wasip2 ← getrandom`, alvo
  `wasm32-wasip2`. Quem contar «WASM já no lock» erra.
- **`tracing` 0.1.44 compila hoje** (axum, h2, hyper) e ninguém o usa direto: o meio do `telemetria_otel`
  custa +6 (`tracing-subscriber`), não uma árvore nova.

## 9. Fontes consultadas (todas em 09/10/2026)

Material do dono: os três arquivos em `scratchpad/estudo-phoenix/`, lidos inteiros.

Repositórios (clone `--depth 1 --filter=blob:none --sparse`; `LICENSE`, `HEAD`; apagados ao fim):
https://github.com/activepieces/activepieces · https://github.com/node-red/node-red ·
https://github.com/windmill-labs/windmill · https://github.com/automatisch/automatisch ·
https://github.com/kestra-io/kestra · https://github.com/apache/nifi · https://github.com/z8run/z8run ·
https://github.com/skitsanos/ironflow · https://github.com/langgenius/dify ·
https://github.com/langflow-ai/langflow · https://github.com/simstudioai/sim ·
https://github.com/FlowiseAI/Flowise · https://github.com/temporalio/temporal ·
https://github.com/PrefectHQ/prefect · https://github.com/triggerdotdev/trigger.dev ·
https://github.com/huginn/huginn

Arquivos lidos nos clones: Prefect `docs/v3/concepts/{deployments,caching,global-concurrency-limits,rate-limits}.mdx`,
`docs/v3/advanced/{transactions,generate-custom-sdk}.mdx`; NiFi `nifi-docs/src/main/asciidoc/user-guide.adoc`;
Node-RED `packages/node_modules/@node-red/nodes/locales/en-US/function/{89-delay,rbe}.html` e
`@node-red/runtime/lib/nodes/context/`; Windmill `LICENSE`, `openflow.openapi.yaml`; Huginn
`app/models/agents/de_duplication_agent.rb`; z8run `README.md`, `Cargo.toml`,
`crates/z8run-runtime/src/sandbox.rs`; IronFlow `Cargo.toml`; Dify e Activepieces árvore de nós;
Langflow árvore de `docs/docs`; Trigger.dev árvore de `docs`.

Documentação oficial:
https://docs.temporal.io/schedule ·
https://docs.temporal.io/workflow-execution/workflowid-runid ·
https://trigger.dev/docs/idempotency · https://trigger.dev/docs/queue-concurrency ·
https://kestra.io/docs/workflow-components/concurrency ·
https://kestra.io/docs/workflow-components/triggers/schedule-trigger ·
https://docs.prefect.io/v3/concepts/caching ·
https://docs.n8n.io/data/transforming-data/ ·
https://docs.n8n.io/integrations/builtin/core-nodes/n8n-nodes-base.removeduplicates/ ·
https://docs.n8n.io/data/data-tables/ ·
https://docs.n8n.io/code/cookbook/builtin/get-workflow-static-data/ ·
https://docs.n8n.io/integrations/builtin/cluster-nodes/root-nodes/n8n-nodes-langchain.information-extractor/ ·
https://docs.n8n.io/integrations/builtin/cluster-nodes/root-nodes/n8n-nodes-langchain.text-classifier/ ·
https://github.com/FlowiseAI/Flowise/discussions/6727 ·
https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html ·
https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/LICENSE ·
https://raw.githubusercontent.com/temporalio/temporal/main/LICENSE ·
índice esparso https://index.crates.io (dependências de 59 crates raiz e da árvore de cada uma).

Código nosso lido (árvore de `phxclaw/v070-nativo`, commit `211b9cd6`): `Cargo.lock`, `Cargo.toml`,
`apps/phxclaw-desktop/src-tauri/Cargo.toml`, `apps/phxclaw/src/{main,ajuda}.rs`,
`crates/phxclaw-agent/src/{plugins,mcp,regras,fluxos,gatilhos,git,agenda,api,esquema,sistema,checkpoint,python,visao,nuvem}.rs`,
`crates/phxclaw-agent/src/canais/webhook.rs`, `crates/phxclaw-sandbox/src/lib.rs`,
`crates/phxclaw-types/src/lib.rs`, `crates/phxclaw-task-graph/src/lib.rs`,
`crates/phxclaw-memory-context/src/bm25.rs`, `crates/phxclaw-terminal/src/lib.rs`,
`docs/absorcao/{fontes,phxclaw}.json`, `docs/absorcao/{SPRINTS.md,gerar_absorcao.py}`,
`docs/propostas/novas-fontes-2026-10.md`, `docs/TECNOLOGIAS.md`,
`docs/diretivas/DECISOES_DO_DONO_20261009.md`, `tools/qualify_release.py`, `.github/workflows/`.
