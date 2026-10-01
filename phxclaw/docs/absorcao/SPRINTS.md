# Backlog de Sprints — PhxClaw até 100% de absorção

```text
Sprint SP000001 | 01/10/2026 | planejamento (concluída)
```

Numeração global, sequencial, sem reuso. Estados: **EM EXECUÇÃO** (frente rodando agora), **PLANEJADA**
(entra quando a anterior integrar), **BLOQUEADA** (espera decisão ou credencial do dono).

Base medida (gerar_absorcao.py, 01/10, contando o que as frentes de git e interação já entregaram e
ainda não comitaram): Claude Code 82,9% · Codex 67,4% · OpenClaw 56,4% · Hermes 55,0% · OpenJarvis 38,2%.
No `phxclaw.json`: **39 chaves «nao» e 15 «parcial»** — são elas que estas sprints fecham. Toda chave
abaixo aparece em exatamente uma sprint.

## Visão geral

| Sprint | Onda | Foco | Chaves | Estado |
|---|---|---|---:|---|
| SP000002 | 3 | Integração da onda 3 (git, interação, canais) e commit | — | CONCLUÍDA (4670bc20) |
| SP000003 | 3 | Canais A: laço único + 8 canais principais | 8 | CONCLUÍDA (4670bc20) |
| SP000004 | 3 | Canais B: 16 canais restantes | 16 | CONCLUÍDA (4670bc20) |
| SP000005 | 4 | Orquestração e plugins | 5 | CONCLUÍDA (4670bc20) |
| SP000006 | 4 | Editor e remoto | 7 | CONCLUÍDA (4670bc20) |
| SP000007 | 4 | Voz e mídia | 5 | CONCLUÍDA (4670bc20) |
| SP000008 | 4 | Integração da onda 4 e commit | — | CONCLUÍDA (4670bc20) |
| SP000009 | 5 | Contexto e dados | 5 | CONCLUÍDA (f27402e5) |
| SP000010 | 5 | Credencial e CI | 4 | CONCLUÍDA (f27402e5) |
| SP000011 | 5 | Medição | 4 | CONCLUÍDA (f27402e5) |
| SP000012 | 5 | Integração da onda 5 e commit | — | CONCLUÍDA (f27402e5) |
| SP000013 | — | Endurecimento (achados ⏸ das revisões) | — | PLANEJADA |
| SP000014 | — | Prova real com credenciais | — | BLOQUEADA (dono) |
| SP000015 | — | Ciclo de auto-evolução | — | BLOQUEADA (dono) |
| SP000016 | — | Entrega v0.71 | — | PLANEJADA |
| SP000017 | — | Provedores ElevenLabs (fala e transcrição) e Nano Banana (gerar e editar imagem) | — | CONCLUÍDA (f27402e5) |
| SP000018 | — | config.json central, fase 1: catálogo, precedência, recusa de segredo, `phxclaw config`, catraca | — | CONCLUÍDA (f27402e5) |
| SP000019 | — | Tela de configuração do config.json (GET/PUT /v1/config) e phx-grid nas listagens | — | CONCLUÍDA (f27402e5) |
| SP000020 | — | config.json, fase 2: leitores migrados ao ponto único (catraca até 0); os 103 JSON de config/ | — | PLANEJADA |
| SP000021 | 6 | Tela→UI-IR com layout pelas caixas do OCR; troca medida para qwen3-vl | — | CONCLUÍDA (onda 6) |
| SP000022 | 6 | Prova de fidelidade da conversão de tela (ida e volta + bloco/texto/posição) | — | CONCLUÍDA (onda 6) |
| SP000023 | 6 | Segredo no commit: gitleaks num hook do git_write | — | CONCLUÍDA (onda 6) |
| SP000024 | 7 | Navegador pela árvore de acessibilidade (refs, elemento novo marcado, coberto por modal fora); MCPs por configuração (context7, dbhub); embedding de código se o recall pedir | — | PLANEJADA |
| SP000025 | 7 | pywinauto pelo device-node num Windows com WinDev | — | BLOQUEADA (dono: máquina Windows) |
| SP000026 | — | Conselho de integradores no agente: `go_no_go` registra parecer por integrador; Go só unânime, um NoGo bloqueia, parecer faltando aguarda | — | CONCLUÍDA (onda 6) |
| SP000027 | — | Qualificação da UI (12/12 telas, Style Phoenix Padrão) | — | CONCLUÍDA (cd48386e) |
| SP000028 | 7 | Portão que valida e confere o fim: validador de esquema com caminho e todos os erros, 2 tentativas por ferramenta, final_answer tipado, comando de verificação, fim com falha sem resolver recusado | — | CONCLUÍDA (onda 7) |
| SP000029 | 7 | Retomar e bifurcar pela gravação: `retomar --do-passo N`, passo humano no fluxo, pergunta pendente que sobrevive a reinício | — | PLANEJADA |
| SP000030 | 7 | Medir melhor: nota parcial (LCS, conjunto) no avaliar, duração/tokens/passo-pai por passo, SHA do prompt, memória com invalid_at | — | PLANEJADA |
| UI-R01 | 8 | Phx Responsive UI — contratos e layout: intenção responsiva no UI-IR (janela e contêiner), breakpoints num JSON único, motor que compila para Grid/Flexbox/container queries, sem perder estado ao redimensionar | — | CONCLUÍDA (onda 7) |
| UI-R02 | 8 | Adaptador Bootstrap substituível: componentes semânticos → Bootstrap 5.3, tokens do PhxClaw nas variáveis do Bootstrap, arquivo local com versão fixada, sem o JS do Bootstrap mexer no DOM controlado | — | CONCLUÍDA (onda 7) |
| UI-R03 | 8 | Studio e templates: editor visual, prévia por largura, inspetor que explica a regra aplicada, template com UUIDv7 e propagação versionada sem apagar sobrescritas | — | PLANEJADA |
| UI-R04 | 8 | Skill phx-responsive-ui e qualidade: propõe mudança no IR, não HTML; regressão visual, teclado, reflow a 320 px e WebViews reais do Tauri | — | PLANEJADA |
| | | **Total de chaves** | **54** | |

