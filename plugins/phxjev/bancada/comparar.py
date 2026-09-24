#!/usr/bin/env python3
"""Bancada do PhxJev: juiz local (Ollama) contra o juiz Claude, nas mesmas perguntas.

Verdade: `casos.json`, conferida no codigo ou medida (nunca o veredito de um
juiz). Lado Claude: as distribuicoes que ele ja deu, lidas do registro pelo
id do pedido. Lado local: uma passada por pergunta, logprob da letra.

A comparacao NAO e trabalho igual, e isso vai no resultado: o Claude leu o
codigo com ferramentas, o local recebe so o trecho do `casos.json`.

Uso: python3 plugins/phxjev/bancada/comparar.py <modelo> [<modelo> ...]
"""
import json
import os
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(AQUI, "..", "scripts"))
import phxjev  # noqa: E402


def claude(caso_id, pergunta, registro):
    """Distribuicao que o juiz Claude deu para esta pergunta deste caso, se houver."""
    prefixo = {"reg-slot": "reg-reaproveita", "321-causa": "causa-321"}.get(caso_id, caso_id + "-")
    for r in phxjev.ler(registro):
        if r["item"].startswith(prefixo) and pergunta in r["perguntas"]:
            return r["perguntas"][pergunta]["dist"]
    return None


def resumo(pares):
    if not pares:
        return None
    return {
        "n": len(pares),
        "brier": round(sum(b for b, _ in pares) / len(pares), 4),
        "acerto": round(sum(a for _, a in pares) / len(pares), 3),
    }


def brier_e_acerto(dist, verdade):
    b = sum((p - (1 if o == verdade else 0)) ** 2 for o, p in dist.items()) / len(dist)
    return b, 1 if max(dist, key=dist.get) == verdade else 0


def main(modelos):
    casos = json.load(open(os.path.join(AQUI, "casos.json"), encoding="utf-8"))["casos"]
    res = {"medido_em": time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime()),
           "trabalho_igual": False,
           "nota": "Claude leu o codigo com ferramentas; o local recebe so o trecho do casos.json",
           "juizes": {}}
    lado_claude, mesmas = [], []
    for c in casos:
        for nome, q in c["perguntas"].items():
            d = claude(c["id"], nome, phxjev.REGISTRO)
            if d:
                lado_claude.append(brier_e_acerto(d, q["verdade"]))
                mesmas.append((c["id"], nome))
    res["juizes"]["claude (registro)"] = resumo(lado_claude)
    for m in modelos:
        todos, so_mesmas, tempos, linhas = [], [], [], []
        for c in casos:
            ctx = "\n".join(c["contexto"])
            for nome, q in c["perguntas"].items():
                ops = phxjev.opcoes_de(q)
                dist, ms, cob = phxjev.perguntar_local(m, ctx, q["pergunta"], ops)
                ba = brier_e_acerto(dist, q["verdade"])
                todos.append(ba)
                tempos.append(ms)
                if (c["id"], nome) in mesmas:
                    so_mesmas.append(ba)
                linhas.append(f"  {c['id']:10s} {nome:11s} p(verdade)={dist[q['verdade']]:.2f} "
                              f"{'ok ' if ba[1] else 'ERR'} {ms:6.0f} ms cobertura {cob:.2f}")
        res["juizes"][f"local {m}"] = dict(resumo(todos), mediana_ms=round(sorted(tempos)[len(tempos) // 2]))
        res["juizes"][f"local {m} (so as do claude)"] = resumo(so_mesmas)
        print(f"{m}:")
        print("\n".join(linhas))
    with open(os.path.join(AQUI, "resultados.json"), "w", encoding="utf-8") as f:
        json.dump(res, f, ensure_ascii=False, indent=1)
        f.write("\n")
    for k, v in res["juizes"].items():
        print(f"{k:40s} {v}")


if __name__ == "__main__":
    main(sys.argv[1:] or ["qwen2.5:1.5b"])
