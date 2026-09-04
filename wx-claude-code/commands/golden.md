---
description: "Golden master: captura o resultado do legado e compara com o do sistema novo, com tolerancia declarada. Igualdade vira numero."
argument-hint: "[capturar|comparar]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Golden master

«Parece igual» não conta. Cada caso tem id, entrada e resultado esperado, e a comparação devolve um número.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/golden.py" capturar --help
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/golden.py" comparar --help
```

Ao comparar, mostre: quantos casos passaram, quais falharam, e **em que campo** cada falha diverge. Diferença de arredondamento, de fuso, de collation ou de nulo é achado, não ruído — é exatamente o que o golden master existe para pegar.

Caso que falha vira item na matriz (`BR-*` reaberto) e entra no relatório do PMO. Nunca ajuste a tolerância para o teste passar: tolerância se decide antes, e mudá-la é `DEC-*`.

Sem baseline executável do legado (o comum hoje), diga isso: a comparação prova o que os dados de amostra cobrem, e o resto continua declarado, não provado.
