---
description: "Conversao do projeto WX por gates G0-G7: pre-flight, inventario, especificacao, arquitetura, piloto, ondas e cutover."
argument-hint: "[inventario|plano|piloto|completo] [raiz-do-projeto]"
allowed-tools: "Read, Glob, Grep, Bash, Write, Edit, Agent, AskUserQuestion"
---

# Converter projeto WX

> **Identificação obrigatória.** Comece toda resposta com a linha que o PMO fornece: `BlocoNNNN-SPNNNNN-Título · data` (`pmo.py identificacao`). O hook a injeta a cada interação; se não vier, gere-a antes de responder. Sem PMO iniciado, escreva `Bloco0000-SP00000-Sem PMO iniciado · data`.

> **Licença.** Se o contexto da sessão disser que o WX Claude Code está sem licença válida, pare aqui: explique o estado (`licenca.py verificar`) e como instalar o serial (`licenca.py instalar`). Não tente contornar o hook.

Pré-requisito: `<projeto>/.wx-migration/questionario.json` existe. Se não existir, pare e execute `/wx-claude-code:questionario` primeiro — o questionário é quem diz quais anexos foram entregues e para onde o projeto vai.

Carregue a skill `conversao-wx` (`${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/SKILL.md`) e siga-a do **Gate G0** em diante. As respostas do questionário substituem a rodada de perguntas da seção 1 da skill; refaça só as perguntas cujo item continue em aberto.

`$ARGUMENTS`: o primeiro termo é o modo (`inventario`, `plano`, `piloto`, `completo`), o segundo é a raiz do projeto. Sem modo, use o gravado em `conversion.config.json`.

Regras que este comando não afrouxa:

- anexo é evidência quando lido, não quando citado;
- o Help WLanguage é fonte de semântica técnica, não de regra de negócio;
- conflito entre evidências para o item e pergunta; não escolhe a versão conveniente;
- piloto vertical (G4) nunca é pulado numa conversão completa;
- quem implementa não aprova o próprio gate: o `quality-auditor` recomenda, o humano decide.

Scripts determinísticos que o G1 e o G4 usam, antes de qualquer agente opinar:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/extrair_pdf.py" --manifest <projeto>/.wx-migration/wx-inputs.manifest.json --allowed-evidence-root <anexos> --output <projeto>/.wx-migration/evidence/pdf-text
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/golden.py" capturar --casos <resultados-esperados.json> --saida <projeto>/.wx-migration/tests/golden-master/casos.json
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/golden.py" comparar --golden <casos.json> --comando "<executa o novo sistema>" --relatorio <projeto>/.wx-migration/tests/results/<data>.json
```

O texto extraído leva `arquivo#page=N` e o hash do PDF; página com pouco texto vira `OCR_REQUIRED`. O golden devolve `equivalência: n/total` com tolerância declarada.

O hook `portao_g0.py` do plugin nega qualquer `Write`/`Edit` fora de `.wx-migration/` enquanto o último pré-flight estiver `BLOCKED`; não tente contornar, resolva o G0.

Delegue ao agente `wx-claude-code:wx-orchestrator` com: caminho do manifesto, modo, resultado do pré-flight e o `questionario.json`.
