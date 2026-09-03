# Rastreabilidade

Use `templates/traceability.csv` como contrato mínimo.

## Relação obrigatória

```text
evidência → regra/comportamento → decisão → componente alvo → teste → resultado
```

Um item pode ter várias evidências e testes; use linhas separadas com o mesmo `trace_id` e localizadores distintos.

## Colunas

- `trace_id`: identificador estável (`BR-*`, `UI-*`, `QRY-*`, `DB-*`, `INT-*`, `RPT-*`, `NFR-*`), coerente com `kind`.
- `kind`: `business_rule`, `ui`, `query`, `database`, `integration`, `report` ou `non_functional`.
- `source_artifact`: caminho relativo ao conjunto de evidências.
- `source_locator`: página, linha, JSON Pointer, símbolo, controle ou região da imagem.
- `source_sha256`: hash da evidência efetivamente analisada.
- `legacy_symbol`: nome no WX, quando houver.
- `rule_summary`: comportamento verificável, sem solução técnica embutida.
- `decision_id`: `DEC-*` quando houve escolha humana ou arquitetural.
- `target_component`, `target_file`, `target_symbol`: destino implementado.
- `test_id`, `test_file`: prova automatizada ou roteiro reproduzível.
- `expected`, `actual`: resultados comparados ou referências aos arquivos grandes.
- `target_commit`: commit que contém a implementação verificada.
- `test_result_ref`: relatório imutável do teste/ensaio.
- `approved_by`, `approved_at`: identidade e data do aceite humano; obrigatórios em `accepted`.
- `status`: `inventoried`, `specified`, `implemented`, `verified`, `accepted` ou `blocked`.
- `confidence`: `high`, `medium` ou `low`, baseada na evidência, não na confiança do agente.
- `notes`: lacunas, exceções e riscos.

## Confiança

- `high`: fontes independentes concordam e há teste reproduzível.
- `medium`: evidência direta existe, mas há cobertura parcial ou teste incompleto.
- `low`: OCR incerto, documento indireto, comportamento não executável ou regra ainda não confirmada.

Itens `verified` ou `accepted` não podem ter confiança `low`, resultado ausente ou teste ausente.

Um CSV sem linhas é inválido. O validador também deve receber inventário e raiz do projeto para comprovar caminhos, e uma lista de IDs esperados quando o gate exigir cobertura total.

## Decisões e lacunas

Cada `DEC-*` registra pergunta, alternativas, escolha, autor, data, impacto e evidências. Cada `GAP-*` registra escopo afetado, severidade, artefato necessário, responsável e condição de desbloqueio.
