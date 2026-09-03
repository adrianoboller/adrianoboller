---
description: "PMO da conversao WX com Scrum, Kanban e PDCA: sprints, quadro com WIP, base de conhecimento, orcamento de tokens e painel medido."
argument-hint: "[iniciar|status|sprint|kanban|pdca|orcamento] [raiz-do-projeto]"
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

- `sprint abrir|fechar` (Scrum): abre a sprint do gate com itens do backlog, ou fecha com a decisão do humano; o fechamento escreve o resumo de doze seções em `pmo/sprints/` e devolve ao backlog o que não atingiu a definição de pronto.
- `kanban`: regenera `pmo/kanban.md` da matriz, com limite de WIP; coluna estourada não recebe cartão.
- `pdca abrir|fechar`: abre um ciclo com hipótese, medida e critério numérico; fecha como frutífero ou infrutífero e grava a linha em `pmo/base_de_conhecimento.md`. Infrutífero exige a próxima hipótese.
- `orcamento`: registra uso medido de tokens de uma rodada (`pmo.py gastar`) e reavalia o roteamento acima de 80 %.

Regras: o PMO não aprova gate nem decide regra de negócio; percentual sem denominador não entra; estimativa vem rotulada `ESTIMADO`; o que não tem fonte é `INDISPONÍVEL`, nunca zero.
