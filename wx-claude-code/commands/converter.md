---
description: "Conversao do projeto WX por gates G0-G7: pre-flight, inventario, especificacao, arquitetura, piloto, ondas e cutover."
argument-hint: "[inventario|plano|piloto|completo] [raiz-do-projeto]"
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, Agent, AskUserQuestion"
---

# Converter projeto WX

Pré-requisito: `<projeto>/.wx-migration/questionario.json` existe. Se não existir, pare e execute `/wx-claude-code:questionario` primeiro — o questionário é quem diz quais anexos foram entregues e para onde o projeto vai.

Carregue a skill `conversao-wx` (`${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/SKILL.md`) e siga-a do **Gate G0** em diante. As respostas do questionário substituem a rodada de perguntas da seção 1 da skill; refaça só as perguntas cujo item continue em aberto.

`$ARGUMENTS`: o primeiro termo é o modo (`inventario`, `plano`, `piloto`, `completo`), o segundo é a raiz do projeto. Sem modo, use o gravado em `conversion.config.json`.

Regras que este comando não afrouxa:

- anexo é evidência quando lido, não quando citado;
- o Help WLanguage é fonte de semântica técnica, não de regra de negócio;
- conflito entre evidências para o item e pergunta; não escolhe a versão conveniente;
- piloto vertical (G4) nunca é pulado numa conversão completa;
- quem implementa não aprova o próprio gate: o `quality-auditor` recomenda, o humano decide.

Delegue ao agente `wx-claude-code:wx-orchestrator` com: caminho do manifesto, modo, resultado do pré-flight e o `questionario.json`.
