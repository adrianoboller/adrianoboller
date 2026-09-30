# PhxClaw Security Policy

## Reporte vulnerabilidades de forma privada

Não publique exploit funcional, secrets ou dados de usuários em issues públicas. O projeto deve disponibilizar um canal privado de segurança antes de abrir o registry à comunidade.

## Fronteiras obrigatórias

- deny-by-default;
- assinatura Ed25519 e SHA-256 de plugins;
- sandbox para código executável;
- egress governado;
- permissions/capabilities explícitas;
- Evidence Ledger com correlação UUIDv7;
- secrets nunca dentro do manifesto ou log comum;
- plugins não podem carregar código dentro do address space do core por ABI Rust instável.

## Supply chain

Releases comunitárias devem transportar licença, SBOM, provenance, hashes e assinatura verificável.


## PRIVATE REPOSITORY POLICY

PhxClaw source repositories are private-only. Public GitHub repositories, public issue trackers, and automatic public publishing are prohibited unless the Product Owner explicitly changes this policy. Community growth must use private repositories/mirrors, reviewed patches, and signed `.phxplugin` packages or a private registry.
