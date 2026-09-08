---
description: "Semaforo fisico ou na tela: vermelho aguardando voce, amarelo em execucao, verde pronto. Liga por um arquivo de configuracao; desligado custa zero."
argument-hint: "[--estado|--testar verde|amarelo|vermelho]"
allowed-tools: "Read, Bash"
---

# Semáforo

Os hooks do plugin acendem três cores conforme o estado da sessão:

| Cor | Quando | Evento do Claude Code |
| --- | --- | --- |
| **vermelho** | o Claude Code parou e espera você (permissão, pergunta) | `Notification` |
| **amarelo** | em execução | `UserPromptSubmit`, `PreToolUse` |
| **verde** | tarefa concluída, pronto para a próxima | `Stop`, `SessionStart` |

Desligado por padrão e custa zero. Para ligar, crie `~/.wx-claude-code/semaforo.json`:

```json
{"url": "http://192.168.0.50/luz", "comando": "", "arquivo": true}
```

- `url`: o semáforo recebe `GET ?cor=verde|amarelo|vermelho` (ESP32 com três LEDs, Home Assistant, ou o painel local `ferramentas/wx-semaforo/painel.py`).
- `comando`: alternativa por linha de comando, com `{cor}` no lugar (ex.: um script que fala com a porta USB).
- `arquivo`: grava `~/.wx-claude-code/semaforo.estado`, para quem quer ler o estado de outro programa.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/semaforo.py" --testar vermelho   # acende de propósito
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/semaforo.py" --estado            # a cor atual e o evento
```

Falha de rede é silenciosa e limitada a 1,5 s: semáforo que não acende não trava a sessão. A receita do hardware está em `ferramentas/wx-semaforo/LEIA-ME.md`.
