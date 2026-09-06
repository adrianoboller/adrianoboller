---
description: "Gemeo da sprint: fotografa contrato, restricoes, evidencias e lacunas do dia, e roda o 'e se' sobre esse estado."
argument-hint: "[fotografar --sprint ID|auditar ID|e-se ID --constraint ID]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Gêmeo da sprint

Uma sprint fechada some: o relatório fica, o **estado** não. Este tira a
fotografia com hash de tudo — contrato, restrições ativas, evidências, decisões
capturadas e as lacunas que o grafo apontava naquele dia.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/gemeo.py" \
  --project-root . "${1:-listar}"
```

`e-se SP00012 --constraint CONST-0007` aplica uma restrição de hoje ao estado
daquele dia. Útil para saber se a regra que você acabou de escrever teria pego
algo — e **perigoso**, porque parece previsão. Não é: o validador roda contra o
código de hoje, e nada ali diz o que a equipe teria feito ao ver a reprovação.
A saída declara isso toda vez.
