# Novas fontes para o PhxClaw — orquestração, cognição própria e rede neural local

Papel J, 09/10/2026. Ordem do dono do mesmo dia: «pesquisar novos projetos e listar quais podem ser
absorvidos para o PhxClaw para a criação de um super sistema de controle de agentes e subagentes […]
muito importante a integração com redes neurais e cognição própria». Decisão do pesquisador pela
pétrea «o pesquisador decide; o dono é o impasse». Nada foi compilado (disco em 1,9 GB livres,
compartilhado): o que está **medido** foi medido no fonte, no `LICENSE`, no `git log` e na API de
arquivos do Hugging Face; o que está **raciocinado** diz que é raciocinado.

**Restrições do dono que entraram como régua (09/10/2026, via orquestrador):** (1) rede neural local
= **só CPU**, modelos de até algumas centenas de MB (classificador de intenção/risco, embeddings);
LoRA de LLM fora; treino pesado só na VM de nuvem que ele vai fornecer, custo por uso. (2)
Auto-evolução: o agente **propõe** com teste e medição e espera o Go (integrador + dono); nunca
aplica sozinho. (3) Itens de produto do n8n entram no escopo (RBAC, segredos externos, galeria,
Docker/Kubernetes). (4) Cloudbot = Clawdbot = OpenClaw: não é fonte nova. E o **material de estudo
do dono** (`jarvis_openclaw_github_2026-10-09.md`, 18 repositórios, licenças «declaradas, não
validadas») entrou como origem; cada item foi conferido na fonte primária (§3.4).

## 0. Decisão, em uma tela

| # | Fonte (chave) | Família | Licença (medida no `LICENSE`) | Capacidades novas | Já existentes que ela também tem | Decisão |
|---|---|---|---|---:|---:|---|
| 8 | `a2a` | orquestração entre agentes | Apache-2.0 | **13** | 1 | absorver como fonte |
| 9 | `letta` | memória / cognição | Apache-2.0 | **10** | 6 | absorver como fonte |
| 10 | `goose` | agente em Rust (contexto, segurança) | Apache-2.0 | **13** | 18 | absorver como fonte |
| 11 | `soar` | arquitetura cognitiva | BSD-2-Clause | **7** | 1 | absorver como fonte (só arquitetura e fórmula) |
| 12 | `semantic_router` | roteamento neural por intenção | MIT | **5** | 1 | absorver como fonte (só arquitetura; o meio é nosso) |
| 1+ | `openclaw` ampliada | clawhub, mcporter, nó Windows, ansible | MIT | **+8** | — | ampliar a fonte 1, não abrir fonte nova |
| 13 | `zeroclaw` (reserva) | agente em Rust | MIT ou Apache-2.0 | 6 | 15 | reserva: entra quando a fila abrir; o `recibo_ferramenta` entra já, como arquitetura, na onda C1 |

**Total: 48 capacidades novas nas cinco fontes + 8 na ampliação da OpenClaw = 56 ids novos**
(nenhum colide com os 191 de hoje; conferido por script contra `phxclaw.json`). Efeito medido na
conta do gerador, se tudo entrar: **88,1% → 76,7%** no agente (290/329 → 316/412). O 88,1% é o da
árvore às 05:42 UTC de 09/10, com o `phxclaw.json` modificado e não comitado por outra frente (o
último commit dizia 87,8%, 289/329). A OpenClaw ampliada cai de **89,7% para 74,5%** (35/39 → 35/47).

## 1. Hipóteses, escritas antes de medir

| H | Hipótese | Rival | Como se decide |
|---|---|---|---|
| H1 | A maior lacuna de orquestração é **outro orquestrador** maduro (LangGraph, CrewAI, AutoGen, OpenAI Agents SDK, ADK) | a lacuna é o **protocolo entre agentes** (A2A) e a **higiene do contexto** (compactação, saída grande, revisor) | capacidades novas por fonte, depois de tirar o que já existe |
| H2 | A cognição avança com **memória vetorial/grafo** (Mem0, Graphiti, Cognee) | avança com **ativação (recência×frequência), episódio rotulado e consolidação proposta** | medida da casa (BM25 × embedding) + o que as fontes primárias fazem por padrão |
| H3 | Já dá para **treinar uma rede local nas gravações** do agente (risco/intenção) | primeiro **rotular**; o primeiro modelo é linear e escrito à mão; embedding só multilíngue e pela bancada | contar gravações com modelo real no disco |
| H4 | As implementações em Rust do material do dono (zeroclaw, openclaw-rs, local-jarvis) são **fonte nova** | são **arquitetura** e só uma tem capacidade nova que pague fonte | capacidades novas + atividade (último commit) + licença |
| H5 | O ecossistema OpenClaw (clawhub, mcporter, nó Windows, ansible) é **fonte própria** | **amplia a fonte 1** | mesma organização, mesma licença, mesma chave de produto |
| H6 | Auto-modificação do agente (Darwin-Gödel Machine, ADAS) entra | entra só **proposta + A/B por faixas + Go** | custo publicado + decisão (2) do dono |

### Veredito, com o número

- **H1 morreu.** Depois de tirar o que já existe: OpenAI Agents SDK **3** novas (handoff, guardrail
  com tripwire, spans de tracing), ADK **2** (escopos de estado `app:`/`user:`/`temp:`, avaliação de
  trajetória — que o `avaliacao.rs` já cobre pela nota parcial), CrewAI **2** (treino com feedback
  humano, nota composta de memória), LangGraph **0** (o checkpoint e o interrupt entraram em 01/10,
  `TRIAGEM_PESQUISAS_2026-10-01.md`), AutoGen **0** e parado desde **06/04/2026** (manutenção; o
  sucessor é o Microsoft Agent Framework). Contra **13** do A2A e **13** do goose. A rival vive: o
  que falta é o fio entre agentes de donos diferentes e a disciplina do contexto longo.
- **H2 morreu.** Três números: (a) a medida da casa, `documentos.rs:13-16`: no acervo `docs/` (156
  arquivos, 8 consultas de gabarito), **BM25 6/8** no 1º lugar contra **3/8** da fusão com o
  all-minilm; (b) a fonte primária da Letta, a mais madura em memória de agente, diz que a MemFS
  «**has no semantic or vector index by default**» — arquivo, índice `MEMORY.md` e busca por
  arquivo; (c) o Graphiti continua exigindo Neo4j/FalkorDB (R24). Convergem com o nosso desenho. O
  que ninguém aqui tem é **ativação** (Soar `base-decay` 0,5, recência e frequência; `lib.rs:329-334`
  do `memory-context` só soma termo e confiança, e a recência só desempata em `:419`) e
  **consolidação** em segundo plano.
- **H3 morreu agora, e a rival vive com portão de dado.** Medido em 09/10/2026: **46** arquivos
  `gravacao.jsonl` no disco (`/tmp`, `/home/user`, `/root`), **92** respostas de modelo gravadas,
  **100%** com `"model":"roteiro"` — **0** de modelo real. A triagem de 01/10 já tinha medido **2 de
  1.556** tarefas com modelo real. Uma rede treinada nisto aprenderia o roteiro dos testes. O
  caminho é rotular primeiro (o desfecho do portão é rótulo de graça) e treinar depois (§5, onda C3).
- **H4 morreu em parte.** zeroclaw: **6** novas, último commit **09/10/2026**, MIT ou Apache-2.0 →
  reserva 13. openclaw-rs: crates «⚠️ Partial» no próprio README, último commit **02/07/2026** (3
  meses) → só arquitetura. local-jarvis: alfa (`v0.1.0`) → só arquitetura, mas é o atalho para
  fechar o `voz_wake` parcial (microfone ao vivo + VAD).
- **H5 morreu.** Os quatro repositórios são da organização `openclaw`, MIT, e respondem ao mesmo
  produto da chave `openclaw`; abrir fonte nova partiria a absorção do mesmo produto em cinco
  números. Ampliam a fonte 1 em **8** ids (§4.3).
- **H6 morreu.** O próprio artigo da DGM estima **US$ 22.000 por corrida** no SWE-bench (~2 semanas,
  80 iterações; as linhas de base sem auto-melhoria, ~US$ 10.000). E a decisão (2) do dono proíbe
  aplicar sozinho. Vive o que já existe e é compatível: o `otimizacao.rs` gera variante, o
  `faixas_decidem` (`avaliacao.rs:89`) julga, e a promoção espera o Go.

## 2. Como se mediu

- **77 repositórios tentados, 75 medidos** por clone `--depth 1 --filter=blob:none --sparse` em
  scratch (maior ocupação: 100 MB; tudo apagado ao fim): `LICENSE` lido na raiz, SHA e data do
  `HEAD`, última tag por ordem de versão (`git ls-remote --sort=-v:refname`; em monorrepos a tag
  pode ser de um subpacote, e a tabela diz isso). Os dois que falharam: `microsoft/agentscope` (o
  repositório é `agentscope-ai/agentscope`) e `openai/openai-agents-rust` (não existe).
- **Estrelas**: a API do GitHub está fechada nesta sessão (403 do proxy, «sessions are bound to
  their configured repositories»); as cinco recomendadas foram lidas na página do repositório.
  Estrela é sinal fraco; a tabela ranqueia por **capacidade nova, licença e último commit**, que
  foram medidos.
