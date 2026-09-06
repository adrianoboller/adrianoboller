---
description: "Gera da matriz o teste que ela ja pede, um por BR-*, QRY-* e UI-* sem prova. O gerado FALHA ate alguem escrever a prova."
argument-hint: "[--gravar] [--perfil rust|php|python]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# O teste que a matriz já está pedindo

A matriz diz o que precisa ser provado; o esqueleto nasce com as pastas de teste
vazias; o grafo depois acusa «requisito sem teste», item por item, à mão.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/gerar_testes_da_matriz.py" \
  --project-root . ${1:---gravar}
```

**O teste gerado falha.** Não nasce vazio nem com `assert(true)`: nasce com uma
falha explícita que nomeia a regra sem prova e cita o localizador da origem.
Esqueleto que passa some do relatório de lacunas **sem provar nada** — e teste
que passa por engano é pior que teste que falta.

Duas recusas, de propósito:

- **não sobrescreve** arquivo de teste que já existe — prova escrita por gente
  não se reescreve;
- **não gera para linguagem que não sabe escrever**: sai com erro pedindo
  `--perfil`, em vez de produzir arquivo que nem compila.

Perfis com modelo hoje: `rust`, `php`, `python`. O perfil sai de
`H_backend.perfil` quando o questionário existe.
