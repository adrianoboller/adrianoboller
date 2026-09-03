---
description: "Estilo das telas convertidas com o Impeccable: paleta, tema e tipografia viram PRODUCT.md e DESIGN.md."
argument-hint: "[raiz-do-projeto] [--preservar|--redesenhar]"
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, AskUserQuestion"
---

# Estilo das telas com o Impeccable

Este comando responde à letra **F** do questionário. Ele define **uma vez** o sistema visual do projeto novo, para que cada tela convertida do WX nasça dentro dele em vez de cada agente inventar a sua.

## Entradas

1. `<projeto>/.wx-migration/questionario.json`, campo `F` (paleta, tema, tipografia, densidade, marca, preservar ou redesenhar). Se o campo estiver vazio, pergunte agora com `AskUserQuestion`, uma rodada por item.
2. O PDF de interfaces (letra **C**) ou o completo (letra **E**) e os screenshots, se existirem: são a evidência do que o WX mostra hoje. Estado de tela, navegação e campos vêm daí; a **cor** vem da resposta F.

## Passos

1. Carregue a skill `impeccable` (`${CLAUDE_PLUGIN_ROOT}/skills/impeccable/SKILL.md`) e rode o contexto uma vez por sessão:

```bash
node "${CLAUDE_PLUGIN_ROOT}/skills/impeccable/scripts/context.mjs"
```

2. Execute `init` do Impeccable com o que o questionário já sabe (público, propósito, plataformas de **I**, restrições). Não pergunte de novo o que já foi respondido. Isso escreve `PRODUCT.md`.
3. Escreva `DESIGN.md` a partir do esboço que `aplicar_questionario.py` deixou: tokens de cor (principal, secundária, fundo, texto, ação, erro, aviso, sucesso), tema (claro, escuro, ambos), família tipográfica com **fallback real**, escala de espaçamento e densidade. Confira contraste mínimo **4,5:1** em texto e diga o valor medido, não «parece bom».
4. Se a resposta for **preservar**, o modo é *Operate* e `fidelity.ui = behavioral`: a tela nova faz o que a do WX faz, com a mesma ordem de campos e os mesmos estados. Se for **redesenhar**, `fidelity.ui = redesign` e o visual antigo vira anti-referência, mas conteúdo, campos, validações e fluxo continuam iguais.
5. Para cada tela convertida daqui em diante, use `/impeccable polish <tela>` antes de declarar a tela pronta e `/impeccable audit` no fim de cada onda (acessibilidade, responsivo, desempenho).

## Regras

- Paleta vem do usuário ou da marca dele; não invente uma. Sem resposta em F, não há `DESIGN.md`, há pergunta.
- Cor de ação segue uma convenção e ela fica escrita no `DESIGN.md` (exemplo já usado neste repositório: verde inclui, amarelo altera, vermelho exclui, azul consulta; sempre contorno, fundo só no `hover`).
- Texto de interface não muda a caixa do dado (`text-transform` em célula de tabela mente sobre o que está gravado).
- Componente novo se abre no navegador e se olha; ler o CSS não prova a tela.
- Anote no `DESIGN.md` a origem de cada decisão (`DEC-*` quando foi escolha humana).
