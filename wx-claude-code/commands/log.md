---
description: "Mostra o registro das operacoes do plugin no projeto: o que rodou, quando, com que codigo de saida e quanto demorou."
argument-hint: "[resumo|ver] [dias]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# Registro das operações

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/registro.py" \
  --project-root . "${1:-resumo}" --dias "${2:-7}"
```

`resumo` agrupa por operação (vezes, quantas com erro, tempo total); `ver` lista as últimas em ordem.

Toda operação do plugin grava uma linha em `.wx-migration/logs/plugin-AAAA-MM-DD.jsonl`: instante, script, argumentos, código de saída, duração, e o erro quando houve. As negativas dos hooks (anexo somente leitura, artefato, segredo em arquivo, portão do G0) entram como operação também — é assim que se descobre que um agente vinha tentando escrever onde não devia.

Ao relatar, **use os números do arquivo**, não a memória da conversa. Operação com código diferente de zero é a primeira coisa a olhar quando algo «não funcionou».

Senha, token e chave nunca entram: argumento com nome suspeito é gravado como `<omitido>`, e texto com formato de segredo é substituído antes de gravar. O zelador apaga log antigo; o registro é do projeto, não é auditoria fiscal.
