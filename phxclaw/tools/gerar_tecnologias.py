#!/usr/bin/env python3
"""Regrava os blocos medidos de `docs/TECNOLOGIAS.md` (entre `<!-- gerado:tec:NOME:inicio/fim -->`).

Nenhum numero do documento se digita: linhas por linguagem, Rust codigo x teste, crates,
pacotes do `Cargo.lock`, dependencias diretas do workspace, atributos de teste e os
geradores da pasta `tools/` saem daqui, da arvore em que o script roda. So biblioteca padrao.
Diz no terminal o que mediu; marcador que nao existe no documento e erro, nao silencio.
"""
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DOC = RAIZ / "docs" / "TECNOLOGIAS.md"
# Fora da conta: artefatos de compilacao, dependencias baixadas, fontes de terceiros vendidos
# e a pasta privada. A lista e de PASTAS, nao de arquivos, para nao envelhecer a cada arquivo novo.
FORA = {"target", "node_modules", "third_party", "private", ".git"}
EXT = ["rs", "js", "mjs", "py", "ts", "html", "css", "sh"]


def arquivos(ext):
    for p in RAIZ.rglob(f"*.{ext}"):
        if FORA.isdisjoint(p.relative_to(RAIZ).parts):
            yield p


def linhas(p):
    try:
        return sum(1 for _ in p.open("rb"))
    except OSError:
        return 0


def bloco_linguagens():
    out = ["| linguagem | arquivos | linhas | onde (pastas com mais linhas) |", "|---|---:|---:|---|"]
    tot_a = tot_l = 0
    for e in EXT:
        por_pasta = {}
        n = 0
        for p in arquivos(e):
            n += 1
            top = p.relative_to(RAIZ).parts[0] if len(p.relative_to(RAIZ).parts) > 1 else "."
            por_pasta[top] = por_pasta.get(top, 0) + linhas(p)
        l = sum(por_pasta.values())
        tot_a += n
        tot_l += l
        onde = ", ".join(f"`{k}` {v}" for k, v in sorted(por_pasta.items(), key=lambda kv: -kv[1])[:3])
        out.append(f"| `.{e}` | {n} | {l} | {onde} |")
    out.append(f"| **total** | **{tot_a}** | **{tot_l}** | |")
    return "\n".join(out)


def bloco_rust():
    src = tst = ex = 0
    for p in arquivos("rs"):
        partes = p.relative_to(RAIZ).parts
        if "tests" in partes:
            tst += linhas(p)
        elif "examples" in partes:
            ex += linhas(p)
        else:
            src += linhas(p)
    attrs = 0
    for p in arquivos("rs"):
        attrs += len(re.findall(r"#\[(?:tokio::)?test\]", p.read_text(errors="replace")))
    crates = sorted(d.name for d in (RAIZ / "crates").iterdir() if (d / "Cargo.toml").exists())
    lock = (RAIZ / "Cargo.lock").read_text().count("[[package]]")
    deps = re.findall(r"^([a-z0-9_-]+)\s*=", _secao_toml("workspace.dependencies"), re.M)
    return "\n".join([
        f"- Rust: **{src}** linhas fora de `tests/` e `examples/`, **{tst}** em `tests/` de integracao, "
        f"**{ex}** em `examples/`; proporcao teste/codigo (so integracao) {tst}/{src} = {tst / src:.2f}x.",
        f"- **{len(crates)}** crates em `crates/` + os binarios em `apps/`.",
        f"- **{attrs}** funcoes marcadas `#[test]`/`#[tokio::test]`.",
        f"- **{len(deps)}** dependencias diretas no `[workspace.dependencies]` do `Cargo.toml`: "
        + ", ".join(f"`{d}`" for d in deps) + ".",
        f"- **{lock}** pacotes no `Cargo.lock` (a arvore inteira, transitivas incluidas).",
    ])


def _secao_toml(nome):
    t = (RAIZ / "Cargo.toml").read_text()
    m = re.search(rf"^\[{re.escape(nome)}\]\n(.*?)(?=^\[|\Z)", t, re.M | re.S)
    return m.group(1) if m else ""


def bloco_geradores():
    out = ["| gerador | o que escreve |", "|---|---|"]
    for p in sorted(list((RAIZ / "tools").glob("gerar_*.py")) + list((RAIZ / "tools" / "dossie").glob("*.py"))
                    + list((RAIZ / "docs" / "absorcao").glob("gerar_*.py"))):
        doc = ""
        for ln in p.read_text(errors="replace").splitlines()[:12]:
            s = ln.strip().strip('"').strip("'").lstrip("#").strip()
            if s and not s.startswith("!"):
                doc = s
                break
        out.append(f"| `{p.relative_to(RAIZ)}` | {doc[:140]} |")
    return "\n".join(out)


def bloco_git():
    def g(*a):
        return subprocess.run(["git", *a], cwd=RAIZ, capture_output=True, text=True).stdout.strip()
    return (f"Medido em {date.today().isoformat()} na arvore de trabalho sobre o commit `{g('rev-parse', '--short', 'HEAD')}` "
            f"(branch `{g('rev-parse', '--abbrev-ref', 'HEAD')}`), por `python3 tools/gerar_tecnologias.py`. "
            f"Arquivos modificados e nao comitados no momento da medida: {len(g('status', '--porcelain').splitlines())}.")


BLOCOS = {"carimbo": bloco_git, "linguagens": bloco_linguagens, "rust": bloco_rust, "geradores": bloco_geradores}


def main():
    if not DOC.exists():
        sys.exit(f"nao existe {DOC}: o extrator so regrava blocos, nao cria o documento")
    s = DOC.read_text(encoding="utf-8")
    faltam = []
    for nome, fn in BLOCOS.items():
        ini, fim = f"<!-- gerado:tec:{nome}:inicio -->", f"<!-- gerado:tec:{nome}:fim -->"
        if ini not in s or fim not in s:
            faltam.append(nome)
            continue
        a, b = s.index(ini) + len(ini), s.index(fim)
        s = s[:a] + "\n" + fn() + "\n" + s[b:]
        print(f"ok  {nome}")
    DOC.write_text(s, encoding="utf-8")
    if faltam:
        print("FEZ MENOS DO QUE O NOME PROMETE: sem marcador no documento ->", ", ".join(faltam))
        sys.exit(2)


if __name__ == "__main__":
    main()
