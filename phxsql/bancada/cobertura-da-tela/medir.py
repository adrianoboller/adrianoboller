#!/usr/bin/env python3
"""Quantas operacoes do protocolo a TELA alcanca, e quais ficam de fora.

    python3 bancada/cobertura-da-tela/medir.py

# Por que este script existe

O `docs/PENDENCIAS.md` ja publicou este numero tres vezes, e as tres
envelheceram: «36 das 39», depois «108 das 96», depois «104 das 122». O ultimo
envelheceu em UM DIA -- o `procurar_texto` entrou e o catalogo virou 123.

Numero que se digita envelhece calado. E a RECEITA tambem envelhece: a versao
anterior mandava casar `api("…")` em `ui/`, e `ui/` na raiz nao existe mais --
a tela mora em `crates/phxsql-server/ui/`, e a lista dos arquivos dela sai dos
`include_str!` do `http.rs`. Contar pela receita velha da ZERO.

Entao as duas listas saem do CODIGO:

- as operacoes, do array `OPERACOES` do `catalogo.rs`;
- os arquivos da tela, dos `include_str!` do `http.rs`.

E a chamada tem DUAS formas na tela, `api("nome")` e `"op":"nome"`. Contar so
a primeira foi o que deu «zero» na revisao de 06/09.
"""

import datetime
import json
import pathlib
import re
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ALVO = AQUI / "resultados.json"

CATALOGO = RAIZ / "crates/phxsql-server/src/catalogo.rs"
HTTP = RAIZ / "crates/phxsql-server/src/http.rs"
UI = RAIZ / "crates/phxsql-server/ui"


def operacoes():
    """Os nomes e apelidos do array `OPERACOES`, e nada mais do arquivo.

    O recorte comeca em `pub const OPERACOES` e termina no `];` da coluna 0 --
    sem ele, os `nome:` dos ajudantes `obr()`/`opc()` do topo entrariam na
    conta.
    """
    fonte = CATALOGO.read_text(encoding="utf-8")
    i = fonte.index("pub const OPERACOES")
    j = fonte.index("\n];", i)
    corpo = fonte[i:j]
    nomes = re.findall(r'^\s*nome: "([A-Za-z_0-9]+)"', corpo, re.M)
    apelidos = re.findall(r'apelidos: &\[([^\]]*)\]', corpo)
    extras = set()
    for a in apelidos:
        extras |= set(re.findall(r'"([A-Za-z_0-9]+)"', a))
    if len(nomes) < 50:
        sys.exit(f"SONDA QUEBRADA: so {len(nomes)} operacoes no catalogo")
    return nomes, extras


def arquivos_da_tela():
    """A lista sai dos `include_str!` do `http.rs`, nunca digitada aqui."""
    fonte = HTTP.read_text(encoding="utf-8")
    rel = re.findall(r'include_str!\("(?:\.\./)*ui/([^"]+)"\)', fonte)
    achados = []
    for r in sorted(set(rel)):
        p = UI / r
        if p.exists():
            achados.append(p)
    if not achados:
        sys.exit("SONDA QUEBRADA: nenhum arquivo de tela achado pelos "
                 "include_str! do http.rs -- a receita envelheceu de novo")
    return achados


def main():
    nomes, apelidos = operacoes()
    arquivos = arquivos_da_tela()
    tela = "\n".join(p.read_text(encoding="utf-8", errors="replace")
                     for p in arquivos)

    # As DUAS formas da chamada. Contar so a primeira ja deu zero uma vez.
    chamadas = set(re.findall(r'api\(\s*[\'"]([A-Za-z_0-9]+)[\'"]', tela))
    chamadas |= set(re.findall(r'["\']op["\']\s*:\s*["\']([A-Za-z_0-9]+)["\']', tela))
    chamadas |= set(re.findall(r'op:\s*["\']([A-Za-z_0-9]+)["\']', tela))

    alcancadas = [n for n in nomes if n in chamadas]
    fora = [n for n in nomes if n not in chamadas]

    d = {
        # A data sai do PROPRIO resultado, e nao do `mtime`: um `git checkout`
        # move o mtime sem medir nada, e a pagina dos testes marca quando
        # precisa recorrer a ele.
        "quando": datetime.datetime.now().isoformat(timespec="seconds"),
        "operacoes": len(nomes),
        "alcancadas_pela_tela": len(alcancadas),
        "fora": fora,
        "arquivos_da_tela": [str(p.relative_to(RAIZ)) for p in arquivos],
        "apelidos_no_catalogo": len(apelidos),
    }
    ALVO.write_text(json.dumps(d, indent=2, ensure_ascii=False) + "\n",
                    encoding="utf-8")
    print(f"{len(nomes)} operacoes no catalogo")
    print(f"{len(alcancadas)} alcancadas pela tela "
          f"({len(arquivos)} arquivos, pela lista do http.rs)")
    print(f"{len(fora)} fora: {', '.join(fora)}")
    print(f"gravado: {ALVO.relative_to(RAIZ)}")


if __name__ == "__main__":
    main()
