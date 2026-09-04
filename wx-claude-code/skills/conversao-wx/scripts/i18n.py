#!/usr/bin/env python3
"""Tradutor multilíngue: centraliza todos os textos da interface num unico JSON.

So entra em acao quando o usuario pedir. O arquivo e i18n/textos.json:
{"idiomas": ["pt-BR", "en"], "textos": {"chave": {"pt-BR": "...", "en": "..."}}}.
O agente Tradutor (J) edita os arquivos de tela trocando o literal pela
chave; este script cuida do JSON: cria, acrescenta, verifica o que falta em
cada idioma e importa literais achados no codigo.

Uso:
  i18n.py --project-root <p> iniciar --idioma pt-BR --idioma en
  i18n.py --project-root <p> adicionar --chave botao.gravar --texto pt-BR="Gravar" --texto en="Save"
  i18n.py --project-root <p> verificar          # exit 3 se faltar traducao
  i18n.py --project-root <p> extrair --codigo <dir>   # literais entre aspas em .tsx/.ts/.rs/.py viram chaves pendentes
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


def carregar(p: Path) -> dict:
    if not p.is_file():
        raise ValueError(f"{p} não existe; rode `i18n.py iniciar`")
    return json.loads(p.read_text(encoding="utf-8"))


def salvar(p: Path, d: dict) -> None:
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(d, ensure_ascii=False, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project-root", type=Path, default=Path.cwd())
    sub = ap.add_subparsers(dest="cmd", required=True)
    i = sub.add_parser("iniciar"); i.add_argument("--idioma", action="append", required=True)
    a_ = sub.add_parser("adicionar"); a_.add_argument("--chave", required=True); a_.add_argument("--texto", action="append", default=[], help="idioma=texto")
    sub.add_parser("verificar")
    e = sub.add_parser("extrair"); e.add_argument("--codigo", type=Path, required=True)
    a = ap.parse_args()
    p = a.project_root.resolve() / "i18n" / "textos.json"
    try:
        if a.cmd == "iniciar":
            if p.is_file():
                print(f"SKIPPED {p}"); return 0
            salvar(p, {"idiomas": a.idioma, "textos": {}}); print(f"CREATED {p}"); return 0
        d = carregar(p)
        if a.cmd == "adicionar":
            if not re.fullmatch(r"[a-z0-9_.-]+", a.chave):
                raise ValueError("chave: minúsculas, dígitos, ponto, hífen e sublinhado")
            ent = d["textos"].setdefault(a.chave, {})
            for t in a.texto:
                idioma, _, texto = t.partition("=")
                if idioma not in d["idiomas"]:
                    raise ValueError(f"idioma {idioma!r} não está em {d['idiomas']}")
                ent[idioma] = texto
            salvar(p, d); print(f"{a.chave}: {', '.join(k for k in ent)}"); return 0
        if a.cmd == "verificar":
            faltas = [(k, idm) for k, v in d["textos"].items() for idm in d["idiomas"] if not v.get(idm)]
            print(f"{len(d['textos'])} chaves, {len(d['idiomas'])} idiomas, {len(faltas)} tradução(ões) faltando" + ("" if not faltas else ": " + ", ".join(f"{k}[{i}]" for k, i in faltas[:20])))
            return 3 if faltas else 0
        if a.cmd == "extrair":
            rx = re.compile(r"(?:>|placeholder=|label=|title=|\btext\(|\.text\(|\bmsg\(|\bt\()\s*[\"'`]([^\"'`{}<>]{3,80})[\"'`]")
            novos = 0
            for f in a.codigo.rglob("*"):
                if f.suffix in (".tsx", ".jsx", ".ts", ".js", ".rs", ".py", ".razor", ".html", ".vue", ".svelte") and not set(f.parts) & {"node_modules", "target", ".git", "dist"}:
                    for m in rx.finditer(f.read_text(encoding="utf-8", errors="replace")):
                        lit = m.group(1).strip()
                        if not re.search(r"[A-Za-zÀ-ú]{3,}", lit):
                            continue
                        chave = "pendente." + re.sub(r"[^a-z0-9]+", "-", lit.lower()).strip("-")[:40]
                        if chave not in d["textos"]:
                            d["textos"][chave] = {d["idiomas"][0]: lit, "_origem": str(f.relative_to(a.codigo))}
                            novos += 1
            salvar(p, d); print(f"{novos} literal(is) novo(s) como chaves pendente.*; o Tradutor renomeia a chave e preenche os outros idiomas"); return 0
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr); return 2
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
