---
description: "Gate G0: inventaria as evidencias do legado, classifica a prova e escreve o relatorio que os outros gates leem."
argument-hint: "[raiz-do-projeto]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# G0 — pré-flight das evidências

Sem G0 verde (ou `CONDITIONAL` com zero erros) nenhum outro gate começa: é o hook `portao_g0` que garante isso, e ele falha fechado se não conseguir ler o relatório.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/wx_preflight.py" \
  --manifest <raiz>/.wx-migration/wx-inputs.manifest.json \
  --allowed-evidence-root <raiz-de-evidencias> --workspace-root <raiz> \
  --output <raiz>/.wx-migration/preflight
```

Leia o `report.json` da última pasta em `preflight/runs/` e diga, em uma tela: **status**, classe, contagem de itens, páginas do corpus, **erros** e avisos. Erro bloqueia; aviso não.

Depois explique o que o resultado significa para o próximo passo: `BLOCKED` diz qual grupo de evidência falta (e a letra do questionário que resolve); `CONDITIONAL` com zero erros segue, dizendo o que fica sem prova executável; `DEGRADED` costuma ser corpus ausente.

Não invente número: tudo o que você disser sai do `report.json`.
