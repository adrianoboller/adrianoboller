# ADR-0061 — Safe Source Harvester

## Decisão
Toda fonte externa passa por duas barreiras antes de poder alimentar conhecimento reutilizável: **Provenance & License Firewall** e **Safe Source Harvester**.

## Regras invariantes
- Nunca executar build, install, scripts, macros, binaries ou hooks da fonte durante harvesting.
- `DENY`: não ler o corpus e não copiar conteúdo para o vault.
- `QUARANTINE`: pode preservar conteúdo em vault segregado, mas gera **zero candidatos de conhecimento**.
- `ALLOW`: ainda passa por limites, symlinks, binários e detecção de segredos.
- Qualquer segredo/limite estrutural relevante rebaixa a execução para `QUARANTINE` e remove candidatos provisórios.
- Candidatos entram somente como `raw_observation`/`unverified`; não viram política, decisão ou conhecimento governado automaticamente.
- Promoção governada exige autoridade `system` ou `human` e evidência persistida.
- Receipts e arquivos de harvesting são append-only.

## Fluxo
`Research Core -> Source Registry -> Provenance Firewall -> Safe Source Harvester -> ALLOW-only Candidate -> Knowledge/Evidence Graph -> Promotion Gate`.

## Threat model
Mitiga contaminação de RAG, supply-chain por scripts de instalação, symlink escape, vazamento acidental de chaves, ingestão de artefatos gigantes, duplicação não rastreável e promoção automática de observações para fatos governados.