```mermaid
gantt
    title Dependência das sprints (não é calendário)
    dateFormat X
    axisFormat %s
    section Onda 3
    SP000003 Canais A          :a3, 0, 2
    SP000004 Canais B          :a4, after a3, 2
    SP000002 Integração 3      :a2, after a4, 1
    section Onda 4
    SP000005 Orquestração      :b5, 0, 3
    SP000006 Editor e remoto   :b6, 0, 3
    SP000007 Voz e mídia       :b7, 0, 3
    SP000008 Integração 4      :b8, after a2, 1
    section Onda 5
    SP000009 Contexto          :c9, after b8, 2
    SP000010 Credencial e CI   :c10, after b8, 2
    SP000011 Medição           :c11, after b8, 2
    SP000012 Integração 5      :c12, after c11, 1
    section Fecho
    SP000013 Endurecimento     :d13, after c12, 2
    SP000016 Entrega v0.71     :d16, after d13, 1
```

Portões de toda sprint (não se repetem abaixo): rustfmt nos arquivos tocados, `cargo clippy` zero
avisos, testes verdes, **RED medido** (o teste central falha com o defeito reposto), UI exercitada no
Chromium quando houver tela, catraca de idiomas sem subir, número visível só de gerador, nada comitado
pela frente — só o integrador comita.

---

## SP000002 — Integração da onda 3

| Campo | Conteúdo |
|---|---|
| **Objetivo** | Um commit com git/código, interação e canais, verde no conjunto. |
| **Papéis** | A (integra) · B · F · G · SEC |
| **Dependências** | SP000003, SP000004; conserto da frente de interação (achados do QA) |

**Tarefas**
1. Esperar canais e o conserto do QA na interação (regras para quem roda processo, nome de ferramenta único, filtro do `parallel_research`, catraca de `CAPACIDADES_PADRAO`, guarda genérica do bubblewrap).
2. Conferir no `motor.rs` que `checkpoint::no_portao` continua no `call_tool_com` e que a árvore não tem mutante (`*.bak` de mutação).
3. fmt, clippy do workspace, testes dos crates tocados com `--no-fail-fast`.
4. Regerar `ferramentas.json` (hoje 42; medido 56), `absorcao.json` e o bloco gerado do `AGENTE_AUTONOMO.md`.
5. Cognição do redirecionamento (PENDENTE → FRUTÍFERO só com o commit da prova).

