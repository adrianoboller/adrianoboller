#!/usr/bin/env python3
"""Reponta o `arquivo` das guardas do servidor para a fonte que tem o trecho.

    python3 bancada/guardas/repontar-servidor.py            # so diz o que faria
    python3 bancada/guardas/repontar-servidor.py --gravar   # regrava o catalogo

A divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`) move
texto entre arquivos, e o catalogo aponta cada defeito reposto para UM arquivo.
Esta ferramenta e mecanica de proposito: para cada par `arquivo`/`trecho` cujo
arquivo e uma fonte do servidor (`bancada/fontes_do_servidor.py`), o arquivo
novo e o UNICO da lista que contem o trecho -- e o trecho uma vez so nele.

Zero candidatos (o trecho cruzou a fronteira de dois arquivos) ou dois (o
trecho ficou repetido) PARAM sem gravar nada: a regra do plano e escolher
outra fronteira, nunca afrouxar o trecho. O provador nao muda, e quem prova
as guardas repontadas e o `provar-guardas.py --so`.
"""
import importlib.util
import re
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
sys.path.insert(0, str(RAIZ / "bancada"))
import fontes_do_servidor  # noqa: E402

CATALOGO = AQUI / "catalogo.py"
CAMPO = re.compile(r'("arquivo"\s*:\s*")([^"]+)(")')


def guardas():
    esp = importlib.util.spec_from_file_location("catalogo", CATALOGO)
    mod = importlib.util.module_from_spec(esp)
    esp.loader.exec_module(mod)
    return mod.GUARDAS


def pares(g):
    if "trocas" in g:
        return [(t["arquivo"], t["trecho"]) for t in g["trocas"]]
    return [(g["arquivo"], g["trecho"])]


def principal():
    gravar = "--gravar" in sys.argv
    fontes = {fontes_do_servidor.relativo(f): f.read_text(encoding="utf-8")
              for f in fontes_do_servidor.todas()}
    # Os pares na ordem do catalogo, um por campo `"arquivo"` do texto: o
    # catalogo e uma lista de dicionarios literais, e a ordem dos dois e a
    # mesma. Se um dia nao for, as contagens divergem e a ferramenta para.
    todos = [(g["id"], a, t) for g in guardas() for a, t in pares(g)]
    texto = CATALOGO.read_text(encoding="utf-8")
    campos = list(CAMPO.finditer(texto))
    if [m.group(2) for m in campos] != [a for _i, a, _t in todos]:
        print("PAROU: os campos `arquivo` do texto nao batem, em ordem, com "
              "os pares do catalogo -- repontar pelo texto trocaria o errado")
        return 1
    trocas, parar = [], []
    for (gid, arquivo, trecho), m in zip(todos, campos):
        if arquivo not in fontes:
            continue
        candidatos = [rel for rel, t in fontes.items() if trecho in t]
        if len(candidatos) != 1:
            parar.append((gid, arquivo, candidatos))
            continue
        novo = candidatos[0]
        if fontes[novo].count(trecho) != 1:
            parar.append((gid, arquivo, [novo] * fontes[novo].count(trecho)))
            continue
        if novo != arquivo:
            trocas.append((m, gid, arquivo, novo))
    pares_do_servidor = sum(1 for _g, a, _t in todos if a in fontes)
    print(f"{pares_do_servidor} pares apontam para fontes do servidor; "
          f"{len(trocas)} a repontar; {len(parar)} sem fonte unica")
    for gid, arquivo, cands in parar:
        print(f"   PARA  {gid}: {arquivo} -> {len(cands)} candidato(s) {cands}")
    for _m, gid, velho, novo in trocas:
        print(f"   {gid}: {velho} -> {novo}")
    if parar:
        print("PAROU: trecho em zero ou dois arquivos. Escolha outra "
              "fronteira; nao afrouxe o trecho.")
        return 1
    if gravar and trocas:
        for m, _gid, _velho, novo in reversed(trocas):
            texto = texto[:m.start(2)] + novo + texto[m.end(2):]
        CATALOGO.write_text(texto, encoding="utf-8")
        print(f"catalogo regravado: {len(trocas)} campo(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(principal())
