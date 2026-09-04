---
description: "Instala e confere o ambiente pedido na letra K: privilegios, Rust, PostgreSQL, MySQL, MariaDB, Supabase, GitHub e n8n."
argument-hint: "[instalar|verificar]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Ambiente (letra K)

**Verificar** (não muda nada; devolve 3 quando falta algo):

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/verificar_ambiente.py" \
  --questionario .wx-migration/questionario.json
```

**Instalar**: o script gerado pelo questionário, que é idempotente e resolve `sudo`/`root` conforme K0:

```bash
bash .wx-migration/ambiente/instalar-ambiente.sh
```

Antes de rodar o instalador, **mostre o que ele vai fazer** e peça confirmação: ele mexe em pacotes e serviços da máquina.

Senha nunca sai daqui: o `.env.exemplo` traz só os **nomes** das variáveis, e o usuário preenche o `.env` fora do repositório. Se o usuário colar uma senha na conversa, não repita, não grave, não mascare — diga que basta o nome da variável.

Versão mínima acima da estável do dia aparece como `falta` mesmo depois de atualizar: nesse caso o errado é o mínimo, e corrige-se no questionário com `/wx-claude-code:pergunta K1`.
