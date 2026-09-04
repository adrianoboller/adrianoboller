#!/usr/bin/env python3
"""Gera a tabela das catracas do QA-PDCA. Nenhum numero se digita.

    python3 docs/qa/medir.py            imprime a tabela
    python3 docs/qa/medir.py --gravar   escreve dentro do docs/QA-PDCA.md

Por que ele existe
------------------
A tabela nasceu com `TETO` em 1.577. Noventa minutos depois, na MESMA rodada,
outra frente traduziu 28 textos e baixou a catraca para 1.549 -- e nenhuma das
duas podia ver a outra. Numero digitado nao envelhece «entre versoes» quando ha
trabalho paralelo: ele ja nasce errado.

Como ele acha as catracas, sem lista digitada
---------------------------------------------
Ele NAO tem lista. Varre `crates/*/examples/*.rs` atras de quem imprime
`catraca:`, e pergunta a cada um com `--numeros`. Cada conferidor SE DESCREVE:
o nome da constante, onde ela mora, o valor dela e o numero medido hoje.

E daqui saem duas coisas que uma lista digitada nao daria:

* **catraca frouxa** aparece sozinha -- valor acima do medido e folga onde uma
  regressao se esconde;
* **catraca que ninguem mede** aparece como buraco: uma constante `TETO*` no
  codigo sem conferidor que a reporte nao e catraca, e uma promessa.

E por que ele NAO le o relatorio dos conferidores
-------------------------------------------------
Porque `grep` na prosa e resolver numero por comparacao de FRASE -- a mesma
armadilha que esta casa ja proibiu para texto de tela. No dia em que alguem
melhorar a redacao do relatorio, o gerador quebraria calado e publicaria o
numero de ontem. A chave e estavel; o rotulo e livre.
"""
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
MARCA_INICIO = "<!-- catracas:inicio -->"
MARCA_FIM = "<!-- catracas:fim -->"
MARCA_BAT_INICIO = "<!-- bateria:inicio -->"
MARCA_BAT_FIM = "<!-- bateria:fim -->"


def exemplos_que_se_descrevem():
    """Os exemplos que imprimem `catraca:` -- achados, nao listados."""
    achados = []
    for arq in sorted(RAIZ.glob("crates/*/examples/*.rs")):
        if "catraca:nome=" in arq.read_text(encoding="utf-8", errors="replace"):
            achados.append((arq.parents[1].name, arq.stem))
    return achados


def perguntar(crate, exemplo):
    r = subprocess.run(
        ["cargo", "run", "-q", "--release", "--example", exemplo, "-p", crate,
         "--", "--numeros"],
        cwd=RAIZ, capture_output=True, text=True)
    if r.returncode != 0:
        return [], f"{exemplo}: nao rodou ({r.returncode})"
    linhas = []
    for l in r.stdout.splitlines():
        if l.startswith("catraca:"):
            campos = dict(p.split("=", 1) for p in l[len("catraca:"):].split(";") if "=" in p)
            campos = {k.strip(): v.strip() for k, v in campos.items()}
            campos["exemplo"] = exemplo
            campos["crate"] = crate
            linhas.append(campos)
    return linhas, None


