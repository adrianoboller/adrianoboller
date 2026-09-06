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
| `/wx-claude-code:progresso` | onde o questionário parou: retomar, o que falta, reabrir um item |
| `/wx-claude-code:comandos` | este índice |
| `/wx-claude-code:converter` | a conversão em si, gate a gate (G0 a G7) |
| `/wx-claude-code:interface` | a forma do Rust final (terminal, serviço, web, mobile, IoT…) e o suporte medido |
| `/wx-claude-code:preflight` | só o G0: inventário das evidências, classificação, relatório |
| `/wx-claude-code:artefato` | submeter e catalogar artefato do cliente (bloco M) |
| `/wx-claude-code:dependencias` | o que o legado usa de fora: INI, banco, DLL, COM, webservice, e-mail, FTP |
| `/wx-claude-code:pdf` | converter um PDF em Markdown citável, com página e hash |
| `/wx-claude-code:log` | ver o registro das operações do plugin neste projeto |
| `/wx-claude-code:estilo-telas` | qualidade de tela com o Impeccable, a partir do `DESIGN.md` |
| `/wx-claude-code:golden` | golden master: comparar a saída do destino com a do legado |
| `/wx-claude-code:constraints` | as restrições do projeto e o portão C-GATE (está conforme?) |
| `/wx-claude-code:evidencia` | livro de evidências: o que foi provado, com estado e limite |
| `/wx-claude-code:efeito` | conferir o efeito real de uma ação, não o código de saída dela |
| `/wx-claude-code:grafo` | o grafo de rastreabilidade e as lacunas que ele acha |
| `/wx-claude-code:procedencia` | SLSA e BOM CycloneDX da entrega, com o que não afirmam |
| `/wx-claude-code:replay` | a decisão com a base dela; reconfere se a base ainda vale |
| `/wx-claude-code:gemeo` | fotografia da sprint e o «e se» sobre aquele estado |
| `/wx-claude-code:telemetria` | OTLP/JSON do registro, no disco; enviar é explícito |
| `/wx-claude-code:identidade` | SPIFFE assinado por papel e o atestado da máquina |
| `/wx-claude-code:contrato` | o contrato ativo: o que vale hoje, separado do histórico |
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

`conversao-wx` (o fluxo), `php-legado-e-destino` (PHP como origem e como destino), `pdf-para-markdown` (PDF citável), `impeccable` (qualidade de tela), `laudo-uso-tokens`, e as oito de ERP (`erp-accounting`, `erp-inventory`, `erp-brazil-fiscal`, `erp-multi-company`, `erp-approval-workflows`, `erp-lgpd`, `erp-integration-reliability`, `windev-wlanguage-erp`). Índice das de ERP em `skills/LEIA-ME-erp.md`.

## O que o plugin faz, em uma frase

Converte projetos **WINDEV, WEBDEV e WINDEV Mobile** — WLanguage — para outra linguagem, com evidência, gates e prova. Legado em **PHP ou em outra linguagem** entra junto ou sozinho; o destino pode ser qualquer linguagem. O WLanguage é o caso principal e não sai daqui.
