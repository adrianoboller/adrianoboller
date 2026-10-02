# n8n-nodes-phxclaw

Nó da comunidade do [n8n](https://n8n.io) para o PhxClaw: cria tarefas do agente,
espera o resultado, responde perguntas, aprova planos e roda ferramentas, pela API de
tarefas (`/v1/tasks`) e pelo endpoint MCP (`POST /mcp`) do `phxclaw servir`.

| Operação         | Rota do PhxClaw                                   |
| ---------------- | ------------------------------------------------- |
| Create Task      | `POST /v1/tasks` (+ espera opcional)              |
| Wait for Result  | `GET /v1/tasks/{id}` até estado final             |
| Answer Question  | `POST /v1/tasks/{id}/answer`                      |
| Approve Plan     | `POST /v1/tasks/{id}/approve`                     |
| Run Tool         | `tools/call` em `POST /mcp` (streamable HTTP)     |

Credencial `PhxClaw API`: URL base do `phxclaw servir` e o Bearer da API
(`phxclaw api chave`).

## Instalar

- Pela tela: *Settings > Community Nodes > Install* com `n8n-nodes-phxclaw`.
- Manual (self-hosted): `cd ~/.n8n/nodes && npm i n8n-nodes-phxclaw` e reinicie.
- Em desenvolvimento: `npm install && npm run build` aqui e
  `N8N_CUSTOM_EXTENSIONS=<esta pasta> n8n start`.

Como o n8n documenta a instalação, o licenciamento e o outro sentido (PhxClaw chamando o
n8n) está em `phxclaw/docs/N8N.md`.

## Licença

Apache-2.0 (o pacote). O `n8n-workflow` entra só como dependência de desenvolvimento
(tipos): o nó não embute o n8n.
