# PhxClaw v0.9 — Arquitetura de Plugins e Comunidade

## Objetivo

Permitir que o PhxClaw cresça com contribuições da comunidade sem transformar o microkernel em um monólito ou permitir que plugins tenham acesso irrestrito ao host.

## Princípio central

**Core pequeno, contratos estáveis, extensões isoladas.**

```text
PhxClaw Core
    |
    +-- Plugin Registry
    +-- Extension Host
    +-- Capability Broker
    +-- Event Bus
    +-- Evidence Ledger
    +-- Sandbox
    |
    +-- Process plugins
    +-- WASM plugins
    +-- Remote providers
```

Rust dynamic libraries não são o contrato principal da comunidade, porque ABI Rust não é estável entre toolchains/versões. O formato preferido é **Process Protocol JSONL** e, futuramente, WASM/WASI para plugins portáveis.

## Extension Host

O novo `phxclaw-extension-host` liga:

```text
Plugin Registry
      ↓
Capability Route
      ↓
Permission Check
      ↓
Sandbox
      ↓
Process Protocol
      ↓
Plugin
      ↓
Event Bus
      ↓
Evidence Ledger
```

### Roteamento fail-closed

Se apenas um plugin habilitado fornece uma capability, ele pode ser selecionado.

Se dois ou mais fornecem a mesma capability, o host não escolhe silenciosamente. A capability precisa ser **pinned** para um UUID específico.

Isso evita que um plugin recém-instalado capture uma capability crítica por acidente ou malícia.

## Plugin SDK

`phxclaw-plugin-sdk` define contratos comuns para:

- handshake;
- capability invocation;
- result;
- extension points;
- subscriptions;
- health.

Toda mensagem possui UUIDv7/correlation UUID.

## Extension Points

O manifesto agora pode declarar `extension_points` opcionais e versionados.

Categorias previstas:

- capability provider;
- event subscriber;
- UI panel;
- compiler frontend;
- storage provider;
- data provider;
- model provider;
- channel;
- device node.

## Pacote `.phxplugin`

Formato de distribuição:

```text
plugin.phxplugin
├── PACKAGE.json
├── *.plugin.json
├── README.md
├── LICENSE
├── SBOM.spdx.json
├── bin/
├── schemas/
└── resources/
```

Requisitos de publicação:

1. UUIDv7.
2. semver.
3. `core_api` semver range.
4. SHA-256 do artefato.
5. SHA-256 do pacote.
6. assinatura Ed25519.
7. signer conhecido.
8. provenance.
9. LICENSE.
10. SBOM.
11. testes obrigatórios.
12. rollback.
13. capabilities e permissions explícitas.
14. sandbox policy.

## Níveis de confiança

- `builtin`: mantido junto do core.
- `verified`: publisher verificado.
- `community`: publisher comunitário com assinatura e provenance.
- `local_development`: desenvolvimento local; nunca deve virar trusted automaticamente.

Mesmo plugin comunitário precisa de assinatura. Community não significa unsigned.

## Community Registry

`phxclaw-community-registry` valida metadados publicados sem executar o plugin.

O registry público contém apenas metadados e hashes. A instalação ainda passa por:

```text
Download
  ↓
SHA-256
  ↓
Manifest validation
  ↓
Ed25519 signature
  ↓
Publisher trust
  ↓
Dependency resolution
  ↓
Quarantine scan
  ↓
Tests
  ↓
Install disabled
  ↓
User/Policy enable
```

## Segurança de supply chain

Um pacote não deve ser habilitado só porque está no marketplace.

Gates mínimos:

- hash;
- assinatura;
- provenance;
- SBOM;
- dependências;
- malware/static scan;
- permissions diff;
- capability collision detection;
- sandbox test;
- health test;
- rollback validation.

## Governança da comunidade

Recomendação para repositórios públicos:

```text
phxclaw/core
phxclaw/sdk-rust
phxclaw/plugin-examples
phxclaw/registry-index
phxclaw/docs
```

Um plugin não precisa alterar o core para adicionar uma capability.

Pull requests no core ficam restritos a contratos, segurança, runtime e APIs públicas. Funcionalidades de domínio devem preferencialmente nascer como plugin.

## Compatibilidade

Todo plugin declara:

- `manifest_version`;
- `version`;
- `core_api`;
- versões dos extension points.

Mudanças incompatíveis exigem major version.

## Octopus como primeiro caso real

O `Phoenix Octopus` é utilizado como caso de teste da arquitetura de integração. Ele permanece separado e é chamado pelo `phxclaw-octopus-bridge` por capability allowlisted.

Isto prova que um sistema grande pode ser conectado ao PhxClaw sem inflar o microkernel.
