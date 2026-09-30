# ADR-0002 — Trust Store, Ed25519 e Sandbox Fail-Closed

**Status:** Accepted  
**Version:** 0.3.0

## Contexto

Verificar apenas a presença de campos `signature` e `digest` não protege o runtime. O registry precisa comprovar que o artefato não foi alterado, que a assinatura foi produzida por uma chave confiável e que o plugin não executará fora de uma política explícita.

## Decisão

1. Artefatos são identificados por SHA-256.
2. Assinaturas usam Ed25519.
3. A mensagem assinada vincula UUID, nome, versão e SHA-256.
4. Chaves públicas ficam em trust store declarativo.
5. Cada signer possui namespaces permitidos.
6. Chaves privadas não fazem parte do pacote/runtime.
7. Qualquer falha de hash, assinatura, trust, dependência ou policy envia o plugin para quarentena.
8. Plugins `process` executam somente por backend de sandbox aprovado.
9. Ausência do backend não habilita fallback inseguro.
10. Rede é negada por padrão; allowlist dependerá de proxy dedicado.

## Mensagem de assinatura

```text
PHXCLAW-PLUGIN-V1
uuid=<UUIDv7>
name=<plugin-name>
version=<semver>
sha256=<artifact-sha256>
```

## Consequências

- Manifesto e artefato ficam criptograficamente ligados.
- Rotação de trust roots pode ser feita sem alterar o kernel.
- Ambientes sem sandbox permanecem fail-closed.
- O desenvolvimento local pode usar uma root de desenvolvimento, mas produção deve usar chaves gerenciadas offline/HSM/KMS.
