#!/usr/bin/env python3
"""Modelos do estudo L-001 sobre o .sqlite do extrator serie_do_que_falta.py.

Le so o .sqlite (e, para o lexico do piso do dono, o SPRINTS.md de HEAD, mostrando a frase que
casou). Nada aqui le o relogio; o acaso tem semente fixa. Duas corridas dao os mesmos numeros.

Saidas, todas impressas:
  1. series por rodada nas duas definicoes (P = dia UTC com commit no SPRINTS.md, a
     pre-registrada; R = registro de rodada em docs/sprints/, sensibilidade);
  2. backtest de origem movel, 1 passo: ingenuo, deriva (media das variacoes), tendencia (MQO);
     MAE, MAE relativo ao ingenuo e MASE (escala = erro do ingenuo dentro do treino);
  3. bootstrap (10.000) da media de (saiu - entrou) por rodada -> H2;
  4. simulacao de dois fluxos com bootstrap em dois niveis (reamostra as rodadas -> parametro;
     sorteia rodadas desse conjunto -> trajetoria), 10.000 trajetorias, teto de 50 rodadas,
     cenarios: escopo como tem entrado, escopo congelado, e congelado + dono destravando no
     limite superior que o dado permite (Poisson exato, 0 eventos);
  5. a previsao datada (h = 1 e 3 rodadas).

Uso: python3 docs/ciencia/extratores/modelos_do_que_falta.py [--db CAMINHO]
"""
from __future__ import annotations

import argparse
import math
import random
import sqlite3
import subprocess
from pathlib import Path

PHX = Path(__file__).resolve().parents[3]
SEMENTE = 20261009
REAMOSTRAS = 10_000
TETO = 50
# Lexico do piso do dono na absorcao: id da capacidade -> frase que tem de estar no SPRINTS.md
# de HEAD (linha do SP000032: «iMessage, voz ao vivo, RAPL e nuvem dependem de recurso do dono»).
# A frase casada e impressa: quem le julga o casamento em vez de acreditar nele.
LEXICO_DONO = {"canal_imessage": "iMessage", "voz_wake": "voz ao vivo",
               "telemetria_energia": "RAPL", "ambientes_nuvem": "nuvem"}
INICIO_R = "2026-10-01T20:53:33+00:00"  # ultimo commit das duas fontes antes do 1o registro de rodada


def q(con, sql, *a):
    return con.execute(sql, a).fetchall()


# ------------------------------------------------------------------ series por rodada

def rodadas_P(con):
    """Def P: dia UTC com commit no SPRINTS.md. O nascimento (seq 1) e estoque, nao fluxo."""
    out = []
    for (dia,) in q(con, "SELECT DISTINCT dia FROM fato_sprints ORDER BY dia"):
        f = q(con, "SELECT total, concluidas, bloqueadas, falta, falta_pct FROM fato_sprints "
                   "WHERE dia=? ORDER BY seq DESC LIMIT 1", dia)[0]
        ent, sai = q(con, "SELECT SUM(entrou), SUM(saiu) FROM fato_sprints WHERE dia=? AND seq>1", dia)[0]
        out.append({"rodada": dia, "total": f[0], "C": f[1], "B": f[2], "falta": f[3],
                    "pct": f[4], "entrou": ent or 0, "saiu": sai or 0})
    return out


def fronteiras_R(con):
    """Def R: cada registro de docs/sprints/ fecha uma rodada na data do commit que o criou."""
    linhas = subprocess.run(["git", "log", "--reverse", "--diff-filter=A", "--format=%cI",
                             "--name-only", "--", "phxclaw/docs/sprints/"], cwd=PHX.parent,
                            capture_output=True, text=True).stdout.split("\n")
    fr, iso = [], None
    for l in linhas:
        if l[:4].isdigit() and "T" in l:
            iso = l
        elif "Sessao_" in l:
            fr.append((l.rsplit("_Sprint_", 1)[1][:8], iso))
    return fr


