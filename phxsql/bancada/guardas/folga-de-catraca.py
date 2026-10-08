#!/usr/bin/env python3
"""A catraca da FOLGA escondida nos testes das catracas -- pedido 656.

    python3 bancada/guardas/folga-de-catraca.py --catraca
    python3 bancada/guardas/folga-de-catraca.py --numeros
    python3 bancada/guardas/folga-de-catraca.py --autoteste

# O defeito que motivou

A catraca so desce, e o teste dela reprova nos dois sentidos: acima do teto
(alguem acrescentou divida) e abaixo (alguem pagou divida e nao baixou o
numero -- catraca frouxa nao segura nada). Duas delas tinham o segundo
sentido AFROUXADO por um numero solto na condicao:

    faltando.len() >= TETO_ROTULOS_E_CRASE.saturating_sub(30)
    achadas.len() + 3 >= TETO_NUMERO_CRAVADO_EM_TELA

Trinta textos traduzidos (ou tres numeros tirados da tela) sem a catraca
descer passavam calados, enquanto o `docs/QA-PDCA.md` publicava as duas «em
cima, sem folga» -- porque a tabela compara a constante com o MEDIDO, e nao
com o que o teste tolera. A folga nao aparecia em lugar nenhum que alguem
lesse.

# O que ela conta

Asserts em `crates/**/*.rs` cuja CONDICAO (o primeiro argumento, e nao a
mensagem) cita uma constante `TETO*`, compara uma contagem (`.len()`, o crivo
de catraca do `docs/qa/medir.py`) e traz um numero de folga: um
`.saturating_sub(N)`, ou um `+ N`/`- N` com N > 0. Teto ZERO, nascido no
medido de 07/10/2026 depois de as duas folgas sairem (eram 2).

Folga legitima, se um dia existir, nao se esconde num literal: vira uma
constante com nome (`FOLGA_*`), e entao aparece em quem le -- esta regua nao
conta folga nomeada, de proposito, porque o defeito era a folga MUDA.

So a condicao, e nao o assert inteiro: a mensagem do teste do
`TETO_NUMERO_CRAVADO_EM_TELA` imprime `TETO - achadas.len()`, e contar a
mensagem acusaria quem explica o numero.
"""
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

TETO_FOLGA_ESCONDIDA = 0

ASSERT = re.compile(r"\bassert(?:_eq|_ne)?!\s*\(")
TETO = re.compile(r"\bTETO\w*")
FOLGA = re.compile(r"saturating_sub\(\s*[1-9][0-9_]*\s*\)|[+-]\s*[1-9][0-9_]*\b")


def argumentos(texto, inicio):
    """Os argumentos de topo de um `assert!(`, com parenteses, colchetes,
    chaves e strings balanceados -- a virgula de dentro de um `format!` ou de
    uma string nao separa nada."""
    args, atual, prof, i, n = [], [], 1, inicio, len(texto)
    while i < n and prof:
        c = texto[i]
        if c == '"':
            j = i + 1
            while j < n and texto[j] != '"':
                j += 2 if texto[j] == "\\" else 1
            atual.append(texto[i:j + 1])
            i = j + 1
            continue
        if c in "([{":
            prof += 1
        elif c in ")]}":
            prof -= 1
            if prof == 0:
                break
        if c == "," and prof == 1:
            args.append("".join(atual))
            atual = []
        else:
            atual.append(c)
        i += 1
    args.append("".join(atual))
    return args


def folgas_no_texto(texto):
    """(linha, condicao) de cada assert com folga muda."""
    achadas = []
    for m in ASSERT.finditer(texto):
        args = argumentos(texto, m.end())
        quantos = 2 if "assert_eq" in m.group(0) or "assert_ne" in m.group(0) else 1
        cond = " ".join(a.strip() for a in args[:quantos])
        # `.len()` junto do TETO e o crivo de catraca do `docs/qa/medir.py`
        # (`_imposta_por_teste_no_proprio_arquivo`): sem ele, o teste de um
        # LIMITE de funcionamento (`TETO_DO_REGISTRO + 1` bytes) contaria como
        # folga -- e la o `+ 1` e o caso-limite, nao tolerancia nenhuma.
        if TETO.search(cond) and ".len()" in cond and FOLGA.search(cond):
            linha = texto.count("\n", 0, m.start()) + 1
            achadas.append((linha, " ".join(cond.split())))
    return achadas


def medir():
    achadas = []
    for dirpath, dirs, arquivos in os.walk(os.path.join(RAIZ, "crates")):
        dirs[:] = [d for d in dirs if d not in ("target", ".git")]
        for nome in sorted(arquivos):
            if not nome.endswith(".rs"):
                continue
            caminho = os.path.join(dirpath, nome)
            with open(caminho, encoding="utf-8", errors="replace") as f:
                texto = f.read()
            if "TETO" not in texto:
                continue
            for linha, cond in folgas_no_texto(texto):
                achadas.append((os.path.relpath(caminho, RAIZ), linha, cond))
    return achadas


