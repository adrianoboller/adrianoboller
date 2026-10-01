#!/usr/bin/env python3
"""Quanto o PhxClaw absorveu de cada fonte, medido capacidade por capacidade.

    python3 docs/absorcao/gerar_absorcao.py

Le `fontes.json` (a lista FECHADA de capacidades de cada fonte, tirada da documentacao
oficial, com a data da leitura) e `phxclaw.json` (o estado de cada capacidade no
PhxClaw, com a evidencia no codigo). Duas porcentagens por fonte, e as duas aparecem:
- no agente: so o que o agente ou a CLI entregam (peso 1);
- com bibliotecas: soma meio ponto para o que existe pela metade ou como biblioteca
  testada fora do agente. Biblioteca que o agente nao usa nao e recurso para quem o usa,
  por isso ela nunca entra na primeira conta.
Grava `absorcao.json`, que e de onde o dossie e a pagina leem.
"""
import json
from pathlib import Path

AQUI = Path(__file__).resolve().parent
fontes = json.loads((AQUI / "fontes.json").read_text())
phx = json.loads((AQUI / "phxclaw.json").read_text())["estados"]
PESO = {"agente": 1.0, "parcial": 0.5, "nao": 0.0}

saida = {}
for nome, f in fontes.items():
    ids = f["ids"]
    falta = [i for i in ids if i not in phx]
    if falta:
        raise SystemExit(f"{nome}: capacidade sem estado no phxclaw.json: {falta}")
    agente = [i for i in ids if phx[i]["estado"] == "agente"]
    parcial = [i for i in ids if phx[i]["estado"] == "parcial"]
    nao = [i for i in ids if phx[i]["estado"] == "nao"]
    saida[nome] = {
        "total": len(ids), "no_agente": len(agente), "parcial": len(parcial), "nao": len(nao),
        "pct_agente": round(100 * len(agente) / len(ids), 1),
        "pct_com_bibliotecas": round(100 * sum(PESO[phx[i]["estado"]] for i in ids) / len(ids), 1),
        "falta": nao, "pela_metade": parcial, "lido_em": f["lido_em"],
        # Uma linha por capacidade, na ordem da fonte: e o que a grade da tela agrupa por
        # produto e por estado. As contagens acima continuam sendo as do painel.
        "capacidades": [{"id": i, "estado": phx[i]["estado"]} for i in ids],
    }
    s = saida[nome]
    print(f"{nome:12} {s['pct_agente']:5.1f}% no agente | {s['pct_com_bibliotecas']:5.1f}% com bibliotecas"
          f"  ({s['no_agente']} sim, {s['parcial']} pela metade, {s['nao']} nao, de {s['total']})")
(AQUI / "absorcao.json").write_text(json.dumps(saida, ensure_ascii=False, indent=1) + "\n")
