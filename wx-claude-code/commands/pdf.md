---
description: "Converte um PDF em Markdown citavel: uma secao por pagina, hash no cabecalho, e pagina sem texto marcada OCR em vez de inventada."
argument-hint: "<arquivo.pdf> [wlanguage|php|sql]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# PDF para Markdown

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/pdf_para_markdown.py" \
  --pdf "$1" --saida .wx-migration/extraidos --linguagem "${2:-nenhuma}" --project-root .
```

Depois de converter:

1. **Diga o que veio**: páginas, quantas ficaram `OCR_REQUERIDO`, quantos segredos foram omitidos. Tudo isso sai no `.json` ao lado do `.md`.
2. **Não descreva página marcada OCR.** Ela não tem texto; inventar o conteúdo contamina toda regra que sair dali. Peça OCR ou o arquivo em outro formato.
3. **Indexe** com `/wx-claude-code:rag` para que a busca ache o conteúdo por `arquivo#linha`.
4. Se o PDF veio do cliente por fora, **arquive o original** com `/wx-claude-code:artefato`, dizendo onde ele deve ser usado.

Ao citar, use a página: `estoque-codigo.pdf#p12`. A skill `pdf-para-markdown` tem o resto, inclusive o que o markdown não resolve (tabela e coluna dupla).
