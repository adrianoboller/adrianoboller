# Declarative Agent Registry — XLSX → JSON

A planilha `PhxClaw_Equipe_110_Agentes_Detalhada_v3_RustOffline.xlsx` é a
fonte declarativa humana dos 110 papéis PhxClaw.

O build gera um manifesto JSON por papel em `config/agents/` e um índice em
`config/agents/registry.index.json`.

## Campos derivados

Cada manifesto contém:

- UUIDv7 preservado da planilha;
- agent_id e nome;
- macroárea e núcleo;
- missão/responsabilidades;
- capabilities;
- permissions/scopes;
- dependências declaradas e dependências resolvidas;
- lifecycle;
- modelos permitidos;
- módulos F15–F25;
- política de execução/aprovação;
- Event Topics;
- Memory/Skill/State;
- knowledge sources;
- referência exata à linha de origem da planilha.

## Regras importantes

1. A planilha não concede poder automaticamente: capabilities geradas ainda
   passam pelo Capability Broker/Policy Engine.
2. Alteração de UUID existente é tratada como mudança de identidade.
3. Duplicidade de nome/UUID deve falhar o build.
4. O runtime não lê células arbitrárias como comandos; a planilha é dado
   declarativo e passa por schema/validação.
5. Modelos sugeridos são allowlist de roteamento, não obrigação de uso.

## Rust official source

Estes papéis recebem o knowledge source `rust-official`:

- Research Agent;
- Research Lead / Pesquisador PDCA;
- Documentador;
- Perséfone.

Todos exigem `knowledge.rust.read` e `knowledge.source_registry.read`.
