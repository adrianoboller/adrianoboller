#!/usr/bin/env python3
"""A catraca do catalogo envelhecido -- pedido 263.

    python3 bancada/guardas/trecho-vivo.py --catraca

# O defeito que motivou

Em 16/09/2026 o `provar-guardas.py` correu inteiro e devolveu **11 guardas
QUEBRADAS** -- nao reprovadas, QUEBRADAS: o `trecho` que a entrada manda
substituir para repor o defeito **nao existe mais** no arquivo, entao a
guarda nao pode nem ser tentada. Guarda que existe e nao guarda e pior que
guarda faltando, porque o catalogo a conta como cobertura.

Medido commit a commit depois: **um unico commit aposentou cinco delas de
uma vez** -- `2fe8658` (12/09, «a conferencia de FK dentro da transacao ve o
pai empilhado»), que mexeu em `table.rs` e `transacao.rs`. Ninguem percebeu
por QUATRO DIAS, e o motivo e o custo: o provador leva cerca de uma hora,
porque repoe o defeito e roda `cargo test` para cada uma das 142 entradas.
Guarda que so se confere em uma hora e guarda que nao se confere.

# O que esta regua faz, e o que ela NAO faz

FAZ, em 0,18 s (medido, tres corridas) e sem compilar nada: pergunta, para cada entrada
do catalogo, se o `trecho` ainda existe **literalmente** no arquivo que ela
nomeia, e se cada teste citado em `caem`/`seguem` ainda existe como `fn` em
`crates/**/*.rs`.

NAO FAZ, e isto importa: ela **nao substitui o provador**. Achar o trecho
nao prova que repo-lo derruba o teste -- so o provador prova isso, e
continua sendo ele a autoridade. Esta regua e o aviso barato, que pega a
classe de envelhecimento que custou os quatro dias: o codigo andou e a
entrada ficou para tras.

# A armadilha que esta medicao ja pagou

A primeira versao varria so `crates/<pacote>/src/` atras dos testes e
acusou **154** nomes mortos. Eram 154 falsos: o teste de integracao mora em
`crates/<pacote>/tests/`, nao em `src/`. Regua que mede um terco da caixa e
anuncia o numero inteiro e a mesma lei do KiB da interface -- **quando um
gerador depende de uma lista, a lista tem de sair do codigo**. Hoje a
varredura e `crates/**/*.rs` inteiro, e o numero medido e zero.

# As catracas

So DESCEM. Nascem hoje no numero MEDIDO, nao no numero desejado:

- `TETO_TRECHO_MORTO = 8` -- a divida velha, nomeada no pedido 263, cada uma
  com o commit que a quebrou. Nao se conserta por varredura: cada entrada
  pede ler o codigo de hoje, achar para onde o ponto de reposicao andou, e
  provar que o defeito reposto derruba o teste nomeado. Entrada consertada
  no chute produz guarda que passa por engano -- pior que a quebrada.
- `TETO_TESTE_MORTO = 0` -- medido depois do conserto de
  `leitura-sem-recuo-para-a-exclusiva`, cuja entrada nomeava
  `so_uma_operacao_usa_a_ficha_compartilhada`, renomeado em `f2b87aa` para
  `so_as_duas_operacoes_medidas_usam_a_ficha_compartilhada`.
"""
import importlib.util
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))

TETO_TRECHO_MORTO = 8
TETO_TESTE_MORTO = 0


def catalogo():
    """Le o catalogo pelo proprio modulo, nunca por copia da lista.

    Receita duplicada e receita que diverge: uma segunda lista de guardas
    aqui envelheceria sozinha, e a regua passaria a medir um catalogo que
    nao e o que o provador roda."""
    caminho = os.path.join(AQUI, "catalogo.py")
    spec = importlib.util.spec_from_file_location("catalogo_das_guardas", caminho)
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    return getattr(modulo, "CATALOGO", None) or getattr(modulo, "GUARDAS")


def pares(guarda):
    """Cada (arquivo, trecho) da guarda -- o catalogo tem duas formas.

    A comum traz `arquivo`/`trecho` no topo; a de varias trocas traz uma
    lista em `trocas`, e cada item dela tem o seu proprio par."""
    if guarda.get("trocas"):
        return [(t["arquivo"], t["trecho"]) for t in guarda["trocas"]]
    return [(guarda["arquivo"], guarda["trecho"])]