MEDE = "asserts de catraca com folga numerica muda na condicao"


def catraca():
    print("=== a catraca da folga escondida (pedido 656) ===")
    achadas = medir()
    for arq, linha, cond in achadas:
        print(f"      {arq}:{linha}  {cond}")
    n = len(achadas)
    if n > TETO_FOLGA_ESCONDIDA:
        print(f"   SUBIU  TETO_FOLGA_ESCONDIDA: {n} (teto {TETO_FOLGA_ESCONDIDA})")
        print("   Reprovado: o teste da catraca tolera um numero que ninguem le. "
              "O teto e o medido; se a folga e mesmo necessaria, ela vira "
              "constante FOLGA_* com o motivo, e aparece no QA-PDCA.")
        return 1
    if n < TETO_FOLGA_ESCONDIDA:
        print(f"   DESCEU -- BAIXE O TETO  TETO_FOLGA_ESCONDIDA: {n}")
        return 1
    print(f"   ok  TETO_FOLGA_ESCONDIDA: {n} (teto {TETO_FOLGA_ESCONDIDA})")
    return 0


def numeros():
    print(f"catraca:nome=TETO_FOLGA_ESCONDIDA;onde={EU};"
          f"valor={TETO_FOLGA_ESCONDIDA};medido={len(medir())};tipo=teto;"
          f"mede={MEDE}")
    return 0


def autoteste():
    """Os dois defeitos de 02/10 repostos no texto de verdade do
    `conferidor.rs`, e os controles que uma regua por padrao de texto erraria."""
    falhas = []

    def conferir(nome, cond, detalhe=""):
        print("   %s  %s%s" % ("ok  " if cond else "FALHOU", nome,
                               "" if cond else "  -- " + detalhe))
        if not cond:
            falhas.append(nome)

    real = open(os.path.join(RAIZ, "crates/phxsql-server/src/conferidor.rs"),
                encoding="utf-8").read()
    conferir("o conferidor.rs de hoje nao tem folga muda",
             folgas_no_texto(real) == [], str(folgas_no_texto(real)))
    conferir("a arvore inteira mede o teto", len(medir()) == TETO_FOLGA_ESCONDIDA,
             str(medir()))
    reposto = real.replace("faltando.len() >= TETO_ROTULOS_CRASE_E_JS,",
                           "faltando.len() >= TETO_ROTULOS_CRASE_E_JS.saturating_sub(30),", 1)
    reposto = reposto.replace("achadas.len() >= TETO_NUMERO_CRAVADO_EM_TELA,",
                              "achadas.len() + 3 >= TETO_NUMERO_CRAVADO_EM_TELA,", 1)
    conferir("o recorte achou os dois lugares", reposto.count("TETO_ROTULOS_CRASE_E_JS.saturating_sub(30),") == 1
             and reposto.count("achadas.len() + 3 >=") == 1)
    conferir("as duas folgas de 02/10 repostas: acusa as DUAS",
             len(folgas_no_texto(reposto)) == 2, str(folgas_no_texto(reposto)))
    # controles
    conferir("condicao sem folga passa",
             folgas_no_texto("assert!(x.len() <= TETO_A, \"m\");") == [])
    conferir("folga na MENSAGEM nao conta (so a condicao)",
             folgas_no_texto('assert!(x.len() >= TETO_A, "sobram {}", TETO_A - 3);') == [])
    conferir("virgula dentro de string da condicao nao corta a condicao",
             len(folgas_no_texto('assert!(f(",").len() + 2 >= TETO_A, "m");')) == 1)
    conferir("assert_eq com folga no segundo argumento acusa",
             len(folgas_no_texto("assert_eq!(x.len(), TETO_A - 1);")) == 1)
    conferir("folga NOMEADA nao e muda: nao conta",
             folgas_no_texto("assert!(x.len() + FOLGA_A >= TETO_A);") == [])
    conferir("LIMITE de funcionamento (sem `.len()`) nao e catraca: nao conta",
             folgas_no_texto("assert!(t.entregues <= TETO_DO_REGISTRO + 1);") == [])
    conferir("assert sem TETO nao interessa",
             folgas_no_texto("assert!(x + 1 >= y);") == [])
    print("   %s" % ("todos passaram" if not falhas
                     else "FALHOU: " + ", ".join(falhas)))
    return 1 if falhas else 0


def principal():
    if "--autoteste" in sys.argv:
        print("=== autoteste da catraca da folga escondida (pedido 656) ===")
        return autoteste()
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    for arq, linha, cond in medir():
        print(f"{arq}:{linha}  {cond}")
    return 0


if __name__ == "__main__":
    sys.exit(principal())
