#!/usr/bin/env python3
"""Extrator dos numeros do docs/TECNOLOGIAS.md -- so biblioteca padrao.

    python3 docs/tecnologias/extrair.py

Imprime, em ordem, os blocos Markdown que o TECNOLOGIAS.md cola verbatim.
Cada numero sai daqui porque a regra do projeto e clara: "todo numero
visivel sai de um gerador, ou esta errado e ninguem percebeu ainda" -- e o
corolario que ja custou caro: quando um gerador depende de uma lista, a
lista tem de sair do codigo, nunca copiada para dentro do script. E o caso
aqui: a lista dos arquivos que a interface embute sai do proprio
crates/phxsql-server/src/http.rs (funcao `arquivos_da_interface`), no lugar
de uma lista digitada -- foi exatamente essa lista digitada que fez o
rodape do dossie publicar 780 KiB quando eram 1.032.

Quem editar este arquivo: nao digite um numero que da para medir. Se algo
nao for medivel daqui, a funcao correspondente devolve None e o texto que
usa isso tem de dizer "nao medido", nunca estimar.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]


# ============================================================ utilidades


def ler(caminho: Path) -> str:
    return caminho.read_text(encoding="utf-8")


def linhas(caminho: Path) -> list[str]:
    return ler(caminho).splitlines()


# ======================================================= 1. RUST: LINHAS
#
# Classificador de arquivo .rs em codigo-de-teste vs codigo-de-produto.
#
# Duas passadas sobre o texto bruto, caractere a caractere, para nao se
# confundir com chave dentro de string/char (json.rs tem literais como
# '{' e strings JSON de teste -- uma contagem ingenua de chaves erraria
# exatamente no arquivo do parser JSON, que e o que mais importa aqui):
#
#   1. "visao de profundidade": comentarios, strings, chars e raw strings
#      viram espacos (preservando quebras de linha), sobrando so a
#      sintaxe real -- e so nela se contam as chaves para achar onde um
#      bloco `#[cfg(test)] mod nome { ... }` comeca e termina.
#   2. "visao de classificacao": so os COMENTARIOS viram espacos (strings
#      ficam como estao, porque uma linha dentro de uma string de teste
#      continua sendo codigo, nao comentario) -- e essa visao decide se
#      uma linha e "vazia", "comentario" ou "codigo".


def _apagar(s: str) -> str:
    """Troca todo caractere por espaco, PRESERVANDO quebra de linha real --
    sem isto, uma string com continuacao por `\\` no fim da linha (comum em
    `const USO: &str = "\\` seguido de quebra) perde a quebra na visao de
    profundidade, desalinha a contagem de linhas com o arquivo real, e o
    classificador erra por 1 ou 2 linhas sem dar erro nenhum -- silencioso
    e exatamente o tipo de furo que este documento existe para nao ter."""
    return "".join(" " if ch != "\n" else "\n" for ch in s)


def _varrer_rust(texto: str):
    """Devolve (visao_profundidade, visao_classificacao), mesmo tamanho."""
    prof = []
    clas = []
    i, n = 0, len(texto)
    estado = "normal"  # normal | linha | bloco | string | rawstring | char
    prof_bloco_nivel = 0
    raw_hashes = 0
    while i < n:
        c = texto[i]
        if estado == "normal":
            if c == "/" and i + 1 < n and texto[i + 1] == "/":
                estado = "linha"
                prof.append(" ")
                clas.append(" ")
                i += 1
                continue
            if c == "/" and i + 1 < n and texto[i + 1] == "*":
                estado = "bloco"
                prof_bloco_nivel = 1
                prof.append(" ")
                clas.append(" ")
                i += 2
                continue
            if c == "\"":
                estado = "string"
                prof.append(" ")
                clas.append(c)
                i += 1
                continue
            # raw string: r"...", r#"...", r##"...", tambem br/b prefixados
            m = re.match(r'(?:b|B)?r(#*)"', texto[i:])
            if m:
                raw_hashes = len(m.group(1))
                estado = "rawstring"
                prof.append(_apagar(m.group(0)))
                clas.append(m.group(0))
                i += len(m.group(0))
                continue
            if c == "b" and i + 1 < n and texto[i + 1] == "\"":
                estado = "string"
                prof.append(" ")
                clas.append(c)
                i += 1
                continue
            if c == "'":
                # char literal 'x' ou '\n' -- nunca lifetime, que nao fecha
                # com aspa simples a 2 ou 3 posicoes de distancia.
                fechamento = None
                if i + 2 < n and texto[i + 1] == "\\":
                    # \n \t \\ \' \0 \xNN \u{...}
                    j = i + 2
                    if texto[j] == "u" and j + 1 < n and texto[j + 1] == "{":
                        k = texto.find("}", j)
                        if k != -1 and k + 1 < n and texto[k + 1] == "'":
                            fechamento = k + 1
                    elif texto[j] == "x" and j + 2 < n and texto[j + 3] == "'":
                        fechamento = j + 3
                    elif j + 1 < n and texto[j + 1] == "'":
                        fechamento = j + 1
                elif i + 2 < n and texto[i + 2] == "'":
                    fechamento = i + 2
                if fechamento is not None:
                    trecho = texto[i : fechamento + 1]
                    prof.append(_apagar(trecho))
                    clas.append(trecho)
                    i = fechamento + 1
                    continue
                # lifetime: passa a aspa como caractere normal
                prof.append(c)
                clas.append(c)
                i += 1
                continue
            if c in "{}":
                prof.append(c)
            else:
                prof.append(c)
            clas.append(c)
            i += 1
            continue
        if estado == "linha":
            if c == "\n":
                estado = "normal"
                prof.append(c)
                clas.append(c)
            else:
                prof.append(" ")
                clas.append(" ")
            i += 1
            continue
        if estado == "bloco":
            if c == "/" and i + 1 < n and texto[i + 1] == "*":
                prof_bloco_nivel += 1
                prof.append("  ")
                clas.append("  ")
                i += 2
                continue
            if c == "*" and i + 1 < n and texto[i + 1] == "/":
                prof_bloco_nivel -= 1
                prof.append("  ")
                clas.append("  ")
                i += 2
                if prof_bloco_nivel == 0:
                    estado = "normal"
                continue
            prof.append(" " if c != "\n" else "\n")
            clas.append(" " if c != "\n" else "\n")
            i += 1
            continue
        if estado == "string":
            if c == "\\" and i + 1 < n:
                prof.append(_apagar(texto[i : i + 2]))
                clas.append(texto[i : i + 2])
                i += 2
                continue
            if c == "\"":
                estado = "normal"
                prof.append(" ")
                clas.append(c)
                i += 1
                continue
            prof.append(" " if c != "\n" else "\n")
            clas.append(c)
            i += 1
            continue
        if estado == "rawstring":
            fecho = '"' + ("#" * raw_hashes)
            if texto[i:].startswith(fecho):
                prof.append(" " * len(fecho))
                clas.append(fecho)
                i += len(fecho)
                estado = "normal"
                continue
            prof.append(" " if c != "\n" else "\n")
            clas.append(c)
            i += 1
            continue
    return "".join(prof), "".join(clas)


def classificar_rust(caminho: Path):
    """Devolve dict com total/vazias/comentario/codigo/teste (linhas)."""
    texto = ler(caminho)
    prof, clas = _varrer_rust(texto)
    linhas_orig = texto.split("\n")
    linhas_prof = prof.split("\n")
    linhas_clas = clas.split("\n")
    # se o arquivo termina com \n, split gera uma ultima entrada vazia --
    # remove-a das tres vistas igualmente para nao contar linha fantasma.
    if linhas_orig and linhas_orig[-1] == "" and texto.endswith("\n"):
        linhas_orig = linhas_orig[:-1]
        linhas_prof = linhas_prof[:-1]
        linhas_clas = linhas_clas[:-1]

    # acha os intervalos [inicio, fim] (indice 0-based, fim inclusivo) de
    # todo bloco `#[cfg(test)] mod nome { ... }` ou `#[cfg(test)] mod nome;`
    # usando a visao de profundidade para contar chaves com seguranca.
    intervalos_teste = []
    i = 0
    while i < len(linhas_prof):
        if re.match(r"\s*#\[cfg\(test\)\]\s*$", linhas_orig[i]):
            # acha a proxima linha nao-vazia (na visao de profundidade)
            j = i + 1
            while j < len(linhas_prof) and linhas_prof[j].strip() == "":
                j += 1
            if j < len(linhas_prof):
                m_mod = re.match(r"\s*mod\s+\w+\s*;", linhas_prof[j])
                m_bloco = re.match(r"\s*mod\s+\w+\s*\{", linhas_prof[j])
                if m_mod:
                    intervalos_teste.append((i, j))
                    i = j + 1
                    continue
                if m_bloco:
                    depth = linhas_prof[j].count("{") - linhas_prof[j].count(
                        "}"
                    )
                    k = j
                    while depth > 0 and k + 1 < len(linhas_prof):
                        k += 1
                        depth += linhas_prof[k].count("{") - linhas_prof[
                            k
                        ].count("}")
                    intervalos_teste.append((i, k))
                    i = k + 1
                    continue
        i += 1

    em_teste = [False] * len(linhas_orig)
    for a, b in intervalos_teste:
        for k in range(a, min(b + 1, len(em_teste))):
            em_teste[k] = True
    # `mod nome;` (arquivo externo) soma o arquivo apontado inteiro como
    # teste -- resolvido por quem chama, via MODULOS_TESTE_EXTERNOS.

    vazias = comentario = codigo = teste = 0
    for idx, (orig, clas_l) in enumerate(zip(linhas_orig, linhas_clas)):
        if orig.strip() == "":
            vazias += 1
            continue
        if clas_l.strip() == "":
            comentario += 1
            continue
        if em_teste[idx]:
            teste += 1
        else:
            codigo += 1

    return {
        "total": len(linhas_orig),
        "vazias": vazias,
        "comentario": comentario,
        "codigo": codigo,
        "teste": teste,
    }


# `mod nome;` sob #[cfg(test)] aponta um arquivo externo inteiramente de
# teste. Achado por classificar_rust (ele devolve a linha do `mod ...;`
# como "teste", 1 linha) -- o resto do arquivo apontado entra aqui por
# correspondencia de caminho, resolvida no momento da soma por crate.
def achar_modulos_teste_externos(crate_dir: Path) -> dict[Path, Path]:
    """Mapa {arquivo-que-declara -> arquivo externo} para `#[cfg(test)]
    mod nome;` (sem chaves = declara modulo em outro arquivo)."""
    mapa = {}
    for rs in sorted((crate_dir / "src").rglob("*.rs")):
        texto = ler(rs)
        for m in re.finditer(
            r"#\[cfg\(test\)\]\s*\n\s*mod\s+(\w+)\s*;", texto
        ):
            nome = m.group(1)
            candidato = rs.parent / f"{nome}.rs"
            if candidato.exists():
                mapa[rs] = candidato
    return mapa


def contar_crate(crate_dir: Path):
    src = crate_dir / "src"
    if not src.exists():
        return None
    arquivos = sorted(src.rglob("*.rs"))
    externos = set(achar_modulos_teste_externos(crate_dir).values())
    soma = {"total": 0, "vazias": 0, "comentario": 0, "codigo": 0, "teste": 0}
    por_arquivo = {}
    for rs in arquivos:
        r = classificar_rust(rs)
        por_arquivo[rs] = r
        if rs in externos:
            # arquivo inteiro e teste (modulo declarado com `mod nome;`
            # sob #[cfg(test)]): a parte "codigo" dele (se houver, por
            # exemplo comentario de cabecalho fora de qualquer chave) vira
            # teste tambem, porque o arquivo so existe para isso.
            soma["total"] += r["total"]
            soma["vazias"] += r["vazias"]
            soma["comentario"] += r["comentario"]
            soma["teste"] += r["codigo"] + r["teste"]
        else:
            for k in soma:
                soma[k] += r[k]
    examples = crate_dir / "examples"
    ex_linhas = 0
    ex_arquivos = 0
    if examples.exists():
        for rs in sorted(examples.rglob("*.rs")):
            ex_linhas += len(linhas(rs))
            ex_arquivos += 1
    tests_dir = crate_dir / "tests"
    it_linhas = 0
    it_arquivos = 0
    if tests_dir.exists():
        for rs in sorted(tests_dir.rglob("*.rs")):
            it_linhas += len(linhas(rs))
            it_arquivos += 1
    return {
        "src": soma,
        "arquivos_src": len(arquivos),
        "examples_linhas": ex_linhas,
        "examples_arquivos": ex_arquivos,
        "tests_dir_linhas": it_linhas,
        "tests_dir_arquivos": it_arquivos,
    }


def bloco_linguagens_rust() -> str:
    crates_dir = RAIZ / "crates"
    nomes = sorted(p.name for p in crates_dir.iterdir() if (p / "Cargo.toml").exists())
    linhas_out = []
    linhas_out.append(
        "| crate | arquivos .rs | codigo | teste | comentario | vazias | total |"
    )
    linhas_out.append("|---|---:|---:|---:|---:|---:|---:|")
    totais = {"arquivos": 0, "codigo": 0, "teste": 0, "comentario": 0, "vazias": 0, "total": 0}
    ex_total_linhas = 0
    ex_total_arquivos = 0
    it_total_linhas = 0
    it_total_arquivos = 0
    for nome in nomes:
        r = contar_crate(crates_dir / nome)
        if r is None:
            continue
        s = r["src"]
        linhas_out.append(
            f"| `{nome}` | {r['arquivos_src']} | {s['codigo']} | {s['teste']} | "
            f"{s['comentario']} | {s['vazias']} | {s['total']} |"
        )
        totais["arquivos"] += r["arquivos_src"]
        totais["codigo"] += s["codigo"]
        totais["teste"] += s["teste"]
        totais["comentario"] += s["comentario"]
        totais["vazias"] += s["vazias"]
        totais["total"] += s["total"]
        ex_total_linhas += r["examples_linhas"]
        ex_total_arquivos += r["examples_arquivos"]
        it_total_linhas += r["tests_dir_linhas"]
        it_total_arquivos += r["tests_dir_arquivos"]
    linhas_out.append(
        f"| **total** | **{totais['arquivos']}** | **{totais['codigo']}** | "
        f"**{totais['teste']}** | **{totais['comentario']}** | "
        f"**{totais['vazias']}** | **{totais['total']}** |"
    )
    prop = (
        totais["teste"] / totais["codigo"] if totais["codigo"] else 0
    )
    linhas_out.append("")
    linhas_out.append(
        f"Proporcao teste/codigo (so `src/`, sem comentario nem linha vazia): "
        f"**{totais['teste']}/{totais['codigo']} = {prop:.2f}×**."
    )
    linhas_out.append("")
    linhas_out.append(
        f"Alem do `src/`: **{ex_total_arquivos}** programas de medicao em "
        f"`examples/` ({ex_total_linhas} linhas — bancada em Rust, nao "
        f"produto nem teste) e **{it_total_arquivos}** arquivos em "
        f"`tests/` de integracao fora de `src/` ({it_total_linhas} linhas)."
    )
    return "\n".join(linhas_out)


# =================================================== 2. OUTRAS LINGUAGENS


def contar_arquivos(padroes: list[str], base: Path, excluir: list[str] | None = None):
    excluir = excluir or []
    total_linhas = 0
    total_arquivos = 0
    for pad in padroes:
        for f in sorted(base.rglob(pad)):
            rel = str(f.relative_to(RAIZ))
            if any(e in rel for e in excluir):
                continue
            total_linhas += len(linhas(f))
            total_arquivos += 1
    return total_arquivos, total_linhas


def arquivos_da_interface_embutidos() -> list[Path]:
    """A LISTA sai do codigo, nao e digitada aqui -- ver docstring do
    modulo. Le crates/phxsql-server/src/http.rs e extrai todo
    `include_str!`/`include_bytes!` que aponta para dentro de `ui/`."""
    http_rs = RAIZ / "crates" / "phxsql-server" / "src" / "http.rs"
    texto = ler(http_rs)
    achados = re.findall(r'include_(?:str|bytes)!\("\.\./([^"]+)"\)', texto)
    caminhos = [RAIZ / "crates" / "phxsql-server" / a for a in achados]
    faltando = [c for c in caminhos if not c.exists()]
    if faltando:
        raise SystemExit(
            f"http.rs cita um arquivo que nao existe: {faltando} -- "
            "a lista saiu do codigo, mas o codigo esta errado ou o "
            "arquivo sumiu. Conserte antes de gerar o documento."
        )
    return caminhos


def bloco_interface() -> str:
    arquivos = arquivos_da_interface_embutidos()
    linhas_out = []
    linhas_out.append(
        "Lista extraida de `crates/phxsql-server/src/http.rs` "
        "(todo `include_str!`/`include_bytes!` que aponta para `ui/`), "
        "nao digitada -- e a mesma lista que decide o que o binario embute:"
    )
    linhas_out.append("")
    linhas_out.append("| arquivo embutido | linhas | KiB |")
    linhas_out.append("|---|---:|---:|")
    total_bytes = 0
    total_linhas = 0
    for c in arquivos:
        b = c.stat().st_size
        n = len(linhas(c))
        total_bytes += b
        total_linhas += n
        rel = c.relative_to(RAIZ / "crates" / "phxsql-server")
        linhas_out.append(f"| `{rel}` | {n} | {b / 1024:.1f} |")
    linhas_out.append(
        f"| **total ({len(arquivos)} arquivos)** | **{total_linhas}** | "
        f"**{total_bytes / 1024:.1f}** |"
    )
    # tudo que existe em ui/ mas NAO e embutido (morto, ou servido de outro
    # jeito) -- vale registrar para nao mentir por omissao.
    todos = set((RAIZ / "crates" / "phxsql-server" / "ui").rglob("*"))
    todos_arq = {p for p in todos if p.is_file()}
    embutidos = set(arquivos)
    fora = sorted(todos_arq - embutidos)
    if fora:
        linhas_out.append("")
        linhas_out.append(
            f"Em `ui/` mas **fora** do `include_str!`/`include_bytes!` "
            f"({len(fora)} arquivos, não embutidos no binário):"
        )
        for f in fora:
            linhas_out.append(f"- `{f.relative_to(RAIZ)}`")
    return "\n".join(linhas_out)


def bloco_outras_linguagens() -> str:
    linhas_out = []
    linhas_out.append("| o que | onde | arquivos | linhas |")
    linhas_out.append("|---|---|---:|---:|")

    a, l = contar_arquivos(["*.mjs"], RAIZ / "testes-web")
    linhas_out.append(f"| JavaScript (prova ponta a ponta) | `testes-web/` | {a} | {l} |")

    a, l = contar_arquivos(["*.py"], RAIZ / "bancada")
    linhas_out.append(f"| Python (bancada de medicao) | `bancada/` | {a} | {l} |")

    a, l = contar_arquivos(["*.sh"], RAIZ, excluir=["/target/", "/.git/"])
    linhas_out.append(f"| Shell (empacotar, zelador, provas) | todo o repositorio | {a} | {l} |")

    a, l = contar_arquivos(["*.md"], RAIZ / "docs")
    linhas_out.append(f"| Markdown (documentacao tecnica) | `docs/` (nao recursivo em `dossie/`, `design/`, `video/`) | {a} | {l} |")

    a, l = contar_arquivos(["*.py"], RAIZ / "docs" / "dossie")
    linhas_out.append(f"| Python (geradores de dossie/pedidos) | `docs/dossie/` | {a} | {l} |")

    # C do lado do embutido, se houver
    a, l = contar_arquivos(["*.c", "*.h"], RAIZ / "bancada" / "embutido")
    if a:
        linhas_out.append(f"| C (prova da ABI embutida) | `bancada/embutido/` | {a} | {l} |")

    return "\n".join(linhas_out)


# =================================================== 3. DEPENDENCIAS


def bloco_dependencias() -> str:
    cargo_lock = RAIZ / "Cargo.lock"
    texto = ler(cargo_lock)
    nomes = re.findall(r'(?m)^name = "([^"]+)"', texto)
    fontes = re.findall(r'(?m)^source = ', texto)
    crates_dir = RAIZ / "crates"
    crates_workspace = sorted(
        p.name for p in crates_dir.iterdir() if (p / "Cargo.toml").exists()
    )
    externos = [n for n in nomes if n not in crates_workspace]
    linhas_out = []
    linhas_out.append(
        f"`Cargo.lock` lista **{len(nomes)}** pacotes. Todos: `{', '.join(nomes)}`."
    )
    linhas_out.append("")
    linhas_out.append(
        f"Nenhuma linha `source = ` no arquivo (contadas: {len(fontes)}) -- "
        "todo pacote e `path`, isto e, um crate deste proprio workspace. "
        f"Pacotes externos ao workspace: **{len(externos)}**."
    )
    linhas_out.append("")
    linhas_out.append("Confirmando pelo `[dependencies]` de cada `Cargo.toml`:")
    linhas_out.append("")
    linhas_out.append("| crate | dependencias (`[dependencies]`) |")
    linhas_out.append("|---|---|")
    for nome in crates_workspace:
        toml = ler(crates_dir / nome / "Cargo.toml")
        m = re.search(r"\[dependencies\]\n((?:.+\n?)*?)(?:\n\[|\Z)", toml)
        deps_txt = m.group(1) if m else ""
        deps = [
            l.split("=")[0].strip()
            for l in deps_txt.splitlines()
            if l.strip() and not l.strip().startswith("#")
        ]
        linhas_out.append(f"| `{nome}` | {', '.join(deps) if deps else '(nenhuma)'} |")
    return "\n".join(linhas_out)


# =================================================== 4. CRIPTO/FORMATO A MAO


# Caminhos relativos a RAIZ -- nao so phxsql-core: o SCRAM do cliente
# PostgreSQL(R) mora em phxsql-server/src/pg/.
ARQUIVOS_NORMA = [
    "crates/phxsql-core/src/sha1.rs",
    "crates/phxsql-core/src/sha512.rs",
    "crates/phxsql-core/src/hash.rs",
    "crates/phxsql-core/src/ed25519.rs",
    "crates/phxsql-core/src/x25519.rs",
    "crates/phxsql-core/src/hkdf.rs",
    "crates/phxsql-core/src/cifra.rs",
    "crates/phxsql-core/src/base64.rs",
    "crates/phxsql-core/src/uuid.rs",
    "crates/phxsql-core/src/crc.rs",
    "crates/phxsql-core/src/json.rs",
    "crates/phxsql-core/src/zip.rs",
    "crates/phxsql-server/src/pg/scram.rs",
]


def bloco_normas() -> str:
    linhas_out = []
    linhas_out.append("| arquivo | o que implementa | norma citada no proprio codigo | teste(s) que conferem |")
    linhas_out.append("|---|---|---|---|")
    for rel in ARQUIVOS_NORMA:
        caminho = RAIZ / rel
        # so o pedaco depois de .../src/, para distinguir hash.rs de
        # phxsql-core de um eventual homonimo em outro crate.
        nome = re.sub(r"^.*?/src/", "", rel)
        if not caminho.exists():
            continue
        texto = ler(caminho)
        # junta as linhas `//!` consecutivas do topo (o primeiro paragrafo
        # do doc de modulo), nao so a primeira -- muita frase aqui
        # continua na linha seguinte, e cortar no meio mentia por omissao.
        paragrafo = []
        for l in texto.splitlines():
            if l.startswith("//!"):
                conteudo = l[3:].strip()
                if not conteudo and paragrafo:
                    break
                if conteudo:
                    paragrafo.append(conteudo)
            elif paragrafo:
                break
        primeira_linha_doc = " ".join(paragrafo)
        normas = sorted(
            set(
                re.findall(
                    r"FIPS \d{3}-\d|RFC \d{3,4}|draft-irtf-cfrg-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*",
                    texto,
                )
            )
        )
        # nomes de teste que este projeto usa para "isto confere contra
        # norma publicada": vetor, rfc, fips, oficial, anexo (dos anexos
        # da RFC 5869), conhecid(o/os) -- todos vistos nos arquivos desta
        # lista. Vale so dentro de ARQUIVOS_NORMA, por isso a rede larga
        # nao pega nome de teste generico de outro lugar do projeto.
        testes = re.findall(
            r"fn (\w*(?:vetor|rfc|fips|oficial|anexo|conhecid)\w*)\s*\(\)",
            texto,
            re.I,
        )
        testes_fmt = ", ".join(f"`{t}`" for t in testes) if testes else "(nenhum teste com esse padrao de nome)"
        normas_fmt = ", ".join(normas) if normas else "(nenhuma citada)"
        linhas_out.append(
            f"| `{nome}` | {primeira_linha_doc or '(sem doc de modulo)'} "
            f"| {normas_fmt} | {testes_fmt} |"
        )
    return "\n".join(linhas_out)


# =================================================== 5. FERRAMENTAS


def bloco_catracas() -> str:
    server_src = RAIZ / "crates" / "phxsql-server" / "src"
    linhas_out = ["| constante | valor | arquivo |", "|---|---:|---|"]
    achados = []
    for rs in sorted(server_src.rglob("*.rs")):
        texto = ler(rs)
        for m in re.finditer(r"(?:pub )?const (TETO\w*)\s*:\s*\w+\s*=\s*([^;]+);", texto):
            achados.append((m.group(1), m.group(2).strip(), rs.relative_to(RAIZ)))
    for nome, valor, arq in achados:
        linhas_out.append(f"| `{nome}` | {valor} | `{arq}` |")
    linhas_out.append("")
    linhas_out.append(f"**{len(achados)}** catracas (`TETO*`) encontradas em `crates/phxsql-server/src/`.")
    return "\n".join(linhas_out)


def bloco_guardas() -> str:
    catalogo = RAIZ / "bancada" / "guardas" / "catalogo.py"
    if not catalogo.exists():
        return "Nao encontrado: `bancada/guardas/catalogo.py`."
    texto = ler(catalogo)
    m = re.search(r"GUARDAS\s*=\s*\[(.*)\]\s*$", texto, re.S)
    corpo = m.group(1) if m else texto
    n_defeitos = len(re.findall(r"\{\s*\n", corpo))
    return (
        f"`bancada/guardas/catalogo.py` cataloga **{n_defeitos}** defeitos "
        f"repostos (linhas do arquivo: {len(linhas(catalogo))}). Refazer a "
        f"prova: `python3 bancada/guardas/provar-guardas.py`."
    )


def bloco_bancadas() -> str:
    bancada_dir = RAIZ / "bancada"
    subdirs = sorted(p.name for p in bancada_dir.iterdir() if p.is_dir())
    com_leiame = [d for d in subdirs if (bancada_dir / d / "LEIA-ME.md").exists()]
    return (
        f"`bancada/` tem **{len(subdirs)}** frentes de medicao "
        f"({', '.join(sorted(subdirs))}), das quais **{len(com_leiame)}** "
        "documentam a propria metodologia em `LEIA-ME.md`."
    )


def bloco_conferidores() -> str:
    server_src = RAIZ / "crates" / "phxsql-server" / "src"
    achados = sorted(
        p.name for p in server_src.glob("conferidor*.rs")
    )
    examples_dir = RAIZ / "crates" / "phxsql-server" / "examples"
    provas = sorted(
        p.name
        for p in examples_dir.glob("*.rs")
        if "textos-fora" in p.name or "prova" in p.name or "grades-fora" in p.name
    )
    return (
        f"Conferidores em `crates/phxsql-server/src/`: {', '.join(f'`{a}`' for a in achados)}. "
        f"Executaveis de prova em `crates/phxsql-server/examples/`: "
        f"{', '.join(f'`{p}`' for p in provas)}."
    )


def bloco_testes_cargo() -> str | None:
    """Roda `cargo test --workspace` e soma os `test result:`.

    Devolve None se nao rodar (por exemplo, cargo ocupado por outra
    frente) -- quem chama tem de escrever "nao medido", nunca chutar."""
    # Sem --release: e o comando que o CLAUDE.md do projeto manda rodar
    # antes de commitar, entao e o numero que qualquer pessoa reproduz
    # sem esperar a compilacao otimizada.
    try:
        r = subprocess.run(
            ["cargo", "test", "--workspace"],
            cwd=RAIZ,
            capture_output=True,
            text=True,
            timeout=1800,
        )
    except Exception:  # pragma: no cover - so no ambiente do operador
        return None
    saida = r.stdout + r.stderr
    linhas_resultado = re.findall(
        r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", saida
    )
    if not linhas_resultado:
        return None
    total_passou = sum(int(p) for _, p, _, _ in linhas_resultado)
    total_falhou = sum(int(f) for _, _, f, _ in linhas_resultado)
    total_ignorado = sum(int(i) for _, _, _, i in linhas_resultado)
    linhas_out = []
    # Esta casa tem varias frentes mexendo na mesma arvore ao mesmo tempo.
    # Um `cargo test` que comeca no meio de uma gravacao de outra frente
    # aborta cedo (fail-fast) com poucos binarios e erro de compilacao --
    # numero real, mas da arvore instavel, nao do codigo. Reportar isso
    # como se fosse "979 passaram, 1 falhou" seria um numero medido que
    # mente por contexto faltando. Sinaliza em vez de fingir que e limpo.
    tem_erro_de_compilacao = bool(re.search(r"^error(\[E\d+\])?:", saida, re.M))
    suspeito = r.returncode != 0 and (tem_erro_de_compilacao or len(linhas_resultado) < 40)
    if suspeito:
        linhas_out.append(
            "**RODADA SUSPEITA, NAO USAR COMO NUMERO FINAL.** "
            f"`cargo test --workspace` terminou com codigo {r.returncode} "
            f"depois de so **{len(linhas_resultado)}** binarios "
            f"({total_passou} passaram, {total_falhou} falharam) -- "
            f"{'com erro de compilacao' if tem_erro_de_compilacao else 'bem menos que o esperado'}. "
            "Rodada normal tem dezenas de binarios; parar cedo e o padrao "
            "de pegar a arvore no meio da gravacao de outra frente "
            "(varios `.rs` estavam `M` no `git status` nesta rodada). "
            "Refazer com `cargo test --workspace` quando a arvore estiver parada."
        )
    else:
        linhas_out.append(
            f"`cargo test --workspace`: **{len(linhas_resultado)}** "
            f"binarios de teste, **{total_passou}** testes passaram, "
            f"{total_falhou} falharam, {total_ignorado} ignorados "
            f"(codigo de saida do processo: {r.returncode})."
        )
    return "\n".join(linhas_out)


# =================================================== 6. RECUSADO, COM NUMERO


LINHA_PEDIDO = re.compile(
    r"^\|\s*(☑️|◐|☐)\s*\|\s*(\d+)\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*$"
)


def ler_pedidos():
    fonte = RAIZ / "docs" / "PENDENCIAS.md"
    itens = []
    for l in linhas(fonte):
        m = LINHA_PEDIDO.match(l)
        if m:
            itens.append(
                {
                    "estado": m.group(1),
                    "n": int(m.group(2)),
                    "pedido": m.group(3),
                    "detalhe": m.group(4),
                }
            )
    return itens


def bloco_recusados() -> str:
    itens = ler_pedidos()
    recusados = [
        i for i in itens if "RECUSAD" in i["pedido"].upper() or "RECUSAD" in i["detalhe"].upper()
    ]
    linhas_out = []
    linhas_out.append(
        f"`docs/PENDENCIAS.md` tem **{len(itens)}** pedidos numerados; "
        f"**{len(recusados)}** trazem a palavra RECUSADO no proprio texto:"
    )
    linhas_out.append("")
    linhas_out.append("| # | pedido |")
    linhas_out.append("|---:|---|")
    for i in recusados:
        linhas_out.append(f"| {i['n']} | {i['pedido']} |")
    return "\n".join(linhas_out)


def bloco_dez_propostas() -> str:
    """Extrai a tabela «As dez propostas» de docs/DESEMPENHO.md ao vivo,
    em vez de copia-la -- se o documento mudar, a proxima geracao pega a
    mudanca."""
    fonte = RAIZ / "docs" / "DESEMPENHO.md"
    texto = ler(fonte)
    m = re.search(
        r"## 3\. As dez propostas.*?\n(\|.*?\n)(?=\n---|\n## )", texto, re.S
    )
    if not m:
        return "Nao encontrei a secao 'As dez propostas' em docs/DESEMPENHO.md."
    return m.group(1).rstrip()


def bloco_gpu_veredito() -> str:
    fonte = RAIZ / "docs" / "GPU.md"
    texto = ler(fonte)
    m = re.search(r"## 1\. O veredito.*?\n\s*\n((?:>.*\n)+)", texto)
    if not m:
        return "Nao encontrei o veredito em docs/GPU.md."
    return m.group(1).rstrip()


def bloco_transacoes_nao_entrou() -> str:
    fonte = RAIZ / "docs" / "TRANSACOES.md"
    texto = ler(fonte)
    m = re.search(
        r"(## 11\. O que NÃO entrou.*?)\n## 12\.", texto, re.S
    )
    if not m:
        return "Nao encontrei a secao 11 em docs/TRANSACOES.md."
    secao = m.group(1)
    subitens = re.findall(r"### (11\.\d+ .+)", secao)
    return "Subsecoes de `docs/TRANSACOES.md` §11 (\"O que NAO entrou, e o motivo de cada um\"):\n\n" + "\n".join(
        f"- {s}" for s in subitens
    )


def bloco_comparacao_fora() -> str:
    fonte = RAIZ / "docs" / "COMPARACAO.md"
    texto = ler(fonte)
    m = re.search(r"## O que ficou de fora, e por quê\n(.*?)\n## ", texto, re.S)
    if not m:
        return "Nao encontrei a secao em docs/COMPARACAO.md."
    return m.group(1).strip()


def _extrair_secao(fonte: Path, inicio_regex: str, fim_regex: str | None) -> str:
    """Devolve o texto entre um cabecalho e o proximo, ambos por regex de
    linha inteira, sem os proprios cabecalhos. `fim_regex=None` vai ate o
    fim do arquivo -- caso da ultima secao de um documento. Generico para
    nao repetir a mesma extracao ad hoc para cada documento-fonte."""
    texto = ler(fonte)
    fim = r"(?=^" + fim_regex + r")" if fim_regex else r"\Z"
    padrao = re.compile(r"^" + inicio_regex + r"\s*\n(.*?)" + fim, re.M | re.S)
    m = padrao.search(texto)
    if not m:
        return f"Nao encontrei a secao em {fonte.relative_to(RAIZ)}."
    return m.group(1).strip()


def bloco_empacotamento_zero_deps() -> str:
    return _extrair_secao(
        RAIZ / "docs" / "EMPACOTAMENTO.md",
        r"## 5\. Zero dependências externas, medido",
        r"---",
    )


def bloco_empacotamento_plataformas() -> str:
    return _extrair_secao(
        RAIZ / "docs" / "EMPACOTAMENTO.md",
        r"### 7\.7 Resumindo em uma tabela",
        None,
    )


# =================================================== 7. ORQUESTRACAO


def bloco_modelos() -> str:
    fonte = RAIZ / "docs" / "MODELOS.md"
    if not fonte.exists():
        return "docs/MODELOS.md nao existe."
    texto = ler(fonte)
    rodadas = re.findall(r"^### (.+)$", texto, re.M)
    return (
        f"`docs/MODELOS.md` registra **{len(rodadas)}** rodadas: "
        + "; ".join(rodadas)
    )


# ================================================================ MAIN


def main():
    print("# GERADO POR docs/tecnologias/extrair.py -- nao editar a mao\n")

    print("## Rust por crate\n")
    print(bloco_linguagens_rust())
    print()

    print("## Interface web embutida (fonte: http.rs)\n")
    print(bloco_interface())
    print()

    print("## Outras linguagens\n")
    print(bloco_outras_linguagens())
    print()

    print("## Dependencias (Cargo.lock + Cargo.toml de cada crate)\n")
    print(bloco_dependencias())
    print()

    print("## EMPACOTAMENTO.md secao 5 -- zero dependencias, medido\n")
    print(bloco_empacotamento_zero_deps())
    print()

    print("## EMPACOTAMENTO.md secao 7.7 -- plataformas\n")
    print(bloco_empacotamento_plataformas())
    print()

    print("## Normas conferidas por vetor oficial\n")
    print(bloco_normas())
    print()

    print("## Catracas (TETO*)\n")
    print(bloco_catracas())
    print()

    print("## Guardas (defeitos repostos)\n")
    print(bloco_guardas())
    print()

    print("## Bancadas de medicao\n")
    print(bloco_bancadas())
    print()

    print("## Conferidores e provas\n")
    print(bloco_conferidores())
    print()

    print("## Testes (cargo test --workspace)\n")
    r = bloco_testes_cargo()
    print(r if r else "NAO MEDIDO nesta rodada (ver motivo no relatorio).")
    print()

    print("## Pedidos recusados, com numero\n")
    print(bloco_recusados())
    print()

    print("## As dez propostas de DESEMPENHO.md\n")
    print(bloco_dez_propostas())
    print()

    print("## Veredito de GPU.md\n")
    print(bloco_gpu_veredito())
    print()

    print("## TRANSACOES.md paragrafo 11\n")
    print(bloco_transacoes_nao_entrou())
    print()

    print("## COMPARACAO.md: o que ficou de fora\n")
    print(bloco_comparacao_fora())
    print()

    print("## MODELOS.md\n")
    print(bloco_modelos())
    print()


if __name__ == "__main__":
    main()
