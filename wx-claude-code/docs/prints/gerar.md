# Como os prints foram gerados

Nenhum print é montagem: cada um é a saída real de uma sessão do Claude Code
com `--plugin-dir wx-claude-code` (ou de um script do plugin), gravada em texto
e renderizada num terminal pelo Chromium (Playwright), a 2× de escala.

| Print | Origem |
| --- | --- |
| 01 | `claude plugin validate` no plugin e no marketplace, mais o validador offline em modo estrito |
| 02 | sessão `-p` pedindo a lista de skills e agentes com prefixo `wx-claude-code:` |
| 03 | `claude -p "/wx-claude-code:questionario <projeto>"`, sem `AskUserQuestion` (modo texto) |
| 04 | `aplicar_questionario.py` sobre respostas de exemplo e o `wx_preflight.py` real (BLOCKED por PDF de mentira, como devia) |
| 05 | o `DESIGN.md` que a letra F gera |
| 06 | `query_wlanguage_help.py --verify` e `--query HReadSeekFirst` |
| 07 | `claude -p "/wx-claude-code:laudo-tokens fase-1"` com fonte de sessões marcada INDISPONÍVEL |

Para refazer: grave a saída em `.txt` e rode o renderizador (um `.mjs` de 40
linhas com Playwright; o de referência fica fora do repositório porque depende
do caminho do Playwright da máquina).
