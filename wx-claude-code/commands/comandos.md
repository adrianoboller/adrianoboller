---
description: "Indice de tudo que o plugin faz: comandos, ids das perguntas do questionario, scripts e skills, com o que cada um resolve."
argument-hint: "[palavra para filtrar]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# O que este plugin sabe fazer

Se `$1` vier preenchido, filtre por essa palavra e mostre só o que casa; sem argumento, mostre tudo.

## Comandos

| comando | resolve |
| --- | --- |
| `/wx-claude-code:questionario` | o questionário inteiro: bloco 0, letras A a M. É por onde se começa |
| `/wx-claude-code:pergunta <id>` | **uma** pergunta, pelo id, sem refazer o resto |
| `/wx-claude-code:comandos` | este índice |
| `/wx-claude-code:converter` | a conversão em si, gate a gate (G0 a G7) |
| `/wx-claude-code:preflight` | só o G0: inventário das evidências, classificação, relatório |
| `/wx-claude-code:artefato` | submeter e catalogar artefato do cliente (bloco M) |
| `/wx-claude-code:estilo-telas` | qualidade de tela com o Impeccable, a partir do `DESIGN.md` |
| `/wx-claude-code:golden` | golden master: comparar a saída do destino com a do legado |
| `/wx-claude-code:pmo` | sprints, Kanban, PDCA, relatório de onze seções, entrega |
| `/wx-claude-code:equipe` | acionar um papel da equipe prioritária (zelador, pesquisador, documentador…) |
| `/wx-claude-code:ambiente` | instalar e conferir o ambiente pedido na letra K |
| `/wx-claude-code:help-wl` | consultar o corpus do Help WLanguage (12k) por tema |
| `/wx-claude-code:rag` | indexar e buscar nos documentos do projeto, com `arquivo#linha` |
| `/wx-claude-code:exportar` | salvar o projeto resultante, organizado, na pasta do usuário |
| `/wx-claude-code:zelador` | limpar temporários e medir espaço |
| `/wx-claude-code:licenca` | ativar por serial e conferir a licença |
| `/wx-claude-code:laudo-tokens` | laudo de uso de tokens |

## Perguntas do questionário

Rode e mostre a lista (ela sai do modelo, não de uma tabela escrita à mão):

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/listar_perguntas.py"
```

Cada id da saída é o argumento de `/wx-claude-code:pergunta <id>`.

## Scripts

Todos em `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/`. Rode com `--help` para o contrato de cada um:

```bash
ls "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/"*.py
```

## Skills

`conversao-wx` (o fluxo), `php-legado-e-destino` (PHP como origem e como destino), `impeccable` (qualidade de tela), `laudo-uso-tokens`, e as oito de ERP (`erp-accounting`, `erp-inventory`, `erp-brazil-fiscal`, `erp-multi-company`, `erp-approval-workflows`, `erp-lgpd`, `erp-integration-reliability`, `windev-wlanguage-erp`). Índice das de ERP em `skills/LEIA-ME-erp.md`.

## O que o plugin faz, em uma frase

Converte projetos **WINDEV, WEBDEV e WINDEV Mobile** — WLanguage — para outra linguagem, com evidência, gates e prova. Legado em **PHP ou em outra linguagem** entra junto ou sozinho; o destino pode ser qualquer linguagem. O WLanguage é o caso principal e não sai daqui.
