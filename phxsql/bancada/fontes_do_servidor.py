#!/usr/bin/env python3
"""As fontes do servidor: a LISTA UNICA em Python, derivada do disco.

    python3 bancada/fontes_do_servidor.py             # a lista, uma por linha
    python3 bancada/fontes_do_servidor.py --conferir  # disco x lista do Rust

O `servidor.rs` se divide em `servidor/*.rs` (`docs/propostas/divisao-do-
servidor.md`). Todo leitor que media «o servidor.rs» passa a medir a uniao
`servidor.rs` + `servidor/**/*.rs`, e mede por AQUI: leitor que monta a
propria lista e leitor que um dia mede um pedaco dizendo que mediu o todo.

A irma em Rust e o `FONTES_DO_SERVIDOR` do `servidor.rs`, com o teste que a
compara com o `read_dir`. As duas saem do disco, na mesma ordem (o
`servidor.rs` e depois os filhos pelo caminho em texto), e o `--conferir`
reprova quando a chamada `fontes_do_servidor!` do Rust e o disco divergem.

Teste e producao se separam pela DECLARACAO, e nao pelo nome: arquivo que
algum `.rs` declara com `#[cfg(test)] mod x;` (ou `cfg(all(test, ...))`) e
teste, e tudo debaixo da pasta dele tambem. Separar por `testes_*` no nome
funcionaria hoje e mentiria no primeiro modulo de teste com outro nome.
"""
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
SRC = RAIZ / "crates/phxsql-server/src"
PRINCIPAL = SRC / "servidor.rs"
PASTA = SRC / "servidor"

# `#[cfg(test)]` ou `#[cfg(all(test, ...))]`, outros atributos no meio, e um
# `mod x;` SEM corpo: e a declaracao de um arquivo que so existe em teste.
DECLARACAO_DE_TESTE = re.compile(
    r"#\s*\[\s*cfg\s*\(\s*(?:test|all\s*\(\s*test\b[^\]]*\))\s*\)\s*\]"
    r"\s*(?:#\s*\[[^\]]*\]\s*)*"
    r"(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+(\w+)\s*;")


def _chave(caminho):
    """A ordem do Rust: o caminho relativo a `src/`, em texto."""
    return caminho.relative_to(SRC).as_posix()


def todas():
    """`servidor.rs` e depois `servidor/**/*.rs`, na ordem do caminho."""
    filhos = sorted(PASTA.rglob("*.rs"), key=_chave) if PASTA.is_dir() else []
    return [PRINCIPAL] + filhos


def pasta_dos_filhos(arquivo):
    """Onde moram os `mod x;` que este arquivo declara: ao lado dele se ele
    e `mod.rs`/`lib.rs`/`main.rs`, e na pasta com o nome dele se nao."""
    if arquivo.name in ("mod.rs", "lib.rs", "main.rs"):
        return arquivo.parent
    return arquivo.parent / arquivo.stem


def so_de_teste(arquivos):
    """Os arquivos da lista que algum deles declara so em teste -- e tudo o
    que mora debaixo da pasta de um modulo de teste. Resolve como o
    compilador: `x.rs` ou `x/mod.rs`. Nome declarado sem arquivo nao entra
    (o crivo e textual, e um comentario que cita `mod x;` nao resolve nada)."""
    lista = [Path(a) for a in arquivos]
    presentes = set(lista)
    teste = set()
    for arq in lista:
        texto = arq.read_text(encoding="utf-8", errors="replace")
        base = pasta_dos_filhos(arq)
        for nome in DECLARACAO_DE_TESTE.findall(texto):
            for candidato in (base / f"{nome}.rs", base / nome / "mod.rs"):
                if candidato in presentes:
                    teste.add(candidato)
            pasta = base / nome
            teste |= {p for p in presentes if pasta in p.parents}
    return teste


def producao():
    """As fontes do servidor que compilam fora de teste."""
    lista = todas()
    fora = so_de_teste(lista)
    return [a for a in lista if a not in fora]


def testes():
    """As fontes do servidor que so existem em teste."""
    lista = todas()
    fora = so_de_teste(lista)
    return [a for a in lista if a in fora]


def texto(arquivos=None):
    """O texto unido, na ordem da lista -- o mesmo do `FONTE_DO_SERVIDOR`."""
    return "".join(a.read_text(encoding="utf-8")
                   for a in (todas() if arquivos is None else arquivos))


def relativo(caminho):
    """Relativo a `phxsql/`, o caminho que o catalogo e os mapas escrevem."""
    return Path(caminho).resolve().relative_to(RAIZ).as_posix()


def lista_do_rust():
    """Os nomes da chamada `fontes_do_servidor! { ... }` do `servidor.rs`."""
    t = PRINCIPAL.read_text(encoding="utf-8")
    m = re.search(r"\nfontes_do_servidor!\s*\{([^}]*)\}", t)
    if not m:
        return None
    return re.findall(r'"([^"]+)"', m.group(1))


def conferir():
    rust = lista_do_rust()
    disco = [_chave(a) for a in todas()]
    if rust is None:
        print("REPROVADO: a chamada `fontes_do_servidor!` sumiu do servidor.rs")
        return 1
    if rust != disco:
        print("REPROVADO: a lista do Rust e o disco divergem")
        for n in sorted(set(disco) - set(rust)):
            print(f"   no disco, fora do Rust: {n}")
        for n in sorted(set(rust) - set(disco)):
            print(f"   no Rust, fora do disco: {n}")
        if set(rust) == set(disco):
            print("   (mesmos nomes, ordem diferente)")
        return 1
    print(f"ok: {len(disco)} fontes, a mesma lista no Rust e no disco "
          f"({len(producao())} de producao, {len(testes())} de teste)")
    return 0


if __name__ == "__main__":
    if "--conferir" in sys.argv:
        raise SystemExit(conferir())
    for a in todas():
        print(relativo(a))
