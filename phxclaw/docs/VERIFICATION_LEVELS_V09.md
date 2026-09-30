# PhxClaw — Verification Levels v0.9

Para impedir que "código escrito" seja confundido com "funciona em produção", PhxClaw usa cinco níveis:

1. `source_ready` — fonte/contrato existe.
2. `static_verified` — schemas, políticas e testes estáticos passaram.
3. `runtime_verified` — compilou e executou com dependências reais.
4. `e2e_verified` — missão completa passou sem mocks/test doubles.
5. `release_ready` — todos os gates obrigatórios passaram e nenhum gate requerido foi pulado.

`config/release-policy.json` proíbe mock providers, test doubles e SKIPs de gates obrigatórios em release.

O build atual neste ambiente **não pode** ser classificado como `release_ready`, porque `cargo/rustc`, PostgreSQL real, sandbox Bubblewrap E2E, provider real e Octopus compilado não estão disponíveis aqui.
