#!/usr/bin/env python3
"""Regrava a tabela das guardas no `docs/TESTES.md` com o resultado MEDIDO.

Existe pela mesma lei dos geradores do dossie: numero digitado a mao envelhece
calado. Uma tabela que diz «quinze guardas provadas» e digitada mente no dia em
que a decima sexta entra -- e mente justamente sobre a unica coisa que ela
serve para dizer, que e quais provas ainda pegam o defeito que as motivou.

    python3 bancada/guardas/provar-guardas.py --json /tmp/guardas.json
    python3 bancada/guardas/tabela-no-testes.py /tmp/guardas.json

    python3 bancada/guardas/tabela-no-testes.py /tmp/guardas.json --so-medir

O arquivo de entrada e o `--json` do executor: o veredito nao se digita aqui,
ele vem de uma rodada. Sem rodada nao ha tabela -- e nao ter tabela e melhor
que ter uma que ninguem mediu.
"""

import json
import pathlib
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ALVO = RAIZ / "docs" / "TESTES.md"

INICIO = "<!-- guardas:inicio -->"
FIM = "<!-- guardas:fim -->"

sys.path.insert(0, str(AQUI))
from catalogo import GUARDAS  # noqa: E402

MARCA = {
    "PROVADA": "✅ provada",
    "REDUNDANTE": "🟰 redundante",
    "NAO PEGOU": "❌ **não pegou**",
    "ESTRAGOU": "⚠️ estragou demais",
    "QUEBRADA": "⚠️ quebrada",
}


def tabela(dados):
    por_id = {g["id"]: g for g in GUARDAS}
    linhas = [
        "| guarda | o defeito reposto | testes que caem | veredito |",
        "|---|---|---:|---|",
    ]
    conta = {}
    for r in dados["guardas"]:
        g = por_id.get(r["id"], {})
        conta[r["veredito"]] = conta.get(r["veredito"], 0) + 1
        quantos = len(g.get("caem", []))
        linhas.append(
            "| `%s` | %s | %s | %s |"
            % (r["id"], r.get("titulo", g.get("titulo", "")),
               quantos if quantos else "—",
               MARCA.get(r["veredito"], r["veredito"])))
    total = len(dados["guardas"])
    # O plural sai do numero, e nao de uma segunda tabela escrita a mao: uma
    # guarda «provada» e catorze «provada» e o tipo de erro que ninguem revisa.
    plural = {"PROVADA": ("provada", "provadas"),
              "REDUNDANTE": ("redundante", "redundantes"),
              "NAO PEGOU": ("não pegou", "não pegaram"),
              "ESTRAGOU": ("estragou", "estragaram"),
              "QUEBRADA": ("quebrada", "quebradas")}
    resumo = ", ".join(
        "%d %s" % (n, plural.get(v, (v.lower(), v.lower()))[0 if n == 1 else 1])
        for v, n in sorted(conta.items()))
    segundos = sum(r["segundos"] for r in dados["guardas"])
    linhas += [
        "",
        "**%d guardas: %s** — %d s de mutação, medido em %s."
        % (total, resumo, round(segundos), dados.get("quando", "?")),
    ]
    notas = [(r["id"], n) for r in dados["guardas"] for n in r["notas"]]
    if notas:
        linhas += ["", "As notas que a rodada deixou:", ""]
        for ident, n in notas:
            linhas.append("- `%s` — %s" % (ident, n))
    return "\n".join(linhas)


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    dados = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
    bloco = tabela(dados)
    if "--so-medir" in sys.argv:
        print(bloco)
        return
    txt = ALVO.read_text(encoding="utf-8")
    if INICIO not in txt or FIM not in txt:
        raise SystemExit("%s nao tem as marcas %s / %s" % (ALVO, INICIO, FIM))
    antes = txt.split(INICIO)[0]
    depois = txt.split(FIM)[1]
    ALVO.write_text("%s%s\n%s\n%s%s" % (antes, INICIO, bloco, FIM, depois),
                    encoding="utf-8")
    print("%s: %d guardas" % (ALVO, len(dados["guardas"])))


main()
