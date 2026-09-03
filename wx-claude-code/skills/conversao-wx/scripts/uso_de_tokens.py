#!/usr/bin/env python3
"""Le o uso de tokens medido pelo proprio Claude Code e alimenta o orcamento do PMO.

O Claude Code grava cada resposta do modelo, com o campo usage, em
~/.claude/projects/<projeto>/<sessao>.jsonl. Este script soma esses campos por
sessao e por modelo (MEDIDO, nao estimado) e, se pedido, lanca o total num gate
do orcamento (pmo/orcamento.json) via a mesma logica de pmo.py gastar.

  uso_de_tokens.py --project-root . resumo
  uso_de_tokens.py --project-root . lancar --gate G1 [--sessao <id>]

Mensagens repetidas no log (mesmo id) contam uma vez. Modelos sao mapeados
para as classes haiku/sonnet/opus pelo nome; o que nao casar fica em 'outro'.
Nada aqui reproduz conteudo de prompt ou resposta: so numeros.
"""

from __future__ import annotations

import argparse
import json
import re
import os
import sys
from collections import defaultdict
from pathlib import Path


def pasta_do_projeto(project_root: Path) -> Path:
    # O Claude Code nomeia a pasta trocando os separadores do caminho absoluto por '-'.
    nome = re.sub(r"[^A-Za-z0-9]", "-", str(project_root.resolve()))
    return Path.home() / ".claude" / "projects" / nome


def classe(modelo: str) -> str:
    m = modelo.lower()
    for c in ("haiku", "sonnet", "opus", "fable"):
        if c in m:
            return c
    return "outro"


def ler(project_root: Path) -> dict:
    pasta = pasta_do_projeto(project_root)
    if not pasta.is_dir():
        return {"pasta": str(pasta), "estado": "INDISPONÍVEL", "motivo": "pasta de sessões não existe", "sessoes": {}}
    sessoes: dict[str, dict] = {}
    for arq in sorted(pasta.glob("*.jsonl"), key=lambda p: p.stat().st_mtime):
        vistos: set[str] = set()
        tot = defaultdict(lambda: {"input": 0, "output": 0, "cache_creation": 0, "cache_read": 0, "chamadas": 0})
        turnos = 0
        with arq.open(encoding="utf-8") as f:
            for linha in f:
                try:
                    o = json.loads(linha)
                except json.JSONDecodeError:
                    continue
                if o.get("type") == "user":
                    turnos += 1
                if o.get("type") != "assistant":
                    continue
                msg = o.get("message", {})
                uso = msg.get("usage")
                if not uso:
                    continue
                chave = msg.get("id") or o.get("uuid")
                if chave in vistos:
                    continue
                vistos.add(chave)
                t = tot[msg.get("model", "?")]
                t["input"] += int(uso.get("input_tokens", 0))
                t["output"] += int(uso.get("output_tokens", 0))
                t["cache_creation"] += int(uso.get("cache_creation_input_tokens", 0))
                t["cache_read"] += int(uso.get("cache_read_input_tokens", 0))
                t["chamadas"] += 1
        sessoes[arq.stem] = {"arquivo": arq.name, "modificado": arq.stat().st_mtime, "turnos_usuario": turnos, "por_modelo": dict(tot)}
    return {"pasta": str(pasta), "estado": "MEDIDO", "sessoes": sessoes}


def total_de(s: dict) -> dict:
    por_classe = defaultdict(int)
    chamadas = 0
    for modelo, t in s["por_modelo"].items():
        por_classe[classe(modelo)] += t["input"] + t["output"] + t["cache_creation"] + t["cache_read"]
        chamadas += t["chamadas"]
    return {"por_classe": dict(por_classe), "tokens": sum(por_classe.values()), "chamadas": chamadas}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--project-root", type=Path, default=Path("."))
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("resumo")
    l = sub.add_parser("lancar"); l.add_argument("--gate", required=True); l.add_argument("--sessao", help="id da sessão; padrão: a mais recente")
    a = ap.parse_args()
    d = ler(a.project_root)
    if d["estado"] != "MEDIDO":
        print(json.dumps(d, ensure_ascii=False)); return 3
    if a.cmd == "resumo":
        print(f"fonte: {d['pasta']} (MEDIDO, campo usage das respostas; cache incluído)")
        print("| sessão | turnos | chamadas | haiku | sonnet | opus | fable | outro | total |")
        print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        for sid, s in d["sessoes"].items():
            t = total_de(s); pc = t["por_classe"]
            print(f"| {sid[:8]}… | {s['turnos_usuario']} | {t['chamadas']} | {pc.get('haiku',0)} | {pc.get('sonnet',0)} | {pc.get('opus',0)} | {pc.get('fable',0)} | {pc.get('outro',0)} | {t['tokens']} |")
        return 0
    sid = a.sessao or (list(d["sessoes"])[-1] if d["sessoes"] else None)
    if not sid or sid not in d["sessoes"]:
        print("erro: sessão não encontrada"); return 2
    t = total_de(d["sessoes"][sid])
    orc = a.project_root / ".wx-migration" / "pmo" / "orcamento.json"
    if not orc.is_file():
        print(f"erro: {orc} não existe; rode pmo.py iniciar"); return 2
    o = json.loads(orc.read_text(encoding="utf-8"))
    g = o.setdefault("gates", {}).get(a.gate)
    if g is None:
        print(f"erro: gate {a.gate!r} não existe no orçamento (G0 a G7)"); return 2
    g["tokens_gastos"] = g.get("tokens_gastos", 0) + t["tokens"]; g["chamadas"] = g.get("chamadas", 0) + t["chamadas"]
    g.setdefault("por_modelo", {})
    for c, n in t["por_classe"].items():
        g["por_modelo"][c] = g["por_modelo"].get(c, 0) + n
    g.setdefault("lancamentos", []).append({"sessao": sid, "tokens": t["tokens"], "chamadas": t["chamadas"], "fonte": "usage do Claude Code (MEDIDO)"})
    orc.write_text(json.dumps(o, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    prev = g.get("tokens_previstos", 0)
    print(f"{a.gate}: +{t['tokens']} tokens da sessão {sid[:8]}… → {g['tokens_gastos']} gastos" + (f" ({100*g['tokens_gastos']/prev:.0f}% do previsto)" if prev else " (sem previsão)"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
