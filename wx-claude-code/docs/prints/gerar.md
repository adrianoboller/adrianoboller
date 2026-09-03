# Como os prints foram gerados

Nenhum print é montagem: cada um é a saída real de uma sessão do Claude Code
com `--plugin-dir wx-claude-code` (ou de um script do plugin), gravada em texto
e renderizada num terminal pelo Chromium (Playwright), a 2× de escala.

| Print | Origem |
| --- | --- |
| 01 | `claude plugin validate` no plugin e no marketplace, mais o validador offline em modo estrito |
| 02 | sessão `-p` pedindo a lista de skills e agentes com prefixo `wx-claude-code:` |
| 03 | três turnos reais (`claude -p` e `-c`): pergunta A, resposta, confirmação e pergunta B, resposta «não tenho», pergunta C |
| 04 | `aplicar_questionario.py` sobre respostas de exemplo e o `wx_preflight.py` real (BLOCKED por PDF de mentira, como devia) |
| 05 | o `DESIGN.md` que a letra F gera |
| 06 | `query_wlanguage_help.py --verify` e `--query HReadSeekFirst` |
| 08 | `pmo.py iniciar/gastar/status` e `rotear_modelo.py` com subida, rebaixamento a 85 %, bloqueio a 105 % e fallback |
| 09 | sessão `-p` com `Agent` liberado: dois símbolos delegados aos `wl-hfsql-specialist` e `wl-communication-specialist`, com o member do Help de cada um |
| 10 | `pmo.py sprint abrir`, dois ciclos `pdca` (um infrutífero, recusado sem `--proxima`), `kanban` com WIP e `sprint fechar` com resumo de doze seções |
| 11 | sessão `-p` com `Agent` liberado: `/wx-claude-code:pmo status` delegado ao agente do PMO, que rodou os scripts e apontou a dessincronia entre plano e sprint |
| 07 | `claude -p "/wx-claude-code:laudo-tokens fase-1"` com fonte de sessões marcada INDISPONÍVEL |

Para refazer: grave a saída em `.txt` e rode o renderizador (um `.mjs` de 40
linhas com Playwright; o de referência fica fora do repositório porque depende
do caminho do Playwright da máquina).
