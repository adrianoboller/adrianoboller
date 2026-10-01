# Triagem das pesquisas de 01/10/2026 (papel J)

Entrada: duas pesquisas documentais trazidas pelo dono — visão e redes neurais (18 itens) e
repositórios do GitHub (31 itens). Nenhuma delas instalou ou testou nada; esta triagem conferiu
licença no arquivo LICENSE de cada repositório e tamanho no Ollama.

Resultado: **6 entram (2 ADOTAR, 4 INTEGRAR), 7 já existiam ou estão na onda 5, 36 ficam como
inspiração ou recusa.**

## Achados que mudam o plano

- A conversão de tela em UI-IR por visão **já existe**: `screenshot_to_erp_ui`
  (`crates/phxclaw-agent/src/ui.rs`), qwen2.5vl:3b + confirmação por OCR (`phxclaw-ui-ir/src/imagem.rs`),
  ~92 s por tela (`docs/ESTEIRA_ABSORCAO.md` U3). Falta layout (posição, grupo, ordem) e prova de fidelidade.
- Licenças: Serena GPL-3.0 (só SolidLSP é MIT); Widget2Code sem LICENSE (todos os direitos
  reservados); OmniParser CC-BY-4.0; motor do Semgrep LGPL-2.1; SDK Rust do MCP em transição MIT →
  Apache-2.0; Langfuse com pasta `ee` comercial.
- Tamanhos (Ollama): qwen3-vl:2b 1,9 GB, qwen3-vl:4b 3,3 GB, qwen3-embedding:0.6b 639 MB,
  qwen3-coder:30b 19 GB. gitleaks 8.28: 8,2 MB.

## Decisões