- **Tamanho de modelo**: API de arquivos do Hugging Face (`/api/models/<id>?blobs=true`), em bytes.
- **Código nosso**: lido na árvore de trabalho de `phxclaw/v070-nativo`, citado por arquivo:linha.
- **Não medido aqui**: qualidade de embedding multilíngue no nosso acervo (precisa do modelo e da
  bancada; §5, onda C4), qualquer número de desempenho de terceiros (o +8,3% do ReasoningBank no
  WebArena é do blog do Google de 21/04/2026, não nosso).

## 3. Tabela ranqueada

Colunas: licença lida no `LICENSE` da raiz; `HEAD` = data do último commit do ramo padrão em
09/10/2026; «novas» = capacidades que nenhum dos 191 ids de hoje cobre. **Fonte** = entra no
`fontes.json`; **arquitetura** = lê-se e se redecide, nenhum código entra; **recusar** = com o motivo.

### 3.1 Orquestração de agentes e subagentes

| Candidato | Licença | HEAD | Tag | Novas | Custo contra o nosso gargalo | Decisão |
|---|---|---|---|---:|---|---|
| a2aproject/A2A (26,1 mil ★) | Apache-2.0 | 2026-10-07 | v1.0.1 (26/05/2026; 1.0.0 em 12/03/2026) | 13 | JSON-RPC (§9) e REST (§11) cabem no `axum` que já existe; gRPC (§10) pediria protobuf/tonic | **fonte 8** |
| aaif-goose/goose (ex-block/goose; 55,1 mil ★) | Apache-2.0 | 2026-10-09 | v1.54.0 | 13 | Rust, lê-se direto; 1.319 pacotes no `Cargo.lock` deles contra 849 nossos — nada se embute | **fonte 10** |
| zeroclaw-labs/zeroclaw | MIT ou Apache-2.0 | 2026-10-09 | v0.8.5 | 6 | Rust; 1.313 pacotes | **reserva 13** |
| langchain-ai/langgraph (42,9 mil ★) | MIT | 2026-10-08 | — | 0 | já absorvido em 01/10 (SP000029) | arquitetura (feito) |
| langchain-ai/deepagents | MIT | 2026-10-08 | — | 0 | planejar, subagente com contexto isolado, sistema de arquivos e aprovação humana já existem | recusar |
| openai/openai-agents-python | MIT | 2026-10-08 | v0.23.1 | 3 | handoff ≈ `team_delegate` com troca de controle; tripwire ≈ `guardrails` parcial | arquitetura |
| google/adk-python | Apache-2.0 | 2026-10-08 | v2.11.0 | 2 | escopo de estado; avaliação de trajetória já coberta | arquitetura |
| microsoft/agent-framework | MIT | 2026-10-08 | (tags por subpacote) | 1 | orquestração «Magentic»; sucessor de AutoGen + Semantic Kernel | arquitetura |
| crewAIInc/crewAI | MIT | 2026-10-08 | v1.10.0.1 | 2 | `crewai train` (feedback humano) e memória por nota composta (semântica + recência + importância) — a recência vem pela fonte 11 | arquitetura |
| microsoft/autogen | MIT (código) / CC (docs) | **2026-04-06** | v0.4.4 | 0 | em manutenção | recusar |
| ag2ai/ag2 | Apache-2.0 | 2026-10-08 | v1.1.2 | 0 | conversa em grupo = equipe | recusar |
| agno-agi/agno | Apache-2.0 | 2026-10-08 | v3.1.2 | 0 | recusado em 01/10 (modos de time) | recusar |
| mastra-ai/mastra | Apache-2.0 + `ee/` + `packages/connect` ELv2 | 2026-10-09 | — | 0 | absorvido em 01/10; partes `ee/`/ELv2 nunca | arquitetura |
| huggingface/smolagents | Apache-2.0 | 2026-10-06 | v1.26.0 | 1 | ação como código = `modo_codigo_ferramentas` (já pela fonte 10) | arquitetura |
| pydantic/pydantic-ai | MIT | 2026-10-08 | v2.54.0 | 0 | absorvido (SP000028) | — |
| microsoft/semantic-kernel | MIT | 2026-10-07 | — | 0 | substituído pelo Agent Framework | recusar |
| FoundationAgents/MetaGPT | MIT | **2026-01-21** | v0.8.2 | 0 | SOP por papel = equipe de 110 papéis; 9 meses sem commit | recusar |
| camel-ai/camel | Apache-2.0 | 2026-10-05 | v0.2.91a7 (alfa) | 0 | — | recusar |
| kyegomez/swarms | Apache-2.0 | 2026-10-08 | 6.8.1 | 0 | duplica equipe | recusar |
| All-Hands-AI/OpenHands | MIT | 2026-10-09 | v1.26.0 | 0 | o repositório hoje é a interface (`electron/`, `src/routes/condenser-settings.tsx`); o condensador = compactação, que vem pela fonte 10 em Rust | arquitetura |
| SWE-agent / mini-swe-agent | MIT | 2026-07-16 / 2026-09-03 | v1.1.0 / v2.4.6 | 0 | laço só-bash já existe | recusar |
| cline/cline | Apache-2.0 | 2026-10-08 | v4.1.23 | 0 | plan/act = `plan_mode` | recusar |
| RooCodeInc/Roo-Code | Apache-2.0 | **2026-05-15** | v3.54.0 | 0 | parado | recusar |
| Aider-AI/aider | Apache-2.0 | **2026-05-22** | v0.86.2 | 0 | repo-map = ADR-0063 | recusar |
| 0xPlaygrounds/rig | MIT | 2026-10-08 | v0.44.0 | 0 | abstração LLM/vetor = `phxclaw-llm` | arquitetura |
| ag-ui-protocol/ag-ui | MIT | 2026-10-08 | release/2026-10-08 | 0 | protocolo agente↔tela; temos `LIVE_EVENT_API` | arquitetura |
| agentscope-ai/agentscope | Apache-2.0 | 2026-10-09 | v2.0.9 | — | **não lido nesta rodada** (só licença e data) | pendente |

### 3.2 Cognição própria

| Candidato | Licença | HEAD | Tag | Novas | Custo / restrição | Decisão |
|---|---|---|---|---:|---|---|
| letta-ai/letta + letta-code (25,1 mil ★) | Apache-2.0 | 2026-09-10 / 2026-10-08 | 0.16.8 / v0.34.8 | 10 | arquivo + git, sem índice vetorial por padrão; memória compartilhada deles é só na nuvem | **fonte 9** |
| SoarGroup/Soar (444 ★) | BSD-2-Clause | 2026-10-07 | releases/9.6.5 | 7 | C++: só fórmula e desenho | **fonte 11** |
| google-research/reasoning-bank | Apache-2.0 | 2026-05-18 | — | (3) | memória de **estratégia** de sucesso **e** de falha; o juiz deles é o próprio modelo | arquitetura → camada C2, com o juiz trocado por portão |
| ace-agent/ace; kayba-ai/agentic-context-engine | Apache-2.0 | 2026-08-24; 2026-09-23 | —; v0.13.0 | (1) | «playbook» por deltas (gerador, refletor, curador = três prompts do mesmo modelo) | arquitetura (delta = revisão por linha da memória) |
| stanfordnlp/dspy; gepa-ai/gepa | MIT | 2026-10-08 | v2.4.12 (tag antiga); v0.1.4 | (2) | mutação reflexiva e fronteira de Pareto; «promover pela média» morreu em 01/10 | arquitetura: gerador de proposta, quem decide é `faixas_decidem` |
| mem0ai/mem0 (~55 mil ★, fonte secundária) | Apache-2.0 | 2026-10-07 | v2.2.1 | 0 | ADD/UPDATE/DELETE pelo modelo = «contradição pelo modelo», morta em 01/10 (uma chamada por gravação num 3B) | arquitetura |
| getzep/graphiti | Apache-2.0 | 2026-10-09 | v0.30.2 | 0 | Neo4j/FalkorDB (R24); `invalid_at` já absorvido | recusar (mantido) |
| topoteretes/cognee | Apache-2.0 | 2026-10-08 | v1.6.3 | 0 | grafo extraído pelo modelo | arquitetura |
| agiresearch/A-mem | MIT | 2025-12-12 | — | 0 | ligações entre notas = `ativacao_espalhada` (fonte 11) | arquitetura |
| MemTensor/MemOS | Apache-2.0 | 2026-09-22 | v2.0.34 | — | não lido em profundidade | arquitetura |
| noahshinn/reflexion | MIT | 2025-01-13 | — | 0 | autocrítica sem sinal externo morreu em 01/10 | recusar |
| MineDojo/Voyager | MIT | **2023-07-27** | — | 0 | biblioteca de skills de código verificado = `chunking_procedural` | arquitetura |
| ShengranHu/ADAS; jennyzzt/dgm | Apache-2.0 | 2025-01-27; 2025-08-13 | — | 0 | US$ 22.000/corrida (DGM); aplica sozinho | recusar (H6) |
| trueagi-io/hyperon-experimental (MeTTa) | MIT | 2026-02-11 | v0.2.10 | 0 | experimental, 8 meses sem commit | recusar |
| scallop-lang/scallop | MIT | 2026-06-26 | 0.2.4 | 0 | Datalog neuro-simbólico de pesquisa | recusar |
| ACT-R | (site da CMU) | — | — | 0 | a equação de ativação de base chega pela fonte 11 (Soar implementa a aproximação de Petrov 2006) | arquitetura via Soar |

