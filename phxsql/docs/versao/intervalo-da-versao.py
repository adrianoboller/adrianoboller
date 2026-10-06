#!/usr/bin/env python3
"""O INTERVALO DA VERSAO -- gera, no CHANGELOG, os numeros do que a versao
corrente ja andou sobre a anterior. Nada deles se digita.

O defeito que o motivou (fecho de 01-02/10/2026): o cabecalho da `## 0.19.0`
dizia «894 commits sobre a 0.18.0» enquanto `git rev-list --count baff46e..HEAD`
dava 1313. O numero era verdadeiro no dia em que foi escrito (23/09) e ficou
dizendo isso por dez dias. O proprio texto avisava que ele «andou tres vezes
enquanto a rodada fechava» -- e mesmo assim foi digitado.

    python3 docs/versao/intervalo-da-versao.py            # imprime o que mediria
    python3 docs/versao/intervalo-da-versao.py --gravar   # regrava o bloco GERADO do CHANGELOG.md

De onde sai cada numero (nenhum e digitado):

* a versao corrente e a anterior: os dois primeiros `## X.Y.Z` do CHANGELOG;
* o commit que selou cada uma: `git log -S 'version = "X.Y.Z"' -- Cargo.toml`
  (o mesmo pente do `portao-da-versao.py`);
* commits sobre a anterior: `git rev-list --count <selo da anterior>..HEAD`;
* commits desde o selo atual: `git rev-list --count <selo atual>..HEAD`;
* frentes `###`: contadas no proprio CHANGELOG, na secao «Nao lancado» e na
  secao da versao corrente.

O bloco leva a data e o HEAD em que foi medido: numero de intervalo tem
validade de minutos numa arvore com frente viva, e o que importa e o do commit
que sela. NAO entra no `PLANO` do `portao-dos-geradores.py` de proposito: o
numero muda a CADA commit, entao nenhuma comparacao «re-rodar muda algo?»
poderia ser verde -- ele se roda no fecho, no commit do selo.

Sai != 0 se nao medir (repositorio raso, marca ausente, selo nao achado).
"""

import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
CHANGELOG = RAIZ / "CHANGELOG.md"
INICIO = "<!-- GERADO: intervalo-da-versao.py -->"
FIM = "<!-- /GERADO: intervalo-da-versao.py -->"


def git(*args):
    r = subprocess.run(["git", *args], cwd=RAIZ, capture_output=True, text=True)
    return r.returncode, r.stdout.strip(), r.stderr.strip()


def selo_de(versao):
    codigo, saida, erro = git("log", "--format=%H", "-S", f'version = "{versao}"', "--", "Cargo.toml")
    linhas = [l for l in saida.splitlines() if l.strip()]
    if codigo != 0 or not linhas:
        sys.exit(f"intervalo-da-versao: nenhum commit introduz version = \"{versao}\" ({erro})")
    # `-S` lista tambem o commit que TIRA a string (o do bump seguinte): a
    # introducao e a mais antiga da lista, porque esta arvore nunca reusou
    # uma string de versao.
    return linhas[-1]


def contar(base):
    codigo, saida, erro = git("rev-list", "--count", f"{base}..HEAD")
    if codigo != 0:
        sys.exit(f"intervalo-da-versao: nao contei {base[:7]}..HEAD ({erro})")
    return int(saida)


def secoes(texto):
    """[(titulo do `##`, [titulos `###` da secao])], na ordem do arquivo."""
    saida = []
    for linha in texto.splitlines():
        if linha.startswith("## "):
            saida.append((linha[3:], []))
        elif linha.startswith("### ") and saida:
            saida[-1][1].append(linha[4:])
    return saida


def medir():
    codigo, raso, _ = git("rev-parse", "--is-shallow-repository")
    if codigo != 0 or raso == "true":
        sys.exit("intervalo-da-versao: repositorio raso ou sem git -- recuso medir")
    texto = CHANGELOG.read_text(encoding="utf-8")
    sec = secoes(texto)
    versoes = [(t, h) for t, h in sec if re.match(r"\d+\.\d+\.\d+", t)]
    if len(versoes) < 2:
        sys.exit("intervalo-da-versao: preciso de duas secoes `## X.Y.Z` no CHANGELOG")
    atual = re.match(r"(\d+\.\d+\.\d+)", versoes[0][0]).group(1)
    anterior = re.match(r"(\d+\.\d+\.\d+)", versoes[1][0]).group(1)
    nao_lancado = [h for t, h in sec if t.startswith("Não lançado")]
    selo_ant, selo_atual = selo_de(anterior), selo_de(atual)
    _, head, _ = git("rev-parse", "--short", "HEAD")
    return {
        "atual": atual, "anterior": anterior,
        "selo_ant": selo_ant[:7], "selo_atual": selo_atual[:7],
        "sobre_anterior": contar(selo_ant), "desde_selo": contar(selo_atual),
        "frentes_nao_lancado": sum(len(h) for h in nao_lancado),
        "frentes_versao": len(versoes[0][1]),
        "head": head,
        "quando": datetime.now(timezone.utc).strftime("%d/%m/%Y %H:%M UTC"),
    }


def bloco(m):
    return (
        f"{INICIO}\n"
        f"**{m['sobre_anterior']} commits** sobre a {m['anterior']} "
        f"(`git rev-list --count {m['selo_ant']}..HEAD`, medido em {m['quando']} "
        f"no commit `{m['head']}`; o número muda a cada commit e vale o do que sela). "
        f"Desde o commit que pôs `version = \"{m['atual']}\"` no `Cargo.toml` "
        f"(`{m['selo_atual']}`) andaram **{m['desde_selo']}**, e são eles que "
        f"estão em «Não lançado» acima da seção abaixo: **{m['frentes_nao_lancado']}** "
        f"títulos `###` ali e **{m['frentes_versao']}** na seção da {m['atual']}. "
        f"Gerado por `docs/versao/intervalo-da-versao.py`.\n"
        f"{FIM}"
    )


def main():
    m = medir()
    novo = bloco(m)
    if "--gravar" not in sys.argv:
        print(novo)
        return 0
    texto = CHANGELOG.read_text(encoding="utf-8")
    if INICIO not in texto or FIM not in texto:
        sys.exit(f"intervalo-da-versao: faltam as marcas {INICIO} / {FIM} no CHANGELOG.md")
    a = texto.index(INICIO)
    b = texto.index(FIM) + len(FIM)
    CHANGELOG.write_text(texto[:a] + novo + texto[b:], encoding="utf-8")
    print(f"gravado: {m['sobre_anterior']} commits sobre a {m['anterior']}, "
          f"{m['desde_selo']} desde o selo {m['selo_atual']}, "
          f"{m['frentes_nao_lancado']} frentes em Não lançado, {m['frentes_versao']} na {m['atual']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
