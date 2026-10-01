# Schema Diff v0.7 — arquivo SQL × banco vivo

## Objetivo

Comparar duas origens diferentes usando somente o contrato normalizado.

```text
Arquivo SQL -> Parser --------> Model A --+
                                      |    |
                                      | SchemaDiff -> DiffResult -> UI
                                      |    |
Banco vivo -> Introspector ---> Model B --+
```

## Compara

- tabela adicionada/removida;
- coluna adicionada/removida/alterada;
- tipo, nullability, default, PK, unique;
- generated/computed expression/kind;
- CHECK constraints;
- partitioning;
- objetos avançados por `kind + nome`;
- foreign keys.

## Não compara

Cardinalidade e tamanho de tabela são deliberadamente excluídos. Eles variam com carga e manutenção do banco e não representam alteração estrutural.

## Direção dos termos

Na UI:

- **só no vivo / adicionada** = existe no banco conectado e não no arquivo;
- **ausente no vivo / removida** = existe no arquivo e não no banco conectado;
- **alterada** = existe nos dois lados, mas o fingerprint estrutural difere.

## Próxima evolução possível

O `DiffResult` já permite uma futura camada separada `MigrationPlanner` gerar DDL de convergência sem colocar geração de SQL nos renderers.
