# Fontes do WX Claude Code 3.28.0

Inventário medido em 2026-09-05 por `docs/dossie/gerar-fontes.py`. Não se edita à mão: rode o script depois de acrescentar arquivo, e o teste `test_fontes_md_esta_em_dia` avisa quando ele envelhece.

| grupo | arquivos | linhas | o que é |
| --- | ---: | ---: | --- |
| Comandos | 19 | 751 | um por recurso; `/wx-claude-code:<nome>` invoca cada um |
| Agentes | 94 | 3.459 | conversão, papéis PDCA, Impeccable e a equipe prioritária |
| Skills | 21 | 4.310 | conversão, PHP, PDF, laudo de tokens, Impeccable e as oito de ERP |
| Scripts | 26 | 13.120 | o que faz o trabalho: questionário, gates, PMO, licença, RAG, registro |
| Hooks | 3 | 304 | as guardas que rodam nos eventos do Claude Code |
| Referências | 19 | 1.312 | o que os agentes leem antes de decidir |
| Modelos | 6 | 863 | questionário, CLAUDE.md e matriz que viram o projeto do cliente |
| Testes | 2 | 1.483 | a bateria; o validador estrito a roda |
| Exemplo | 21 | 1.036 | projeto sintético que é o teste de regressão do fluxo inteiro |
| Documentos | 4 | 1.116 | manual, README, fontes e a instrução de ativação, na raiz |
| Documentos de apoio | 5 | 195 | relatório, análises, origens dos prints e o vídeo |
| Instaladores | 2 | 431 | bash para Linux e macOS, PowerShell para Windows |
| **total** | **222** | **28.380** | |

## O que não é fonte, mas vem no pacote

- **Corpus do Help WLanguage**: `skills/conversao-wx/resources/Help_WL_12k_Json.zip`, 25 MB, SHA-256 `a95ed5536549ecc3…`. Uso privado; não é redistribuível.
- **Prints e vídeo**: `docs/prints/` e `docs/video/`, saída de sessões reais, sem edição.
- **PDFs**: `docs/*.pdf` e `docs/dossie/*.pdf`, gerados dos HTML do mesmo repositório.
- **Marca**: `marca-wx-claude-code.png` e `licenca/chave-publica.json` (a de demonstração).

## Como instalar

```bash
./instalar.sh                     # Linux e macOS
```

```powershell
.\instalar.ps1                    # Windows
```

Os dois fazem o mesmo caminho: pré-requisitos, corpus, validação, instalação no Claude Code e licença. `--conferir` (ou `-Conferir`) mostra o que aconteceria sem mudar nada.

## Como conferir que o pacote está inteiro

```bash
python3 skills/conversao-wx/scripts/validate_plugin_bundle.py . --strict
```

Esperado: `valid: true`, `tests: OK`, zero erros e zero avisos. O `--strict` roda a bateria inteira.

Último commit no momento da medição: `6327345 2026-09-05`.
