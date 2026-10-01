# Pesquisa das ondas 4 e 5 (papel J, 01/10/2026)

Fontes primárias lidas em clones rasos: openclaw `11828a08` (MIT), hermes-agent `b4ed99f3` (MIT),
codex `90d7f27` (Apache-2.0), openjarvis `f0ecea0` (Apache-2.0), claude-code `525d3b3` (só a
documentação como inspiração; licença proprietária).

## Medido no contêiner (01/10)
- Sem `/sys/class/powercap`: energia **não se mede aqui**; o painel diz «não medida (sem RAPL)».
- Daemon docker parado; bwrap funciona.
- Crate `piper` do Cargo.lock é o tubo do smol, **não** o TTS.
- Cliente MCP por HTTP não manda `Authorization`: bloqueia Linear e Google até ganhar Bearer/OAuth.
- `phxclaw-llm/src/ollama.rs` já manda `images`; a mensagem do agente é que não leva imagem.
- 350 `SKILL.md` reais nos clones (81 com `scripts/`) servem de corpus para importar_skills.
- Clima: MET Norway responde 200 (CC BY 4.0); Open-Meteo é só não comercial; OpenWeatherMap pede chave.
- Tempo do whisper tiny medido SOB CARGA (8,2): 10–23 s por janela de 2 s. Refazer com a máquina quieta.

## Decisões (escolhida / hipótese que morreu)
| id | escolhida | morreu, e por quê |
|---|---|---|
| tts | comando externo com SHA-256 do modelo (sherpa-onnx + Kokoro/Piper) | Piper embutido: GPL-3 + espeak-ng; edge-tts: nuvem |
| voz_wake | sherpa-onnx KWS em fluxo de WAV | openWakeWord: modelos CC BY-NC-SA; Porcupine: proprietário; whisper contínuo: lento |
| voz_conversa | whisper → Ollama → TTS, turno a turno | Realtime/WebRTC: nuvem e áudio |
| geracao_midia | cliente ComfyUI/API de imagens; local = SVG renderizado | FLUX-schnell Q2_K 4,0 GB só o modelo; SD-Turbo licença Stability |
| canvas | widget HTML servido pelo `servir`, JS checado por tree-sitter, CSP sandbox | JS no servidor (QuickJS/boa): dependência nova |
| ide_acp | `phxclaw acp` à mão sobre o JSON-RPC do mcp-lsp-runtime; prova com `acpx` | crate agent-client-protocol: 20 deps fora do lock |
| lsp | LspStdioSession no agente: definição/referências/símbolos + diagnóstico após edição | busca por texto: é o que já existe |
| web_nuvem/celular/remote_control | UM cliente PWA; remoto por WSS de saída via device-transport | porta de entrada/túnel; três clientes (fere «mesmo motor») |
| busca_x | x_search da xAI | API v2 do X: paga |
| canal_imessage | REST+webhook BlueBubbles, URL redigida analisando (senha vai na query) | chat.db: só no Mac |
| workflows | DAG executado pelo phxclaw-task-graph, retomável | roteiro JS; motor de fluxo novo (duplicaria) |
| plugins | `.claude-plugin/`/`.codex-plugin/` mapeados a skills/hooks/MCP/subagentes + Ed25519 | WASM (wasmtime pesado); .so dinâmico |
| dispositivos | `node_invoke` com tripla lista (nó, pareamento, capacidade) | SSH: transporte duplicado |
| tarefas/ambientes paralelos | worktree + bwrap por tarefa, best-of-N | Docker (sem daemon); VM remota = decisão de produto |
| github_action | ação composta rodando o phxclaw | ação Docker |
| linear / conectores_google | MCP oficial + Bearer / OAuth PKCE | cliente próprio GraphQL/REST (duplica o MCP) |
| clima | MET Norway | Open-Meteo (não comercial), OWM (chave) |
| instrucoes_projeto | AGENTS.md da raiz ao cwd, teto 32 KiB, override; CLAUDE.md reserva; varredura anti-injeção | só o cwd (os três convergem no contrário) |
| entrada_imagem | partes de imagem na mensagem → Ollama/Anthropic/OpenAI | só OCR |
| indexacao_documentos | BM25 dentro do memory-context + reordenação all-minilm | FTS5/tantivy fora do lock; FTS do PG |
| pesquisa_profunda | planejar→buscar→ler→citar, citação conferida como trecho literal | confiar na citação do modelo |
| importar_skills | importador com scripts desligados por padrão e origem com SHA-256 | executar scripts importados |
| otimizacao_skills | A/B sobre gravações, promove só sem cruzar faixas | DSPy/GEPA; efeito não medido nem pelo OpenJarvis |
| avaliacao_modelos/telemetria_energia | model-arena + ai-benchmark: p50/p95, tokens/s, CPU-s; energia «não medida» | RAPL/NVIDIA: hardware ausente |
| gravar_repetir | JSONL de modelo+ferramentas, segredos redigidos analisando; repetição por modelo falso | só transcrição |

## Ordem
- Onda 4: editor e remoto (lsp → ide_acp → PWA → busca_x → imessage); orquestração e plugins; voz e mídia (canvas → tts → voz_conversa → voz_wake → geracao_midia).
- Onda 5: contexto e dados; credencial e CI (Bearer/OAuth no MCP primeiro); medição.

## Sobe ao dono (produto)
- VM na nuvem para ambientes (preço).
- Instalador baixar binário de TTS que possa embutir espeak-ng (GPL-3) — não conferido.
