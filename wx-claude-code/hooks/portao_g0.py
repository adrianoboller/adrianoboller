#!/usr/bin/env python3
"""Hook PreToolUse: enquanto o ultimo pre-flight (G0) estiver BLOCKED, nenhum
Write/Edit fora de .wx-migration/ passa.

A regra «BLOCKED nao escreve codigo» existia so como texto para o modelo
obedecer. Aqui ela vira portao do harness: o hook le o relatorio mais recente
em <projeto>/.wx-migration/preflight/runs/*/report.json e nega a escrita com
o motivo. Escritas dentro de .wx-migration/ (inventario, gaps, decisoes)
continuam liberadas, porque sao exatamente o que um G0 bloqueado deve produzir.

Sem .wx-migration/ no projeto o hook nao opina: projeto que nao usa o plugin
nao e afetado.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def ultimo_relatorio(inicio: Path) -> tuple[Path, dict] | None:
    for pasta in (inicio, *inicio.parents):
        runs = pasta / ".wx-migration" / "preflight" / "runs"
        if runs.is_dir():
            rels = sorted(runs.glob("*/report.json"))
            if rels:
                try:
                    return rels[-1], json.loads(rels[-1].read_text(encoding="utf-8"))
                except (OSError, json.JSONDecodeError):
                    return None
            return None
    return None


def main() -> int:
    try:
        entrada = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0
    ferramenta = entrada.get("tool_name")
    ti = entrada.get("tool_input", {}) or {}
    if ferramenta in {"Write", "Edit", "MultiEdit", "NotebookEdit"}:
        caminho = ti.get("file_path") or ti.get("notebook_path") or ""
    elif ferramenta == "Bash":
        # Escrita por shell: so o alvo de um redirecionamento; comando sem > nao e escrita.
        m = re.search(r">>?\s*['\"]?([^\s'\"|;&]+)", ti.get("command") or "")
        caminho = m.group(1) if m else ""
    else:
        return 0
    if not caminho:
        return 0
    alvo = Path(caminho.replace("\\", "/"))
    if not alvo.is_absolute():
        alvo = Path(entrada.get("cwd", ".")) / alvo
    alvo = alvo.resolve()  # antes de olhar as partes: src/../.wx-migration/../src passava
    if ".wx-migration" in alvo.parts:
        return 0
    achado = ultimo_relatorio(alvo.parent if alvo.parent.exists() else Path(entrada.get("cwd", ".")))
    if achado is None:
        # Sem relatorio nenhum: projeto que nao usa o plugin. Relatorio ilegivel: fecha.
        raiz = alvo.parent if alvo.parent.exists() else Path(entrada.get("cwd", "."))
        if any((p / ".wx-migration" / "preflight" / "runs").is_dir() and list((p / ".wx-migration" / "preflight" / "runs").glob("*/report.json")) for p in (raiz, *raiz.parents)):
            print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": "Relatorio do G0 ilegivel; rode o pre-flight de novo antes de escrever codigo."}}))
        return 0
    rel_path, rel = achado
    if rel.get("status") != "BLOCKED":
        return 0
    erros = rel.get("counts", {}).get("errors")
    if erros is None:
        erros = rel.get("errors")
    n = len(erros) if isinstance(erros, list) else (erros if isinstance(erros, int) else "?")
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": (
                f"Gate G0 está BLOCKED ({n} erro(s) em {rel_path}). O plugin WX não escreve código de produto "
                "com pré-flight bloqueado: resolva os anexos ou registre as lacunas em .wx-migration/gaps.md e rode o pré-flight de novo."
            ),
        }
    }))
    return 0


if __name__ == "__main__":
    sys.exit(main())
