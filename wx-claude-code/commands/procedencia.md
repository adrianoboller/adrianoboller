---
description: "Procedencia da entrega: SLSA provenance e BOM CycloneDX medidos do projeto, com o que eles nao afirmam."
argument-hint: "[tudo|bom|slsa|conferir ARQ]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Procedência: SLSA e CycloneDX

Em banco, governo e saúde, «confie em mim» não compra nada. Estes são os dois
formatos que a área de segurança do cliente já lê.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/procedencia.py" \
  --project-root . "${1:-tudo}" ${2:+"$2"}
```

O que ele **afirma**: hash de cada artefato, commit, quem rodou, quando, com
que versão do plugin e que skills. O que ele **não afirma**, escrito dentro do
próprio documento: nível de SLSA (depende da infraestrutura de build, que este
plugin não controla), reprodutibilidade bit a bit, e assinatura de terceiro.

`--assinar chave-privada.json` assina com a **mesma RSA do serial** — sem
dependência nova. `conferir ARQ` reconfere os hashes contra os arquivos de hoje
e sai 1 se algum mudou.
