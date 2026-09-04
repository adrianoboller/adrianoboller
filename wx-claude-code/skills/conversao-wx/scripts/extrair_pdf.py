#!/usr/bin/env python3
"""Extrai o texto dos PDFs do manifesto, pagina a pagina, com localizador e hash.

Fecha a cadeia de evidencia do G1: cada trecho que um agente citar tem de
apontar para arquivo + pagina + hash do PDF de origem, e isso nao pode
depender de o modelo ter lido a pagina certa. Saida em
.wx-migration/evidence/pdf-text/<arquivo>/<pagina>.txt e um sumario JSON com
o total de caracteres por pagina; pagina com menos de --minimo caracteres vira
OCR_REQUIRED.

Depende de pypdf (pip install pypdf) ou pdfminer.six; sem nenhum dos dois, o
script diz isso e sai com codigo 3, sem inventar texto.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()


def paginas(path: Path) -> list[str]:
    try:
        from pypdf import PdfReader  # type: ignore
        return [(pg.extract_text() or "") for pg in PdfReader(str(path)).pages]
    except ImportError:
        pass
    try:
        from pdfminer.high_level import extract_text  # type: ignore
        texto = extract_text(str(path))
        return texto.split("\f")
    except ImportError:
        raise SystemExit("erro: instale pypdf (pip install pypdf) ou pdfminer.six para extrair texto de PDF")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--manifest", required=True, type=Path)
    ap.add_argument("--allowed-evidence-root", required=True, type=Path)
    ap.add_argument("--output", required=True, type=Path, help=".wx-migration/evidence/pdf-text")
    ap.add_argument("--minimo", type=int, default=80, help="caracteres por página abaixo dos quais é OCR_REQUIRED")
    args = ap.parse_args()

    raiz = args.allowed_evidence_root.resolve(strict=True)
    m = json.loads(args.manifest.read_text(encoding="utf-8"))
    sumario = {"arquivos": [], "ocr_required": []}
    grupos = ("code_documents", "ui_documents", "query_documents", "business_rule_documents")
    vistos: set[str] = set()
    for grupo in grupos:
        for item in m.get("artifacts", {}).get(grupo, {}).get("items", []):
            rel = item.get("path")
            if not rel or rel in vistos:
                continue
            vistos.add(rel)
            pdf = (raiz / rel).resolve()
            if raiz not in pdf.parents or not pdf.is_file():
                sumario["arquivos"].append({"path": rel, "erro": "fora da raiz de evidências ou inexistente"})
                continue
            if pdf.suffix.lower() != ".pdf":
                sumario["arquivos"].append({"path": rel, "erro": "nao e PDF; extraia por outro meio"})
                continue
            h = sha256(pdf)
            try:
                textos = paginas(pdf)
            except Exception as exc:  # PDF corrompido ou cifrado nao pode derrubar os demais
                sumario["arquivos"].append({"path": rel, "sha256": h, "grupo": grupo, "erro": f"{type(exc).__name__}: {str(exc)[:160]}"})
                continue
            destino = args.output / Path(rel).with_suffix("")  # preserva a pasta: b/x.pdf e c/x.pdf nao colidem
            destino.mkdir(parents=True, exist_ok=True)
            pags = []
            for i, t in enumerate(textos, 1):
                (destino / f"{i:04d}.txt").write_text(t, encoding="utf-8")
                pags.append({"pagina": i, "caracteres": len(t.strip()), "locator": f"{rel}#page={i}"})
                if len(t.strip()) < args.minimo:
                    sumario["ocr_required"].append(f"{rel}#page={i}")
            sumario["arquivos"].append({"path": rel, "sha256": h, "grupo": grupo, "paginas": len(textos), "caracteres": sum(p["caracteres"] for p in pags), "por_pagina": pags})
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "sumario.json").write_text(json.dumps(sumario, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for a in sumario["arquivos"]:
        if "erro" in a:
            print(f"ERRO {a['path']}: {a['erro']}")
        else:
            print(f"{a['path']}: {a['paginas']} páginas, {a['caracteres']} caracteres, sha256 {a['sha256'][:12]}…")
    print(f"OCR_REQUIRED: {len(sumario['ocr_required'])} página(s)")
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
