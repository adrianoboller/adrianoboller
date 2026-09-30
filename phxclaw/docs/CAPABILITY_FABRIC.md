# PhxClaw Capability Fabric — v0.6

## Fluxo

```text
User / Product Owner
        |
        v
Master Orchestrator
        |
        v
Task Graph + BPM ------------------------------+
        |                                      |
        v                                      v
Agent Runtime                           Approval / Audit
        |
        +-------------------+------------------+------------------+
        |                   |                  |                  |
        v                   v                  v                  v
Model Gateway          Connectivity       Desktop Host       Document/Media
        |              Fabric             Bridge              Tools
        |                   |                  |                  |
        v                   v                  v                  v
Ollama/Providers   Egress->HTTP/PG      Shell/Input/Screen   OCR/TTS/STT/I-O
```

## Capabilities registradas

### Identity
- `uuid.v7.generate`

### Workflow
- `mindset.profile.evaluate`
- `bpm.process.import`
- `bpm.process.validate`
- `bpm.process.execute`
- `bpm.approval.request`

### Connectivity
- `http.request`
- `http.result.get`
- `postgres.connect`
- `postgres.execute`
- `postgres.query`
- `postgres.transaction`
- `ollama.version`
- `ollama.models.list`
- `ollama.generate`
- `ollama.chat`
- `ollama.embed`
- `ollama.pull`
- `ollama.show`
- `ollama.delete`

### WebView / DOM
- `webview.navigate`
- `webview.load_html`
- `webview.javascript.evaluate`
- `webview.css.inject`
- `dom.query`
- `dom.query_all`
- `dom.html.get`
- `dom.html.set`
- `dom.attribute.set`
- `dom.attribute.remove`
- `dom.click`
- `dom.focus`
- `dom.type`
- `dom.event.dispatch`
- `dom.scroll_into_view`
- `dom.computed_style.get`
- `dom.svg.export`

### System automation
- `system.command.execute`
- `desktop.input`
- `screen.capture`

### Media intelligence
- `ocr.read`
- `speech.tts`
- `speech.stt`

### Documents
- `document.read`
- `document.write`

## Política

Toda capability com efeito externo usa deny-by-default. O solicitante precisa ter permissão declarada no manifesto/role/capability policy. Operações laterais relevantes geram registro em `phoenix_capability_audit`.

Rede não é aberta para qualquer plugin. A rota suportada é:

```text
plugin/agent -> capability request -> EgressBroker -> exact origin allowlist -> HTTP client
```

Ollama local pode ser permitido explicitamente como `http://127.0.0.1:11434`. HTTP externo deve ser autorizado por origem.

Comandos do sistema e input de desktop são separados do sandbox normal. Eles só funcionam no **trusted desktop host service**, com autorização explícita e auditoria.