def constantes_teto():
    """Toda `pub const TETO*` do codigo, para achar quem NAO tem conferidor."""
    achadas = {}
    for arq in RAIZ.glob("crates/*/src/**/*.rs"):
        for n, linha in enumerate(arq.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            m = re.match(r"\s*pub const (TETO\w*)\s*:", linha)
            if m:
                achadas[m.group(1)] = f"{arq.relative_to(RAIZ)}:{n}"
    return achadas


def tabela():
    linhas = [MARCA_INICIO,
              "",
              "| Catraca | Onde mora | Valor | Medido hoje | Estado |",
              "|---|---|---:|---:|---|"]
    vistos, problemas = set(), []
    for crate, exemplo in exemplos_que_se_descrevem():
        cs, erro = perguntar(crate, exemplo)
        if erro:
            problemas.append(erro)
            continue
        for c in cs:
            vistos.add(c["nome"])
            valor, medido = int(c["valor"]), int(c["medido"])
            if medido > valor:
                estado = f"**REPROVANDO** — {medido - valor} acima"
            elif medido == valor:
                estado = "em cima, sem folga"
            else:
                estado = f"**FROUXA** — {valor - medido} de folga, baixe-a"
            # O separador de milhar se troca SO no numero. A primeira versao
            # fazia `.replace(",", ".")` na linha inteira, e comia a virgula da
            # frase: «em cima, sem folga» saiu «em cima. sem folga». Formatacao
            # aplicada onde nao devia e a mesma familia de «rotulo se estiliza,
            # dado nunca» -- so que aqui o estrago foi na prosa.
            fmt = lambda n: f"{n:,}".replace(",", ".")
            linhas.append(
                f"| `{c['nome']}` ({c['mede']}) | `{c['onde']}` | "
                f"{fmt(valor)} | **{fmt(medido)}** | {estado} |")

    orfas = [(n, o) for n, o in sorted(constantes_teto().items()) if n not in vistos]
    linhas += ["",
               f"*{len(vistos)} catraca(s) medida(s) por conferidor. "
               f"Refaz com `python3 docs/qa/medir.py`.*"]
    if orfas:
        linhas += ["",
                   "**Constantes `TETO*` que NENHUM conferidor reporta.** Elas não são",
                   "catracas: são limites, ou promessas. A diferença importa — catraca",
                   "sem medidor não segura nada e ainda parece que segura:",
                   ""]
        linhas += [f"- `{n}` — `{o}`" for n, o in orfas]
    if problemas:
        linhas += ["", "**Não consegui medir:** " + "; ".join(problemas)]
    linhas.append(MARCA_FIM)
    return "\n".join(linhas)


def bateria():
    """Quantas partes a bateria tem -- PERGUNTADAS a ela, nao contadas na prosa.

    O texto de abertura deste documento dizia «25 partes» e a bateria ja tinha
    26 quando alguem foi olhar. Numero digitado a mao envelhece calado, e este
    envelheceu no mesmo lugar que ensina a nao digitar numero.

    Perguntar ao `provar.py --listar` e o mesmo principio das catracas: quem
    sabe o numero e quem o tem. Um `grep` por `parte(` na fonte contaria
    tambem as chamadas comentadas e as de dentro de um exemplo do docstring.
    """
    r = subprocess.run([sys.executable, "provar.py", "--listar"],
                       cwd=RAIZ, capture_output=True, text=True)
    m = re.search(r"^(\d+) partes:", r.stdout, re.M)
    if not m:
        return (MARCA_BAT_INICIO
                + "\n\n*nao consegui perguntar ao `provar.py --listar` "
                  "quantas partes a bateria tem.*\n\n"
                + MARCA_BAT_FIM)
    return (MARCA_BAT_INICIO
            + f"\n- `provar.py` orquestra a bateria única, hoje **{m.group(1)} "
              "partes** — o número sai de `python3 provar.py --listar`, nunca "
              "digitado aqui.\n"
            + MARCA_BAT_FIM)


def substituir(t, inicio, fim, novo, doc):
    if inicio not in t:
        print(f"nao achei {inicio} no {doc} -- ponha as duas marcas em volta "
              "do trecho e rode de novo")
        return None
    i, f = t.index(inicio), t.index(fim) + len(fim)
    return t[:i] + novo + t[f:]


def principal():
    saida, contagem = tabela(), bateria()
    if "--gravar" in sys.argv:
        doc = RAIZ / "docs/QA-PDCA.md"
        t = doc.read_text(encoding="utf-8")
        t = substituir(t, MARCA_INICIO, MARCA_FIM, saida, doc.name)
        if t is None:
            return 2
        t = substituir(t, MARCA_BAT_INICIO, MARCA_BAT_FIM, contagem, doc.name)
        if t is None:
            return 2
        doc.write_text(t, encoding="utf-8")
        print(f"tabela e contagem gravadas em {doc.relative_to(RAIZ)}")
    else:
        print(contagem)
        print()
        print(saida)
    return 0


if __name__ == "__main__":
    sys.exit(principal())
