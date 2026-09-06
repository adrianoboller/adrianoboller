---
description: "Captura a decisao com a base dela (fontes, contrato, restricoes, commit) e reconfere se essa base ainda vale."
argument-hint: "[capturar|listar|reconferir [ID]]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Decisão reproduzível

O contrato diz **o que** vale. Este diz **com que informação na mesa** foi
decidido — a pergunta que aparece seis meses depois, quando troca o CTO.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/replay.py" \
  --project-root . "${1:-reconferir}"
```

`capturar` exige ao menos uma `--alternativa`: decisão sem alternativa não se
defende, vira «foi assim porque sim».

`reconferir` devolve **ESTÁVEL**, **BASE MUDOU** ou **INCONCLUSIVO** (fonte que
sumiu). Base mudou não quer dizer decisão errada — quer dizer que ninguém sabe
sem reexaminar, e agora está escrito. Ele **não** reexecuta o julgamento: isso é
trabalho de gente com o material na mão, e o material é o que ele preserva.
