#!/usr/bin/env python3
"""A prova real do pedido 254: a marca `.tx` do COMMIT feito dentro de uma
tabela reservada tem de SAIR no `bulkinsert(false)`.

    cargo build --release
    python3 bancada/tomada/marca-apos-bulkinsert.py

Sai 0 quando a marca some no «ok» do `bulkinsert(false)` e a tomada chutada
DEPOIS desse «ok» reabre com o arranque calado; sai 1 quando a marca sobra --
que e o defeito do pedido 254, achado 1 da bancada `chutar-a-tomada.py`
(hipotese escrita antes e confirmada sem queda em 16/09/2026).

# Por que um roteiro a parte, e nao so a bancada inteira

A bancada inteira leva mais de dez minutos e espera bancada de tempo vizinha;
o defeito se reproduz em segundos e precisa ser medido DUAS vezes -- com o
conserto e com o defeito reposto --, e uma prova que custa vinte minutos por
lado e uma prova que ninguem repete. Este roteiro mede so o ponto
`tx_em_bulk`, nos dois jeitos em que a bancada o mede:

  1. SEM QUEDA: reserva, BEGIN, N `inserir`, COMMIT, `bulkinsert(false)`, e
     a lista de marcas `.tx` no diretorio do banco logo depois do «ok» e 300
     ms depois -- com o caminho e o instante de cada uma;
  2. COM QUEDA depois do «ok» final: a mesma `corrida()` da bancada, com o
     atraso alem da calibracao, e a classe que o `julgar_tx_em_bulk` devolve:
     `APOS_BULKINSERT_FALSE_SEM_MARCA` e o motor certo,
     `APOS_BULKINSERT_FALSE_MARCA_REPORTADA` e o defeito.

Tudo que sobe, mata, reabre e julga vem da bancada -- reuso, e nao terceira
copia: se a bancada mudar o que «marca» quer dizer, este roteiro muda junto.
Passa pelo mesmo portao `esta-medindo.sh` e congela a mesma copia do binario.
"""
import importlib.util
import shutil
import json
import os
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))

_spec = importlib.util.spec_from_file_location(
    "tomada", os.path.join(AQUI, "chutar-a-tomada.py"))
tomada = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tomada)
dur = tomada.dur
Ligacao = tomada.Ligacao

RODADAS = int(os.environ.get("PHX_254_RODADAS", "3"))


def instante(t=None):
    t = time.time() if t is None else t
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(t)) + ".%03dZ" % int((t % 1) * 1000)


def marcas_com_caminho():
    """Cada marca com o caminho ABSOLUTO e o `mtime` (o instante em que o
    COMMIT a gravou): e o numero que o pedido exige -- caminho e instante."""
    saida = []
    for nome in dur.marcas():
        caminho = os.path.join(dur.caminho_db(), nome)
        try:
            saida.append({"caminho": caminho, "gravada_em": instante(os.path.getmtime(caminho))})
        except OSError:
            saida.append({"caminho": caminho, "gravada_em": None})
    return saida


def sem_queda():
    p, _ = tomada.subir(limpar=True)
    try:
        c = Ligacao()
        tomada.montar_bulk_tx(c)
        comecou = threading.Event()
        prog = {"confirmadas": 0}
        tomada.roteiro_tx_em_bulk(c, comecou, prog)
        t_ok = time.time()
        logo = marcas_com_caminho()
        time.sleep(0.3)
        tarde = marcas_com_caminho()
        c.fechar()
        return {"reservou": prog.get("reservou"), "begin": prog.get("begin"),
                "commit": prog.get("commit"), "soltou": prog.get("soltou"),
                "recusa": prog.get("recusa"), "confirmadas": prog.get("confirmadas"),
                "marcas_apos_commit": prog.get("marcas_apos_commit"),
                "ok_do_bulkinsert_false_em": instante(t_ok),
                "marcas_logo_apos_o_ok": logo,
                "lidas_300ms_depois_em": instante(),
                "marcas_300ms_depois": tarde}
    finally:
        dur.derrubar_limpo(p)


def com_queda_depois_do_ok():
    """As corridas que caem DEPOIS do «ok» final. O atraso sai da calibracao
    (nunca se chuta) com folga de 25%; a corrida em que a tomada caiu ANTES
    do `bulkinsert(false)` nao conta e e refeita com mais folga -- o ponto que
    se quer medir e o de depois do «ok», e so ele."""
    t_limpo, prog_limpo = tomada.calibrar(tomada.montar_bulk_tx, tomada.roteiro_tx_em_bulk)
    print("  calibracao (tx_em_bulk, sem matar): %.1f ms  progresso=%s" % (t_limpo * 1000, prog_limpo))
    atraso = t_limpo * 1.25
    corridas, tentativas = [], 0
    while len(corridas) < RODADAS and tentativas < RODADAS * 3:
        tentativas += 1
        r = tomada.corrida("tx_em_bulk#254.%d" % tentativas, tomada.montar_bulk_tx,
                           tomada.roteiro_tx_em_bulk, atraso, tomada.julgar_tx_em_bulk)
        caiu_depois = bool(r["progresso"].get("soltou"))
        print("  corrida %d: atraso %.1f ms -> %-40s marcas antes de reabrir=%s relatorio=%s%s"
              % (tentativas, r["atraso_ms"], r["classe"], r["marcas_antes_de_reabrir"],
                 None if r["relatorio"] is None else
                 {k: v for k, v in r["relatorio"].items() if k != "impossiveis_linhas"},
                 "" if caiu_depois else "   (caiu ANTES do ok final: nao conta, refeita)"))
        if caiu_depois:
            corridas.append({k: r[k] for k in ("rotulo", "atraso_ms", "atraso_medido_ms", "classe",
                                               "valido", "registros", "marcas_antes_de_reabrir",
                                               "relatorio", "marcas_depois", "detalhe")})
        else:
            atraso *= 1.5
    return {"calibracao_ms": round(t_limpo * 1000, 1), "corridas": corridas,
            "tentativas": tentativas}