### 3.3 Rede neural integrada, local e em CPU

| Candidato | Licença | HEAD | Tag | Papel | Custo medido | Decisão |
|---|---|---|---|---|---|---|
| aurelio-labs/semantic-router (3,9 mil ★) | MIT | 2026-09-29 | linha 0.x estável até 0.2.0; 1.x em desenvolvimento (README) | rota por exemplos, limiar ajustado com rótulos, híbrido denso+esparso | Python; o meio é a camada N1 daqui | **fonte 12** |
| huggingface/candle | Apache-2.0/MIT | 2026-10-02 | 0.11.0 | inferência em Rust (o goose usa `candle-core 0.11` para o Whisper) | dependência nova | arquitetura; decisão de dependência na onda C6 |
| tracel-ai/burn | Apache-2.0/MIT | 2026-10-08 | v0.22 | treino em CPU em Rust | dependência nova | arquitetura (C6) |
| pykeio/ort | Apache-2.0/MIT | 2026-10-02 | 2.0.0-rc.13 (pino do fastembed) | ONNX Runtime | baixa a biblioteca C++ no build (`ort/download-binaries`); o `voz.rs:588` já usa a `libonnxruntime.so` **por processo externo** (sherpa-onnx) | recusar como crate; o caminho por processo já existe |
| Anush008/fastembed-rs | Apache-2.0 | 2026-10-07 | v7.1.1 | embeddings, esparsos, reranker | `ort` + `tokenizers` (onig, C) | arquitetura (catálogo de modelos) |
| MinishLab/model2vec-rs | MIT | 2026-10-05 | v0.3.0 | embedding estático (consulta + média) | multilíngue `potion-multilingual-128M` = **512,4 MB** fp32; o de 30,2 MB é só inglês | candidato da bancada C4 só depois de reduzir |
| ggml-org/llama.cpp; utilityai/llama-cpp-rs | MIT; Apache/MIT | 2026-10-08 | — | LLM local em processo | o Ollama já cobre (`modelo_local`); LoRA fora pela decisão (1) | recusar (seria o segundo motor para a mesma pergunta) |
| LaurentMazare/tch-rs | Apache/MIT | 2026-08-23 | — | libtorch | libtorch tem GB | recusar |
| rust-ml/linfa | Apache/MIT | 2026-08-22 | 0.8.1 | ML clássico | o N0 é regressão logística sobre n-gramas por hashing: raciocinado em ~200 linhas sem crate | recusar |
| lm-sys/RouteLLM | Apache-2.0 | **2024-08-10** | — | roteador de modelo treinado | 26 meses parado | recusar |
| k2-fsa/sherpa-onnx | Apache-2.0 | 2026-10-06 | — | voz | já em uso (`voz.rs`) | — |

Modelos (API do Hugging Face, 09/10/2026):

| Modelo | Licença | Maior arquivo útil em CPU | Língua | Decisão |
|---|---|---|---|---|
| intfloat/multilingual-e5-small | MIT | `onnx/model_qint8_avx512_vnni.onnx` **118,3 MB** | multilíngue | **candidato 1 da bancada C4** |
| sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2 | Apache-2.0 | `model_quint8_avx2.onnx` **118,5 MB** | multilíngue | candidato 2 da C4 |
| all-minilm (o de hoje, por Ollama) | Apache-2.0 | 23 MB (`documentos.rs:8`) | inglês | **perdeu: 3/8 contra 6/8 do BM25** |
| minishlab/potion-multilingual-128M | MIT | **512,4 MB** fp32 | multilíngue | acima do teto (1) como está |
| protectai/deberta-v3-base-prompt-injection-v2 | Apache-2.0 | **738,6 MB** (só fp32) | inglês | recusar como está: teto (1) e língua |
| meta-llama/Llama-Prompt-Guard-2-22M | licença Llama («other») | 283,3 MB | multilíngue | **sobe ao dono** (licença) — recomendação: não |
| jinaai/jina-reranker-v2-base-multilingual | **CC-BY-NC-4.0** | 279,6 MB int8 | multilíngue | recusar: não comercial |

### 3.4 O material do dono (18 repositórios), conferido na fonte primária

`openclaw/openclaw` e `open-jarvis/OpenJarvis` já são as fontes 1 e 5. Os outros 16:

| Repositório | Licença declarada no material | **Licença conferida** | HEAD | Tag | O que tem e nós não | Decisão |
|---|---|---|---|---|---|---|
| openclaw/clawhub | MIT | MIT | 2026-10-08 | v0.24.0 | publicar/versionar/renomear/fundir skills; fixar (`pin`) instalação | **amplia fonte 1**: `registro_skills`, `pin_skill` |
| openclaw/mcporter | MIT | MIT | 2026-10-06 | v0.14.2 | gerar CLI/cliente tipado de MCP; gravar/repetir sessão MCP | **amplia fonte 1**: `mcp_gerar_cliente`, `mcp_gravar_repetir` (a ponte em TS segue recusada, R30) |
| openclaw/openclaw-windows-node | MIT | MIT | 2026-10-07 | v2026.9.8-1 | nó Windows que anuncia só o permitido; câmera; localização; sandbox em três níveis | **amplia fonte 1**: `no_windows`, `camera`, `localizacao` — prova na máquina Windows do dono |
| openclaw/openclaw-ansible | MIT | MIT | 2026-10-07 | v2.0.0 | UFW, fail2ban, atualização automática, Tailscale, Docker | **amplia fonte 1**: `instalacao_servidor_endurecida` — prova na VM do dono |
| VoltAgent/awesome-openclaw-skills | MIT da lista | MIT (só a lista) | 2026-10-05 | — | catálogo | corpus para `importar_skills`, licença conferida skill por skill |
| zeroclaw-labs/zeroclaw | MIT ou Apache-2.0 | MIT + Apache-2.0 | 2026-10-09 | v0.8.5 | §4.4 | **reserva 13** |
| neul-labs/openclaw-rs | MIT | MIT | **2026-07-02** | — | plugin em WASM (wasmtime), sessão como log de eventos (sled) | arquitetura |
| Sycatle/local-jarvis | MIT ou Apache-2.0 | MIT + Apache-2.0 | 2026-09-20 | v0.1.0 (alfa) | microfone ao vivo + VAD + openWakeWord; laço de ferramenta com gramática GBNF | arquitetura (atalho para o `voz_wake` parcial) |
| skyfireitdiy/Jarvis | MIT | MIT | 2026-10-08 | v6.1.0 | esteira C→Rust retomável (`jarvis_c2rust/`: scanner → library_replacer → transpiler → optimizer de `unsafe`/clippy → verify), e a regra `builtin/rules/development_workflow/c2rust_transpiler.md` | **importar a regra como skill** (MIT) pelo `importar_skills`; a esteira é arquitetura — serve ao Phoenix do dono, não pede id novo no PhxClaw |
| microsoft/JARVIS (HuggingGPT) | MIT | MIT | **2025-07-29** | — | o modelo planeja e escolhe modelo especializado lendo a descrição | arquitetura; escolher modelo por descrição lida pelo LLM é «juiz por modelo» — aqui escolhe a medida (`avaliacao.rs`) e, na onda C4, o roteador por exemplos |
| isair/jarvis | não comercial | **não comercial** («for non-commercial purposes») | 2026-10-09 | v2.6.0 | — | **só arquitetura, nunca código** |
| vierisid/jarvis | Source Available 2.0 | **Jarvis Source Available License 2.0 (Based on RSALv2)** | 2026-10-08 | v0.15.0 | — | **só arquitetura, nunca código** |
| Priler/jarvis | CC BY-NC-SA 4.0 | **CC BY-NC-SA 4.0** | 2026-02-18 | — | — | **só arquitetura, nunca código** |
| MatrixCoreX/RustClaw | não comercial | **«Agent Runtime Non-Commercial Source-Available License v1.0»** | 2026-10-08 | v0.1.8 | — | **só arquitetura, nunca código** |
| PanPenek/JarvisAi | MIT «segundo o README» | **sem arquivo `LICENSE`**; só o selo e a palavra MIT no README | 2026-03-01 | — | — | só arquitetura até existir `LICENSE` (sem arquivo, não há concessão a conferir) |
| sukeesh/Jarvis | MIT | MIT | 2025-12-01 | v1.0 | «assistente sem IA» | recusar |

**As implementações em Rust contra a nossa** (números de `Cargo.lock` e da árvore, 09/10/2026):

| | PhxClaw | goose | zeroclaw | openclaw-rs | local-jarvis |
|---|---|---|---|---|---|
| pacotes no `Cargo.lock` | 849 | 1.319 | 1.313 | 503 | 454 |
| sandbox | bwrap (Linux) | — | Landlock → Bubblewrap → Firejail → Docker; Seatbelt no macOS | «system deps» | — |
| canais | 24 ids, 20 no agente | gateway (Telegram) | 22 páginas em `docs/book/src/channels` | «⚠️ Partial» | D-Bus |
| modelo local | Ollama, processo externo | llama.cpp + candle **embutidos** | Ollama | — | llama.cpp + whisper-rs embutidos |
| o que eles têm e nós não | — | compactação, revisor adversário, tool shim, OSV, modo código | recibo HMAC por chamada, GPIO/I2C/SPI/USB, gatilho MQTT, pacote de agente portável | plugin WASM, log de eventos | microfone ao vivo, GBNF |

