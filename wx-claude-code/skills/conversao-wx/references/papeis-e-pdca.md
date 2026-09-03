# Os dez papéis, os quarenta subagentes PDCA e o backlog do Kanban

Um projeto de grande porte não se coordena com uma lista de especialistas
soltos. Precisa de **papéis** com dono, um **backlog** que diga de quem é cada
item, e um jeito único de executar cada item. É isso que esta camada faz, em
cima dos especialistas técnicos que já existem.

## Os papéis

| Letra | Papel | Modelo | Faz | Não faz |
| --- | --- | --- | --- | --- |
| A | orquestrador | opus | quebra o backlog em tarefas, resolve dependências, devolve ao PMO o que trava | implementar |
| B | engenheiro | sonnet | módulos, procedures e telas no destino, a partir de item com evidência | escolher item |
| C | DBA | sonnet | schema, migrações, queries, transações, reconciliação | mudar regra de negócio |
| D | zelador | haiku | nomes, pastas, órfãos, logs, lint, formatação | tocar em lógica |
| E | designer | sonnet | telas conforme `DESIGN.md` e Impeccable, contraste medido | inventar paleta |
| F | prova real | sonnet | teste que falha com o defeito reposto; golden master | aprovar o próprio teste |
| G | QA | opus | refutar: negativos, limites, concorrência, regressão | corrigir |
| H | documentação | sonnet | ADRs, DEC, GAP, resumo de sprint, manual | número sem fonte |
| I | versionador | haiku | commits com motivo, branches por onda, tags por gate | reescrever histórico |
| J | pesquisador | sonnet | Help por tema, bibliotecas do destino, licenças | palpite |

Agentes: `papel-a-orquestrador` … `papel-j-pesquisador`.

## Os quatro subagentes de cada papel

Todo item é executado como um ciclo PDCA, um subagente por fase, nesta ordem:

| Fase | Agente | Faz | Modelo |
| --- | --- | --- | --- |
| Plan | `papel-<x>-<nome>-plan` | hipótese, critério numérico, o que medir, premissa; abre o ciclo | haiku |
| Do | `papel-<x>-<nome>-do` | executa o escopo do item, nada além | sonnet |
| Check | `papel-<x>-<nome>-check` | mede contra o critério; frutífero ou infrutífero, sem adjetivo | sonnet |
| Act | `papel-<x>-<nome>-act` | fecha o ciclo na base de conhecimento; move o item na matriz | haiku |

Os modelos são o ponto de partida; `rotear_modelo.py` ajusta pelo risco e
pelo orçamento do gate.

## O backlog manda

`pmo/backlog.md` tem a coluna **papel**. Um papel só pega item cujo papel é a
sua letra e que esteja em `A fazer` no Kanban, respeitando o WIP da coluna de
destino. Quem prioriza, atribui papel e reordena é o
`pmo-gerente-de-projetos`, na abertura da sprint:

```bash
pmo.py sprint abrir --nome "..." --objetivo "..." --gate G5 --item BR-012:B --item DB-003:C --item UI-004:E
```

O Kanban mostra o papel em cada cartão (`[B engenheiro] BR-012 …`). Item
sem papel aparece como `[sem papel]` e ninguém o pega até o PMO atribuir.

## Fluxo de um item

1. PMO põe o item no backlog com papel e sprint.
2. Papel A (orquestrador) confere dependências e libera.
3. O papel dono roda Plan → Do → Check → Act.
4. Act move o item na matriz (`implemented`, `verified`…); o Kanban reflete.
5. Papel F prova, papel G tenta refutar, papel H documenta, papel I versiona.
6. O PMO fecha a sprint e gera a entrega para o stakeholder.

## A entrega para o stakeholder

```bash
pmo.py entregar --sprint 3 --plugin-root "$CLAUDE_PLUGIN_ROOT"
```

Gera `pmo/entregas/sprint-03-G5-<data>.zip` com:

- `resumo-da-sprint.md`: as doze seções do fechamento;
- `tecnicas-aplicadas.md`: Scrum, Kanban, PDCA e balanceamento de modelos, com os números medidos da sprint;
- `base-de-conhecimento.md`: todos os ciclos PDCA, frutíferos e infrutíferos;
- `ferramentas.md`: o que cada script `.py` faz, lido do cabeçalho do próprio script;
- `kanban.md`, `status.md`, `backlog.md`, `riscos.md`, `gaps.md`, `traceability.csv`;
- `decisoes/` (DEC-*), `pdca/` (um arquivo por ciclo) e `desenvolvimento/` (os `.md` de especificação e arquitetura).

Nada no zip é digitado: tudo é copiado dos artefatos ou gerado deles.
