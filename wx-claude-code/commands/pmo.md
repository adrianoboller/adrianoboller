---
description: "PMO da conversao WX: plano por gates, orcamento de tokens por modelo, RAID, sprints e painel de status com numeros medidos."
argument-hint: "[iniciar|status|sprint|orcamento] [raiz-do-projeto]"
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, Agent, AskUserQuestion"
---

# PMO da conversão

Delegue ao agente `wx-claude-code:pmo-gerente-de-projetos` com o subcomando de `$ARGUMENTS` e a raiz do projeto. Leia antes `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/references/pmo.md` e `balanceamento-de-modelos.md`.

Subcomandos:

- `iniciar`: cria `.wx-migration/pmo/` (plano, orçamento, RAID) e pede ao usuário o aprovador de cada gate, as datas previstas e o orçamento de tokens por gate. Sem previsão o painel mostra `—`, não inventa.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/pmo.py" --project-root <projeto> iniciar --aprovador "<nome>"
```

- `status`: regenera e mostra `pmo/status.md`. Todo número sai de um arquivo e leva a fonte ao lado.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/pmo.py" --project-root <projeto> status
```

- `sprint`: escreve o resumo da sprint corrente em `pmo/sprints/`, no formato de onze seções de `references/pmo.md`, a partir dos artefatos do gate. Testes citados têm evidência; problema tem tratamento; pendência tem dono.
- `orcamento`: registra uso medido de tokens de uma rodada (`pmo.py gastar`) e reavalia o roteamento acima de 80 %.

Regras: o PMO não aprova gate nem decide regra de negócio; percentual sem denominador não entra; estimativa vem rotulada `ESTIMADO`; o que não tem fonte é `INDISPONÍVEL`, nunca zero.
