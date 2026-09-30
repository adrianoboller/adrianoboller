# F07 — Model Gateway

## Regra de arquitetura

LLMs não entram no core. Cada integração é um plugin `model_provider`.

## Provider profile

O manifesto declara:

- display name;
- priority;
- transport;
- modelos;
- capabilities por modelo;
- context window;
- custo de input/output em microunits por 1M tokens;
- flag `local`.

## Router

O request contém:

- capability;
- input;
- estimativa de tokens;
- output token limit;
- teto de custo;
- preferred models;
- `requires_local`;
- data classification;
- budget account.

Dados `restricted` não são enviados a providers não locais.

## Budget

O gateway separa:

- ceiling;
- reserved;
- spent.

O fluxo é `route → reserve → execute → settle`.

## Fallback

A decisão retorna o provider/model selecionado e a lista ordenada de fallbacks. Fallback não ignora política de privacidade nem budget.

## Provider de desenvolvimento

`com.phxclaw.model-provider.local-deterministic` valida o contrato localmente, sem rede e sem API keys. Não é um modelo de produção.
