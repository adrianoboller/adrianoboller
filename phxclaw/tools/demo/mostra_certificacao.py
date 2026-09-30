#!/usr/bin/env python3
"""Mostra o reports/RELEASE_CERTIFICATION_v0.70.json em tabela de terminal (le o gerado, nao digita)."""
import json, pathlib, textwrap
d = json.loads((pathlib.Path(__file__).resolve().parents[2] / "reports/RELEASE_CERTIFICATION_v0.70.json").read_text())
cor = {"passed": "\033[1;32mPASSOU   \033[0m", "failed": "\033[1;31mFALHOU   \033[0m", "blocked": "\033[1;33mBLOQUEADO\033[0m"}
print(f"veredito: \033[1m{d['verdict']}\033[0m   obrigatorios {d['required_passed']}/{d['required_total']}   ({d['certified_at']})\n")
for g in d["gates"]:
    info = g.get("reason") or (json.dumps(g["measured"], ensure_ascii=False) if g.get("measured") else "")
    linha = textwrap.shorten(str(info).strip('"'), 68, placeholder="...")
    print(f" {cor[g['status']]} {g['gate']:<42.42} {linha}")
