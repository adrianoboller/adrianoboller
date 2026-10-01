#!/usr/bin/env python3
"""Extrator do kit portatil: acha as cognicoes que JA trazem evidencia no texto.

    python3 kit-portatil/extrair.py                 # grava kit-portatil/aprendizados/CANDIDATAS.md
    python3 kit-portatil/extrair.py --stdout        # imprime, nao grava
    python3 kit-portatil/extrair.py --cognicao DIR --codigo DIR --git DIR

O que ele faz (a parte MECANICA; a curadoria final e humana):

  1. le cada `cognicao_*.md` da pasta de cognicao (so le, nunca escreve nela);
  2. tira o titulo e o estado (`**Estado:**`; sem a linha = PENDENTE);
  3. procura, no TEXTO INTEIRO, referencias verificaveis entre crases, e
     CONFERE cada uma:
       - nome de teste que existe como `#[test] fn` no codigo;
       - hash de commit que existe no git (`git cat-file -e <h>^{commit}`);
       - caminho de arquivo (medicao, script, teste) que existe no disco;
  4. procura a frase da prova nos dois sentidos («defeito reposto»,
     «mutante», «falha sem», «fica vermelho»...);
  5. classifica: FORTE = referencia conferida E frase dos dois sentidos;
     REFERENCIA = so a referencia; NADA = nenhuma.

Por que conferir e nao so casar o padrao: citar um nome de teste inventado e o
mesmo que nao ter evidencia. Hash ou nome que nao existe nao conta.

Ele NAO promove nada. Estado de cognicao so muda no arquivo de origem, por
quem tem a prova.
"""
import argparse
import os
import re
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(AQUI)

DOIS_SENTIDOS = re.compile(
    r"defeito reposto|reposto o defeito|repondo o defeito|com o defeito|mutante|"
    r"falha sem|falhou sem|ficam? vermelho|reprova(?:va)? com|passa com o conserto",
    re.I)
CRASE = re.compile(r"`([^`\n]{3,200})`")
HASH = re.compile(r"^[0-9a-f]{7,40}$")
NOME_TESTE = re.compile(r"(?:^|::)([a-z_][a-z0-9_]{6,})$")
ARQUIVO = re.compile(r"\.(?:json|py|mjs|sh|rs|log|txt)$")
TESTE_RS = re.compile(
    r"#\[(?:\w+::)?test\][^\n]*\n(?:\s*#\[[^\n]*\n)*\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")


def testes_existentes(codigo):
    """Todo `fn nome` logo depois de `#[test]`, em qualquer `.rs` do codigo."""
    nomes = set()
    for raiz, dirs, arqs in os.walk(codigo):
        dirs[:] = [d for d in dirs if d not in ("target", ".git", "node_modules", "worktrees")]
        for a in arqs:
            if a.endswith(".rs"):
                try:
                    with open(os.path.join(raiz, a), encoding="utf-8", errors="replace") as fh:
                        nomes.update(TESTE_RS.findall(fh.read()))
                except OSError:
                    pass
    return nomes


_CACHE_COMMIT = {}


def commit_existe(gitdir, h):
    if h not in _CACHE_COMMIT:
        r = subprocess.run(["git", "-C", gitdir, "cat-file", "-e", h + "^{commit}"],
                           capture_output=True)
        _CACHE_COMMIT[h] = r.returncode == 0
    return _CACHE_COMMIT[h]


def ler(caminho):
    with open(caminho, encoding="utf-8", errors="replace") as fh:
        texto = fh.read()
    titulo = next((l[2:].strip() for l in texto.splitlines() if l.startswith("# ")),
                  os.path.basename(caminho))
    m = re.search(r"^\*\*Estado:\*\*\s*(\S+)", texto, re.M)
    estado = m.group(1).upper().rstrip(".") if m else "PENDENTE"
    return texto, titulo, estado