| id | item | estado no PhxClaw | decisão | motivo |
|---|---|---|---|---|
| V01 | Qwen3-VL | parcial (qwen2.5vl:3b) | INTEGRAR: troca medida pelo Ollama | Apache-2.0; 2b tem 1,9 GB contra 3,2 GB |
| V02 | OmniParser | não | INSPIRAR | CC-BY-4.0, torch + YOLO; fica a ideia de id + caixa por elemento |
| V03 | GUI-Actor | não | RECUSAR | só torch, sem GGUF; localiza alvo de clique, não converte tela |
| V04 | UI-TARS | não | RECUSAR | 7B em CPU; automação Windows sem prova aqui |
| V05 | ShowUI-2B | não | RECUSAR | duplica o qwen3-vl:2b |
| U01 | ScreenCoder | parcial | INSPIRAR → SP000021 | fica a etapa «planejar layout» entre perceber e gerar |
| U02 | Screenshot-to-Code | parcial | RECUSAR | modelo em nuvem e HTML direto, sem UI-IR |
| U03 | Widget2Code | não | RECUSAR | sem LICENSE; a DSL intermediária já é o UI-IR |
| C01 | Qwen3-Coder | não | RECUSAR local | 19 GB; modelo forte por API é o bloqueio da SP000015 |
| C02 | Qwen3-Embedding/Reranker | onda 5 (BM25 + all-minilm) | INTEGRAR condicional (SP000024) | 639 MB; só se a onda 5 medir recall@5 baixo |
| C03 | CodeBERT e família | não | RECUSAR | antigos, sem Rust/WLanguage; lsp + BM25 cobrem |
| C04 | CodeT5 | não | RECUSAR | repositório arquivado |
| T01 | UI-TARS Desktop | parcial (DesktopTool) | RECUSAR | Electron; captura e repetição na onda 5 |
| T02 | Design2Code | não | INSPIRAR → SP000022 | fica a métrica bloco/texto/posição, sem CLIP |
| T03 | OSWorld | não | RECUSAR | imagens de VM de dezenas de GB |
| T04 | Windows Agent Arena | não | RECUSAR aqui | precisa de VM Windows (dono) |
| T05 | LPIPS | não | RECUSAR | mede aparência; a tela gerada difere de propósito |
| T06 | Playwright | já tem (CDP próprio) | RECUSAR | duplicaria |
| R01 | spec-kit | já tem (plan_mode, SPRINTS) | onda 5 (corpus de skills) | MIT |
| R02 | superpowers | já tem | onda 5 (corpus) | MIT |
| R03 | oh-my-claudecode | já tem (equipe, parallel_tasks) | RECUSAR | duplica |
| R04 | ECC | já tem (hooks) | onda 5 (corpus) | MIT |
| R05 | BMAD | — | RECUSAR | processo concorrente |
| R06 | serena | já tem (lsp.rs) | RECUSAR | GPL-3.0 e duplica |
| R07 | context7 | não | INTEGRAR por MCP (SP000024) | MIT; precisa de rede e chave |
| R08 | github-mcp-server | já tem (forja.rs) | RECUSAR | fere «mesmo motor» |
| R09 | windmill | já tem (fluxos.rs) | RECUSAR | AGPLv3 + binário proprietário |
| R10 | temporal | já tem (fluxos retomáveis) | RECUSAR | serviço pesado |
| R11 | activepieces | já tem (gatilhos + webhooks HMAC) | RECUSAR embutir | integra por webhook hoje |
| R12 | n8n | já tem (idem) | RECUSAR embutir | Sustainable Use License |
| R13 | robotframework | não | RECUSAR | runner Python; provas são Rust + Chromium |
| R14 | pywinauto | não | INTEGRAR pelo device-node Windows (SP000025) | BSD-3; só no Windows |
| R15 | playwright-mcp | parcial (seletor CSS) | INSPIRAR → ADOTAR nativo (SP000024) | árvore de acessibilidade com refs via CDP |
| R16 | playwright-cli | onda 5 (gravar_repetir) | — | — |
| R17 | browser-use | já tem (browser_*) | RECUSAR | duplica |
| R18 | shadcn | parcial (react.rs) | INSPIRAR | estados vazio/erro/carregando |
| R19 | refine | parcial (react.rs) | RECUSAR | duplica CRUD gerado |
| R20 | xyflow | não | RECUSAR (⏸) | editor de fluxo não pedido |
| R21 | dbhub | parcial (postgres READ ONLY) | INTEGRAR por MCP (SP000024) | MIT; MySQL/MariaDB/SQLite; não cobre HFSQL nem PhxSql |
| R22 | docling | parcial (read_document, OCR) | RECUSAR | torch + modelos de layout |
| R23 | pgvector | onda 5 | — | índice no memory-context venceu |
| R24 | graphiti | não | RECUSAR | exige Neo4j/FalkorDB |
| R25 | promptfoo | onda 5 | — | — |
| R26 | langfuse | parcial (gravar_repetir) | RECUSAR embutir | ClickHouse, Redis, S3; parte `ee` |
| R27 | semgrep | parcial (clippy + code_review) | RECUSAR | regras com licença fora da OSI |
| R28 | gitleaks | não | INTEGRAR por CLI num hook no bwrap (SP000023) | MIT, 8,2 MB |
| R29 | sandbox-runtime | já tem (bwrap + egress-broker) | RECUSAR | duplica |
| R30 | mcporter | já tem (mcp.rs) | RECUSAR | ponte em TS |
| R31 | SDK Rust do MCP | já tem (mcp.rs próprio) | RECUSAR a troca | licença em transição; serve de referência de conformidade |

## Hipóteses que morreram

- **Converter a tela:** modelo de interface dedicado (OmniParser/GUI-Actor/ShowUI) somado ao atual —
  morreu: torch, +1 a 7 GB, fora do Ollama. Ficou: caixas do OCR que já temos + troca medida do modelo de visão.
- **Fidelidade:** LPIPS ou CLIP — morreu: mede aparência, e a tela gerada difere da original de
  propósito. Ficou: ida e volta determinística SQL → tela → PNG → screenshot_to_erp_ui → SQL.
