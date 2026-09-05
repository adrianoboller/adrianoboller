---
description: "Confere o efeito real de uma acao (arquivo, comando de leitura, commit), com veredito verificado, divergente ou inconclusivo."
argument-hint: "[conferir|listar]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Verificação de efeito

`ALTER TABLE` que sai 0 não prova que a coluna está lá. Este comando lê o
**estado real** e compara com o esperado.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/efeito.py" \
  --project-root . "${1:-listar}"
```

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/efeito.py" \
  --project-root . conferir --acao "criar índice em customers" \
  --esperado arquivo-contem --alvo database/schema.sql --valor idx_customers_cnpj
```

Três vereditos, com códigos de saída próprios: **verificado** (0),
**divergente** (1) e **inconclusivo** (2). O terceiro existe porque, quando a
conferência falha, a resposta honesta é «não sei» — e ela não pode virar
sucesso dentro de um script. O comando de conferência não pode mudar o estado
que confere: `rm`, `drop`, `commit` e afins são recusados.
