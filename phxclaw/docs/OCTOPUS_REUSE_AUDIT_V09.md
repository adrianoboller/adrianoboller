# PhxClaw v0.9 — Auditoria de Reaproveitamento do Phoenix Octopus

Data: 2026-09-27

## Conclusão

O Phoenix Octopus contém vários blocos que valem reaproveitar, mas o reaproveitamento correto é por **contratos, adapters e plugins**, não por fusão indiscriminada no core do PhxClaw.

O workspace do Octopus analisado declara dezenas de crates especializados, incluindo `octopus-agent-runtime`, `octopus-provider-gateway`, `octopus-context-manager`, `octopus-fleet`, `octopus-shadow-check`, `octopus-repo-mapper`, `octopus-grep-ast`, `octopus-code-analysis`, `octopus-phoenix-ir`, `octopus-continuous-learning`, `octopus-contract-verifier`, `octopus-security-scanner`, `octopus-supply-chain-verifier`, `octopus-replay-engine`, `octopus-migration-planner`, entre outros.

## Regra de licença

O workspace Octopus declara licença `Proprietary`, enquanto o PhxClaw Core Bootstrap declara `Apache-2.0`.

Portanto:

1. código Octopus não deve ser automaticamente copiado para o core Apache-2.0;
2. recursos Octopus podem ser consumidos por bridge/process plugin mantendo a licença separada;
3. uma futura migração de código para o core exige decisão explícita de relicenciamento pelo titular;
4. ideias, contratos e arquitetura podem ser reimplementados clean-room no PhxClaw.

## Recursos de alto valor

### 1. Repo Mapper

O Octopus já possui mapeamento/ranking de repositório com orçamento de tokens. Isso encaixa em `phxclaw-repo-intelligence` e no Context Compiler.

Destino recomendado: plugin `repo-intelligence`.

### 2. Grep AST

Pesquisa estrutural por código é superior a grep textual para refatoração e análise de impacto.

Destino recomendado: extension point `code.query.ast`.

### 3. Code Analysis

O analisador observado combina clean code, SOLID, bugs prováveis, performance, arquitetura, security scan, quality gate e repo map.

Destino recomendado: plugin de análise com evidências tipadas, sem acoplamento ao Orchestrator.

### 4. Shadow Check

É um conceito importante para validar código antes de merge/release.

Destino recomendado: gate plugável antes de `Objective Proof`.

### 5. Contract Verifier

Comparação de contrato entre implementação original e migrada é central para o PhxClaw/Phoenix Octopus.

Destino recomendado: `objective-proof.contract-equivalence`.

### 6. Polyglot Benchmark / Benchmark Engine

Pode medir desempenho e equivalência entre implementações.

Destino recomendado: plugin de benchmark, nunca dentro do microkernel.

### 7. Security Scanner + Supply Chain Verifier

Devem ser reutilizados como providers do Security Pipeline.

Destino recomendado: plugins com gates obrigatórios configuráveis por política.

### 8. Migration Planner

Recurso diretamente alinhado à fábrica de modernização de legado.

Destino recomendado: `migration.plan` com saída Task Graph.

### 9. Replay Engine

Muito valioso para depuração determinística e auditoria.

Destino recomendado: integração com Event Bus + Evidence Ledger.

### 10. Context Manager

O Octopus contém compactação e orçamento de contexto. O PhxClaw já possui Memory/Context Engine; portanto não deve duplicar. O correto é importar estratégias úteis como políticas de compactação plugáveis.

### 11. Fleet / subagentes paralelos

O conceito é útil, porém o PhxClaw deve executá-lo através do Team Runtime + Task Graph, com lease, fencing token, heartbeat, retry e Evidence Ledger.

### 12. Continuous Learning

Não deve ser ligado diretamente. Toda aprendizagem deve passar por:

`Candidate Skill -> Hypothesis -> Test -> Objective Proof -> QA -> Promotion`.

### 13. Phoenix IR / Knowledge Compiler

É um dos blocos mais estratégicos. A documentação Octopus define fontes imutáveis, Semantic IR, Knowledge Graph, Wiki versionada, claims com evidências, contradições e human-in-the-loop.

Destino recomendado: F25 Knowledge/Evidence Graph + futuros frontends de compilador.

## O que NÃO trazer diretamente

- CLI monolítico com dezenas de comandos dentro de um único binário.
- UUID v4 para identidade persistente; PhxClaw mantém UUIDv7.
- mocks como fallback silencioso de produção.
- acesso direto a shell sem Capability Broker.
- duplicação de Ollama, Model Gateway, Event Bus ou Context Engine já existentes no PhxClaw.
- dependências com licença incompatível sem declaração explícita.

## Estratégia adotada na v0.9

Foi criada uma integração separada:

`phxclaw-octopus-bridge`

Ela:

- chama uma instalação externa do `octopus-console`;
- possui allowlist fixa de capabilities;
- não aceita shell arbitrário;
- confina caminhos ao workspace configurado;
- é executável como plugin de processo;
- pode passar pelo Sandbox;
- pode ser roteada pelo Extension Host;
- produz Event Bus + Evidence Ledger quando invocada pelo host;
- preserva separação de licença.

Capabilities iniciais:

- `octopus.repo.map`
- `octopus.repo.grep_ast`
- `octopus.code.analyze`
- `octopus.quality.evaluate`
- `octopus.security.scan`
- `octopus.migration.plan`
- `octopus.contract.verify`
- `octopus.shadow.check`
- `octopus.dependency.map`
- `octopus.knowledge.search`
- `octopus.toolchain.status`

## Próxima migração recomendada

A ordem de absorção é:

1. Repo Mapper + Grep AST.
2. Code Analysis + Quality Gate.
3. Contract Verifier + Shadow Check.
4. Replay Engine.
5. Migration Planner.
6. Security/Supply Chain.
7. Phoenix IR + Knowledge Compiler.
8. Continuous Learning, somente após Promotion Gate completo.
