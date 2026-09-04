#!/usr/bin/env python3
"""Esqueleto de projeto ERP (letra L6 do questionario).

Gera, no projeto de destino, a arvore de pastas e os documentos-guia do pacote
erp-skills-pack: contexto, arquitetura, seguranca, linguagem ubiqua, ADRs,
banco (schema/migrations/seeds/rollback), src por modulo, testes por camada,
scripts e workflows. Cada arquivo e um esboco preenchido com as respostas do
questionario e nunca sobrescreve o que ja existe (write_new de quem chama).

Por que um modulo separado: o aplicar_questionario ja passa de mil linhas, e o
esqueleto e opcional (L6.gerar = false nao toca em nada).
"""
from __future__ import annotations

import re
from datetime import date

# Modulo do 0.8 -> skill do plugin que orienta aquele modulo. A ordem das
# chaves e a ordem de tentativa: a primeira que casar com o nome ganha.
MAPA_MODULO_SKILL = [
    (r"contab|financ|razao|razão|cont[aá]bil|caixa|banc[oá]|pagar|receber|tesour", "erp-accounting"),
    (r"estoque|invent|moviment|almox|deposit|dep[oó]sito|lote|s[eé]rie|compra|suprim|produt", "erp-inventory"),
    (r"fiscal|nf-?e|nfc-?e|cte|mdfe|sped|tribut|imposto|nota", "erp-brazil-fiscal"),
    (r"empresa|filial|estabelec|tenant|grupo|matriz", "erp-multi-company"),
    (r"aprov|al[cç]ada|workflow|autoriz|delega", "erp-approval-workflows"),
    (r"lgpd|privac|titular|dado pessoal|dados pessoais|consent", "erp-lgpd"),
    (r"integra|api|webhook|fila|evento|edi|sincron", "erp-integration-reliability"),
    (r"venda|pedido|comercial|crm|cliente|or[cç]amento|faturamento", "erp-brazil-fiscal"),
    (r"rh|folha|pessoal|funcion", "erp-lgpd"),
]
SKILLS_ERP = ["erp-accounting", "erp-inventory", "erp-brazil-fiscal", "erp-multi-company",
              "erp-approval-workflows", "erp-lgpd", "erp-integration-reliability", "windev-wlanguage-erp"]

DOMINIOS = ["cadastros", "compras", "vendas", "estoque", "financeiro", "contabilidade", "fiscal", "rh", "integracoes", "auditoria"]


