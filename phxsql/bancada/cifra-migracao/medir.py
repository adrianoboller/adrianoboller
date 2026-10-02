#!/usr/bin/env python3
"""O custo de Criptografar/Descriptografar (pedido 268): tempo por slot, FASE A
e FASE B, e o `fsync` -- com N e a faixa min-max.

    cargo build --release -p phxsql-store --example custo-da-migracao-da-cifra
    python3 bancada/cifra-migracao/medir.py [--rapido]

O parecer do papel C estimou 1,3-1,6 us/slot compondo numeros de outros dias e
disse que o `fsync` NAO foi medido. Aqui os dois saem de uma corrida so, do
motor de verdade (`--example custo-da-migracao-da-cifra`), e cada numero traz
quantas vezes foi medido e a faixa -- esta casa ja declarou vencedor dentro do
ruido (pedido 155).

# Como o `fsync` e contado

O motor nao tem contador de `fsync`; quem conta e o `strace`
(`-f -e trace=fsync,fdatasync`), por DIFERENCA entre tres corridas do mesmo
cenario: so o preparo, preparo + Criptografar, preparo + Criptografar +
Descriptografar. A diferenca isola a migracao do `fsync` que o preparo (a
carga e o `sincronizar`) ja paga. O numero nao depende de N -- depende de
quantos VOLUMES a tabela tem --, por isso e contado em duas formas: um volume
e dez.

# O que ele NAO mede

Nao mede o espelho (`.bkp`), que dobra a escrita da FASE A sem mudar a conta
por slot, nem o servidor (o roteiro de trava e congelamento) -- so o motor.
O PBKDF2 do sal novo (~130-300 ms, uma vez por Criptografar) sai separado e
e descontado do `us_por_slot` da FASE A de Criptografar; em N pequeno esse
desconto e ruido, e por isso a conta por slot so vale de 100.000 linhas em
diante.

Grava `resultados.json` ao lado.
"""
import datetime
import json
import os
import re
import statistics
import subprocess
import sys
import tempfile

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
EXE = os.path.join(RAIZ, "target", "release", "examples", "custo-da-migracao-da-cifra")

# (linhas, linhas por volume ou None, repeticoes)
GRADE = [
    (10_000, None, 5),
    (100_000, None, 5),
    (100_000, 10_000, 5),
    (1_000_000, None, 3),
]
GRADE_RAPIDA = [(20_000, None, 3), (20_000, 2_000, 3)]

LINHA = re.compile(
    r"^(cifrar|decifrar): slots=(\d+) fase_a_ms=([\d.]+) fase_b_ms=([\d.]+) "
    r"us_por_slot=([\d.]+) volumes=(\d+) bytes_depois=(\d+)"
)


def rodar(n, por_volume, modo, sob_strace=None):
    cmd = [EXE, str(n), "--modo", modo]
    if por_volume:
        cmd += ["--por-volume", str(por_volume)]
    if sob_strace:
        cmd = ["strace", "-f", "-qq", "-e", "trace=fsync,fdatasync", "-o", sob_strace] + cmd
    p = subprocess.run(cmd, capture_output=True, text=True)
    if p.returncode != 0:
        raise SystemExit(f"falhou: {' '.join(cmd)}\n{p.stdout}\n{p.stderr}")
    return p.stdout


def faixa(valores):
    return {
        "min": round(min(valores), 3),
        "mediana": round(statistics.median(valores), 3),
        "max": round(max(valores), 3),
    }


def fsyncs(n, por_volume):
    """Quantos fsync/fdatasync cada migracao paga, por diferenca."""
    contagens = {}
    for modo in ("preparo", "cifrar", "decifrar"):
        with tempfile.NamedTemporaryFile("r", suffix=".strace") as f:
            rodar(n, por_volume, modo, sob_strace=f.name)
            contagens[modo] = sum(1 for _ in open(f.name) if "sync(" in _)
    return {
        "preparo": contagens["preparo"],
        "criptografar": contagens["cifrar"] - contagens["preparo"],
        "descriptografar": contagens["decifrar"] - contagens["cifrar"],
    }


def medir(n, por_volume, repeticoes):
    por = {"cifrar": [], "decifrar": []}
    for _ in range(repeticoes):
        for linha in rodar(n, por_volume, "decifrar").splitlines():
            m = LINHA.match(linha)
            if m:
                por[m.group(1)].append(
                    {
                        "slots": int(m.group(2)),
                        "fase_a_ms": float(m.group(3)),
                        "fase_b_ms": float(m.group(4)),
                        "us_por_slot": float(m.group(5)),
                        "volumes": int(m.group(6)),
                        "bytes_depois": int(m.group(7)),
                    }
                )
    saida = {"linhas": n, "por_volume": por_volume, "repeticoes": repeticoes}
    for sentido, medidas in por.items():
        assert len(medidas) == repeticoes, (sentido, len(medidas))
        saida[sentido] = {
            "slots": medidas[0]["slots"],
            "volumes": medidas[0]["volumes"],
            "fase_a_ms": faixa([x["fase_a_ms"] for x in medidas]),
            "fase_b_ms": faixa([x["fase_b_ms"] for x in medidas]),
            "us_por_slot": faixa([x["us_por_slot"] for x in medidas]),
            "bytes_depois": medidas[0]["bytes_depois"],
        }
    return saida


def main():
    if not os.path.exists(EXE):
        raise SystemExit(
            "falta o medidor: cargo build --release -p phxsql-store "
            "--example custo-da-migracao-da-cifra  (binario velho mede o passado)"
        )
    medindo = subprocess.run([os.path.join(RAIZ, "bancada", "esta-medindo.sh")],
                             capture_output=True, text=True)
    grade = GRADE_RAPIDA if "--rapido" in sys.argv else GRADE
    resultado = {
        "quando": datetime.datetime.now().isoformat(timespec="seconds"),
        "esta_medindo_antes": medindo.returncode == 0,
        "esta_medindo_quem_antes": medindo.stdout.strip(),
        "modo": "rapido" if "--rapido" in sys.argv else "completo",
        "medidas": [],
        "fsync": {},
    }
    for n, por_volume, rep in grade:
        m = medir(n, por_volume, rep)
        resultado["medidas"].append(m)
        c, d = m["cifrar"], m["decifrar"]
        print(
            f"N={n:>9,} volumes={c['volumes']:>3}  "
            f"criptografar {c['us_por_slot']['mediana']:.3f} us/slot "
            f"[{c['us_por_slot']['min']:.3f}-{c['us_por_slot']['max']:.3f}]  "
            f"descriptografar {d['us_por_slot']['mediana']:.3f} us/slot "
            f"[{d['us_por_slot']['min']:.3f}-{d['us_por_slot']['max']:.3f}]  "
            f"FASE B {c['fase_b_ms']['mediana']:.1f} ms  (x{rep})"
        )
    n_fsync = 20_000 if "--rapido" in sys.argv else 100_000
    for por_volume in (None, n_fsync // 10):
        chave = f"{n_fsync}_linhas_{'1_volume' if por_volume is None else '10_volumes'}"
        resultado["fsync"][chave] = fsyncs(n_fsync, por_volume)
        print(f"fsync {chave}: {resultado['fsync'][chave]}")
    caminho = os.path.join(AQUI, "resultados.json")
    with open(caminho, "w") as f:
        json.dump(resultado, f, indent=2)
        f.write("\n")
    print("RESULTADO", caminho)


if __name__ == "__main__":
    main()
