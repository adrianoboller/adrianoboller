# wx-semaforo — o semáforo do Claude Code

Vermelho: o Claude Code parou e espera você. Amarelo: em execução. Verde:
pronto para a próxima tarefa. As três cores saem dos eventos dos hooks, sem
inferência: `Notification` acende o vermelho, `UserPromptSubmit` e
`PreToolUse` o amarelo, `Stop` e `SessionStart` o verde.

O hook (`skills/conversao-wx/scripts/semaforo.py`) vem **desligado** e custa zero: sem o arquivo de
configuração ele sai antes de ler o stdin. Para ligar:

```json
// ~/.wx-claude-code/semaforo.json
{"url": "http://192.168.0.50/luz", "comando": "", "arquivo": true}
```

## Três jeitos de acender

| Jeito | Como | Quando |
| --- | --- | --- |
| **painel na tela** | `python3 painel.py --porta 8777` e `"url": "http://127.0.0.1:8777/luz"`; abra `http://127.0.0.1:8777/` num segundo monitor | testar a configuração, ou quem não quer hardware |
| **ESP32 com três LEDs** | grave `semaforo-esp32.ino`, ponha o IP na `url` | o semáforo físico da foto: três LEDs com resistor de 220 Ω nos pinos 25, 26 e 27 |
| **comando** | `"comando": "meu-script {cor}"` | Home Assistant, lâmpada USB, o que responder a uma linha de comando |

Teste sem esperar uma sessão:

```bash
python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/semaforo.py" --testar vermelho
python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/semaforo.py" --estado
```

## O que ele não faz

- Não acende sem a configuração, e não pergunta: quem quer, cria o arquivo.
- Não espera a rede: 1,5 s e desiste, em silêncio. Semáforo apagado não pode
  travar a sessão.
- Não distingue «esperando permissão» de «esperando resposta»: os dois são
  `Notification`, e os dois são vermelho — você é o próximo passo.
