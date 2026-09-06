---
description: "Onde o questionario parou: quantas respondidas, o proximo item, e reabrir um ja respondido."
argument-hint: "[progresso|retomar|revisar <id>|fechar <id>]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Onde o questionário parou

São **60 itens** — medidos, não estimados. Ninguém responde isso numa sessão só.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/progresso_do_questionario.py" \
  --project-root . ${1:-retomar}
```

`retomar` aponta o próximo item e como respondê-lo; `progresso` lista o que
falta; `revisar <id>` reabre um item já respondido e `fechar <id>` desfaz.

Quatro estados, porque três mentiriam:

| estado | o que quer dizer |
| --- | --- |
| respondida | o valor difere do modelo: alguém digitou |
| pendente | vazio |
| **como o modelo** | preenchida com o valor que o modelo já trazia — **ninguém confirmou** |
| reaberta | respondida, mas marcada para rever |

O quarto nasceu de rodar isto no exemplo: F5 e F12 estão preenchidas com os
valores do próprio modelo. Chamar de «pendente» mente (o valor está lá) e
chamar de «respondida» mente também.

O script **não guarda uma segunda cópia** do que foi respondido: isso se deriva
do `questionario.json`. Duas fontes para o mesmo fato é como as duas discordam
depois. O único estado próprio é a reabertura, em `.wx-migration/progresso.json`
— fora do questionário do cliente.