FUNCAO = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")


def nomes_de_funcao():
    """Toda `fn` declarada no Rust versionado, num conjunto.

    `src/` guarda o teste de modulo e `tests/` o de integracao: varrer so um
    dos dois mede parte da caixa e anuncia o numero inteiro.

    O conjunto e o ponto: a primeira versao guardava o fonte inteiro numa
    string e corria uma busca por nome citado -- sao mais de quatrocentas
    buscas varrendo os mesmos megabytes, e medido deu **31,6 s**. Uma
    passagem so, com o nome virando chave, da o mesmo veredito. Regua cara
    e regua que nao se roda."""
    achados_ = set()
    for dirpath, dirs, arquivos in os.walk(os.path.join(RAIZ, "crates")):
        dirs[:] = [d for d in dirs if d not in ("target", ".git")]
        for nome in sorted(arquivos):
            if not nome.endswith(".rs"):
                continue
            try:
                with open(os.path.join(dirpath, nome), encoding="utf-8",
                          errors="replace") as f:
                    achados_.update(FUNCAO.findall(f.read()))
            except OSError:
                continue
    return achados_


def achados():
    """(trechos mortos, testes mortos), cada um com a guarda que o nomeia."""
    lido = {}

    def ler(rel):
        if rel not in lido:
            caminho = os.path.join(RAIZ, rel)
            try:
                with open(caminho, encoding="utf-8", errors="replace") as f:
                    lido[rel] = f.read()
            except OSError:
                lido[rel] = None
        return lido[rel]

    declaradas = nomes_de_funcao()
    trechos, testes = [], []
    for g in catalogo():
        gid = g.get("id")
        for arq, trecho in pares(g):
            texto = ler(arq)
            if texto is None:
                trechos.append((gid, arq, "o arquivo nao existe mais"))
            elif trecho not in texto:
                trechos.append((gid, arq, "o trecho nao esta mais la"))
        for campo in ("caem", "seguem"):
            for teste in g.get(campo) or []:
                if teste.split("::")[-1] not in declaradas:
                    testes.append((gid, campo, teste))
    return trechos, testes


def catraca():
    trechos, testes = achados()
    print("=== a catraca do catalogo envelhecido (pedido 263) ===")
    print(f"   {len(catalogo())} guardas no catalogo")
    if trechos:
        print("   -- trecho que o codigo nao tem mais:")
        for gid, arq, porque in trechos:
            print(f"      {gid}: {porque} ({arq})")
    if testes:
        print("   -- teste nomeado que nao existe em crates/**/*.rs:")
        for gid, campo, teste in testes:
            print(f"      {gid}: {campo} -> {teste}")

    ruim = 0
    for medido, teto, nome, recado in (
        (len(trechos), TETO_TRECHO_MORTO, "TETO_TRECHO_MORTO",
         "guarda que nao pode nem ser tentada nao esta guardando nada -- "
         "ache para onde o ponto de reposicao andou e prove com o provador."),
        (len(testes), TETO_TESTE_MORTO, "TETO_TESTE_MORTO",
         "o catalogo nomeia um teste que o fonte nao tem -- alguem renomeou "
         "o teste e a entrada ficou para tras."),
    ):
        if medido > teto:
            print(f"\n   SUBIU  {nome}: {medido} (teto {teto})")
            print(f"   Reprovado: {recado}")
            ruim = 1
        elif medido < teto:
            print(f"\n   DESCEU -- BAIXE O TETO  {nome}: {medido} (teto {teto})")
            print(f"   Melhorou. Ponha o numero novo em {nome}, no mesmo "
                  "commit -- catraca frouxa nao segura nada.")
            ruim = 1
        else:
            print(f"\n   ok  {nome}: {medido} (teto {teto})")
    return ruim


def principal():
    if "--catraca" in sys.argv:
        return catraca()
    trechos, testes = achados()
    for gid, arq, porque in trechos:
        print(f"trecho-morto {gid}: {porque} ({arq})")
    for gid, campo, teste in testes:
        print(f"teste-morto  {gid}: {campo} -> {teste}")
    return 0


if __name__ == "__main__":
    sys.exit(principal())
