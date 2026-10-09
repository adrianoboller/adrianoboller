# Radar de IA (24/09 a 09/10/2026) contra o PhxClaw

Fonte: `Resumo_Sessao_Radar_IA_2026-09-24_a_2026-10-09.md`, enviado pelo dono em 09/10/2026. O
próprio resumo avisa que reúne anúncios e propostas, não testes feitos; nada dele entra aqui como
número nosso. A coluna «hoje» foi medida por busca no fonte (`crates/phxclaw-agent`,
`phxclaw-agent-core`, `apps/phxclaw`), em 09/10/2026.

## O que já existe (não é proposta nova)

| Ideia do radar | Hoje no PhxClaw | Evidência |
|---|---|---|
| Política fora do modelo, permissões mínimas por agente | portão único de capacidades, deny-by-default | `motor.rs` (`call_tool`), `tests/guardas.rs` |
| Credenciais fora do contexto do modelo | SecretBroker, credencial por nome, motor único de detecção | `phxclaw-secret-broker`, `phxclaw_types::segredo` |
| Sandbox de execução | bwrap com máscara do diretório do agente | `phxclaw-sandbox` |
| Aprovação humana antes de ação sensível | `ask_user` / perguntar no portão | `motor.rs` |
| Histórico verificável de execução | EvidenceLedger por tarefa | `phxclaw-evidence-ledger` |
| Cancelamento, checkpoint e retomada | tarefa cancelável, checkpoint, retomada de fluxo | `tarefa.rs`, `phxclaw-checkpoint`, `api.rs` |
| Gatilho por evento | webhook, formulário, pasta, poll, agenda | `gatilhos.rs`, `gatilho_poll.rs`, `agenda.rs` |
| Detecção de segredo exposto | varredura no `git_write` e na entrada externa | `segredos.rs`, `fluxos.rs` |
| Trocar de modelo por adaptador | provedores Anthropic, OpenAI, Gemini, Ollama | `phxclaw-*-provider`, `phxclaw-llm` |

## Lacunas medidas, em ordem de valor

| # | Lacuna | Hoje | Proposta | Depende do dono? |
|---|---|---|---|---|
| R1 | **DecisionProvider**: decisão restrita (escolher agente, ferramenta, prioridade, repetir ou não) com incerteza explícita, barata primeiro e escalando o caso incerto | 0 ocorrências de decisor ou classificador no agente | Interface separada do provedor de geração; saídas `predicate`, `choice`, `score` com confiança; abaixo do limiar escala para o modelo forte ou para uma pessoa. **A probabilidade nunca concede permissão**: a decisão só endurece, como a lei da cognição C1 | Não |
| R2 | **Custo em dinheiro por tarefa concluída corretamente** | a avaliação mede p50/p95 e tokens; 0 ocorrências de custo em moeda no agente | Tabela de preço por modelo na config do operador, com data e fonte (nunca embutida como verdade); custo por tarefa no relatório; métrica «custo por tarefa correta» na avaliação | Não |
| R3 | **Orçamento por missão** em tokens e em dinheiro, com corte | só há o orçamento de argumentos inválidos | Teto por tarefa e por fluxo; ao bater, para e diz o gasto; nunca segue calado | Não |
| R4 | **Roteamento e fallback entre provedores** por política | sem fallback de modelo no agente | Regra determinística (tipo de tarefa, custo, falha do provedor) com R1 opcional; registro de qual modelo atendeu | Não |
| R5 | **Bateria comum entre provedores** («testar agora» do radar) | `phxclaw-model-arena` existe como biblioteca fora do agente | Mesma bateria, gabarito fixo, medindo acerto, custo, duração, tentativas e intervenção; vencedor só sem faixas cruzadas | **Sim, para os pagos**: chaves de API via `phxclaw … chave`. Com Ollama local, roda já |
| R6 | **MCP Events**: servidor MCP empurra evento que dispara trabalho | só `notifications/initialized` | Assinar `resources/updated` e similares como gatilho de fluxo, pelo mesmo portão | Não |
| R7 | **Teto organizacional que plugin não amplia** | capacidades por config; não está provado que servidor MCP ou skill não amplie | Prova por teste: skill ou MCP pedindo capacidade fora do teto é recusado | Não |
| R8 | **Computer Use** (Holo4 e similares) | 0 ocorrências | ⏸ Depois da versão: precisa de tela (a VM), conferência de licença por variante e reprodução própria | Sim (VM) |

## Fora, com o motivo

- **Escolher «a melhor IA»**: o radar conclui o mesmo. O núcleo não se amarra a provedor (já é assim).
- **Benchmarks de fornecedor como promessa**: não entram; só número que a bateria R5 reproduzir.
- **Integração financeira do Grok, anúncios no ChatGPT**: sem relação com o produto, ou sem fonte
  primária suficiente.
- **Lição dos Claude Mods (extensão sem sandbox)**: vale como alerta para R7. Extensão do PhxClaw
  não pode rodar fora do portão.

## Recomendação

Onda única com R1 a R4, R6 e R7 (nenhum depende do dono), R5 com Ollama local primeiro, e R8 em ⏸.
R1 é o maior ganho: separa «decidir» de «gerar» e alimenta o roteamento (R4) e o orçamento (R3).
