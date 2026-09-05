#!/usr/bin/env python3
"""Quem abre tabela COM sessao e quem abre SEM.

A pergunta nasceu do desenho de senha por banco vinda do login: se a chave
mora na sessao, todo sitio que abre tabela SEM sessao deixa de abrir tabela
cifrada. Este script nao adivinha -- classifica cada chamada de
`abrir_qualificada` pela ASSINATURA da funcao dona.

  python3 quem-abre-tabela-sem-sessao.py
"""
import re
import pathlib
import sys

RAIZ = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "/home/user/adrianoboller/phxsql")
ALVOS = ["crates/phxsql-server/src/servidor.rs",
         "crates/phxsql-server/src/transacao.rs",
         "crates/phxsql-server/src/idiomas.rs"]

com = sem = 0
for rel in ALVOS:
    linhas = (RAIZ / rel).read_text(encoding="utf-8").split("\n")
    # A assinatura pode ocupar varias linhas: junta ate achar o `{` do corpo.
    fns = []
    for i, l in enumerate(linhas):
        m = re.match(r"\s*(pub\s+)?(async\s+)?fn\s+(\w+)", l)
        if not m:
            continue
        ass, j = l, i
        while "{" not in ass and j - i < 12 and j + 1 < len(linhas):
            j += 1
            ass += " " + linhas[j].strip()
        fns.append((i, m.group(3), ass))
    for i, l in enumerate(linhas):
        # Comentario que MENCIONA a funcao nao e chamada dela. Medido: o
        # `op_criar_tabela` entrava na conta por uma linha de comentario, e o
        # numero saiu errado antes de alguem conferir sitio por sitio.
        if l.lstrip().startswith("//") or l.lstrip().startswith("///"):
            continue
        if "abrir_qualificada" not in l or "fn abrir_qualificada" in l:
            continue
        dono = None
        for (li, nome, ass) in fns:
            if li < i:
                dono = (li, nome, ass)
            else:
                break
        if dono is None:
            print(f"?    | {rel}:{i+1} | <sem funcao dona>")
            continue
        li, nome, ass = dono
        tem = "Sessao" in ass
        com, sem = com + bool(tem), sem + (not tem)
        print(f"{'COM ' if tem else 'SEM '} | {rel}:{i+1} | fn {nome} (linha {li+1})")

print(f"\nCOM sessao: {com}   SEM sessao: {sem}   total: {com + sem}")
