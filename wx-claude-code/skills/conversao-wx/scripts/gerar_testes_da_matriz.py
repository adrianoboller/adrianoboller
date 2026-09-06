#!/usr/bin/env python3
"""Tira da matriz o teste que ela ja esta pedindo.

A matriz de rastreabilidade diz o que precisa ser provado: cada `BR-*` e uma
regra, cada `QRY-*` uma consulta, cada `UI-*` uma tela. O esqueleto do destino
nasce com as pastas de teste vazias, e o grafo depois acusa «requisito sem
teste» -- item por item, a mao.

A regra que manda aqui, e que inverte o defeito classico: **o teste gerado
FALHA**. Ele nao nasce vazio nem com `assert(true)`; nasce com uma falha
explicita que diz qual regra ainda nao tem prova, e o localizador de onde a
regra veio. Teste que passa por engano e pior que teste que falta -- este
projeto ja pagou por isso -- e um esqueleto que passa e exatamente isso: some
do relatorio de lacunas sem provar nada.

Nao gera para o que ja tem `test_file` preenchido: reescrever prova que alguem
escreveu seria estrago, nao ajuda.

Uso:
  gerar_testes_da_matriz.py [--project-root .] [--gravar] [--perfil rust|php|python]
"""
from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from pathlib import Path

# Cada perfil: pasta, extensao, e como se escreve um teste que FALHA dizendo por que.
PERFIS = {
    "rust": {
        "pasta": "tests", "ext": ".rs",
        "arquivo": lambda nome: f"{nome}_test.rs",
        "corpo": lambda t: (
            f"// {t['trace_id']} — {t['rule_summary']}\n"
            f"// origem: {t['origem']}\n"
            f"// Gerado por gerar_testes_da_matriz.py. Este teste FALHA de proposito\n"
            f"// ate alguem escrever a prova: esqueleto que passa some do relatorio\n"
            f"// de lacunas sem provar nada.\n\n"
            f"#[test]\n"
            f"fn {t['fn']}() {{\n"
            f"    panic!(\"{t['trace_id']} sem prova: {t['rule_summary_esc']}\");\n"
            f"}}\n"),
    },
    "php": {
        "pasta": "tests", "ext": ".php",
        "arquivo": lambda nome: f"{nome}Test.php",
        "corpo": lambda t: (
            f"<?php\n// {t['trace_id']} — {t['rule_summary']}\n"
            f"// origem: {t['origem']}\n"
            f"// Gerado por gerar_testes_da_matriz.py; FALHA ate haver prova.\n\n"
            f"declare(strict_types=1);\n\n"
            f"final class {t['classe']}Test extends \\PHPUnit\\Framework\\TestCase\n{{\n"
            f"    public function test{t['classe']}(): void\n    {{\n"
            f"        $this->fail('{t['trace_id']} sem prova: {t['rule_summary_esc']}');\n"
            f"    }}\n}}\n"),
    },
    "python": {
        "pasta": "tests", "ext": ".py",
        "arquivo": lambda nome: f"test_{nome}.py",
        "corpo": lambda t: (
            f"# {t['trace_id']} — {t['rule_summary']}\n"
            f"# origem: {t['origem']}\n"
            f"# Gerado por gerar_testes_da_matriz.py; FALHA ate haver prova.\n\n"
            f"def test_{t['fn']}():\n"
            f"    raise AssertionError(\"{t['trace_id']} sem prova: {t['rule_summary_esc']}\")\n"),
    },
}


