# Orquestração de agentes

## Topologia

| Agente | Modelo | Responsabilidade | Pode criar subagentes |
| --- | --- | --- | --- |
| `wx-orchestrator` | Opus, high | plano, dependências, gates, síntese e decisões pendentes | sim |
| `evidence-curator` | Sonnet, high | inventário, hashes, PDFs, imagens e mapa de cobertura | sim |
| `help-indexer` | Haiku, medium | verificação e consulta mecânica do corpus 12k e de overrides | não |
| `pdf-forensics` | Sonnet, medium | qualidade de extração/OCR e localizadores | não |
| `wlanguage-specialist` | Opus, high | semântica WLanguage e equivalências | não |
| `business-rules-analyst` | Opus, high | regras, exceções, conflitos e critérios de aceite | sim |
| `ui-flow-analyst` | Sonnet, high | telas, estados, navegação e responsividade | não |
| `data-migration-architect` | Sonnet, high | schema, queries, transações e migração de dados | sim |
| `target-architect` | Opus, high | arquitetura-alvo, ADRs e piloto | não |
| `implementation-lead` | Sonnet, high | ondas de implementação e integração | sim |
| `module-converter` | Sonnet, high | implementação delimitada em worktree | não |
| `test-engineer` | Sonnet, high | golden, integração, E2E e regressão | não |
| `runtime-operations-specialist` | Sonnet, high | baseline executável, ambiente, deploy e recuperação | não |
| `security-privacy-specialist` | Opus, high | ameaça, autorização, segredos, privacidade e gates | não |
| `integration-specialist` | Sonnet, high | APIs, arquivos, jobs, dispositivos e contratos | não |
| `reports-printing-specialist` | Sonnet, high | relatórios, impressão, etiquetas e exportações | não |
| `performance-capacity-specialist` | Sonnet, high | carga, concorrência, capacidade, recursos e tolerâncias | não |
| `release-cutover-specialist` | Sonnet, high | ensaios, reconciliação, corte, rollback e suporte | sim |
| `quality-auditor` | Opus, max | revisão independente, somente leitura, e recomendação do gate | não |
| `design-quality-specialist` | Sonnet, high | revisão visual/acessibilidade e companions opcionais | não |
| `grid-migration-specialist` | Sonnet, high | contratos de grid, virtualização, edição e exportação | não |
| `spreadsheet-evidence-specialist` | Sonnet, medium | plano seguro para sincronização opcional de evidências | não |
| `pmo-gerente-de-projetos` | Opus, high | plano por gates, orçamento de tokens, RAID, sprints e painel medido | sim |
| `wl-hfsql-specialist` | Sonnet, high | Help temas 01-03-*, 10-01, 12-01: HFSQL, Big Data, conectores | não |
| `wl-ui-controls-specialist` | Sonnet, high | Help temas 01-04-02, 02-03-*, 02-04, 13-01: controles, janelas, páginas | não |
| `wl-communication-specialist` | Sonnet, high | Help temas 01-04-01, 17-01: e-mail, HTTP, REST, SOAP, soquete | não |
| `wl-standard-functions-specialist` | Sonnet, high | Help temas 01-04-04, 01-05, 01-06, 01-02, 07-01: funções, propriedades, sintaxe | não |
| `wl-mobile-specialist` | Sonnet, high | Help temas 01-04-03, 15-01: WINDEV Mobile | não |
| `wl-web-specialist` | Sonnet, high | Help temas 01-04-05, 02-05, 05-*: WEBDEV | não |
| `wl-errors-specialist` | Sonnet, high | Help temas 01-01, 03-01: erros de compilação e runtime | não |

Para projeto de grande porte há a camada de **papéis** (A orquestrador … J pesquisador), cada um com quatro subagentes PDCA, trabalhando só em itens do backlog com o seu papel: [papeis-e-pdca.md](papeis-e-pdca.md). Os especialistas desta tabela são chamados pelos papéis.