Divergência nossa que se mantém: **modelo por processo externo**, não embutido. Goose e local-jarvis
embutem o llama.cpp; aqui o Ollama já é o motor de modelo local (`phxclaw-llm`), e um segundo motor
para a mesma pergunta fere «função e comando vêm do mesmo motor».

## 4. As fontes 8 a 12: lista fechada, onde está na doc, e o bloco para colar

Todas lidas em **09/10/2026**. Cada id abaixo é **novo** (não está entre os 191 de hoje); os ids já
existentes que a fonte também tem vão no bloco JSON para a conta por fonte sair certa.

### 4.1 Lista fechada

**8 · `a2a`** — `docs/specification.md` do commit `12e9d2f` (especificação 1.0.0).

| id | o que é | onde |
|---|---|---|
| `a2a_cartao_agente` | publicar o Agent Card no URI bem conhecido | §8.1–8.3, §8.5 |
| `a2a_cartao_assinado` | cartão assinado por JWS sobre JSON canonizado (RFC 7515 + RFC 8785) | §8.4 |
| `a2a_cartao_estendido` | cartão estendido só para cliente autenticado | §3.1.11, §6.9 |
| `a2a_servidor_mensagem` | `SendMessage` devolve Task ou Message | §3.1.1 |
| `a2a_streaming` | `SendStreamingMessage` e `SubscribeToTask` por SSE | §3.1.2, §3.1.6, §3.5.2 |
| `a2a_ciclo_tarefa` | `GetTask`/`ListTasks`/`CancelTask` e os estados terminais | §3.1.3–3.1.5, §4.1.3 |
| `a2a_push` | configuração de notificação por webhook, por tarefa | §3.1.7–3.1.10, §3.5.3 |
| `a2a_multiturno` | `contextId`, `input-required` e autorização dentro da tarefa | §3.4, §7.6 |
| `a2a_artefatos` | Part (texto, arquivo, dado) e Artifact com evento de atualização | §4.1.6–4.1.7, §4.2.2, §6.7–6.8 |
| `a2a_cliente_delegacao` | o PhxClaw delega a um agente remoto achado pelo cartão | §5, §8.2 |
| `a2a_multi_inquilino` | inquilino no pedido | `docs/whats-new-v1.md` «NEW: Multi-Tenancy Support», `docs/topics/multi-tenancy.md` |
| `a2a_extensoes` | extensões declaradas no cartão | §4.6 |
| `a2a_versionamento` | negociação de versão do protocolo | §3.6, §6.4 |

Divergência nossa já decidida: **o cartão se assina com Ed25519** (JWS `EdDSA`, RFC 8037), porque
o `ed25519-dalek` já está no workspace e o RSA ainda não (SP000032 R1); conferir cartão RS256 de
terceiros espera o RSA. **gRPC fora** (§10): pediria protobuf/tonic, e JSON-RPC + REST bastam para
a equivalência funcional que a própria §5.1 exige.

**9 · `letta`** — docs.letta.com, páginas citadas.

| id | o que é | onde |
|---|---|---|
| `memoria_blocos_contexto` | blocos sempre no contexto, com `label`, `description`, `limit` e `read_only` | `guides/agents/memory-blocks` |
| `memoria_autoeditavel` | o agente reescreve o próprio bloco | idem |
| `memoria_compartilhada` | o mesmo bloco/arquivo anexado a vários agentes | `concepts/shared-memory` |
| `memoria_versionada_git` | cada edição de memória é um commit | `concepts/memfs` |
| `memoria_hierarquica_indice` | raiz sempre no contexto; subpastas com `MEMORY.md` lidas sob demanda | idem |
| `consolidacao_sono` | «dreaming»: subagente de fundo revisa as conversas e consolida, a cada N passos ou na compactação | `configuration/memory` |
| `revisao_antes_de_aplicar` | segunda conversa revisa a atualização proposta | idem |
| `auditoria_memoria` | `/doctor`: posição, duplicação, tokens no prompt | idem |
| `memoria_bootstrap_projeto` | `/init`: lê sessões anteriores e monta a memória do projeto | idem |
| `skills_na_memoria` | skills do agente versionadas junto da memória | `concepts/memfs` |

Divergência: na Letta, a revisão «does not ask you for approval». Aqui **toda consolidação nasce
proposta** e espera o Go — decisão (2) do dono e a pétrea «PENDENTE não vira FRUTÍFERO sem evidência».

**10 · `goose`** — `documentation/docs` e `crates/goose/src` do commit `7bdeadb`.

| id | o que é | onde |
|---|---|---|
| `compactacao_contexto` | compacta o histórico ao chegar a 80% do limite; resume chamadas antigas em lotes de 10 | `guides/sessions/smart-context-management.md`; `context_mgmt/mod.rs` |
| `saida_grande_em_arquivo` | saída de ferramenta acima de 200.000 caracteres vai para arquivo (aqui: corte em 6.000, `motor.rs:67`) | `agents/large_response_handler.rs` |
| `revisor_adversario` | revisor independente diz ALLOW/BLOCK antes da chamada, por regras do operador | `guides/security/adversary-mode.md`; `security/adversary_inspector.rs` |
| `deteccao_injecao` | padrões + classificador com limiar (padrão 0,8) sobre o argumento da ferramenta | `guides/security/prompt-injection-detection.md`, `classification-api-spec.md` |
| `verificacao_malware_extensao` | consulta o OSV antes de instalar extensão | `agents/extension_malware_check.rs` |
| `allowlist_extensoes` | lista de servidores MCP permitidos pelo administrador | `guides/allowlist.md` |
| `modo_codigo_ferramentas` | meta-ferramentas: descobre sob demanda e encadeia chamadas por código | `guides/managing-tools/code-mode.md` |
| `tool_shim` | intérprete converte chamada em texto em chamada estruturada (modelo local sem tool calling) | `guides/tool-shim.md` |
| `instrucoes_por_turno` | lembrete persistente injetado a cada turno | `guides/context-engineering/using-persistent-instructions.md`; `agents/moim.rs` |
| `receitas_parametrizadas` | receita com parâmetros; subreceita vira ferramenta | `guides/recipes/recipe-reference.md`, `subrecipes.md` |
| `elicitacao_mcp` | servidor MCP pede dado por formulário no meio da tarefa | `guides/mcp-elicitation.md` |
| `roots_mcp` | a pasta de trabalho chega ao servidor MCP | `guides/mcp-roots.md` |
| `telemetria_otel` | OpenTelemetry | `crates/goose/src/otel` |

Divergência: o revisor do goose é **fail-open** («If the reviewer fails for any reason, the tool
call is allowed through»). Aqui é o contrário, pela restrição do portão único: o revisor e o
classificador **só endurecem** (viram `perguntar`), nunca liberam o que a `regras.rs` não liberou; se
falharem, vale o comportamento de hoje.

**11 · `soar`** — manual em soar.eecs.umich.edu.

| id | o que é | onde |
|---|---|---|
| `ativacao_base` | ranqueia por recência e frequência (`activation-mode base-level`, `base-decay` 0,5, aproximação de Petrov) | cap. 6; `reference/cli/cmd_smem` |
| `ativacao_espalhada` | relevância que se espalha pelas ligações (continuar 0,9; limite 300; profundidade 10) | cap. 6 |
| `memoria_episodica` | episódio gravado por gatilho; busca por pista com consulta negativa; nota = cardinalidade × ativação | cap. 7 |
| `navegacao_temporal_episodios` | `next`/`previous`/`before`/`after` | cap. 7 |
| `chunking_procedural` | regra nova aprendida do resultado de um subobjetivo; **não aprende de escolha não confiável** | cap. 4 |
| `impasse_subobjetivo` | impasse vira subestado com objetivo próprio | caps. 2 e 4 |
| `reforco_preferencias` | valor numérico de preferência por recompensa (Sarsa/Q; taxa 0,3 e desconto 0,9 no exemplo do manual) | cap. 5 |

Divergência: no Soar, quem «resolve» o subobjetivo é a regra que disparou. Aqui **só tarefa cujo
desfecho um portão provou** (teste verde, gabarito, aprovação) gera procedimento — é a mesma
recusa do Soar a «operator selected unreliably», lida contra o nosso fato: a escolha de um LLM é,
por construção, não confiável.

**12 · `semantic_router`** — `docs/*.ipynb` do commit `88fb4dc`.

| id | o que é | onde |
|---|---|---|
| `rota_por_exemplos` | intenção decidida por proximidade a frases de exemplo | `docs/00-introduction.ipynb` |
| `limiar_otimizado` | limiar por rota ajustado com dados rotulados | `docs/06-threshold-optimization.ipynb` |
| `rotas_dinamicas_parametros` | a rota extrai parâmetros e vira chamada | `docs/02-dynamic-routes.ipynb` |
| `roteador_hibrido` | denso + esparso | `docs/examples/hybrid-router.ipynb` |
| `filtro_de_rotas` | restringe as rotas candidatas por pedido | `docs/09-route-filter.ipynb` |