**Aceite**
- [ ] Clippy do workspace com 0 avisos e testes verdes — saída anexada ao commit.
- [ ] `gerar_doc_agente.py` sem «FEZ MENOS».
- [ ] Nenhum `.bak` nem mutante na árvore.

---

## SP000003 — Canais A: laço único + principais

| Campo | Conteúdo |
|---|---|
| **Objetivo** | Arquitetura comum extraída do Telegram e os canais de maior uso. |
| **Chaves** | canal_discord, canal_slack, canal_whatsapp, canal_teams, canal_matrix, canal_email, webhooks, webchat |
| **Papéis** | B (titular) · F · SEC |
| **Dependências** | — |

**Tarefas:** trait de provedor + laço único (permitido → `criar_tarefa` → cursor depois da tarefa → resposta partida); `channel_send` com parâmetro de canal; `AwaitingInput` vira pergunta no chat; senha SMTP pelo SecretBroker; limpeza de segredo única na `Credencial`.

**Aceite**
- [ ] Testes do Telegram verdes sem mudar comportamento.
- [ ] Cada provedor provado contra servidor falso (dito como «contra falso»).
- [ ] Teste de vazamento de segredo em erro roda em laço por todos os provedores.

---

## SP000004 — Canais B: restantes

| Campo | Conteúdo |
|---|---|
| **Chaves** | canal_signal, canal_googlechat, canal_sms, canal_mattermost, canal_rocketchat, canal_zulip, canal_irc, canal_xmpp, canal_mastodon, canal_line, canal_viber, canal_messenger, canal_feishu, canal_reddit, canal_twitch, canal_nostr |
| **Dependências** | SP000003 (laço único) |

**Aceite:** cada canal pelo mesmo laço, sem segundo laço; IRC/Twitch contra servidor IRC falso por TCP; testes agrupados num só binário (disco).

> canal_imessage fica em SP000006 (depende do Mac do dono para a prova real). Contagem: 16 chaves.

---

## SP000005 — Orquestração e plugins

| Campo | Conteúdo |
|---|---|
| **Chaves** | workflows, plugins, dispositivos, tarefas_nuvem_paralelas, ambientes_nuvem |
| **Dependências** | worktrees (onda 3) |

**Tarefas:** DAG pelo `phxclaw-task-graph`, retomável; plugins `.claude-plugin/` e `.codex-plugin/` mapeados a skills/hooks/MCP/subagentes com Ed25519; `node_invoke` com a tripla lista; N tarefas em worktree + bwrap, best-of-N.

**Aceite:** DAG com falha no meio retoma de onde parou; plugin adulterado recusado; comando fora de qualquer das três listas recusado; ambientes_nuvem fica **parcial** até decisão de VM (produto, SP000014).

---

## SP000006 — Editor e remoto

| Campo | Conteúdo |
|---|---|
| **Chaves** | lsp, ide_acp, web_nuvem, celular, remote_control, busca_x, canal_imessage |

**Aceite:** erro de tipo plantado aparece pelo rust-analyzer real; `phxclaw acp` responde a cliente ACP; PWA instalável conferido no Chromium; controle remoto só por conexão de saída (3 processos locais); busca_x e iMessage contra falso, URL do BlueBubbles redigida analisando.

---

## SP000007 — Voz e mídia

| Campo | Conteúdo |
|---|---|
| **Chaves** | canvas, tts, voz_conversa, voz_wake, geracao_midia |
| **Risco** | licença do binário de TTS (espeak-ng GPL-3) — se embutir, para e sobe ao dono |

**Aceite:** WAV gerado transcreve de volta ao texto; widget com erro de sintaxe recusado com linha e coluna; iframe sem acesso ao token; geração de imagem contra falso (sem GPU, dito).

---

## SP000008 — Integração da onda 4

Mesmo roteiro da SP000002. Aceite extra: E2E do desktop e `ui_navegacao` verdes com as telas novas.

