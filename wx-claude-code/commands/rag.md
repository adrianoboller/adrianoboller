---
description: "Indexa e busca nos documentos do projeto (BM25 local, sem dependencia), devolvendo trechos com arquivo e linha."
argument-hint: "[indexar|buscar] <termo>"
allowed-tools: "Read, Glob, Grep, Bash"
---

# RAG do projeto

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/rag.py" --project-root . indexar
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/rag.py" --project-root . buscar "$2"
```

Indexa o que o plugin gera e lê: `.wx-migration/*.md`, a matriz, as decisões, o PMO, `CLAUDE.md`, `DESIGN.md` e o `docs/` do projeto. A busca devolve `arquivo#linha` — **cite assim**, para que o usuário confira.

Serve para achar antes de ler: procure o id (`BR-012`, `GAP-003`, `DEC-0001`) em vez de abrir diretório inteiro.

O índice é do projeto, não do legado: evidência em PDF se lê com `extrair_pdf.py`, que dá o localizador página#linha.
