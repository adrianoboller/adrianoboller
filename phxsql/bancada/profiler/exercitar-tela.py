#!/usr/bin/env python3
"""Sobe o servidor com a interface web ligada e roda o exercício da tela.

    cargo build --release -p phxsql-server --bin phxsqld   # a UI e include_str!
    python3 bancada/profiler/exercitar-tela.py [pasta-das-capturas]

A interface entra no binário por `include_str!`: mexer no `ui/index.html` e
esquecer de recompilar exercita a tela de ontem. Por isso este script diz, na
primeira linha, a hora do binário que subiu.

Portas 6280 (dados) e 6281 (web). Mata só o PID que subiu.
"""
import os
import subprocess
import sys
import time

from comum import AQUI, PHXSQLD, TOKEN, config_padrao, baixar, subir

BASE = os.path.join(AQUI, "srv-tela")
CHEIO = os.path.join(AQUI, "cheio")
PORTA = 6280
SAIDA = sys.argv[1] if len(sys.argv) > 1 else os.path.join(AQUI, "tiros")


def main():
    print("binario: %s (%s)" % (
        PHXSQLD, time.strftime("%Y-%m-%d %H:%M:%S",
                               time.localtime(os.path.getmtime(PHXSQLD)))))
    os.makedirs(CHEIO, exist_ok=True)
    tem_cheio = os.path.ismount(CHEIO) or subprocess.run(
        ["mount", "-t", "tmpfs", "-o", "size=64k", "tmpfs", CHEIO],
        capture_output=True).returncode == 0
    alvo_cheio = os.path.join(CHEIO, "tela.txt") if tem_cheio else ""
    if alvo_cheio and os.path.exists(alvo_cheio):
        os.remove(alvo_cheio)
    if not tem_cheio:
        print("aviso: sem tmpfs (precisa de root) -- o caso do disco cheio "
              "sera pulado")

    p = subir(BASE, PORTA, config=config_padrao(PORTA, web=True))
    try:
        r = subprocess.run(
            ["node", os.path.join(AQUI, "exercicio-tela.mjs"),
             str(PORTA + 1), TOKEN, SAIDA,
             os.path.join(BASE, "tela.txt"), alvo_cheio])
        return r.returncode
    finally:
        baixar(p)
        if tem_cheio:
            subprocess.run(["umount", CHEIO], capture_output=True)


if __name__ == "__main__":
    raise SystemExit(main())
