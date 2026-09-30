# PhxClaw — Pesquisa de Capabilities no GitHub

Data: 2026-09-27
Versão alvo: 0.5.0

Objetivo: mapear projetos públicos e componentes maduros que podem sustentar as capabilities pedidas para o PhxClaw sem inflar o microkernel. A regra adotada é: o Core nativo continua mínimo; integrações ficam em platform services ou plugins com manifesto, permissões, auditoria e rollback.

## Matriz de pesquisa e decisão técnica

| Capability | Projeto/referência | Uso no PhxClaw v0.6 | Estado |
|---|---|---|---|
| UUID v7 | https://github.com/uuid-rs/uuid | `uuid` com feature `v7`; todos os identificadores operacionais usam UUIDv7 | Implementado |
| Mindset + BPM | https://github.com/Colin4k1024/bpm-engine | Referência para token state machine, replay, persistência e BPMN | Engine inicial implementada |
| BPMN visual | https://github.com/bpmn-io/bpmn-js | Viewer/modeler no Command Center/WebView | Integração de UI planejada |
| Ollama | https://github.com/ollama/ollama | Adapter HTTP local para version/tags/ps/generate/chat/embed/pull/show/delete | Implementado; E2E depende de Ollama instalado |
| PostgreSQL | https://github.com/rust-postgres/rust-postgres | Driver Rust + adapter PhxClaw + migrations + transactions | Implementado; E2E depende de PostgreSQL |
| HTTPRequest / HTTPGetResult | https://github.com/seanmonstar/reqwest | GET/POST/PUT/PATCH/DELETE etc., headers, auth, JSON, form, multipart, proxy, redirects, timeout, cookies, binário | Implementado |
| View HTML | https://github.com/tauri-apps/wry | WebView nativa para HTML/CSS/JS | Contrato implementado; shell desktop nativo pendente de compile target |
| Desktop shell / comandos | https://github.com/tauri-apps/plugins-workspace | Processo/sidecar com capability permission | Executor PhxClaw implementado, deny-by-default |
| Teclado e mouse | https://github.com/enigo-rs/enigo | Input cross-platform | Provider Enigo implementado via feature `desktop-input` |
| Captura de tela | https://github.com/nashaofu/xcap | Screenshot cross-platform; gravação onde suportada | Provider XCap + FFmpeg implementados |
| TTS | https://github.com/ndarilek/tts-rs | Interface de voz cross-platform | Contrato Rust + backend de validação eSpeak |
| STT | https://github.com/tazz4843/whisper-rs | Referência Rust para whisper.cpp; repositório GitHub arquivado e migrado | Adapter Whisper implementado; runtime/modelo ainda necessários |
| OCR | https://github.com/antimatter15/tesseract-rs | Bindings Rust para Tesseract | Adapter OCR funcional com saída TXT |
| PDF | https://github.com/J-F-Liu/lopdf | Manipulação PDF em Rust | Facade Rust + adapter funcional atual |
| XLSX | https://github.com/MathNya/umya-spreadsheet | Leitura/escrita XLSX/XLSM | Adapter funcional atual; native Rust previsto |
| DOCX | https://github.com/bokuweb/docx-rs | Leitura/escrita DOCX | Adapter funcional atual; native Rust previsto |

## Pontos importantes encontrados

### UUID v7

O projeto `uuid-rs/uuid` é a base adotada. No workspace:

```toml
uuid = { version = "1", features = ["v7", "serde"] }
```

Não existe contador local inventado: criação de IDs passa por `phxclaw_types::new_uuid_v7()`.

### HTTP

`reqwest` cobre o conjunto necessário para uma função equivalente a HTTPRequest completa: cliente assíncrono, headers, corpos brutos, JSON, form-urlencoded, multipart, redirects, proxy, TLS e cookies. O PhxClaw mantém a resposta como binário Base64 e oferece `http_get_result()` para extrair bytes, texto, JSON, headers, status e metadados.

### HTML / WebView

A escolha arquitetural é WRY/Tauri para a casca desktop. O módulo `phxclaw-webview-control` não dá acesso irrestrito por string arbitrária sem política; ele expõe comandos tipados para navegar, carregar HTML, executar JavaScript, injetar CSS, consultar/alterar DOM, clicar, focar, digitar e extrair HTML/SVG.

### Comandos de sistema

O projeto não habilita shell por padrão. O executor suporta:

- Windows: `cmd.exe`, PowerShell e execução direta;
- Linux/macOS: `sh`, `bash` e execução direta;
- stdin/stdout/stderr;
- cwd e ambiente;
- timeout e kill;
- auditoria e capability gate.

Execução destrutiva/privilegiada deverá passar por autorização explícita, política e registro de evidência.

### Teclado e mouse

`enigo` foi incorporado como backend opcional. O provider tipado do PhxClaw cobre movimento, botões, scroll, texto, tecla e hotkey. O E2E precisa ser executado em sessão gráfica real; o ambiente de build atual é headless.

### Captura de tela

- PNG/JPG: imagem raster;
- MP4: gravação de tela via FFmpeg / backend nativo;
- SVG: para HTML/DOM é exportado como SVG; para uma tela arbitrária, o formato SVG pode encapsular o bitmap capturado, mas isso não transforma pixels em vetores reais.

`xcap` tem suporte de captura cross-platform, porém seu próprio README marca parte da gravação/Wayland como incompleta. Por isso o PhxClaw mantém fallback/abstração por backend e não assume paridade total em todas as plataformas.

### Voz

- TTS: contrato Rust + backend local; validação atual usa eSpeak onde disponível.
- STT: contrato local Whisper. O antigo repositório `tazz4843/whisper-rs` no GitHub foi arquivado e aponta para manutenção fora do GitHub; não será pinado cegamente sem validação de supply chain.

### Documentos

O `Office Document Tool` assinado suporta hoje:

- PDF: leitura + escrita;
- TXT: leitura + escrita;
- CSV: leitura + escrita;
- JSON: leitura + escrita;
- XML: leitura + escrita/validação;
- PY: leitura + escrita como texto UTF-8;
- XLSX/XLSM: leitura + escrita;
- DOCX: leitura + escrita.

No caminho de produção, PDF/XLSX/DOCX serão progressivamente trazidos para adapters Rust nativos ou isolados como plugins assinados, sem colocar parsers complexos dentro do microkernel.

## Decisão de arquitetura

Nenhuma dessas libraries vira “parte do Core” só por ser útil. Elas entram em quatro camadas:

1. **Native Core** — Research, Hypothesis, Installer.
2. **Platform Services** — Registry, Sandbox, Task Graph, Model Gateway, Event Bus, HTTP/Egress, PostgreSQL, Ollama, System Automation, WebView, BPM.
3. **Signed Plugins** — agents, model providers, document/media tools e integrações.
4. **Desktop Host Bridge** — operações que exigem janela, teclado, mouse, captura de tela e permissões do SO.

Isso preserva isolamento, atualização por plugin e possibilidade de revogação/rollback.
