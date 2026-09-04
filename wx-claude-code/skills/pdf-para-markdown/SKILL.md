---
name: pdf-para-markdown
description: "Converte PDF em Markdown sem perder o localizador de página, marcando o que é imagem em vez de inventar texto. Use antes de citar qualquer PDF."
metadata:
  short-description: PDF vira .md com página, hash e OCR marcado
  origem: escrita para o plugin WX Claude Code em 4 de setembro de 2026
---

# PDF para Markdown

O PDF é o formato em que quase toda evidência chega: código exportado do IDE, telas, queries, manual, modelo de relatório, contrato. Ler PDF direto, página a página, gasta contexto e perde a origem. Esta skill converte o PDF em `.md` **uma vez**, e o `.md` fica citável.

## Quando usar

- Antes de citar qualquer coisa de um PDF: a citação precisa de página, e o `.md` guarda a página.
- Quando o mesmo PDF vai ser consultado várias vezes (o caso normal: o PDF do código é aberto em todo gate).
- Quando o PDF vai alimentar o RAG do projeto — o índice lê `.md`, não PDF.

**Não use** para o inventário formal do G1: ali quem manda é `extrair_pdf.py`, que grava página a página com hash e alimenta a cadeia de evidência. Esta skill é para leitura e trabalho; a outra é para prova.

## Como converter

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/pdf_para_markdown.py" \
  --pdf inputs/estoque-codigo.pdf --saida .wx-migration/extraidos \
  --linguagem wlanguage --project-root .
```

`--linguagem` (`wlanguage`, `php`, `sql`, `nenhuma`) só decide o que vira bloco de código. Erra para menos de propósito: código que ficou como texto ainda se lê; texto embrulhado em bloco de código atrapalha.

Sai um `.md` e um `.json` com o resumo. O `.md` traz, no cabeçalho: nome do PDF, **SHA-256**, número de páginas, data e ferramenta.

## As três regras que fazem o resultado valer

1. **Página sempre.** Cada página vira uma seção com `<!-- pagina N -->` e `## Página N`. Ao citar, escreva `estoque-codigo.pdf#p12` ou `estoque-codigo.md:120`. Citação sem página é opinião.
2. **Página sem texto não vira texto.** PDF escaneado, ou página que é uma imagem, fica marcada `OCR_REQUERIDO` com o número de caracteres encontrados. **Não descreva o que você imagina que está ali** — peça OCR ou o arquivo em outro formato. Uma página inventada contamina toda regra que sair dela.
3. **Segredo não passa.** Token e chave privada saem substituídos por `<segredo omitido>`, e o cabeçalho diz quantas vezes. Se o PDF do cliente tem senha escrita, ela não entra no `.md` — e vale avisar o cliente.

## Depois de converter

- **Indexe**: `rag.py indexar` passa a achar o conteúdo com `arquivo#linha`.
- **Catalogue**, se o PDF veio do cliente por fora: `/wx-claude-code:artefato` arquiva o PDF original em `artefatos/<tipo>/` com hash e a resposta de **onde usar**.
- **Extraia o que interessa**: classes OOP viram `docs/domain/<módulo>.md`; consultas viram `QRY-*`; regra encontrada vira `BR-*` **com a página na origem**.

## O que o `.md` não resolve

Tabela de PDF sai como texto corrido com frequência: se a tabela importa (plano de contas, alíquotas, layout de relatório), confira contra o PDF antes de usar, e diga que conferiu. Coluna dupla e cabeçalho repetido também confundem o refluxo. O `.md` é conveniência de leitura; a prova continua sendo o PDF, e é por isso que o hash está no cabeçalho.
