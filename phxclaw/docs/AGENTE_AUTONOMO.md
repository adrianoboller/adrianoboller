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

O que configurar primeiro, credenciais, confiança no projeto e medição:
[GUIA_DO_OPERADOR.md](GUIA_DO_OPERADOR.md).

Modelos: `ollama:<modelo>` (local), `openai:<modelo>`, `anthropic:<modelo>`, `gemini:<modelo>`
(chaves em `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`).

## Ferramentas e política

Nada roda sem a capacidade concedida; a ferramenta negada nem aparece ao modelo. A lista
abaixo **não se digita**: sai de `python3 tools/gerar_doc_agente.py`, que roda
`phxclaw ferramentas` (a mesma `Montagem` do agente) e lê o fonte. A montagem varia por
máquina — sem `bwrap` não há `shell` nem `python_project`, sem Chromium não há navegador,
sem SMTP não há `send_email`, sem token não há `github` — e por isso há duas tabelas: o que
montou aqui e o que existe no código mas não montou.

<!-- gerado:ferramentas:inicio -->
Medido em 2026-10-09 por `python3 tools/gerar_doc_agente.py`, de `phxclaw ferramentas` (versao 0.70.0, binario de 2026-10-09 14:22), com `PHXCLAW_CAPACIDADES` no padrao. **74 ferramentas montadas nesta maquina**, 61 concedidas por padrao.

| Capacidade | Padrao | Ferramentas |
|---|---|---|
| `agent.parallel` | sim | `parallel_tasks` |
| `agent.spawn` | sim | `parallel_research` |
| `calc` | sim | `calculator` |
| `code.review` | sim | `code_review` |
| `desktop.control` | **nao** | `desktop` |
| `doc.write` | sim | `design_erp_ui`, `screenshot_to_erp_ui`, `create_document`, `create_spreadsheet`, `create_presentation` |
| `flow.run` | **nao** | `fluxo` |
| `fs.read` | sim | `read_file`, `list_files`, `ocr`, `image`, `read_document`, `zip_list`, `data_file`, `pdf`, `glob`, `grep`, `file_history`, `notebook_read`, `checkpoint_list`, `lsp` |
| `fs.write` | sim | `write_file`, `edit_file`, `image_render`, `zip`, `data_file_format`, `pdf_create`, `replace_in_project`, `file_history_restore`, `notebook_edit`, `checkpoint_restore` |
| `git.read` | sim | `git` |
| `git.write` | sim | `git_write`, `git_worktree` |
| `gonogo.write` | **nao** | `go_no_go` |
| `http.request` | **nao** | `http_request` |
| `media.generate` | **nao** | `image_generate` |
| `media.stt` | **nao** | `transcribe` |
| `media.tts` | **nao** | `speak` |
| `media.wake` | **nao** | `wake_word` |
| `memory.read` | sim | `memory_search` |
| `memory.write` | sim | `memory_save` |
| `net.diagnose` | **nao** | `network` |
| `net.lan` | **nao** | `network_lan` |
| `session.read` | sim | `session_search`, `daily_summary` |
| `shell.exec` | sim | `shell`, `shell_bg`, `rust_project`, `python_repl`, `python_project`, `test_list`, `test_run`, `debug`, `project_task` |
| `site.publish` | sim | `canvas`, `publish_site` |
| `skill.read` | sim | `skill_load` |
| `system.admin` | **nao** | `linux_system_admin` |
| `system.read` | **nao** | `linux_system` |
| `team.delegate` | sim | `team_delegate` |
| `team.read` | sim | `team_list` |
| `weather.read` | **nao** | `weather` |
| `web.browse` | sim | `browser_open`, `browser_read`, `browser_click`, `browser_type`, `browser_screenshot` |
| `web.research` | sim | `deep_research` |
| `web.search` | sim | `web_search` |

<details><summary>Descricao de cada uma (a que o modelo le)</summary>

