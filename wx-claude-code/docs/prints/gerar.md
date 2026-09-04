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
| 26 | sessão `-p` com `Agent` e `Bash`: `/wx-claude-code:pmo exportar` grava o projeto organizado na pasta pedida e explica o que ficou de fora; achou um defeito real (o `.env.exemplo` caía no filtro de `.env`), corrigido com teste |
| 27 | sessão `-p` num projeto com log antigo: o hook `SessionStart` rodou o zelador e a sessão mostra o registro com bytes medidos |
| 28 | dois turnos reais (`-p` e `-c`): a resposta abre com a identificação `Bloco0001-SP00002-Análise da base de dados · data` injetada pelo hook; no segundo turno o agente se recusa a inventar o objetivo da sprint nova e antecipa a identificação `SP00003` |
| 29 | sessão real: perguntada qual agente entra num PDCA infrutífero, apontou o Pesquisador (equipe B), achou o pedido pendente em `pmo/pesquisas.md` com a hipótese morta e a próxima, e deu o status por agente recusando inventar estado para quem não registrou |
| 30 | duas sessões reais: a pergunta sobre a estratégia foi respondida citando `processo-de-conversao.md#L5-L9`, o localizador que o RAG injeta; pedido para apagar um anexo, o agente recusou pela regra antes de o hook precisar negar (o hook está provado por teste, com stdin) |
| 32 | `claude --plugin-dir wx-claude-code -p "liste as skills"` (skills globais do ambiente omitidas do print): as onze skills do plugin aparecem, as oito de ERP com descrição de até 150 caracteres; perguntada por partidas dobradas e estorno, apontou `erp-accounting` |
| 33 | sessão real num projeto com L6 = sim: leu `CLAUDE.md` e `AGENTS.md`, listou módulo → skill, resumiu a ADR 0002 (sem multiempresa na v1) e carregou a `erp-inventory`, citando INV-01 e INV-06 sobre saldo |
| 34 | sessão real na 3.18.0: leu `docs/skills-recomendadas.md` e separou as que cabem (Rust, PostgreSQL, React) das que ficaram fora (Supabase não instalado, sem multiempresa); citou `NUMERIC(19,4)` e `TIMESTAMPTZ` do modelo de dados; explicou que cada mitigação STRIDE vira `SEC-*` e teste |
| 35 | sessão real na 3.19.0: leu `artefatos/CATALOGO.md` e o `CLAUDE.md`, distinguiu o artefato arquivado dos três só declarados e recusou usar o que não foi submetido; explicou o hash como prova; carregou a skill de PHP e citou ponto flutuante em dinheiro e comparação frouxa, com `BigDecimal`/centavos e `DECIMAL(19,4)` |
| 31 | sessão real num projeto recém-aplicado: perguntada o que faz `HReadSeekFirst`, consultou o Help pelo tema que o hook do RAG apontou, citou a página com id e hash, e separou semântica técnica de regra de negócio, como o `CLAUDE.md` gerado manda |
| 07 | `claude -p "/wx-claude-code:laudo-tokens fase-1"` com fonte de sessões marcada INDISPONÍVEL |
| 08 | `pmo.py iniciar/gastar/status` e `rotear_modelo.py` com subida, rebaixamento a 85 %, bloqueio a 105 % e fallback |
| 09 | sessão `-p` com `Agent` liberado: dois símbolos delegados aos `wl-hfsql-specialist` e `wl-communication-specialist`, com o member do Help de cada um |
| 10 | `pmo.py sprint abrir`, dois ciclos `pdca` (um infrutífero, recusado sem `--proxima`), `kanban` com WIP e `sprint fechar` com resumo de doze seções |
| 11 | sessão `-p` com `Agent` liberado: `/wx-claude-code:pmo status` delegado ao agente do PMO, que rodou os scripts e apontou a dessincronia entre plano e sprint |
| 12 | o painel gerado sozinho por `pmo.py sprint fechar` num projeto com bloco 0, lacunas, decisões, riscos, PDCA e roteamento: o relatório de onze seções, o kanban e a base, aberto no Chromium com `prefers-color-scheme: light` |
| 13 | sessão `-p` e `-c`: letra H com «não sei a linguagem», sinais, três opções com a recomendada primeiro |
| 14 | o exemplo `estoque-wx`: `aplicar_questionario.py`, G0 `CONDITIONAL` sem erros, `extrair_pdf.py`, `golden.py` 9/10 |
| 15 | `sprint abrir` com `--item ID:PAPEL`, `kanban` com `[C dba]` etc., `sprint fechar` e `entregar` listando o zip; `tecnicas-aplicadas.md` de dentro do zip |
| 16 | `aplicar_questionario.py` no exemplo com as treze subperguntas de F; trechos do `DESIGN.md` (grids, tabela de botões, posição, fundo) e do `PRODUCT.md` |
| 17 | três turnos reais: abertura do wizard com o item 0.1 sozinho, resposta, confirmação e 0.2, resposta e 0.3 |
| 18 | sessão limpa em que o usuário cola a senha do GitHub no meio da resposta: o agente não a reproduz de forma nenhuma, pede para revogar e registra só URL e usuário (a senha digitada foi mascarada no print; a saída do agente é a real, e o `grep` pelo valor devolve 0) |
| 19 | sessão `-p` e `-c` com `--allowedTools Read`: letra H com os quatro sinais, três opções com a recomendada primeiro, e o processo de conversão para Python lido de `references/perfis-de-destino.md`, peça por peça |
| 20 | sessão `-p` e `-c` com leitura liberada: letra F pede a tela principal como modelo (F0); o agente abriu as capturas de `inputs/screenshots/`, propôs o que preservar a partir do que viu, registrou F0 e só então passou à F1 |
| 21 | `licenca.py verificar` sem serial, sessão `-p` do PMO recusando pelo contexto do hook `SessionStart`, `licenca.py instalar` com um serial de demonstração e a mesma sessão rodando o PMO em seguida |
| 22 | sessão nova, sem contexto, com leitura liberada: perguntada sobre o aprovador e o prazo, achou os dois em `respostas_questionario.md` pelo `CLAUDE.md`, sem perguntar de volta |
| 23 | sessão limpa em que o usuário adianta a letra K2 com a senha do root do PostgreSQL: o agente não a reproduz de forma nenhuma, registra banco, superusuário e papéis, e diz que vai pedir só o nome da variável (a senha digitada foi mascarada no print; o `grep` pelo valor na saída do agente devolve 0) |
| 24 | dois turnos reais (`-p` e `-c`): K7 «sim» ao n8n, o item K7.1 sozinho, resposta, e o K7.2 avisando que sem K2 o banco do n8n pode ser SQLite ou PostgreSQL |
| 25 | sessão nova com leitura liberada, num projeto recém-aplicado: perguntada o que ler e qual o escopo da v1, listou a ordem de leitura, os cinco requisitos do kickoff, a estratégia, e recusou escrever código por falta de G0; de quebra achou um conflito real entre os anexos e o manifesto |

Para refazer: grave a saída em `.txt` e rode o renderizador (um `.mjs` de 40
linhas com Playwright; o de referência fica fora do repositório porque depende
do caminho do Playwright da máquina).
