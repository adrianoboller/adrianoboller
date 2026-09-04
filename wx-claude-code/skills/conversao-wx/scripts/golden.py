#!/usr/bin/env python3
"""Golden master: captura resultados do legado e compara com os do sistema novo.

Torna «equivalencia» um numero. Cada caso tem id, entrada e resultado esperado
(capturado do legado); o resultado novo vem de um comando que recebe a entrada
em JSON pela entrada padrao e devolve JSON pela saida padrao, ou de um arquivo
JSON ja produzido. Tolerancias vem de conversion.config.json
(acceptance.data_reconciliation_tolerances) ou de --tolerancia.

  golden.py capturar --casos resultados-esperados.json --saida golden-master/casos.json
  golden.py comparar --golden golden-master/casos.json --comando "python3 novo.py" --relatorio results/comp.json
  golden.py comparar --golden golden-master/casos.json --resultados novo.json --relatorio results/comp.json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import date
from pathlib import Path


def normalizar(v, tol: float):
    if isinstance(v, bool) or v is None or isinstance(v, str):
        return v
    if isinstance(v, (int, float)):
        return round(float(v), 6)
    if isinstance(v, list):
        return [normalizar(x, tol) for x in v]
    if isinstance(v, dict):
        return {k: normalizar(v[k], tol) for k in sorted(v)}
    return v


def igual(a, b, tol: float) -> bool:
    if isinstance(a, bool) != isinstance(b, bool):
        return False  # True esperado e 1 obtido nao sao iguais
    if isinstance(a, (int, float)) and not isinstance(a, bool) and isinstance(b, (int, float)) and not isinstance(b, bool):
        return abs(float(a) - float(b)) <= tol
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(igual(x, y, tol) for x, y in zip(a, b))
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(igual(a[k], b[k], tol) for k in a)
    return a == b


def capturar(casos: Path, saida: Path) -> int:
    d = json.loads(casos.read_text(encoding="utf-8"))
    lista = d["casos"] if isinstance(d, dict) else d
    golden = {"capturado_em": date.today().isoformat(), "origem": str(casos), "casos": []}
    for c in lista:
        golden["casos"].append({"id": c["id"], "regra": c.get("regra", ""), "entrada": c["entrada"], "esperado": c["esperado"], "observacao": c.get("observacao", "")})
    saida.parent.mkdir(parents=True, exist_ok=True)
    saida.write_text(json.dumps(golden, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"{len(golden['casos'])} casos capturados em {saida}")
    return 0


def comparar(golden_p: Path, comando: str | None, resultados_p: Path | None, relatorio: Path, tol: float, config: Path | None) -> int:
    golden = json.loads(golden_p.read_text(encoding="utf-8"))
    if config and config.is_file():
        tols = json.loads(config.read_text(encoding="utf-8")).get("acceptance", {}).get("data_reconciliation_tolerances", {})
        tol = float(tols.get("monetario", tols.get("default", tol)))
    novos: dict[str, object] = {}
    if resultados_p:
        r = json.loads(resultados_p.read_text(encoding="utf-8"))
        novos = {x["id"]: x["resultado"] for x in (r["resultados"] if isinstance(r, dict) else r)}
    linhas = []
    ok = 0
    for c in golden["casos"]:
        if comando:
            try:
                proc = subprocess.run(comando, shell=True, input=json.dumps({"id": c["id"], "regra": c["regra"], "entrada": c["entrada"]}), capture_output=True, text=True, timeout=60)
                try:
                    novo = json.loads(proc.stdout)
                    novo = novo.get("resultado", novo) if isinstance(novo, dict) and "resultado" in novo else novo
                except json.JSONDecodeError:
                    novo = {"erro": (proc.stderr or proc.stdout).strip()[:200]}
            except subprocess.TimeoutExpired:
                novo = {"erro": "timeout (60 s) neste caso; os demais seguem"}
        else:
            novo = novos.get(c["id"], {"erro": "sem resultado"})
        esperado = normalizar(c["esperado"], tol)
        obtido = normalizar(novo, tol)
        passou = igual(esperado, obtido, tol)
        ok += passou
        linhas.append({"id": c["id"], "regra": c["regra"], "esperado": esperado, "obtido": obtido, "passou": passou})
    total = len(linhas)
    rel = {"comparado_em": date.today().isoformat(), "golden": str(golden_p), "tolerancia": tol, "total": total, "passaram": ok, "falharam": total - ok, "equivalencia": f"{ok}/{total}", "casos": linhas}
    relatorio.parent.mkdir(parents=True, exist_ok=True)
    relatorio.write_text(json.dumps(rel, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for l in linhas:
        print(("PASS " if l["passou"] else "FAIL ") + l["id"] + ("" if l["passou"] else f"  esperado={l['esperado']} obtido={l['obtido']}"))
    print(f"equivalência: {ok}/{total} (tolerância {tol}); relatório em {relatorio}")
    return 0 if ok == total else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("capturar"); c.add_argument("--casos", required=True, type=Path); c.add_argument("--saida", required=True, type=Path)
    k = sub.add_parser("comparar"); k.add_argument("--golden", required=True, type=Path); k.add_argument("--comando"); k.add_argument("--resultados", type=Path); k.add_argument("--relatorio", required=True, type=Path); k.add_argument("--tolerancia", type=float, default=0.005); k.add_argument("--config", type=Path)
    a = ap.parse_args()
    if a.cmd == "capturar":
        return capturar(a.casos, a.saida)
    if not a.comando and not a.resultados:
        ap.error("comparar exige --comando ou --resultados")
    return comparar(a.golden, a.comando, a.resultados, a.relatorio, a.tolerancia, a.config)


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