O modelo da tabela é o ponto de partida; a escolha final por tarefa sai de `scripts/rotear_modelo.py`, conforme [balanceamento-de-modelos.md](balanceamento-de-modelos.md). A divisão da equipe WLanguage por tema do Help está em [equipe-wlanguage.md](equipe-wlanguage.md).

Use aliases de modelo para acompanhar atualizações do Claude Code. Se um modelo não estiver autorizado na organização, registre o fallback efetivamente usado.

## Sequência

1. O contexto principal conclui o questionário, verifica o corpus bundled e executa o pré-flight.
2. `wx-claude-code:wx-orchestrator` cria um plano com dependências e critérios de parada.
3. Em paralelo: evidências, WLanguage, regras, UI, dados, integrações, runtime e segurança. Somente leitura nesta fase.
4. O orquestrador consolida conflitos e devolve perguntas ao contexto principal.
5. Após decisão humana, `wx-claude-code:target-architect` define arquitetura e piloto.
6. `wx-claude-code:implementation-lead` divide módulos independentes. Escrita paralela somente em worktrees ou diretórios sem sobreposição.
7. Relatórios, desempenho/capacidade e cutover são verificados por seus especialistas contra baselines e tolerâncias aprovados.
8. `wx-claude-code:quality-auditor` verifica provas sem corrigir o próprio trabalho e recomenda; o aprovador humano decide.

## Subagentes

- `wx-claude-code:evidence-curator` pode delegar a `wx-claude-code:help-indexer` e `wx-claude-code:pdf-forensics`.
- `wx-claude-code:wlanguage-specialist` delega por tema aos sete `wx-claude-code:wl-*-specialist`, cada um restrito à sua fatia do corpus (`--group`).
- `wx-claude-code:pmo-gerente-de-projetos` acompanha toda a sequência: abre a sprint do gate, registra o uso medido e escreve o resumo no fechamento.
- `wx-claude-code:business-rules-analyst` pode delegar análises por domínio funcional.
- `wx-claude-code:data-migration-architect` pode delegar schema, queries e ensaio de migração.
- `wx-claude-code:implementation-lead` pode delegar por módulo e depois encadear testes.
- Limite a seis tarefas simultâneas por padrão; prefira lotes menores quando os arquivos forem grandes.
- Agentes folha não recebem `Agent` em `tools`.
- Para tarefas longas, mantenha ledger com `task_id`, hash/commit de entrada, dono de paths, checkpoint, custo/contexto, estado e condição de invalidação. Mudança de evidência invalida somente os resultados dependentes.

## Perfis de execução

- `PLUGIN_AGENTS`: use os agentes namespaced do plugin e preserve revisão independente.
- `GENERIC_SUBAGENTS`: quando os agentes do plugin não existirem, crie papéis equivalentes com os modelos disponíveis e registre o fallback.
- `SEQUENTIAL`: em uma skill standalone sem subagentes, execute os papéis em sequência no contexto principal. Não chame a revisão de independente e informe a perda de paralelismo.

## Contrato de delegação

Cada tarefa precisa informar:

- objetivo e fora de escopo;
- entradas e caminhos autorizados;
- identificadores de rastreabilidade envolvidos;
- arquivos que pode criar/editar;
- testes e evidências exigidos;
- condições de parada;
- formato de retorno.

## Contrato de retorno

Todo agente retorna:

```text
STATUS: completed | partial | blocked
SCOPE: ...
EVIDENCE: caminho + localizador + hash quando aplicável
FINDINGS: ...
GAPS/CONFLICTS: ...
DECISIONS_NEEDED: ...
FILES_CHANGED: ...
TESTS: comando + resultado
TRACE_IDS: ...
NEXT: ...
```

Não repasse logs extensos ao orquestrador. Salve-os em `.wx-migration/logs/` e retorne localizadores.

## Independência de revisão

Quem implementa não aprova o próprio gate. O `wx-claude-code:quality-auditor` não edita código e deve tentar refutar a equivalência com casos negativos, limites, falhas, concorrência e regressão.
