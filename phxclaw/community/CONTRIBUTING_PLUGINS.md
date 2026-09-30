# Contribuindo com plugins PhxClaw

PhxClaw deve crescer por **contratos públicos estáveis**, não por patches no microkernel.

## Regra de arquitetura

Um plugin comunitário não recebe acesso irrestrito ao processo do core. A integração padrão é:

```text
manifest assinado
      ↓
Plugin Registry
      ↓
Extension Host
      ↓
capability + permission + policy
      ↓
sandbox/process protocol
      ↓
plugin
      ↓
result + Event Bus + Evidence Ledger
```

## Formato mínimo

Todo pacote `.phxplugin` precisa conter:

- `phxclaw.plugin.json`;
- executável/artefato declarado no manifesto;
- `README.md`;
- `LICENSE`;
- `SBOM.spdx.json`;
- `PACKAGE.json` com hashes;
- assinatura Ed25519 válida;
- provenance;
- testes obrigatórios;
- estratégia de rollback.

## Compatibilidade

Use `core_api` no manifesto e declare apenas extension points existentes em `config/extension-points.json`.

Mudanças incompatíveis em contratos públicos exigem nova major do contrato. O microkernel não deve importar diretamente código de plugins comunitários.

## Segurança

- permissões mínimas;
- rede negada por padrão;
- allowlist explícita quando houver egress;
- paths confinados;
- sem `shell -c`/`cmd /c` arbitrário no core;
- secrets apenas via Secret Broker;
- dados sensíveis não devem ser gravados integralmente no Evidence Ledger;
- falha de assinatura, hash, schema, capability ou policy = fail closed.

## Publicação

1. Execute testes locais.
2. Gere SBOM.
3. Compile um artefato reproduzível.
4. Atualize o digest do manifesto.
5. Assine com Ed25519 fora do repositório.
6. Execute `scripts/plugin_pack.py`.
7. Publique release imutável.
8. Abra a submissão ao Community Registry com licença, source repository e provenance.

Plugins podem ter licença própria compatível com sua distribuição. A licença do plugin não muda automaticamente a licença do PhxClaw Core.
