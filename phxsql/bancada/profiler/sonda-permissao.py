#!/usr/bin/env python3
"""Quem consegue LIGAR o profiler e LER o que ele viu?

A ficha do `op_profiler_ligar` diz «**So administrador**». O portao geral do
`despachar` pergunta `Atividade::Administrar` sobre a base do pedido -- e o
pedido do profiler NAO TEM base. Entao a pergunta cai na base VAZIA, e quem
tem `bases: {"*": {"administrar": true}}` responde sim sem ser admin do
servidor. E o mesmo furo do juntar/unir, com o sinal trocado.
"""
import json
import os

from comum import AQUI, TOKEN, Conexao, baixar, subir

BASE = os.path.join(AQUI, "srv-perm")
PORTA = 6255
LOG = os.path.join(BASE, "espiao.txt")


def tenta(nome, senha, alvo):
    c = Conexao(PORTA)
    if nome:
        r = c.fala({"op": "login", "usuario": nome, "senha": senha})
        if not r.get("ok"):
            print("   %-9s nao conseguiu nem entrar: %s" % (nome, r["erro"]))
            c.fechar()
            return
    r = c.fala({"op": "profiler_ligar", "arquivo": alvo, "guardar": 100})
    print("   %-9s profiler_ligar -> %s"
          % (nome or "(so token)",
             "LIGOU (arquivo %r)" % r["resultado"]["arquivo"] if r.get("ok")
             else "negado: %s" % r["erro"]))
    r2 = c.fala({"op": "profiler", "max": 10})
    print("   %-9s profiler       -> %s"
          % (nome or "(so token)",
             "LEU %d evento(s)" % len(r2["resultado"]["eventos"])
             if r2.get("ok") else "negado: %s" % r2["erro"]))
    if r.get("ok"):
        c.fala({"op": "profiler_desligar"})
    c.fechar()


def main():
    proc = subir(BASE, PORTA)
    try:
        a = Conexao(PORTA)
        a.entrar("adm", "senha-do-adm")
        a.ok({"op": "criar_database", "database": "folha"})
        a.ok({"op": "criar_tabela", "database": "folha", "tabela": "salarios",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "quanto", "tipo": "Int8"}],
              "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                           "primario": True}]})

        print("=== quem liga o profiler e le o trafego dos outros ===")
        tenta("leitor", "senha-do-leitor", os.path.join(BASE, "leitor.txt"))
        tenta("curioso", "senha-do-curioso", LOG)
        tenta("adm", "senha-do-adm", os.path.join(BASE, "adm.txt"))
        tenta("", "", os.path.join(BASE, "token.txt"))

        print("\n=== o que o `curioso` chega a ver de um trafego alheio ===")
        c = Conexao(PORTA)
        c.entrar("curioso", "senha-do-curioso")
        ligou = c.fala({"op": "profiler_ligar", "arquivo": LOG,
                        "guardar": 100})
        if ligou.get("ok"):
            a.ok({"op": "inserir", "database": "folha", "tabela": "salarios",
                  "linha": {"id": 1, "quanto": 987654}})
            r = c.ok({"op": "profiler", "max": 10})
            for e in r["eventos"]:
                print("   %s %-12s %-16s %s"
                      % (e["usuario"], e["op"],
                         e["database"] + "." + e["tabela"], e["pedido"][:90]))
            c.fala({"op": "profiler_desligar"})
            print("   arquivo escrito pelo curioso: %s (%d B)"
                  % (LOG, os.path.getsize(LOG) if os.path.exists(LOG) else -1))
        c.fechar()

        print("\n=== e o `curioso` PODE ler a tabela folha.salarios direto? ===")
        c = Conexao(PORTA)
        c.entrar("curioso", "senha-do-curioso")
        r = c.fala({"op": "ler", "database": "folha", "tabela": "salarios",
                    "rowid": 1})
        print("   ler folha.salarios -> %s"
              % ("pode" if r.get("ok") else "negado: %s" % r["erro"]))
        c.fechar()
        a.fechar()
    finally:
        baixar(proc)


if __name__ == "__main__":
    main()