def classificar(texto, testes, gitdir, raizes):
    refs = []
    for tok in CRASE.findall(texto):
        tok = tok.strip()
        if " " in tok:
            continue
        if HASH.match(tok) and not tok.isdigit():
            if commit_existe(gitdir, tok):
                refs.append(("commit", tok))
            continue
        m = NOME_TESTE.search(tok)
        if m and m.group(1) in testes:
            refs.append(("teste", m.group(1)))
            continue
        if "/" in tok and ARQUIVO.search(tok):
            if any(os.path.exists(os.path.join(r, tok)) for r in raizes):
                refs.append(("arquivo", tok))
    unicos = list(dict.fromkeys(refs))
    frase = DOIS_SENTIDOS.search(texto)
    if unicos and frase:
        nivel = "FORTE"
    elif unicos:
        nivel = "REFERENCIA"
    else:
        nivel = "NADA"
    return nivel, unicos, (frase.group(0) if frase else "")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cognicao", default=os.path.join(REPO, "phxsql", "docs", "cognicao"))
    ap.add_argument("--codigo", default=os.path.join(REPO, "phxsql"))
    ap.add_argument("--git", default=REPO)
    ap.add_argument("--saida", default=os.path.join(AQUI, "aprendizados", "CANDIDATAS.md"))
    ap.add_argument("--stdout", action="store_true")
    a = ap.parse_args()

    testes = testes_existentes(a.codigo)
    raizes = [a.codigo, REPO]
    arqs = sorted(f for f in os.listdir(a.cognicao)
                  if f.startswith("cognicao_") and f.endswith(".md"))
    linhas, por_nivel, por_estado = [], {}, {}
    for f in arqs:
        texto, titulo, estado = ler(os.path.join(a.cognicao, f))
        nivel, refs, frase = classificar(texto, testes, a.git, raizes)
        por_nivel[nivel] = por_nivel.get(nivel, 0) + 1
        por_estado[estado] = por_estado.get(estado, 0) + 1
        if nivel == "NADA":
            continue
        rel = os.path.relpath(os.path.join(a.cognicao, f), os.path.dirname(os.path.abspath(a.saida)))
        mostra = "; ".join("%s `%s`" % r for r in refs[:4])
        if len(refs) > 4:
            mostra += " (+%d)" % (len(refs) - 4)
        linhas.append((nivel != "FORTE", f, "| %s | %s | [%s](%s) | %s | %s |" % (
            nivel, estado, titulo.replace("|", "/"), rel, mostra.replace("|", "/"), frase)))
    linhas.sort()

    corpo = [
        "# Candidatas: cognicoes com evidencia conferida no texto",
        "",
        "<!-- GERADO por kit-portatil/extrair.py -- NAO EDITE A MAO. -->",
        "",
        "Varridas: %d cognicoes em `%s`; %d testes `#[test]` conhecidos no codigo." % (
            len(arqs), os.path.relpath(a.cognicao, REPO), len(testes)),
        "",
        "Estado declarado nos arquivos: " +
        ", ".join("%s %d" % kv for kv in sorted(por_estado.items())) + ".",
        "",
        "Nivel da evidencia achada: FORTE %d (referencia conferida + frase dos dois sentidos), "
        "REFERENCIA %d (so referencia conferida), NADA %d." % (
            por_nivel.get("FORTE", 0), por_nivel.get("REFERENCIA", 0), por_nivel.get("NADA", 0)),
        "",
        "Isto e a triagem MECANICA. A lista curada, com o «por que serve fora», e "
        "`SELECIONADAS.md`. Nada aqui promove estado no projeto de origem.",
        "",
        "| nivel | estado | cognicao | referencias conferidas | frase dos dois sentidos |",
        "|---|---|---|---|---|",
    ] + [l for _, _, l in linhas]
    out = "\n".join(corpo) + "\n"
    if a.stdout:
        sys.stdout.write(out)
    else:
        with open(a.saida, "w", encoding="utf-8") as fh:
            fh.write(out)
    print("cognicoes %d | FORTE %d | REFERENCIA %d | NADA %d | testes conhecidos %d" % (
        len(arqs), por_nivel.get("FORTE", 0), por_nivel.get("REFERENCIA", 0),
        por_nivel.get("NADA", 0), len(testes)), file=sys.stderr)


if __name__ == "__main__":
    main()
