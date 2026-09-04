# Hooks e RAG: o que o harness faz sozinho

Regra que é só texto depende de o modelo obedecer. Estas viraram hook, e o harness as executa antes ou depois de cada ferramenta.

## Hooks do plugin (`hooks/hooks.json`)

| Evento | Hook | O que faz | Custo medido |
| --- | --- | --- | --- |
| SessionStart | `licenca.py hook-sessao` | injeta o estado da licença | ~54 ms |
| SessionStart | `zelador.py hook-sessao` | limpa temporários, uma vez por dia | só no primeiro dia |
| SessionStart, UserPromptSubmit | `pmo.py hook-identificacao` | injeta `BlocoNNNN-SPNNNNN-Título · data` | ~50 ms |
| UserPromptSubmit | `rag.py hook` | injeta os 4 trechos do projeto mais próximos da pergunta, com `arquivo#Lnn` | 48 ms com 256 trechos (índice em cache; reindexa quando marcado) |
| PreToolUse | `licenca.py hook` | sem serial válido nega scripts do plugin e escrita em `.wx-migration/` | ~54 ms |
| PreToolUse | `portao_g0.py` | G0 BLOCKED nega escrita de código fora de `.wx-migration/` (resolve `..`, cobre Bash com `>`) | — |
| PreToolUse | `guarda_anexos_e_segredos.py` | anexos somente leitura (nega Write/Edit e `rm`, `mv`, `>` na raiz de evidências); nega conteúdo com formato de token, gravação de `.env` e `git add .env` | — |
| PostToolUse | `sincronizar_pmo.py` | `traceability.csv` ou `backlog.md` editados regeram o Kanban; `questionario.json` editado lembra de reaplicar; qualquer doc de `.wx-migration/` marca o RAG para reindexar | só quando toca `.wx-migration/` |
| PostToolUse, Stop | Impeccable | revisa a tela editada | — |

Projeto sem `.wx-migration/` não é afetado por nenhum deles. Os hooks de guarda são **fail-open**: erro interno libera, porque protegem o usuário de si mesmo; o portão de segurança de verdade contra injeção é o `validar_entradas` do `aplicar_questionario.py`, que recusa antes de gravar.

## O RAG do projeto (`rag.py`)

- **O que indexa:** tudo que o plugin gera e lê em `.wx-migration/` (respostas, empresa, processo, ambiente, prompts, matriz, lacunas, decisões, PMO inteiro, base de conhecimento, sprints, pareceres), `CLAUDE.md`, `INDEX_FILES.md`, `DESIGN.md`, `PRODUCT.md`, `docs/` do projeto e as `references/` do plugin. Nunca o `questionario.json` bruto nem os anexos.
- **Como:** trechos de ~900 caracteres com sobreposição, cada um com `arquivo#Lnn`; busca BM25 em Python puro, sem dependência. `indexar`, `buscar "…" --k 5`, e o `hook`.
- **Por que localizador:** regra de negócio só vale com origem localizável; o trecho aponta o arquivo e a linha para o modelo abrir antes de afirmar. O contexto injetado diz isso.
- **O que não é:** o corpus WLanguage 12k continua no `query_wlanguage_help.py --group`, por tema; este RAG é do projeto.

## No projeto de destino

`aplicar_questionario.py` gera `.claude/settings.json` com os hooks do projeto (teste ao parar, lint ao editar, com os comandos de L4) e nega `Read(./.env)`. Os hooks do plugin somam-se a esses.