def rodadas_R_sprints(con):
    out, ini = [], INICIO_R
    base = q(con, "SELECT total, concluidas, bloqueadas, falta, falta_pct FROM fato_sprints "
                  "WHERE data_iso<=? ORDER BY seq DESC LIMIT 1", ini)[0]
    out.append({"rodada": "inicio", "total": base[0], "C": base[1], "B": base[2], "falta": base[3],
                "pct": base[4], "entrou": 0, "saiu": 0})
    for sp, fim in fronteiras_R(con):
        ent, sai = q(con, "SELECT SUM(entrou), SUM(saiu) FROM fato_sprints WHERE data_iso>? AND data_iso<=?", ini, fim)[0]
        f = q(con, "SELECT total, concluidas, bloqueadas, falta, falta_pct FROM fato_sprints "
                   "WHERE data_iso<=? ORDER BY seq DESC LIMIT 1", fim)[0]
        out.append({"rodada": sp, "total": f[0], "C": f[1], "B": f[2], "falta": f[3], "pct": f[4],
                    "entrou": ent or 0, "saiu": sai or 0})
        ini = fim
    return out


def absorcao_por_commit(con):
    return q(con, "SELECT commit_h, data_iso, SUM(total), SUM(no_agente) FROM fato_absorcao "
                  "GROUP BY commit_h ORDER BY data_iso")


def rodadas_absorcao(con, fronteiras):
    """Lacuna = itens - no agente. entrou = aumento de itens; saiu = aumento do «no agente»."""
    serie = absorcao_por_commit(con)

    def estado_em(iso):
        ult = [r for r in serie if r[1] <= iso]
        return ult[-1] if ult else None

    out, ant = [], estado_em(fronteiras[0][1])
    out.append({"rodada": fronteiras[0][0], "total": ant[2], "C": ant[3], "falta": ant[2] - ant[3],
                "pct": 100 * (ant[2] - ant[3]) / ant[2], "entrou": 0, "saiu": 0})
    for nome, iso in fronteiras[1:]:
        e = estado_em(iso)
        out.append({"rodada": nome, "total": e[2], "C": e[3], "falta": e[2] - e[3],
                    "pct": 100 * (e[2] - e[3]) / e[2], "entrou": e[2] - ant[2], "saiu": e[3] - ant[3]})
        ant = e
    return out


# ------------------------------------------------------------------ backtest

def mqo(ys):
    n = len(ys)
    xs = list(range(n))
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    b = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx if sxx else 0.0
    return my + b * (n - mx)  # previsao para o indice n


def backtest(ys, minimo=3):
    erros = {"ingenuo": [], "deriva": [], "tendencia": []}
    escalados = {k: [] for k in erros}
    for t in range(minimo, len(ys)):
        tr, real = ys[:t], ys[t]
        prev = {"ingenuo": tr[-1],
                "deriva": tr[-1] + (tr[-1] - tr[0]) / (len(tr) - 1),
                "tendencia": mqo(tr)}
        escala = sum(abs(tr[i] - tr[i - 1]) for i in range(1, len(tr))) / (len(tr) - 1)
        for k, p in prev.items():
            erros[k].append(abs(real - p))
            if escala > 0:
                escalados[k].append(abs(real - p) / escala)
    res = {}
    for k in erros:
        mae = sum(erros[k]) / len(erros[k]) if erros[k] else float("nan")
        res[k] = {"mae": mae, "mase": (sum(escalados[k]) / len(escalados[k])) if escalados[k] else None,
                  "n": len(erros[k]), "n_mase": len(escalados[k])}
    base = res["ingenuo"]["mae"]
    for k in res:
        res[k]["rel"] = res[k]["mae"] / base if base else (0.0 if res[k]["mae"] == 0 else math.inf)
    return res


# ------------------------------------------------------------------ bootstrap e simulacao

