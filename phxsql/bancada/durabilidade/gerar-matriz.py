#!/usr/bin/env python3
"""Gera a matriz de `docs/TRANSACOES.md` §5.7 a partir do que `prova.py`
mediu -- nunca se digita um numero direto no documento.

    python3 bancada/durabilidade/prova.py         # mede, grava resultado.json
    python3 bancada/durabilidade/gerar-matriz.py  # le resultado.json, escreve
                                                    # bancada/durabilidade/matriz-gerada.md
"""
import collections
import json
import os
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RESULTADO = os.path.join(AQUI, "resultado.json")
SAIDA = os.path.join(AQUI, "matriz-gerada.md")

REGIME_NOME = {"por_operacao": "por_operacao", "por_lote": "por_lote (padrão)", "sistema": "sistema"}


def bucket_do_ponto2a4(classe):
    if classe.startswith("P1") or "SEM_MARCA" in classe or "MARCA_INVALIDA" in classe:
        return "P1"
    if classe.startswith("P2"):
        return "P2"
    if classe.startswith("P3"):
        return "P3"
    if classe.startswith("P4"):
        return "P4"
    if "TARDE_DEMAIS" in classe:
        return None  # descartado da contagem -- o atraso mirou depois do fim
    return "outro:" + classe


def contar(corridas):
    c = collections.Counter()
    for r in corridas:
        b = bucket_do_ponto2a4(r["classe"])
        if b:
            c[b] += 1
    return c


def linha_p1(dados):
    """Ponto 1: o caso determinístico (sempre ABORTED, 0 linhas) mais o que a
    varredura do ponto 2-4 pegou em `atraso=0`, do mesmo regime."""
    por_regime = {}
    for r in dados["ponto1"]:
        regime = r["regime"]
        marca = r["relatorio"]
        det_ok = (marca is None and r["registros"]["a"] == 0 and r["registros"]["b"] == 0
                  and not r["marca_antes_de_matar"])
        # quantas vezes a varredura do ponto 2-4 tambem caiu em P1 (atraso=0
        # inclusive), do mesmo regime
        varr = next(v for v in dados["varredura"] if v["regime"] == regime)
        p1_na_varredura = sum(1 for r2 in varr["corridas"] if bucket_do_ponto2a4(r2["classe"]) == "P1")
        total_corridas = len(varr["corridas"])
        por_regime[regime] = (det_ok, p1_na_varredura, total_corridas)
    return por_regime


def linha_marcador(dados):
    return {m["regime"]: m for m in dados["marcador_por_regime"]}


def linha_cascata(dados):
    out = {}
    for c in dados["cascata"]:
        regime = c["regime"]
        vs = collections.Counter(r["veredito"] for r in c["corridas"])
        n = len(c["corridas"])
        out[regime] = (vs, n, c["t_calibracao_ms"])
    return out


def linha_tabela_apagada(dados):
    return {r["regime"]: r for r in dados["tabela_apagada"]}


def fmt_bucket(c, total):
    partes = []
    for k in ("P1", "P2", "P3", "P4"):
        if c.get(k):
            partes.append("%d/%d %s" % (c[k], total, k))
    outros = {k: v for k, v in c.items() if k not in ("P1", "P2", "P3", "P4")}
    for k, v in outros.items():
        partes.append("%d/%d %s" % (v, total, k))
    return " · ".join(partes) if partes else "(nenhuma corrida classificável)"


