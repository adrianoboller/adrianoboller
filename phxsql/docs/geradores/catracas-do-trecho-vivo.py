#!/usr/bin/env python3
"""Escreve, no `docs/CATRACAS.md` §12.3, a tabela das reguas do `trecho-vivo.py`.

# Por que ele existe

A tabela dizia `PISO_DAS_ENTRADAS | piso | 177 | 177` enquanto a constante
valia 773: o piso subiu quatro vezes em 16 dias e a tabela foi digitada uma
vez. A propria secao se antecipava («quem mexe no piso atualiza esta secao no
mesmo passo») -- e e exatamente a promessa que ninguem cumpre. O numero tem um
gerador desde o pedido 263: `trecho-vivo.py --numeros`, que imprime, por
regua, o `valor` da constante e o `medido` de agora. Este script so o publica.

Fica digitada, de proposito, a coluna «Nasceu»: e historia datada («nasceu em
143 a 16/09»), nao estado -- e o gerador escreve o estado ao lado dela.

Uso:  python3 docs/geradores/catracas-do-trecho-vivo.py
Roda `trecho-vivo.py --numeros` (Python puro, ~1 s, sem cargo). Se o comando
falhar, ou se aparecer regua que a tabela nao conhece, ou faltar uma que ela
conhece, ele PARA com o motivo -- regua nova sem linha na tabela e a mesma
doenca de novo.
"""
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
DOC = RAIZ / "docs/CATRACAS.md"
FONTE = RAIZ / "bancada/guardas/trecho-vivo.py"

ABRE = "<!-- GERADO: catracas-do-trecho-vivo.py -->"
FECHA = "<!-- /GERADO: catracas-do-trecho-vivo.py -->"

# Historia datada de cada regua (o que NAO e estado). Ordem = ordem da tabela.
NASCEU = [
    ("TETO_TRECHO_MORTO", "16/09, em 8; desceu para 0 no mesmo dia"),
    ("TETO_TRECHO_AMBIGUO", "16/09, nesta frente"),
    ("TETO_TESTE_MORTO", "16/09"),
    ("TETO_TESTE_FORA_DO_BINARIO", "16/09, nesta frente"),
    ("TETO_TESTE_SEM_MODULO", "17/09, pedido 273 — depois do conserto dos três nomes da §15.7.7; §12.7"),
    ("TETO_NAO_JULGADA_ESCONDIDA",
     "16/09, pedido 269: nasceu medido em **26** e desceu para **0** no mesmo passo, republicando a corrida de 15:25"),
    ("PISO_DAS_ENTRADAS",
     "16/09 em 143, e sobe junto com o catálogo (145, 151, 160, 169, 170 e 177 em 16–17/09, e daí em diante — o valor de hoje é o da coluna «Valor»). "
     "Piso só sobe, e sobe no mesmo passo em que o catálogo cresce"),
]


def medir():
    r = subprocess.run([sys.executable, str(FONTE), "--numeros"], cwd=RAIZ,
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"catracas-do-trecho-vivo: `trecho-vivo.py --numeros` falhou ({r.returncode}):\n{r.stderr[-400:]}")
    achadas = {}
    for linha in r.stdout.splitlines():
        if not linha.startswith("catraca:"):
            continue
        campos = dict(p.split("=", 1) for p in linha[len("catraca:"):].split(";") if "=" in p)
        achadas[campos["nome"]] = campos
    return achadas


def tabela(achadas):
    conhecidas = [n for n, _ in NASCEU]
    novas = sorted(set(achadas) - set(conhecidas))
    faltam = [n for n in conhecidas if n not in achadas]
    if novas or faltam:
        sys.exit(f"catracas-do-trecho-vivo: a tabela e o `--numeros` divergem -- "
                 f"regua sem linha: {novas}; linha sem regua: {faltam}. Edite NASCEU.")
    linhas = ["| Régua | Lado | Valor | Medido | Nasceu |", "|---|---|---:|---:|---|"]
    for nome, historia in NASCEU:
        c = achadas[nome]
        lado = "**piso**" if c["tipo"] == "piso" else "teto"
        linhas.append(f"| `{nome}` | {lado} | {c['valor']} | **{c['medido']}** | {historia} |")
    return "\n".join(linhas)


def main():
    achadas = medir()
    bloco = f"{ABRE}\n{tabela(achadas)}\n{FECHA}"
    texto = DOC.read_text(encoding="utf-8")
    if ABRE in texto:
        a, b = texto.index(ABRE), texto.index(FECHA) + len(FECHA)
        novo = texto[:a] + bloco + texto[b:]
    else:
        # primeira vez: troca a tabela escrita a mao pela marcada
        i = texto.index("### 12.3 ")
        j = texto.index("| Régua | Lado | Valor | Medido | Nasceu |", i)
        k = texto.index("\n\n", j)
        novo = texto[:j] + bloco + texto[k:]
    if novo != texto:
        DOC.write_text(novo, encoding="utf-8")
    print(f"catracas-do-trecho-vivo: {len(NASCEU)} reguas; PISO_DAS_ENTRADAS = "
          f"{achadas['PISO_DAS_ENTRADAS']['valor']} (medido {achadas['PISO_DAS_ENTRADAS']['medido']})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
