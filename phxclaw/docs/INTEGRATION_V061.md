# PhxClaw v0.61 — Integration

Crate: `phxclaw-safe-source-harvester`.

Entrada obrigatória: UUID do tenant, UUID do artefato de origem, SHA-256 do artefato, diretório da fonte, vault de destino, ator e `ProvenanceClaim`.

Saída: `HarvestReceipt` determinístico/auditável, arquivos content-addressed e candidatos de conhecimento somente quando `final_decision == ALLOW`.

O crate não possui `Command`, `process::Command`, FFI nem `unsafe`.