### 4.2 Bloco para o `fontes.json` (NÃO editado nesta rodada)

```json
{
 "a2a": {
  "repositorio": "https://github.com/a2aproject/A2A",
  "doc": "https://a2a-protocol.org/latest/specification/",
  "lido_em": "09/10/2026",
  "licenca": "Apache-2.0. Lido no fonte: A2A 12e9d2f (docs/specification.md, especificacao 1.0.0 de 12/03/2026; v1.0.1 de 26/05/2026). Binding gRPC (§10) fora: exige protobuf/tonic",
  "ids": [
   "api_servidor",
   "a2a_cartao_agente",
   "a2a_cartao_assinado",
   "a2a_cartao_estendido",
   "a2a_servidor_mensagem",
   "a2a_streaming",
   "a2a_ciclo_tarefa",
   "a2a_push",
   "a2a_multiturno",
   "a2a_artefatos",
   "a2a_cliente_delegacao",
   "a2a_multi_inquilino",
   "a2a_extensoes",
   "a2a_versionamento"
  ]
 },
 "letta": {
  "repositorio": "https://github.com/letta-ai/letta",
  "doc": "https://docs.letta.com",
  "lido_em": "09/10/2026",
  "licenca": "Apache-2.0 (letta 5bcdd17 tag 0.16.8; letta-code 253a3bc tag v0.34.8). Memoria compartilhada por repositorio e so na nuvem deles: aqui vira arquitetura",
  "ids": [
   "memoria",
   "subagentes",
   "skills",
   "permissoes",
   "workflows",
   "busca_sessoes",
   "memoria_blocos_contexto",
   "memoria_autoeditavel",
   "memoria_compartilhada",
   "memoria_versionada_git",
   "memoria_hierarquica_indice",
   "consolidacao_sono",
   "revisao_antes_de_aplicar",
   "auditoria_memoria",
   "memoria_bootstrap_projeto",
   "skills_na_memoria"
  ]
 },
 "goose": {
  "repositorio": "https://github.com/aaif-goose/goose",
  "doc": "https://github.com/aaif-goose/goose/tree/main/documentation/docs",
  "lido_em": "09/10/2026",
  "licenca": "Apache-2.0. Lido no fonte: goose 7bdeadb (tag v1.54.0), documentation/docs e crates/goose/src. Rust: leitura direta; nada se embute (1.319 pacotes no Cargo.lock deles)",
  "ids": [
   "shell",
   "edicao_patch",
   "mcp_cliente",
   "subagentes",
   "skills",
   "hooks",
   "plugins",
   "memoria",
   "cron",
   "permissoes",
   "instrucoes_projeto",
   "ide_acp",
   "app_desktop",
   "modelo_local",
   "canal_telegram",
   "remote_control",
   "stt",
   "output_styles",
   "compactacao_contexto",
   "saida_grande_em_arquivo",
   "revisor_adversario",
   "deteccao_injecao",
   "verificacao_malware_extensao",
   "allowlist_extensoes",
   "modo_codigo_ferramentas",
   "tool_shim",
   "instrucoes_por_turno",
   "receitas_parametrizadas",
   "elicitacao_mcp",
   "roots_mcp",
   "telemetria_otel"
  ]
 },
 "soar": {
  "repositorio": "https://github.com/SoarGroup/Soar",
  "doc": "https://soar.eecs.umich.edu/soar_manual/",
  "lido_em": "09/10/2026",
  "licenca": "BSD-2-Clause. So arquitetura e formula (C++); lido no manual caps. 4-7 e cmd_smem; HEAD 3fc337a, tag releases/9.6.5",
  "ids": [
   "memoria",
   "ativacao_base",
   "ativacao_espalhada",
   "memoria_episodica",
   "navegacao_temporal_episodios",
   "chunking_procedural",
   "impasse_subobjetivo",
   "reforco_preferencias"
  ]
 },
 "semantic_router": {
  "repositorio": "https://github.com/aurelio-labs/semantic-router",
  "doc": "https://docs.aurelio.ai/semantic-router/get-started/introduction",
  "lido_em": "09/10/2026",
  "licenca": "MIT. Python: so arquitetura do roteamento; o meio (embedding) e o da camada N1 daqui. HEAD 88fb4dc (docs/*.ipynb)",
  "ids": [
   "guardrails",
   "rota_por_exemplos",
   "limiar_otimizado",
   "rotas_dinamicas_parametros",
   "roteador_hibrido",
   "filtro_de_rotas"
  ]
 }
}
```

E o bloco para o `phxclaw.json` — sem ele o `gerar_absorcao.py` para com «capacidade sem estado»
(`gerar_absorcao.py:26-28`). Cada estado é o medido hoje, com a evidência:

