# ADR-0001 — Microkernel + Plugins declarados por JSON

**Status:** Accepted  
**Version:** 0.3.0

## Decisão

1. Rust é a linguagem do núcleo e da execução determinística.
2. JSON descreve contratos, manifests, configuração e estado intercambiável.
3. PostgreSQL é a persistência oficial.
4. UUIDv7 é a identidade universal persistente.
5. Research Core, Hypothesis Core e Installer Core fazem parte do núcleo inicial.
6. Demais capacidades entram por plugins e não podem exigir alteração do core.
7. Todo plugin exige semver, dependências, compatibilidade, capabilities/permissões, lifecycle, contratos, assinatura/hash/proveniência, testes e rollback.
8. A política padrão de permissão é `deny`.
9. Plugins executáveis devem ser isolados/sandboxed e não podem quebrar a API pública.
10. Agentes são plugins; Agent Runtime é camada de plataforma, não expansão do microkernel.

## Consequências

- O crescimento da plataforma ocorre por extensão, não por acoplamento ao kernel.
- Compatibilidade e rollback são gates de release.
- Plugins inválidos falham fechados e entram em quarentena antes de qualquer ativação.
- A identidade/persona de um agente não concede privilégios; capabilities e permissions são explícitas no manifesto.
