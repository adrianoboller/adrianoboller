#!/usr/bin/env python3
"""Prova do classificar.py nos dois sentidos: cada regra do dono reprova o
caso ruim e aprova o bom. Roda em pasta temporaria; nao toca as cognicoes.

    python3 phxsql/docs/cognicao/testar_classificar.py
"""

import importlib.util
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "classificar", Path(__file__).with_name("classificar.py")
)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

BASE = "# Titulo\n\n## A regra\n\nFaca assim.\n"
CASOS = [
    # (nome, secao Estado, estado esperado, deve reprovar)
    ("sem estado e PENDENTE, nunca FRUTIFERO", "", "PENDENTE", False),
    ("FRUTIFERO sem evidencia reprova",
     "- **Estado:** FRUTÍFERO\n- **Validado em:** 24/09/2026\n", "FRUTÍFERO", True),
    ("FRUTIFERO com arquivo inexistente reprova",
     "- **Estado:** FRUTÍFERO\n- **Evidência:** `nao/existe.json`\n"
     "- **Validado em:** 24/09/2026\n", "FRUTÍFERO", True),
    ("FRUTIFERO com teste inexistente reprova",
     "- **Estado:** FRUTÍFERO\n- **Evidência:** `teste:teste_que_nao_existe_xyz`\n"
     "- **Validado em:** 24/09/2026\n", "FRUTÍFERO", True),
    ("FRUTIFERO com commit inexistente reprova",
     "- **Estado:** FRUTÍFERO\n- **Evidência:** `commit:0000000deadbeef`\n"
     "- **Validado em:** 24/09/2026\n", "FRUTÍFERO", True),
    ("evidencia falsa nao se dilui na verdadeira",
     "- **Estado:** FRUTÍFERO\n- **Evidência:** `CLAUDE.md`, `nao/existe`\n"
     "- **Validado em:** 24/09/2026\n", "FRUTÍFERO", True),
    ("FRUTIFERO sem data reprova",
     "- **Estado:** FRUTÍFERO\n- **Evidência:** `CLAUDE.md`\n", "FRUTÍFERO", True),
    ("FRUTIFERO com evidencia que confere passa",
     "- **Estado:** FRUTÍFERO\n- **Evidência:** `CLAUDE.md`\n"
     "- **Validado em:** 24/09/2026\n", "FRUTÍFERO", False),
    ("INFRUTIFERO sem causa reprova",
     "- **Estado:** INFRUTÍFERO\n- **Evidência:** `CLAUDE.md`\n"
     "- **Prevenção:** isto\n", "INFRUTÍFERO", True),
    ("INFRUTIFERO sem prevencao reprova",
     "- **Estado:** INFRUTÍFERO\n- **Evidência:** `CLAUDE.md`\n"
     "- **Causa:** aquilo\n", "INFRUTÍFERO", True),
    ("INFRUTIFERO completo passa",
     "- **Estado:** INFRUTÍFERO\n- **Evidência:** `CLAUDE.md`\n"
     "- **Causa:** aquilo\n- **Prevenção:** isto\n", "INFRUTÍFERO", False),
    ("estado desconhecido reprova",
     "- **Estado:** PROMISSOR\n", "PENDENTE", True),
]


def main():
    falhas = 0
    with tempfile.TemporaryDirectory() as d:
        for i, (nome, estado, esperado, reprova) in enumerate(CASOS):
            texto = BASE + (f"\n## Estado\n\n{estado}" if estado else "")
            arq = Path(d) / f"cognicao_caso-{i}_20260924_0000.md"
            arq.write_text(texto)
            r = c.avaliar(arq, rodar=False)
            ok = bool(r["erros"]) == reprova and r["estado"] == esperado
            print(f"{'ok ' if ok else 'FALHOU'} {nome}"
                  + ("" if ok else f" -> {r['estado']} {r['erros']}"))
            falhas += not ok
    print(f"{len(CASOS) - falhas}/{len(CASOS)}")
    raise SystemExit(1 if falhas else 0)


if __name__ == "__main__":
    main()
