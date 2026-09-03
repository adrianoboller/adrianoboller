#!/usr/bin/env python3
"""Painel do PMO da conversao WX: le os artefatos e gera .wx-migration/pmo/status.md.

Todo numero do painel sai de um arquivo (traceability.csv, gaps.md, decisions/,
orcamento.json, roteamento.jsonl, gate-status.md). Nada e digitado: numero
digitado a mao envelhece calado.

Subcomandos:
  iniciar   cria plano.json, orcamento.json e riscos.md (sem sobrescrever)
  status    regenera status.md e o imprime
  gastar    registra uso medido de tokens num gate
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
    }
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
    ]


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
    args = parser.parse_args()
    wx = args.project_root.resolve(strict=True) / ".wx-migration"
    if args.cmd == "iniciar":
        print("\n".join(iniciar(wx, args.aprovador)))
    elif args.cmd == "gastar":
        print(gastar(wx, args.gate, args.modelo, args.tokens, args.chamadas))
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
