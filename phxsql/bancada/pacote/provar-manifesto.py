#!/usr/bin/env python3
"""As duas ferramentas que conferem um pacote tem de CONCORDAR.

# Por que ela existe

Um pacote traz duas maneiras de se conferir, e o COMECE-AQUI oferece as duas:

    phxsql conferir-pacote <dir>       o binario que viaja dentro do pacote
    sha256sum -c MANIFESTO.sha256      para quem nao quer rodar o binario

Em 30/08/2026 elas passaram a discordar, e ninguem viu por cinco dias. A receita
que grava o manifesto existia em QUATRO lugares do `empacotar.sh`; um deles
tirava o `./` que o `find .` poe na frente do caminho, e os outros tres nao. O
`sha256sum -c` ACEITA o `./` e passava verde; o `conferir-pacote` caminha a
arvore e monta o caminho sem prefixo, entao para ele cada arquivo virava DUAS
divergencias -- uma "a mais" e uma "faltando". Tres pacotes intactos
(dossie, conhecimento, kit) reprovavam por diferenca de grafia do caminho.

Isso e a petrea do IRMAO em forma de shell: *o conserto entrou no caminho que o
motivou e os tres caminhos irmaos ficaram*. E o motivo de ninguem ver foi a
segunda ferramenta passando verde -- **conferidor que discorda de conferidor
nao acusa, acalma**.

# O que ela prova, nos dois sentidos

1. a receita de HOJE (`./empacotar.sh manifesto`) sai INTEGRA no conferidor;
2. a receita ANTIGA, reposta aqui de proposito, REPROVA -- e reprova com
   exatamente 2 divergencias por arquivo, que e a assinatura do defeito;
3. um byte trocado depois do manifesto REPROVA -- o conferidor serve para isso;
4. a receita mora num lugar so no `empacotar.sh`.

O ponto 2 e o que impede esta prova de passar por engano: sem ele, um
conferidor que dissesse INTEGRO para tudo passaria verde.

# Como ela roda

    python3 bancada/pacote/provar-manifesto.py

Nao sobe servidor, nao usa porta e nao toca em `pacotes/`. Trabalha num
diretorio temporario proprio e o apaga no fim.
"""

import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

RAIZ = pathlib.Path(__file__).resolve().parents[2]
EMPACOTAR = RAIZ / "empacotar.sh"
CONFERIDOR = RAIZ / "target" / "release" / "phxsql"

falhas = []


def confere(rotulo, cond, detalhe=""):
    print(("  ok    " if cond else "  FALHA ") + rotulo
          + (f"  -> {detalhe}" if not cond and detalhe else ""))
    if not cond:
        falhas.append(rotulo)


def povoar(dir_):
    """Um pacote de mentira com o que quebrou de verdade: SUBDIRETORIO.

    O defeito do `./` aparecia igual na raiz e no subdiretorio, mas e o
    subdiretorio que distingue "caminho relativo" de "nome do arquivo" -- e
    era ai que uma terceira grafia poderia se esconder.
    """
    (dir_ / "LEIA-ME.md").write_text("pacote de mentira, so para a prova\n")
    (dir_ / "CHANGELOG.md").write_text("0.0.0 -- nada\n")
    (dir_ / "geradores").mkdir()
    (dir_ / "geradores" / "conta.py").write_text("print(1)\n")
    (dir_ / "geradores" / "LEIA-ME.md").write_text("os geradores\n")
    (dir_ / "com espaco.txt").write_text("nome com espaco tambem viaja\n")
    return 5


def manifesto_de_hoje(dir_):
    subprocess.run([str(EMPACOTAR), "manifesto", str(dir_)],
                   check=True, capture_output=True, text=True, cwd=RAIZ)


def manifesto_do_defeito(dir_):
    """A receita ANTIGA, reposta ao pe da letra: `find . -print0 | xargs`.

    Ela grava `./LEIA-ME.md` onde a de hoje grava `LEIA-ME.md`.
    """
    subprocess.run(
        'find . -type f ! -name MANIFESTO.sha256 -print0 '
        '| sort -z | xargs -0 sha256sum > MANIFESTO.sha256',
        shell=True, check=True, cwd=dir_)