```json
{
 "a2a_cartao_agente": {
  "estado": "nao",
  "evidencia": "grep a2a/agent-card em crates/ = 0 (09/10/2026); api.rs serve /v1 proprio, sem /.well-known/agent-card.json"
 },
 "a2a_cartao_assinado": {
  "estado": "nao",
  "evidencia": "sem JWS/JCS; ha ed25519-dalek no workspace (assinar EdDSA, RFC 8037); conferir RS256 alheio depende do RSA (SP000032 R1)"
 },
 "a2a_cartao_estendido": {
  "estado": "nao",
  "evidencia": "sem GetExtendedAgentCard autenticado"
 },
 "a2a_servidor_mensagem": {
  "estado": "nao",
  "evidencia": "api.rs aceita tarefa pelo formato proprio, nao SendMessage (§3.1.1)"
 },
 "a2a_streaming": {
  "estado": "nao",
  "evidencia": "LIVE_EVENT_API tem eventos proprios; sem SendStreamingMessage/SubscribeToTask"
 },
 "a2a_ciclo_tarefa": {
  "estado": "nao",
  "evidencia": "TaskStore tem get/list/cancel no formato proprio; sem TaskState A2A"
 },
 "a2a_push": {
  "estado": "nao",
  "evidencia": "sem configuracao de push por tarefa (§3.1.7-3.1.10)"
 },
 "a2a_multiturno": {
  "estado": "nao",
  "evidencia": "perguntas.rs AwaitingInput existe, sem contextId/input-required A2A"
 },
 "a2a_artefatos": {
  "estado": "nao",
  "evidencia": "/v1/tasks/{id}/artifacts existe no formato proprio; sem Part/Artifact A2A"
 },
 "a2a_cliente_delegacao": {
  "estado": "nao",
  "evidencia": "equipe.rs delega so a papeis locais (equipe.rs:1-8); sem agente remoto descoberto por cartao"
 },
 "a2a_multi_inquilino": {
  "estado": "nao",
  "evidencia": "api.rs: um Bearer unico (ver projetos_rbac)"
 },
 "a2a_extensoes": {
  "estado": "nao",
  "evidencia": ""
 },
 "a2a_versionamento": {
  "estado": "nao",
  "evidencia": ""
 },
 "memoria_blocos_contexto": {
  "estado": "parcial",
  "evidencia": "motor.rs:480-495 injeta as MEMORIAS_INJETADAS=3 (memoria.rs:33) por relevancia ao objetivo; nao ha bloco fixo com limite, descricao e somente-leitura"
 },
 "memoria_autoeditavel": {
  "estado": "parcial",
  "evidencia": "memory_save com substitui marca invalid_at (memoria.rs:11-14, :107); o agente nao reescreve um bloco"
 },
 "memoria_compartilhada": {
  "estado": "parcial",
  "evidencia": "o arquivo _memoria/<escopo>.json e o mesmo para as tarefas do escopo (memoria.rs:1-9); sem anexar memoria por agente/papel"
 },
 "memoria_versionada_git": {
  "estado": "parcial",
  "evidencia": "invalid_at guarda o que se acreditou antes; sem historico por commit"
 },
 "memoria_hierarquica_indice": {
  "estado": "nao",
  "evidencia": ""
 },
 "consolidacao_sono": {
  "estado": "nao",
  "evidencia": ""
 },
 "revisao_antes_de_aplicar": {
  "estado": "nao",
  "evidencia": "gonogo.rs e o conselho de integradores para integracao, nao para memoria"
 },
 "auditoria_memoria": {
  "estado": "nao",
  "evidencia": ""
 },
 "memoria_bootstrap_projeto": {
  "estado": "nao",
  "evidencia": ""
 },
 "skills_na_memoria": {
  "estado": "parcial",
  "evidencia": "skills em <raiz>/_skills (skills.rs:18), sem versao junto da memoria"
 },
 "compactacao_contexto": {
  "estado": "nao",
  "evidencia": "motor.rs: max_steps 16 (:66) e truncate_for_model por saida (:869); nada resume o historico"
 },
 "saida_grande_em_arquivo": {
  "estado": "nao",
  "evidencia": "motor.rs:869 trunca a saida em max_tool_output_chars; o resto se perde"
 },
 "revisor_adversario": {
  "estado": "nao",
  "evidencia": "regras.rs perguntar pede ao HUMANO; nenhum revisor independente por chamada"
 },
 "deteccao_injecao": {
  "estado": "parcial",
  "evidencia": "instrucoes.rs/importar_skills.rs varrem injecao em instrucao e skill; nada sobre argumento de ferramenta, sem classificador"
 },
 "verificacao_malware_extensao": {
  "estado": "nao",
  "evidencia": "plugins.rs confere assinatura, nao vulnerabilidade (OSV)"
 },
 "allowlist_extensoes": {
  "estado": "nao",
  "evidencia": "nao achado em mcp.rs lista de servidores permitida pelo administrador"
 },
 "modo_codigo_ferramentas": {
  "estado": "nao",
  "evidencia": ""
 },
 "tool_shim": {
  "estado": "nao",
  "evidencia": "phxclaw-llm le tool_calls nativos; modelo sem tool calling nao tem interprete"
 },
 "instrucoes_por_turno": {
  "estado": "nao",
  "evidencia": "instrucoes do projeto entram uma vez no prompt de sistema"
 },
 "receitas_parametrizadas": {
  "estado": "parcial",
  "evidencia": "fluxos.rs (JSON com expressoes) e skills; sem receita com parametros tipados que vira ferramenta"
 },
 "elicitacao_mcp": {
  "estado": "nao",
  "evidencia": ""
 },
 "roots_mcp": {
  "estado": "nao",
  "evidencia": ""
 },
 "telemetria_otel": {
  "estado": "nao",
  "evidencia": "ver observabilidade_insights: sem OTel"
 },
 "ativacao_base": {
  "estado": "nao",
  "evidencia": "phxclaw-memory-context/src/lib.rs:329-334: termo + confianca; recencia so desempata (:419); nenhum registro de acesso"
 },
 "ativacao_espalhada": {
  "estado": "nao",
  "evidencia": ""
 },
 "memoria_episodica": {
  "estado": "parcial",
  "evidencia": "gravacao.rs grava cada passo (v2, :1-21); sessoes.rs busca textual; sem consulta por pista com negativa nem desfecho no episodio"
 },
 "navegacao_temporal_episodios": {
  "estado": "nao",
  "evidencia": ""
 },
 "chunking_procedural": {
  "estado": "parcial",
  "evidencia": "phxclaw-skill-evolution propoe candidato de skill (lib.rs:73-101, :187) fora do agente; otimizacao.rs A/B por faixas"
 },
 "impasse_subobjetivo": {
  "estado": "nao",
  "evidencia": "motor.rs:730-739 corta a 3a chamada repetida, mas impasse nao vira subobjetivo"
 },
 "reforco_preferencias": {
  "estado": "nao",
  "evidencia": ""
 },
 "rota_por_exemplos": {
  "estado": "nao",
  "evidencia": ""
 },
 "limiar_otimizado": {
  "estado": "nao",
  "evidencia": ""
 },
 "rotas_dinamicas_parametros": {
  "estado": "nao",
  "evidencia": ""
 },
 "roteador_hibrido": {
  "estado": "parcial",
  "evidencia": "documentos.rs:6-16 funde BM25 e cosseno (RRF k=60) para BUSCA, nao para rota; desligado por medida (6/8 x 3/8)"
 },
 "filtro_de_rotas": {
  "estado": "nao",
  "evidencia": ""
 },
 "registro_skills": {
  "estado": "parcial",
  "evidencia": "loja.rs instala do indice do community-registry; importar_skills.rs importa SKILL.md; sem publicar/versionar/renomear skill"
 },
 "pin_skill": {
  "estado": "nao",
  "evidencia": ""
 },
 "mcp_gerar_cliente": {
  "estado": "nao",
  "evidencia": "R30 recusou a ponte em TS; capacidade de gerar CLI/cliente tipado nao existe"
 },
 "mcp_gravar_repetir": {
  "estado": "parcial",
  "evidencia": "gravacao.rs grava as chamadas mcp__ dentro da tarefa; sem gravar/repetir uma sessao MCP avulsa"
 },
 "no_windows": {
  "estado": "nao",
  "evidencia": "device-node provado em Linux; no Windows e a SP000025 (maquina do dono)"
 },
 "camera": {
  "estado": "nao",
  "evidencia": ""
 },
 "localizacao": {
  "estado": "nao",
  "evidencia": ""
 },
 "instalacao_servidor_endurecida": {
  "estado": "nao",
  "evidencia": "phxclaw-installer e backup/restore local; sem firewall/fail2ban/VPN/atualizacao automatica (VM do dono)"
 }
}
```

(O bloco acima já traz os 8 ids da ampliação da OpenClaw, §4.3.)

### 4.3 Ampliação da fonte 1 (`openclaw`)

Acrescentar ao fim de `"openclaw".ids`:

```json
["registro_skills", "pin_skill", "mcp_gerar_cliente", "mcp_gravar_repetir", "no_windows", "camera", "localizacao", "instalacao_servidor_endurecida"]
```

Onde: `openclaw/clawhub` README «What you can do with it» (publicar versão com changelog e tag,
renomear, fundir, `pin`); `openclaw/mcporter` README «Core workflows» (`generate-cli`, `emit-ts`,
`record`/`replay`); `openclaw/openclaw-windows-node` README «Capabilities» (Camera, Location; sandbox
Locked Down/Recommended/Unprotected); `openclaw/openclaw-ansible` README «Features». Os quatro: MIT,
commits de 06 a 08/10/2026.

### 4.4 Reserva 13 (`zeroclaw`) — não entra agora

| id | o que é | onde (commit `35dad4a`) |
|---|---|---|
| `recibo_ferramenta` | HMAC-SHA256 com chave efêmera sobre chamada+resultado, anexado à saída: o modelo não consegue fingir que rodou uma ferramenta | `docs/book/src/security/tool-receipts.md` (Basu 2026, arXiv:2603.10060) |
| `perifericos_hardware` | GPIO/I2C/SPI/USB (Raspberry Pi, STM32, Arduino, ESP32) | `docs/book/src/hardware/index.md` |
| `gatilho_mqtt` | SOP disparado por tópico MQTT | `docs/book/src/sop/how-it-works.md` |
| `sandbox_multiplataforma` | cadeia Landlock → Bubblewrap → Firejail → Docker; Seatbelt no macOS | `docs/book/src/security/sandboxing.md` |
| `pacote_agente_portavel` | «agent bundle» para mover um agente entre instalações | `docs/book/src/agents/portability.md` |
| `rotas_modelo_por_dica` | rota de modelo por dica no pedido, com fallback entre perfis | `docs/book/src/providers/routing.md` |

Por que reserva e não fonte: o pedido fixou 3 a 5 fontes, e as cinco acima trazem 48 novas contra
6. O `recibo_ferramenta` porém custa quase nada — o HMAC-SHA256 já está escrito
(`canais/cripto.rs`, conferido contra RFC 4231) — e ataca o defeito que mais pesa num modelo de 3B:
narrar resultado que não obteve. Entra na onda C1 como arquitetura.

**Os itens de produto do n8n (decisão 3).** Os ids já existem (`projetos_rbac`, `segredos_externos`,
`modelos_fluxo`, `instalacao_docker_k8s`, todos `nao`). O que as novas fontes trazem para eles: o
A2A, inquilino e autorização dentro da tarefa (§7, `multi-tenancy.md`) para o RBAC; o
`openclaw-ansible` e os `Dockerfile`/`docker-compose.yml` do zeroclaw e do goose para a instalação;
o clawhub como desenho de galeria. **Lacuna:** nenhuma das 75 conferidas resolve `segredos_externos`
(Vault/AWS/Azure/GCP) — continua só com a n8n como referência.

## 5. Cognição própria em camadas: o que existe, o que falta, a ordem

Regra que atravessa as camadas, e é o que torna o desenho nosso: **nada aprende de uma escolha do
modelo que nenhum portão conferiu, e nada aprendido se aplica sem Go.** O sinal externo é o que já
existe — portão de argumento (SP000028), teste, gabarito, recusa da `regras.rs`, parecer do
`gonogo.rs`. É por isso que a autocrítica pelo próprio modelo morreu em 01/10 e a versão do
ReasoningBank entra com o juiz trocado.

