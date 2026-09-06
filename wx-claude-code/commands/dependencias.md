---
description: "Inventario das dependencias externas do legado achadas no texto: INI, banco, DLL, COM, webservice, e-mail, FTP, impressao."
argument-hint: "[--gravar]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# O que o legado usa de fora

Cada dependência externa é uma decisão de conversão. Sem esta lista elas
aparecem uma a uma, quando o agente tropeça: chega no `INIRead` e descobre que
há arquivo de configuração; chega no `SOAPExecute` e descobre que há webservice.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/inventario_de_dependencias.py" \
  --project-root . ${1:---gravar}
```

Dez categorias, achadas por **sinal** — a chamada que só existe quando a
dependência existe, nunca palavra solta (`Email` casaria com um comentário sobre
e-mail). Cada achado vem com `arquivo#linha`. Serve para WLanguage, PHP e C/C++,
porque o legado é E/OU.

**A lista é um piso, não um inventário fechado**, e o relatório diz isso na
primeira linha. Não alcança: componente `.wdk` referenciado só no projeto, DLL
declarada no IDE e nunca chamada, driver de impressora configurado fora do
código, conexão criada pelo assistente, e o que não estiver no texto que chegou.
Cada um desses vira `GAP-*` se ninguém confirmar que não existe.