def quantil(xs, p):
    s = sorted(xs)
    i = (len(s) - 1) * p
    lo, hi = math.floor(i), math.ceil(i)
    return s[lo] + (s[hi] - s[lo]) * (i - lo)


def ic_media(vals, rng):
    medias = []
    for _ in range(REAMOSTRAS):
        a = [rng.choice(vals) for _ in vals]
        medias.append(sum(a) / len(a))
    return sum(vals) / len(vals), quantil(medias, 0.025), quantil(medias, 0.975)


def simular(pares, O0, B0, C0, rng, congelado=False, p_bloq=0.0, destrava=0.0, alvo="tudo", piso=0):
    """Devolve (rodadas ate o alvo ou None, pct na rodada 1, pct na rodada 3) por trajetoria."""
    res = []
    for _ in range(REAMOSTRAS):
        param = [rng.choice(pares) for _ in pares]  # nivel 1: incerteza do parametro
        O, B, C = O0, B0, C0
        hit, p1, p3 = None, None, None
        for r in range(1, TETO + 1):
            mu, lam = rng.choice(param)  # nivel 2: a rodada
            fecha = min(mu, O)
            O -= fecha
            C += fecha
            if destrava:
                solta = sum(1 for _ in range(B) if rng.random() < destrava)
                B -= solta
                O += solta
            if not congelado:
                for _ in range(lam):
                    if rng.random() < p_bloq:
                        B += 1
                    else:
                        O += 1
            tot = O + B + C
            pct = 100 * (O + B) / tot if tot else 0.0
            if r == 1:
                p1 = pct
            if r == 3:
                p3 = pct
            chegou = (O + B == 0) if alvo == "tudo" else (O <= piso)
            if hit is None and chegou:
                hit = r
            if hit is not None and r >= 3:
                break
        res.append((hit, p1, p3))
    return res


def resumo_sim(res):
    hits = [h for h, _, _ in res if h is not None]
    p_nao = 1 - len(hits) / len(res)
    if len(hits) >= len(res) * 0.5:
        todos = sorted([h if h is not None else TETO + 1 for h, _, _ in res])
        med = quantil(todos, 0.5)
        q05, q95 = quantil(todos, 0.05), quantil(todos, 0.95)
    else:
        med = q05 = q95 = None
    p1 = [x for _, x, _ in res]
    p3 = [x for _, _, x in res if x is not None]
    return {"p_nao": p_nao, "med": med, "q05": q05, "q95": q95,
            "h1": (quantil(p1, 0.05), quantil(p1, 0.5), quantil(p1, 0.95)),
            "h3": (quantil(p3, 0.05), quantil(p3, 0.5), quantil(p3, 0.95)) if p3 else None,
            "p1_abaixo": sum(1 for x in p1 if x < 33.3333 - 1e-6) / len(p1)}


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 1.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    m = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - m), min(1.0, c + m))


