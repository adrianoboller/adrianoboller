# F08 — PhxClaw UI Shell

## Splash

Arquivo principal:

`apps/phxclaw-ui/index.html`

A splash mostra a sequência visual:

1. Microkernel;
2. Research Core;
3. Hypothesis Core;
4. Installer Core;
5. Plugin Registry;
6. Agent Runtime;
7. Task Graph;
8. Model Gateway.

Ao concluir, abre o Command Center.

## Command Center

A primeira tela apresenta:

- status do Kernel;
- Task Graph;
- Model Gateway;
- segurança dos plugins;
- Mission Pipeline;
- tarefas e approval gate;
- agentes essenciais;
- Model Router.

## Executar localmente

```bash
python3 scripts/serve_ui.py --port 8080
```

Abrir:

`http://127.0.0.1:8080/`

Para desenvolvimento visual:

- `?screen=splash`
- `?screen=dashboard`

## Integração futura

A UI não acessará PostgreSQL diretamente. A integração prevista é por API + SSE/WebSocket, com events provenientes do backend Rust.
