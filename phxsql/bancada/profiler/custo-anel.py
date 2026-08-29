#!/usr/bin/env python3
"""O anel do Profiler LIGADO: procurar o desfecho do mais novo ou do mais velho.

    python3 bancada/profiler/custo-anel.py

`terminou` acha o evento pelo serial para costurar nele a duração e o
desfecho. Ele procurava do MAIS ANTIGO para o mais novo — e o evento
procurado é quase sempre o **último** que entrou, porque `chegou` empurra
atrás e o desfecho chega logo depois. Com o anel cheio isso é varrer o anel
inteiro a cada pedido.

Este medidor separa as duas coisas que se confundiam no número: o **tamanho do
anel** (que o operador escolhe na tela, padrão 500) e o **sentido da busca**.
A comparação é emparelhada, pelo mesmo motivo do `custo.py`.

Nunca usa pkill: mata só os PIDs que subiu.
"""
import io
import json
import os
import shutil
import subprocess
import sys

from comum import PHXSQLD, RAIZ
from custo import BIN, comparar

PROF = os.path.join(RAIZ, "crates", "phxsql-server", "src", "profiler.rs")
NOVO = "self.anel.iter_mut().rev().find(|e| e.serial == serial)"
VELHO = "self.anel.iter_mut().find(|e| e.serial == serial)"


def compilar_sem_rev():
    original = io.open(PROF, encoding="utf-8").read()
    assert original.count(NOVO) == 1, "a busca invertida não está no código"
    os.makedirs(BIN, exist_ok=True)
    try:
        io.open(PROF, "w", encoding="utf-8").write(
            original.replace(NOVO, VELHO))
        print("  compilando a variante SEM a busca invertida ...", flush=True)
        r = subprocess.run(["cargo", "build", "--release", "--offline",
                            "-p", "phxsql-server", "--bin", "phxsqld"],
                           cwd=RAIZ, capture_output=True, text=True)
        if r.returncode:
            sys.exit(r.stdout + r.stderr)
        shutil.copy2(PHXSQLD, os.path.join(BIN, "sem-rev"))
    finally:
        io.open(PROF, "w", encoding="utf-8").write(original)
    print("  compilando a variante de agora ...", flush=True)
    subprocess.run(["cargo", "build", "--release", "--offline",
                    "-p", "phxsql-server", "--bin", "phxsqld"],
                   cwd=RAIZ, capture_output=True, text=True)
    shutil.copy2(PHXSQLD, os.path.join(BIN, "com-rev"))


def main():
    if "--sem-compilar" not in sys.argv:
        compilar_sem_rev()
    com = ("com rev", os.path.join(BIN, "com-rev"))
    sem = ("sem rev", os.path.join(BIN, "sem-rev"))
    tudo = {}
    for guardar in (500, 20_000):
        tudo["anel %d" % guardar] = comparar(
            "busca invertida, anel de %d (>1,00 = a invertida ganha)" % guardar,
            com, sem, guardar_a=guardar, guardar_b=guardar)
    json.dump(tudo, open(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                      "custo-anel.json"), "w"), indent=1)
    print("\nRESULTADO " + json.dumps(tudo))


if __name__ == "__main__":
    main()
