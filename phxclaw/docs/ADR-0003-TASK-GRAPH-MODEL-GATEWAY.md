# ADR-0003 — Task Graph determinístico e Model Gateway desacoplado

Status: Accepted — v0.4

## Contexto

O PhxClaw precisa coordenar agentes e modelos sem transformar o Master Orchestrator em um bloco monolítico e não auditável.

## Decisão

1. Toda missão executável é decomposta em um DAG explícito de tarefas.
2. Dependências, retries, approvals e idempotência pertencem ao Task Graph, não ao prompt de um LLM.
3. Agentes são selecionados pelo Agent Runtime por capability/policy.
4. IPC com processos usa protocolo JSONL versionado.
5. Model providers são plugins e passam pelo Model Gateway.
6. Budget, privacy/locality e fallback são aplicados antes da chamada do modelo.
7. A UI apenas observa/comanda via APIs futuras; ela não é autoridade do estado.

## Consequências

Positivas:

- rastreabilidade;
- replay mais simples;
- decisões auditáveis;
- troca de provider sem alterar o core;
- approval humano explícito;
- custos controláveis.

Trade-offs:

- mais contratos e estados;
- persistência precisa ser transacional;
- worker distribuído exigirá fencing e outbox.