def conferir(dir_):
    r = subprocess.run([str(CONFERIDOR), "conferir-pacote", str(dir_)],
                       capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def main():
    if not CONFERIDOR.exists():
        print(f"binario ausente: {CONFERIDOR}\n"
              "  cargo build --release --offline -p phxsql-cli --bin phxsql")
        return 2

    print("== as duas ferramentas de conferencia concordam? ==")
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="phx-manifesto-"))
    try:
        dir_ = tmp / "pacote-de-mentira"
        dir_.mkdir()
        n = povoar(dir_)

        # 1. a receita de hoje
        manifesto_de_hoje(dir_)
        linhas = (dir_ / "MANIFESTO.sha256").read_text().splitlines()
        confere("a receita de hoje lista os %d arquivos" % n, len(linhas) == n,
                f"listou {len(linhas)}")
        confere("nenhum caminho do manifesto comeca com ./",
                not any(l.split("  ", 1)[1].startswith("./") for l in linhas),
                linhas[0] if linhas else "")
        cod, saida = conferir(dir_)
        confere("conferir-pacote diz INTEGRO", cod == 0 and "INTEGRO" in saida,
                saida.strip().splitlines()[-1] if saida.strip() else "")
        r = subprocess.run("sha256sum -c MANIFESTO.sha256", shell=True,
                           cwd=dir_, capture_output=True, text=True)
        confere("sha256sum -c concorda com ele", r.returncode == 0,
                r.stdout + r.stderr)

        # 2. o defeito reposto -- e a assinatura dele
        manifesto_do_defeito(dir_)
        cod, saida = conferir(dir_)
        m = re.search(r"(\d+) DIVERGENCIA", saida)
        confere("a receita ANTIGA reprova no conferir-pacote", cod != 0,
                saida.strip().splitlines()[-1] if saida.strip() else "")
        confere("e reprova com 2 divergencias por arquivo (%d)" % (2 * n),
                m is not None and int(m.group(1)) == 2 * n,
                m.group(1) if m else "nao achei o numero")
        r = subprocess.run("sha256sum -c MANIFESTO.sha256", shell=True,
                           cwd=dir_, capture_output=True, text=True)
        confere("e o sha256sum -c passava verde -- por isso ninguem viu",
                r.returncode == 0, r.stdout + r.stderr)

        # 3. byte trocado depois do manifesto
        manifesto_de_hoje(dir_)
        (dir_ / "geradores" / "conta.py").write_text("print(2)\n")
        cod, saida = conferir(dir_)
        confere("byte trocado depois do manifesto REPROVA",
                cod != 0 and "geradores/conta.py" in saida,
                saida.strip().splitlines()[-1] if saida.strip() else "")

        # 4. a receita mora num lugar so
        fonte = EMPACOTAR.read_text().splitlines()
        ini = next(i for i, l in enumerate(fonte) if l.startswith("manifesto() {"))
        fim = next(i for i in range(ini, len(fonte)) if fonte[i] == "}")
        gravam = [i for i, l in enumerate(fonte)
                  if re.search(r'>>?\s*"\$MANIFESTO"', l)]
        confere("so o manifesto() grava o MANIFESTO.sha256",
                gravam and all(ini <= i <= fim for i in gravam),
                "linhas fora do manifesto(): "
                + ", ".join(str(i + 1) for i in gravam if not ini <= i <= fim))
        confere("a receita antiga nao voltou ao empacotar.sh",
                "xargs -0 sha256sum" not in EMPACOTAR.read_text())
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print()
    if falhas:
        print("REPROVOU em %d: %s" % (len(falhas), "; ".join(falhas)))
        return 1
    print("as duas ferramentas concordam, e o defeito reposto reprova")
    return 0


if __name__ == "__main__":
    sys.exit(main())
