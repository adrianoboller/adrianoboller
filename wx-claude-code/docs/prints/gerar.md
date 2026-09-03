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
| 12 | `pmo.py painel` aberto no Chromium com `prefers-color-scheme: light` |
| 13 | sessão `-p` e `-c`: letra H com «não sei a linguagem», sinais, três opções com a recomendada primeiro |
| 14 | o exemplo `estoque-wx`: `aplicar_questionario.py`, G0 `CONDITIONAL` sem erros, `extrair_pdf.py`, `golden.py` 9/10 |
| 15 | `sprint abrir` com `--item ID:PAPEL`, `kanban` com `[C dba]` etc., `sprint fechar` e `entregar` listando o zip; `tecnicas-aplicadas.md` de dentro do zip |
| 16 | `aplicar_questionario.py` no exemplo com as treze subperguntas de F; trechos do `DESIGN.md` (grids, tabela de botões, posição, fundo) e do `PRODUCT.md` |
| 17 | três turnos reais: abertura do wizard com o item 0.1 sozinho, resposta, confirmação e 0.2, resposta e 0.3 |
| 18 | sessão limpa em que o usuário cola a senha do GitHub no meio da resposta: o agente não a reproduz de forma nenhuma, pede para revogar e registra só URL e usuário (a senha digitada foi mascarada no print; a saída do agente é a real, e o `grep` pelo valor devolve 0) |
| 07 | `claude -p "/wx-claude-code:laudo-tokens fase-1"` com fonte de sessões marcada INDISPONÍVEL |

Para refazer: grave a saída em `.txt` e rode o renderizador (um `.mjs` de 40
linhas com Playwright; o de referência fica fora do repositório porque depende
do caminho do Playwright da máquina).
