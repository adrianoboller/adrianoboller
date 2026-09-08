#!/usr/bin/env python3
"""Escreve, no `docs/SEGURANCA.md`, a secao medida do direito por coluna.

# Por que ele existe

Porque a lista das operacoes e a RECEITA do numero, e a receita tambem
envelhece: quem digitar «4 leem, 3 escrevem» na mao publica o numero de hoje e
nao percebe quando a operacao 130 entrar. A lista sai do proprio
`direito_coluna.rs` -- que e onde ela DECIDE alguma coisa -- e a contagem sai
da lista.

Uso:  python3 docs/geradores/direito-por-coluna.py
"""
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
FONTE = RAIZ / "crates/phxsql-server/src/direito_coluna.rs"
CATALOGO = RAIZ / "crates/phxsql-server/src/catalogo.rs"
DOC = RAIZ / "docs/SEGURANCA.md"

ABRE = "<!-- direito-por-coluna: gerado por docs/geradores/direito-por-coluna.py -->"
FECHA = "<!-- fim direito-por-coluna -->"

ROTULO = {
    "Le": "devolve linha, e a peneira a alcança",
    "Escreve": "recebe colunas para gravar",
    "Estrutura": "descreve a estrutura",
    "Recusa": "devolve ou grava linha por caminho que a peneira não alcança",
    "Nenhum": "não toca em dado de linha",
}


def classes():
    """(op, classe) na ordem em que a tabela do Rust os declara."""
    fonte = FONTE.read_text(encoding="utf-8")
    i = fonte.index("pub const CLASSES")
    corpo = fonte[i : fonte.index("\n];", i)]
    achados = re.findall(r'\(\s*"([^"]+)"\s*,\s*PorColuna::(\w+)', corpo)
    if not achados:
        sys.exit("nao achei nenhuma classe em direito_coluna.rs")
    return achados


def canonicos():
    """Os nomes CANONICOS do catalogo, e o apelido -> canonico."""
    texto = CATALOGO.read_text(encoding="utf-8")
    i = texto.index("pub const OPERACOES: &[Operacao] = &[")
    corpo = texto[i:]
    de_apelido = {}
    nomes = []
    for bloco in corpo.split("Operacao {")[1:]:
        m = re.search(r'nome:\s*"([^"]+)"', bloco)
        if not m:
            continue
        nome = m.group(1)
        nomes.append(nome)
        de_apelido[nome] = nome
        a = re.search(r"apelidos:\s*&\[([^\]]*)\]", bloco)
        if a:
            for ap in re.findall(r'"([^"]+)"', a.group(1)):
                de_apelido[ap] = nome
    return nomes, de_apelido


def main():
    todas = classes()
    nomes, de_apelido = canonicos()
    faltando = sorted({op for op, _ in todas} - set(de_apelido))
    if faltando:
        sys.exit(f"classe para operacao que nao existe no catalogo: {faltando}")

    # So os canonicos entram na tabela: o apelido nao e outra operacao, e
    # conta-lo duas vezes inflaria todo numero desta secao.
    por_op = {}
    for op, c in todas:
        por_op.setdefault(de_apelido[op], c)
    sem_classe = [n for n in nomes if n not in por_op]
    if sem_classe:
        sys.exit(f"operacao do catalogo sem classe: {sem_classe}")

    grupos = {}
    for nome in nomes:
        grupos.setdefault(por_op[nome], []).append(nome)

    n = {k: len(v) for k, v in grupos.items()}
    linhas = [
        ABRE,
        # O TITULO tambem sai daqui, e nao da mao de quem escreve o documento:
        # ele carrega tres numeros, e numero digitado envelhece calado.
        f"## 15. Direito por coluna: as {n['Le']} que devolvem linha, "
        f"as {n['Escreve']} que escrevem e as {n['Recusa']} que recusam",
        "",
        f"Medido em {len(nomes)} operações do catálogo "
        f"(`crates/phxsql-server/src/catalogo.rs`), classificadas uma a uma em "
        f"`CLASSES`, no `crates/phxsql-server/src/direito_coluna.rs`. Os "
        f"apelidos viajam com a operação e não contam de novo.",
        "",
        "| classe | quantas | o que o servidor faz |",
        "|---|---:|---|",
    ]
    for chave in ["Le", "Escreve", "Estrutura", "Recusa", "Nenhum"]:
        if chave not in grupos:
            continue
        linhas.append(f"| `{chave}` | {n[chave]} | {ROTULO[chave]} |")
    linhas += ["", "As listas que decidem alguma coisa:", ""]
    for chave, titulo in [
        ("Le", "**Devolvem linha, e a coluna negada sai da resposta**"),
        ("Escreve", "**Recebem colunas, e a coluna negada é conferida**"),
        ("Estrutura", "**Descreve a estrutura, que continua inteira**"),
        ("Recusa", "**Recusam a tabela restrita, para não vazar**"),
    ]:
        if chave not in grupos:
            continue
        ops = ", ".join(f"`{o}`" for o in grupos[chave])
        linhas.append(f"- {titulo} ({n[chave]}): {ops}.")
    linhas += [
        "",
        f"As outras {n.get('Nenhum', 0)} não devolvem nem recebem dado de "
        "linha, e por isso passam sem custo nenhum.",
        "",
        FECHA,
    ]
    novo = "\n".join(linhas)

    doc = DOC.read_text(encoding="utf-8")
    if ABRE not in doc or FECHA not in doc:
        sys.exit(
            f"nao achei as marcas no {DOC.name}. Ponha, onde a secao deve "
            f"nascer:\n{ABRE}\n{FECHA}"
        )
    inicio = doc.index(ABRE)
    fim = doc.index(FECHA) + len(FECHA)
    DOC.write_text(doc[:inicio] + novo + doc[fim:], encoding="utf-8")
    print(f"{DOC.relative_to(RAIZ)}: {len(nomes)} operacoes, " + ", ".join(
        f"{v} {k}" for k, v in sorted(n.items())
    ))


if __name__ == "__main__":
    main()
