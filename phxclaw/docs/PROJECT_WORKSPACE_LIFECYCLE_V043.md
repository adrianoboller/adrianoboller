# PhxClaw v0.43 — Project Workspace & Lifecycle Manager

## Objetivo
Criar, importar, abrir, reabrir e versionar projetos com fontes, logs, artefatos e backups separados.

## Layout padrão
```text
workspace/
  projects/<slug>/
    project.phx.json
    src/
    docs/
    tests/
    assets/
    logs/
    artifacts/
    reports/
    .phx/runtime/
    .phx/evidence/
    .phx/versions/
  backups/<project_uuid>/<timestamp>/
```

Backups ficam fora do diretório de fontes por padrão. Abrir versão histórica é read-only ou em novo Git worktree.