def fmt(x, c=1):
    return "—" if x is None else f"{x:.{c}f}".replace(".", ",")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default=str(PHX / "target/ciencia/serie_do_que_falta.sqlite"))
    con = sqlite3.connect(ap.parse_args().db)
    rng = random.Random(SEMENTE)

    P = rodadas_P(con)
    R = rodadas_R_sprints(con)
    fr = [("inicio", INICIO_R)] + fronteiras_R(con)
    A = rodadas_absorcao(con, fr)
    print("== 1. series por rodada")
    for nome, s in (("P sprints (dia)", P), ("R sprints (registro)", R), ("R absorcao (lacuna)", A)):
        print(f"-- {nome}: {len(s)} pontos")
        for x in s:
            print(f"   {x['rodada']:<10} total {x['total']:>3}  falta {x['falta']:>3}  "
                  f"{fmt(x['pct'])}%  entrou {x['entrou']:>2}  saiu {x['saiu']:>2}")

    print("\n== 2. backtest de origem movel, 1 passo (minimo 3 pontos de treino)")
    com = [r[0] for r in q(con, "SELECT falta_pct FROM fato_sprints ORDER BY seq")]
    for nome, ys in (("sprints por commit (sensib.)", com), ("sprints P", [x["pct"] for x in P]),
                     ("sprints R", [x["pct"] for x in R]), ("absorcao R (lacuna, itens)", [x["falta"] for x in A])):
        bt = backtest(ys)
        print(f"-- {nome}: {len(ys)} pontos, {bt['ingenuo']['n']} origens")
        for k, v in bt.items():
            print(f"   {k:<10} MAE {fmt(v['mae'], 2)}  rel.ingenuo {fmt(v['rel'], 2)}  "
                  f"MASE {fmt(v['mase'], 2)} (n={v['n_mase']})")

    print("\n== 3. H2: bootstrap da media de (saiu - entrou) por rodada, 10.000 reamostras")
    for nome, s in (("P sprints", P), ("R sprints", R[1:]), ("R absorcao", A[1:])):
        liq = [x["saiu"] - x["entrou"] for x in s]
        m, lo, hi = ic_media(liq, rng)
        print(f"   {nome:<11} n={len(liq)} liquido/rodada {liq}  media {fmt(m, 2)}  IC95 [{fmt(lo, 2)}; {fmt(hi, 2)}]")

    print("\n== 4. simulacao de dois fluxos (10.000 trajetorias, teto 50 rodadas)")
    ult = P[-1]
    B0, C0 = ult["B"], ult["C"]
    O0 = ult["falta"] - B0
    # entradas que nasceram BLOQUEADA, por definicao (id novo cujo primeiro estado e BLOQUEADA)
    nasc = q(con, "SELECT sprint, MIN(data_iso) FROM fato_sprint_item GROUP BY sprint")
    prim = {s: q(con, "SELECT estado FROM fato_sprint_item WHERE sprint=? AND data_iso=?", s, d)[0][0] for s, d in nasc}
    seq1 = q(con, "SELECT data_iso FROM fato_sprints WHERE seq=1")[0][0]
    pos = [s for s, d in nasc if d > seq1]
    pb_P = sum(1 for s in pos if prim[s] == "BLOQUEADA") / len(pos)
    posR = [s for s, d in nasc if d > INICIO_R]
    pb_R = (sum(1 for s in posR if prim[s] == "BLOQUEADA") / len(posR)) if posR else 0.0
    # destravamento do dono: 0 saidas de BLOQUEADA observadas; limite superior Poisson exato 95%
    exp_R = sum(x["B"] for x in R[:-1])  # sprint-rodadas bloqueadas expostas
    saidas_bloq = q(con, "SELECT COUNT(*) FROM fato_sprint_item a JOIN fato_sprint_item b "
                         "ON a.sprint=b.sprint AND b.data_iso>a.data_iso WHERE a.estado='BLOQUEADA' "
                         "AND b.estado<>'BLOQUEADA'")[0][0]
    sup = -math.log(0.05) / exp_R if saidas_bloq == 0 else None
    print(f"   estado inicial (HEAD do SPRINTS.md): abertas nao bloqueadas {O0}, bloqueadas {B0}, "
          f"concluidas {C0}; p(entrada nasce bloqueada) P={fmt(pb_P, 3)} R={fmt(pb_R, 3)}")
    print(f"   saidas de BLOQUEADA observadas: {saidas_bloq} em {exp_R} sprint-rodadas (def R); "
          f"taxa sup. 95% (Poisson exato) {fmt(sup, 3)}/sprint-rodada")
    cenarios = []
    for nome_def, s, pb in (("P", P, pb_P), ("R", R[1:], pb_R)):
        pares = [(x["saiu"], x["entrou"]) for x in s]
        for cen, kw in (("como tem entrado", {}), ("congelado", {"congelado": True}),
                        ("congelado + dono no teto", {"congelado": True, "destrava": sup or 0.0})):
            for alvo in ("tudo", "nao_bloqueadas"):
                if alvo == "nao_bloqueadas" and "dono" in cen:
                    continue
                r = resumo_sim(simular(pares, O0, B0, C0, rng, p_bloq=pb, alvo=alvo, **kw))
                cenarios.append((nome_def, cen, alvo, r))
                print(f"   [{nome_def}] {cen:<25} alvo={alvo:<15} P(nao chega em 50)={fmt(100 * r['p_nao'])}%  "
                      f"rodadas med {fmt(r['med'], 0)} IC90 [{fmt(r['q05'], 0)}; {fmt(r['q95'], 0)}]  "
                      f"falta% h1 {fmt(r['h1'][1])} [{fmt(r['h1'][0])}; {fmt(r['h1'][2])}]  "
                      f"h3 {fmt(r['h3'][1]) if r['h3'] else '—'} [{fmt(r['h3'][0]) if r['h3'] else '—'}; "
                      f"{fmt(r['h3'][2]) if r['h3'] else '—'}]  P(h1<33,3%)={fmt(100 * r['p1_abaixo'])}%")

    # absorcao: lacuna, piso do dono pelo lexico casado no SPRINTS.md de HEAD
    texto = (PHX / "docs/absorcao/SPRINTS.md").read_text(encoding="utf-8")
    casou = {k: f for k, f in LEXICO_DONO.items() if f in texto}
    import json
    ab = json.loads((PHX / "docs/absorcao/absorcao.json").read_text(encoding="utf-8"))
    piso = sum(1 for v in ab.values() for i in v["falta"] + v["pela_metade"] if i in casou)
    print(f"\n   absorcao: lexico do dono casado {casou}; itens da lacuna atual no piso do dono: {piso}")
    ultA = A[-1]
    paresA = [(x["saiu"], x["entrou"]) for x in A[1:]]
    for cen, kw in (("como tem entrado", {}), ("congelado", {"congelado": True})):
        # o piso do dono entra como BLOQUEADO: nenhuma rodada de trabalho o fecha (0 saidas
        # observadas desses ids); «tudo» so chega a zero se o dono destravar
        for alvo in ("tudo", "nao_bloqueadas"):
            r = resumo_sim(simular(paresA, ultA["falta"] - piso, piso, ultA["C"], rng, alvo=alvo, **kw))
            print(f"   [R absorcao] {cen:<18} alvo={alvo:<15} P(nao chega em 50)={fmt(100 * r['p_nao'])}%  "
                  f"rodadas med {fmt(r['med'], 0)} IC90 [{fmt(r['q05'], 0)}; {fmt(r['q95'], 0)}]  "
                  f"lacuna% h1 {fmt(r['h1'][1])} [{fmt(r['h1'][0])}; {fmt(r['h1'][2])}]")
    for nome, s in (("P", P), ("R", R[1:])):
        fecharam = [x for x in s if x["saiu"] > 0]
        k = sum(1 for x in fecharam if x["entrou"] > 0)
        lo, hi = wilson(k, len(fecharam))
        print(f"   [{nome}] rodadas que fecharam sprint e tambem abriram: {k}/{len(fecharam)}  "
              f"Wilson95 [{fmt(100 * lo)}%; {fmt(100 * hi)}%]")
    k = sum(1 for x in R[1:] if x["saiu"] > 0)
    lo, hi = wilson(k, len(R) - 1)
    print(f"\n   rodadas R que fecharam >=1 sprint: {k}/{len(R) - 1}  Wilson95 [{fmt(100 * lo)}%; {fmt(100 * hi)}%]")
    k = sum(1 for x in R[1:] if x["entrou"] > 0)
    lo, hi = wilson(k, len(R) - 1)
    print(f"   rodadas R que abriram >=1 sprint: {k}/{len(R) - 1}  Wilson95 [{fmt(100 * lo)}%; {fmt(100 * hi)}%]")


if __name__ == "__main__":
    main()
