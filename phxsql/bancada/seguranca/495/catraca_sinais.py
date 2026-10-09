#!/usr/bin/env python3
"""A catraca do corpo legitimo do detector de injecao (pedido 495, fatia F1).

    python3 bancada/seguranca/495/catraca_sinais.py --catraca   extrai, mede, compara ao teto
    python3 bancada/seguranca/495/catraca_sinais.py --numeros   para o docs/qa/medir.py

O aceite da F1 (`docs/propostas/ia-495-496-desenho.md` §7) diz: no maximo 1
comando do corpo legitimo extraido do codigo acusado pelas quatro classes de
`phxsql_sql::sinais`. Este script refaz o corpo pelos dois extratores daqui
(nunca um `.jsonl` velho: o corpo acompanha o repositorio de hoje), roda o
medidor `sinais-no-legitimo` e julga.

O 1 medido em 09/10/2026 e `SELECT * FROM t; SELECT * FROM t2`, do teste
`sobra_depois_do_comando` do `sintaxe.rs`: um segundo comando de verdade, que
o motor recusa -- acusa-lo e o detector certo, e por isso ele fica no teto em
vez de numa lista de excecao.

Igual ao teto passa; acima reprova (o detector passou a acusar o habitual, e
sinal que dispara no habitual ensina a ignorar o sinal); abaixo tambem
reprova, para o teto descer no mesmo commit -- catraca frouxa nao segura nada.
E reprova se algum ataque do corpo de deteccao deixar de ser acusado: um
detector morto mede zero no legitimo, e esse zero passaria por virtude.
"""
import os
import re
import subprocess
import sys

TETO_SINAIS_NO_LEGITIMO = 1

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", "..", ".."))
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)


def medir():
    for extrator in ("extrair_legitimo.py", "extrair_deteccao.py"):
        subprocess.run([sys.executable, os.path.join(AQUI, extrator)], check=True,
                       stdout=subprocess.DEVNULL)
    r = subprocess.run(
        ["cargo", "run", "-q", "--example", "sinais-no-legitimo", "-p", "phxsql-sql", "--",
         os.path.join(AQUI, "legitimo.jsonl"), os.path.join(AQUI, "deteccao.jsonl")],
        cwd=RAIZ, capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stdout + r.stderr, file=sys.stderr)
        raise SystemExit("o medidor sinais-no-legitimo nao rodou")
    acusados = int(re.search(r"^legitimo_acusados=(\d+)$", r.stdout, re.M).group(1))
    m = re.search(r"^ataques_acusados=(\d+) de=(\d+)$", r.stdout, re.M)
    linhas = [l for l in r.stdout.splitlines() if "\t" in l]
    return acusados, int(m.group(1)), int(m.group(2)), linhas


def catraca():
    print("=== a catraca do detector de injecao no corpo legitimo (495 F1) ===")
    acusados, ataques_ok, ataques, linhas = medir()
    for l in linhas:
        print("   " + l)
    print(f"   ataques do corpo de deteccao acusados: {ataques_ok} de {ataques}")
    if ataques == 0 or ataques_ok != ataques:
        print("   Reprovado: o detector deixou de acusar ataque do repositorio; o "
              "numero do legitimo nao vale.")
        return 1
    if acusados > TETO_SINAIS_NO_LEGITIMO:
        print(f"\n   SUBIU  {acusados} (teto {TETO_SINAIS_NO_LEGITIMO})")
        print("   Reprovado: o detector passou a acusar SQL legitimo do repositorio.")
        return 1
    if acusados < TETO_SINAIS_NO_LEGITIMO:
        print(f"\n   DESCEU -- BAIXE O TETO  {acusados} (teto {TETO_SINAIS_NO_LEGITIMO})")
        print("   Ponha o numero novo em TETO_SINAIS_NO_LEGITIMO, no mesmo commit.")
        return 1
    print(f"\n   ok  {acusados} (teto {TETO_SINAIS_NO_LEGITIMO})")
    return 0


def numeros():
    acusados, ataques_ok, ataques, _ = medir()
    if ataques == 0 or ataques_ok != ataques:
        print("o detector deixou de acusar ataque do repositorio", file=sys.stderr)
        return 1
    print(f"catraca:nome=TETO_SINAIS_NO_LEGITIMO;onde={EU};"
          f"valor={TETO_SINAIS_NO_LEGITIMO};medido={acusados};tipo=teto;"
          "mede=comandos SQL legitimos do repositorio acusados por phxsql_sql::sinais")
    return 0


def principal():
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(principal())