def perfil_do_projeto(raiz: Path) -> str:
    for c in (raiz / ".wx-migration/questionario.json", raiz / "questionario.json"):
        if c.is_file():
            try:
                q = json.loads(c.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                continue
            return str((q.get("H_backend") or {}).get("perfil") or "").lower()
    return ""


def identificador(trace_id: str, resumo: str) -> str:
    base = re.sub(r"[^a-z0-9]+", "_", f"{trace_id}_{resumo}".lower()).strip("_")
    return (base[:60] or "sem_nome").rstrip("_")


def pendentes(raiz: Path) -> list[dict]:
    arq = raiz / ".wx-migration/traceability.csv"
    if not arq.is_file():
        raise SystemExit("nao achei .wx-migration/traceability.csv; rode o questionario antes")
    saida = []
    with arq.open(encoding="utf-8", newline="") as f:
        for linha in csv.DictReader(f):
            tid = (linha.get("trace_id") or "").strip()
            if not tid or (linha.get("test_file") or "").strip():
                continue
            resumo = (linha.get("rule_summary") or "sem resumo").strip()
            fn = identificador(tid, resumo)
            saida.append({
                "trace_id": tid, "kind": (linha.get("kind") or "").strip(),
                "rule_summary": resumo,
                "rule_summary_esc": resumo.replace('"', "'").replace("\\", "/"),
                "origem": (linha.get("source_locator") or linha.get("source_artifact")
                           or "origem não registrada").strip(),
                "fn": fn,
                "classe": "".join(p.capitalize() for p in fn.split("_"))[:60] or "SemNome",
                "nome": fn,
            })
    return saida


def main() -> int:
    p = argparse.ArgumentParser(description="gera da matriz o teste que ela pede")
    p.add_argument("--project-root", default=".")
    p.add_argument("--perfil", choices=sorted(PERFIS))
    p.add_argument("--gravar", action="store_true")
    p.add_argument("--json", action="store_true")
    args = p.parse_args()
    raiz = Path(args.project_root).resolve()
    perfil = args.perfil or perfil_do_projeto(raiz)
    if perfil not in PERFIS:
        # Gerar teste numa linguagem que nao se sabe escrever produz arquivo que
        # nao compila e some no relatorio como se fosse prova. Melhor recusar.
        print(f"perfil {perfil or '(nenhum)'} sem modelo de teste aqui; conheço: "
              f"{', '.join(sorted(PERFIS))}. Passe --perfil.", file=sys.stderr)
        return 2
    m = PERFIS[perfil]
    itens = pendentes(raiz)
    escritos, pulados = [], []
    for t in itens:
        destino = raiz / m["pasta"] / m["arquivo"](t["nome"])
        if destino.exists():
            pulados.append({"trace_id": t["trace_id"], "arquivo": str(destino.relative_to(raiz)),
                            "porque": "o arquivo já existe; não sobrescrevo prova escrita por gente"})
            continue
        escritos.append({"trace_id": t["trace_id"], "arquivo": str(destino.relative_to(raiz)),
                         "conteudo": m["corpo"](t)})
    if args.gravar:
        for e in escritos:
            alvo = raiz / e["arquivo"]
            alvo.parent.mkdir(parents=True, exist_ok=True)
            alvo.write_text(e["conteudo"], encoding="utf-8")
    if args.json:
        print(json.dumps({"perfil": perfil, "gerados": [{k: v for k, v in e.items() if k != "conteudo"}
                                                        for e in escritos],
                          "pulados": pulados, "gravado": args.gravar}, ensure_ascii=False, indent=2))
        return 0
    print(f"perfil {perfil}: {len(escritos)} testes {'gravados' if args.gravar else 'a gerar'}"
          f"{f', {len(pulados)} pulados' if pulados else ''}\n")
    for e in escritos:
        print(f"  {e['trace_id']:<10} {e['arquivo']}")
    for x in pulados:
        print(f"  {x['trace_id']:<10} {x['arquivo']} — {x['porque']}")
    if escritos and args.gravar:
        print("\nTodos falham de propósito: cada um nomeia a regra que ainda não tem prova.")
    elif not args.gravar:
        print("\nNada foi escrito; use --gravar.")
    return 0


try:
    import registro
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    sys.exit(registro.envolver(__file__, main))
