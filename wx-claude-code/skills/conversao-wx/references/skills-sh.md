# Catálogo skills.sh e o plugin

Pesquisa feita pelo dono do projeto no catálogo [skills.sh](https://www.skills.sh/) em 4 de setembro de 2026 (via ChatGPT), conferida contra o plugin na 3.18.0. Registrada aqui para que a mesma pesquisa não volte como novidade.

## O que a pesquisa concluiu

Não há uma skill única para ERP. A combinação recomendada tem 27 skills do catálogo por área (requisitos, linguagem empresarial, arquitetura, banco, API, autenticação, multiempresa, segurança, interface, cadastros, grades, acessibilidade, testes, produção, mais React e Rust quando se aplicam) e oito que **não existem** no catálogo e precisam ser próprias: contabilidade, estoque, fiscal brasileiro, multiempresa, alçadas, LGPD, integrações e WLanguage.

## O que o plugin já fazia

| área da pesquisa | onde está no plugin |
| --- | --- |
| as oito skills próprias | vendorizadas em `skills/erp-*` e `windev-wlanguage-erp` (3.17.0) |
| requisitos e entrevista (`prd`, `to-questionnaire`) | o questionário: bloco 0, A–L; `docs/PRD.md` gerado |
| linguagem ubíqua, modelagem, ADRs, mapa de contextos | `UBIQUITOUS_LANGUAGE.md`, `docs/domain/`, `docs/adr/`, `CONTEXT-MAP.md` (L6) |
| interface densa, formulários, grades, filtros, WCAG | letra F, `DESIGN.md`, Impeccable (`shape`, `harden`, `audit`), `qualidade-erp.md` |
| testes por risco, prova antes de concluir | golden master, sete camadas em `tests/`, a regra da prova real |
| CI, backup, runbook | `.github/workflows/`, `scripts/backup/`, `docs/operations/runbook.md` |

## O que entrou por causa dela (3.18.0)

- Sete arquivos a mais no esqueleto: `docs/security/threat-model.md` (STRIDE) e `requisitos.md` (SEC-*), `docs/api/openapi.yaml` e `events.asyncapi.yaml`, `docs/data/erd.md` e `data-dictionary.md`, `docs/domain/invariants.md` e `workflows.md`, `docs/runbooks/incident-response.md` e `backup-restore.md`.
- Regras absorvidas como texto nosso, não como skill de fora: tipos de dinheiro e data, constraint no banco, índice por chave estrangeira, `FOR UPDATE` no saldo e versão otimista na ficha (`modelo-de-dados.md`); negar por padrão e teste de isolamento entre empresas (`SECURITY.md`); densidade e teclado da interface operacional (`qualidade-erp.md`).
- `docs/skills-recomendadas.md` no projeto de destino: só as skills do catálogo que cabem nas respostas (PostgreSQL se K2, Supabase se K5, React se I, Rust se H), com o comando e a ressalva. **O plugin não as instala.**

## Por que não vendorizar as 27

- Medido: descrição longa some da listagem, e a listagem já oscila com 11 skills. Com 38, as de ERP correm o risco de sumir.
- São de autores diversos, com licenças e auditorias próprias; o catálogo diz que a auditoria não garante todos os arquivos.
- O lugar delas é o projeto de destino, instaladas uma a uma pelo usuário depois de ler e fixar a versão.
