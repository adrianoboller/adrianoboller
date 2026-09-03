# Balanceamento dos modelos Claude

O plugin usa três classes de modelo e escolhe por **classe de tarefa**, não por
agente fixo. A regra é única, fica em `scripts/rotear_modelo.py`, e o
orquestrador e o PMO chamam o script em vez de decidir de cabeça. O que o
script devolve vai para o campo `model` do subagente na hora de delegar.

## Classes de tarefa

| Classe | O que é | Modelo | Effort | Exemplos |
| --- | --- | --- | --- | --- |
| `mecanica` | leitura, hash, contagem, busca no corpus, extração de texto, validação de schema | `haiku` | `medium` | help-indexer, pdf-forensics, inventário de anexos |
| `analise` | interpretar código, telas, queries, integrações; implementar módulo delimitado; escrever testes | `sonnet` | `high` | especialistas WLanguage, ui-flow, data-migration, module-converter, test-engineer |
| `decisao` | regra de negócio, arquitetura, conflito entre evidências, síntese para o humano, segurança | `opus` | `high` | wx-orchestrator, business-rules-analyst, target-architect, security-privacy |
| `revisao` | tentar refutar; auditoria independente de gate | `opus` | `max` | quality-auditor |

## Escaladas e rebaixamentos

O roteador começa pela classe e ajusta pelos sinais da tarefa:

- **Sobe um degrau** quando: há conflito entre evidências; a tarefa toca
  dinheiro, fiscal, permissão ou dado pessoal; o resultado vai direto para
  decisão humana; ou a mesma tarefa já falhou uma vez no modelo menor.
- **Desce um degrau** quando: a tarefa é repetição de um padrão já aprovado
  (o segundo módulo igual ao primeiro); o volume é grande e o critério é
  objetivo (mil páginas para classificar); ou o orçamento do gate está acima
  de 80 % do previsto.
- `revisao` nunca desce. `mecanica` nunca sobe além de `sonnet`.

## Orçamento

Cada gate tem um orçamento em tokens e em chamadas, definido no PMO
(`.wx-migration/pmo/orcamento.json`). O roteador lê o gasto acumulado e:

- acima de 80 %: aplica o rebaixamento acima onde a regra permite e avisa;
- acima de 100 %: devolve `BLOQUEADO` e a tarefa vai para o PMO decidir, com o
  número, não com adjetivo.

Gasto se **registra medido**, do campo de uso da resposta, nunca estimado por
tamanho de arquivo. Sem campo de uso, a linha entra como `INDISPONÍVEL`.

## Paralelismo

- Até seis tarefas simultâneas; `opus` no máximo duas ao mesmo tempo.
- Investigação é paralela; escrita é paralela só em worktrees ou pastas sem
  sobreposição.
- Tarefas `mecanica` são agrupadas em lotes (um subagente para dez PDFs, não
  dez subagentes).

## Fallback

Se um modelo não estiver disponível na organização, o roteador devolve o
próximo abaixo e registra o fallback em `.wx-migration/pmo/roteamento.jsonl`.
Nunca inventa alias: os nomes aceitos são `opus`, `sonnet`, `haiku` e
`inherit`.
