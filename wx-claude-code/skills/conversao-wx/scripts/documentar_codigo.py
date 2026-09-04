#!/usr/bin/env python3
"""Documentador: extrai de cada funcao do codigo a finalidade, os parametros, o
processamento e os resultados possiveis, e gera docs/funcoes.md, docs/funcoes.html
e docs/indice.json (indexavel por outros desenvolvedores e por outras IAs).

Le Python, Rust, TypeScript/JavaScript, C#, Go e Java por assinatura e pelo
comentario imediatamente acima (docstring, ///, /** */, //). O que o codigo nao
diz fica «(nao documentado)», nunca inventado: o Documentador (agente C) e quem
completa lendo o corpo.

Uso: documentar_codigo.py --codigo <dir> --saida <dir/docs> [--projeto nome]
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from datetime import date
from pathlib import Path

PADROES = {
    ".py": re.compile(r"^(?P<ind>[ \t]*)def\s+(?P<nome>\w+)\s*\((?P<params>[^)]*)\)\s*(->\s*(?P<ret>[^:]+))?:", re.M),
    ".rs": re.compile(r"^(?P<ind>[ \t]*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(?P<nome>\w+)\s*(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)\s*(->\s*(?P<ret>[^{]+))?\{", re.M),
    ".ts": re.compile(r"^(?P<ind>[ \t]*)(?:export\s+)?(?:async\s+)?function\s+(?P<nome>\w+)\s*(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)\s*(:\s*(?P<ret>[^{]+))?\{", re.M),
    ".cs": re.compile(r"^(?P<ind>[ \t]*)(?:public|private|protected|internal|static|\s)+\s*(?P<ret>[\w<>\[\],\s]+?)\s+(?P<nome>\w+)\s*\((?P<params>[^)]*)\)\s*(?:where[^{]*)?\{", re.M),
    ".go": re.compile(r"^(?P<ind>)func\s+(?:\([^)]*\)\s*)?(?P<nome>\w+)\s*\((?P<params>[^)]*)\)\s*(?P<ret>[^{]*)\{", re.M),
    ".java": re.compile(r"^(?P<ind>[ \t]*)(?:public|private|protected|static|final|\s)+\s*(?P<ret>[\w<>\[\],\s]+?)\s+(?P<nome>\w+)\s*\((?P<params>[^)]*)\)\s*(?:throws[^{]*)?\{", re.M),
}
PADROES[".js"] = PADROES[".ts"]
EXCLUIR = {".git", "target", "node_modules", "__pycache__", "dist", "build", ".venv", "venv", "vendor", "bin", "obj"}


def comentario_acima(texto: str, pos: int) -> str:
    """O comentario imediatamente acima da assinatura, em qualquer das sintaxes."""
    antes = texto[:pos].rstrip("\n").split("\n")
    linhas = []
    for l in reversed(antes):
        s = l.strip()
        if s.startswith(("///", "//!", "//", "#", "*", "/**", "*/")) or s.startswith("'''") or s.startswith('"""'):
            linhas.append(re.sub(r"^(///|//!|//|#|/\*\*|\*/|\*)\s?", "", s).strip('"\' '))
        elif s.endswith(("*/", '"""', "'''")):
            linhas.append(re.sub(r"(\*/|\"\"\"|''')$", "", s).strip())
        elif s == "":
            if linhas:
                break
            continue
        else:
            break
    texto_doc = " ".join(x for x in reversed(linhas) if x and x not in ("/**", "*/")).strip()
    return re.sub(r"\s*\*/\s*$", "", re.sub(r"^/\*\*?\s*", "", texto_doc)).strip()


def docstring_python(texto: str, fim: int) -> str:
    m = re.match(r"\s*(?:\"\"\"|''')(.*?)(?:\"\"\"|''')", texto[fim:], re.S)
    return " ".join(m.group(1).split()) if m else ""


def extrair(codigo: Path) -> list[dict]:
    itens = []
    for f in sorted(codigo.rglob("*")):
        if not f.is_file() or f.suffix not in PADROES or set(f.relative_to(codigo).parts) & EXCLUIR:
            continue
        try:
            texto = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for m in PADROES[f.suffix].finditer(texto):
            nome = m.group("nome")
            if nome in ("if", "for", "while", "switch", "catch", "return") or nome.startswith("__") and f.suffix != ".py":
                continue
            doc = docstring_python(texto, m.end()) if f.suffix == ".py" else ""
            doc = doc or comentario_acima(texto, m.start())
            params = [p.strip() for p in (m.group("params") or "").split(",") if p.strip() and p.strip() not in ("self", "cls", "&self", "&mut self", "self", "mut self")]
            ret = (m.group("ret") or "").strip() if "ret" in m.groupdict() else ""
            itens.append({"arquivo": str(f.relative_to(codigo)), "linha": texto.count("\n", 0, m.start()) + 1, "funcao": nome, "linguagem": f.suffix.lstrip("."),
                          "parametros": params, "retorno": ret or "(nao declarado)", "finalidade": doc or "(nao documentado)",
                          "processamento": "(nao documentado)", "resultados_possiveis": "(nao documentado)"})
    return itens


