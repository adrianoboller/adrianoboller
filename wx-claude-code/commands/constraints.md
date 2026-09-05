---
description: "Restricoes do projeto e o portao C-GATE: registra regra com validador e confere se o resultado esta conforme."
argument-hint: "[listar|c-gate|semear|criar|revogar]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Restrições e C-GATE

`F-GATE` responde **funciona?**; `C-GATE` responde **está conforme às regras
deste projeto?**. A Sprint só é aprovada com os dois, porque teste verde não é
Sprint aprovada.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/constraints.py" \
  --project-root . "${1:-listar}"
```

- `semear` propõe as restrições que o próprio questionário já implica (não
  grava sem `--aplicar`: guarda nova entra pedida, não imposta);
- `criar --titulo … --severidade bloqueante --validador "comando"` registra uma;
- `c-gate` roda os validadores. Regra sem validador volta **INCONCLUSIVA** —
  nunca aprovada, porque portão que aprova o que não conferiu é pior que portão
  nenhum;
Duas pegadinhas do validador, as duas achadas rodando:

- ele roda **sem shell** — nada de `|`, `&&` ou `>`; para usar shell, escreva
  `sh -c "..."` de propósito;
- **`grep` sai 1 quando não acha.** Um validador de «não há segredo aqui»
  acusaria violação justamente com o projeto limpo. Para esses, use
  `--inverter`, que diz que sair 0 significa **achou o problema**.

- `revogar CONST-0001 --motivo "…" --por CONST-0007` tira do portão **sem
  apagar do histórico**.
