---
description: "Aciona um papel da equipe prioritaria: zelador, pesquisador, documentador, qualidade, tarefas, GP, testes, status, base, tradutor."
argument-hint: "<papel> [assunto]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Equipe prioritária (papéis A a J)

`$1` é o papel; `$2` o assunto.

| papel | quando | o que roda |
| --- | --- | --- |
| `zelador` | temporário acumulado, disco apertado | `zelador.py limpar` / `espaco` |
| `pesquisador` | uma hipótese morreu; ela precisa virar aprendizado | registra o infrutífero na base e propõe a próxima |
| `documentador` | código sem documento | `documentar_codigo.py` |
| `qualidade` | antes de fechar sprint | confere prova real de cada item |
| `tarefas` | escolher modelo para a tarefa | `pesar_tarefa.py pesar` |
| `gp` | prazo, risco, escopo | `pmo.py status` e `relatorio` |
| `testes` | item pronto | roda a bateria e exige teste que falha com o defeito reposto |
| `status` | «como está?» | `pmo.py status --por-agente`, sem inventar número |
| `base` | ciclo PDCA fechado | `pmo.py pdca fechar` → frutíferos, infrutíferos, índice |
| `tradutor` | **só a pedido** | `i18n.py` |

Papel desconhecido: liste os dez e pare. O detalhe de cada um está em `references/equipe-prioritaria.md`.

Regra que vale para todos: **número que aparece sai de arquivo**, nunca de memória; e o tradutor não é acionado sozinho.
