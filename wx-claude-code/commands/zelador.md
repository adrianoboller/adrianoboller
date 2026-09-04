---
description: "Limpa temporarios antigos do projeto (preflight, logs, caches) uma vez por dia e mede o espaco, sem tocar no que importa."
argument-hint: "[limpar|espaco]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Zelador

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/zelador.py" --project-root . espaco
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/zelador.py" --project-root . limpar
```

Apaga o que é reproduzível: rodadas antigas do pré-flight (guardando o último relatório), logs velhos, `__pycache__`. **Não toca** em evidência, artefato, matriz, decisão, PMO nem código.

Roda sozinho uma vez por dia pelo hook de sessão; rodar de novo no mesmo dia não faz nada e diz isso. Relate o que apagou e quanto liberou — medido, não estimado.
