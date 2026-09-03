---
description: "Estilo das telas convertidas com o Impeccable: paleta, tema e tipografia viram PRODUCT.md e DESIGN.md."
argument-hint: "[raiz-do-projeto] [--preservar|--redesenhar]"
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, Agent, AskUserQuestion"
---

# Estilo das telas com o Impeccable

> **Identificação obrigatória.** Comece toda resposta com a linha que o PMO fornece: `BlocoNNNN-SPNNNNN-Título · data` (`pmo.py identificacao`). O hook a injeta a cada interação; se não vier, gere-a antes de responder. Sem PMO iniciado, escreva `Bloco0000-SP00000-Sem PMO iniciado · data`.

> **Licença.** Se o contexto da sessão disser que o WX Claude Code está sem licença válida, pare aqui: explique o estado (`licenca.py verificar`) e como instalar o serial (`licenca.py instalar`). Não tente contornar o hook.

Este comando responde à letra **F** do questionário. Ele define **uma vez** o sistema visual do projeto novo, para que cada tela convertida do WX nasça dentro dele em vez de cada agente inventar a sua.

## Entradas

1. `<projeto>/.wx-migration/questionario.json`, campo `F` (paleta, tema, tipografia, densidade, marca, preservar ou redesenhar). Se o campo estiver vazio, pergunte agora com `AskUserQuestion`, uma rodada por item.
2. O PDF de interfaces (letra **C**) ou o completo (letra **E**) e os screenshots, se existirem: são a evidência do que o WX mostra hoje. Estado de tela, navegação e campos vêm daí; a **cor** vem da resposta F.

## Passos

1. Carregue a skill `impeccable` (`${CLAUDE_PLUGIN_ROOT}/skills/impeccable/SKILL.md`) e rode o contexto uma vez por sessão:

```bash
node "${CLAUDE_PLUGIN_ROOT}/skills/impeccable/scripts/context.mjs"
```

2. Execute `init` do Impeccable a partir do `PRODUCT.md` que o questionário já deixou (F1: quem opera, horas, ambiente, tela típica; modo *Operate*) (público, propósito, plataformas de **I**, restrições). Não pergunte de novo o que já foi respondido. O `init` **atualiza** esse `PRODUCT.md`; não o recria.
3. Escreva `DESIGN.md` a partir do esboço que `aplicar_questionario.py` deixou: tokens de cor (principal, secundária, fundo, texto, ação, erro, aviso, sucesso), tema (claro, escuro, ambos), família tipográfica com **fallback real**, escala de espaçamento e densidade. Confira contraste mínimo **4,5:1** em texto e diga o valor medido, não «parece bom».
4. Se a resposta for **preservar**, o modo é *Operate* e `fidelity.ui = behavioral`: a tela nova faz o que a do WX faz, com a mesma ordem de campos e os mesmos estados. Se for **redesenhar**, `fidelity.ui = redesign` e o visual antigo vira anti-referência, mas conteúdo, campos, validações e fluxo continuam iguais.
5. Rode um comando do Impeccable por seção respondida do `DESIGN.md`, nesta ordem, e nenhum para seção sem resposta (aí é pergunta): `shape` nas grids (F3), `harden` em formulários e estados (F4, F7), `typeset` nos formatos (F5), `layout` na impressão (F6), `audit` na acessibilidade (F8), `adapt` para a tela típica de F1; `polish` e `colorize` nas tabelas de botões (F9, F11, F12) e no fundo (F13), medindo o contraste de cada cor de ação sobre o fundo escolhido e escrevendo o número na coluna da tabela. Registre o resultado de cada um como `DEC-*` ou `GAP-*`.
6. Para cada tela convertida daqui em diante, use `/impeccable polish <tela>` antes de declarar a tela pronta e `/impeccable audit` no fim de cada onda (acessibilidade, responsivo, desempenho).

## Regras

- Paleta vem do usuário ou da marca dele; não invente uma. Sem resposta em F, não há `DESIGN.md`, há pergunta.
- Cor de ação segue uma convenção e ela fica escrita no `DESIGN.md` (a tabela F12 do `DESIGN.md`, gravada pelo questionário: uma cor por ação, contorno por padrão; não invente outra lista aqui).
- Texto de interface não muda a caixa do dado (`text-transform` em célula de tabela mente sobre o que está gravado).
- Componente novo se abre no navegador e se olha; ler o CSS não prova a tela.
- Anote no `DESIGN.md` a origem de cada decisão (`DEC-*` quando foi escolha humana).
