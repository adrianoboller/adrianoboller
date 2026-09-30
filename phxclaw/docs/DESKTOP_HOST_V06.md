# PhxClaw Desktop Host v0.6

## Objetivo

F14 conecta o Command Center ao runtime real do PhxClaw. O host desktop é um processo Tauri/WRY separado do microkernel e atua como **broker de capacidades locais**.

```text
Agents / Task Graph
        |
        | EventEnvelope: desktop.action/requested
        v
+---------------------------+
| LiveEventHub              |
| broadcast + replay ring   |
+-------------+-------------+
              |
              v
+---------------------------+
| PhxClaw Desktop Host  |
| Tauri 2.12 / WRY          |
| policy + dispatcher       |
+----+---------+-------------+
     |         |
     |         +--> trusted WebView DOM bridge
     |
     +--> Shell / app launch / input / screenshot
              |
              v
       Evidence Ledger
       UUIDv7 + SHA-256 chain
```

## Contratos

- `phxclaw-desktop-v1` para ações e resultados.
- `EventEnvelope` para tráfego assíncrono.
- `phoenix:event` para streaming Rust → UI pelo sistema de eventos Tauri.
- `phoenix:webview-request` para operações DOM tipadas no WebView local confiável.
- API loopback para clientes externos: snapshot, SSE, WebSocket e publish controlado.

## Deny-by-default

As capacidades de host não são ativadas só porque o código existe.

Variáveis de opt-in:

```text
PHXCLAW_ENABLE_HOST_EXEC=1
PHXCLAW_ENABLE_SHELLS=1
PHXCLAW_ENABLE_INPUT=1
PHXCLAW_ENABLE_SCREEN_CAPTURE=1
PHXCLAW_ENABLE_WEBVIEW_CONTROL=1
PHXCLAW_ENABLE_EXTERNAL_WEBVIEWS=1
PHXCLAW_WEBVIEW_ALLOWED_ORIGINS=https://example.com,https://docs.example.com
PHXCLAW_API_HOST_CONTROL=1
PHXCLAW_API_PORT=48187
PHXCLAW_API_TOKEN=<secret opcional>
```

Por padrão todas as operações de host sensíveis permanecem desabilitadas.

## Managed WebViews

WebViews externos:

- exigem opt-in;
- aceitam somente `http`/`https`;
- rejeitam credenciais na URL;
- exigem origem exata na allowlist;
- não recebem a capability Tauri do Command Center;
- em v0.6 só recebem JavaScript arbitrário via `WebviewWindow::eval` quando `webview_control` está habilitado;
- consultas DOM com retorno estruturado permanecem limitadas ao WebView `main` local.

Essa divisão evita entregar IPC privilegiado diretamente a conteúdo web remoto.

## API realtime

O API Gateway:

- recusa bind fora de loopback;
- usa Bearer token de 256 bits;
- mantém `/v1/health` simples;
- exige Bearer em snapshot/SSE/WebSocket/publish;
- bloqueia tópicos de host-control via HTTP, salvo opt-in explícito;
- não expõe shell/input diretamente como endpoint REST em v0.6.

## Evidência

Cada ação concluída/negada/falha gera um `EvidenceRecord` UUIDv7.

O ledger local é JSON Lines append-only com:

- `previous_hash`;
- `record_hash` SHA-256;
- ação/capability/actor;
- request/result resumidos;
- artifacts;
- timestamp UTC.

A migration `0009_desktop_host_and_evidence.sql` prepara o espelho PostgreSQL, que permanece o store oficial do projeto.

## Build alvo

O scaffold usa Tauri 2.12.x. WRY é o runtime WebView por baixo do Tauri. O host de montagem desta entrega não possui `cargo/rustc`; por isso a compilação nativa não é marcada como aprovada nesta versão.
