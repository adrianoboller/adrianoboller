---
description: "Ativa o plugin por serial, confere a licenca instalada e explica o que ela protege e o que nao."
argument-hint: "[ativar|conferir] [serial]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Licença

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/licenca.py" verificar
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/licenca.py" instalar "$2"
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/licenca.py" maquina
```

`verificar` diz se há licença válida e até quando; `instalar <serial>` grava a licença; `maquina` mostra a impressão desta máquina, que é o que o cliente manda para receber o serial. `gerar` e `chaves` são do lado de quem emite, não do cliente.

O serial é assinado (RSA-2048) e amarrado à máquina; alterado, vencido ou de outra máquina, é recusado, e o hook recusa os scripts do plugin.

Seja honesto sobre o alcance: a licença é **dissuasão**. Ela não impede quem tem o pacote de ler os arquivos; a proteção real (servir corpus e agentes de um servidor) está documentada como pendente em `licenca/LEIA-ME.md`.

Nunca peça nem repita a chave privada. O serial do cliente pode aparecer na conversa; a chave, não.
