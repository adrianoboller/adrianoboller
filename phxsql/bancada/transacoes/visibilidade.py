#!/usr/bin/env python3
"""A transacao enxerga o que ela mesma escreveu?

    python3 bancada/transacoes/visibilidade.py

Achado 2 de uma auditoria externa da 0.18.0. Mede, nao le: visibilidade e
COMPORTAMENTO, e a doc do proprio codigo ja avisava o desenho -- «o que uma
transacao aberta EMPILHA em vez de gravar». Ler isso e formar hipotese; a
hipotese so vira resultado depois de exercitada, e por soquete, que e como a
casa prova o que depende de estado de processo.

O que ela mede, e por que os TRES numeros importam
--------------------------------------------------
Medir so «a linha aparece dentro da transacao?» diria que as transacoes estao
QUEBRADAS. Nao estao -- e a diferenca esta nos outros dois:

  1. dentro da transacao, a propria escrita NAO aparece  -> falta o RYOW
  2. depois do commit, aparece                           -> atomicidade OK
  3. depois do rollback, NAO aparece                     -> descarte OK

Os pontos 2 e 3 sao o que separa «modelo de empilhamento coerente, com
visibilidade por implementar» de «transacao com defeito». O empilhamento
entrega o A do ACID; o que falta e o I -- e falta por CONSTRUCAO, porque a
escrita fica fora da tabela ate o commit e a leitura vai na tabela.

Consertar o RYOW nao e corrigir um erro: e fazer o caminho de LEITURA consultar
a pilha pendente antes de responder. E trabalho de projeto, e por isso a
decisao e do dono.

ESTADO EM 02/09: FEITO, pela SP000006. Este script mede 1 -> 2 -> 2 -> 3 -> 2, e
o papel dele inverteu: nasceu para PROVAR a ausencia, e hoje e a GUARDA contra a
volta dela. A sobreposicao mora no `store::table` e o servidor a preenche no
`abrir_travada`; o desenho esta na secao 4.4.1 do `docs/TRANSACOES.md`.

Prova real, nos dois sentidos
-----------------------------
Se alguem implementar o RYOW, a primeira contagem passa de 1 para 2 e este
script diz que a auditoria caducou. Se alguem quebrar o commit ou o rollback,
os outros dois numeros mudam e ele nomeia qual. Nenhum dos tres passa por
vacuidade: a tabela nasce com uma linha antes de qualquer transacao abrir.
"""
import importlib.util
import os
import shutil
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
os.environ.setdefault("PORTA", "7496")
os.environ.setdefault("PHX_TRABALHO", f"/tmp/phx-visib-{os.getpid()}")

# Reaproveita o `Phxsqld` e o `Cliente` da prova do PostgreSQL: subir servidor
# e falar o protocolo ja estava escrito, e copiar isso seria a terceira copia.
_p = RAIZ / "bancada/dblink/prova-postgres.py"
spec = importlib.util.spec_from_file_location("pp", _p)
pp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pp)  # nao roda principal(): esta sob __main__


def principal():
    if not pp.PHXSQLD.exists():
        print(f"falta {pp.PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    srv = pp.Phxsqld()
    falhas = []
    try:
        c = pp.Cliente()
        c.call({"op": "login", "usuario": "root", "senha": pp.SENHA})
        c.call({"op": "criar_database", "database": "t"})
        c.call({"op": "criar_tabela", "database": "t", "tabela": "c",
                "colunas": [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                            {"nome": "nome", "tipo": "Str(20)"}],
                "indices": [{"nome": "porId", "colunas": ["id"],
                             "unico": True, "primario": True}]})

        def conta(rotulo):
            r = c.call({"op": "varrer", "database": "t", "tabela": "c",
                        "limite": 100}, exigir=False)
            if not r.get("ok"):
                return f"ERRO {r.get('erro')}"
            n = len(r["resultado"]["linhas"])
            print(f"  {rotulo:42s} {n}")
            return n

        def afirma(rotulo, condicao, visto):
            print(f"  {'ok  ' if condicao else 'ERRO'} {rotulo}: {visto}")
            if not condicao:
                falhas.append(rotulo)

        print("=== a transacao enxerga a propria escrita? ===\n")
        print("-- 1. fora de transacao, para a tabela nao comecar vazia")
        c.call({"op": "inserir", "database": "t", "tabela": "c",
                "linha": {"nome": "antes"}})
        base = conta("linhas antes de abrir")

        print("\n-- 2. dentro da transacao")
        c.call({"op": "begin"})
        c.call({"op": "inserir", "database": "t", "tabela": "c",
                "linha": {"nome": "dentro"}})
        dentro = conta("a MESMA sessao ve a propria escrita?")
        afirma("read-your-own-writes", dentro == base + 1,
               "SIM" if dentro == base + 1 else "NAO -- a escrita fica empilhada")

        print("\n-- 3. o commit aplica (atomicidade)")
        c.call({"op": "commit"})
        depois = conta("linhas depois do commit")
        afirma("o commit aplicou o que estava empilhado", depois == base + 1, depois)

        print("\n-- 4. o rollback descarta")
        c.call({"op": "begin"})
        c.call({"op": "inserir", "database": "t", "tabela": "c",
                "linha": {"nome": "descartada"}})
        conta("dentro da 2a transacao")
        c.call({"op": "rollback"})
        rb = conta("depois do rollback")
        afirma("o rollback descartou", rb == depois, rb)

        print()
        if dentro == base + 1:
            print("O read-your-own-writes EXISTE -- o achado da auditoria caducou.")
        else:
            print("NAO ha read-your-own-writes: a escrita so aparece no commit.")
            print("O commit e o rollback estao corretos, entao o modelo de")
            print("EMPILHAMENTO e coerente -- falta a visibilidade, nao a transacao.")
        if falhas:
            print(f"\n== ATENCAO em {len(falhas)}: " + "; ".join(falhas))
            print("   (o RYOW existe desde a SP000006 -- se ele sumiu, e regressao)")
        return 0
    finally:
        srv.parar()
        shutil.rmtree(pp.TRABALHO, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(principal())