---

## SP000009 — Contexto e dados

| Campo | Conteúdo |
|---|---|
| **Chaves** | instrucoes_projeto, entrada_imagem, importar_skills, indexacao_documentos, pesquisa_profunda |

**Tarefas:** `AGENTS.md` da raiz ao cwd (teto 32 KiB, override, `CLAUDE.md` reserva, varredura anti-injeção); imagem na mensagem até Ollama/Anthropic/OpenAI; importador de skills com `scripts/` desligado e origem SHA-256; BM25 dentro do memory-context; citação conferida como trecho literal.

**Aceite:** os 350 `SKILL.md` reais importam; PNG com texto conhecido lido pelo qwen2.5vl:3b; citação inventada é recusada.

---

## SP000010 — Credencial e CI

| Campo | Conteúdo |
|---|---|
| **Chaves** | linear, conectores_google, github_action, clima |
| **Primeiro passo** | Bearer + OAuth PKCE no cliente MCP (uma peça abre Linear e Google) |

**Aceite:** MCP falso que exige Bearer; OAuth falso; ação composta com `GITHUB_EVENT_PATH` falso; clima **real** pelo MET Norway (CC BY 4.0).

---

## SP000011 — Medição

| Campo | Conteúdo |
|---|---|
| **Chaves** | gravar_repetir, avaliacao_modelos, telemetria_energia, otimizacao_skills |

**Aceite:** gravar com Ollama e repetir sem ele dá a mesma sequência de chamadas; p50/p95 e tokens/s com faixa e data; energia aparece como **«não medida (sem RAPL)»**, nunca estimada (telemetria_energia fica parcial); variante pior de skill não é promovida (faixas min–max).

---

## SP000012 — Integração da onda 5

Mesmo roteiro da SP000002.

---

## SP000013 — Endurecimento

Achados das revisões que nasceram ⏸ (não bloqueavam a entrega):
1. Agenda: formato com versão, erro do `save()` devolvido ou «pelo menos uma vez» documentado.
2. Checkpoint: pontos por pasta no `mcp-serve --trabalho` com trava; poda dos antigos.
3. Pergunta pendente que sobrevive a reinício; `phxclaw responder <id>`.
4. Regras de comando: `rm -fr` × `rm -rf` (normalizar opções).
5. UI: 46 textos fora da fábrica (topo, splash, painel, rodapé) e chaves com pt em inglês.
6. Testes que pulam calados sem pg/chromium/whisper/tesseract: passam a dizer que pularam.
7. `phxclaw-postgres-bootstrap` cita `phx db migrate`, comando que não existe.

**Aceite:** cada item com teste RED → GREEN; catraca de idiomas baixa.

---

## SP000014 — Prova real com credenciais — BLOQUEADA

Troca «contra falso» por «real». Depende do dono:

| Precisa | Para |
|---|---|
| tokens de cada canal | SP000003/4 |
| token GitHub e GitLab (`phxclaw forja token`) | github, gitlab |
| `XAI_API_KEY` | busca_x |
| chave do Linear, cliente OAuth Google + prévia do Gmail MCP | SP000010 |
| Mac com BlueBubbles | canal_imessage |
| máquina com RAPL | telemetria_energia |
| decisão de VM na nuvem (preço) | ambientes_nuvem |

---

## SP000015 — Ciclo de auto-evolução — BLOQUEADA

| Campo | Conteúdo |
|---|---|
| **Objetivo** | O PhxClaw escolhe um item deste backlog, implementa numa worktree e entrega um branch verde esperando aprovação. |
| **Bloqueio** | (1) modelo forte por API e a chave; (2) alcance permitido (proposta: ferramentas e testes; vetado: segurança, sandbox, capacidades) |

**Tarefas:** heartbeat → escolher item → `git_worktree` → modo plano → implementar → portões (fmt, clippy, testes) → `code_review` do próprio diff → branch + relatório. Ligar ou aposentar `phxclaw-self-evolving-intelligence` e `phxclaw-skill-evolution` (hoje nenhum dos dois é dependência do agente).