| Camada | O que já existe (arquivo:linha) | O que falta (id) | Fonte |
|---|---|---|---|
| **C0 Trabalho** (o contexto da tarefa) | laço único `motor.rs`; `max_steps` 16 (`:66`); corte da saída em 6.000 caracteres (`:67`, `:869`); 3ª chamada idêntica bloqueada (`:728-739`) | `compactacao_contexto`, `saida_grande_em_arquivo`, `instrucoes_por_turno`, `recibo_ferramenta` | goose, zeroclaw |
| **C1 Episódica** (o que aconteceu) | `gravacao.rs:1-21` (cada pedido, resposta e chamada; v2 com duração e tokens; segredo tirado analisando); `sessoes.rs:1-3` (busca textual); `checkpoint` | `memoria_episodica` (pista + negativa, **desfecho do portão gravado no episódio**), `navegacao_temporal_episodios` | soar |
| **C2 Semântica** (o que se sabe) | `memoria.rs` (`_memoria/<escopo>.json`; `invalid_at` em `:11-14`; injeta 3 em `:33` pelo `motor.rs:480-495`); pontuação `memory-context/src/lib.rs:329-334`, recência só no desempate `:419`; BM25 `bm25.rs`; embedding desligado por medida `documentos.rs:13-16` | `ativacao_base`, `ativacao_espalhada`, `memoria_blocos_contexto`, `memoria_hierarquica_indice`, `memoria_compartilhada`, `memoria_versionada_git` | soar, letta |
| **C3 Procedural** (como se faz) | `skills.rs:18`; `importar_skills.rs` (350 SKILL.md, origem com SHA-256); `otimizacao.rs:1-9` (A/B, promove só sem cruzar faixas); `phxclaw-skill-evolution` (`lib.rs:73-101` observação, `:187` avaliar candidato) **fora do agente** | `chunking_procedural`, `skills_na_memoria`, estratégia de sucesso **e** de falha (ReasoningBank) | soar, letta, reasoning-bank (arq.) |
| **C4 Reflexão e consolidação** | `gonogo.rs:1-3` (conselho de integradores); estados epistêmicos no `knowledge-evidence-graph` (`lib.rs:1-3`, contradição `:420`) **fora do agente** | `consolidacao_sono` (gera PROPOSTA), `revisao_antes_de_aplicar` (o conselho, não um segundo prompt), `auditoria_memoria`, `memoria_bootstrap_projeto` | letta |
| **C5 Avaliação** | `avaliacao.rs` (`faixas_decidem` `:89`; nota parcial de ferramentas; N, faixa e data) | `limiar_otimizado` (usa a mesma régua) | semantic-router |
| **C6 Rede neural local (CPU)** | embedding por Ollama `phxclaw-llm/src/ollama.rs:71`; fusão RRF `documentos.rs:52`; voz por sherpa-onnx (processo externo) `voz.rs:586-588` | N0 classificador linear; N1 embedding multilíngue; N2 `rota_por_exemplos`, `roteador_hibrido`; N3 `deteccao_injecao` | semantic-router, goose |
| **C7 Decisão e controle** | `regras.rs` (`perguntar`), portão único `Agent::call_tool`, `phxclaw-adaptive-model-intelligence` `route_adaptive` (`lib.rs:79`) **fora do agente** | `revisor_adversario` (só endurece), `reforco_preferencias` (valor por ferramenta/papel a partir do desfecho), `impasse_subobjetivo` | goose, soar |
| **C8 Entre agentes** | `equipe.rs:1-8` (110 papéis; filho com a interseção das capacidades) | os 13 `a2a_*` | a2a |

A rede neural, dentro do teto (1) do dono:

- **N0 — classificador linear escrito à mão**, sem crate: regressão logística sobre n-gramas de
  caractere com *hashing* (2^18 posições ≈ 1 MB de pesos em f32 — raciocinado). Rótulos de graça,
  vindos dos portões: chamada recusada / perguntada / aprovada / argumento inválido; tarefa com
  portão verde / vermelho. Usos: **risco** (só endurece: vira `perguntar`) e **intenção → papel ou
  skill** (só sugere; o portão decide). Treina na própria máquina em segundos (raciocinado).
- **N1 — embedding multilíngue pelo Ollama que já existe** (zero dependência nova):
  `multilingual-e5-small` (MIT, 118,3 MB em int8) e `paraphrase-multilingual-MiniLM-L12-v2`
  (Apache-2.0, 118,5 MB). Entra só se ganhar do BM25 **sem cruzar faixas** no gabarito em
  português — o all-minilm, em inglês, perdeu por 3 contra 6.
- **N2 — roteador por exemplos** sobre o N1 (`rota_por_exemplos`, `limiar_otimizado` com os rótulos do N0).
- **N3 — injeção**: o deberta da protectai (738,6 MB fp32, inglês) está fora do teto e da língua;
  primeiro o N0 com rótulos em português, depois um encoder pequeno afinado **na VM de nuvem** do
  dono (custo por uso), nunca nesta máquina.
- **Neuro-simbólico, do jeito daqui:** a rede propõe uma probabilidade; a regra simbólica
  (`regras.rs`, política de capacidades) decide. A rede nunca afrouxa uma regra — é a mesma
  assimetria do «guarda nova entra pedida, não imposta».

### Ondas

| Onda | O quê | Depende de | Aceite (prova real nos dois sentidos) |
|---|---|---|---|
| **C1** — sem modelo e sem dependência | `ativacao_base` (registro de acesso por memória; d = 0,5); `compactacao_contexto`; `saida_grande_em_arquivo`; `recibo_ferramenta`; **desfecho do portão gravado em cada episódio** (prepara o dado de C3) | nada | gabarito de memória: a lembrança usada ontem sobe acima da gravada há um mês com o mesmo termo, e o teste falha com a pontuação de hoje; resultado de ferramenta inventado pelo roteiro é acusado pelo recibo |
| **C2** — estratégia e proposta | memória de estratégia de sucesso e de falha (ReasoningBank com juiz = portão) **como INFRUTÍFERO/FRUTÍFERO**; `consolidacao_sono` gerando propostas PENDENTES; fila de propostas com Go (conselho + dono); `memoria_blocos_contexto`; ligar o `phxclaw-skill-evolution` ao agente **só pelo caminho humano** (§8) | C1 | uma estratégia só vira FRUTÍFERA com o A/B sem cruzar faixas; nada se aplica sem o registro do Go |
| **C3** — N0 | classificador linear de risco e de intenção | **dado**: gravações de modelo real com desfecho; hoje **0**. Portão proposto, raciocinado: ≥ 200 exemplos por classe; a bancada fixa o número | o N0 só endurece: teste que falha se o classificador liberar algo que `regras.rs` pergunta |
| **C4** — N1/N2 | embedding multilíngue pelo Ollama; roteador por exemplos | Ollama instalado + ~120 MB de modelo | ganha do BM25 sem cruzar faixas no gabarito em português, ou fica desligado com o número escrito |
| **C5** — A2A | servidor (cartão, mensagem, SSE, ciclo, push) e cliente | C0 (contexto) | conformidade contra o `a2a.proto`/JSON da `specification/json` do commit fixado, e um agente de outra casa (ADK ou Agents SDK) conversando com o PhxClaw de verdade |
| **C6** — depende do dono | encoder afinado na VM de nuvem; decisão de dependência (`candle`/`ort` em processo) | VM + custo por uso; decisão de dependência | — |

## 6. Hipóteses que morreram, com o número

| Hipótese | O número que a matou |
|---|---|
| H1 — outro orquestrador maduro é a maior lacuna | Agents SDK 3, ADK 2, CrewAI 2, LangGraph 0, AutoGen 0 novas, contra A2A 13 e goose 13 |
| H2 — memória vetorial/grafo primeiro | BM25 6/8 × fusão com all-minilm 3/8 (`documentos.rs:13-16`); Letta MemFS sem índice vetorial por padrão; Graphiti exige Neo4j/FalkorDB |
| H3 — treinar rede nas gravações agora | 46 gravações, 92 respostas, **0** de modelo real (100% `roteiro`); 2 de 1.556 em 01/10 |
| H4 — openclaw-rs e local-jarvis como fonte | openclaw-rs: crates parciais e 3 meses sem commit; local-jarvis: v0.1.0 alfa |
| H5 — ecossistema OpenClaw como fonte própria | mesma organização e licença; partiria um produto em cinco números (fonte 1: 89,7% → 74,5% ampliada) |
| H6 — auto-modificação (DGM/ADAS) | US$ 22.000 por corrida da DGM; decisão (2) do dono |
| deberta-v3 como classificador de injeção | 738,6 MB só em fp32 e treinado em inglês: fora do teto (1) |
| potion-multilingual-128M como embedding padrão | 512,4 MB em fp32: fora do teto (1) como está |
| jina-reranker-v2 multilíngue | CC-BY-NC-4.0 |
| `ort` como crate | baixa a biblioteca C++ no build; o processo externo (sherpa-onnx) já resolve o caso de voz |
| llama.cpp embutido (como goose e local-jarvis) | segundo motor de modelo local ao lado do Ollama: a mesma pergunta respondida por dois motores |
| gRPC no A2A | protobuf/tonic novos; a §5.1 só exige equivalência funcional entre bindings |
| RouteLLM | 26 meses sem commit |
| MetaGPT, Roo-Code, Aider | 9, 5 e 4,5 meses sem commit, e nada novo |
| HuggingGPT escolhendo modelo pela descrição | é juiz por modelo; 14 meses sem commit; aqui escolhe a medida |
| Revisor adversário fail-open (como no goose) | fere o portão único: falha do revisor liberaria o que a política perguntaria |

## 7. O que sobe ao dono — e só isto

1. **Licença** — `meta-llama/Llama-Prompt-Guard-2-22M` (licença Llama, «other»): só se ele quiser
   um classificador de injeção pronto e multilíngue. Recomendação do pesquisador: **não**; o N0 com
   rótulos nossos vem primeiro. O `jina-reranker-v2` (CC-BY-NC) fica recusado sem precisar subir.
2. **Hardware e custo** — (a) a **VM de nuvem** para a onda C6, com custo por uso; (b) a **máquina
   Windows** para provar `no_windows`, `camera`, `localizacao`; (c) a **VM** para provar
   `instalacao_servidor_endurecida`. Ele já disse que fornece as três; o que sobe é **quando**.
   (d) Microfone real para fechar o `voz_wake`; placas para `perifericos_hardware` (só se a reserva
   13 entrar).
