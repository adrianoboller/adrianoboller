---
description: "Livro de evidencias: registra o que foi provado, com estado e limite, e avisa quando a prova vence."
argument-hint: "[listar|conferir|do-golden RELATORIO]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Evidências da conversão

Quatro estados, porque `passou/falhou` esconde o caso mais comum de migração —
7 de 10 casos do golden batem, e isso é **PARCIAL**. Toda evidência declara o
que **não** prova; sem isso o script recusa gravar.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/evidencia.py" \
  --project-root . "${1:-listar}" ${2:+"$2"}
```

Para registrar uma prova nova, com o limite dela escrito:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/evidencia.py" \
  --project-root . registrar --afirmacao "..." --metodo teste --estado parcial \
  --assunto src/regras/desconto.rs --medida 7/10 \
  --prova "o que foi conferido" --nao-prova "o que isto NÃO prova"
```

`conferir` reconfere o SHA-256 do arquivo provado e marca **VENCIDA** o que
mudou depois: prova de ontem sobre código de hoje não é prova.
