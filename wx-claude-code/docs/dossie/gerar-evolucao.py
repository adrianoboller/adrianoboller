#!/usr/bin/env python3
"""Mede a evolucao do plugin no git, versao a versao, e regenera evolucao.json.

Cada ponto e contado NO COMMIT daquela versao, com git ls-tree e git show: nada
de estimativa. Roda de novo a cada versao nova; antes disso a pagina parava na
3.17.0 enquanto o plugin ia na 3.25.0.

Uso: python3 docs/dossie/gerar-evolucao.py
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
PLUGIN = "wx-claude-code"


def git(*args: str) -> str:
    return subprocess.run(["git", *args], capture_output=True, text=True, cwd=RAIZ.parent).stdout


def conta(commit: str, padrao: str) -> int:
    saida = git("ls-tree", "-r", "--name-only", commit, f"{PLUGIN}/")
    return len([l for l in saida.splitlines() if re.fullmatch(padrao, l)])


def texto(commit: str, caminho: str) -> str:
    return git("show", f"{commit}:{PLUGIN}/{caminho}")


def main() -> int:
    log = git("log", "--date=short", "--format=%H|%ad|%s", "--", PLUGIN).splitlines()
    vistos, versoes = set(), []
    for linha in reversed(log):
        h, d, s = linha.split("|", 2)
        m = re.match(r"^(\d+\.\d+\.\d+): (.+)$", s)
        if m and m.group(1) not in vistos:
            vistos.add(m.group(1))
            versoes.append({"commit": h, "data": d, "versao": m.group(1), "titulo": m.group(2)})
    linhas = []
    for v in versoes:
        c = v["commit"]
        arquivos_py = [l for l in git("ls-tree", "-r", "--name-only", c, f"{PLUGIN}/").splitlines()
                       if l.endswith(".py") and ("/scripts/" in l or "/hooks/" in l)]
        linhas_py = 0
        for a in arquivos_py:
            linhas_py += len(git("show", f"{c}:{a}").splitlines())
        testes = len(re.findall(r"^\s+def test_", texto(c, "tests/testes.py"), re.M))
        try:
            modelo = json.loads(texto(c, "skills/conversao-wx/templates/questionario.json"))
            itens = len([k for k in modelo if k not in ("schema_version", "respondido_em")])
            for bloco in modelo.values():
                if isinstance(bloco, dict):
                    itens += len([k for k in bloco if re.match(r"^(0_\d+_|F\d+_|K\d+_|L\d+_)", k)])
        except (json.JSONDecodeError, ValueError):
            itens = 0
        try:
            hooks = sum(len(x) for x in json.loads(texto(c, "hooks/hooks.json"))["hooks"].values())
        except (json.JSONDecodeError, ValueError, KeyError):
            hooks = 0
        linhas.append({
            "versao": v["versao"], "data": v["data"], "titulo": v["titulo"],
            "agentes": conta(c, rf"{PLUGIN}/agents/.+\.md"),
            "skills": conta(c, rf"{PLUGIN}/skills/[^/]+/SKILL\.md"),
            "comandos": conta(c, rf"{PLUGIN}/commands/.+\.md"),
            "scripts": conta(c, rf"{PLUGIN}/skills/conversao-wx/scripts/.+\.py"),
            "linhas_py": linhas_py,
            "prints": conta(c, rf"{PLUGIN}/docs/prints/.+\.png"),
            "testes": testes, "itens_questionario": itens, "hooks": hooks,
            "manual": len(texto(c, "MANUAL.md").splitlines()),
        })
        print(f"  {v['versao']:<8} {v['data']}  {linhas[-1]['agentes']:>3} agentes  {testes:>3} testes")
    (RAIZ / "docs/dossie/evolucao.json").write_text(json.dumps(linhas, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    pagina = RAIZ / "docs/dossie/evolucao.html"
    if pagina.is_file():
        html = pagina.read_text(encoding="utf-8")
        novo, n = re.subn(r"const D = \[.*?\];", "const D = " + json.dumps(linhas, ensure_ascii=False) + ";", html, flags=re.S)
        if n != 1:
            print("aviso: nao achei o bloco 'const D = [...]' em evolucao.html; a pagina nao foi atualizada")
        else:
            # o texto do cabecalho tambem conta versoes
            novo = re.sub(r"\b\d+ versões em", f"{len(linhas)} versões em", novo)
            pagina.write_text(novo, encoding="utf-8")
            print(f"  ok evolucao.html ({len(linhas)} versoes)")
    print(f"\nok evolucao.json ({len(linhas)} versoes, da {linhas[0]['versao']} a {linhas[-1]['versao']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
