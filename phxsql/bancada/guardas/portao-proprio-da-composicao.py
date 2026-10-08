#!/usr/bin/env python3
"""A catraca da conferencia propria da composicao -- divisao do `servidor.rs`.

    python3 bancada/guardas/portao-proprio-da-composicao.py --catraca
    python3 bancada/guardas/portao-proprio-da-composicao.py --numeros

# O risco que ela segura

O portao de permissao e UM so e le o campo `"tabela"` do pedido. Quatro
operacoes escondem tabela desse campo -- `juntar` (`a.tabela`/`b.tabela`),
`unir` (uma lista), `pivotar` (as de consulta num `juntar` aninhado) e
`diferencas` -- e por isso cada uma paga `pode_em(..., Atividade::Ler)`
propria (`CLAUDE.md`, «Portao de permissao e UM so»). Numa divisao do
`servidor.rs` essa conferencia PARECE duplicacao do portao; limpa-la reabre a
porta dos fundos, e nenhum teste do portao acusa -- so as guardas
`*-sem-portao` do catalogo, que rodam por prova e nao a cada commit.

As quatro moram juntas em `servidor/servico_composicao_01.rs`
(`docs/propostas/divisao-do-servidor.md`, risco 1), e esta catraca conta as
chamadas de codigo (comentario fora) de `pode_em(` com `Atividade::Ler`
naquele arquivo. O numero nasceu medido nas quatro funcoes ANTES da divisao,
no `servidor.rs` inteiro, e e o mesmo depois: 4.

# A catraca

PISO, e exato: medir MENOS reprova (alguem limpou uma conferencia); medir MAIS
tambem reprova ate o piso subir no mesmo commit, porque catraca frouxa nao
segura nada. Se uma das quatro funcoes sair do arquivo, a catraca reprova
dizendo qual -- contar zero num arquivo que perdeu a funcao nao e melhora.
"""
import os
import re
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
ALVO = os.path.join(RAIZ, "crates/phxsql-server/src/servidor/servico_composicao_01.rs")
EU = os.path.relpath(os.path.abspath(__file__), RAIZ)

PISO_PORTAO_PROPRIO_DA_COMPOSICAO = 4
FUNCOES = ("op_pivotar", "op_juntar", "op_diferencas", "op_unir")
CHAMADA = re.compile(r"\bpode_em\s*\([^;{}]*?Atividade::Ler", re.S)


def codigo():
    """O texto do alvo sem as linhas de comentario."""
    with open(ALVO, encoding="utf-8") as f:
        return "".join(l for l in f if not l.lstrip().startswith("//"))


def medir():
    t = codigo()
    faltam = [f for f in FUNCOES if not re.search(r"\bfn\s+%s\s*\(" % f, t)]
    return len(CHAMADA.findall(t)), faltam


def catraca():
    medido, faltam = medir()
    print("=== a conferencia propria da composicao (pode_em com Atividade::Ler) ===")
    print(f"   {os.path.relpath(ALVO, RAIZ)}")
    if faltam:
        print(f"\n   REPROVADO: {', '.join(faltam)} saiu do arquivo -- as quatro "
              "moram juntas, e contar so as que ficaram nao e melhora")
        return 1
    if medido < PISO_PORTAO_PROPRIO_DA_COMPOSICAO:
        print(f"\n   DESCEU  {medido} (piso {PISO_PORTAO_PROPRIO_DA_COMPOSICAO})")
        print("   Reprovado: a conferencia propria NAO e duplicacao do portao. "
              "Sem ela, a tabela negada entra como lado de uma composicao.")
        return 1
    if medido > PISO_PORTAO_PROPRIO_DA_COMPOSICAO:
        print(f"\n   SUBIU -- SUBA O PISO  {medido} (piso "
              f"{PISO_PORTAO_PROPRIO_DA_COMPOSICAO})")
        print("   Ponha o numero novo em PISO_PORTAO_PROPRIO_DA_COMPOSICAO no "
              "mesmo commit -- catraca frouxa nao segura nada.")
        return 1
    print(f"\n   ok  {medido} (piso {PISO_PORTAO_PROPRIO_DA_COMPOSICAO})")
    return 0


def numeros():
    """Saida de maquina para o inventario das catracas (`docs/qa/medir.py`)."""
    medido, _ = medir()
    print(f"catraca:nome=PISO_PORTAO_PROPRIO_DA_COMPOSICAO;onde={EU};"
          f"valor={PISO_PORTAO_PROPRIO_DA_COMPOSICAO};medido={medido};tipo=piso;"
          "mede=chamadas `pode_em(` com `Atividade::Ler` em "
          "servidor/servico_composicao_01.rs")
    return 0


def principal():
    if "--catraca" in sys.argv:
        return catraca()
    if "--numeros" in sys.argv:
        return numeros()
    medido, faltam = medir()
    print(medido, faltam)
    return 0


if __name__ == "__main__":
    sys.exit(principal())
