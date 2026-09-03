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
  riscos.md           RAID: riscos, premissas, issues, dependências
  sprints/            um resumo por sprint, no formato abaixo
  status.md           painel gerado: gates, cobertura, orçamento, bloqueios
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
