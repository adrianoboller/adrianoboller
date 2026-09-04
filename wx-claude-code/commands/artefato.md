---
description: "Submete um artefato do cliente (anotacao, classe OOP, SQL, relatorio, manual, codigo PHP) e o arquiva com hash no catalogo."
argument-hint: "<arquivo> [tipo]"
allowed-tools: "Read, Glob, Grep, Bash, AskUserQuestion"
---

# Submeter um artefato (bloco M)

`$1` é o arquivo; `$2` o tipo, se o usuário já souber.

## Ordem

1. **Pergunte o tipo**, se não veio: `anotacao`, `classe-oop`, `query-sql`, `relatorio`, `regra-de-negocio`, `tela`, `manual`, `contrato-de-api`, `codigo-php`, `dado-de-amostra`, `outro`.
2. **Pergunte onde usar** — obrigatório, e é o que dá valor ao bloco: em que gate e em que arquivo do destino aquele artefato entra («G3: viram QRY-* e métodos de repositório»). Sem isso o script recusa, e com razão: artefato sem destino declarado vira arquivo que ninguém abre.
3. **Pergunte se é confidencial.** Sendo, ele se cita pelo nome, nunca copiando o conteúdo.
4. **Arquive:**

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/arquivar_artefato.py" \
  --project-root . --arquivo "$1" --tipo <tipo> --onde-usar "<onde>" \
  --descricao "<o que é>" --origem cliente \
  --questionario .wx-migration/questionario.json
```

5. **Leia o artefato** e diga o que ele acrescenta: regra nova vira `BR-*` com origem nele; consulta vira `QRY-*`; relatório vira `RPT-*`. O que não der para analisar você reporta pelo tamanho em bytes, não recortando texto.

O script recusa arquivo de texto com token ou chave, recusa sobrescrever um arquivado com outro conteúdo, e não duplica o mesmo arquivo. A pasta `artefatos/` é somente leitura: **não** edite `CATALOGO.md` nem `registro.json` — um hook recusa, e eles saem dos fatos.

Para só reler o catálogo: `arquivar_artefato.py --project-root . catalogo`.
