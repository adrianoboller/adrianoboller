# SDK Python do PhxClaw

Cliente da API HTTP de tarefas (`phxclaw servir`) e do servidor MCP por stdio
(`phxclaw mcp-serve`). So a biblioteca padrao.

```python
from phxclaw import ClienteApi, ClienteMcp

api = ClienteApi("http://127.0.0.1:8787")          # token: PHXCLAW_API_TOKEN ou <pasta>/api.token
t = api.executar("escreva nota.txt com um resumo")  # cria e espera
print(t["status"], t["answer"], [a["path"] for a in t["artifacts"]])

with ClienteMcp(["phxclaw", "mcp-serve", "--trabalho", "/tmp/proj"]) as mcp:
    print([f["name"] for f in mcp.ferramentas()])
    r = mcp.chamar("python_project", {"action": "typecheck"})
    print(r.json()["diagnosticos"])
```

Os tipos (`Tarefa`, `ResumoTarefa`, `Artefato`) sao `TypedDict` sobre o JSON que o
servidor ja serializa: as chaves sao as dele, sem traducao.

Testes (sobem o binario de verdade, `target/debug/phxclaw`, com um Ollama de roteiro):

```bash
cargo build -p phxclaw
cd sdk/python && /opt/phxclaw-python/bin/python -m pytest -q
```
