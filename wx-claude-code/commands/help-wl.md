---
description: "Consulta o corpus do Help WLanguage (12k paginas) por tema ou funcao, devolvendo a pagina com id e hash."
argument-hint: "<funcao ou tema>"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Help WLanguage (corpus 12k)

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" --query "$1"
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py" --group GG-SS-TT --query "$1"
```

Cite sempre **id e hash** da página que você usou: é o que separa o que está no Help do que você lembra.

O Help é **semântica técnica**, nunca regra de negócio. O que a função faz vem daqui; o que o sistema do cliente faz vem da evidência e da matriz. Não misture os dois na mesma frase.

Corpus ausente ou com hash diferente do esperado: diga, e trate como `DEGRADED` — não responda por memória.
