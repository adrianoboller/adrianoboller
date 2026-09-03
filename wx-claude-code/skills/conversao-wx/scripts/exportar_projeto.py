#!/usr/bin/env python3
"""Exporta o projeto resultante, organizado, para a pasta que o usuario definir.

A pasta de saida ganha uma subpasta <nome>-<data> com sete partes numeradas
(questionario, evidencias, inventario e decisoes, PMO, ambiente e prompts,
codigo, relatorio final), um LEIA-ME que diz o que ha em cada uma e um
manifesto.json com o SHA-256 de todo arquivo copiado. Nada sensivel viaja:
.env, chaves, target/, node_modules/ e .git/ ficam de fora, e um arquivo
com formato de token e recusado com o caminho no erro.

Evidencias (inputs/) sao copiadas so com --com-evidencias; sem a flag entram
por hash, porque podem ser grandes e conter dados de clientes.

Uso:
  exportar_projeto.py --project-root <p> --destino <pasta> [--codigo <dir>] [--com-evidencias]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from datetime import date
from pathlib import Path

EXCLUIR_DIRS = {".git", "target", "node_modules", "__pycache__", ".claude/worktrees", "dist", "build", ".venv", "venv"}
EXCLUIR_ARQ = re.compile(r"^\.env(\.(?!exemplo$|example$).*)?$|\.pem$|\.key$|chave-privada\.json$|\.pyc$")  # .env.exemplo nao tem valor e viaja
TOKEN = re.compile(rb"ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY-----")
PARTES = [
    ("01-questionario", "as respostas: questionario.json e respostas_questionario.md"),
    ("02-evidencias", "anexos do legado (copiados com --com-evidencias; senão, só os hashes)"),
    ("03-inventario-e-decisoes", "traceability.csv, gaps.md, decisions/, specifications/, architecture/, empresa.md, processo-de-conversao.md"),
    ("04-pmo", "plano, backlog, kanban, sprints, PDCA, base de conhecimento, riscos, relatório, painel e entregas zipadas"),
    ("05-ambiente-e-prompts", "ambiente.md, ambiente/ (instalador, SQL, .env.exemplo, n8n), prompts/, CLAUDE.md, INDEX_FILES.md, DESIGN.md, PRODUCT.md, .claude/, .mcp.json, Dockerfile, docker-compose.yml"),
    ("06-codigo", "o código do projeto convertido (sem target/, node_modules/, .git/ e .env)"),
    ("07-relatorio-final", "relatorio.md e painel.html do PMO no momento da exportação"),
]


def sha256(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def pode(p: Path, base: Path) -> bool:
    rel = p.relative_to(base)
    if any(parte in EXCLUIR_DIRS for parte in rel.parts) or "/".join(rel.parts[:2]) in EXCLUIR_DIRS:
        return False
    return not EXCLUIR_ARQ.search(p.name)


def copiar(origem: Path, destino: Path, base: Path, manifesto: list, recusados: list) -> int:
    n = 0
    if origem.is_file():
        arquivos = [origem]
    elif origem.is_dir():
        arquivos = [f for f in origem.rglob("*") if f.is_file()]
    else:
        return 0
    for f in arquivos:
        if not pode(f, base):
            continue
        if f.stat().st_size < 4_000_000:
            with f.open("rb") as fh:
                if TOKEN.search(fh.read()):
                    recusados.append(str(f.relative_to(base)))
                    continue
        alvo = destino / (f.relative_to(origem) if origem.is_dir() else f.name)
        alvo.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(f, alvo)
        manifesto.append({"arquivo": str(alvo), "sha256": sha256(alvo), "bytes": alvo.stat().st_size})
        n += 1
    return n


def exportar(projeto: Path, destino: Path, codigo: Path | None, com_evidencias: bool) -> Path:
    wx = projeto / ".wx-migration"
    if not wx.is_dir():
        raise ValueError(f"{wx} não existe: nada a exportar")
    q = json.loads((wx / "questionario.json").read_text(encoding="utf-8")) if (wx / "questionario.json").is_file() else {}
    nome = re.sub(r"[^A-Za-z0-9]+", "-", ((q.get("projeto") or {}).get("nome") or projeto.name)).strip("-").lower() or "projeto"
    raiz = destino.resolve() / f"{nome}-{date.today().isoformat()}"
    if raiz.exists():
        raise ValueError(f"{raiz} já existe; escolha outra pasta ou apague a anterior")
    if projeto.resolve() in raiz.parents or raiz == projeto.resolve():
        raise ValueError("a pasta de saída não pode ficar dentro do projeto")
    raiz.mkdir(parents=True)
    manifesto: list = []; recusados: list = []; contagem: dict = {}
    def parte(n, *pares):
        d = raiz / n; d.mkdir(exist_ok=True); k = 0
        for origem, sub in pares:
            k += copiar(origem, d / sub if sub else d, projeto, manifesto, recusados)
        contagem[n] = k
    parte("01-questionario", (wx / "questionario.json", ""), (wx / "respostas_questionario.md", ""))
    raiz_ev = (projeto / ((q.get("projeto") or {}).get("raiz_de_evidencias") or "inputs")).resolve()
    if com_evidencias:
        parte("02-evidencias", (raiz_ev, ""))
    else:
        (raiz / "02-evidencias").mkdir(exist_ok=True)
        hashes = [{"arquivo": str(f.relative_to(raiz_ev)), "sha256": sha256(f), "bytes": f.stat().st_size} for f in sorted(raiz_ev.rglob("*")) if f.is_file()] if raiz_ev.is_dir() else []
        (raiz / "02-evidencias" / "hashes.json").write_text(json.dumps(hashes, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        contagem["02-evidencias"] = len(hashes)
    parte("03-inventario-e-decisoes", (wx / "traceability.csv", ""), (wx / "gaps.md", ""), (wx / "decisions", "decisions"), (wx / "specifications", "specifications"), (wx / "architecture", "architecture"), (wx / "empresa.md", ""), (wx / "processo-de-conversao.md", ""), (wx / "entrega.json", ""), (wx / "preflight", "preflight"))
    parte("04-pmo", (wx / "pmo", ""))
    parte("05-ambiente-e-prompts", (wx / "ambiente.md", ""), (wx / "ambiente", "ambiente"), (wx / "prompts", "prompts"), (projeto / "CLAUDE.md", ""), (projeto / "INDEX_FILES.md", ""), (projeto / "DESIGN.md", ""), (projeto / "PRODUCT.md", ""), (projeto / ".claude", ".claude"), (projeto / ".mcp.json", ""), (projeto / "Dockerfile", ""), (projeto / "docker-compose.yml", ""))
    if codigo:
        parte("06-codigo", (codigo.resolve(), ""))
    else:
        (raiz / "06-codigo").mkdir(exist_ok=True); contagem["06-codigo"] = 0
    parte("07-relatorio-final", (wx / "pmo" / "relatorio.md", ""), (wx / "pmo" / "painel.html", ""), (wx / "pmo" / "status.md", ""))
    for m in manifesto:
        m["arquivo"] = str(Path(m["arquivo"]).relative_to(raiz))
    (raiz / "manifesto.json").write_text(json.dumps({"projeto": nome, "exportado_em": date.today().isoformat(), "origem": str(projeto.resolve()), "arquivos": manifesto, "recusados_por_segredo": recusados}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    total = sum(m["bytes"] for m in manifesto)
    L = [f"# {nome} — projeto exportado em {date.today().isoformat()}", "",
         f"Origem: `{projeto.resolve()}`. {len(manifesto)} arquivos, {total} bytes, cada um com SHA-256 em `manifesto.json`. Gerado por `exportar_projeto.py`.", "",
         "| pasta | o que há | arquivos |", "| --- | --- | ---: |"]
    L += [f"| `{n}/` | {d} | {contagem.get(n, 0)} |" for n, d in PARTES]
    L += ["", "Fora, de propósito: `.env`, chaves privadas, `target/`, `node_modules/`, `.git/`." + (f" Recusados por conter formato de token: {', '.join(recusados)}." if recusados else ""), ""]
    (raiz / "00-LEIA-ME.md").write_text("\n".join(L), encoding="utf-8")
    return raiz


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project-root", type=Path, default=Path.cwd())
    ap.add_argument("--destino", type=Path, help="pasta definida pelo usuario (ou L3.pasta_de_saida do questionario); a exportacao cria <nome>-<data> dentro dela")
    ap.add_argument("--codigo", type=Path, help="diretorio do codigo convertido (06-codigo); por padrao, o diretorio_destino do 0.15 se existir")
    ap.add_argument("--com-evidencias", action="store_true")
    a = ap.parse_args()
    projeto = a.project_root.resolve()
    destino = a.destino
    if destino is None:
        qp = projeto / ".wx-migration" / "questionario.json"
        if qp.is_file():
            ps = (((json.loads(qp.read_text(encoding="utf-8")).get("L_contexto_e_implantacao") or {}).get("L3_implantacao") or {}).get("pasta_de_saida")) or ""
            if ps:
                destino = Path(ps).expanduser()
        if destino is None:
            print("erro: informe --destino ou preencha L3.pasta_de_saida no questionario", file=sys.stderr); return 2
    codigo = a.codigo
    if codigo is None:
        ent = projeto / ".wx-migration" / "entrega.json"
        if ent.is_file():
            d = json.loads(ent.read_text(encoding="utf-8")).get("diretorio_destino") or ""
            if d and (projeto / d).is_dir():
                codigo = projeto / d
    try:
        raiz = exportar(projeto, destino, codigo, a.com_evidencias)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr); return 2
    m = json.loads((raiz / "manifesto.json").read_text(encoding="utf-8"))
    print(f"CREATED {raiz} ({len(m['arquivos'])} arquivos" + (f"; {len(m['recusados_por_segredo'])} recusado(s) por segredo" if m["recusados_por_segredo"] else "") + ")")
    return 0


if __name__ == "__main__":
    sys.exit(main())
