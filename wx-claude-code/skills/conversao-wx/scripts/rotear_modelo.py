#!/usr/bin/env python3
"""Escolhe modelo e effort para uma tarefa da conversao WX.

A regra esta em references/balanceamento-de-modelos.md; aqui ela vira codigo
para que orquestrador e PMO nao decidam cada um de um jeito. Le o orcamento do
gate em .wx-migration/pmo/orcamento.json (se existir) e registra cada decisao
em .wx-migration/pmo/roteamento.jsonl.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

DEGRAUS = ["haiku", "sonnet", "opus"]
CLASSES = {
    "mecanica": ("haiku", "medium"),
    "analise": ("sonnet", "high"),
    "decisao": ("opus", "high"),
    "revisao": ("opus", "max"),
}
SINAIS_SOBE = {"conflito", "fiscal", "dinheiro", "permissao", "dado-pessoal", "decisao-humana", "falhou-antes"}
SINAIS_DESCE = {"padrao-aprovado", "volume-grande", "criterio-objetivo"}


def rotear(classe: str, sinais: set[str], gasto_pct: float | None, indisponiveis: set[str]) -> dict:
    if classe not in CLASSES:
        raise ValueError(f"classe inválida: {classe!r} (aceitas: {', '.join(CLASSES)})")
    modelo, effort = CLASSES[classe]
    motivos: list[str] = [f"classe {classe}"]
    grau = DEGRAUS.index(modelo)
    if classe != "revisao":
        if sinais & SINAIS_SOBE:
            teto = 1 if classe == "mecanica" else 2
            if grau < teto:
                grau += 1
                motivos.append("subiu: " + ", ".join(sorted(sinais & SINAIS_SOBE)))
        elif sinais & SINAIS_DESCE and grau > 0:
            grau -= 1
            motivos.append("desceu: " + ", ".join(sorted(sinais & SINAIS_DESCE)))
    estado = "OK"
    if gasto_pct is not None:
        if gasto_pct >= 100:
            estado = "BLOQUEADO"
            motivos.append(f"orçamento do gate em {gasto_pct:.0f}%")
        elif gasto_pct >= 80 and classe != "revisao" and grau > 0:
            grau -= 1
            motivos.append(f"orçamento em {gasto_pct:.0f}%: rebaixado")
    while grau > 0 and DEGRAUS[grau] in indisponiveis:
        motivos.append(f"fallback: {DEGRAUS[grau]} indisponível")
        grau -= 1
    return {"modelo": DEGRAUS[grau], "effort": effort, "estado": estado, "motivos": motivos}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--classe", required=True, choices=sorted(CLASSES))
    parser.add_argument("--sinal", action="append", default=[], help="conflito, fiscal, dinheiro, permissao, dado-pessoal, decisao-humana, falhou-antes, padrao-aprovado, volume-grande, criterio-objetivo")
    parser.add_argument("--gate", default="", help="G0..G7, para ler o orçamento")
    parser.add_argument("--tarefa", default="", help="identificação curta, só para o registro")
    parser.add_argument("--indisponivel", action="append", default=[], help="modelo sem acesso na organização")
    parser.add_argument("--project-root", type=Path, default=Path("."))
    args = parser.parse_args()

    pmo = args.project_root / ".wx-migration" / "pmo"
    gasto_pct = None
    orcamento = pmo / "orcamento.json"
    if args.gate and orcamento.is_file():
        dados = json.loads(orcamento.read_text(encoding="utf-8"))
        g = dados.get("gates", {}).get(args.gate)
        if g and g.get("tokens_previstos"):
            gasto_pct = 100.0 * float(g.get("tokens_gastos", 0)) / float(g["tokens_previstos"])

    decisao = rotear(args.classe, set(args.sinal), gasto_pct, set(args.indisponivel))
    decisao.update({"gate": args.gate, "tarefa": args.tarefa, "quando": datetime.now(timezone.utc).isoformat(timespec="seconds")})
    if pmo.is_dir():
        with (pmo / "roteamento.jsonl").open("a", encoding="utf-8") as f:
            f.write(json.dumps(decisao, ensure_ascii=False) + "\n")
    print(json.dumps(decisao, ensure_ascii=False))
    return 0 if decisao["estado"] == "OK" else 3


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr)
        sys.exit(2)
