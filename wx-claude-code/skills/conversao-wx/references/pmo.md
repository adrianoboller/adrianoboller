# Módulo PMO: gerente de projetos da conversão

O PMO é quem responde «em que pé está o projeto, quanto custou, o que trava e
quem decide». Ele não converte nada e não aprova gate: mede, planeja, cobra e
registra. O agente é `pmo-gerente-de-projetos`; o comando é
`/wx-claude-code:pmo`; os números saem de `scripts/pmo.py`, nunca digitados.

## Onde vive

```text
.wx-migration/pmo/
  plano.json          gates, ondas, sprints, responsáveis, datas previstas
  orcamento.json      tokens e chamadas previstos × gastos por gate (medidos)
  roteamento.jsonl    cada decisão de modelo do rotear_modelo.py
  riscos.md           RAID: riscos, premissas, issues, dependências (já com os RSK-* do bloco 0)
  projeto.json        bloco 0 do questionário: softhouse, prazo final, marcos, orçamento financeiro
  cronograma.md, organograma.md, fluxograma.md   do bloco 0; o iniciar lê os marcos com gate
  sprints/            um resumo por sprint, no formato abaixo
  backlog.md          backlog do produto priorizado (Scrum)
  kanban.md           quadro gerado da matriz, com WIP (Kanban)
  pdca/               um PDCA-NNN.md por ciclo
  base_de_conhecimento.md  uma linha por ciclo fechado, frutífero ou não
  status.md           painel gerado: gates, cobertura, orçamento, bloqueios
  relatorio.md        relatório de onze seções, gerado ao fechar sprint e no entregar
  painel.html         o relatório, o kanban e a base em HTML (tema claro e escuro)
```

## O que o PMO mede (e de onde)

| Indicador | Fonte | Estado |
| --- | --- | --- |
| Gate atual e decisão de cada gate | `gate-status.md`, relatórios de gate | MEDIDO |
| Itens por estado (`inventoried` … `accepted`, `blocked`) | `traceability.csv` | MEDIDO |
| Lacunas abertas por severidade | `gaps.md` | MEDIDO |
| Decisões pendentes de humano | `decisions/DEC-*.md` com `status: proposed` | MEDIDO |
| Tokens e chamadas por gate | `orcamento.json`, preenchido do campo de uso das respostas | MEDIDO ou INDISPONÍVEL |
| Modelos usados e fallbacks | `roteamento.jsonl` | MEDIDO |
| Percentual de conclusão | itens `accepted` ÷ itens totais, com o denominador escrito | MEDIDO |

Percentual sem denominador é inválido. Estimativa entra rotulada `ESTIMADO`
com a premissa ao lado. O que não tem fonte é `INDISPONÍVEL`, nunca zero.

## As três técnicas, e como se encaixam

O PMO usa **Scrum** para organizar o tempo, **Kanban** para ver o fluxo e travar
o excesso, e **PDCA** para transformar cada hipótese de trabalho em
aprendizado registrado. Os três saem de `scripts/pmo.py` e leem os mesmos
artefatos; nenhum é texto solto.

### PDCA: o loop que alimenta a base de conhecimento

Toda hipótese de trabalho (uma otimização, um jeito de extrair PDF, uma
equivalência de função) abre um ciclo:

```bash
pmo.py pdca abrir --gate G4 --hipotese "..." --medida "o que medir" --criterio "número"
pmo.py pdca fechar --id PDCA-001 --resultado frutifero|infrutifero --medido "..." --aprendizado "..." [--proxima "..."]
```

- **Plan**: hipótese, o que medir, critério com número, premissa a confirmar antes.
- **Do**: o que foi feito, com comando e evidência.
- **Check**: o medido contra o critério.
- **Act**: frutífero ou infrutífero, aprendizado e próxima hipótese.

O fechamento grava uma linha em `pmo/base_de_conhecimento.md` **nos dois
casos**. A recusa com o número é resultado tão válido quanto o ganho: é o que
impede a mesma ideia de voltar sem medição. Ciclo infrutífero sem
`--proxima` é recusado, porque hipótese que morre gera a próxima. A base é
lida na abertura de cada sprint, antes de planejar.

### Kanban: fluxo e limite de WIP

`pmo.py kanban` gera `pmo/kanban.md` da matriz de rastreabilidade. As colunas
são os estados da matriz (`A fazer`, `Em andamento`, `Em verificação`,
`Concluído`, `Bloqueado`); o quadro não se edita, muda-se o estado na matriz.
Limites de WIP ficam em `plano.json` (padrão: 6 em andamento, 4 em
verificação). Coluna estourada aparece marcada e não recebe cartão novo:
termina-se antes de começar. Lacunas e itens `blocked` mostram o motivo no
cartão.

### Scrum: sprint por gate ou onda

```bash
pmo.py sprint abrir --nome "..." --objetivo "..." --gate G4 --item BR-001 --item QRY-001
pmo.py sprint fechar --decisao APPROVED|CONDITIONAL|REJECTED --pedido "..."
```

Uma sprint por vez, com backlog em `pmo/backlog.md` (priorizado, cada item
ligado a um `trace_id`). Cerimônias: planejamento (lê a base de conhecimento
e o orçamento), diário (o painel), revisão (o `quality-auditor` recomenda, o
humano decide) e retrospectiva (o que mudar vira hipótese PDCA da próxima
sprint). A **definição de pronto** é a da matriz: evidência com localizador,
implementação apontada, teste, resultado comparado, aprovação humana e
confiança nunca `low`. Item que não fecha volta ao backlog, e o resumo diz
quais.

## Sprints

Uma sprint cobre um gate ou uma onda do G5. O resumo de sprint segue o
formato já usado nos projetos do Adriano (`governanca-pmo`):

1. Identificação (sessão, sprint, data, objetivo, projeto)
2. Solicitação (o pedido, literal)
3. Insumos recebidos (arquivo, bytes, situação)
4. Atividades realizadas
5. Arquivos criados e movidos
6. Decisões técnicas (`DEC-*`, com fundamento)
7. Testes executados (com evidência e status)
8. Problemas encontrados e tratamento
9. Conflitos resolvidos e residuais
10. Pendências e gaps (`GAP-*`)
11. Orçamento da sprint: previsto × gasto, por modelo
12. Retrospectiva e decisão do gate (o que mudar vira PDCA)

A seção 9 lista os ciclos PDCA fechados na sprint, copiados da base de conhecimento.

## Cerimônias

- **Abertura de gate**: o PMO gera o plano da sprint com o orçamento por
  classe de tarefa (`balanceamento-de-modelos.md`) e a lista de aprovadores.
- **Acompanhamento**: a cada retorno de agente, o PMO atualiza `orcamento.json`
  com o uso medido e reavalia o roteamento acima de 80 %.
- **Fechamento**: o `quality-auditor` recomenda, o humano decide, o PMO grava a
  decisão e o resumo de sprint. Gate `REJECTED` volta com plano de correção,
  não com nova promessa.

## RAID

`riscos.md` tem quatro tabelas: riscos (probabilidade × impacto × resposta),
premissas (o que se assumiu e quem confirma), issues (o que já aconteceu) e
dependências (de quem, para quando). Toda linha tem dono e data. Risco sem
resposta é risco aceito, e isso fica escrito.

## O que o PMO não faz

Não decide regra de negócio, não aprova gate, não muda escopo sozinho, não
apresenta estimativa como medição, e não esconde gate vermelho atrás de
percentual.