def slug(nome: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", (nome or "").lower().replace("ç", "c").replace("ã", "a").replace("õ", "o").replace("é", "e").replace("á", "a").replace("í", "i").replace("ó", "o").replace("ú", "u").replace("â", "a").replace("ê", "e").replace("ô", "o")).strip("-")
    return s or "modulo"


def skill_para(modulo: str) -> str:
    m = (modulo or "").lower()
    for rx, skill in MAPA_MODULO_SKILL:
        if re.search(rx, m):
            return skill
    return "windev-wlanguage-erp"


def modulos_de(q: dict) -> list[str]:
    e = q.get("0_empresa_e_projeto") or {}
    mods = ((e.get("0_8_descricao_do_software") or {}).get("modulos") or [])
    l6 = ((q.get("L_contexto_e_implantacao") or {}).get("L6_esqueleto_erp") or {})
    mods = list(mods) + [m for m in (l6.get("modulos_extras") or []) if m not in mods]
    return [m for m in mods if isinstance(m, str) and m.strip()] or ["cadastros", "vendas", "estoque", "financeiro"]


def _cab(titulo: str, q: dict) -> str:
    p = q.get("projeto") or {}
    return f"# {titulo}\n\n_{p.get('nome') or 'projeto'} — esboço gerado pelo questionário (L6) em {date.today().isoformat()}; complete e mantenha._\n\n"


def _lista(itens, vazio="- (a preencher)") -> str:
    itens = [str(i) for i in (itens or []) if str(i).strip()]
    return "\n".join(f"- {i}" for i in itens) if itens else vazio


def arquivos(q: dict) -> dict[str, str]:
    """Caminho relativo -> conteudo. Diretorios vazios recebem .gitkeep."""
    e = q.get("0_empresa_e_projeto") or {}
    p = q.get("projeto") or {}
    h = q.get("H_backend") or {}
    i = q.get("I_frontend") or {}
    k = q.get("K_ambiente") or {}
    l = q.get("L_contexto_e_implantacao") or {}
    l6 = l.get("L6_esqueleto_erp") or {}
    d8 = e.get("0_8_descricao_do_software") or {}
    mods = modulos_de(q)
    nome = p.get("nome") or "ERP"
    ap = ((e.get("0_16_aprovador") or {}).get("nome")) or p.get("aprovador") or "(pendente)"
    banco = h.get("banco") or "postgresql"
    multi = bool(l6.get("multi_empresa", True))
    fiscal = bool(l6.get("fiscal_brasil", True))
    A: dict[str, str] = {}

    tab_mod = "\n".join(f"| {m} | `src/{slug(m)}/` | `{skill_para(m)}` |" for m in mods)
    A["AGENTS.md"] = _cab("AGENTS.md — como um agente trabalha neste ERP", q) + (
        "Leia nesta ordem: `CLAUDE.md`, `CONTEXT.md`, `UBIQUITOUS_LANGUAGE.md`, `ARCHITECTURE.md`, `SECURITY.md`; depois o `docs/domain/<módulo>.md` da tarefa.\n\n"
        "## Regras que não se negociam\n\n"
        "1. Regra empresarial vem do legado com origem (`BR-*` na matriz); sem origem é `GAP-*`.\n"
        "2. Toda escrita contábil, fiscal ou de estoque é transacional, idempotente e auditável (quem, quando, o quê, por quê).\n"
        "3. Nada de dado pessoal ou segredo em código, teste, log ou documento; só nomes de variáveis.\n"
        "4. Migração de banco tem `rollback` correspondente antes de ser aplicada.\n"
        "5. Mudança que atravessa módulos passa pelo `CONTEXT-MAP.md` e vira ADR.\n\n"
        "## Módulo → pasta → skill\n\n| módulo | código | skill do plugin WX Claude Code |\n| --- | --- | --- |\n" + tab_mod + "\n\n"
        f"Aprovador: **{ap}**. Decisões `DEC-*` só com ele.\n")
    A["CONTEXT.md"] = _cab("CONTEXT.md — contexto do negócio", q) + (
        f"## Finalidade\n\n{e.get('0_6_finalidade') or '(a preencher)'}\n\n## Objetivos\n\n{_lista(e.get('0_7_objetivos'))}\n\n"
        f"## O software\n\n{d8.get('descricao') or '(a preencher)'}\n\n### Recursos\n\n{_lista(d8.get('recursos'))}\n\n### Módulos\n\n{_lista(mods)}\n\n"
        f"## Quem opera\n\nVeja `PRODUCT.md` (F1). Multiempresa: **{'sim' if multi else 'não'}**. Fiscal brasileiro: **{'sim' if fiscal else 'não'}**.\n")
    A["CONTEXT-MAP.md"] = _cab("CONTEXT-MAP.md — mapa de contextos", q) + (
        "Cada módulo é um contexto delimitado; a linha diz quem é dono de qual regra e como os outros a consomem.\n\n"
        "| contexto | é dono de | publica | consome |\n| --- | --- | --- | --- |\n" +
        "\n".join(f"| {m} | (regras `BR-*` do módulo) | eventos `{slug(m)}.*` | |" for m in mods) +
        "\n\nRegra: dado de outro contexto entra por evento ou API, nunca por leitura direta da tabela alheia.\n")
    A["UBIQUITOUS_LANGUAGE.md"] = _cab("UBIQUITOUS_LANGUAGE.md — linguagem ubíqua", q) + (
        "Um termo, um significado, um nome no código. Preencha ao converter cada tela e procedure; o nome do legado fica na coluna de origem.\n\n"
        "| termo | significado | nome no código | nome no legado (WX) | módulo |\n| --- | --- | --- | --- | --- |\n" +
        "\n".join(f"| | | | | {m} |" for m in mods) + "\n")
    A["ARCHITECTURE.md"] = _cab("ARCHITECTURE.md — arquitetura", q) + (
        f"- Backend: **{h.get('linguagem') or h.get('perfil') or '?'}** ({h.get('framework') or '?'}), banco **{banco}**.\n"
        f"- Frontend: **{i.get('linguagem') or i.get('perfil') or '?'}** ({i.get('framework') or '?'}).\n"
        f"- Estilo: monólito modular, um diretório por módulo em `src/`, fronteiras do `CONTEXT-MAP.md`.\n"
        f"- Multiempresa: {'toda tabela de negócio carrega `empresa_id` e toda consulta filtra por ele (skill `erp-multi-company`)' if multi else 'não previsto; se entrar, é ADR'}.\n"
        "- Escrita: transação por caso de uso; evento de domínio gravado na mesma transação (outbox); integrações lêem o outbox (skill `erp-integration-reliability`).\n"
        "- Auditoria: tabela `auditoria` por módulo, imutável, com ator, instante, antes e depois.\n\n"
        "Decisões registradas em `docs/adr/`.\n")
    A["SECURITY.md"] = _cab("SECURITY.md — segurança", q) + (
        "- Autenticação e papéis: os de `K` (root só no instalador; papéis `app`, `leitura`, `migracao`).\n"
        "- Segredos só por variável de ambiente (`.wx-migration/ambiente/.env.exemplo`); nunca em repositório, log ou teste.\n"
        "- Segregação de funções: quem cria não aprova (skill `erp-approval-workflows`).\n"
        "- Dados pessoais: inventário em `docs/security/dados-pessoais.md`, retenção e direitos do titular (skill `erp-lgpd`).\n"
        "- Toda rota de escrita exige autorização por módulo e por empresa.\n"
        "- Dependências: `scripts/verification/` roda auditoria de vulnerabilidades no CI (`.github/workflows/security.yml`).\n")
    A["CHANGELOG.md"] = "# CHANGELOG\n\nFormato: Keep a Changelog; versões SemVer.\n\n## [Não lançado]\n\n- Esqueleto de ERP gerado pelo questionário (L6).\n"
    A[".editorconfig"] = "root = true\n\n[*]\ncharset = utf-8\nend_of_line = lf\ninsert_final_newline = true\nindent_style = space\nindent_size = 4\ntrim_trailing_whitespace = true\n\n[*.{md,yml,yaml,json}]\nindent_size = 2\n\n[Makefile]\nindent_style = tab\n"

    # docs/
    A["docs/PRD.md"] = _cab("PRD — requisitos do produto", q) + "## Requisitos da v1 (L1)\n\n" + _lista((l.get("L1_requisitos_da_v1") or {}).get("itens")) + "\n\n## Fora da v1\n\n" + _lista((l.get("L1_requisitos_da_v1") or {}).get("fora_da_v1")) + "\n"
    A["docs/ROADMAP.md"] = _cab("ROADMAP", q) + "Marcos vêm de `.wx-migration/pmo/cronograma.md`; cada onda de módulos é uma linha.\n\n| onda | módulos | marco | estado |\n| --- | --- | --- | --- |\n" + "\n".join(f"| {n} | {m} | | planejado |" for n, m in enumerate(mods, 1)) + "\n"
    A["docs/BACKLOG.md"] = _cab("BACKLOG do produto", q) + "O backlog vivo é `.wx-migration/pmo/backlog.md` (PMO). Aqui ficam só os épicos por módulo.\n\n" + "\n".join(f"- [ ] {m}: converter telas, procedures e queries do legado (`BR-*`, `UI-*`, `QRY-*`)" for m in mods) + "\n"
    adrs = [
        ("0001-monolito-modular", "Monólito modular como estilo inicial", "Um deploy, um banco, fronteiras por módulo em `src/` e `CONTEXT-MAP.md`. Extrair serviço só com medição de dor real."),
        ("0002-multiempresa", "Isolamento por empresa", ("`empresa_id` em toda tabela de negócio e filtro obrigatório nas consultas; papéis de banco não conseguem ler outra empresa." if multi else "Não há multiempresa na v1; entrar depois exige migração de toda tabela de negócio.")),
        ("0003-auditoria-e-outbox", "Auditoria imutável e outbox transacional", "Toda escrita grava auditoria e evento na mesma transação; integrações consomem o outbox com idempotência e DLQ."),
        ("0004-fiscal-brasil", "Motor fiscal brasileiro", ("Documento fiscal eletrônico e eventos ficam no módulo fiscal, com fontes oficiais listadas na skill `erp-brazil-fiscal`; nenhuma alíquota fica em código." if fiscal else "Sem obrigações fiscais brasileiras na v1; registrar aqui quando entrar.")),
    ]
    for arq, titulo, texto in adrs:
        A[f"docs/adr/{arq}.md"] = f"# ADR {arq[:4]} — {titulo}\n\n- Data: {date.today().isoformat()}\n- Estado: proposta\n- Aprovador: {ap}\n\n## Contexto\n\n{nome}: conversão de legado WINDEV/WEBDEV.\n\n## Decisão\n\n{texto}\n\n## Consequências\n\n(a preencher)\n"
    for m in mods:
        A[f"docs/domain/{slug(m)}.md"] = _cab(f"Domínio: {m}", q) + (
            f"Skill que orienta: `{skill_para(m)}` (plugin WX Claude Code).\n\n## Entidades\n\n- (a preencher)\n\n## Invariantes\n\n- (uma linha por `BR-*` da matriz que pertence a este módulo)\n\n## Eventos que publica\n\n- `{slug(m)}.` …\n\n## Origem no legado\n\n- Telas, procedures e queries do WX que viram este módulo (ids da `traceability.csv`).\n")
    A["docs/data/modelo-de-dados.md"] = _cab("Modelo de dados", q) + f"Banco: {banco}. Uma seção por módulo; toda tabela de negócio {'tem `empresa_id`, ' if multi else ''}tem `criado_em`, `criado_por`, `atualizado_em`, `atualizado_por`.\n\n" + "\n".join(f"## {m}\n\n| tabela | chave | origem no legado (DB-*) |\n| --- | --- | --- |\n" for m in mods)
    A["docs/api/README.md"] = _cab("API", q) + "Contratos por módulo em `docs/api/<módulo>.md`; toda rota de escrita é idempotente por `Idempotency-Key` e exige empresa e papel. Testes de contrato em `tests/contracts/`.\n"
    A["docs/security/dados-pessoais.md"] = _cab("Inventário de dados pessoais (LGPD)", q) + "| dado | módulo | finalidade | base legal | retenção | titular pode |\n| --- | --- | --- | --- | --- | --- |\n"
    A["docs/security/papeis-e-permissoes.md"] = _cab("Papéis e permissões", q) + "| papel | módulo | pode | não pode | segregação |\n| --- | --- | --- | --- | --- |\n"
    A["docs/operations/runbook.md"] = _cab("Runbook", q) + f"- Implantação: alvo `{(l.get('L3_implantacao') or {}).get('alvo') or 'nenhum'}`; `Dockerfile` e `docker-compose.yml` na raiz.\n- Backup: `scripts/backup/`; restauração testada em `tests/migration/`.\n- Healthcheck: `{(l.get('L3_implantacao') or {}).get('healthcheck') or '/health'}`.\n- Incidente com dado pessoal: `SECURITY.md` e skill `erp-lgpd`.\n"
    A["docs/testing/estrategia.md"] = _cab("Estratégia de testes", q) + (
        "| camada | pasta | prova |\n| --- | --- | --- |\n| unitário | `tests/unit/` | funções puras, cálculos |\n| domínio | `tests/domain/` | invariantes `BR-*`, golden master do legado |\n| integração | `tests/integration/` | banco real, transação e auditoria |\n| contratos | `tests/contracts/` | API e eventos |\n| segurança | `tests/security/` | isolamento por empresa, permissões, segredo em log |\n| migração | `tests/migration/` | up, rollback, backup/restore |\n| e2e | `tests/e2e/` | fluxo de tela igual ao legado |\n\nRegra: teste novo tem de falhar com o defeito reposto (prova real).\n")

    # database/
    A["database/README.md"] = f"# database/\n\nBanco {banco}. `schema/` é o estado desejado; `migrations/` o caminho até ele, cada uma com par em `rollback/`; `seeds/` só dado anonimizado; `views/` e `procedures/` versionados aqui, nunca só no servidor.\n"
    A["database/migrations/0001_base.sql"] = f"-- 0001_base: auditoria e {'empresas' if multi else 'base'}. Rollback em database/rollback/0001_base.sql\n-- (a preencher conforme docs/data/modelo-de-dados.md)\n"
    A["database/rollback/0001_base.sql"] = "-- desfaz 0001_base\n"
    for d in ("schema", "seeds", "views", "procedures"):
        A[f"database/{d}/.gitkeep"] = ""

    # src/, tests/, scripts/
    for m in mods:
        A[f"src/{slug(m)}/README.md"] = f"# {m}\n\nDomínio em `docs/domain/{slug(m)}.md`; skill `{skill_para(m)}`. Entra aqui: entidades, casos de uso, repositório, rotas. Não lê tabela de outro módulo.\n"
    for t in ("unit", "domain", "integration", "contracts", "security", "migration", "e2e"):
        A[f"tests/{t}/.gitkeep"] = ""
    for s, txt in (("build", "compila e empacota"), ("deploy", "publica no alvo de L3"), ("migration", "aplica e reverte migrações"), ("backup", "backup e restauração"), ("verification", "lint, auditoria de dependências, verificação de segredos")):
        A[f"scripts/{s}/README.md"] = f"# scripts/{s}\n\n{txt}. Idempotente; nunca lê segredo de arquivo versionado.\n"

    # .github/workflows
    teste = (l.get("L4_hooks_do_projeto") or {}).get("comando_de_teste") or "echo 'defina L4.comando_de_teste'"
    lint = (l.get("L4_hooks_do_projeto") or {}).get("comando_de_lint") or "echo 'defina L4.comando_de_lint'"
    def wf(nome, passos):
        return f"name: {nome}\non:\n  push:\n    branches: [main]\n  pull_request:\n\njobs:\n  {nome}:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n" + "".join(f"      - name: {n}\n        run: {c}\n" for n, c in passos)
    A[".github/workflows/build.yml"] = wf("build", [("lint", lint), ("build", "bash scripts/build/build.sh || true")])
    A[".github/workflows/tests.yml"] = wf("tests", [("testes", teste)])
    A[".github/workflows/security.yml"] = wf("security", [("segredos", "git grep -nE '(senha|password|token)\\s*[:=]\\s*[\"\\x27][^\"\\x27]{6,}' -- ':!*.md' && exit 1 || true"), ("dependencias", "bash scripts/verification/auditar.sh || true")])
    A[".github/workflows/release.yml"] = "name: release\non:\n  push:\n    tags: ['v*']\n\njobs:\n  release:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - name: changelog\n        run: sed -n '/^## /,/^## /p' CHANGELOG.md | head -40\n"
    return A


def secao_claude_md(q: dict) -> str:
    mods = modulos_de(q)
    linhas = "\n".join(f"| {m} | `src/{slug(m)}/` | `{skill_para(m)}` |" for m in mods)
    return ("## Skills de ERP\n\n"
            "Este projeto tem esqueleto de ERP (L6): `AGENTS.md`, `CONTEXT.md`, `CONTEXT-MAP.md`, `UBIQUITOUS_LANGUAGE.md`, `ARCHITECTURE.md`, `SECURITY.md`, `docs/`, `database/`, `src/<módulo>/`, `tests/<camada>/`. "
            "O plugin WX Claude Code traz oito skills de ERP; antes de mexer num módulo, carregue a skill da linha dele. Tarefa que atravessa módulos carrega só as necessárias e decide qual é dono da regra.\n\n"
            "| módulo | código | skill |\n| --- | --- | --- |\n" + linhas + "\n\n"
            "Transversais: `erp-multi-company` (isolamento por empresa), `erp-approval-workflows` (alçadas), `erp-lgpd` (dados pessoais), `erp-integration-reliability` (API, fila, idempotência), `windev-wlanguage-erp` (ler o legado WX como ERP).\n")


def entradas_index(q: dict) -> list[tuple[str, str]]:
    mods = modulos_de(q)
    fixos = [("AGENTS.md", "como um agente trabalha neste ERP: ordem de leitura, regras, módulo → skill"),
             ("CONTEXT.md", "finalidade, objetivos, recursos e módulos (bloco 0)"),
             ("CONTEXT-MAP.md", "quem é dono de qual regra e como os módulos se falam"),
             ("UBIQUITOUS_LANGUAGE.md", "um termo, um significado, um nome no código; nome do legado ao lado"),
             ("ARCHITECTURE.md", "monólito modular, multiempresa, outbox, auditoria"),
             ("SECURITY.md", "papéis, segredos, segregação de funções, LGPD"),
             ("docs/adr/", "decisões de arquitetura 0001–0004"),
             ("docs/data/modelo-de-dados.md", "tabelas por módulo com origem DB-*"),
             ("docs/testing/estrategia.md", "camadas de teste e o que cada uma prova"),
             ("database/", "schema, migrations com rollback, seeds anonimizados"),
             (".github/workflows/", "build, tests, security, release")]
    return fixos + [(f"docs/domain/{slug(m)}.md", f"domínio {m}: entidades, invariantes, eventos, origem no legado") for m in mods]
