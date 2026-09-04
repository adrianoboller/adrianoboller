---
description: "Exporta o projeto resultante, organizado em sete pastas, com manifesto e SHA-256, para a pasta que o usuario escolheu."
argument-hint: "[pasta-de-saida]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Exportar o projeto

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/exportar_projeto.py" \
  --project-root . --saida "$1"
```

Sem `$1`, use `L3.pasta_de_saida` do questionário; sem ela, pergunte.

Sai organizado em sete pastas, com manifesto e SHA-256 de cada arquivo. O que **não** vai junto: `.env` e derivados (o `.env.exemplo` vai), `target/`, `node_modules/`, `.git/`, e qualquer arquivo que pareça carregar token — nesse caso o script recusa e diz qual.

Depois de exportar, confira e relate: quantos arquivos, quantos bytes, e o que ficou de fora e por quê.
