# Corpus WLanguage bundled

`Help_WL_12k_Json.zip` é distribuído como recurso somente leitura após uma
sanitização determinística e restrita de chaves privadas demonstrativas.

- SHA-256 distribuído: `a95ed5536549ecc39fb1163415042d6597c8913e5edbfdb531cba756546a82a2`
- SHA-256 do anexo recebido: `a6b42f59796ccf51298712aff00c043a9be2c404ce761a99720ea31b91ca6b93`
- tamanho distribuído: `26.750.976` bytes
- conteúdo: 12.037 JSONs (um índice e 12.036 páginas) e `progresso.ini`
- páginas JSON válidas conhecidas: 12.035
- sanitização: 15 blocos PEM em 2 páginas, sem remover as páginas
- estado: `DEGRADED/CONDITIONAL`

Não extraia nem execute o ZIP. Use `../scripts/query_wlanguage_help.py --verify`
e consulte `../references/bundled-help-corpus.md`. A transformação reproduzível
está no script de sanitização (`sanitize_help_corpus.py`, mantido fora desta distribuição).

O corpus é derivado da documentação PC SOFT e não inclui licença de redistribuição. Mantenha-o em uso privado até haver autorização aplicável.
