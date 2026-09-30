# Agente autônomo do PhxClaw

O equivalente local do Manus: recebe um objetivo, planeja, usa ferramentas (web, navegador,
shell isolado, documentos, e-mail, site), registra cada passo em evidência com hash
encadeado e entrega arquivos.

## Rodar

```bash
cargo build -p phxclaw
./target/debug/phxclaw agente "Pesquise X e crie relatorio.docx" --modelo ollama:qwen2.5:3b [--plano]
./target/debug/phxclaw servir --porta 8787          # API de tarefas + agenda
./target/debug/phxclaw core status                 # estado SONDADO (sandbox, navegador, modelo)
```

Modelos: `ollama:<modelo>` (local), `openai:<modelo>`, `anthropic:<modelo>`, `gemini:<modelo>`
(chaves em `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`).

## Ferramentas e política

Nada roda sem a capacidade concedida; a ferramenta negada nem aparece ao modelo.

| Capacidade | Ferramentas | Padrão |
|---|---|---|
| `web.search` | `web_search` (DuckDuckGo; SearXNG com `PHXCLAW_SEARXNG_URL`; Brave com `BRAVE_API_KEY`) | sim |
| `web.browse` | `browser_open/read/click/type/screenshot` (Chromium headless; rede interna bloqueada) | sim |
| `fs.read` / `fs.write` | `read_file`, `list_files`, `write_file`, `read_document` (só a pasta da tarefa) | sim |
| `doc.write` | `create_document` (.docx), `create_spreadsheet` (.xlsx), `create_presentation` (.pptx) | sim |
| `shell.exec` | `shell` (bwrap, `/work` persistente, **sem rede**) | sim |
| `agent.spawn` | `parallel_research` (até 6 subagentes em paralelo) | sim |
| `site.publish` | `publish_site` (servido em `/sites/<tarefa>/<pasta>/`, CSP sandbox) | sim |
| `mail.send` | `send_email` (SMTP; só destinatários de `PHXCLAW_EMAIL_PERMITIDOS`) | **não** |

`PHXCLAW_CAPACIDADES=web.search,fs.read,...` troca a lista. SMTP: `PHXCLAW_SMTP_HOST`,
`_PORT`, `_SECURITY` (tls|starttls|plain-só-loopback), `_USER`, `_PASSWORD`, `PHXCLAW_EMAIL_FROM`.

## API (`phxclaw servir`)

Bearer em `var/agente/api.token` (0600) ou `PHXCLAW_API_TOKEN`. Só loopback por padrão.

| Método | Rota | O quê |
|---|---|---|
| POST | `/v1/tasks` | `{objective, model?, plan_first?, webhook?}` → `{id}` |
| GET | `/v1/tasks`, `/v1/tasks/{id}` | lista / estado, passos, resposta, artefatos |
| POST | `/v1/tasks/{id}/plan`, `/approve` | Plan Mode: editar o plano e aprovar |
| POST | `/v1/tasks/{id}/cancel` | cancela |
| GET | `/v1/tasks/{id}/artifacts/{path}` | baixa artefato (CSP sandbox, nosniff) |
| POST/GET | `/v1/schedules` | `{name, objective, cron \| every_seconds>=60}` |
| GET | `/sites/{id}/{pasta}/` | site publicado pelo agente |

Webhook de fim de tarefa só para origens de `PHXCLAW_WEBHOOK_ORIGINS`.

## Guardas do motor (cada uma nasceu de uma falha medida)

- teto de passos e de tokens; prazo por ferramenta que chega ao processo filho;
- terceira chamada idêntica não roda;
- o fim é a ferramenta `final_answer`; texto solto recebe até 2 lembretes;
- **conclusão verificada**: arquivo citado no objetivo tem de existir, senão a tarefa termina
  `failed` dizendo qual falta.

## O que está provado e o que não está

- Provado no fio: os 4 provedores (formato, auth, chamada de ferramenta) contra servidor
  local; Ollama real; Chromium real; DuckDuckGo real; SMTP contra servidor local; .docx/.xlsx/
  .pptx abertos por leitores independentes e pelo LibreOffice; API HTTP; agenda.
- Ponta a ponta com modelo local de 3B: busca real → `rust.xlsx` gravado. **O conteúdo saiu
  errado** (limite do modelo); a tarefa do site falhou e o status diz `failed`.
- **Não provado**: nuvem real (sem chaves), e-mail num servidor real, canais Telegram/Slack como
  entrada, tela de tarefas no Command Center.
