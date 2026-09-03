---
description: "PMO da conversao WX com Scrum, Kanban e PDCA: sprints, quadro com WIP, base de conhecimento, orcamento de tokens e painel medido."
argument-hint: "[iniciar|status|relatorio|sprint|kanban|pdca|entregar|painel|exportar|limpar|orcamento (só no comando: chama uso_de_tokens.py)] [raiz-do-projeto]"
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, Agent, AskUserQuestion"
---

# PMO da conversão

> **Licença.** Se o contexto da sessão disser que o WX Claude Code está sem licença válida, pare aqui: explique o estado (`licenca.py verificar`) e como instalar o serial (`licenca.py instalar`). Não tente contornar o hook.

Delegue ao agente `wx-claude-code:pmo-gerente-de-projetos` com o subcomando de `$ARGUMENTS` e a raiz do projeto. Leia antes `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/references/pmo.md` e `balanceamento-de-modelos.md`.

Subcomandos:

- `iniciar`: cria `.wx-migration/pmo/` (plano, orçamento, RAID) e usa o aprovador do item 0.16 do questionário e os marcos do bloco 0; pergunta só o que estiver vazio: aprovador, datas previstas e o orçamento de tokens por gate. Sem previsão o painel mostra `—`, não inventa.

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
- `entregar`: zipa a entrega da sprint para o stakeholder (`pmo.py entregar --sprint N --plugin-root "${CLAUDE_PLUGIN_ROOT}"`): resumo de doze seções, técnicas aplicadas com números, base de conhecimento, ferramentas usadas lidas dos scripts, decisões, lacunas, RAID e o Kanban do fechamento.
- `exportar`: grava o projeto resultante, organizado em sete pastas numeradas com manifesto de hashes, na pasta que o usuário definiu (`--destino`, ou `L3.pasta_de_saida` do questionário): `python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/exportar_projeto.py" --project-root <projeto> --destino <pasta> [--codigo <dir>] [--com-evidencias]`. `.env`, chaves, `target/`, `node_modules/` e `.git/` ficam de fora; arquivo com formato de token é recusado com o caminho.
- `limpar`: o zelador (papel D) apaga temporários: execuções antigas do pré-flight (ficam as três últimas), logs com mais de 7 dias, `__pycache__`, worktrees parados. `python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/zelador.py" --project-root <projeto> limpar [--dias 7] [--executar]`. Sem `--executar` só relata. O hook `SessionStart` roda isso sozinho no máximo uma vez por dia e registra em `.wx-migration/logs/zelador.md`.
- `painel`: gera `pmo/painel.html` (status, kanban e base de conhecimento) para o aprovador abrir no navegador; regenera-se, não se edita.
- `orcamento`: prefira `uso_de_tokens.py --project-root <projeto> lancar --gate G<n>`, que lê o campo `usage` das sessões do Claude Code (MEDIDO); `pmo.py gastar` fica para lançamento manual. Depois, registra uso medido de tokens de uma rodada (`pmo.py gastar`) e reavalia o roteamento acima de 80 %.

Regras: o PMO não aprova gate nem decide regra de negócio; percentual sem denominador não entra; estimativa vem rotulada `ESTIMADO`; o que não tem fonte é `INDISPONÍVEL`, nunca zero.
