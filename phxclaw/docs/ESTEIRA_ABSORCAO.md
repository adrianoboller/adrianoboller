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
| E1 | Processo de terminal em segundo plano (iniciar, ler saída parcial, matar) | só `shell` síncrono com prazo | ferramenta `shell_bg` no mesmo `run_in_workdir` (bwrap), com teto de processos por tarefa |
| E2 | Edição precisa de arquivo (`str_replace`, ver faixa de linhas, inserir) | só `write_file` inteiro | ferramenta `edit_file` com substituição única exigida (recusa se 0 ou >1 ocorrência) |
| E3 | Limite de taxa por token na API | ausente | balde por token em `api.rs`, 429 com `Retry-After` |

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
| U1 | ferramenta do agente `design_erp_ui` (grava `ui-ir.json` + `site/index.html`, publicável) | ☐ |
| U2 | Prompt → UI-IR (modelo preenche o IR, validado pelo serde) | ☐ |
| U3 | Screenshot → UI-IR (visão) | ☐ bloqueado: modelo com visão |
| U4 | renderizadores WinDev/WebDev (WLanguage), React, Flutter | ☐ |
| U5 | data no formato do idioma (o print mostra `mm/dd/yyyy`: é o locale do navegador, não do IR) | ☐ |
