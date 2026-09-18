#!/usr/bin/env python3
"""Prova real do leitor do PENDENCIAS.md: o defeito REPOSTO tem de PARAR.

    python3 docs/dossie/prova-do-leitor-de-pedidos.py

Por que ela existe, com o numero: em 18/09/2026 o `pagina-dos-pedidos.py`
imprimiu «369 pedidos» quando o arquivo tinha **380** linhas de pedido, e
imprimiu junto tres linhas de exito. Onze linhas nao casavam a forma -- seis
sem o pipe de fechamento, e cinco porque o fecho entrou como QUINTA coluna --
e o leitor as pulava em silencio.

A guarda do pedido 150 ja existia, e nao pegou: ela olhava o SIMBOLO de
estado, e nestas onze o simbolo estava certo. Guarda que cobre o motivo em vez
do EFEITO deixa a porta irma aberta -- e o efeito aqui e' um so, «pedido que
existe no arquivo e nao existe na pagina», por qualquer caminho.

Esta prova e' nos dois sentidos: o arquivo SAO passa, e cada defeito reposto
tem de parar o leitor. Teste que passa por engano e' pior que teste que falta.
"""

import pathlib
import subprocess
import sys
import tempfile

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]

CABECA = """# Pendencias de mentira

|  | # | O que voce pediu | Estado |
|---|---:|---|---|
"""

SA = "| ☑️ | 1 | **um pedido bem formado** | fechado, com o pipe no fim |"

# Cada defeito reposto e' um caso REAL ja pago aqui, com o pedido que o pagou
# e a guarda que o pega. Os quatro sao o mesmo EFEITO -- «pedido que existe no
# arquivo e nao existe na pagina» -- por quatro caminhos, e e' por isso que os
# quatro ficam juntos: guarda nova costuma cobrir um caminho so.
#
# O terceiro custou a licao desta prova. Eu o escrevi primeiro COM o pipe de
# fechamento, e ele passou mesmo com a guarda nova arrancada -- porque assim a
# linha casa o regex e cai na guarda VELHA do pipe cru. As cinco linhas reais
# nao tinham o fecho, entao nao casavam nada. Prova que reproduz um defeito
# PARECIDO com o de origem e' prova que passa por engano.
REPOSTOS = [
    ("sem o pipe de fechamento (pedidos 375-380)",
     "| ☐ | 2 | **linha que nao fecha** | falta o pipe no fim"),
    ("quinta coluna E sem fecho, como o script de fecho deixou "
     "(pedidos 358, 366, 370, 373, 367)",
     "| ☑️ | 2 | **linha com coluna a mais** | o que faltava | "
     "**FECHADO**, e este fecho devia estar na coluna de cima"),
    ("quinta coluna COM fecho -- pipe cru, guarda velha (quatro linhas de 2026)",
     "| ☑️ | 2 | **linha com pipe cru** | um `|` solto no meio | sobra |"),
    ("simbolo fora da legenda (pedido 150, a guarda velha)",
     "| ⏳ | 2 | **linha com estado desconhecido** | o `⏳` nunca esteve na legenda |"),
]


def ler_com(linhas):
    """Roda o leitor contra um PENDENCIAS de mentira, e devolve (codigo, saida).

    O gerador le um caminho FIXO (`docs/PENDENCIAS.md`), entao a prova nao pode
    chamar o script inteiro: ela importa o modulo e troca a FONTE. Chamar o
    script gravaria por cima do dossie de verdade -- prova que estraga o que
    prova nao e' prova.
    """
    with tempfile.NamedTemporaryFile("w", suffix=".md", encoding="utf-8",
                                     delete=False) as f:
        f.write(CABECA + "\n".join(linhas) + "\n")
        falso = pathlib.Path(f.name)
    codigo = ("import sys, pathlib; sys.path.insert(0, %r);\n"
              "import importlib.util as u;\n"
              "e = u.spec_from_file_location('g', %r); g = u.module_from_spec(e);\n"
              "e.loader.exec_module(g);\n"
              "g.FONTE = pathlib.Path(%r);\n"
              "print(len(g.ler()), 'lidos')\n"
              % (str(AQUI), str(AQUI / "pagina-dos-pedidos.py"), str(falso)))
    r = subprocess.run([sys.executable, "-c", codigo],
                       capture_output=True, text=True, cwd=str(RAIZ))
    falso.unlink()
    return r.returncode, (r.stdout + r.stderr).strip()


def main() -> int:
    falhas = []

    codigo, saida = ler_com([SA])
    if codigo != 0 or "1 lidos" not in saida:
        falhas.append(f"o arquivo SAO nao passou: {saida}")
    else:
        print("  ok    arquivo sao -> le 1 pedido")

    for nome, torta in REPOSTOS:
        codigo, saida = ler_com([SA, torta])
        if codigo == 0:
            falhas.append(f"defeito reposto NAO parou o leitor -- {nome}\n"
                          f"          (ele devolveu: {saida})")
            print(f"  FALHA {nome}")
        elif "PENDENCIAS.md:" not in saida or "pedido 2" not in saida:
            falhas.append(f"parou, mas sem dizer a LINHA e o PEDIDO -- {nome}\n"
                          f"          (disse: {saida})")
            print(f"  FALHA {nome} (parou sem nomear linha/pedido)")
        else:
            print(f"  ok    {nome} -> para, e nomeia a linha")

    print()
    if falhas:
        print(f"VERMELHO: {len(falhas)} caso(s) da prova do leitor falharam:")
        for f in falhas:
            print("  - " + f)
        return 1
    print(f"VERDE: o arquivo sao passa e os {len(REPOSTOS)} defeitos repostos "
          "param o leitor nomeando a linha.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
