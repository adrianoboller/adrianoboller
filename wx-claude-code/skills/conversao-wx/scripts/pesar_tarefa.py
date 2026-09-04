#!/usr/bin/env python3
"""Gestor de tarefas: pesa uma tarefa e decide o modelo.

O peso sai de tres medidas, todas registradas com a fonte: linhas estimadas
(por referencia a tarefas parecidas ja feitas no projeto, lidas de
pmo/pesos.jsonl, ou por faixa declarada), tempo estimado (a media das
parecidas, ou a faixa) e sinais de complexidade (banco, fiscal, concorrencia,
UI densa, integracao, sem regra localizada). O grau decide o modelo do
Claude Code: simples -> haiku; medio -> sonnet; complexo -> opus; em ultimo
caso (revisao de gate, conflito, fiscal) -> opus com effort max.

Pesquisa na internet sobre atividades similares e trabalho do agente
(Gestor de tarefas, com WebSearch); o que ele achar entra por --referencia,
com a fonte, e fica no registro. O script nao inventa media sem referencia:
sem nenhuma, marca ESTIMADO.

Uso:
  pesar_tarefa.py --project-root <p> pesar --id BR-012 --titulo "..." [--linhas N] [--horas H]
                  [--sinal banco --sinal fiscal ...] [--referencia "fonte: linhas=120 horas=3"]...
  pesar_tarefa.py --project-root <p> registrar --id BR-012 --linhas-reais N --horas-reais H
  pesar_tarefa.py --project-root <p> listar
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
from datetime import date
from pathlib import Path

SINAIS = {"banco": 2, "fiscal": 3, "concorrencia": 3, "ui-densa": 2, "integracao": 2, "sem-regra": 3, "relatorio": 1, "seguranca": 3, "migracao-de-dados": 2}
GRAUS = [(0, "simples", "haiku", "medium"), (4, "medio", "sonnet", "high"), (8, "complexo", "opus", "high"), (12, "critico", "opus", "max")]


def carregar(wx: Path) -> list[dict]:
    p = wx / "pmo" / "pesos.jsonl"
    return [json.loads(l) for l in p.read_text(encoding="utf-8").splitlines() if l.strip()] if p.is_file() else []


def pesar(wx: Path, ident: str, titulo: str, linhas: int | None, horas: float | None, sinais: list[str], referencias: list[str]) -> dict:
    for s in sinais:
        if s not in SINAIS:
            raise ValueError(f"sinal {s!r} desconhecido (aceitos: {', '.join(SINAIS)})")
    hist = [h for h in carregar(wx) if h.get("linhas_reais")]
    refs = []
    for r in referencias:
        m = re.search(r"linhas=(\d+)", r); h = re.search(r"horas=([\d.,]+)", r)
        refs.append({"fonte": r.split(":")[0].strip(), "linhas": int(m.group(1)) if m else None, "horas": float(h.group(1).replace(",", ".")) if h else None})
    fontes = []
    if linhas is None:
        cand = [r["linhas"] for r in refs if r["linhas"]] + [h["linhas_reais"] for h in hist]
        if cand:
            linhas = int(statistics.median(cand)); fontes.append(f"linhas: mediana de {len(cand)} referência(s)")
    if horas is None:
        cand = [r["horas"] for r in refs if r["horas"]] + [h["horas_reais"] for h in hist if h.get("horas_reais")]
        if cand:
            horas = round(statistics.median(cand), 1); fontes.append(f"horas: mediana de {len(cand)} referência(s)")
    pontos = sum(SINAIS[s] for s in sinais)
    if linhas is not None:
        pontos += 0 if linhas < 80 else 2 if linhas < 300 else 4 if linhas < 1000 else 6
    if horas is not None:
        pontos += 0 if horas < 2 else 1 if horas < 8 else 3
    grau = next(g for g in reversed(GRAUS) if pontos >= g[0])
    reg = {"id": ident, "titulo": titulo, "quando": date.today().isoformat(), "linhas_estimadas": linhas, "horas_estimadas": horas, "sinais": sinais, "pontos": pontos,
           "grau": grau[1], "modelo": grau[2], "effort": grau[3], "referencias": refs, "fontes": fontes, "estado": "MEDIDO" if (refs or hist) else "ESTIMADO"}
    (wx / "pmo").mkdir(parents=True, exist_ok=True)
    with (wx / "pmo" / "pesos.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps(reg, ensure_ascii=False) + "\n")
    return reg


def registrar(wx: Path, ident: str, linhas_reais: int, horas_reais: float) -> str:
    reg = {"id": ident, "quando": date.today().isoformat(), "linhas_reais": linhas_reais, "horas_reais": horas_reais}
    with (wx / "pmo" / "pesos.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps(reg, ensure_ascii=False) + "\n")
    return f"{ident}: real {linhas_reais} linhas, {horas_reais} h; vira referência das próximas"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project-root", type=Path, default=Path.cwd())
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("pesar"); p.add_argument("--id", required=True); p.add_argument("--titulo", required=True); p.add_argument("--linhas", type=int); p.add_argument("--horas", type=float)
    p.add_argument("--sinal", action="append", default=[]); p.add_argument("--referencia", action="append", default=[], help='"fonte: linhas=120 horas=3"')
    r = sub.add_parser("registrar"); r.add_argument("--id", required=True); r.add_argument("--linhas-reais", type=int, required=True); r.add_argument("--horas-reais", type=float, required=True)
    sub.add_parser("listar")
    a = ap.parse_args()
    wx = a.project_root.resolve() / ".wx-migration"
    try:
        if a.cmd == "pesar":
            reg = pesar(wx, a.id, a.titulo, a.linhas, a.horas, a.sinal, a.referencia)
            print(f"{reg['id']}: grau {reg['grau']} ({reg['pontos']} pontos) → {reg['modelo']} effort {reg['effort']}; linhas {reg['linhas_estimadas'] if reg['linhas_estimadas'] is not None else 'INDISPONÍVEL'}, horas {reg['horas_estimadas'] if reg['horas_estimadas'] is not None else 'INDISPONÍVEL'} [{reg['estado']}]" + (": " + "; ".join(reg["fontes"]) if reg["fontes"] else ""))
        elif a.cmd == "registrar":
            print(registrar(wx, a.id, a.linhas_reais, a.horas_reais))
        else:
            for h in carregar(wx):
                print(json.dumps(h, ensure_ascii=False))
    except (OSError, ValueError) as exc:
        print(f"erro: {exc}", file=sys.stderr); return 2
    return 0


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
