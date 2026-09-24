#!/usr/bin/env python3
"""Prova real do `portoes.sh` (pedido 421), nos dois sentidos.

O cenario e o de 23/09/2026: suite inteira verde e uma catraca em Python
reprovada. O portao unico tem de sair VERMELHO nesse caso; e o mesmo portao
com o defeito reposto (sem o passo das catracas) tem de sair verde nele --
senao esta prova passaria por engano.

Roda numa arvore exata tirada do `git archive HEAD`, com o `portoes.sh` vivo
copiado por cima, e com um cargo FALSO (`PHX_CARGO`): o que se prova aqui e a
costura dos passos, nao a suite -- compilar o workspace quatro vezes para
isso custaria minutos e nao provaria nada a mais.
"""
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent                      # bancada/catracas -> phxsql
PORTOES = RAIZ / "portoes.sh"

CARGO_FALSO = """#!/bin/sh
# cargo de mentira: `test` sai com FALSO_TESTE (padrao 0); o resto sai 0.
if [ "$1" = test ]; then exit "${FALSO_TESTE:-0}"; fi
exit 0
"""

# O modo montado em dois pedacos, e nunca na mesma linha que o `sys.argv`: o
# `todas.py` acha catraca por uma linha que junte os dois, e achou ESTA prova
# no dia em que ela entrou no HEAD -- rodou-a como catraca, a prova montou a
# arvore do HEAD, que a continha, e a rodou de novo la dentro. Na arvore exata
# de antes do commit isso nao aparecia, porque o HEAD ainda nao tinha a prova.
MODO = "--" + "catraca"

CATRACA_QUE_REPROVA = (
    "import sys\n"
    "# Catraca de mentira desta prova: declara o modo e sempre reprova.\n"
    f"if {MODO!r} in sys.argv:\n"
    '    print("SUBIU 1 (teto 0) -- catraca de mentira da prova do 421")\n'
    "    sys.exit(1)\n"
)


def raiz_do_git() -> pathlib.Path:
    return pathlib.Path(subprocess.run(
        ["git", "rev-parse", "--show-toplevel"], cwd=RAIZ,
        capture_output=True, text=True, check=True).stdout.strip())


def montar_arvore(destino: pathlib.Path) -> pathlib.Path:
    git = str(raiz_do_git())
    rel = str(RAIZ.relative_to(git))
    arq = subprocess.run(["git", "archive", "HEAD", rel], cwd=git,
                         capture_output=True, check=True).stdout
    subprocess.run(["tar", "-x", "-C", str(destino)], input=arq, check=True)
    arvore = destino / rel
    (arvore / "cargo-falso.sh").write_text(CARGO_FALSO)
    (arvore / "cargo-falso.sh").chmod(0o755)
    return arvore


def rodar(script: pathlib.Path, arvore: pathlib.Path, falso_teste="0"):
    # O `portoes.sh` copiado para a arvore temporaria nao acha repositorio
    # nenhum ao lado dele; a arvore saiu DESTE, e e nele que a regua dos
    # aprendizados confere os commits citados como evidencia.
    env = dict(os.environ, PHX_CARGO=str(arvore / "cargo-falso.sh"),
               FALSO_TESTE=falso_teste, PHXSQL_GIT_DIR=str(raiz_do_git()))
    r = subprocess.run(["bash", str(script), "--raiz", str(arvore)],
                       capture_output=True, text=True, env=env)
    return r.returncode, r.stdout + r.stderr


def conferir(script: pathlib.Path, arvore: pathlib.Path) -> list:
    """Os tres casos; devolve as falhas (vazio = o portao se comporta)."""
    falhas = []
    injetada = arvore / "bancada" / "guardas" / "sempre-reprova-421.py"
    injetada.unlink(missing_ok=True)

    rc, saida = rodar(script, arvore)
    if rc != 0:
        falhas.append(f"arvore limpa e suite verde saiu {rc}, esperado 0:\n{saida[-600:]}")

    rc, saida = rodar(script, arvore, falso_teste="101")
    if rc == 0:
        falhas.append("suite VERMELHA (cargo test 101) e o portao saiu 0")

    injetada.write_text(CATRACA_QUE_REPROVA)
    try:
        rc, saida = rodar(script, arvore)
        if rc == 0:
            falhas.append("suite VERDE com catraca REPROVADA e o portao saiu 0 "
                          "-- e o defeito do 421")
        elif "FALHOU catracas" not in saida:
            falhas.append(f"saiu {rc}, mas sem nomear o passo das catracas:\n{saida[-600:]}")
    finally:
        injetada.unlink(missing_ok=True)
    return falhas


def nao_e_catraca() -> list:
    """Esta prova nao pode ser achada como catraca pelo `todas.py`: roda-la
    de dentro dele a faria rodar a si mesma na arvore temporaria."""
    r = subprocess.run([sys.executable, str(AQUI / "todas.py"), "--lista"],
                       capture_output=True, text=True, cwd=RAIZ)
    if "prova-portoes.py" in r.stdout:
        return ["o todas.py acha esta prova como catraca -- ela se rodaria "
                "dentro da propria arvore temporaria"]
    return []


def main() -> int:
    falhas_de_forma = nao_e_catraca()
    for f in falhas_de_forma:
        print("  FALHA ", f)
    if falhas_de_forma:
        return 1
    print("  ok    o todas.py nao acha esta prova como catraca")
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="prova421-"))
    try:
        arvore = montar_arvore(tmp)
        vivo = arvore / "portoes.sh"
        # As tres pecas da costura saem da copia VIVA, e nao do HEAD: o que
        # se prova e o que vai ser comitado, e o HEAD so tem o que ja foi.
        shutil.copy(PORTOES, vivo)
        for rel in ("bancada/catracas/todas.py", "bancada/catracas/prova-portoes.py"):
            shutil.copy(RAIZ / rel, arvore / rel)

        falhas = conferir(vivo, arvore)
        for f in falhas:
            print("  FALHA ", f)
        if not falhas:
            print("  ok    portoes.sh: limpa -> 0; suite vermelha -> 1; "
                  "suite verde + catraca reprovada -> 1, nomeando «catracas»")

        # Defeito reposto: o portao de antes do 421, sem o passo das catracas.
        reposto = arvore / "portoes-sem-catracas.sh"
        texto = vivo.read_text()
        linha = 'passo "catracas" python3 bancada/catracas/todas.py\n'
        if linha not in texto:
            print("  FALHA  nao achei a linha do passo das catracas para repor o defeito")
            return 1
        reposto.write_text(texto.replace(linha, ""))
        falhas_reposto = conferir(reposto, arvore)
        if any("defeito do 421" in f for f in falhas_reposto):
            print("  ok    defeito reposto (sem o passo das catracas): a prova ACUSA "
                  "-- suite verde com catraca reprovada sairia 0")
        else:
            print("  FALHA  com o defeito reposto a prova nao acusou nada -- "
                  "ela passaria por engano")
            falhas.append("prova nao pega o defeito reposto")

        if falhas:
            print("VERMELHO")
            return 1
        print("VERDE: o portao unico reprova a catraca mesmo com a suite verde, "
              "e a prova pega o defeito reposto.")
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