def main():
    if not os.path.exists(dur.PHXSQLD):
        print("falta %s -- rode `cargo build --release`" % dur.PHXSQLD)
        return 2
    bruto = {"medido_em_utc": instante(), "binario": tomada.binario(), "rodadas": RODADAS,
             "n_bulk_tx": tomada.N_BULK_TX, "porta": tomada.PORTA}
    print("== pedido 254: a marca do COMMIT dentro da reserva sai no bulkinsert(false)? ==")
    print("   binario %s (sha256 %s..., compilado %s)" % (
        bruto["binario"]["versao"], bruto["binario"]["sha256"][:12],
        bruto["binario"]["compilado_em_utc"]))
    if bruto["binario"]["fontes_mais_novos_que_o_binario"]:
        print("   aviso: fontes mais novos que o binario: %s"
              % ", ".join(bruto["binario"]["fontes_mais_novos_que_o_binario"]))
    bruto["maquina"] = tomada.portao(esperar=os.environ.get("PHX_TOMADA_SEM_ESPERA") != "1")
    print("   portao: %s" % bruto["maquina"])
    try:
        print("\n== 1. sem queda: reserva, BEGIN, %d inserir, COMMIT, bulkinsert(false) ==" % tomada.N_BULK_TX)
        s = sem_queda()
        bruto["sem_queda"] = s
        for k in ("reservou", "begin", "commit", "soltou", "confirmadas"):
            print("   %-12s %s" % (k, s[k]))
        print("   marcas apos o COMMIT:            %s" % s["marcas_apos_commit"])
        print("   ok do bulkinsert(false) em:      %s" % s["ok_do_bulkinsert_false_em"])
        print("   marcas logo apos o ok:           %s" % json.dumps(s["marcas_logo_apos_o_ok"]))
        print("   marcas 300 ms depois (%s): %s" % (s["lidas_300ms_depois_em"],
                                                     json.dumps(s["marcas_300ms_depois"])))
        aceito = bool(s["commit"]) and bool(s["soltou"])
        limpo_sem_queda = aceito and not s["marcas_logo_apos_o_ok"] and not s["marcas_300ms_depois"]
        marca_no_commit = bool(s["marcas_apos_commit"])

        print("\n== 2. com queda DEPOIS do ok final (x%d) ==" % RODADAS)
        q = com_queda_depois_do_ok()
        bruto["com_queda"] = q
        classes = sorted({r["classe"] for r in q["corridas"]})
        limpo_com_queda = (len(q["corridas"]) == RODADAS
                           and all(r["classe"] == "APOS_BULKINSERT_FALSE_SEM_MARCA"
                                   and r["valido"] and r["relatorio"] is None
                                   for r in q["corridas"]))
    finally:
        tomada.apagar_copia_do_binario()
        shutil.rmtree(dur.BASE, ignore_errors=True)

    print("\n== veredito ==")
    print("   o COMMIT dentro da reserva deixa a marca pendente (janela aberta): %s"
          % ("sim" if marca_no_commit else "NAO -- o cenario nao e o do pedido"))
    print("   sem queda: marca depois do ok do bulkinsert(false): %s"
          % ("NENHUMA" if limpo_sem_queda else "SOBROU " + json.dumps(s["marcas_logo_apos_o_ok"])))
    print("   com queda depois do ok: %d/%d corridas, classes %s"
          % (len(q["corridas"]), RODADAS, classes))
    veredito = limpo_sem_queda and limpo_com_queda
    bruto["veredito"] = "CONSERTADO: a marca sai no bulkinsert(false)" if veredito else \
        "DEFEITO (pedido 254): a marca do COMMIT sobrevive ao ok do bulkinsert(false)"
    print("   " + bruto["veredito"])
    saida = os.environ.get("PHX_254_SAIDA")
    if saida:
        with open(saida, "w", encoding="utf-8") as f:
            json.dump(bruto, f, indent=1, ensure_ascii=False)
        print("   gravado em %s" % saida)
    return 0 if veredito else 1


if __name__ == "__main__":
    sys.exit(main())
