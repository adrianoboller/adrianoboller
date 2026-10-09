#!/usr/bin/env python3
"""A conferencia dos pacotes do PhxZip fica VERMELHA quando deve -- pedido 455,
fatia Z12.

    ./empacotar-phxzip.sh                     # monta em pacotes/phxzip/
    python3 bancada/pacote/provar-phxzip.py

Trabalha numa COPIA de `pacotes/phxzip/` (num temporario, apontado ao
empacotador por `PHXZIP_SAIDA`) e nunca toca nos pacotes de verdade. Cinco
corridas do `./empacotar-phxzip.sh conferir`:

1. a copia intacta CONFERE -- sem isto, as quatro de baixo passariam com um
   conferidor que reprovasse tudo;
2. UM BYTE TROCADO no `phxzipcmd` de dentro de um zip, com o SHA256SUMS de
   fora refeito sobre o zip adulterado (para que so o manifesto de dentro
   possa acusar) -> vermelho, com `DIFERE  phxzipcmd`;
3. UM BYTE TROCADO no proprio zip, sem refazer nada -> vermelho, pelo
   SHA256SUMS de fora;
4. um arquivo A MAIS dentro do zip -> vermelho, com `A MAIS`;
5. o binario de Linux dentro do zip de ARM64, com manifesto e SHA256SUMS
   refeitos pela receita de verdade (`./empacotar.sh manifesto`) -- o hash
   bate, e so a FORMA pega -> vermelho, com `ERRADO`.

O ponto 1 e o que impede a prova de passar por engano; os pontos 2 a 5 sao o
RED: tirar a conferencia correspondente do empacotador deixa a corrida verde,
e esta prova reprova.
"""
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
PACOTES = RAIZ / "pacotes" / "phxzip"
falhas = []


def confere(rotulo, cond, detalhe=""):
    print(("  ok    " if cond else "  FALHA ") + rotulo
          + (f"  -> {detalhe}" if not cond and detalhe else ""))
    if not cond:
        falhas.append(rotulo)


def conferir(saida: Path):
    env = dict(os.environ, PHXZIP_SAIDA=str(saida), CARGO_INCREMENTAL="0")
    p = subprocess.run([str(RAIZ / "empacotar-phxzip.sh"), "conferir"], cwd=RAIZ,
                       env=env, capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def refaz_sha256sums(saida: Path):
    linhas = []
    for z in sorted(saida.glob("*.zip")):
        linhas.append(f"{hashlib.sha256(z.read_bytes()).hexdigest()}  {z.name}\n")
    (saida / "SHA256SUMS").write_text("".join(linhas))


def abre(z: Path, onde: Path) -> Path:
    with zipfile.ZipFile(z) as f:
        f.extractall(onde)
    return onde / z.stem


def fecha(pasta: Path, z: Path):
    z.unlink()
    subprocess.run(["zip", "-qr", str(z), pasta.name], cwd=pasta.parent, check=True)


def troca_um_byte(arq: Path):
    dados = bytearray(arq.read_bytes())
    meio = len(dados) // 2
    dados[meio] ^= 0x01
    arq.write_bytes(bytes(dados))


def copia(base: Path, nome: str) -> Path:
    destino = base / nome
    shutil.copytree(PACOTES, destino)
    return destino


def main():
    zips = sorted(PACOTES.glob("phxzip-*-linux.zip"))
    arm = sorted(PACOTES.glob("phxzip-*-arm64.zip"))
    if not zips or not arm or not (PACOTES / "SHA256SUMS").is_file():
        sys.exit(f"faltam os pacotes em {PACOTES}: ./empacotar-phxzip.sh")
    linux, arm64 = zips[-1].name, arm[-1].name

    with tempfile.TemporaryDirectory(prefix="provar-phxzip-") as tmp:
        base = Path(tmp)

        print("1. a copia intacta")
        rc, saida = conferir(copia(base, "intacta"))
        confere("confere (saida 0)", rc == 0, saida[-600:])
        confere("diz INTEGRO", "INTEGRO" in saida)

        print("2. um byte trocado no phxzipcmd, SHA256SUMS refeito")
        s = copia(base, "byte-dentro")
        pasta = abre(s / linux, base / "abre-2")
        troca_um_byte(pasta / "phxzipcmd")
        fecha(pasta, s / linux)
        refaz_sha256sums(s)
        rc, saida = conferir(s)
        confere("reprova (saida != 0)", rc != 0)
        confere("acusa DIFERE  phxzipcmd", "DIFERE  phxzipcmd" in saida, saida[-600:])

        print("3. um byte trocado no proprio zip")
        s = copia(base, "byte-fora")
        troca_um_byte(s / linux)
        rc, saida = conferir(s)
        confere("reprova (saida != 0)", rc != 0)
        confere("o SHA256SUMS acusa FAILED", "FAILED" in saida, saida[-600:])

        print("4. um arquivo a mais dentro do zip")
        s = copia(base, "a-mais")
        pasta = abre(s / linux, base / "abre-4")
        (pasta / "leia-tambem.exe").write_bytes(b"MZ nada aqui")
        fecha(pasta, s / linux)
        refaz_sha256sums(s)
        rc, saida = conferir(s)
        confere("reprova (saida != 0)", rc != 0)
        confere("acusa A MAIS", "A MAIS" in saida, saida[-600:])

        print("5. o binario de Linux no zip de ARM64, manifesto refeito")
        s = copia(base, "forma")
        de_linux = abre(s / linux, base / "abre-5a")
        pasta = abre(s / arm64, base / "abre-5b")
        shutil.copy2(de_linux / "phxzipcmd", pasta / "phxzipcmd")
        subprocess.run([str(RAIZ / "empacotar.sh"), "manifesto", str(pasta)],
                       cwd=RAIZ, check=True, capture_output=True)
        fecha(pasta, s / arm64)
        refaz_sha256sums(s)
        rc, saida = conferir(s)
        confere("reprova (saida != 0)", rc != 0)
        confere("acusa a forma ERRADO", "ERRADO  phxzipcmd" in saida, saida[-600:])

    print()
    if falhas:
        print(f"{len(falhas)} FALHA(S): " + "; ".join(falhas))
        sys.exit(1)
    print("a conferencia do PhxZip reprova byte trocado (dentro e fora), arquivo a "
          "mais e binario da plataforma errada, e aprova o pacote intacto.")


if __name__ == "__main__":
    main()
