# Esteira de absorção — manus-open e o anexo de projetos (30/09/2026)

Regra: **inspiração, não cópia**. Nada entra sem passar pelo firewall de proveniência
(`phxclaw-provenance-core`), agora também pela linha de comando:

```bash
echo '{"source":"org/repo","license":"permissive","license_file":true,"hash":"…","origin_known":true}' \
  | cargo run -q -p phxclaw-provenance-core --example avaliar   # sai 3 se houver DENY
```

## 1. manus-open — veredito

| Fonte | Licença | Decisão | Motivo |
|---|---|---|---|
| `whit3rabbit/manus-open` | nenhuma (sem LICENSE) | **DENY** | README declara código **descompilado** do bytecode do Manus: `DeclaredLeak` + `ProprietarySource` |

Nenhuma linha do repositório foi lida para dentro do PhxClaw. O que se absorve é só a
**lista de lacunas de capacidade**, que é pública (o Manus as anuncia) e se resolve com
desenho nosso:

| # | Lacuna | Estado no PhxClaw | Próximo passo |
|---|---|---|---|
| E1 | Processo de terminal em segundo plano (iniciar, ler saída parcial, matar) | `shell_bg` (start/status/stop/list) montado pela mesma função do `shell`; teto de 4 vivos por tarefa e vigia de 10 min; stop conferido pelo `/proc`; RED medido | ✓ 30/09 |
| E2 | Edição precisa de arquivo (`str_replace`, ver faixa de linhas, inserir) | `edit_file` (trecho único, recusa 0 ou >1) e `read_file` com faixa numerada; 4 testes, RED medido | ✓ 30/09 |
| E3 | Limite de taxa por token na API | balde de fichas na criação de tarefa (`PHXCLAW_API_TAREFAS_POR_MINUTO`, padrão 10); 429 com `Retry-After`; consulta e pedido inválido não gastam | ✓ 30/09 |

## 2. Anexo — o que se estuda, por licença

Licenças marcadas **conferida** vieram da pesquisa de 28/09 no arquivo LICENSE de cada
repositório; as demais **ainda não foram conferidas** e o firewall exige conferir antes de
abrir o fonte.

| Projeto | Licença | Decisão | O que estudar |
|---|---|---|---|
| Suna | Elastic-2.0 (conferida) | **DENY para código** | só ideias públicas |
| Crush | FSL-1.1-MIT (conferida) | **DENY até virar MIT** | — |
| Claude Code | proprietária (conferida) | **DENY** | — |
| SuperDesign | AGPL + comercial (conferida) | **DENY para código** | ideia de tela gerada por prompt |
| OpenStitch | sem licença (conferida) | **DENY** | — |
| Lambda ERP | Apache-2.0 (conferida) | ALLOW estudo | padrões de tela ERP |
| UI-Flow, ERP Agent AI | não encontrados | — | — |
| Goose, Codex, OpenCode, Cline, Gemini CLI, Aider, OpenHands, OpenManus, Browser Use, LangGraph, CrewAI, Agent Zero, IronClaw, ZeroClaw, OpenFang, Pi, mini-SWE-agent, nanobot | a conferir | pendente | E1/E2 (Codex, Goose), MCP (Goose), repo map (Aider) |

Prioridade do anexo mantida: Goose, Codex, IronClaw, ZeroClaw, Pi, Agent Zero.

## 3. Phoenix ERP UI Designer — v0 entregue

Crate `phxclaw-ui-ir`: **SQL → UI-IR → HTML**. O analisador não conhece o renderizador (a
exigência do anexo): `schema.rs` produz o `App` do `ir.rs`; `html.rs` só lê o `App`.

- DDL PostgreSQL/MySQL: `CREATE TABLE`, PK, FK (coluna e tabela), `CHECK IN`, serial/identity/
  auto_increment; o que não entende vira aviso, não silêncio.
- Heurística ERP: FK → lookup; `CHECK IN` → select; nome monetário → dinheiro; texto longo →
  área; auditoria → só leitura; filha com FK para a mãe → **mestre/detalhe** com totais;
  menu Cadastros/Movimentos; rótulos PT (`dt_emissao` → «Data emissão»).
- Prova: 5 testes de análise + 1 no **Chromium real** (dois itens, total «R$ 1.244,75»).
  Três rótulos ruins (`Pedidos item`, `Cliente código`, `Total valor total`) só apareceram
  **no print** — consertados com teste que falha com o defeito reposto.