- **Segredos:** reescrever as ~200 regras do gitleaks em Rust — morreu: manutenção nossa. Ficou: o
  binário com SHA-256 no bwrap, padrão da casa (whisper, tesseract, sherpa).
- **Navegador:** embutir playwright-mcp — morreu: traz Node e duplica o phxclaw-browser. Ficou: retrato
  da árvore de acessibilidade pelo CDP próprio.

## Não medido

- Ganho real do layout pelo OCR numa tela de ERP e o tempo por tela: decide o TSV do tesseract nas
  telas do gabarito da SP000022, com a máquina quieta.
- Se o WinDev expõe árvore UIA: decide a SP000025 numa tela WinDev real.
- Qwen3-VL contra qwen2.5vl: decide o gabarito da SP000022.

## Sobe ao dono

- Máquina ou VM Windows com WinDev (SP000025).
- Chave do context7 (o serviço recebe o nome das bibliotecas consultadas).
- Modelo de código forte só por API paga (o mesmo bloqueio da SP000015).


---

# Acréscimo: os 10 repositórios de agentes (01/10/2026, tarde)

LangGraph, PydanticAI, Browser Use (MIT); Agno, Cognee, Graphiti, E2B, DeepEval (Apache-2.0);
Mastra (Apache-2.0 menos `ee/`); Langfuse (parte `ee`). Fonte lido em clone raso, já apagado.

**O dado que decidiu:** das 1.556 tarefas gravadas no disco, só 2 usaram modelo real (qwen2.5:3b).
Numa delas, 4 de 9 chamadas falharam por argumento inválido; o erro do serde não dava o caminho do
campo, o modelo repetiu o mesmo erro duas vezes e a tarefa fechou `completed` sem resolver.
Amostra de 1 tarefa: raciocinado, não medido como taxa — a bancada antes/depois decide.

| repo | mecanismo | decisão |
|---|---|---|
| LangGraph | estado por passo com ponteiro ao anterior; retomar de um passo antigo bifurca; interrupt | ADOTAR nativo pela gravação que já existe (SP000029) — nós desfazemos também os arquivos |
| PydanticAI | valida antes de rodar, devolve todos os erros com caminho, orçamento de tentativas, saída tipada | ADOTAR nativo (SP000028): validador próprio das 7 palavras-chave usadas nos 66 esquemas, orçamento 2 |
| Mastra | suspender/retomar passo com esquema; checagem de conclusão 0/1 | INSPIRAR → passo humano no fluxo (SP000029) e comando de verificação (SP000028) |
| Agno | quatro modos de time | RECUSAR: três já existem |
| Graphiti / Cognee | arestas bitemporais; grafo extraído pelo modelo | INSPIRAR → memória com substituição explícita e `invalid_at` (SP000030) |
| Browser Use | índice nos elementos, `*` no novo, filtro por ordem de pintura | INSPIRAR → somar à SP000024 |
| E2B | pausa/bifurca microVM | RECUSAR a VM; bifurcar cai na SP000029 |
| Langfuse | prompt por (nome, versão) | ADOTAR mínimo: SHA do prompt e das skills na tarefa (SP000030) |
| DeepEval | ToolCorrectness determinístico (LCS/conjunto); G-Eval com juiz | ADOTAR a nota parcial (SP000030); RECUSAR juiz de 3B |
| laço avaliação → retry | — | já existe no motor; falta comando de verificação e recusar fim com falha (SP000028) |

**Hipóteses que morreram:** checkpointer próprio (duplica a gravação); crate `jsonschema` (árvore
grande para 7 palavras-chave); orçamento 1 (mataria a tarefa num 3B); contradição de memória pelo
modelo (uma chamada por gravação num 3B, Kuzu sem manutenção); CRIU (não há processo vivo, GPL-2.0);
autocrítica pelo próprio modelo (ganho fraco sem sinal externo); modos do Agno como frente; G-Eval por
logprobs (juiz de 3B, suporte do Ollama não medido).
