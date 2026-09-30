# PhxClaw v0.27 — Multi-Platform Qualification + GA

A v0.27 não transforma ausência de ferramenta em sucesso. Cada plataforma precisa executar os testes no sistema operacional nativo, produzir logs, assinar a evidência e apontar para o mesmo RC/source-state.

## Targets obrigatórios

- Linux x86_64 — GitHub runner de referência: `ubuntu-24.04`.
- Windows x86_64 — runner: `windows-2025`; Authenticode é verificado com política `/pa`, timestamp e allowlist de thumbprints.
- macOS ARM64 — runner: `macos-15`; o artefato GA exigido nesta versão é um **DMG** com Developer ID permitido, Gatekeeper aprovado e ticket de notarização validável por `stapler`.

## N/N-1

Quando não for o primeiro release, as três plataformas precisam comprovar `fresh_install`, `upgrade_n_minus_1` e `rollback_or_restore`. A versão anterior é parte da evidência assinada e deve ser a mesma nas três plataformas.

## Update seguro

O manifest de update é separado do GA, mas seu hash entra no manifest GA assinado. Cada target contém plataforma, arquitetura, URL HTTPS, SHA-256 e tamanho. Clientes rejeitam sequência não crescente, metadata expirada e downgrade sem token de rollback separado.

## Trust boundaries

O comando de assinatura (`PHXCLAW_RELEASE_SIGN_CMD`) deve apontar para HSM/KMS/key service de release; a chave privada não deve entrar no repositório. Windows usa `PHXCLAW_WINDOWS_SIGNER_THUMBPRINTS`; macOS usa `PHXCLAW_MACOS_TEAM_IDS`.

GitHub Artifact Attestations/Sigstore podem ser adicionadas como defesa adicional. Elas não substituem a cadeia Ed25519 do PhxClaw nem os gates nativos.
