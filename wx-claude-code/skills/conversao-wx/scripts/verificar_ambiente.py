#!/usr/bin/env python3
"""Mede o ambiente pedido na letra K do questionario: o que esta instalado, em que
versao, e se atende ao minimo. Nada e suposto: cada linha e a saida real de
`--version`, ou INDISPONIVEL quando o programa nao esta no PATH.

Uso: verificar_ambiente.py --questionario .wx-migration/questionario.json [--json]
Exit 0 quando tudo que foi marcado atende; 3 quando falta algo.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

FERRAMENTAS = {
    # chave K -> (programa, argumentos, regex da versao)
    "K1_rust": [("rustc", ["--version"], r"(\d+\.\d+(?:\.\d+)?)"), ("cargo", ["--version"], r"(\d+\.\d+(?:\.\d+)?)")],
    "K2_postgresql": [("psql", ["--version"], r"(\d+(?:\.\d+)?)")],
    "K3_mysql": [("mysql", ["--version"], r"(\d+\.\d+(?:\.\d+)?)")],
    "K4_mariadb": [("mariadb", ["--version"], r"(\d+\.\d+(?:\.\d+)?)")],
    "K5_supabase": [("supabase", ["--version"], r"(\d+\.\d+(?:\.\d+)?)")],
    "K6_github": [("git", ["--version"], r"(\d+\.\d+(?:\.\d+)?)"), ("gh", ["--version"], r"(\d+\.\d+(?:\.\d+)?)")],
}


def versao_de(prog: str, args: list[str], rx: str) -> str | None:
    if not shutil.which(prog):
        return None
    try:
        out = subprocess.run([prog, *args], capture_output=True, text=True, timeout=20)
    except (OSError, subprocess.TimeoutExpired):
        return None
    m = re.search(rx, out.stdout + out.stderr)
    return m.group(1) if m else "?"


def atende(instalada: str | None, minima: str) -> bool:
    if not instalada or instalada == "?" or not minima:
        return bool(instalada) and instalada != "?"
    def n(v): return [int(x) for x in re.findall(r"\d+", v)]
    return n(instalada) >= n(minima)


def medir(q: dict) -> list[dict]:
    k = q.get("K_ambiente", {}) or {}
    linhas = []
    for chave, progs in FERRAMENTAS.items():
        bloco = k.get(chave, {}) or {}
        pedido = bool(bloco.get("instalar_ou_atualizar") or bloco.get("ligar_projeto"))
        minima = bloco.get("versao_minima") or bloco.get("versao") or ""
        if chave in {"K5_supabase", "K6_github"}:
            minima = ""
        for prog, args, rx in progs:
            inst = versao_de(prog, args, rx)
            linhas.append({"item": chave, "programa": prog, "pedido": pedido, "minima": minima, "instalada": inst or "INDISPONÍVEL",
                           "estado": ("ok" if atende(inst, minima) else "falta") if pedido else ("presente" if inst else "ausente")})
    return linhas


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--questionario", required=True, type=Path)
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    q = json.loads(a.questionario.read_text(encoding="utf-8"))
    linhas = medir(q)
    if a.json:
        print(json.dumps(linhas, ensure_ascii=False, indent=2))
    else:
        print("| item | programa | pedido | mínima | instalada | estado |\n| --- | --- | --- | --- | --- | --- |")
        for l in linhas:
            print(f"| {l['item']} | {l['programa']} | {'sim' if l['pedido'] else 'não'} | {l['minima'] or '—'} | {l['instalada']} | {l['estado']} |")
        print("\nMEDIDO com `--version` de cada programa nesta máquina.")
    return 3 if any(l["estado"] == "falta" for l in linhas) else 0


if __name__ == "__main__":
    sys.exit(main())
