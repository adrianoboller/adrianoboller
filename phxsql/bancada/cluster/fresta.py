#!/usr/bin/env python3
"""A fresta entre "o master calou" e "os pares envelheceram" -- prova real.

    cargo build --release
    python3 bancada/cluster/fresta.py [diretorio]     # padrao /tmp/phx-fresta

## O que se mede

A eleicao conta quem PULSOU dentro da janela (`EstadoCluster::vivos`), e o
silencio do master sai do MESMO relogio (`master_visto_ms`). Os dois prazos
nao vencem juntos quando os nos caem em momentos diferentes: um par que morre
DEPOIS do master ainda esta dentro da janela no instante em que o master e
declarado calado. Nesse instante um no minoritario ve MAIORIA, e a eleicao --
que esta certa em relacao ao que enxerga -- o promove.

Este roteiro sobe tres nos em 5320-5322 (nunca as portas da `provar.py`, nem
as do demo) e mata dois deles nas DUAS ordens, com 1,5 s entre as mortes:

  master-primeiro : mata o master, espera, mata o par  -> o par sobrevive na
                    janela; o no isolado se elege
  par-primeiro    : mata o par, espera, mata o master  -> o par ja envelheceu;
                    o no isolado nao se elege

Mesmo binario, mesma configuracao, mesmos tres nos: muda so a ordem. E por
isso que a diferenca nao pode ser atribuida a outra coisa.

## O que este roteiro AFIRMA, e o que ele so MEDE

Afirma o que tem de valer nas DUAS ordens, porque e garantia:

  1. o no isolado RECUSA a escrita -- em qualquer papel que ele se ache;
  2. a recusa nao aponta um master morto (`REDIRECIONA`);
  3. voltada a maioria, os tres convergem para UM master e UMA epoca;
  4. os retratos SHA-256 dos tres ficam iguais.

Apenas MEDE -- imprime, nao reprova -- o papel e a epoca com que o no isolado
fica, porque e exatamente o que a fresta muda. Assim, no dia em que o motor
fechar a fresta, este roteiro continua VERDE e o numero e que muda; guarda que
afirmasse o defeito viraria catraca contra o proprio conserto.

Ver docs/CLUSTER.md 2.4, item 5.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import provar as P                                              # noqa: E402

# Portas proprias: a `provar.py` usa 5310-5312/5316 e o demo usa 5199/5599.
P.PORTAS = {"no1": 5320, "no2": 5321, "no3": 5322}
P.SMTP_PORTA = 5326
# O no3 e o mais graduado entre os que NAO sao master: se houver eleicao na
# fresta, e ele quem ganha o desempate. E a mesma forma do passo (e) da
# bancada, onde o master era o no2 (prioridade 2) e sobravam no3 (1) e no1 (0).
P.PRIORIDADES = {"no1": 0, "no2": 1, "no3": 3}

ESPERA_ENTRE_AS_MORTES_S = 1.5
FALHAS = []


def ok(nome, cond, detalhe=""):
    print(f"    {'ok ' if cond else 'FALHOU'} {nome}"
          + (f" -- {detalhe}" if detalhe else ""))
    if not cond:
        FALHAS.append(f"{nome}: {detalhe}")


def semear(base):
    """Tres nos em cluster, uma tabela, 200 linhas alcancadas nos tres."""
    subprocess.run(["rm", "-rf", base], check=False)
    h = P.hash_da_senha(P.SENHA)
    for nome in P.PORTAS:
        P.escrever_config(base, nome, P.config_de(nome, h))
    P.subir(base, "no1")
    time.sleep(1)
    P.subir(base, "no2")
    P.subir(base, "no3")
    C = {n: P.liga(p) for n, p in P.PORTAS.items()}
    time.sleep(3)
    m = C["no1"]
    m({"op": "criar_database", "database": "loja"})
    m({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
       "motivo_obrigatorio": False,
       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True}],
       "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                    "primario": True}]})
    P.inserir(m, [{"id": k, "nome": f"Cliente {k}"} for k in range(1, 201)])
    alvo = P.posicao(m)
    P.esperar(lambda: all(P.posicao(C[n]) >= alvo for n in ("no2", "no3")), 20)
    time.sleep(2)   # um pulso inteiro com os tres no lugar
    return C


def rodada(base, ordem):
    """Uma ordem de mortes. Devolve o que se mediu."""
    print(f"  [{ordem}] tres nos; no1 e o master; sobra o no3")
    C = semear(base)
    epoca_antes = P.estado(C["no3"])["epoca"]

    if ordem == "master-primeiro":
        P.matar("no1")
        time.sleep(ESPERA_ENTRE_AS_MORTES_S)
        P.matar("no2")
    else:
        P.matar("no2")
        time.sleep(ESPERA_ENTRE_AS_MORTES_S)
        P.matar("no1")
    del C["no1"], C["no2"]

    time.sleep(P.JANELA_S * 3)
    e3 = P.estado(C["no3"])
    rec = P.inserir(C["no3"], [{"id": 99001, "nome": "escrita do isolado"}])
    log = open(os.path.join(base, "no3", "servidor.log")).read()
    decisao = next((l for l in log.splitlines()
                    if "PROMOVIDO" in l or "NAO promovo" in l), "(nenhuma)")

    print(f"    medido: papel={e3['papel']} epoca={epoca_antes}->{e3['epoca']}"
          f" escrita_liberada={e3['escrita_liberada']}")
    print(f"    log ..: {decisao.replace('cluster: ', '')}")

    # (1) e (2): a garantia, nas duas ordens.
    ok(f"[{ordem}] o no isolado recusa a escrita",
       not rec.get("ok"), str(rec)[:120])
    ok(f"[{ordem}] a recusa nao aponta um master morto",
       rec.get("nome") != "REDIRECIONA", str(rec.get("nome")))

    # (3) e (4): voltada a maioria, o cluster converge.
    P.subir(base, "no1")
    P.subir(base, "no2")
    C["no1"] = P.liga(P.PORTAS["no1"])
    C["no2"] = P.liga(P.PORTAS["no2"])
    time.sleep(P.JANELA_S * 3)
    es = {n: P.estado(C[n]) for n in ("no1", "no2", "no3")}
    masters = {n: (e["master"] or {}).get("id") for n, e in es.items()}
    epocas = {n: e["epoca"] for n, e in es.items()}
    ok(f"[{ordem}] um master so, visto igual pelos tres",
       len(set(masters.values())) == 1 and None not in masters.values(),
       str(masters))
    ok(f"[{ordem}] uma epoca so nos tres", len(set(epocas.values())) == 1,
       str(epocas))
    retratos = {n: P.retrato(C[n]) for n in ("no1", "no2", "no3")}
    ok(f"[{ordem}] retratos SHA-256 identicos",
       len(set(retratos.values())) == 1, str(retratos))
    P.matar_tudo()

    return {"papel_do_isolado": e3["papel"],
            "epoca_antes": epoca_antes, "epoca_depois": e3["epoca"],
            "escrita_liberada": e3["escrita_liberada"],
            "escrita_aceita": bool(rec.get("ok")),
            "decisao": decisao.replace("cluster: ", ""),
            "master_no_fim": masters["no3"], "epoca_no_fim": epocas["no3"]}


def main():
    base = sys.argv[1] if len(sys.argv) > 1 else "/tmp/phx-fresta"
    if not os.path.exists(P.PHXSQLD):
        sys.exit(f"nao achei {P.PHXSQLD} -- rode `cargo build --release`")
    P.smtp_falso()   # os nos avisam por e-mail; alguem tem de atender
    r = {"espera_entre_as_mortes_s": ESPERA_ENTRE_AS_MORTES_S}
    for ordem in ("master-primeiro", "par-primeiro"):
        r[ordem] = rodada(base, ordem)
        print()
    r["falhas"] = FALHAS
    print("RESULTADO " + json.dumps(r, ensure_ascii=False))
    if FALHAS:
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
    finally:
        P.matar_tudo()
