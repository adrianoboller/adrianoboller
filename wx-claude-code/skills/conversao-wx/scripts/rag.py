#!/usr/bin/env python3
"""RAG local do projeto, sem dependencias: indexa os documentos que o plugin gera
e le (.wx-migration/*.md, matriz, decisoes, PMO, CLAUDE.md, DESIGN.md, docs/ do
projeto e as references/ do plugin), em trechos com localizador arquivo#Lnn, e
busca por BM25. O hook UserPromptSubmit injeta os melhores trechos como contexto
de cada pergunta, com o localizador, para o modelo abrir o arquivo certo em vez
de ler tudo.

O corpus WLanguage 12k continua no query_wlanguage_help.py (por tema); este RAG
e do PROJETO. Regra de negocio so vale com origem localizavel: o trecho traz o
localizador justamente por isso.

Uso:
  rag.py --project-root <p> indexar [--plugin-root <r>]
  rag.py --project-root <p> buscar "consulta" [--k 5]
  rag.py --project-root <p> hook          # UserPromptSubmit: le o prompt do stdin
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import time
import unicodedata
from collections import Counter
from pathlib import Path

TAM = 900      # caracteres por trecho
PASSO = 700    # sobreposicao de 200
PARAR = set("a o os as de do da dos das e em um uma para por com que se ao na no nas nos the of and to in is".split())


def normalizar(t: str) -> list[str]:
    t = unicodedata.normalize("NFKD", t.lower()).encode("ascii", "ignore").decode()
    return [w for w in re.findall(r"[a-z0-9_]{2,}", t) if w not in PARAR]


def fontes(projeto: Path, plugin_root: Path | None) -> list[Path]:
    wx = projeto / ".wx-migration"
    arqs: list[Path] = []
    for pad in ("*.md", "*.csv", "*.json", "pmo/*.md", "pmo/*.json", "pmo/conhecimento/*.md", "pmo/sprints/*.md", "pmo/qualidade/*.md", "decisions/*.md", "specifications/**/*.md", "architecture/**/*.md", "prompts/*.md", "ambiente/**/*.md"):
        arqs += sorted(wx.glob(pad))
    for nome in ("CLAUDE.md", "INDEX_FILES.md", "DESIGN.md", "PRODUCT.md", "README.md"):
        if (projeto / nome).is_file():
            arqs.append(projeto / nome)
    arqs += sorted((projeto / "docs").rglob("*.md")) if (projeto / "docs").is_dir() else []
    if plugin_root:
        arqs += sorted((plugin_root / "skills" / "conversao-wx" / "references").glob("*.md"))
    vistos, saida = set(), []
    for a in arqs:
        if a.is_file() and a.name != "questionario.json" and "rag" not in a.parts and a.stat().st_size < 2_000_000 and a not in vistos:
            vistos.add(a); saida.append(a)
    return saida


def trechos(arq: Path, projeto: Path) -> list[dict]:
    try:
        texto = arq.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    try:
        rel = str(arq.relative_to(projeto))
    except ValueError:
        rel = "plugin:" + arq.name
    linhas = texto.split("\n")
    saida, i = [], 0
    while i < len(linhas):
        bloco, j = [], i
        while j < len(linhas) and sum(len(x) + 1 for x in bloco) < TAM:
            bloco.append(linhas[j]); j += 1
        corpo = "\n".join(bloco).strip()
        if corpo:
            saida.append({"arquivo": rel, "linha": i + 1, "texto": corpo})
        avanco = max(1, j - i - 2)
        i += avanco
    return saida


def indexar(projeto: Path, plugin_root: Path | None) -> dict:
    docs = []
    for a in fontes(projeto, plugin_root):
        docs += trechos(a, projeto)
    df: Counter = Counter()
    for d in docs:
        d["termos"] = Counter(normalizar(d["texto"]))
        d["tam"] = sum(d["termos"].values())
        df.update(d["termos"].keys())
    idx = {"gerado_em": time.strftime("%Y-%m-%dT%H:%M"), "n": len(docs), "media": (sum(d["tam"] for d in docs) / len(docs)) if docs else 0, "df": dict(df),
           "docs": [{"arquivo": d["arquivo"], "linha": d["linha"], "texto": d["texto"], "termos": dict(d["termos"]), "tam": d["tam"]} for d in docs]}
    pasta = projeto / ".wx-migration" / "rag"; pasta.mkdir(parents=True, exist_ok=True)
    (pasta / "indice.json").write_text(json.dumps(idx, ensure_ascii=False), encoding="utf-8")
    (pasta / "indice.json.desatualizado").unlink(missing_ok=True)
    return idx


def buscar(idx: dict, consulta: str, k: int) -> list[dict]:
    q = normalizar(consulta)
    if not q or not idx["docs"]:
        return []
    n, media, df = idx["n"], idx["media"] or 1, idx["df"]
    k1, b = 1.5, 0.75
    pont = []
    for d in idx["docs"]:
        s = 0.0
        for t in q:
            f = d["termos"].get(t, 0)
            if not f:
                continue
            idf = math.log(1 + (n - df.get(t, 0) + 0.5) / (df.get(t, 0) + 0.5))
            s += idf * f * (k1 + 1) / (f + k1 * (1 - b + b * d["tam"] / media))
        if s > 0:
            pont.append((s, d))
    pont.sort(key=lambda x: -x[0])
    return [{"pontos": round(s, 2), "arquivo": d["arquivo"], "linha": d["linha"], "trecho": d["texto"][:300].replace("\n", " ")} for s, d in pont[:k]]


# ---------------------------------------------------------------------------
# Corpus WLanguage 12k: o nome de cada membro do zip traz o simbolo e o tema
# (01-03-03_00679__hreadseekfirst_function__3044036.json). Listar o zip custa
# ~70 ms e nao extrai nada; e o suficiente para, numa pergunta que cita
# HReadSeekFirst, apontar o tema e o comando exato de consulta por tema
# (0,5 s) em vez da varredura inteira (13 s medidos).
# ---------------------------------------------------------------------------

def indexar_corpus(plugin_root: Path, pasta: Path) -> dict:
    import zipfile
    zp = plugin_root / "skills" / "conversao-wx" / "resources" / "Help_WL_12k_Json.zip"
    mapa: dict = {}
    if not zp.is_file():
        return {"simbolos": {}, "membros": 0}
    rx = re.compile(r"^Help_WL_12k_Json/(\d\d-\d\d-\d\d)_\d+__([a-z0-9_]+?)_(function|property|example|variable_type|type|constant|event|structure|class|keyword|operator|statement|control|element)__\d+\.json$")
    with zipfile.ZipFile(zp) as z:
        nomes = z.namelist()
    for n in nomes:
        m = rx.match(n)
        if not m:
            continue
        grupo, simbolo, tipo = m.groups()
        e = mapa.setdefault(simbolo, {"grupos": [], "tipos": []})
        if grupo not in e["grupos"]:
            e["grupos"].append(grupo)
        if tipo not in e["tipos"]:
            e["tipos"].append(tipo)
    idx = {"gerado_em": time.strftime("%Y-%m-%dT%H:%M"), "membros": len(nomes), "simbolos": mapa}
    pasta.mkdir(parents=True, exist_ok=True)
    (pasta / "corpus-simbolos.json").write_text(json.dumps(idx, ensure_ascii=False), encoding="utf-8")
    return idx


def simbolos_wlanguage(texto: str, mapa: dict) -> list[str]:
    """Palavras da pergunta que sao simbolos do Help: CamelCase (HReadSeekFirst), ou nome exato em minusculas."""
    achados = []
    for w in re.findall(r"[A-Za-z][A-Za-z0-9_]{2,}", texto):
        k = w.lower()
        if k in mapa and (re.match(r"^[A-Z][a-z0-9]+[A-Z]|^[a-z]+[A-Z]|^H[A-Z]|^f[A-Z]|^Str[A-Z]|^Date[A-Z]|^SQL[A-Z]", w) or len(k) > 6):
            if k not in achados:
                achados.append(k)
    return achados[:4]


def contexto_corpus(plugin_root: Path, pasta: Path, prompt: str) -> str:
    p = pasta / "corpus-simbolos.json"
    try:
        mapa = json.loads(p.read_text(encoding="utf-8"))["simbolos"] if p.is_file() else indexar_corpus(plugin_root, pasta)["simbolos"]
    except (OSError, json.JSONDecodeError):
        return ""
    achados = simbolos_wlanguage(prompt, mapa)
    if not achados:
        return ""
    partes = []
    for s in achados:
        e = mapa[s]
        partes.append(f"{s} ({', '.join(e['tipos'])}) → tema {', '.join(e['grupos'][:3])}: python3 \"${{CLAUDE_PLUGIN_ROOT}}/skills/conversao-wx/scripts/query_wlanguage_help.py\" --group {e['grupos'][0]} --query {s} --limit 3")
    return " Help WLanguage 12k (semântica técnica, nunca regra de negócio; consulte por tema, 0,5 s): " + " | ".join(partes)


def carregar_ou_indexar(projeto: Path, plugin_root: Path | None) -> dict:
    p = projeto / ".wx-migration" / "rag" / "indice.json"
    if p.is_file() and not (p.parent / "indice.json.desatualizado").exists():
        try:
            return json.loads(p.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            pass
    return indexar(projeto, plugin_root)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project-root", type=Path, default=Path.cwd())
    ap.add_argument("--plugin-root", type=Path, default=Path(__file__).resolve().parents[3])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("indexar")
    sub.add_parser("indexar-corpus")
    bq = sub.add_parser("buscar"); bq.add_argument("consulta"); bq.add_argument("--k", type=int, default=5); bq.add_argument("--json", action="store_true")
    sub.add_parser("hook")
    a = ap.parse_args()
    projeto = a.project_root.resolve()
    if a.cmd == "hook":
        if not (projeto / ".wx-migration").is_dir():
            return 0
        try:
            prompt = (json.load(sys.stdin).get("prompt") or "").strip()
        except (json.JSONDecodeError, ValueError):
            return 0
        if len(prompt) < 12 or prompt.startswith("/"):
            return 0
        t0 = time.perf_counter()
        idx = carregar_ou_indexar(projeto, a.plugin_root)
        res = buscar(idx, prompt, 4)
        corpus = contexto_corpus(a.plugin_root, projeto / ".wx-migration" / "rag", prompt)
        if not res and not corpus:
            return 0
        ctx = ("RAG do projeto (trechos mais próximos da pergunta, com localizador; abra o arquivo antes de afirmar): " + " | ".join(f"{r['arquivo']}#L{r['linha']}: {r['trecho'][:160]}" for r in res) if res else "RAG do projeto: nada próximo.") + corpus + f" [{len(idx['docs'])} trechos, {(time.perf_counter() - t0) * 1000:.0f} ms]"
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": ctx}}, ensure_ascii=False))
        return 0
    if a.cmd == "indexar-corpus":
        idx = indexar_corpus(a.plugin_root, projeto / ".wx-migration" / "rag")
        print(f"CREATED corpus-simbolos.json ({len(idx['simbolos'])} símbolos de {idx['membros']} membros)")
        return 0
    if a.cmd == "indexar":
        idx = indexar(projeto, a.plugin_root)
        print(f"CREATED {projeto / '.wx-migration' / 'rag' / 'indice.json'} ({idx['n']} trechos de {len({d['arquivo'] for d in idx['docs']})} arquivos)")
        return 0
    idx = carregar_ou_indexar(projeto, a.plugin_root)
    res = buscar(idx, a.consulta, a.k)
    if a.json:
        print(json.dumps(res, ensure_ascii=False, indent=2))
    else:
        for r in res:
            print(f"{r['pontos']:6.2f}  {r['arquivo']}#L{r['linha']}  {r['trecho'][:120]}")
        if not res:
            print("nada encontrado")
    return 0


if __name__ == "__main__":
    sys.exit(main())