**Aceite:** nunca faz merge; diff que toca caminho vetado é recusado pelo portão; item sem portão verde não vira branch.

---

## SP000016 — Entrega v0.71

**Tarefas:** documentação gerada sem «FEZ MENOS»; tabela de absorção final; pacote de fontes e binário por script; instalação em diretório limpo e uma tarefa ponta a ponta.

**Aceite:** prova de instalação limpa anexada; números da UI e dos docs batem com os geradores.

---

## Contagem

| Grupo | Sprints | Chaves |
|---|---:|---:|
| Onda 3 (canais) | 3 | 24 |
| Onda 4 | 4 | 17 |
| Onda 5 | 4 | 13 |
| Fecho | 4 | — |
| **Total** | **15** | **54** |

> Números de chaves contados do `phxclaw.json` em 01/10/2026. Nenhuma sprint desta lista está
> concluída além da SP000001.


---

## Acréscimos de 01/10/2026 (pedidos do dono depois do plano)

- **SP000017–SP000020** saíram de pedidos diretos: ElevenLabs e Nano Banana, config.json central e a tela dele, phx-grid nas listagens.
- **SP000021–SP000025** saíram da triagem do papel J sobre as duas pesquisas de 01/10 (visão e redes
  neurais; repositórios do GitHub): dos 49 itens, 6 entram, 7 já existiam ou estão na onda 5, 36 ficam
  como inspiração ou recusa. Decisões e hipóteses que morreram em `TRIAGEM_PESQUISAS_2026-10-01.md`.
- Achado da triagem que muda o plano: a conversão de tela em UI-IR por visão **já existe**
  (`screenshot_to_erp_ui`, qwen2.5vl:3b + confirmação por OCR, ~92 s por tela). O que falta é layout e
  prova de fidelidade, e é isso que a onda 6 faz.


---

## Phx Responsive UI (decisão do dono, 01/10/2026)

A interface é descrita pela INTENÇÃO no UI-IR (quantas colunas por espaço disponível, com base
na janela ou no contêiner); um motor responsivo compila isso para CSS nativo; um adaptador visual
veste os componentes. O adaptador «phoenix» (nativo, tokens do Style Phoenix Padrão) é o da
interface do próprio PhxClaw; o adaptador Bootstrap é um plugin substituível para os sistemas
gerados. Bootstrap nunca decide layout (nada de `col-*` como contrato) e não é dependência do
núcleo. As sprints usam o nome que o dono deu (UI-R01..R04), sem renumerar as SP.

## Achados de segurança de 01/10/2026 que entram na SP000013 (endurecimento)

Da revisão adversária de f27402e5 e dbdb4dc1 (os ALTOS A1–A3 e M1, M2, B1 voltaram à frente de
git/gonogo e se consertam antes de qualquer outro Go):
- M3 renovação OAuth não serializada (oauth.rs:583-589) — duas tarefas renovam com o mesmo refresh token.
- M4 segredo do broker preso só ao nome do servidor MCP, não ao endpoint (oauth.rs:312-345, mcp.rs:269-279).
- M5 confiança do projeto vale para a raiz do .git mas a leitura parte do cwd; AGENTS.md por symlink (config.rs:75-88, instrucoes.rs:50-73).
- M6 detecção de credencial no config.json por lista curta de prefixos, e o erro ecoa o valor (carga.rs:158-314).
- B2 If-Match do config só dentro do processo; .json.tmp com nome fixo.
- B3 importar_skills segue symlink em scripts e trava em laço de symlinks; corpo da skill sem varredura anti-injeção.
- B4 imagens: dimensões não conferidas (bomba de descompressão).
- B5 cerca do AGENTS.md fecha com maiúsculas; lista negra fraca.
- B6 pesquisa profunda: answer não conferido, [n] de citação recusada fica, páginas sem cerca.
- Da documentação: comandos de credencial leem do ambiente e a ajuda não diz; 6 segredos sem comando de broker.

## Parecer do DBA de 01/10/2026 (formatos em disco) — defeitos ATIVOS, na conta

1. **Caixa dos canais perde mensagem que já respondeu 200** (canais/caixa.rs:57-126): linha cortada
   no fim não é truncada e a próxima gravação nasce colada nela; falha no meio de um lote duplica `seq`.
   Conserto: `set_len` até o último `\n` no abrir e na falha.
