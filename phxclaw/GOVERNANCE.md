# PhxClaw Governance

## Princípio

O core é pequeno; a plataforma cresce por plugins assinados e contratos versionados.

## Áreas

- **Core maintainers**: contratos públicos, segurança, releases e compatibilidade.
- **Plugin maintainers**: código, licença, SBOM, testes e suporte de cada extensão.
- **Community reviewers**: revisão técnica, segurança, documentação e compatibilidade.

## Mudanças no core

Uma mudança só entra no microkernel quando não puder ser implementada de forma segura como plugin ou quando definir um contrato transversal indispensável.

Mudanças de API pública devem incluir:

1. ADR;
2. impacto de compatibilidade;
3. migration/adapter quando possível;
4. testes de contrato;
5. plano de rollback;
6. período de depreciação para contratos estáveis.

## Plugins

Nenhum publisher comunitário ganha confiança automática. Trust, instalação e permissões são decisões separadas.


## PRIVATE REPOSITORY POLICY

PhxClaw source repositories are private-only. Public GitHub repositories, public issue trackers, and automatic public publishing are prohibited unless the Product Owner explicitly changes this policy. Community growth must use private repositories/mirrors, reviewed patches, and signed `.phxplugin` packages or a private registry.
