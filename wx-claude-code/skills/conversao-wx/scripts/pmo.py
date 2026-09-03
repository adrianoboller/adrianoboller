#!/usr/bin/env python3
"""Painel do PMO da conversao WX: le os artefatos e gera .wx-migration/pmo/status.md.

Todo numero do painel sai de um arquivo (traceability.csv, gaps.md, decisions/,
orcamento.json, roteamento.jsonl, gate-status.md). Nada e digitado: numero
digitado a mao envelhece calado.

Subcomandos:
  iniciar   cria plano.json, orcamento.json, riscos.md, backlog e kanban (sem sobrescrever)
  status    regenera status.md e o imprime
  gastar    registra uso medido de tokens num gate
  pdca      abre ou fecha um ciclo PDCA; o fechamento grava em base_de_conhecimento.md
  kanban    regenera o quadro kanban.md da matriz de rastreabilidade, com limite de WIP
  sprint    abre ou fecha uma sprint Scrum (backlog, objetivo, definicao de pronto)
  painel    gera pmo/painel.html e pmo/relatorio.md (relatorio completo + kanban + base) para o aprovador abrir sem terminal
  relatorio imprime o relatorio completo em markdown (11 secoes, tudo medido dos arquivos)
  entregar  zipa a entrega da sprint para o stakeholder: resumo, tecnicas aplicadas, base de conhecimento,
            ferramentas usadas (docstrings dos .py), decisoes, lacunas e o kanban do fechamento

As tres tecnicas sao do mesmo PMO: o Scrum organiza o tempo (sprint por gate ou
onda), o Kanban mostra o fluxo e trava o WIP, e o PDCA e como cada hipotese de
trabalho vira aprendizado registrado -- inclusive quando morre.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import sys
from collections import Counter
from datetime import date
from pathlib import Path

GATES = ["G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7"]
COLUNAS = [
    ("A fazer", {"inventoried", "specified", ""}),
    ("Em andamento", {"implemented"}),
    ("Em verificacao", {"verified"}),
    ("Concluido", {"accepted"}),
    ("Bloqueado", {"blocked"}),
]
WIP_PADRAO = {"Em andamento": 6, "Em verificacao": 4}
DEFINICAO_DE_PRONTO = [
    "evidência de origem com localizador e hash",
    "implementação apontada em target_file/target_symbol",
    "teste automatizado ou roteiro reproduzível (test_id)",
    "resultado comparado (expected × actual) com test_result_ref",
    "aprovado por humano (approved_by, approved_at)",
    "confiança high ou medium; nunca low",
]
ESTADOS = ["inventoried", "specified", "implemented", "verified", "accepted", "blocked"]


def write_new(destination: Path, payload: str) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        return f"SKIPPED {destination}"
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(payload)
    return f"CREATED {destination}"


def iniciar(wx: Path, aprovador: str) -> list[str]:
    pmo = wx / "pmo"
    # Sem --aprovador, vale o que o questionario registrou (0.16 ou projeto.aprovador).
    qp = wx / "questionario.json"
    if not aprovador and qp.is_file():
        q = json.loads(qp.read_text(encoding="utf-8"))
        aprovador = ((q.get("0_empresa_e_projeto") or {}).get("0_16_aprovador") or {}).get("nome") or (q.get("projeto") or {}).get("aprovador") or ""
    plano = {
        "criado_em": date.today().isoformat(),
        "aprovador_padrao": aprovador,
        "gates": {g: {"status": "não iniciado", "aprovador": aprovador, "previsto_para": "", "decidido_em": ""} for g in GATES},
        "sprints": [],
        "kanban": {"wip": dict(WIP_PADRAO)},
        "scrum": {"duracao_dias": 10, "cerimonias": ["planejamento", "diario", "revisao", "retrospectiva"]},
    }
    # O bloco 0 do questionario (cronograma, prazo final, orcamento) chega por
    # pmo/projeto.json; marco com gate vira previsto_para, sem ninguem digitar.
    proj_p = pmo / "projeto.json"
    if proj_p.is_file():
        proj = json.loads(proj_p.read_text(encoding="utf-8"))
        plano["prazo_final"] = proj.get("prazo_final", "")
        for m in proj.get("marcos", []):
            if m.get("gate") in plano["gates"] and m.get("data"):
                plano["gates"][m["gate"]]["previsto_para"] = m["data"]
    backlog = (
        "# Backlog do produto\n\n"
        "Uma linha por item, priorizada de cima para baixo. O trace_id liga ao traceability.csv.\n\n"
        "| prioridade | trace_id | item | papel | gate | estimativa | sprint |\n| ---: | --- | --- | --- | --- | --- | --- |\n"
    )
    base = (
        "# Base de conhecimento\n\n"
        "Cada ciclo PDCA fechado deixa uma entrada aqui, frutífero ou não. A recusa com o\n"
        "número é resultado tão válido quanto o ganho: é o que impede a mesma ideia de\n"
        "voltar sem medição. Hipótese infrutífera gera a próxima hipótese.\n\n"
        "| ciclo | data | gate | hipótese | resultado | medido | aprendizado | próxima hipótese |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n"
    )
    orcamento = {
        "unidade": "tokens (medidos do campo de uso das respostas)",
        "gates": {g: {"tokens_previstos": 0, "tokens_gastos": 0, "chamadas": 0, "por_modelo": {"haiku": 0, "sonnet": 0, "opus": 0}} for g in GATES},
    }
    riscos = (
        "# RAID da conversão\n\n"
        "## Riscos\n\n| id | risco | prob. | impacto | resposta | dono | data |\n| --- | --- | --- | --- | --- | --- | --- |\n\n"
        "## Premissas\n\n| id | premissa | quem confirma | até |\n| --- | --- | --- | --- |\n\n"
        "## Issues\n\n| id | o que aconteceu | efeito | tratamento | dono | data |\n| --- | --- | --- | --- | --- | --- |\n\n"
        "## Dependências\n\n| id | dependemos de | para | quando | dono |\n| --- | --- | --- | --- | --- |\n"
    )
    return [
        write_new(pmo / "plano.json", json.dumps(plano, ensure_ascii=False, indent=2) + "\n"),
        write_new(pmo / "orcamento.json", json.dumps(orcamento, ensure_ascii=False, indent=2) + "\n"),
        write_new(pmo / "riscos.md", riscos),
        write_new(pmo / "sprints" / "LEIA-ME.md", "Um resumo por sprint, no formato de references/pmo.md.\n"),
        write_new(pmo / "backlog.md", backlog),
        write_new(pmo / "base_de_conhecimento.md", base),
        write_new(pmo / "pdca" / "LEIA-ME.md", "Um arquivo PDCA-NNN.md por ciclo: Plan, Do, Check, Act.\n"),
    ]


PAPEIS = {"A": "orquestrador", "B": "engenheiro", "C": "dba", "D": "zelador", "E": "designer", "F": "prova-real", "G": "qa", "H": "documentacao", "I": "versionador", "J": "pesquisador"}


def ler_backlog(wx: Path) -> dict[str, dict]:
    """trace_id -> {papel, item, gate, sprint}; o backlog e a fonte do dono de cada item."""
    p = wx / "pmo" / "backlog.md"
    itens: dict[str, dict] = {}
    if not p.is_file():
        return itens
    for l in p.read_text(encoding="utf-8").splitlines():
        if not l.startswith("|") or l.startswith("| prioridade") or l.startswith("| ---"):
            continue
        c = [x.strip() for x in l.strip().strip("|").split("|")]
        if len(c) >= 7 and re.match(r"^[A-Z]{2,3}-\d+", c[1]):
            itens[c[1]] = {"prioridade": c[0], "item": c[2], "papel": c[3].upper(), "gate": c[4], "estimativa": c[5], "sprint": c[6]}
    return itens


def backlog_acrescentar(wx: Path, trace_id: str, papel: str, item: str, gate: str, sprint: str) -> None:
    p = wx / "pmo" / "backlog.md"
    linhas = p.read_text(encoding="utf-8").splitlines() if p.is_file() else []
    if any(l.startswith(f"| ") and f"| {trace_id} |" in l for l in linhas):
        novas = []
        for l in linhas:
            if f"| {trace_id} |" in l:
                c = [x.strip() for x in l.strip().strip("|").split("|")]
                c[3] = papel; c[6] = sprint
                l = "| " + " | ".join(c) + " |"
            novas.append(l)
        linhas = novas
    else:
        n = len(ler_backlog(wx)) + 1
        linhas.append(f"| {n} | {trace_id} | {item} | {papel} | {gate} |  | {sprint} |")
    p.write_text("\n".join(linhas) + "\n", encoding="utf-8")


def proximo_id(pasta: Path, prefixo: str) -> str:
    numeros = [int(m.group(1)) for p in pasta.glob(f"{prefixo}-*.md") if (m := re.match(prefixo + r"-(\d+)", p.name))]
    return f"{prefixo}-{(max(numeros) + 1) if numeros else 1:03d}"


def pdca_abrir(wx: Path, gate: str, hipotese: str, medida: str, criterio: str) -> str:
    pasta = wx / "pmo" / "pdca"
    pasta.mkdir(parents=True, exist_ok=True)
    ident = proximo_id(pasta, "PDCA")
    corpo = (
        f"# {ident} — {hipotese}\n\n"
        f"- Gate: {gate}\n- Aberto em: {date.today().isoformat()}\n- Status: aberto\n\n"
        f"## Plan\n\n- Hipótese: {hipotese}\n- O que medir: {medida}\n- Critério de sucesso: {criterio}\n- Premissa a confirmar antes de fazer:\n\n"
        "## Do\n\n- O que foi feito:\n- Comando/evidência:\n\n"
        "## Check\n\n- Medido:\n- Critério atingido? \n\n"
        "## Act\n\n- Resultado: (frutífero | infrutífero)\n- Aprendizado:\n- Próxima hipótese:\n"
    )
    return write_new(pasta / f"{ident}.md", corpo)


def pdca_fechar(wx: Path, ident: str, resultado: str, medido: str, aprendizado: str, proxima: str) -> str:
    arquivo = wx / "pmo" / "pdca" / f"{ident}.md"
    texto = arquivo.read_text(encoding="utf-8")
    if "- Status: fechado" in texto:
        raise ValueError(f"{ident} já está fechado")
    if resultado == "infrutifero" and not proxima:
        raise ValueError("ciclo infrutífero exige --proxima: hipótese que morre gera a próxima")
    hip = re.search(r"^# PDCA-\d+ — (.*)$", texto, re.M)
    gate = re.search(r"^- Gate: (.*)$", texto, re.M)
    texto = texto.replace("- Status: aberto", f"- Status: fechado em {date.today().isoformat()}")
    texto = texto.replace("- Medido:\n", f"- Medido: {medido}\n").replace("- Resultado: (frutífero | infrutífero)", f"- Resultado: {resultado}")
    texto = texto.replace("- Aprendizado:\n", f"- Aprendizado: {aprendizado}\n").replace("- Próxima hipótese:\n", f"- Próxima hipótese: {proxima or '—'}\n")
    arquivo.write_text(texto, encoding="utf-8")
    base = wx / "pmo" / "base_de_conhecimento.md"
    if not base.is_file():
        raise ValueError("base_de_conhecimento.md não existe; rode `pmo.py iniciar`")
    linha = f"| {ident} | {date.today().isoformat()} | {gate.group(1) if gate else ''} | {hip.group(1) if hip else ''} | {resultado} | {medido} | {aprendizado} | {proxima or '—'} |\n"
    with base.open("a", encoding="utf-8") as f:
        f.write(linha)
    return f"{ident} fechado ({resultado}); base_de_conhecimento.md ganhou uma linha"


def kanban(wx: Path) -> str:
    tr = wx / "traceability.csv"
    rows: list[dict] = []
    if tr.is_file():
        with tr.open(encoding="utf-8", newline="") as f:
            rows = list(csv.DictReader(f))
    plano_p = wx / "pmo" / "plano.json"
    wip = dict(WIP_PADRAO)
    if plano_p.is_file():
        wip.update(json.loads(plano_p.read_text(encoding="utf-8")).get("kanban", {}).get("wip", {}))
    backlog = ler_backlog(wx)
    linhas = ["# Kanban", "", f"Gerado por `pmo.py kanban` em {date.today().isoformat()} de `traceability.csv` ({len(rows)} itens) e `backlog.md` (papel dono). Não edite: mude o estado na matriz; o papel muda no backlog, pelo PMO.", ""]
    violacoes = []
    for coluna, estados in COLUNAS:
        cartoes = [r for r in rows if r.get("status", "").strip() in estados]
        limite = wip.get(coluna)
        cab = f"## {coluna} ({len(cartoes)}" + (f" / WIP {limite}" if limite else "") + ")"
        if limite and len(cartoes) > limite:
            cab += "  **WIP ESTOURADO**"
            violacoes.append(f"{coluna}: {len(cartoes)} > {limite}")
        linhas += [cab, ""]
        for r in cartoes:
            resumo = (r.get("rule_summary") or "").strip()[:70]
            dono = r.get("approved_by") or r.get("target_component") or ""
            papel = backlog.get(r.get("trace_id", ""), {}).get("papel", "")
            tag = f"[{papel} {PAPEIS.get(papel, '?')}] " if papel else "[sem papel] "
            linhas.append(f"- {tag}`{r.get('trace_id','?')}` {resumo}" + (f" — {dono}" if dono else "") + (f" — {r.get('notes','').strip()[:60]}" if coluna == "Bloqueado" and r.get("notes") else ""))
        if not cartoes:
            linhas.append("- (vazio)")
        linhas.append("")
    if violacoes:
        linhas += ["## Limite de WIP estourado", "", "Regra: coluna acima do limite não recebe cartão novo; termina-se antes de começar.", ""] + [f"- {v}" for v in violacoes] + [""]
    texto = "\n".join(linhas)
    (wx / "pmo").mkdir(parents=True, exist_ok=True)
    (wx / "pmo" / "kanban.md").write_text(texto + "\n", encoding="utf-8")
    return texto


def sprint_abrir(wx: Path, nome: str, objetivo: str, gate: str, itens: list[str], aprovador: str) -> str:
    plano_p = wx / "pmo" / "plano.json"
    plano = json.loads(plano_p.read_text(encoding="utf-8"))
    if any(s.get("status") == "aberta" for s in plano["sprints"]):
        raise ValueError("já existe sprint aberta; feche-a antes (Scrum: uma sprint por vez)")
    numero = len(plano["sprints"]) + 1
    # item pode vir como ID ou ID:PAPEL; o papel vai para o backlog, que e quem diz o dono
    tr = wx / "traceability.csv"
    resumos: dict[str, str] = {}
    if tr.is_file():
        with tr.open(encoding="utf-8", newline="") as f:
            resumos = {r.get("trace_id"): (r.get("rule_summary") or "") for r in csv.DictReader(f)}
    limpos = []
    for it in itens:
        tid, _, papel = it.partition(":")
        papel = papel.upper()
        if papel and papel not in PAPEIS:
            raise ValueError(f"papel inválido em {it!r}: use uma letra de A a J")
        if papel:
            backlog_acrescentar(wx, tid, papel, resumos.get(tid, ""), gate, f"{numero:02d}")
        limpos.append(tid)
    itens = limpos
    sprint = {"numero": numero, "nome": nome, "objetivo": objetivo, "gate": gate, "itens": itens, "aberta_em": date.today().isoformat(), "status": "aberta", "aprovador": aprovador}
    plano["sprints"].append(sprint)
    plano_p.write_text(json.dumps(plano, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return f"sprint {numero} «{nome}» aberta para {gate} com {len(itens)} itens do backlog"


def sprint_fechar(wx: Path, decisao: str, pedido: str) -> str:
    plano_p = wx / "pmo" / "plano.json"
    plano = json.loads(plano_p.read_text(encoding="utf-8"))
    abertas = [s for s in plano["sprints"] if s.get("status") == "aberta"]
    if not abertas:
        raise ValueError("nenhuma sprint aberta")
    s = abertas[0]
    rows: list[dict] = []
    tr = wx / "traceability.csv"
    if tr.is_file():
        with tr.open(encoding="utf-8", newline="") as f:
            rows = list(csv.DictReader(f))
    por_id = {r.get("trace_id"): r for r in rows}
    prontos = [i for i in s["itens"] if por_id.get(i, {}).get("status") == "accepted"]
    nao = [i for i in s["itens"] if i not in prontos]
    orc = json.loads((wx / "pmo" / "orcamento.json").read_text(encoding="utf-8"))["gates"].get(s["gate"], {})
    base = wx / "pmo" / "base_de_conhecimento.md"
    pdcas = [l for l in base.read_text(encoding="utf-8").splitlines() if l.startswith("| PDCA-")] if base.is_file() else []
    hoje = date.today().isoformat()
    md = [
        f"# Resumo de Sprint — {s['nome']}", "",
        f"```text\nSprint {s['numero']:02d} | {s['gate']} | {hoje}\n```", "",
        "## 1. Identificação", "", "| Campo | Valor |", "|---|---|",
        f"| Sprint | {s['numero']:02d} — {s['nome']} |", f"| Gate | {s['gate']} |", f"| Aberta / fechada | {s['aberta_em']} / {hoje} |",
        f"| Objetivo | {s['objetivo']} |", f"| Aprovador | {s.get('aprovador','')} |", "",
        "## 2. Solicitação", "", pedido or "(registrar o pedido literal)", "",
        "## 3. Insumos recebidos", "", "| Arquivo | Bytes | Situação |", "|---|---:|---|", "| (do inventário do gate) | | |", "",
        "## 4. Atividades realizadas", "", "1. (do ledger de tarefas do orquestrador)", "",
        "## 5. Itens da sprint e definição de pronto", "",
        f"Prontos (accepted): {len(prontos)}/{len(s['itens'])} — " + (", ".join(prontos) if prontos else "nenhum") + ".",
        f"Não concluídos: " + (", ".join(nao) if nao else "nenhum") + ".", "",
        "Definição de pronto (todas obrigatórias):", ""] + [f"- {d}" for d in DEFINICAO_DE_PRONTO] + ["",
        "## 6. Decisões técnicas (DEC-*)", "", "| ID | Decisão | Fundamento |", "|---|---|---|", "| | | |", "",
        "## 7. Testes executados", "", "| # | Teste | Resultado | Evidência | Status |", "|---|---|---|---|---|", "| | | | | |", "",
        "## 8. Problemas encontrados", "", "| # | Problema | Tratamento |", "|---|---|---|", "| | | |", "",
        "## 9. Ciclos PDCA fechados na sprint", "", "| ciclo | data | gate | hipótese | resultado | medido | aprendizado | próxima hipótese |", "| --- | --- | --- | --- | --- | --- | --- | --- |"] + (pdcas or ["| (nenhum) | | | | | | | |"]) + ["",
        "## 10. Pendências e gaps", "", "- itens não concluídos voltam ao backlog: " + (", ".join(nao) if nao else "nenhum"), "",
        "## 11. Orçamento da sprint", "", "| previsto | gasto | haiku | sonnet | opus |", "|---:|---:|---:|---:|---:|",
        f"| {orc.get('tokens_previstos',0)} | {orc.get('tokens_gastos',0)} | {orc.get('por_modelo',{}).get('haiku',0)} | {orc.get('por_modelo',{}).get('sonnet',0)} | {orc.get('por_modelo',{}).get('opus',0)} |", "",
        f"## 12. Retrospectiva e decisão do gate", "", f"- Decisão registrada: **{decisao}**", "- O que manter:", "- O que mudar (vira hipótese PDCA da próxima sprint):", "",
    ]
    s["status"] = "fechada"; s["fechada_em"] = hoje; s["decisao"] = decisao; s["prontos"] = len(prontos)
    plano_p.write_text(json.dumps(plano, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    destino = wx / "pmo" / "sprints" / f"sprint-{s['numero']:02d}-{s['gate']}-{hoje}.md"
    saida = write_new(destino, "\n".join(md) + "\n")
    # Fechar sprint e o momento em que o aprovador quer ver o todo: o painel sai junto.
    p = painel(wx)
    return f"sprint {s['numero']} fechada ({decisao}); prontos {len(prontos)}/{len(s['itens'])}; {saida}; CREATED {p} (+ relatorio.md)"


def gastar(wx: Path, gate: str, modelo: str, tokens: int, chamadas: int) -> str:
    p = wx / "pmo" / "orcamento.json"
    dados = json.loads(p.read_text(encoding="utf-8"))
    g = dados["gates"][gate]
    g["tokens_gastos"] += tokens
    g["chamadas"] += chamadas
    g["por_modelo"][modelo] = g["por_modelo"].get(modelo, 0) + tokens
    p.write_text(json.dumps(dados, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    prev = g["tokens_previstos"]
    pct = f"{100.0 * g['tokens_gastos'] / prev:.0f}%" if prev else "sem previsão"
    return f"{gate}: {g['tokens_gastos']} tokens gastos ({pct})"


def status(wx: Path) -> str:
    linhas = ["# Painel do PMO", "", f"Gerado por `pmo.py` em {date.today().isoformat()}. Todo número tem fonte ao lado.", ""]

    # Gates
    plano_p = wx / "pmo" / "plano.json"
    linhas += ["## Gates", ""]
    if plano_p.is_file():
        plano = json.loads(plano_p.read_text(encoding="utf-8"))
        linhas += ["| gate | status | aprovador | previsto | decidido |", "| --- | --- | --- | --- | --- |"]
        for g in GATES:
            d = plano["gates"].get(g, {})
            linhas.append(f"| {g} | {d.get('status','?')} | {d.get('aprovador','')} | {d.get('previsto_para','')} | {d.get('decidido_em','')} |")
        linhas.append("")
        linhas.append("Fonte: `pmo/plano.json`. MEDIDO.")
    else:
        linhas.append("INDISPONÍVEL: `pmo/plano.json` não existe; rode `pmo.py iniciar`.")
    linhas.append("")

    # Prazo final e orcamento financeiro vem do bloco 0 do questionario
    proj_p = wx / "pmo" / "projeto.json"
    linhas += ["## Prazo e orçamento do contrato", ""]
    if proj_p.is_file():
        proj = json.loads(proj_p.read_text(encoding="utf-8"))
        prazo = proj.get("prazo_final", "")
        if prazo:
            try:
                dias = (date.fromisoformat(prazo) - date.today()).days
                linhas.append(f"- Prazo final de entrega: {prazo} ({dias} dias {'restantes' if dias >= 0 else 'de atraso'}, contados de hoje). MEDIDO.")
            except ValueError:
                linhas.append(f"- Prazo final de entrega: {prazo} (data fora do formato AAAA-MM-DD; não dá para contar dias).")
        else:
            linhas.append("- Prazo final de entrega: INDISPONÍVEL (não informado no questionário).")
        o = proj.get("orcamento_financeiro", {})
        linhas.append(f"- Orçamento: {o.get('valor') if o.get('valor') is not None else 'INDISPONÍVEL'} {o.get('moeda', '')} ({o.get('base') or 'base não informada'}), aprovado por {o.get('aprovado_por') or '(pendente)'}.")
        linhas.append(f"- Marcos: {len(proj.get('marcos', []))}; riscos iniciais: {proj.get('riscos_iniciais', 0)}; pessoas: {proj.get('pessoas', 0)}. Fonte: `pmo/projeto.json`.")
    else:
        linhas.append("INDISPONÍVEL: `pmo/projeto.json` não existe; o bloco 0 do questionário não foi aplicado.")
    linhas.append("")

    # Rastreabilidade
    tr = wx / "traceability.csv"
    linhas += ["## Itens rastreados", ""]
    if tr.is_file():
        with tr.open(encoding="utf-8", newline="") as f:
            rows = list(csv.DictReader(f))
        c = Counter(r.get("status", "").strip() for r in rows)
        total = len(rows)
        linhas += ["| estado | itens |", "| --- | ---: |"]
        for e in ESTADOS:
            linhas.append(f"| {e} | {c.get(e, 0)} |")
        outros = total - sum(c.get(e, 0) for e in ESTADOS)
        if outros:
            linhas.append(f"| (outro/vazio) | {outros} |")
        linhas.append(f"| **total** | **{total}** |")
        linhas.append("")
        if total:
            linhas.append(f"Concluído (accepted ÷ total): {c.get('accepted',0)}/{total} = {100.0*c.get('accepted',0)/total:.1f}%. Fonte: `traceability.csv`. MEDIDO.")
        else:
            linhas.append("Matriz vazia: percentual de conclusão INDISPONÍVEL (sem denominador).")
    else:
        linhas.append("INDISPONÍVEL: `traceability.csv` não existe.")
    linhas.append("")

    # Lacunas
    gaps = wx / "gaps.md"
    linhas += ["## Lacunas (GAP-*)", ""]
    if gaps.is_file():
        texto = gaps.read_text(encoding="utf-8")
        ids = re.findall(r"\bGAP-\d+\b", texto)
        sev = Counter(m.lower() for m in re.findall(r"\|\s*(cr[ií]tica|alta|m[eé]dia|baixa)\s*\|", texto, flags=re.I))
        linhas.append(f"{len(set(ids))} lacunas identificadas" + (f" ({', '.join(f'{k}: {v}' for k, v in sev.items())})" if sev else "") + ". Fonte: `gaps.md`. MEDIDO.")
    else:
        linhas.append("INDISPONÍVEL: `gaps.md` não existe.")
    linhas.append("")

    # Decisões
    dec = wx / "decisions"
    linhas += ["## Decisões pendentes (DEC-*)", ""]
    if dec.is_dir():
        pend = []
        for p in sorted(dec.glob("DEC-*.md")):
            t = p.read_text(encoding="utf-8")
            if re.search(r"Status:\s*proposed", t):
                pend.append(p.stem)
        linhas.append(f"{len(pend)} pendentes: {', '.join(pend) if pend else 'nenhuma'}. Fonte: `decisions/`. MEDIDO.")
    else:
        linhas.append("0 registradas: `decisions/` não existe.")
    linhas.append("")

    # Orçamento
    orc = wx / "pmo" / "orcamento.json"
    linhas += ["## Orçamento de tokens por gate", ""]
    if orc.is_file():
        o = json.loads(orc.read_text(encoding="utf-8"))
        linhas += ["| gate | previsto | gasto | % | chamadas | haiku | sonnet | opus |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
        for g in GATES:
            d = o["gates"][g]
            pct = f"{100.0*d['tokens_gastos']/d['tokens_previstos']:.0f}%" if d["tokens_previstos"] else "—"
            pm = d.get("por_modelo", {})
            linhas.append(f"| {g} | {d['tokens_previstos']} | {d['tokens_gastos']} | {pct} | {d['chamadas']} | {pm.get('haiku',0)} | {pm.get('sonnet',0)} | {pm.get('opus',0)} |")
        linhas.append("")
        linhas.append("Fonte: `pmo/orcamento.json`, alimentado por `pmo.py gastar` com o uso medido. Gate sem previsão: `—`.")
    else:
        linhas.append("INDISPONÍVEL: `pmo/orcamento.json` não existe.")
    linhas.append("")

    # Scrum, Kanban e PDCA
    linhas += ["## Scrum, Kanban e PDCA", ""]
    if plano_p.is_file():
        plano = json.loads(plano_p.read_text(encoding="utf-8"))
        abertas = [s for s in plano.get("sprints", []) if s.get("status") == "aberta"]
        fechadas = [s for s in plano.get("sprints", []) if s.get("status") == "fechada"]
        if abertas:
            s = abertas[0]
            linhas.append(f"Sprint aberta: {s['numero']:02d} «{s['nome']}» ({s['gate']}), {len(s['itens'])} itens, desde {s['aberta_em']}.")
        else:
            linhas.append("Nenhuma sprint aberta.")
        linhas.append(f"Sprints fechadas: {len(fechadas)}" + (" (" + ", ".join(f"{s['numero']:02d}: {s.get('prontos',0)}/{len(s['itens'])} prontos, {s.get('decisao','')}" for s in fechadas) + ")" if fechadas else "") + ". Fonte: `pmo/plano.json`. MEDIDO.")
    kb = wx / "pmo" / "kanban.md"
    if kb.is_file():
        kt = kb.read_text(encoding="utf-8")
        est = re.findall(r"^## (.+?) \((\d+)(?: / WIP (\d+))?\)(.*)$", kt, re.M)
        linhas.append("Kanban: " + ", ".join(f"{c} {n}" + (f"/{w}" if w else "") + (" ESTOURADO" if "ESTOURADO" in x else "") for c, n, w, x in est) + ". Fonte: `pmo/kanban.md`. MEDIDO.")
    base = wx / "pmo" / "base_de_conhecimento.md"
    if base.is_file():
        bl = [l for l in base.read_text(encoding="utf-8").splitlines() if l.startswith("| PDCA-")]
        fr = sum(1 for l in bl if "| frutifero |" in l); inf = sum(1 for l in bl if "| infrutifero |" in l)
        abertos = len([p for p in (wx / "pmo" / "pdca").glob("PDCA-*.md") if "- Status: aberto" in p.read_text(encoding="utf-8")]) if (wx / "pmo" / "pdca").is_dir() else 0
        linhas.append(f"PDCA: {abertos} abertos; {len(bl)} fechados na base de conhecimento ({fr} frutíferos, {inf} infrutíferos). Fonte: `pmo/base_de_conhecimento.md`. MEDIDO.")
    linhas.append("")

    # Roteamento
    rot = wx / "pmo" / "roteamento.jsonl"
    linhas += ["## Roteamento de modelos", ""]
    if rot.is_file():
        decs = [json.loads(l) for l in rot.read_text(encoding="utf-8").splitlines() if l.strip()]
        c = Counter(d["modelo"] for d in decs)
        fall = sum(1 for d in decs if any(m.startswith("fallback") for m in d.get("motivos", [])))
        bloq = sum(1 for d in decs if d.get("estado") == "BLOQUEADO")
        linhas.append(f"{len(decs)} decisões: " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())) + f"; {fall} fallbacks; {bloq} bloqueadas por orçamento. Fonte: `pmo/roteamento.jsonl`. MEDIDO.")
    else:
        linhas.append("0 decisões registradas.")
    linhas.append("")
    return "\n".join(linhas)


def _tabela_md(caminho: Path, prefixo_id: str) -> list[str]:
    """Copia as linhas de tabela de um .md que comecem por um id (GAP-, RSK-...), com o cabecalho."""
    if not caminho.is_file():
        return []
    linhas = caminho.read_text(encoding="utf-8").splitlines()
    saida, cab = [], None
    for i, l in enumerate(linhas):
        if l.startswith("| id") or l.startswith("| ID"):
            cab = (l, linhas[i + 1] if i + 1 < len(linhas) else "| --- |")
        if l.startswith(f"| {prefixo_id}") and cab:
            if not saida:
                saida += list(cab)
            saida.append(l)
    return saida


def relatorio(wx: Path) -> str:
    """Relatorio detalhado do projeto: o status() mais o contrato, a rastreabilidade por tipo,
    as tabelas inteiras de lacunas, decisoes e riscos, a historia das sprints, o roteamento
    por classe e os proximos passos. Tudo lido dos arquivos; o que nao existe fica INDISPONIVEL."""
    hoje = date.today()
    L = ["# Relatório do projeto", "", f"Gerado por `pmo.py relatorio` em {hoje.isoformat()}. Todo número tem fonte ao lado; o que não foi medido está marcado INDISPONÍVEL.", ""]

    # 1. Contrato (bloco 0 do questionario)
    L += ["## 1. Empresa e contrato", ""]
    proj_p = wx / "pmo" / "projeto.json"
    emp_p = wx / "empresa.md"
    if proj_p.is_file():
        pj = json.loads(proj_p.read_text(encoding="utf-8"))
        L.append(f"- Softhouse: {pj.get('softhouse') or '(pendente)'}")
        L.append(f"- Solicitação: {pj.get('solicitacao') or '(pendente)'}")
        prazo = pj.get("prazo_final", "")
        if prazo:
            try:
                d = (date.fromisoformat(prazo) - hoje).days
                L.append(f"- Prazo final: **{prazo}** ({d} dias {'restantes' if d >= 0 else 'de atraso'}). MEDIDO.")
            except ValueError:
                L.append(f"- Prazo final: {prazo} (fora do formato AAAA-MM-DD)")
        else:
            L.append("- Prazo final: INDISPONÍVEL")
        o = pj.get("orcamento_financeiro", {})
        L.append(f"- Orçamento: {o.get('valor') if o.get('valor') is not None else 'INDISPONÍVEL'} {o.get('moeda', '')} ({o.get('base') or 'base não informada'}), aprovado por {o.get('aprovado_por') or '(pendente)'}")
        if pj.get("marcos"):
            L += ["", "| marco | data | gate |", "| --- | --- | --- |"] + [f"| {m.get('marco','')} | {m.get('data','')} | {m.get('gate','')} |" for m in pj["marcos"]]
        L += ["", "Fonte: `pmo/projeto.json` (bloco 0 do questionário)" + (", `empresa.md`" if emp_p.is_file() else "") + ".", ""]
    else:
        L += ["INDISPONÍVEL: `pmo/projeto.json` não existe; o bloco 0 do questionário não foi aplicado.", ""]

    # 2. Painel resumido (status)
    st, pular = [], False
    for l in status(wx).split("\n")[3:]:
        if l.startswith("## "):
            pular = l.startswith("## Prazo e orçamento")  # ja esta na secao 1
        if not pular:
            st.append(l.replace("## ", "### "))
    L += ["## 2. Painel", ""] + st + [""]

    # 3. Rastreabilidade por tipo e itens bloqueados
    tr = wx / "traceability.csv"
    L += ["## 3. Rastreabilidade por tipo", ""]
    if tr.is_file():
        with tr.open(encoding="utf-8", newline="") as f:
            rows = list(csv.DictReader(f))
        tipos = {}
        for r in rows:
            k = r.get("kind") or "?"
            tipos.setdefault(k, Counter())[r.get("status") or "?"] += 1
        estados = ["inventoried", "specified", "implemented", "verified", "accepted", "blocked"]
        L += ["| tipo | " + " | ".join(estados) + " | total |", "| --- | " + " | ".join("---:" for _ in estados) + " | ---: |"]
        for k in sorted(tipos):
            L.append(f"| {k} | " + " | ".join(str(tipos[k].get(e, 0)) for e in estados) + f" | {sum(tipos[k].values())} |")
        bloq = [r for r in rows if r.get("status") == "blocked"]
        L += ["", f"Itens bloqueados: {len(bloq)}" + ("" if not bloq else " — " + "; ".join(f"{r['trace_id']} ({r.get('notes') or r.get('rule_summary') or 'sem nota'})" for r in bloq)) + ". Fonte: `traceability.csv`. MEDIDO.", ""]
    else:
        L += ["INDISPONÍVEL: `traceability.csv` não existe.", ""]

    # 4. Lacunas, 5. Decisoes, 6. Riscos
    L += ["## 4. Lacunas (GAP-*)", ""]
    t = _tabela_md(wx / "gaps.md", "GAP-")
    L += (t + ["", "Fonte: `gaps.md`."]) if t else ["Nenhuma lacuna registrada" + ("" if (wx / "gaps.md").is_file() else " (`gaps.md` não existe)") + "."]
    L.append("")
    L += ["## 5. Decisões (DEC-*)", ""]
    dec = wx / "decisions"
    if dec.is_dir() and any(dec.glob("DEC-*.md")):
        L += ["| id | título | status |", "| --- | --- | --- |"]
        for p in sorted(dec.glob("DEC-*.md")):
            txt = p.read_text(encoding="utf-8")
            titulo = next((l.lstrip("# ").strip() for l in txt.splitlines() if l.startswith("# ")), p.stem)
            m = re.search(r"Status:\s*(\w+)", txt)
            L.append(f"| {p.stem} | {titulo} | {m.group(1) if m else 'INDISPONÍVEL'} |")
        L += ["", "Fonte: `decisions/`."]
    else:
        L.append("Nenhuma decisão registrada em `decisions/`.")
    L.append("")
    L += ["## 6. Riscos (RAID)", ""]
    t = _tabela_md(wx / "pmo" / "riscos.md", "RSK-")
    L += (t + ["", "Fonte: `pmo/riscos.md`."]) if t else ["Nenhum risco registrado em `pmo/riscos.md`."]
    L.append("")

    # 7. Sprints
    L += ["## 7. Sprints", ""]
    plano_p = wx / "pmo" / "plano.json"
    if plano_p.is_file():
        plano = json.loads(plano_p.read_text(encoding="utf-8"))
        sp = plano.get("sprints", [])
        if sp:
            L += ["| nº | nome | gate | itens | prontos | decisão | aberta em | fechada em |", "| ---: | --- | --- | ---: | ---: | --- | --- | --- |"]
            for s in sp:
                L.append(f"| {s['numero']:02d} | {s['nome']} | {s['gate']} | {len(s['itens'])} | {s.get('prontos', '—') if s.get('status') == 'fechada' else '—'} | {s.get('decisao', 'aberta')} | {s.get('aberta_em', '')} | {s.get('fechada_em', '')} |")
            fech = [s for s in sp if s.get("status") == "fechada"]
            if fech:
                tot = sum(len(s["itens"]) for s in fech); pr = sum(s.get("prontos", 0) for s in fech)
                L += ["", f"Vazão medida: {pr}/{tot} itens prontos em {len(fech)} sprint(s) fechada(s) ({100.0*pr/tot:.0f}%)." if tot else ""]
            L += ["", "Fonte: `pmo/plano.json`; resumos em `pmo/sprints/`."]
        else:
            L.append("Nenhuma sprint registrada.")
    else:
        L.append("INDISPONÍVEL: `pmo/plano.json` não existe.")
    L.append("")

    # 8. PDCA
    L += ["## 8. Ciclos PDCA e base de conhecimento", ""]
    base = wx / "pmo" / "base_de_conhecimento.md"
    bl = [l for l in base.read_text(encoding="utf-8").splitlines() if l.startswith("|")] if base.is_file() else []
    L += (bl + ["", "Fonte: `pmo/base_de_conhecimento.md`."]) if len(bl) > 2 else ["Nenhum ciclo fechado ainda."]
    L.append("")

    # 9. Roteamento por classe e modelo
    L += ["## 9. Roteamento de modelos", ""]
    rot = wx / "pmo" / "roteamento.jsonl"
    if rot.is_file():
        decs = [json.loads(l) for l in rot.read_text(encoding="utf-8").splitlines() if l.strip()]
        if decs:
            por = {}
            for d in decs:
                classe = next((m.split(" ", 1)[1] for m in d.get("motivos", []) if m.startswith("classe ")), d.get("classe", "?"))
                por.setdefault(classe, Counter())[d.get("modelo", "?")] += 1
            modelos = sorted({d.get("modelo", "?") for d in decs})
            L += ["| classe | " + " | ".join(modelos) + " |", "| --- | " + " | ".join("---:" for _ in modelos) + " |"]
            for c in sorted(por):
                L.append(f"| {c} | " + " | ".join(str(por[c].get(m, 0)) for m in modelos) + " |")
            esc = Counter(m for d in decs for m in d.get("motivos", []))
            L += ["", "Motivos de escalada e rebaixamento: " + (", ".join(f"{k} {v}" for k, v in esc.most_common(8)) if esc else "nenhum") + ". Fonte: `pmo/roteamento.jsonl`. MEDIDO."]
        else:
            L.append("0 decisões registradas.")
    else:
        L.append("0 decisões registradas (`pmo/roteamento.jsonl` não existe).")
    L.append("")

    # 10. Processo e entrega
    L += ["## 10. Processo de conversão e entrega", ""]
    proc = wx / "processo-de-conversao.md"
    if proc.is_file():
        pt = proc.read_text(encoding="utf-8")
        est = re.findall(r"Estrategia: \*\*(.+?)\*\*", pt)
        L.append(f"- Estratégia: backend **{est[0] if est else 'pendente'}**, frontend **{est[1] if len(est) > 1 else 'pendente'}**. Fonte: `processo-de-conversao.md`.")
    else:
        L.append("- Processo de conversão: INDISPONÍVEL (`processo-de-conversao.md` não existe).")
    ent = wx / "entrega.json"
    if ent.is_file():
        e = json.loads(ent.read_text(encoding="utf-8"))
        g = e.get("github", {})
        L.append(f"- Destino: {g.get('url') or '(pendente)'} (branch `{g.get('branch', 'main')}`, usuário {g.get('usuario') or '?'}), diretório `{e.get('diretorio_destino') or '?'}`; credencial em `{e.get('credencial_ref') or '(pendente)'}`. Fonte: `entrega.json`.")
    else:
        L.append("- Destino: INDISPONÍVEL (`entrega.json` não existe).")
    L.append("")

    # 11. Proximos passos, derivados do que esta acima
    L += ["## 11. Próximos passos (derivados)", ""]
    passos = []
    if plano_p.is_file():
        plano = json.loads(plano_p.read_text(encoding="utf-8"))
        prox = next((g for g in GATES if plano["gates"].get(g, {}).get("status") != "aprovado"), None)
        if prox:
            passos.append(f"Próximo gate: **{prox}** ({plano['gates'][prox].get('status', '?')}; previsto para {plano['gates'][prox].get('previsto_para') or 'sem data'}), aprovador {plano['gates'][prox].get('aprovador', '?')}.")
        ab = [s for s in plano.get("sprints", []) if s.get("status") == "aberta"]
        if ab:
            passos.append(f"Fechar a sprint {ab[0]['numero']:02d} «{ab[0]['nome']}» com `pmo.py sprint fechar`.")
    if tr.is_file():
        with tr.open(encoding="utf-8", newline="") as f:
            bloq = [r["trace_id"] for r in csv.DictReader(f) if r.get("status") == "blocked"]
        if bloq:
            passos.append(f"Desbloquear {len(bloq)} item(ns): {', '.join(bloq)}.")
    if dec.is_dir():
        pend = [p.stem for p in sorted(dec.glob("DEC-*.md")) if re.search(r"Status:\s*proposed", p.read_text(encoding="utf-8"))]
        if pend:
            passos.append(f"Decidir {', '.join(pend)}.")
    if (wx / "gaps.md").is_file():
        crit = len(re.findall(r"\|\s*cr[ií]tica\s*\|", (wx / "gaps.md").read_text(encoding="utf-8"), flags=re.I))
        if crit:
            passos.append(f"Resolver {crit} lacuna(s) crítica(s) antes do próximo gate.")
    if proj_p.is_file():
        pj = json.loads(proj_p.read_text(encoding="utf-8"))
        try:
            if pj.get("prazo_final") and (date.fromisoformat(pj["prazo_final"]) - hoje).days < 0:
                passos.append("Prazo final vencido: renegociar o cronograma com o aprovador.")
        except ValueError:
            pass
    L += [f"{i}. {p}" for i, p in enumerate(passos, 1)] or ["Nada derivado: os arquivos do PMO não existem ainda (`pmo.py iniciar`)."]
    L.append("")
    return "\n".join(L)


def painel(wx: Path) -> Path:
    """HTML do painel, gerado do mesmo relatorio()/kanban(): nenhum numero e digitado aqui."""
    import html as _html
    md_status = relatorio(wx)
    (wx / "pmo" / "relatorio.md").write_text(md_status + "\n", encoding="utf-8")
    md_kanban = kanban(wx)
    base = wx / "pmo" / "base_de_conhecimento.md"
    md_base = base.read_text(encoding="utf-8") if base.is_file() else "INDISPONÍVEL"

    def md2html(md: str) -> str:
        saida, tabela = [], []
        def fecha():
            nonlocal tabela
            if tabela:
                linhas = [l for l in tabela if not re.match(r"^\|\s*-", l)]
                cab, corpo = linhas[0], linhas[1:]
                celulas = lambda l, tag: "".join(f"<{tag}>{_html.escape(c.strip())}</{tag}>" for c in l.strip().strip("|").split("|"))
                saida.append("<table><thead><tr>" + celulas(cab, "th") + "</tr></thead><tbody>" + "".join("<tr>" + celulas(l, "td") + "</tr>" for l in corpo) + "</tbody></table>")
                tabela = []
        for l in md.splitlines():
            if l.startswith("|"):
                tabela.append(l); continue
            fecha()
            if l.startswith("# "): saida.append(f"<h1>{_html.escape(l[2:])}</h1>")
            elif l.startswith("## "): saida.append(f"<h2>{_html.escape(l[3:])}</h2>")
            elif l.startswith("### "): saida.append(f"<h3>{_html.escape(l[4:])}</h3>")
            elif re.match(r"^\d+\. ", l):
                item = re.sub(r"^\d+\. ", "", l)
                saida.append(f"<li>{_html.escape(item)}</li>")
            elif l.startswith("- "): saida.append(f"<li>{_html.escape(l[2:])}</li>")
            elif l.strip(): saida.append(f"<p>{_html.escape(l)}</p>")
        fecha()
        return "\n".join(saida).replace("**", "")
    css = ("<style>:root{--g:#FBFAF7;--p:#fff;--i:#14161F;--m:#7A7E90;--l:#E4E2DB;--a:#C63C0A}"
           "@media(prefers-color-scheme:dark){:root{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#8A8FA6;--l:#252A42;--a:#E2261C}}"
           "body{margin:0;background:var(--g);color:var(--i);font:15px/1.5 system-ui,sans-serif}.w{max-width:1000px;margin:0 auto;padding:28px 20px}"
           "h1{font-size:26px;color:var(--a);margin:.2em 0}h2{font-size:17px;margin:1.6em 0 .4em;border-bottom:1px solid var(--l);padding-bottom:4px}h3{font-size:14px;margin:1.2em 0 .3em;color:var(--m);text-transform:uppercase;letter-spacing:.05em}"
           "table{border-collapse:collapse;width:100%;font-size:14px;background:var(--p)}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--l);vertical-align:top}th{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--m)}"
           "td:nth-child(n+2):not(:last-child){font-variant-numeric:tabular-nums}li{margin:.2em 0}code{font-family:ui-monospace,monospace}.est{color:var(--a);font-weight:700}"
           ".col{display:grid;grid-template-columns:1fr 1fr;gap:24px}@media(max-width:800px){.col{grid-template-columns:1fr}}</style>")
    corpo = (f"<!doctype html><html lang=\"pt-BR\"><head><meta charset=\"utf-8\"><title>Painel do PMO</title>{css}</head><body><div class=\"w\">"
             + md2html(md_status).replace("ESTOURADO", "<span class=\"est\">ESTOURADO</span>")
             + "<div class=\"col\"><div>" + md2html(md_kanban).replace("ESTOURADO", "<span class=\"est\">ESTOURADO</span>") + "</div><div>" + md2html(md_base) + "</div></div>"
             + f"<p style=\"color:var(--m);font-size:12px\">Gerado por pmo.py painel em {date.today().isoformat()}; regenerar em vez de editar. O mesmo conteúdo em texto está em pmo/relatorio.md.</p></div></body></html>")
    destino = wx / "pmo" / "painel.html"
    destino.write_text(corpo, encoding="utf-8")
    return destino


def entregar(wx: Path, numero: int | None, plugin_root: Path | None) -> Path:
    """Zip da sprint para o stakeholder. Tudo dentro e gerado ou copiado; nada e digitado aqui."""
    import zipfile, ast
    plano = json.loads((wx / "pmo" / "plano.json").read_text(encoding="utf-8"))
    sprints = plano.get("sprints", [])
    if not sprints:
        raise ValueError("nenhuma sprint no plano")
    s = next((x for x in sprints if x["numero"] == numero), None) if numero else sprints[-1]
    if s is None:
        raise ValueError(f"sprint {numero} não existe")
    n = s["numero"]
    pasta = wx / "pmo" / "entregas"
    pasta.mkdir(parents=True, exist_ok=True)
    destino = pasta / f"sprint-{n:02d}-{s['gate']}-{date.today().isoformat()}.zip"
    hoje = date.today().isoformat()

    # tecnicas aplicadas: contadas dos artefatos
    base = wx / "pmo" / "base_de_conhecimento.md"
    bl = [l for l in base.read_text(encoding="utf-8").splitlines() if l.startswith("| PDCA-")] if base.is_file() else []
    kb = kanban(wx)
    est = re.findall(r"^## (.+?) \((\d+)(?: / WIP (\d+))?\)(.*)$", kb, re.M)
    rot = wx / "pmo" / "roteamento.jsonl"
    decs = [json.loads(l) for l in rot.read_text(encoding="utf-8").splitlines() if l.strip()] if rot.is_file() else []
    orc = json.loads((wx / "pmo" / "orcamento.json").read_text(encoding="utf-8"))["gates"].get(s["gate"], {}) if (wx / "pmo" / "orcamento.json").is_file() else {}
    tecnicas = [
        f"# Técnicas aplicadas na sprint {n:02d} ({s['gate']})", "",
        f"Gerado por `pmo.py entregar` em {hoje}. Cada número tem a fonte ao lado; o que não tem fonte está marcado INDISPONÍVEL.", "",
        "## Scrum", "",
        f"- Sprint {n:02d} «{s['nome']}», objetivo: {s['objetivo']}. Aberta em {s['aberta_em']}" + (f", fechada em {s['fechada_em']} com decisão {s.get('decisao','')}" if s.get("status") == "fechada" else ", ainda aberta") + ".",
        f"- Itens: {len(s['itens'])} ({', '.join(s['itens']) or 'nenhum'}); prontos pela definição de pronto: {s.get('prontos', 'INDISPONÍVEL (sprint aberta)')}. Fonte: `plano.json`.",
        "- Definição de pronto: " + "; ".join(DEFINICAO_DE_PRONTO) + ".", "",
        "## Kanban", "",
        "- Colunas no fechamento: " + ", ".join(f"{c} {q}" + (f"/{w}" if w else "") + (" ESTOURADO" if "ESTOURADO" in x else "") for c, q, w, x in est) + ". Fonte: `kanban.md`.",
        "- Limite de WIP: " + ", ".join(f"{k} {v}" for k, v in plano.get("kanban", {}).get("wip", WIP_PADRAO).items()) + ".", "",
        "## PDCA", "",
        f"- Ciclos fechados até aqui: {len(bl)} ({sum('| frutifero |' in l for l in bl)} frutíferos, {sum('| infrutifero |' in l for l in bl)} infrutíferos). Fonte: `base_de_conhecimento.md`.",
        "- Regra: ciclo infrutífero só fecha com a próxima hipótese; a base recebe a linha nos dois casos.", "",
        "## Balanceamento de modelos", "",
        f"- Decisões de roteamento registradas: {len(decs)}" + (" (" + ", ".join(f"{k} {v}" for k, v in sorted(Counter(d['modelo'] for d in decs).items())) + ")" if decs else "") + ". Fonte: `roteamento.jsonl`.",
        f"- Orçamento do gate {s['gate']}: previsto {orc.get('tokens_previstos', 'INDISPONÍVEL')}, gasto {orc.get('tokens_gastos', 'INDISPONÍVEL')} tokens, {orc.get('chamadas', 0)} chamadas. Fonte: `orcamento.json`.", "",
        "## Papéis e subagentes", "",
        "- Dez papéis (A orquestrador … J pesquisador), cada um com quatro subagentes Plan, Do, Check e Act; itens do backlog levam o papel dono. Fonte: `backlog.md`.", "",
    ]

    # ferramentas: docstrings dos .py do plugin (e do projeto, se houver)
    ferr = [f"# Ferramentas usadas na sprint {n:02d}", "", "Descrição lida do cabeçalho (docstring) de cada script; nada aqui foi escrito à mão.", ""]
    fontes = []
    if plugin_root:
        fontes += sorted((plugin_root / "skills" / "conversao-wx" / "scripts").glob("*.py")) + sorted((plugin_root / "hooks").glob("*.py"))
    fontes += sorted((wx.parent).glob("**/*.py"))[:50] if False else []
    for f in fontes:
        try:
            doc = ast.get_docstring(ast.parse(f.read_text(encoding="utf-8"))) or "(sem docstring)"
        except SyntaxError:
            doc = "(não foi possível ler)"
        ferr += [f"## `{f.name}`", "", doc.strip(), ""]

    with zipfile.ZipFile(destino, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr(f"sprint-{n:02d}/LEIA-ME.md", f"# Entrega da sprint {n:02d} ({s['gate']})\n\nGerado em {hoje} por `pmo.py entregar`.\n\n- `resumo-da-sprint.md`: resumo de doze seções (ou aviso se a sprint ainda estiver aberta)\n- `tecnicas-aplicadas.md`: Scrum, Kanban, PDCA e balanceamento, com números medidos\n- `base-de-conhecimento.md`: todos os ciclos PDCA fechados\n- `ferramentas.md`: o que cada script faz, lido do próprio script\n- `kanban.md`, `status.md`: estado no fechamento\n- `relatorio.md`, `painel.html`: o relatório completo (contrato, rastreabilidade por tipo, lacunas, decisões, riscos, sprints, PDCA, roteamento, entrega, próximos passos) em texto e em HTML\n- `decisoes/`, `gaps.md`, `riscos.md`: decisões, lacunas e RAID\n- `desenvolvimento/`: os .md produzidos em specifications/ e architecture/\n")
        resumo = sorted(pasta.parent.joinpath("sprints").glob(f"sprint-{n:02d}-*.md"))
        z.writestr(f"sprint-{n:02d}/resumo-da-sprint.md", resumo[-1].read_text(encoding="utf-8") if resumo else f"Sprint {n:02d} ainda aberta: o resumo de doze seções é escrito por `pmo.py sprint fechar`.\n")
        z.writestr(f"sprint-{n:02d}/tecnicas-aplicadas.md", "\n".join(tecnicas) + "\n")
        z.writestr(f"sprint-{n:02d}/base-de-conhecimento.md", base.read_text(encoding="utf-8") if base.is_file() else "INDISPONÍVEL\n")
        z.writestr(f"sprint-{n:02d}/ferramentas.md", "\n".join(ferr) + "\n")
        z.writestr(f"sprint-{n:02d}/kanban.md", kb + "\n")
        z.writestr(f"sprint-{n:02d}/status.md", status(wx) + "\n")
        z.writestr(f"sprint-{n:02d}/relatorio.md", relatorio(wx) + "\n")
        z.write(painel(wx), f"sprint-{n:02d}/painel.html")
        for nome_arq in ("gaps.md", "traceability.csv"):
            if (wx / nome_arq).is_file():
                z.write(wx / nome_arq, f"sprint-{n:02d}/{nome_arq}")
        for nome_arq in ("riscos.md", "backlog.md", "plano.json", "orcamento.json", "projeto.json", "cronograma.md", "organograma.md", "fluxograma.md"):
            if (wx / "pmo" / nome_arq).is_file():
                z.write(wx / "pmo" / nome_arq, f"sprint-{n:02d}/{nome_arq}")
        for sub in ("decisions", "specifications", "architecture"):
            for f in sorted((wx / sub).rglob("*.md")) if (wx / sub).is_dir() else []:
                z.write(f, f"sprint-{n:02d}/{'decisoes' if sub == 'decisions' else 'desenvolvimento/' + sub}/{f.relative_to(wx / sub)}")
        for f in sorted((wx / "pmo" / "pdca").glob("PDCA-*.md")) if (wx / "pmo" / "pdca").is_dir() else []:
            z.write(f, f"sprint-{n:02d}/pdca/{f.name}")
    return destino


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project-root", type=Path, default=Path("."))
    sub = parser.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("iniciar"); s.add_argument("--aprovador", default="")
    sub.add_parser("status")
    g = sub.add_parser("gastar")
    g.add_argument("--gate", required=True, choices=GATES)
    g.add_argument("--modelo", required=True, choices=["haiku", "sonnet", "opus"])
    g.add_argument("--tokens", required=True, type=int)
    g.add_argument("--chamadas", type=int, default=1)
    pd = sub.add_parser("pdca"); pds = pd.add_subparsers(dest="acao", required=True)
    a = pds.add_parser("abrir"); a.add_argument("--gate", required=True, choices=GATES); a.add_argument("--hipotese", required=True); a.add_argument("--medida", required=True, help="o que será medido"); a.add_argument("--criterio", required=True, help="critério de sucesso, com número")
    fch = pds.add_parser("fechar"); fch.add_argument("--id", required=True); fch.add_argument("--resultado", required=True, choices=["frutifero", "infrutifero"]); fch.add_argument("--medido", required=True); fch.add_argument("--aprendizado", required=True); fch.add_argument("--proxima", default="")
    sub.add_parser("kanban")
    sub.add_parser("painel")
    sub.add_parser("relatorio")
    en = sub.add_parser("entregar"); en.add_argument("--sprint", type=int, help="número; padrão: a última"); en.add_argument("--plugin-root", type=Path, help="para listar as ferramentas do plugin")
    sp = sub.add_parser("sprint"); sps = sp.add_subparsers(dest="acao", required=True)
    sa = sps.add_parser("abrir"); sa.add_argument("--nome", required=True); sa.add_argument("--objetivo", required=True); sa.add_argument("--gate", required=True, choices=GATES); sa.add_argument("--item", action="append", default=[], help="trace_id, ou trace_id:PAPEL (A–J) para registrar o dono no backlog; repetível"); sa.add_argument("--aprovador", default="")
    sf = sps.add_parser("fechar"); sf.add_argument("--decisao", required=True, choices=["APPROVED", "CONDITIONAL", "REJECTED"]); sf.add_argument("--pedido", default="")
    args = parser.parse_args()
    wx = args.project_root.resolve(strict=True) / ".wx-migration"
    if args.cmd == "iniciar":
        print("\n".join(iniciar(wx, args.aprovador)))
    elif args.cmd == "gastar":
        print(gastar(wx, args.gate, args.modelo, args.tokens, args.chamadas))
    elif args.cmd == "pdca":
        if args.acao == "abrir":
            print(pdca_abrir(wx, args.gate, args.hipotese, args.medida, args.criterio))
        else:
            print(pdca_fechar(wx, args.id, args.resultado, args.medido, args.aprendizado, args.proxima))
    elif args.cmd == "kanban":
        print(kanban(wx))
    elif args.cmd == "painel":
        print(f"CREATED {painel(wx)}")
    elif args.cmd == "relatorio":
        print(relatorio(wx))
    elif args.cmd == "entregar":
        z = entregar(wx, args.sprint, args.plugin_root)
        import zipfile
        with zipfile.ZipFile(z) as zz:
            print("\n".join(f"  {i.filename} ({i.file_size} bytes)" for i in zz.infolist()))
        print(f"CREATED {z}")
    elif args.cmd == "sprint":
        if args.acao == "abrir":
            print(sprint_abrir(wx, args.nome, args.objetivo, args.gate, args.item, args.aprovador))
        else:
            print(sprint_fechar(wx, args.decisao, args.pedido))
    else:
        texto = status(wx)
        (wx / "pmo").mkdir(parents=True, exist_ok=True)
        (wx / "pmo" / "status.md").write_text(texto + "\n", encoding="utf-8")
        print(texto)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr)
        sys.exit(2)