2. **config.json sem trava entre processos** (config-runtime/lib.rs:185-230, agente/carga.rs:564-631):
   temporário de nome fixo, CLI e servidor juntos escrevem dentro do arquivo vivo; conferência de
   revisão tautológica no `definir`; revisão da API como soma tem ABA (3+1 = 2+2). Conserto: padrão do
   gonogo (trava, temporário por pid, fsync de arquivo e pasta) e token de revisão como par ou SHA.
3. **Gravação não tolera cauda cortada** (gravacao.rs:157-233, 381-438), embora prometa.
4. **BM25 entra em pânico com índice incoerente** (memory-context/bm25.rs:249-272).
Entram agora por custo zero: `"v": 1` por linha da caixa; `"formato": 1` no ORIGEM.json; simetria e
leitura de versão antes do serde no UI-IR (junto da v3). Achado sistêmico: sete lugares com
temporário + rename em três níveis de garantia — um helper só (o do gonogo, aprovado).
Provas executadas pelo DBA: 0 de 6 (disco cheio durante a revisão).

## Inventário do QA de 01/10/2026 — guardas que faltam (na conta da SP000013)

Nenhuma catraca subiu (config 135/135, idiomas 0/0, CAPACIDADES_PADRAO 21 → 23 com motivo escrito).
Pétreas sem guarda provada:
1. Ferramenta que cria processo e não está em CAPACIDADES_QUE_EXECUTAM: ocr, screenshot_to_erp_ui,
   image_render, lsp, web.browse — a regra de comando «negar» não as alcança.
2. Portas indiretas fora da varredura do bwrap: `Browser::launch` (adaptadores.rs, fidelidade_ui.rs e
   responsivo_ui.rs em curso) e `McpStdioSession::spawn` (comando do operador fora do bwrap).
3. Segredo de LLM fora do broker: phxclaw-llm/src/lib.rs lê OPENAI/ANTHROPIC/GEMINI_API_KEY do
   ambiente; a GEMINI_API_KEY vive em dois regimes desde a onda 6.
4. Laço de eco dos provedores pagos com contagem digitada (8) e sem o xAI; deve sair da lista de `Servico`.
5. Teste que pula calado conta como verde (segredos, voz_e_midia, sistema, visao) — os placares
   «407/410 verdes» incluem pulos; nenhum portão reprova PULADO.
6. ferramentas.json e AGENTE_AUTONOMO.md sem teste de «arquivo velho».
7. Paridade CLI × ferramenta do gonogo sem guarda; isento «EN» sem uso na catraca de idiomas.
Nenhuma guarda de guardas.rs tem prova de que falha com o defeito reposto.

## Prova F de 01/10/2026 (f27402e5/dbdb4dc1): na conta da SP000013

12 mutantes: 10 sobreviviam à suíte comitada, 1 morria, 1 era defeito real. Os testes novos caem
com cada mutante e passam sem ele.

- ☐ **Defeito ativo:** `config::iniciar` não é chamado em produção. Depois de um PUT, a leitura volta
  à `pasta_padrao()`, e com `--pasta X` o `git.segredos.exigir` gravado não vale. O teste está com
  `#[ignore]` em `tests/config.rs`. Mandado à frente da onda 7.
- ☐ **PULADO conta como verde:** sem o gitleaks, a suíte dá `ok` igual. Já existe
  `tests/comum/pulado.rs`, que grava `target/tmp/pulados.jsonl`. Falta o portão ler o registro (pulo
  é NoGo na máquina que tem o recurso), migrar os outros 21 PULADO em 7 arquivos e dar registro a 3
  `return` mudos do `contexto_dados.rs`.
- ☐ **M10 da ui-ir:** `grupo_rand` fixo em 1.0 sobrevive. Teste proposto e mandado à frente
  responsiva.
- ☐ **Pulo que volta calado:** teste que faz `return` sem imprimir nada não aparece nem nos 46 lugares
  nem no registro. O grep do integrador de 01/10 não achou nenhum, mas o limite é da busca.
