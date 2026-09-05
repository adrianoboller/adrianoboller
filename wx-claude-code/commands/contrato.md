---
description: "Contrato ativo: o que vale hoje no projeto, separado do historico, com hash para a sessao perceber mudanca."
argument-hint: "[gerar|conferir]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Contrato ativo

Na sprint 10 decidiu-se MySQL; na 30, PhxSql. Um agente que lê o histórico
inteiro sem saber o que foi superado usa a decisão velha com toda a confiança
do mundo. O contrato é o subconjunto que está **em vigor agora** — e o
histórico continua inteiro nas fichas.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/contrato.py" \
  --project-root . "${1:-gerar}"
```

Decisão com `Status: superseded` sai do contrato e vai para a lista de
superadas, com o motivo. Ficha **sem status legível** não entra como vigente:
campo em branco não é aprovação, e ela aparece como pendência.

`conferir` sai 1 quando o contrato mudou desde o gravado — é o que uma sessão
nova pergunta antes de confiar no que leu ontem.
