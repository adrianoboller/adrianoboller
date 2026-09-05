#!/usr/bin/env python3
"""Atualiza todas as paginas de documentacao de uma vez.

Existe por causa de um achado da revisao: paginas geradas uma vez e nunca mais
(organograma, fluxo, ativacao, evolucao) ficaram carimbadas numa versao velha
enquanto o plugin andava. A regra do projeto ja dizia que numero visivel sai de
gerador; faltava UM comando que rodasse todos os geradores e carimbasse o resto.

Ordem: mede os numeros, regenera o que tem gerador proprio, carimba a versao no
que e HTML estatico, e avisa o que sobrou desatualizado.

Uso: python3 docs/dossie/atualizar-paginas.py [--conferir]
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
DOSSIE = RAIZ / "docs/dossie"

# HTML sem gerador proprio: so o selo de versao e carimbado
CARIMBAR = [
    (DOSSIE / "fluxo-atual.html", r"(Versão )\d+\.\d+\.\d+"),
    (RAIZ / "docs/ativacao-do-serial.html", r"(WX Claude Code )\d+\.\d+\.\d+(?= · instrução)"),
]
GERADORES = ["numeros-do-plugin.py", "gerar-organograma.py", "gerar-evolucao.py", "gerar-comandos.py",
             "gerar-relatorio-cenarios.py", "gerar-dossie.py"]


def main() -> int:
    conferir = "--conferir" in sys.argv
    versao = json.loads((RAIZ / ".claude-plugin/plugin.json").read_text(encoding="utf-8"))["version"]
    problemas = []

    for g in GERADORES:
        script = DOSSIE / g
        if not script.is_file():
            problemas.append(f"gerador ausente: {g}")
            continue
        if conferir:
            print(f"  rodaria {g}")
            continue
        args = [sys.executable, str(script)]
        if g == "numeros-do-plugin.py":
            args.append(str(DOSSIE / "dossie-wx-claude-code.html"))
        r = subprocess.run(args, capture_output=True, text=True, cwd=RAIZ)
        if r.returncode:
            problemas.append(f"{g} falhou: {r.stderr.strip()[:200]}")
        else:
            print(f"  ok {g}")

    for arq, rx in CARIMBAR:
        if not arq.is_file():
            problemas.append(f"pagina ausente: {arq.name}")
            continue
        texto = arq.read_text(encoding="utf-8")
        novo, n = re.subn(rx, lambda m: m.group(1) + versao, texto)
        if n == 0:
            problemas.append(f"{arq.name}: nao achei o selo de versao ({rx})")
        elif novo != texto:
            if conferir:
                print(f"  carimbaria {arq.name} para {versao}")
                problemas.append(f"{arq.name} esta numa versao antiga")
            else:
                arq.write_text(novo, encoding="utf-8")
                print(f"  ok {arq.name} carimbado {versao}")
        else:
            print(f"  ok {arq.name} ja em {versao}")

    # varredura final: HTML de documentacao com selo de versao velho
    for html in list(DOSSIE.glob("*.html")) + [RAIZ / "docs/ativacao-do-serial.html"]:
        if not html.is_file():
            continue
        cabecalho = html.read_text(encoding="utf-8")[:4000]
        for m in re.finditer(r"(?:WX Claude Code|Versão) (\d+\.\d+\.\d+)", cabecalho):
            if m.group(1) != versao:
                problemas.append(f"{html.name}: cabecalho ainda diz {m.group(1)} (atual {versao})")

    if problemas:
        print("\nPENDENTE:")
        for p in problemas:
            print("  -", p)
        return 1
    print(f"\ntudo em {versao}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
