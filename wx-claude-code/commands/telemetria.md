---
description: "Telemetria OTLP/JSON gerada do registro de operacoes, no disco do cliente; enviar para fora e explicito."
argument-hint: "[resumo|exportar|enviar --endereco URL]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Telemetria

O registro já mede tudo; faltava o **formato** que a infraestrutura do cliente
consome. Sem SDK, sem agente, sem dependência.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/telemetria.py" \
  --project-root . "${1:-resumo}"
```

Três decisões que valem mais que o código: **arquivo por padrão** (o Sovereign
Mode é o que abre banco e governo, e telemetria que sai sozinha mata isso);
**nada de conteúdo** (só nome de operação, tempo e código — argumento e caminho
do cliente ficam de fora, e há teste que falha se vazarem); e **span é o que
aconteceu** — operação que o registro não tem não vira span.

`enviar --endereco http://127.0.0.1:4318` faz o POST para um coletor OTLP/HTTP,
explícito e nunca automático.