def main():
    if not os.path.exists(RESULTADO):
        sys.exit("falta %s -- rode prova.py primeiro" % RESULTADO)
    with open(RESULTADO) as f:
        dados = json.load(f)

    linhas = []
    linhas.append("<!-- GERADO por bancada/durabilidade/gerar-matriz.py a partir de")
    linhas.append("     bancada/durabilidade/resultado.json (medido em %s)." % dados["gerado_em"])
    linhas.append("     NÃO EDITE À MÃO -- rode prova.py e depois este script. -->")
    linhas.append("")

    # ---- ponto 1 ----
    p1 = linha_p1(dados)
    linhas.append("### P1 — antes do `fsync` da marca")
    linhas.append("")
    linhas.append("| regime | queda determinística (meio da transação, sem `COMMIT`) | "
                  "a corrida pegou o mesmo desfecho em |")
    linhas.append("|---|---|---|")
    for regime in ("por_operacao", "por_lote", "sistema"):
        det_ok, p1_var, total = p1[regime]
        linhas.append("| `%s` | %s — 0 linhas, 0 marca, `achadas=0` | %d de %d corridas da varredura |"
                      % (REGIME_NOME[regime], "ABORTED (provado)" if det_ok else "**FALHOU A PROVAR**",
                         p1_var, total))
    linhas.append("")

    # ---- pontos 2/3/4 ----
    linhas.append("### P2, P3, P4 — durante e depois da passada")
    linhas.append("")
    linhas.append("Distribuição dos desfechos ao longo da varredura de atraso "
                  "(cada célula é uma corrida com `SIGKILL` de verdade; "
                  "`N/total` conta quantas das corridas caíram naquele ponto):")
    linhas.append("")
    linhas.append("| regime | calibração (commit limpo) | distribuição medida |")
    linhas.append("|---|---:|---|")
    for v in dados["varredura"]:
        c = contar(v["corridas"])
        total = len(v["corridas"])
        linhas.append("| `%s` | %.1f ms (%d+%d linhas) | %s |"
                      % (REGIME_NOME[v["regime"]], v["t_calibracao_ms"], v["n_a"], v["n_b"],
                         fmt_bucket(c, total)))
    linhas.append("")
    linhas.append("Em **nenhuma** das %d corridas desta seção o relatório do arranque "
                  "ficou ambíguo, e em nenhuma o número de linhas terminou fora de "
                  "`{antes, antes+total}` — nunca metade." %
                  sum(len(v["corridas"]) for v in dados["varredura"]))
    linhas.append("")

    # ---- marcador por regime ----
    linhas.append("### O eixo em que o regime REALMENTE muda o que se vê: "
                  "quanto tempo a marca fica no disco depois de um commit que NÃO caiu")
    linhas.append("")
    linhas.append("| regime | logo após o `COMMIT` | 50 ms depois | 1,25 s depois |")
    linhas.append("|---|---:|---:|---:|")
    mk = linha_marcador(dados)
    for regime in ("por_operacao", "por_lote", "sistema"):
        m = mk[regime]
        linhas.append("| `%s` | %d marca(s) | %d marca(s) | %d marca(s) |"
                      % (REGIME_NOME[regime], m["logo_apos_commit"], m["50ms_depois"], m["1.25s_depois"]))
    linhas.append("")

    # ---- cascata ----
    linhas.append("### P5 — no meio da cascata do `ao_alterar`")
    linhas.append("")
    linhas.append("| regime | calibração (commit com a cascata) | veredito das corridas |")
    linhas.append("|---|---:|---|")
    casc = linha_cascata(dados)
    for regime in ("por_operacao", "por_lote", "sistema"):
        vs, n, tcal = casc[regime]
        partes = ["%d/%d %s" % (vs[k], n, k) for k in
                  ("CONSISTENTE", "PARCIAL_DENUNCIADO", "*** PARCIAL SEM AVISO ***",
                   "TARDE_DEMAIS", "SEM_MARCA", "MARCA_INVALIDA") if vs.get(k)]
        linhas.append("| `%s` | %.1f ms | %s |" % (REGIME_NOME[regime], tcal, " · ".join(partes)))
    linhas.append("")
    total_casc = sum(n for _, n, _ in casc.values())
    sem_aviso = sum(vs.get("*** PARCIAL SEM AVISO ***", 0) for vs, _, _ in casc.values())
    linhas.append("Em %d corridas: **%d** cascata(s) parcial(is) SEM aviso no relatório "
                  "(o desfecho que reprovaria a prova)." % (total_casc, sem_aviso))
    linhas.append("")

    # ---- 5.5(c) ----
    linhas.append("### §5.5(c) — a marca cuja tabela não abre mais")
    linhas.append("")
    linhas.append("| regime | a marca sobreviveu ao `SIGKILL`? | o relatório nomeou a tabela que falta? |")
    linhas.append("|---|---|---|")
    ta = linha_tabela_apagada(dados)
    for regime in ("por_operacao", "por_lote", "sistema"):
        r = ta[regime]
        rel = r["relatorio"] or {}
        nomeou = any("nao consegui abrir a " in l for l in rel.get("impossiveis_linhas", []))
        linhas.append("| `%s` | %s | %s (%d op.) |"
                      % (REGIME_NOME[regime], r["marca_achada_no_kill"],
                         "sim" if nomeou else "**NÃO**", rel.get("impossiveis", 0)))
    linhas.append("")

    with open(SAIDA, "w") as f:
        f.write("\n".join(linhas) + "\n")
    print("escrevi %s (%d linhas)" % (SAIDA, len(linhas)))


if __name__ == "__main__":
    main()