| # | Próximo | Estado |
|---|---|---|
| U1 | ferramenta do agente `design_erp_ui` (grava `ui-ir.json` + `index.html`, publicável) | ✓ 30/09 — qwen2.5:3b a chamou no 1º passo e concluiu |
| U2 | Prompt → UI-IR | ✓ 30/09 — o modelo escreve o `CREATE TABLE` e o analisador determinístico faz o resto (hipótese «modelo preenche o IR» não foi preciso medir: SQL o modelo já sabe). qwen2.5:3b gerou a oficina com OS mestre/detalhe e total; desviou em 2 chamadas e numa planilha que ninguém pediu. Achou o defeito das tabelas no plural («Clienteses»), consertado com RED |
| U3 | Screenshot → UI-IR (`screenshot_to_erp_ui`) | ✓ 30/09 — OCR (tesseract por) + qwen2.5vl:3b local em 3 perguntas; cada rótulo confirmado contra o OCR (o modelo não inventa campo), tipo/obrigatório por regra. Print do pedido → mestre/detalhe certo em 92 s. Hipóteses mortas com número: moondream (0 rótulos), OCR + qwen2.5:3b escolhendo (4/8 e 2/6) |
| U4a | renderizador React (esbuild; `design_erp_ui` com `react: true`) | ✓ 30/09 — construído com npm real e exercitado no Chromium: itens, total, remoção, data, menu; RED medido |
| U4b | regras em WLanguage (`Validar_`, `Incluir_`, `Excluir_`, `Total_` por arquivo HFSQL) | ☐ gerado; mesmas mensagens do Rust (teste de paridade); as 12 funções emitidas conferidas no Help WLanguage 2026 (corpus do plugin WX, página por função, guarda por teste). A conferência achou o `DateValid` juliano antes de 1582 e o Rust foi alinhado. **Falta compilar no WinDev**, e isso só o dono pode fazer |
| U4c | Flutter (`lib/ui.dart` fixo + `lib/telas.dart` gerado + teste de widget gerado) | ✓ 30/09 — SDK 3.47.5 real: `analyze` sem problema, o teste que o projeto traz passa (itens somam, 31/02 recusada, calendário do `DateValid`), `build web` sobe no Chromium do PhxClaw; RED medido. Achou defeito no navegador do agente: idioma `en-US@posix` quebrava `Intl.Locale` |
| U5 | data no formato do idioma | ✓ 30/09 — máscara dd/mm/aaaa própria (o nativo segue o idioma do navegador); 31/02 recusada no Chromium, RED medido |
| U6 | regras em Rust (crate só `std`: structs, validação, banco com FK e restringir, totais, sequência que não volta) | ✓ 30/09 — o teste gera o crate, compila com `cargo` de verdade e roda o comportamento; RED medido |

**Decisão (30/09, pedido do dono: «todo código WLanguage será no final Rust»).** Hipóteses:
(a) gerar WLanguage e depois traduzir WLanguage→Rust; (b) gerar os dois do mesmo modelo.
(a) morreu: exigiria um frontend de WLanguage só para ler de volta um texto que nós mesmos
escrevemos, e a equivalência teria de ser provada por comportamento de qualquer jeito. (b)
venceu: `regras.rs` é o modelo único, `rust.rs` e `wlanguage.rs` o leem, e o teste de
paridade reprova qualquer mensagem que exista num e não no outro. Tradução WLanguage→Rust
de código **escrito à mão** (o legado) continua sendo o caminho do Octopus, fora desta esteira.

## 4. Material WX do dono (30/09) — plugin de conversão e corpus do Help WLanguage

- **Uso aqui:** o corpus Help_WL_12k (SHA-256 `a95ed553…`, estado DEGRADED/CONDITIONAL
  declarado pelo próprio material) serve de **semântica técnica** das funções que o gerador
  emite, que é o nível 6 da hierarquia de evidências do material. Não é fonte de regra de negócio.
- **Fora do git:** o repositório é público e o próprio material pede para confirmar a
  licença do corpus antes de qualquer distribuição. Ele é lido da área de trabalho da sessão,
  sem extração, e aqui só entram os **identificadores de página**.
- **Plugin (6 skills, 22 agentes, gates G0–G7):** é o caminho para converter **legado WX
  escrito à mão**, com evidência e teste de equivalência. Não foi instalado: ele não se
  instala sozinho, e hoje não há projeto WX real para o G0.

| # | Próximo | Estado |
|---|---|---|
| W1 | Primeiro projeto WX real do dono pelo plugin (G0 intake → G4 piloto vertical, destino Rust) | ☐ aguardando o projeto |

