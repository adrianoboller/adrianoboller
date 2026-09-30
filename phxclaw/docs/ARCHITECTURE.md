# PhxClaw Architecture v0.8

## 1. Princípio central

PhxClaw é um **microkernel Rust + serviços de plataforma + plugins declarativos JSON**. O core não cresce conforme aparecem novos agentes, ferramentas, modelos, canais, UI panels, MCPs ou integrações.

### Core nativo obrigatório

- `Research Core`;
- `Hypothesis Core`;
- `Installer Core`.

## 2. Arquitetura operacional v0.6

```text
Human / API / Channel
        |
        v
Master Orchestrator
        |
        v
Task Graph
        |
        v
Agent Runtime
        |
        +---------------------+
        |                     |
        v                     v
Model Gateway           Capability Fabric
                              |
              +---------------+----------------------+
              |               |                      |
              v               v                      v
            HTTP/PG         Documents              Desktop Host
            Ollama          OCR/Voice             Tauri/WRY
                                                       |
                                                       v
                                                LiveEventHub
                                             /       |       \
                                            v        v        v
                                       Tauri UI     SSE      WS
                                            |
                                            v
                                      Evidence Ledger
                                      UUIDv7/SHA-256
```


## 2A. Research Context Pipeline v0.8

```text
ResearchTask UUIDv7
        |
        v
Agent Catalog (110 manifests)
        |
        v
Logical Agent Runtime
        |
        v
Lazy Skill Resolver
        |
        v
Source Registry
   +----+----------------+
   |                     |
   v                     v
offline rust-docs    official online plan
   |                     |
   +----------+----------+
              v
      Context Compiler
              |
              v
     ResearchRecord/model_context
              |
      +-------+--------+
      v                v
 Live Event Bus    Evidence Ledger
```

O pipeline usa o mesmo `correlation_uuid` para roteamento, skill, fonte, contexto e evidência. Conteúdo online não é baixado silenciosamente; o Source Registry seleciona o endpoint e a execução de rede passa pelo Egress Broker.

## 3. Durable vs realtime

PhxClaw separa explicitamente persistência de entrega ao vivo.

```text
Transactional Outbox (PostgreSQL)
= durabilidade, retries, publicação oficial

LiveEventHub (tokio broadcast)
= baixa latência dentro de um Desktop Host
```

Eventos possuem UUIDv7, correlation UUID e causation UUID. O realtime pode perder frames sob backpressure; o cliente recupera estado via replay/snapshot/outbox.

## 4. Desktop Host

O Desktop Host não entra no microkernel. Ele é o boundary privilegiado entre o PhxClaw e o sistema operacional.

Responsabilidades:

- Tauri/WRY;
- app/window lifecycle;
- execução de comandos sob policy;
- lançamento de aplicações;
- teclado/mouse;
- captura de tela;
- bridge WebView/DOM;
- API loopback;
- live event stream;
- evidence ledger.

### Política

Tudo sensível começa desligado.

```text
command_execution=false
shell_execution=false
desktop_input=false
screen_capture=false
webview_control=false
external_webviews=false
api_host_control=false
```

## 5. Desktop action protocol

`phxclaw-desktop-v1` define:

- request UUIDv7;
- correlation UUID;
- actor;
- capability;
- action tipada;
- result UUIDv7;
- status;
- evidence UUID.

Ações:

```text
launch_application
execute_command
input
capture_screen
web_view
```

Agentes não chamam APIs do SO diretamente. Eles publicam uma intenção tipada que é validada no host.

## 6. WebView boundary

### Trusted local WebView

A janela `main` recebe a capability Tauri do Command Center e executa operações DOM tipadas, retornando resultado estruturado.

### Managed remote WebView

Conteúdo remoto:

- exige origem allowlisted;
- não recebe as capabilities Tauri do `main`;
- fica preso à origem aprovada;
- JavaScript arbitrário, se habilitado, é injetado pelo host nativo;
- consulta DOM com retorno estruturado não é oferecida a conteúdo remoto na v0.6.

## 7. Evidence Ledger

Toda ação local produz evidência.

```text
EvidenceRecord
├── uuid UUIDv7
├── action_uuid
├── correlation_uuid
├── actor
├── capability
├── action
├── outcome
├── request_summary
├── result_summary
├── artifact_uris
├── occurred_at
├── previous_hash
└── record_hash SHA-256
```

O ledger local é append-only e tamper-evident. PostgreSQL permanece o state store oficial e possui tabelas correspondentes na migration 0009.

## 8. API boundary

O API Gateway:

- recusa IP não-loopback;
- bearer token obrigatório para streams e publish;
- `/v1/health` sem segredo;
- publish de host-control bloqueado por padrão;
- SSE e WebSocket carregam `EventEnvelope`.

Não existem endpoints REST públicos para shell/teclado/mouse na v0.6.

## 9. Plugin Registry / Sandbox / Agent Runtime

Continuam valendo as invariantes anteriores:

- SHA-256 + Ed25519;
- trust store;
- UUIDv7;
- semver/Core API;
- permissions;
- lifecycle;
- quarantine fail-closed;
- Bubblewrap no backend Linux disponível;
- capability routing determinístico.

A `plugin_api_version` permanece **0.5.0** na v0.6 porque F14 não altera o contrato de plugins existente.

## 10. Model Gateway

Providers continuam plugins. Routing por capability, context, localização, classificação de dados, budget, provider priority e custo. O Desktop Host nunca escolhe modelo.

## 11. Persistência

Migrations acumuladas:

```text
0001_core.sql
0002_plugin_registry.sql
0003_plugin_quarantine_and_agents.sql
0004_task_graph.sql
0005_model_gateway.sql
0006_event_outbox.sql
0007_capability_audit.sql
0008_bpm.sql
0009_desktop_host_and_evidence.sql
```

## 12. Próxima fronteira

Depois da compilação/E2E nativos do F14:

1. Skill Runtime;
2. Memory + Context Engine;
3. scheduler/background jobs com skills;
4. checkpoint/rollback de workspace;
5. Repo Intelligence/Tree-sitter;
6. Team Runtime;
7. Channel Gateway;
8. Device Nodes;
9. Secret Broker;
10. auto-learning validado por Hypothesis + Objective Proof.
