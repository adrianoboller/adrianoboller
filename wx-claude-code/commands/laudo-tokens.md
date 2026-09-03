---
description: "Laudo de uso de tokens em 3 fases (auditar, corrigir, habitos). Somente leitura; nada muda sem aprovacao."
argument-hint: "[fase-1|fase-2|fase-3]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Laudo de uso de tokens

> **Identificação obrigatória.** Comece toda resposta com a linha que o PMO fornece: `BlocoNNNN-SPNNNNN-Título · data` (`pmo.py identificacao`). O hook a injeta a cada interação; se não vier, gere-a antes de responder. Sem PMO iniciado, escreva `Bloco0000-SP00000-Sem PMO iniciado · data`.

> **Licença.** Se o contexto da sessão disser que o WX Claude Code está sem licença válida, pare aqui: explique o estado (`licenca.py verificar`) e como instalar o serial (`licenca.py instalar`). Não tente contornar o hook.

Carregue a skill `laudo-uso-tokens` (`${CLAUDE_PLUGIN_ROOT}/skills/laudo-uso-tokens/SKILL.md`) e execute o **prompt-mestre** dela, `PROMPT-MESTRE.md`, na íntegra.

`$ARGUMENTS` pode pedir uma fase; sem argumento, comece pela Fase 1 e **pare no fim dela**.

Contrato que não se negocia:

- Fase 1 é somente leitura. Nenhum arquivo, configuração, MCP, hook, skill ou agente muda.
- Todo número é `MEDIDO`, `ESTIMADO` ou `INDISPONÍVEL`, com fonte ou premissa ao lado. `INDISPONÍVEL` não vira zero.
- Fase 2 apresenta **uma** mudança por vez e espera o OK.
- Fase 3 entrega no máximo três hábitos, uma frase cada, só os que tiverem evidência nas sessões analisadas.
- Nada de prompt, resposta, segredo ou identificador sensível é reproduzido no laudo.
