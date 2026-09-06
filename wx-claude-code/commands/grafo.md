---
description: "Grafo de rastreabilidade: acha codigo sem requisito, requisito sem teste, teste sem evidencia e prova vencida."
argument-hint: "[conferir|de ID|mermaid [ID]]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Grafo de rastreabilidade

A matriz já tinha as 22 colunas certas, mas só respondia lendo tudo. O grafo faz
as perguntas que ninguém responde à mão num projeto com duzentas regras:

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/grafo.py" \
  --project-root . "${1:-conferir}" ${2:+"$2"}
```

- **`conferir`** — as sete perguntas de uma vez: código sem requisito, requisito
  sem teste, teste sem evidência, prova vencida, decisão citada que não existe,
  restrição sem alcance, e origem no legado que mudou depois da conversão. Sai 1
  quando há lacuna.
- **`de BR-001`** — tudo que um nó alcança: origem no legado, decisão, arquivo,
  símbolo, teste, estado, quem aprovou e a evidência com o limite dela.
- **`mermaid`** — o desenho, para colar no relatório.

Ele **não inventa aresta**: coluna vazia é ligação inexistente, e isso vira a
lacuna 1, 2 ou 3 — não um palpite. Grafo que completa lacuna sozinho é pior que
planilha, porque parece completo.

Duas exclusões deliberadas, para o sinal não morrer no ruído: arquivo de teste
já está ligado pela coluna `test_file`, e o esqueleto que o próprio questionário
gerou (listado no `INDEX_FILES.md`) não é código convertido.
