#!/usr/bin/env python3
"""Hook PostToolUse (Write/Edit): mantem o PMO em dia com o que acabou de mudar.

- traceability.csv ou pmo/backlog.md editados -> regera pmo/kanban.md (o quadro
  e derivado da matriz; editado a mao ele mente).
- questionario.json editado -> lembra de reaplicar aplicar_questionario.py, que
  regrava respostas_questionario.md e INDEX_FILES.md.
- qualquer .md/.csv/.json em .wx-migration/ -> marca o indice do RAG como
  desatualizado (rag/indice.json.desatualizado), e o hook de prompt reindexa.

Nada aqui falha a ferramenta: erro vira silencio.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "skills" / "conversao-wx" / "scripts"


def main() -> int:
    try:
        entrada = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0
    caminho = (entrada.get("tool_input", {}) or {}).get("file_path") or ""
    if not caminho:
        return 0
    alvo = Path(caminho.replace("\\", "/"))
    if not alvo.is_absolute():
        alvo = Path(entrada.get("cwd", ".")) / alvo
    alvo = alvo.resolve()
    if ".wx-migration" not in alvo.parts:
        return 0
    wx = alvo.parents[len(alvo.parts) - alvo.parts.index(".wx-migration") - 2]
    projeto = wx.parent
    avisos = []
    if alvo.name in ("traceability.csv", "backlog.md"):
        r = subprocess.run([sys.executable, str(SCRIPTS / "pmo.py"), "--project-root", str(projeto), "kanban"], capture_output=True, text=True, timeout=30)
        avisos.append("kanban.md regerado da matriz" if r.returncode == 0 else f"kanban não regerado: {r.stderr.strip()[:120]}")
    if alvo.name == "questionario.json":
        avisos.append("questionario.json mudou: reaplique aplicar_questionario.py para regravar respostas_questionario.md e INDEX_FILES.md")
    if alvo.suffix in (".md", ".csv", ".json"):
        (wx / "rag").mkdir(exist_ok=True)
        (wx / "rag" / "indice.json.desatualizado").write_text(str(alvo), encoding="utf-8")
    if avisos:
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": "PMO: " + "; ".join(avisos)}}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:
        sys.exit(0)
