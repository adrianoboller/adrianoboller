# Fase v0.26 — Release Candidate Factory

## Fluxo

```text
v0.25 Qualification Run
        │
        ├─ 15 gates VERIFIED
        ├─ source-state stable
        └─ trusted Ed25519 attestation
                 │
                 ▼
        RC Eligibility Gate
                 │
     ┌───────────┼────────────┐
     ▼           ▼            ▼
 Source ZIP     SBOM      Artifact Manifest
     │           │            │
     ├───────────┼────────────┤
     ▼           ▼            ▼
 Provenance  Compatibility  Rollback
                 │
                 ▼
        Sprint Promotion Plan
                 │
                 ▼
       Signed RC Manifest
```

## Sprint state

A Factory não edita o status canônico do workspace. Ela deriva um snapshot de promoção. Cada sprint só aparece `green` no RC quando todos os gates definidos em `config/sprint-gate-map.v026.json` estão `verified` para o mesmo source-state. Um RC público exige ainda `release_ready=true` na attestation v0.25 e todos os 26 sprints verdes.

## SBOM

A Factory produz inventário SPDX 2.3 a partir de `Cargo.lock`, manifests do workspace e hashes dos artefatos. `NOASSERTION` não é convertido em licença aprovada: a aprovação de licença continua sendo responsabilidade do gate `license_sbom` da v0.25.

## Provenance

É emitido um in-toto Statement v1 com predicate SLSA provenance v1.0. O subject inclui cada artefato nativo e o source bundle. O builder é `phxclaw-release-candidate-factory/0.26.0`.

## Rollback

Release não inicial exige artefato da versão anterior. A Factory cria um bundle determinístico com manifesto, hash do release anterior e instruções fail-closed. Primeiro release precisa de `--first-release` explícito.

## Compatibilidade

A matriz separa gates qualificados do source-state, ambiente observado do builder e alvos de plataforma. Como a attestation v0.25 não grava criptograficamente OS/arquitetura, a v0.26 usa `rc_builder_attested` para o runner atual e `not_qualified` para os demais; nunca inventa uma qualificação de plataforma.

## Verificação independente

`tools/verify_release_candidate.py` recalcula hashes do source bundle, qualification-run, attestation, artifact manifest, SBOM, provenance, compatibility matrix, promotion plan, rollback bundle e binários; depois verifica a assinatura Ed25519 do RC contra um trust store externo.

## Arquivo final

A Factory também gera `PhxClaw-<semver>-RC.zip`. O SHA-256 externo desse arquivo aparece em `rc-factory-report.json`; o conteúdo interno é protegido pelo `release-candidate.json` assinado.

## Plan-only

`--plan-only` aceita uma attestation Ed25519 confiável ainda não release-ready, deriva o estado green/yellow por sprint e não cria RC. Ausência de gate permanece visível como `missing_gates`.