| Ferramenta | Capacidade | Descricao |
|---|---|---|
| `write_file` | `fs.write` | Create or overwrite a text file (markdown, html, csv, code...) in the task working directory. |
| `read_file` | `fs.read` | Read a text file from the task working directory. With start_line/end_line (1-based, inclusive) returns only that range, each line prefixed by its number. |
| `list_files` | `fs.read` | List files in the task working directory with sizes. |
| `edit_file` | `fs.write` | Replace one exact occurrence of old_text with new_text in a file of the task directory. old_text must appear exactly once (include surrounding lines to make it unique). Empty old_text appends new_text at the end of the file. |
| `design_erp_ui` | `doc.write` | Generate ERP screens (list, form, master-detail with totals, lookups) from SQL CREATE TABLE statements. Give the DDL in 'sql' or a file of the task in 'sql_path'. Writes <folder>/ui-ir.json and <folder>/index.html; publish the folder with publish_site to show it. |
| `screenshot_to_erp_ui` | `doc.write` | Rebuild ERP screens from a SCREENSHOT (png/jpg in the task folder) of an existing system: reads the field labels (OCR + local vision model), writes <folder>/tela.sql and the same outputs as design_erp_ui. Labels not actually written on the screenshot are discarded. |
| `shell` | `shell.exec` | Run a shell command (sh -c) in the task's isolated working directory (/work, persists between calls; python3, coreutils available; no network unless granted). Returns exit code, stdout and stderr. |
| `shell_bg` | `shell.exec` | Background shell process in the task sandbox (/work). action=start with 'command' returns an id; action=status with 'id' returns running/exit code and the output tail; action=stop with 'id' kills it; action=list shows all. Use for servers or long jobs. |
| `web_search` | `web.search` | Search the web. Returns titles, URLs and snippets. Then open promising URLs with browser_open. |
| `browser_open` | `web.browse` | Open a URL in the headless browser and return the page as readable text with links. |
| `browser_read` | `web.browse` | Read the current browser page again (after clicking or typing). |
| `browser_click` | `web.browse` | Click the element matching a CSS selector in the current page and wait for navigation. |
| `browser_type` | `web.browse` | Type text into the input matching a CSS selector; optionally press Enter to submit. |
| `browser_screenshot` | `web.browse` | Save a PNG screenshot of the current page into the task directory. |
| `image_render` | `fs.write` | Render an .svg or .html file of the task folder to a PNG with headless Chromium. The page sees only the task folder (no network, no other local files). Default size: the SVG's own width/height or viewBox, 1280x800 for HTML. |
| `deep_research` | `web.research` | Research a question on the web: plans queries, searches, reads the pages and answers with citations. Each citation is checked as a LITERAL passage of a page actually read; invented citations are rejected and reported. |
| `ocr` | `fs.read` | Extract the text of an image (.png .jpg .jpeg .tif .tiff) or of a .pdf in the task folder by OCR (tesseract; PDF pages rendered at 200 dpi). Default language por+eng. |
| `image` | `fs.read` | Information about an image in the task folder: width, height and color of a .png/.jpg/.jpeg, or validate an .svg (well-formed XML with an <svg> root) and read its width/height/viewBox. To convert SVG or HTML to PNG use image_render. |
| `transcribe` | `media.stt` | Speech to text (whisper.cpp) of a .wav audio file in the task folder (16 kHz mono works best). Optional 'language' (e.g. en, pt, auto). |
| `speak` | `media.tts` | Text to speech: synthesize 'text' into a .wav file in the task folder (default fala.wav) with the configured local voice. |
| `wake_word` | `media.wake` | Detect wake words / keywords (English) in a mono 16-bit .wav of the task folder, streamed through a local keyword spotter. 'keywords': up to 32 phrases of up to 64 characters (letters, space, apostrophe, hyphen). Returns each detection with its time. |
| `image_generate` | `media.generate` | Generate a .png image in the task folder. From 'prompt' via no image server configured: pass 'svg' markup; or, always available, from 'svg' markup you write, drawn by headless Chromium. |
| `desktop` | `desktop.control` | Control the real desktop session (mouse, keyboard, screen). action=move {x,y}; click {x?,y?,button?=left,double?}; scroll {dx?,dy}; key {key,state?=click} (enter, tab, esc, f1.., or one character); hotkey {keys:["ctrl","s"]}; type {text}; position; screenshot {path?=tela.png} saves a PNG in the task folder. |
| `create_document` | `doc.write` | Create a Word .docx file. blocks: [{type:'heading',level:1,text}, {type:'paragraph',text,bold?}, {type:'bullets',items:[..]}, {type:'table',header:[..],rows:[[..]]}]. |
| `create_spreadsheet` | `doc.write` | Create an Excel .xlsx file. Each sheet has rows of plain values: numbers stay numbers, strings starting with '=' are formulas (e.g. '=SUM(B2:B4)'), null is an empty cell. |
| `create_presentation` | `doc.write` | Create a PowerPoint .pptx file with a title slide and content slides (title + bullets + optional speaker notes). |
| `read_document` | `fs.read` | Read a document of the task directory as text: .docx, .pptx (slides and notes), .pdf, .xlsx (cell values), and plain text (.txt .md .csv .html .css .js .json .xml .svg; .html also comes without markup). Long output is truncated. |
| `zip_list` | `fs.read` | List the entries of a .zip in the task directory (name, size, compressed size) without extracting. |
| `zip` | `fs.write` | Extract a .zip into a folder of the task directory (action=extract, path, dest), or create a .zip from files/folders of the task directory (action=create, path, files). Entries with '..', absolute paths or links are refused. |
| `data_file` | `fs.read` | Work with a .json or .xml file of the task directory. action=validate reports errors with line/column; action=format returns it pretty-printed (use data_file_format to save); action=query returns the value at 'query': a JSON Pointer (RFC 6901, e.g. /items/0/name) for JSON, or an element path from the root (e.g. catalog/book/title, '*' matches any name, a final '@attr' reads an attribute) for XML. |
| `data_file_format` | `fs.write` | Validate and pretty-print a .json or .xml file of the task directory and save it (in place, or to 'output'). Key order and numbers are kept as written. |
| `pdf` | `fs.read` | Read a .pdf of the task directory. action=info returns page count, title and metadata; action=text returns the text (layout kept), optionally only pages first_page..last_page. |
| `pdf_create` | `fs.write` | Convert a .docx, .odt, .html or .txt of the task directory to PDF with LibreOffice (no network: remote images in HTML are not fetched). |
| `canvas` | `site.publish` | Create or replace an interactive HTML widget shown to the user in an isolated frame, and get its URL. 'html' is the body markup, 'css' the styles, 'script' the JavaScript (checked for syntax errors; only this script runs, inline handlers like onclick= and <script> tags inside 'html' are blocked; no network access, no cookies). |
| `publish_site` | `site.publish` | Publish a folder of the task directory that contains index.html as a website and get its URL. Write the files first (write_file), e.g. site/index.html and site/style.css. |
| `linux_system` | `system.read` | Inspect this Linux machine (read only). action=services (systemd units), service_status (unit), journal (lines, since, unit, priority), processes (sort=cpu\|mem, filter, limit), panel (item=all\|hostname\|time\|locale\|uptime\|kernel\|memory\|disk\|block), packages (filter, limit), applications (installed .desktop apps, filter). |
| `linux_system_admin` | `system.admin` | Change this Linux machine. action=service with operation (start\|stop\|restart\|enable\|disable) and unit; action=kill with pid and optional signal (TERM default, KILL, HUP, INT). Read-only inspection is the linux_system tool. |
| `network` | `net.diagnose` | Network diagnosis of this machine. action=interfaces, routes, listening (open TCP/UDP ports with pid), resolve (host), tcp_probe (host, port, timeout_ms; public destinations allowed by the egress policy), ping (host, count), traceroute (host). Loopback/private destinations are the network_lan tool. |
| `network_lan` | `net.lan` | Network probes to loopback and private-network destinations only. action=tcp_probe (host, port, timeout_ms) returns open/refused and the connect time; ping (host, count); traceroute (host). |
| `rust_project` | `shell.exec` | Run cargo on a Rust project inside the task directory (isolated sandbox, offline). action=check\|build\|test\|clippy\|fmt (fmt only checks unless fix=true); path is the project directory relative to the task directory (default '.'). Returns structured diagnostics: file, line, column, level, message, code. |
| `python_repl` | `shell.exec` | Persistent Python interpreter for this task (isolated sandbox, offline, cwd /work = task directory). Variables, functions and imports survive between calls, like a notebook. Returns stdout, stderr, the repr of the last expression and the traceback if any. reset=true starts a fresh interpreter. |
| `python_project` | `shell.exec` | Work on a Python project inside the task directory (isolated sandbox, offline). Creates/uses <path>/.venv with uv and installs the dependencies of requirements.txt / pyproject.toml from the local cache. action=setup\|test\|lint\|typecheck\|run: test=pytest, lint=ruff check (fix=true applies safe fixes), typecheck=mypy, run=execute `script` with `args`. path is the project directory relative to the task directory (default '.'); target narrows test/lint/typecheck to a file, directory or pytest node id. Returns structured diagnostics: file, line, column, level, message, code (for pytest the code is the failing test id). |
| `test_list` | `shell.exec` | List the tests of a project in the task directory as a tree (Rust: crate/module/test from `cargo test -- --list`; Python: file/test from `pytest --collect-only`). path is the project directory (default '.'); language is guessed from Cargo.toml / pyproject.toml. Nodes are what test_run accepts. |
| `test_run` | `shell.exec` | Run ONE node of the test tree (see test_list): Rust `CRATE`, `CRATE/module` or `CRATE/module::test`; Python a pytest node id (`tests/test_x.py` or `tests/test_x.py::test_name`). Same sandbox and same cargo/pytest as rust_project and python_project. Returns passed/failed counts and the result lines. |
| `debug` | `shell.exec` | Debug a program of the task directory with a real debugger (DAP) inside the sandbox. Adapters on this host: rust; missing: python: debugpy ausente em /opt/phxclaw-python/lib/python3.14/site-packages (uv pip install debugpy no hospedeiro). action=start (program, optional args, language, breakpoints=[{file,line}]; runs until the first breakpoint or the end), breakpoint (file, line: adds one and reports whether it was verified), continue (runs to the next stop), stack (frames of the stopped thread), variables (locals of a frame; frame=0 is the top), evaluate (expression in the frame -- the debug console; console=true sends it to the adapter's own command line instead), stop. Returns JSON with the state (stopped reason, program output). |
| `calculator` | `calc` | Evaluate an arithmetic expression exactly as written: + - * / % ^ (or **), parentheses, pi, e, and sqrt abs ln log log2 exp sin cos tan asin acos atan floor ceil round trunc min max pow. Use it instead of doing arithmetic in your head. |
| `http_request` | `http.request` | Generic HTTP request (GET/POST/PUT/PATCH/DELETE/HEAD) with headers, query, JSON/form/text body, timeout and size limit. Authentication only by the NAME of a credential declared by the operator ('credencial'); never put a secret in headers, query or body. Supports pagination (cursor field, next URL field, or Link rel=next) and batching. Returns the response items as a JSON array. Internal network addresses are refused unless the operator allowed them. |
| `weather` | `weather.read` | Previsao do tempo para uma coordenada (MET Norway, CC BY 4.0): temperatura, vento, umidade, chuva e simbolo hora a hora. Cite a atribuicao que vem na resposta. |
| `session_search` | `session.read` | Full-text search over previous tasks of this agent (objective, plan, answer and step summaries). Returns the best matches with id, date, status and a snippet. Use it to recall what was done or found before. |
| `daily_summary` | `session.read` | Summary of all tasks created on a day (UTC): counts by status, tokens, and one line per task with its outcome and files. date: YYYY-MM-DD, 'today' (default) or 'yesterday'. |
| `memory_save` | `memory.write` | Save a short note that future tasks should know (a user preference, a fact learned, where something is). Future tasks receive the most relevant notes automatically. Never save passwords or keys: they are redacted. If the note contradicts an existing one, pass its id in `substitui`: the old note is kept but marked invalid, never deleted. |
| `memory_search` | `memory.read` | Search notes saved by previous tasks, by words. Each hit comes with its id (for memory_save `substitui`). Notes replaced by newer ones are hidden unless include_invalid is true. |
| `skill_load` | `skill.read` | Load the full instructions of a skill listed in the system prompt, by name. |
| `glob` | `fs.read` | Find files by name pattern in the task directory, skipping what .gitignore ignores. pattern: '*.rs' (any folder), 'src/**/*.{ts,tsx}' (path). Newest first. Optional path (subfolder), limit, include_ignored. |
| `grep` | `fs.read` | Search file contents with a regex in the task directory, skipping .gitignore'd and binary files. output_mode: content (default; file, line, text, with 'context' lines around), files (only names), count. Optional glob ('*.rs'), path, case_insensitive, limit. |
| `replace_in_project` | `fs.write` | Replace a pattern in every matching file of the task directory (skips .gitignore'd and binary files). pattern is literal unless regex=true (then replacement may use $1 groups). Optional glob ('*.rs'), path (subfolder), case_insensitive. confirm=false (default) only previews: files and count per file, nothing is written. confirm=true writes; a checkpoint is taken before. |
| `file_history` | `fs.read` | Local save history of a file in the task directory (every save seen by the IDE poller, newest last). action=list {path} gives the versions (rowstamp, bytes); action=show {path, version} returns that version's text. Restore is file_history_restore. |
| `file_history_restore` | `fs.write` | Restore a file of the task directory to a saved version from its local save history (.phxclaw/historico). {path, version} where version is a rowstamp from file_history list. The current content is saved to the history first, so it can be undone. |
| `notebook_read` | `fs.read` | Read a Jupyter notebook (.ipynb) of the task directory as cells: index, id, type, source and a summary of the outputs (text, errors, image types). |
| `notebook_edit` | `fs.write` | Edit a Jupyter notebook (.ipynb) by cell. action=replace {index or cell_id, source, cell_type?} (code cells lose stale outputs); insert {index (position, = cell count appends), source, cell_type: code\|markdown\|raw}; delete {index or cell_id}. |
| `checkpoint_list` | `fs.read` | List the restore points of the task working directory (taken before each file write), newest last, with what changed since each one. |
| `checkpoint_restore` | `fs.write` | Restore the task working directory to a checkpoint (one is taken automatically before every file write). Give 'id' from checkpoint_list. Files created after the checkpoint are kept unless remove_new=true. The restore itself is checkpointed, so it can be undone. |
| `code_review` | `code.review` | Review a code diff and return structured findings (file, line, severity critica\|alta\|media\|baixa\|info, finding, suggestion). Either 'diff' (unified diff text, e.g. from github/gitlab pr_diff) or a git repo in the task directory: path?, rev? (e.g. main..HEAD), cached?. Findings outside the diff are discarded and counted. Optional focus. |
| `git` | `git.read` | Read a git repository in the task directory, structured JSON. action: status; diff {rev?, cached?, paths?} (files and hunks); log {rev?, limit?, paths?}; show {rev}; blame {file, start_line?, end_line?, rev?}; branches. 'path' selects the repo folder (default: task root). |
| `git_write` | `git.write` | Change a git repository in the task directory (sandboxed, no network). action: init {branch?}; add {paths}; add_trecho {file, hunk (index or list from git diff) \| start_line, end_line} stages only that part of the file; commit {message, all?}; checkout {branch, create?, base?}; branch_create {branch, base?}; branch_delete {branch, force?}; stash {op: push\|pop\|apply\|drop\|list, message?, include_untracked?, index?}; merge {branch} (never rebase/force); conflitos (lists <<<<<<< blocks with both sides); resolver {file, block, choice: nosso\|deles\|texto, text?}; abort. 'path' selects the repo folder (default: task root). |
| `project_task` | `shell.exec` | Tasks the project declares in .phxclaw/tarefas.json ([{nome, comando, args?, cwd?, grupo: build\|test\|run}]). action=list shows them; action=run {name} runs one in the task sandbox (no network) and returns exit code, stdout and stderr. Without the file, use rust_project / python_project. 'path' selects the project folder (default: task root). |
| `git_worktree` | `git.write` | Isolated git worktrees for parallel tasks. action=add {name, branch?, base?} creates <repo>/.worktrees/<name> on branch phxclaw/<name> (returns its path, usable as 'path' in git/git_write and file tools); action=list; action=remove {name, force?}. 'path' selects the main repo. |
| `go_no_go` | `gonogo.write` | Integrators' council Go/NoGo. action: open {integration, integrators} declares the council (fixed afterwards); record {integration, integrator, verdict: OK\|NOGO, errors? (required for NOGO)} -- the integrator must be in the council, and once a name is recorded only the same task can record for it again; status {integration}. Decision: any current NOGO -> NOGO; a council member without verdict -> WAIT; all OK -> GO. |
| `lsp` | `fs.read` | Read-only language server queries on files in the task directory (rust (.rs), python (.py, .pyi)). action=definition\|references\|hover need path, line and column (1-based); action=symbols with path lists the file's symbols, with query searches the workspace; action=diagnostics returns the file's errors and warnings. Never edits files. |
| `team_list` | `team.read` | List the 111 PhxClaw team roles (id \| name \| macro-area \| type \| criticality \| main capability \| when to call). Filter with 'macroarea' and/or 'text'; pass 'id' (number or name) to get one role in full. Use before team_delegate to pick the right role. |
| `team_delegate` | `team.delegate` | Delegate one self-contained sub-task to a PhxClaw team role (by id or name, see team_list). The role runs as a sub-agent with its own mission and limits and only the tools both it and you are allowed; returns its answer. Human roles do not run: they come back asking for a human decision. |
| `parallel_tasks` | `agent.parallel` | Run up to 6 coding tasks in parallel over a git repo in the working directory. Each task gets its own git worktree (branch phxclaw/<name>) as an isolated sandboxed workspace and a sub-agent; when all finish, each worktree's changes are committed on its branch. Returns per task: answer, branch, commit and changed files. 'attempts' (best-of-N, up to 4) runs the same objective N times in separate worktrees (<name>-1..N) so you can compare and keep the best. Review/merge the branches afterwards with git/git_write. |
| `parallel_research` | `agent.spawn` | Run up to 6 independent sub-agents in parallel, one per sub-task, and return each answer. Use for research over many items (compare products, gather facts about several topics). |
| `fluxo` | `flow.run` | Run a saved PhxClaw flow (DAG, JSON file) as a sub-flow and return the items produced by its last step as a JSON array. Give 'caminho' (path to the .json inside the task folder) or 'nome' (file in the agent's flows folder). 'entrada' are the input items the flow reads as {{entrada}}; 'modo' 'once' (default) runs the flow once with the whole list, 'each' runs it once per item. Every step of the sub-flow goes through the same policy as your own tool calls. |

</details>
<!-- gerado:ferramentas:fim -->

### Existem no fonte, não montadas nesta máquina

<!-- gerado:condicionais:inicio -->
Medido em 2026-10-09: **15 ferramentas existem no fonte e nao montaram nesta maquina** (dependem de configuracao, token, canal ou feature de compilacao; a condicao de cada uma esta em `crates/phxclaw-agent/src/montagem.rs`).

| Ferramenta | Definida em |
|---|---|
| `channel_send` | `crates/phxclaw-agent/src/canais/mod.rs` |
| `doc_search` | `crates/phxclaw-agent/src/documentos.rs` |
| `github` | `crates/phxclaw-agent/src/forja.rs` |
| `github_write` | `crates/phxclaw-agent/src/forja.rs` |
| `gitlab` | `crates/phxclaw-agent/src/forja.rs` |
| `gitlab_write` | `crates/phxclaw-agent/src/forja.rs` |
| `n8n_workflow` | `crates/phxclaw-agent/src/n8n.rs` |
| `node_invoke` | `crates/phxclaw-agent/src/dispositivos.rs` |
| `node_list` | `crates/phxclaw-agent/src/dispositivos.rs` |
| `plugin_catalog` | `crates/phxclaw-agent/src/loja.rs` |
| `postgres` | `crates/phxclaw-agent/src/sistema.rs` |
| `postgres_write` | `crates/phxclaw-agent/src/sistema.rs` |
| `send_email` | `crates/phxclaw-agent/src/email.rs` |
| `voice_list` | `crates/phxclaw-agent/src/elevenlabs.rs` |
| `x_search` | `crates/phxclaw-agent/src/xai.rs` |

Montadas sem nome literal no fonte (MCP externo ou nome dinamico): `browser_click`, `browser_open`, `browser_read`, `browser_screenshot`, `browser_type`.
<!-- gerado:condicionais:fim -->

`PHXCLAW_CAPACIDADES=web.search,fs.read,...` troca a lista. SMTP: `PHXCLAW_SMTP_HOST`,
`_PORT`, `_SECURITY` (tls|starttls|plain-só-loopback), `_USER`, `_PASSWORD`, `PHXCLAW_EMAIL_FROM`.

## Linha de comando

A referência é a ajuda do próprio binário, copiada aqui pelo gerador (os `CLI_REFERENCE_V0xx`
antigos não cobrem o agente).

<!-- gerado:cli:inicio -->
Saida de `phxclaw --help`, gerada em 2026-10-09:

```text
PhxClaw 0.70.0

USO:
  phxclaw <COMANDO> [opcoes]
  phxclaw ajuda <COMANDO>

AGENTE:
  agente        Roda o agente agora, mostrando cada passo
  voz           Conversa por voz, um turno por arquivo WAV
  sessoes       Busca nas tarefas anteriores
  resumo        Resumo das tarefas do dia
  estilos       Lista os estilos de saida
  skills        Importa SKILL.md de outros agentes (scripts desligados, origem com SHA-256)
  indexar       Indexa uma pasta de documentos para o doc_search (BM25)
  projeto       Confia num projeto: os AGENTS.md dele entram no prompt

EQUIPE E FLUXOS:
  equipe        Os papeis da equipe: listar, mostrar, delegar
  gonogo        Conselho de integradores: abrir, registrar parecer, ver, decidir Go/NoGo
  fluxo         Fluxo em DAG: rodar, retomar, responder esperas, pinar, podar, exportar e listar
  agenda        Agenda: listar, adicionar (modelo ou fluxo) e disparar o que venceu

CODIGO:
  revisar       Revisao de codigo de um diff ou PR (serve para CI)
  tarefa        Tarefas do projeto (.phxclaw/tarefas.json): listar e rodar
  testes        Explorador de testes: a arvore e um no dela (Rust e Python)

SERVICOS (API, CANAIS, EDITORES, DISPOSITIVOS):
  servir        API de tarefas, UI web (PWA), gatilhos e heartbeat
  canal         Canal de mensagens como entrada do agente (25 canais)
  mcp-serve     Ferramentas do agente como servidor MCP (stdio)
  acp           Agent Client Protocol para editores (stdio)
  dispositivos  Servidor WSS de dispositivos pareados
  ponte         Ponte de controle remoto (o agente se liga para fora)

CREDENCIAIS (vao para o SecretBroker, nunca para arquivo):
  forja         Guarda o token do GitHub ou do GitLab
  mcp           Credencial de um servidor MCP remoto (Bearer ou OAuth)
  credencial    Segredo de uma credencial nomeada do no HTTP (http_request)
  elevenlabs    Guarda a chave da ElevenLabs ou lista as vozes da conta
  gemini        Guarda a chave da Gemini API (Nano Banana no image_generate)
  xai           Guarda a chave da xAI (habilita x_search)
  n8n           Guarda a chave da API e o segredo do webhook do n8n (habilita n8n_workflow)
  api           Guarda o Bearer da API de tarefas
  usuario       Usuarios, projetos e papeis da API (token so como hash)
  openai        Guarda a chave da OpenAI (modelos openai:*)
  anthropic     Guarda a chave da Anthropic (modelos anthropic:*)
  imagem        Guarda a chave do gerador de imagem openai
  email         Guarda a senha do SMTP (send_email e canal de e-mail)
  plugins       Semente de assinatura, reassinar manifestos, loja: catalogo, instalar, empacotar

MEDICAO (so numero medido, com faixa min-max, N e data):
  repetir       Repete uma gravacao sem modelo e acusa a divergencia com o passo
  medir         Soma uma gravacao por tarefa: chamadas, duracao e tokens de cada passo
  avaliar       Compara modelos pelo agente: p50/p95, tokens/s, CPU, energia, acerto e nota
  ui            Prova as telas geradas: ida e volta (fidelidade) e larguras (responsivo)
  skill         Otimiza uma skill por A/B medido; so promove sem cruzar faixas

DIAGNOSTICO:
  ferramentas   Ferramentas montadas nesta maquina, em JSON
  core          Estado medido do runtime
  db            Plano de instalacao do PostgreSQL
  config        O config.json: valor efetivo e origem de cada chave, validar, definir
  version       Versao

MODELOS: ollama:<modelo> (local), openai:<modelo>, anthropic:<modelo>, gemini:<modelo>
         (chaves de OPENAI_API_KEY / ANTHROPIC_API_KEY / GEMINI_API_KEY)
POLITICA: PHXCLAW_CAPACIDADES=web.search,web.browse,fs.read,fs.write,... (padrao: CAPACIDADES_PADRAO)
```
<!-- gerado:cli:fim -->

## Python e Rust dentro da tarefa

- **`python_project`** (`shell.exec`): `setup` (venv em `<path>/.venv` com `uv`, dependências
  de `requirements.txt`/`pyproject.toml` só do cache local), `test` (pytest), `lint` (ruff;
  `fix=true` aplica as correções seguras), `typecheck` (mypy), `run`. Devolve diagnóstico
  estruturado (arquivo, linha, coluna, nível, mensagem, código). Roda no **mesmo bwrap** do
  `shell`, sem rede; o cache do hospedeiro entra **só leitura** (o `uv` grava as marcas dele no
  `/tmp` do sandbox). pytest, ruff e mypy vêm do interpretador de `PHXCLAW_PYTHON` por um `.pth`;
  o que o projeto declara entra no venv dele e ganha. Sem interpretador, a ferramenta não se
  registra. O `python_repl` usa o mesmo interpretador. Por quê: `crates/phxclaw-agent/src/python.rs`.
- **`rust_project`** (`shell.exec`): cargo no mesmo sandbox, offline.

## SDKs e ponte nativa

| O quê | Onde | Fala com | Dependências |
|---|---|---|---|
| SDK Python | `sdk/python` (`from phxclaw import ClienteApi, ClienteMcp`) | API HTTP (`phxclaw servir`) e MCP por stdio (`phxclaw mcp-serve`) | só a biblioteca padrão |
| SDK Rust | `crates/phxclaw-sdk` (`Cliente`, `NovaTarefa`, `aguardar`) | API HTTP | `phxclaw-agent-core` (os tipos de fio são **os do servidor**, reexportados, nunca cópia), reqwest, serde, tokio |
| Ponte PyO3 | `sdk/python-nativo` (módulo `phxclaw_nativo`, build por maturin) | nada: função pura, sem rede e sem disco | `phxclaw-ui-ir`; expõe `ui_ir` (DDL SQL → UI-IR), o HTML da aplicação e a normalização tolerante a OCR |

- Token da API: argumento, `PHXCLAW_API_TOKEN`, ou o `api.token` da pasta (`PHXCLAW_HOME`).
- O `aguardar` do SDK Rust devolve também em `AwaitingApproval`/`AwaitingInput`: esperar por
  esses é esperar por uma pessoa, e quem chama decide. O `429` do limite de criação chega como
  `Erro::Api` com o `retry_after` do servidor.
- A ponte PyO3 não guarda regra nenhuma: cada função chama a do `phxclaw-ui-ir` e só traduz a
  borda. SQL vazio de tabelas reconhecidas é **recusado** com os avisos do analisador, não
  devolvido como app vazia; há teto de tamanho do SQL (constante `SQL_MAX` em `src/lib.rs`).
- Exemplo e testes: `cargo run -p phxclaw-sdk --example tarefa`; `sdk/python/README.md`
  (os testes sobem o binário de verdade com um Ollama de roteiro); `sdk/python-nativo/tests`.

## Equipe de papéis

<!-- gerado:equipe:inicio -->
**111 papeis** carregados de `config/agents` (medido em 2026-10-09 por `phxclaw equipe listar`).
<!-- gerado:equipe:fim -->

- **`team_list`** (`team.read`) lista os papéis (id, nome, macroárea, tipo, criticidade,
  capacidade principal); **`team_delegate`** (`team.delegate`) entrega uma subtarefa a um
  papel por id ou nome.
- O subagente do papel roda pelo **mesmo laço** do `parallel_research`, com as ferramentas
  restritas à **interseção** do que o papel autoriza com o que o pai tem: delegar nunca amplia
  poder, e o subagente de papel não herda ferramenta de subagente.
- Papéis que a planilha roteia para o Ollama usam `PHXCLAW_MODELO_LOCAL`; spec inválida vira
  aviso e o papel cai no modelo do pai. A pasta dos manifestos é `PHXCLAW_AGENTES_DIR` ou a
  primeira `config/agents` encontrada.
- CLI: `phxclaw equipe listar [--macroarea X] [--texto Y]`, `equipe mostrar ID`,
  `equipe delegar ID "tarefa" [--modelo M]` — as mesmas funções das ferramentas
  (`crates/phxclaw-agent/src/equipe.rs`). O `apps/phxclaw-ui/assets/equipe.json` sai de
  `cargo run -p phxclaw-agent --example equipe_json`.

## Código: git, busca, cadernos, pontos de restauração, forjas e revisão

| Ferramenta | Capacidade | O que vale saber |
|---|---|---|
| `git` | `git.read` | status, diff, log, blame… em JSON. **Não executa código do repositório**: ganchos para `/dev/null`, `fsmonitor`, diff externo e `textconv` desligados |
| `git_write` | `git.write` | init, add, commit… no sandbox, sem rede. O modelo nunca escreve a linha de comando: referência começando com `-` é recusada, caminho vai depois de `--` |
| `git_worktree` | `git.write` | worktrees isoladas para tarefas paralelas |
| `glob`, `grep` | `fs.read` | «onde está», pulando o que o `.gitignore` (e `.git/info/exclude`) ignora; o `list_files` segue sendo o inventário completo |
| `notebook_read`, `notebook_edit` | `fs.read`/`fs.write` | `.ipynb` por célula; célula de código editada perde saídas e `execution_count` (como o Jupyter) |
| `checkpoint_list`, `checkpoint_restore` | `fs.read`/`fs.write` | ponto de restauração **antes de toda escrita**, criado no portão do motor (não em cada ferramenta); mora em `<tarefa>/checkpoints`, fora do `/work`, onde o comando do modelo não alcança |
| `github`/`github_write`, `gitlab`/`gitlab_write` | `github.read`/`.write`, `gitlab.read`/`.write` (**fora do padrão**) | issues e PRs/MRs pela API REST. **Sem token a ferramenta não existe.** O token só entra por `phxclaw forja token github\|gitlab`, que o guarda no SecretBroker; `GITHUB_TOKEN`/`GH_TOKEN` do ambiente não são lidos. Base em `PHXCLAW_GITHUB_API`/`PHXCLAW_GITLAB_API`; sem seguir redirecionamento |
| `code_review` | `code.review` | diff revisado pelo modelo, achados com arquivo, linha e severidade. Achado em arquivo fora do diff, em linha fora de hunk ou com severidade fora da escala é **descartado e contado**; resposta que não é JSON ganha uma segunda chance e a segunda falha é erro, nunca «nada a apontar» |

`phxclaw revisar` é o `code_review` pela CLI (mesmo motor, `revisao::revisar_da_fonte`):
`--repo/--rev/--cached`, `--diff ARQ|-` ou `--pr github:dono/proj#7`; `--falhar-em alta`
sai com 1 nessa severidade ou pior, para CI. O diff do repo, do `git diff` e do PR passam pelo
mesmo analisador (`crates/phxclaw-agent/src/git.rs`).

## Terminal do IDE e Helix

- **`crates/phxclaw-terminal`**: PTY + emulador VT do `alacritty_terminal` (Apache-2.0), grade de
  células entregue **por diferença** (só as linhas que mudaram). Soltar o `Terminal` fecha o PTY e
  mata o grupo do filho mesmo que ele ignore o SIGHUP. Usado pelo desktop
  (`apps/phxclaw-desktop/src-tauri/src/terminal.rs`).
- Retrato da grade em JSON, no formato do evento `terminal_grade` do desktop (é ele que a prova
  de tela usa, em vez de grade inventada):
  `cargo run -p phxclaw-terminal --example retrato -- COLUNAS LINHAS ESPERA_MS ENTRADA PROGRAMA [ARGS...]`.
- **`tools/instalar_helix.sh`** compila o Helix (MPL-2.0) do fonte de uma tag fixada e **confere o
  commit** depois do clone, instalando em `/opt/helix`. Entra como **processo separado e sem
  modificação** — é o que mantém a MPL-2.0 longe do nosso Apache-2.0. Só as gramáticas que o IDE
  usa. O piso de disco do script é **estimativa declarada**, não medida.

## API (`phxclaw servir`)

Bearer em `var/agente/api.token` (0600) ou `PHXCLAW_API_TOKEN`. Só loopback por padrão.

| Método | Rota | O quê |
|---|---|---|
| POST | `/v1/tasks` | `{objective, model?, plan_first?, webhook?, verificar?, saida_esquema?}` → `{id}` |
| GET | `/v1/tasks`, `/v1/tasks/{id}` | lista / estado, passos, resposta, artefatos |
| POST | `/v1/tasks/{id}/plan`, `/approve` | Plan Mode: editar o plano e aprovar |
| POST | `/v1/tasks/{id}/cancel` | cancela |
| GET | `/v1/tasks/{id}/artifacts/{path}` | baixa artefato (CSP sandbox, nosniff) |
| POST/GET | `/v1/schedules` | `{name, objective, cron \| every_seconds>=60}` |
| GET | `/sites/{id}/{pasta}/` | site publicado pelo agente |

Webhook de fim de tarefa só para origens de `PHXCLAW_WEBHOOK_ORIGINS`. Criar tarefa gasta ficha de um
balde (`PHXCLAW_API_TAREFAS_POR_MINUTO`, padrão 10): além dele, `429` com `Retry-After`.

## Guardas do motor (cada uma nasceu de uma falha medida)

- teto de passos e de tokens; prazo por ferramenta que chega ao processo filho;
- terceira chamada idêntica não roda;
- o fim é a ferramenta `final_answer`; texto solto recebe até 2 lembretes;
- **conclusão verificada**: arquivo citado no objetivo tem de existir, senão a tarefa termina
  `failed` dizendo qual falta.
- **argumento validado no portão** (SP000028): antes de a ferramenta rodar, o portão único
  (`call_tool_com`, o mesmo do `mcp-serve` e dos fluxos) confere os argumentos contra o esquema
  dela com o validador próprio (`src/esquema.rs`, sem crate: `type`, `properties`, `required`,
  `enum`, `items`, `minimum`, `maximum`, `pattern`; palavra desconhecida de esquema MCP passa
  com nota). Volta ao modelo **todos** os erros em JSON, cada um com o caminho
  (`blocks[0].level`), o que veio e o esperado. Coerção só do que nenhuma ferramenta lia
  diferente: `"5"` → 5 onde o esquema pede inteiro, `"true"` → booleano; `null` em campo
  opcional é ausência;
- **orçamento de argumento**: 2 novas tentativas **seguidas** por ferramenta
  (`agente.tentativas_argumento`); a chamada que passa zera a conta, como no PydanticAI.
  Esgotou, a tarefa termina `failed` dizendo a ferramenta e o último erro;
- **fim conferido**: a tarefa não fecha `completed` com chamada de argumento inválido sem
  conserto. Decisão: a resposta final é **recusada e devolvida ao modelo** (como as outras
  conferências do fim) até o limite de recusas; esgotado, `failed` com o motivo. O `concluir`
  confere de novo, como última porta. O mesmo vale para o fim em texto;
- **comando de verificação** (`--verificar "cmd"` na CLI, `verificar` na API): roda no mesmo
  executor do hook `Stop` (bwrap, sem rede, a pasta da tarefa em `/work`); código ≠ 0 recusa o
  fim. Pede `shell.exec` — sem ela a API recusa na entrada e o motor falha no começo, para o
  verificar não virar a porta lateral do shell negado;
- **saída tipada** (`--saida-esquema arq.json`, `saida_esquema` na API): o `final_answer` pede
  o objeto do esquema, e o mesmo validador o confere; o texto que contém o JSON é aceito.
- **medido** (01/10/2026, qwen2.5:1.5b, 5 casos de documento/planilha/apresentação/cálculo ×
  3 rodadas, `PHXCLAW_CAPACIDADES=fs.read,fs.write,doc.write,calc`): argumento inválido por
  rodada **0,25–0,71** antes e **0,43–0,62** depois (faixas se cruzam: sem vencedor); acerto por
  rodada **0,20–0,20** antes e **0,20–0,60** depois (encostam: sem vencedor). O que mudou fora da
  faixa: chamadas inválidas **corrigidas** pelo modelo na tentativa seguinte, **0 de 5** tarefas
  antes e **3 de 8** depois (1 por rodada, nas 3) — e só depois de o erro de campo ausente levar a
  `description` dele (sem ela, medido no meio: **0 de 10**). Antes, 4 das 15 execuções caíram no
  prazo de 300 s do provedor (CPU disputada); depois, nenhuma — confusor declarado.
- `create_spreadsheet` aceita linha-objeto (`{"Item": 1, "Qtd": 5}`, o formato do 1.5b): as
  chaves viram o cabeçalho (ordem alfabética do `serde_json`). Antes ela virava linha vazia e a
  planilha saía «criada» sem dado — 2 dos 3 acertos medidos antes eram planilha sem dado.

## O que está provado e o que não está

- Provado no fio: os 4 provedores (formato, auth, chamada de ferramenta) contra servidor
  local; Ollama real; Chromium real; DuckDuckGo real; SMTP contra servidor local; .docx/.xlsx/
  .pptx abertos por leitores independentes e pelo LibreOffice; API HTTP; agenda.
- Ponta a ponta com modelo local de 3B: busca real → `rust.xlsx` gravado. **O conteúdo saiu
  errado** (limite do modelo); a tarefa do site falhou e o status diz `failed`.
- **Não provado**: nuvem real (sem chaves), e-mail num servidor real, canais Telegram/Slack como
  entrada, tela de tarefas no Command Center.
- **Onda de código, Python, equipe, SDKs e terminal**: os testes existem —
  `crates/phxclaw-agent/tests/{codigo,python,equipe}.rs`, `apps/phxclaw/tests/{revisar,equipe}.rs`,
  `crates/phxclaw-sdk/tests/cliente.rs`, `crates/phxclaw-terminal/tests/contra_o_so.rs`,
  `sdk/python/tests`, `sdk/python-nativo/tests` —, mas esta página **não** os rodou e não
  publica resultado deles. **Não provado**: `github`/`gitlab` contra a API real (sem token
  guardado nesta máquina, as ferramentas nem montam; ver a tabela acima), Helix compilado e
  rodando neste ambiente a partir do script.
