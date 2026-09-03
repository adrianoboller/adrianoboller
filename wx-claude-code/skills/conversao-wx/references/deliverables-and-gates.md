# Entregáveis e gates

## Estrutura de trabalho

```text
.wx-migration/
  wx-inputs.manifest.json
  conversion.config.json
  preflight/
  evidence/
    originals.sha256
    inventory.csv
    help-index.jsonl
    pdf-text/
  specifications/
    business-rules.md
    ui-flows.md
    data-and-queries.md
    integrations.md
  architecture/
    target-architecture.md
    adr/
  decisions/
  gaps.md
  traceability.csv
  tests/
    golden-master/
    results/
  logs/
  gate-status.md
```

## Critérios

| Gate | Entrada | Saída obrigatória | Aprovação |
| --- | --- | --- | --- |
| G0 Intake | respostas + manifesto | relatório `READY/CONDITIONAL/BLOCKED`, gaps e escopo | responsável pelos anexos |
| G1 Evidências | G0 apto | inventário, hashes, texto/index e mapa de cobertura | líder técnico |
| G2 Especificação | G1 | regras, UI, dados, integrações, conflitos e testes de aceite | responsável de negócio |
| G3 Arquitetura | G2 | ADRs, plano de ondas, riscos, rollback e piloto | arquitetura/produto |
| G4 Piloto | G3 | fatia executável e comparação legado × alvo | técnico + negócio |
| G5 Implementação | G4 | módulos rastreados e testes verdes | líder técnico |
| G6 Hardening | G5 | segurança, privacidade, desempenho, concorrência, restore, relatórios e E2E | qualidade/operação |
| G7 Cutover | G6 | ensaio, reconciliação, aceite, rollback e suporte | patrocinador |

## Golden master

Capture entradas e saídas determinísticas do legado sempre que possível:

- linhas e agregados de consultas;
- cálculos monetários e fiscais;
- estados de UI e mensagens;
- arquivos, relatórios e payloads;
- efeitos de transações e falhas.

Normalize apenas valores voláteis documentados, como timestamp gerado e identificador aleatório. Uma diferença normalizada precisa de `DEC-*`.

Defina denominadores e metas: objetos classificados/total, regras P0/P1 verificadas/total, fluxos críticos aprovados/total, plataformas executadas/previstas, divergências por severidade, tolerâncias de reconciliação, limites de desempenho e número de ensaios. Percentual sem denominador é inválido.

## Reprovação automática

Um gate falha quando há:

- rastreabilidade quebrada;
- teste ignorado sem exceção aprovada;
- segredo ou dado pessoal em artefato gerado;
- alteração de schema sem migração/rollback;
- regra implementada sem evidência;
- evidência conflitante não resolvida;
- diferença de precisão, nulidade, ordenação ou transação não explicada;
- “funciona na minha máquina” sem comando e ambiente reproduzíveis.

## Relatório de gate

Registre data, escopo, build/configuração, ambiente, dataset, tolerâncias, commit, aprovadores, evidências, testes, métricas, lacunas, exceções com prazo/controle compensatório, riscos residuais e decisão humana `APPROVED | CONDITIONAL | REJECTED`.
