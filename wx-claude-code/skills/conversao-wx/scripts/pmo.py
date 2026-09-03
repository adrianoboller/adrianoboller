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
    plano = {
        "criado_em": date.today().isoformat(),
        "aprovador_padrao": aprovador,
        "gates": {g: {"status": "não iniciado", "aprovador": aprovador, "previsto_para": "", "decidido_em": ""} for g in GATES},
        "sprints": [],
        "kanban": {"wip": dict(WIP_PADRAO)},
        "scrum": {"duracao_dias": 10, "cerimonias": ["planejamento", "diario", "revisao", "retrospectiva"]},
    }
    backlog = (
        "# Backlog do produto\n\n"
        "Uma linha por item, priorizada de cima para baixo. O trace_id liga ao traceability.csv.\n\n"
        "| prioridade | trace_id | item | gate | estimativa | sprint |\n| ---: | --- | --- | --- | --- | --- |\n"
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
    linhas = ["# Kanban", "", f"Gerado por `pmo.py kanban` em {date.today().isoformat()} de `traceability.csv` ({len(rows)} itens). Não edite: mude o estado na matriz.", ""]
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
            linhas.append(f"- `{r.get('trace_id','?')}` {resumo}" + (f" — {dono}" if dono else "") + (f" — {r.get('notes','').strip()[:60]}" if coluna == "Bloqueado" and r.get("notes") else ""))
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
    return f"sprint {s['numero']} fechada ({decisao}); prontos {len(prontos)}/{len(s['itens'])}; {saida}"


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
    sp = sub.add_parser("sprint"); sps = sp.add_subparsers(dest="acao", required=True)
    sa = sps.add_parser("abrir"); sa.add_argument("--nome", required=True); sa.add_argument("--objetivo", required=True); sa.add_argument("--gate", required=True, choices=GATES); sa.add_argument("--item", action="append", default=[], help="trace_id do backlog; repetível"); sa.add_argument("--aprovador", default="")
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
