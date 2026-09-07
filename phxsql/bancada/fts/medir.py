#!/usr/bin/env python3
"""A bancada do indice de texto: o `.fts` contra a varredura.

    python3 bancada/fts/medir.py [linhas] [buscas]

Ela nao mede nada por si: chama os dois medidores do motor, le a linha
`RESULTADO <json>` de cada um e junta num `resultados.json` com a data. Existe
por dois motivos.

O primeiro e a regra da casa: **numero visivel sai de um gerador.** A pagina
dos testes le este arquivo; quem digitasse os numeros la teria um retrato que
envelhece calado.

O segundo e o portao: **bancada compara trabalho igual, e nao so pergunta
igual.** Quem confere isso e o proprio medidor do motor -- ele aborta se os
conjuntos de rowids das duas faixas diferirem em qualquer busca, e aborta se
nenhuma busca achar linha. Este roteiro so recolhe.

Antes de rodar, pergunte ao portao:

    bancada/esta-medindo.sh && echo "ha medicao em curso -- espere"
"""

import datetime
import json
import pathlib
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]
ALVO = pathlib.Path(__file__).resolve().parent / "resultados.json"


def rodar(exemplo, *args):
    binario = RAIZ / "target" / "release" / "examples" / exemplo
    if not binario.exists():
        sys.exit(
            f"falta {binario}\n"
            "  cargo build --release --examples -p phxsql-store\n"
            "  (o `--release` sozinho NAO recompila os examples: medidor com "
            "binario velho mede o passado)"
        )
    r = subprocess.run(
        [str(binario), *map(str, args)], capture_output=True, text=True, cwd=RAIZ
    )
    if r.returncode != 0:
        sys.exit(f"{exemplo} falhou:\n{r.stdout}\n{r.stderr}")
    print(r.stdout)
    for linha in r.stdout.splitlines():
        if linha.startswith("RESULTADO "):
            return json.loads(linha[len("RESULTADO ") :])
    sys.exit(f"{exemplo} nao imprimiu a linha RESULTADO")


def faixa(valores):
    """min, mediana e max -- porque UMA corrida nao e medicao.

    A regra e a do pedido 155, e esta casa a pagou uma vez declarando vencedor
    dentro do ruido. Aqui ela apareceu de novo: duas corridas seguidas da mesma
    bancada deram 4,49x e 5,79x no custo de escrita, e 31.472x e 18.666x no
    ganho da busca. Publicar a primeira teria sido publicar sorte.
    """
    v = sorted(valores)
    return {"min": v[0], "mediana": v[len(v) // 2], "max": v[-1]}


REPETICOES = 3


def main():
    linhas = int(sys.argv[1]) if len(sys.argv) > 1 else 1_000_000
    buscas = int(sys.argv[2]) if len(sys.argv) > 2 else 20

    b, e = [], []
    for i in range(REPETICOES):
        print(f"--- corrida {i + 1} de {REPETICOES} ---")
        b.append(rodar("o-indice-de-texto-contra-a-varredura", linhas, buscas))
        e.append(rodar("custo-do-fts-de-verdade", 50_000, 200))

    achados = {x["achados"] for x in b}
    assert len(achados) == 1, f"as corridas acharam quantidades diferentes: {achados}"
    chaves = {x["chaves"] for x in e}
    assert len(chaves) == 1, f"as corridas puseram quantidades diferentes: {chaves}"

    saida = {
        "quando": datetime.datetime.now().isoformat(timespec="seconds"),
        "repeticoes": REPETICOES,
        "linhas": b[0]["linhas"],
        "buscas": b[0]["buscas"],
        "linhas_achadas": b[0]["achados"],
        "us_varredura": faixa([x["us_varredura"] for x in b]),
        "us_indice": faixa([x["us_indice"] for x in b]),
        "ganho": faixa([x["ganho"] for x in b]),
        "carga_com_indice_s": faixa([x["carga_s"] for x in b]),
        # O lado da ESCRITA, que e o preco do ganho de cima. Sem ele a bancada
        # contaria so a metade que nos favorece.
        "escrita_chaves_por_linha": e[0]["chaves"] / 50_000,
        "escrita_sem_indice_us": faixa([x["a_ms"] * 1000 / 50_000 for x in e]),
        "escrita_com_indice_us": faixa([x["b_ms"] * 1000 / 50_000 for x in e]),
        "escrita_vezes": faixa([x["b_sobre_a"] for x in e]),
        "lote_sobre_linha_a_linha": faixa([x["c_sobre_b"] for x in e]),
    }
    ALVO.write_text(json.dumps(saida, indent=2, ensure_ascii=False) + "\n")
    print(f"gravado: {ALVO}")
    g = saida["ganho"]
    print(
        f"  ganho da busca: {g['mediana']:.0f}x  (faixa {g['min']:.0f}-{g['max']:.0f}x)"
    )
    w = saida["escrita_vezes"]
    print(
        f"  preco na escrita: {w['mediana']:.2f}x  (faixa {w['min']:.2f}-{w['max']:.2f}x)"
    )


if __name__ == "__main__":
    main()
