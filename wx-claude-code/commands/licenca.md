---
description: "Registra o serial (opcional: o plugin roda sem ele), confere a licenca instalada e explica o que ela e."
argument-hint: "[ativar|conferir] [serial]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Licença

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/licenca.py" verificar
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/licenca.py" instalar "$2"
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/licenca.py" maquina
```

**O serial não é portão.** Desde a 3.51.0 o plugin roda sem serial, por decisão do dono: nenhum hook nega nada. O serial serve para registrar a quem o plugin foi licenciado (marca d'água no `CLAUDE.md` gerado, contexto da sessão, aviso ao fornecedor).

`verificar` diz se há licença válida e até quando; `instalar <serial>` grava a licença; `maquina` mostra a impressão desta máquina, que é o que o cliente manda para receber o serial. `gerar` e `chaves` são do lado de quem emite, não do cliente.

O serial é assinado (RSA-2048) e pode ser amarrado à máquina; alterado, vencido ou de outra máquina, `verificar` o diz, e só.

Seja honesto sobre o alcance: a licença é **registro**, não proteção. A proteção real (servir corpus e agentes de um servidor) está documentada como pendente em `licenca/LEIA-ME.md`.

O passo a passo de emissão para um cliente novo está em `licenca/ATIVACAO.md`.

Nunca peça nem repita a chave privada. O serial do cliente pode aparecer na conversa; a chave, não.