3. **Escopo** — entrar com as cinco fontes e a ampliação derruba a absorção de **88,1% para 76,7%**
   (316/412). Pela regra «escopo congelado» que esta casa usa, a sugestão é os 56 ids nascerem
   ⏸ («depois da versão»), menos os da onda C1, que corrigem defeito ativo de modelo pequeno
   (resultado narrado sem ferramenta; contexto perdido no corte de 6.000 caracteres). Prazo e
   escopo são do dono.

Não sobe: qualquer comportamento que as fontes documentam (decidido acima), e a dependência nova
(`candle`/`ort`), que fica fora das ondas C1–C5 e por isso não precisa de decisão agora.

## 8. Achados colaterais

- **`phxclaw-skill-evolution/src/lib.rs:136-138` tem `PromotionPath::AutoLowRisk`.** A decisão (2)
  do dono (09/10) proíbe aplicar sem Go. A crate não está ligada ao agente hoje (só a
  `phxclaw-release-hardening` depende dela, medido pelo `Cargo.toml`), então **não é defeito ativo**
  — mas é o caminho que alguém ligaria na onda C2. Recomendação: a onda C2 entra com um teste que
  falha se o agente promover por `AutoLowRisk`, ou a variante sai.
- **Outra frente mexeu no `phxclaw.json` durante esta pesquisa** (arquivo modificado às 05:41 UTC,
  um `nao` virou `agente`): por isso o «hoje» é 88,1% (290/329) e não o 87,8% do último commit.
- **`gerar_absorcao.py:26-28` para em id sem estado**: colar o bloco do `fontes.json` sem o do
  `phxclaw.json` derruba o gerador. Os dois blocos vão juntos.
- **O goose mudou de casa**: `block/goose` redireciona para `aaif-goose/goose` (o próprio
  `prompt-injection-detection.md` já aponta para lá). A URL da fonte usa a nova.
- **O OpenHands virou interface**: o `HEAD` de `All-Hands-AI/OpenHands` é `electron/` + `src/routes`;
  quem quiser ler o núcleo do agente precisa de outro repositório.

## 9. Fontes consultadas (todas em 09/10/2026)

Repositórios (clone raso e esparso; `LICENSE`, `HEAD`, tags):
https://github.com/a2aproject/A2A ·
https://github.com/aaif-goose/goose ·
https://github.com/letta-ai/letta ·
https://github.com/letta-ai/letta-code ·
https://github.com/SoarGroup/Soar ·
https://github.com/aurelio-labs/semantic-router ·
https://github.com/zeroclaw-labs/zeroclaw ·
https://github.com/langchain-ai/langgraph ·
https://github.com/langchain-ai/deepagents ·
https://github.com/crewAIInc/crewAI ·
https://github.com/microsoft/autogen ·
https://github.com/ag2ai/ag2 ·
https://github.com/openai/openai-agents-python ·
https://github.com/google/adk-python ·
https://github.com/agno-agi/agno ·
https://github.com/mastra-ai/mastra ·
https://github.com/huggingface/smolagents ·
https://github.com/pydantic/pydantic-ai ·
https://github.com/microsoft/semantic-kernel ·
https://github.com/microsoft/agent-framework ·
https://github.com/FoundationAgents/MetaGPT ·
https://github.com/camel-ai/camel ·
https://github.com/kyegomez/swarms ·
https://github.com/All-Hands-AI/OpenHands ·
https://github.com/SWE-agent/SWE-agent ·
https://github.com/SWE-agent/mini-swe-agent ·
https://github.com/cline/cline ·
https://github.com/RooCodeInc/Roo-Code ·
https://github.com/Aider-AI/aider ·
https://github.com/0xPlaygrounds/rig ·
https://github.com/ag-ui-protocol/ag-ui ·
https://github.com/agentscope-ai/agentscope ·
https://github.com/anthropics/claude-agent-sdk-python ·
https://github.com/mem0ai/mem0 ·
https://github.com/getzep/graphiti ·
https://github.com/topoteretes/cognee ·
https://github.com/agiresearch/A-mem ·
https://github.com/MemTensor/MemOS ·
https://github.com/noahshinn/reflexion ·
https://github.com/MineDojo/Voyager ·
https://github.com/stanfordnlp/dspy ·
https://github.com/gepa-ai/gepa ·
https://github.com/ShengranHu/ADAS ·
https://github.com/jennyzzt/dgm ·
https://github.com/google-research/reasoning-bank ·
https://github.com/ace-agent/ace ·
https://github.com/kayba-ai/agentic-context-engine ·
https://github.com/trueagi-io/hyperon-experimental ·
https://github.com/scallop-lang/scallop ·
https://github.com/huggingface/candle ·
https://github.com/tracel-ai/burn ·
https://github.com/LaurentMazare/tch-rs ·
https://github.com/pykeio/ort ·
https://github.com/ggml-org/llama.cpp ·
https://github.com/utilityai/llama-cpp-rs ·
https://github.com/Anush008/fastembed-rs ·
https://github.com/MinishLab/model2vec-rs ·
https://github.com/rust-ml/linfa ·
https://github.com/lm-sys/RouteLLM ·
https://github.com/k2-fsa/sherpa-onnx ·
https://github.com/openclaw/clawhub ·
https://github.com/openclaw/mcporter ·
https://github.com/openclaw/openclaw-windows-node ·
https://github.com/openclaw/openclaw-ansible ·
https://github.com/VoltAgent/awesome-openclaw-skills ·
https://github.com/neul-labs/openclaw-rs ·
https://github.com/Sycatle/local-jarvis ·
https://github.com/skyfireitdiy/Jarvis ·
https://github.com/microsoft/JARVIS ·
https://github.com/isair/jarvis ·
https://github.com/vierisid/jarvis ·
https://github.com/Priler/jarvis ·
https://github.com/MatrixCoreX/RustClaw ·
https://github.com/PanPenek/JarvisAi ·
https://github.com/sukeesh/Jarvis

Documentação e artigos:
https://a2a-protocol.org/v1.0.0/specification ·
https://docs.letta.com/guides/agents/memory-blocks ·
https://docs.letta.com/guides/agents/architectures/sleeptime ·
https://docs.letta.com/llms.txt ·
https://docs.letta.com/concepts/memfs/index.md ·
https://docs.letta.com/configuration/memory/index.md ·
https://docs.letta.com/concepts/shared-memory/index.md ·
https://soar.eecs.umich.edu/soar_manual/ ·
https://soar.eecs.umich.edu/soar_manual/04_ProceduralKnowledgeLearning/ ·
https://soar.eecs.umich.edu/soar_manual/05_ReinforcementLearning/ ·
https://soar.eecs.umich.edu/soar_manual/06_SemanticMemory/ ·
https://soar.eecs.umich.edu/soar_manual/07_EpisodicMemory/ ·
https://soar.eecs.umich.edu/reference/cli/cmd_smem/ ·
https://docs.aurelio.ai/semantic-router/get-started/introduction ·
https://arxiv.org/pdf/2505.22954 (Darwin Gödel Machine, apêndice de custo) ·
https://research.google/blog/reasoningbank-enabling-agents-to-learn-from-experience/ ·
https://arxiv.org/html/2509.25140v1 (ReasoningBank) ·
https://huggingface.co/papers/2510.04618 (ACE) ·
https://doi.org/10.48550/arXiv.2603.10060 (Tool Receipts, citado pelo zeroclaw) ·
https://forkast.news/glossary/agent-memory/ (estrelas do Mem0 — fonte secundária) ·
https://kestra.io/resources/ai/ai-agent-orchestration-frameworks ·
https://www.firecrawl.dev/blog/best-open-source-agent-frameworks ·
https://www.morphllm.com/ai-agent-framework (varredura de «novos em 2026»)

Hugging Face (`/api/models/<id>?blobs=true`): intfloat/multilingual-e5-small,
Xenova/multilingual-e5-small, sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2,
minishlab/potion-multilingual-128M, minishlab/potion-base-8M,
protectai/deberta-v3-base-prompt-injection-v2, meta-llama/Llama-Prompt-Guard-2-22M,
jinaai/jina-reranker-v2-base-multilingual.

Material do dono: `jarvis_openclaw_github_2026-10-09.md` (18 repositórios), conferido item por
item na §3.4.

Código nosso lido (árvore de trabalho de `phxclaw/v070-nativo`, 09/10/2026): `docs/absorcao/fontes.json`,
`docs/absorcao/phxclaw.json`, `docs/absorcao/gerar_absorcao.py`, `docs/TECNOLOGIAS.md` §5,
`docs/absorcao/TRIAGEM_PESQUISAS_2026-10-01.md`, `crates/phxclaw-agent/src/{memoria,motor,gravacao,avaliacao,otimizacao,equipe,skills,documentos,sessoes,gonogo,voz,mcp}.rs`,
`crates/phxclaw-memory-context/src/lib.rs`, `crates/phxclaw-skill-evolution/src/lib.rs`,
`crates/phxclaw-knowledge-evidence-graph/src/lib.rs`, `crates/phxclaw-adaptive-model-intelligence/src/lib.rs`.
