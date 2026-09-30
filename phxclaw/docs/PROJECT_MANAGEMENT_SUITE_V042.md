# PhxClaw v0.42 — Project Management Suite

Integração completa do módulo de Gerência de Projetos ao PhxClaw.

## Escopo

- **77 skills especializadas** definidas pelo usuário.
- **2 adapters opcionais**: Google Calendar e Google Drive.
- **79 skills/adapters** registrados.
- **90 capabilities novas**; total projetado **892**.
- Orquestração híbrida: `MS Project/WBS/Gantt → Product Backlog → Sprint → Kanban → execução → métricas → PDCA → correções → nova Sprint → atualização de cronograma`.
- Fonte canônica de configuração permanece `config/phxclaw.config.json`; o overlay aplica a seção `project_management`.
- Segredos nunca entram no JSON; somente UUIDs/referências do Secret Broker.

## Gate de verdade

Esta entrega comprova source/static e testes locais do overlay. `cargo`, PostgreSQL E2E, Google Calendar/Drive reais e Microsoft Project real continuam gates nativos.
