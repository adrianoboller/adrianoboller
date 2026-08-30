# Align the script with the published recipe
# 28/08 18:17

import io
p='docs/dossie/numeros-do-projeto.py'
s=io.open(p,encoding='utf-8').read()

velho = '''    linhas de Rust    todo `.rs` de `crates/`, testes inclusive, `target/` fora
    testes            o que `cargo test --workspace` REPORTA como passado
    crates            pastas em `crates/`
    arquivos/tabela   quantos arquivos fisicos uma tabela tem, sem o espelho
    linhas de doc     `docs/*.md` + README + CHANGELOG + MANUAL.txt
    interface         o tamanho do `ui/index.html`, em KiB de 1024 bytes
    versao            o `version` do Cargo.toml do workspace'''
novo = '''    linhas de Rust    todo `.rs` de `crates/`, testes inclusive, `target/` fora
    testes            o que `cargo test --workspace` REPORTA como passado
    dependencias      pacotes do Cargo.lock menos os quatro crates daqui
    crates            pastas em `crates/`
    arquivos/tabela   quantos arquivos fisicos uma tabela tem, sem o espelho
    linhas de doc     ver DOCS abaixo
    interface         ver INTERFACE abaixo -- sao TRES arquivos, nao um
    versao            o `version` do Cargo.toml do workspace

As duas ultimas seguem a receita publicada em `docs/dossie/LEIA-ME.md`, e nao a
que fosse mais comoda de escrever aqui: um numero de vitrine que ninguem
consegue reproduzir com a receita ao lado dele e um numero errado, ainda que a
conta esteja certa. Mexeu numa, mexa na outra.'''
assert velho in s
s = s.replace(velho, novo, 1)

velho2 = '''def linhas_de_doc() -> int:
    arquivos = sorted((RAIZ / "docs").glob("*.md"))
    arquivos += [RAIZ / "README.md", RAIZ / "CHANGELOG.md", RAIZ / "MANUAL.txt"]
    total = 0
    for f in arquivos:
        if f.exists():
            total += len(f.read_text(encoding="utf-8", errors="replace").splitlines())
    return total


def kib_da_interface() -> int:
    """KiB de 1024 bytes, arredondado para BAIXO.

    Para baixo de proposito: arredondar um numero de vitrine para cima ja foi
    um dos erros que este script existe para nao repetir.
    """
    return (RAIZ / "crates/phxsql-server/ui/index.html").stat().st_size // 1024'''

novo2 = '''# A receita do LEIA-ME, na letra. Documento novo entra nos DOIS lugares.
DOCS_AVULSOS = (
    "README.md", "CHANGELOG.md", "MANUAL.txt",
    "bancada/LEIA-ME.md", "marca/LEIA-ME.md", "docs/dossie/LEIA-ME.md",
)

# A interface sao os TRES arquivos que o `http.rs` embute com `include_str!`.
# Contar so o index.html daria um numero menor que o publicado, e ninguem
# conseguiria reproduzir o rodape.
INTERFACE = (
    "crates/phxsql-server/ui/index.html",
    "crates/phxsql-server/ui/grid/phx-grid.css",
    "crates/phxsql-server/ui/grid/phx-grid.js",
)


def linhas_de_doc() -> int:
    arquivos = sorted((RAIZ / "docs").glob("*.md"))
    arquivos += [RAIZ / nome for nome in DOCS_AVULSOS]
    faltando = [f for f in arquivos if not f.exists()]
    if faltando:
        sys.exit(
            "a receita de linhas de doc aponta para arquivo que nao existe: "
            + ", ".join(str(f.relative_to(RAIZ)) for f in faltando)
        )
    return sum(
        len(f.read_text(encoding="utf-8", errors="replace").splitlines())
        for f in arquivos
    )


def dependencias_externas() -> int:
    """Pacotes do Cargo.lock menos os crates deste projeto.

    Tem de dar zero. Se der outra coisa, entrou dependencia -- e a capa vai
    dizer isso em vez de continuar publicando o zero por habito.
    """
    lock = (RAIZ / "Cargo.lock").read_text(encoding="utf-8")
    return lock.count("[[package]]") - quantos_crates()


def kib_da_interface() -> int:
    """KiB de 1024 bytes, arredondado para BAIXO.

    Para baixo de proposito: arredondar um numero de vitrine para cima ja foi
    um dos erros que este script existe para nao repetir.
    """
    total = 0
    for nome in INTERFACE:
        f = RAIZ / nome
        if not f.exists():
            sys.exit(f"a receita da interface aponta para {nome}, que nao existe")
        total += f.stat().st_size
    return total // 1024'''
assert velho2 in s
s = s.replace(velho2, novo2, 1)

velho3 = '''        "versao": versao(),
        "arquivos": len(ARQUIVOS_POR_TABELA),
    }'''
novo3 = '''        "versao": versao(),
        "arquivos": len(ARQUIVOS_POR_TABELA),
        "deps": dependencias_externas(),
    }'''
assert velho3 in s
s = s.replace(velho3, novo3, 1)

s = s.replace('''    <div><div class="v">0</div><div class="r">dependências</div></div>''',
              '''    <div><div class="v">{n['deps']}</div><div class="r">dependências</div></div>''', 1)
s = s.replace('''  mais {n['kib']} KiB de interface · {milhar(n['testes'])} testes · nenhuma dependência externa.''',
              '''  mais {n['kib']} KiB de interface · {milhar(n['testes'])} testes ·
  {'nenhuma dependência externa' if n['deps'] == 0 else str(n['deps']) + ' dependências externas'}.''', 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