def gerar(itens: list[dict], saida: Path, projeto: str) -> None:
    saida.mkdir(parents=True, exist_ok=True)
    hoje = date.today().isoformat()
    sem_doc = sum(1 for i in itens if i["finalidade"] == "(nao documentado)")
    idx = {"projeto": projeto, "gerado_em": hoje, "funcoes": len(itens), "sem_finalidade": sem_doc,
           "indice": [{"id": f"{i['arquivo']}#{i['funcao']}", "funcao": i["funcao"], "arquivo": i["arquivo"], "linha": i["linha"], "linguagem": i["linguagem"], "parametros": i["parametros"], "retorno": i["retorno"], "finalidade": i["finalidade"], "processamento": i["processamento"], "resultados_possiveis": i["resultados_possiveis"]} for i in itens]}
    (saida / "indice.json").write_text(json.dumps(idx, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    md = [f"# Funções de {projeto}", "", f"{len(itens)} funções em {len({i['arquivo'] for i in itens})} arquivos; {sem_doc} sem finalidade documentada (o Documentador completa lendo o corpo). Gerado em {hoje} por `documentar_codigo.py`; índice indexável em `indice.json`.", "",
          "| função | arquivo | parâmetros | retorno | finalidade |", "| --- | --- | --- | --- | --- |"]
    md += [f"| `{i['funcao']}` | `{i['arquivo']}:{i['linha']}` | {', '.join(f'`{p}`' for p in i['parametros']) or '—'} | `{i['retorno']}` | {i['finalidade'].replace('|', '/')} |" for i in itens]
    md += ["", "## Por função", ""]
    for i in itens:
        md += [f"### `{i['funcao']}` — `{i['arquivo']}:{i['linha']}`", "", f"- Finalidade: {i['finalidade']}", "- Parâmetros esperados: " + ("; ".join(i["parametros"]) or "nenhum"), f"- Processamento: {i['processamento']}", f"- Resultados possíveis: {i['resultados_possiveis']} (retorno declarado: `{i['retorno']}`)", ""]
    (saida / "funcoes.md").write_text("\n".join(md) + "\n", encoding="utf-8")
    E = html.escape
    linhas = "".join(f'<tr id="{E(i["arquivo"])}-{E(i["funcao"])}"><td><code>{E(i["funcao"])}</code></td><td><code>{E(i["arquivo"])}:{i["linha"]}</code></td><td>{E(", ".join(i["parametros"]) or "—")}</td><td><code>{E(i["retorno"])}</code></td><td>{E(i["finalidade"])}</td></tr>' for i in itens)
    pagina = f'''<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><title>Funções de {E(projeto)}</title>
<style>:root{{--g:#FBFAF7;--p:#fff;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A}}@media(prefers-color-scheme:dark){{:root{{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C}}}}
body{{margin:0;background:var(--g);color:var(--i);font:15px/1.5 system-ui,sans-serif}}.w{{max-width:1100px;margin:0 auto;padding:28px 20px}}h1{{color:var(--a);font-size:24px}}
input{{width:100%;box-sizing:border-box;padding:8px 10px;border:1px solid var(--l);border-radius:8px;background:var(--p);color:var(--i);font-size:14px;margin:12px 0}}
table{{border-collapse:collapse;width:100%;font-size:13px;background:var(--p)}}th,td{{text-align:left;padding:6px 8px;border-bottom:1px solid var(--l);vertical-align:top}}th{{font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:var(--m)}}code{{font-family:ui-monospace,monospace}}</style></head>
<body><div class="w"><h1>Funções de {E(projeto)}</h1><p>{len(itens)} funções; {sem_doc} sem finalidade documentada. Gerado em {hoje}. Índice para máquinas: <code>indice.json</code>.</p>
<input id="q" placeholder="filtrar por nome, arquivo ou finalidade" oninput="for(const r of document.querySelectorAll('tbody tr'))r.hidden=!r.textContent.toLowerCase().includes(this.value.toLowerCase())">
<table><thead><tr><th>função</th><th>arquivo</th><th>parâmetros</th><th>retorno</th><th>finalidade</th></tr></thead><tbody>{linhas}</tbody></table></div></body></html>'''
    (saida / "funcoes.html").write_text(pagina, encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--codigo", type=Path, required=True); ap.add_argument("--saida", type=Path, required=True); ap.add_argument("--projeto", default="")
    a = ap.parse_args()
    if not a.codigo.is_dir():
        print(f"erro: {a.codigo} não é diretório", file=sys.stderr); return 2
    itens = extrair(a.codigo.resolve())
    gerar(itens, a.saida, a.projeto or a.codigo.resolve().name)
    print(f"CREATED {a.saida}/funcoes.md, funcoes.html, indice.json ({len(itens)} funções, {sum(1 for i in itens if i['finalidade'] == '(nao documentado)')} sem finalidade)")
    return 0


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
