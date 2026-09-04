#!/usr/bin/env python3
"""Hook PreToolUse: faz valer duas regras que eram so texto.

1. Anexos sao somente leitura: nenhuma escrita (Write/Edit/NotebookEdit) nem
   comando de shell destrutivo (rm, mv, >, sed -i, truncate) dentro da raiz de
   evidencias (evidence_root do manifesto, ou inputs/ por padrao).
2. Segredo nunca em arquivo: conteudo com formato conhecido de token ou chave
   privada, ou escrita em .env, e negado antes de tocar o disco. Tambem nega
   `git add`/`git commit` que alcance um .env.

Projeto sem .wx-migration/ nao e afetado. O hook le stdin uma vez e responde
em JSON; qualquer erro interno libera (fail-open) porque este portao protege
o usuario de si mesmo, nao de um atacante -- o portao de seguranca de verdade
e o validar_entradas do aplicar_questionario.py.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

TOKEN = re.compile(r"ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}")
DESTRUTIVO = re.compile(r"(^|[;&|]\s*)(rm|mv|truncate|shred|sed\s+-i|tee|cp)\b|>{1,2}\s*")


def raiz_de_evidencias(inicio: Path) -> Path | None:
    for pasta in (inicio, *inicio.parents):
        m = pasta / ".wx-migration" / "wx-inputs.manifest.json"
        if m.is_file():
            try:
                rel = json.loads(m.read_text(encoding="utf-8")).get("evidence_root") or "../inputs"
            except (OSError, json.JSONDecodeError):
                rel = "../inputs"
            return (m.parent / rel).resolve()
        if (pasta / ".wx-migration").is_dir():
            return (pasta / "inputs").resolve()
    return None


def negar(motivo: str) -> int:
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": motivo}}, ensure_ascii=False))
    return 0


def main() -> int:
    try:
        entrada = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0
    ferramenta = entrada.get("tool_name", "")
    ti = entrada.get("tool_input", {}) or {}
    cwd = Path(entrada.get("cwd", "."))
    raiz = raiz_de_evidencias(cwd)
    if ferramenta in {"Write", "Edit", "MultiEdit", "NotebookEdit"}:
        caminho = ti.get("file_path") or ti.get("notebook_path") or ""
        if not caminho:
            return 0
        alvo = Path(caminho.replace("\\", "/"))
        alvo = (alvo if alvo.is_absolute() else cwd / alvo).resolve()
        if raiz and (alvo == raiz or raiz in alvo.parents):
            return negar(f"Anexo é somente leitura: {alvo.name} está na raiz de evidências ({raiz}). O que o plugin gera vai para .wx-migration/.")
        if re.fullmatch(r"\.env(\..+)?", alvo.name) and alvo.name != ".env.exemplo":
            return negar(f"Não se grava {alvo.name} pelo agente: o usuário preenche o .env a partir do .env.exemplo, fora do repositório.")
        conteudo = ti.get("content") or ti.get("new_string") or ti.get("new_source") or ""
        if isinstance(conteudo, str) and TOKEN.search(conteudo):
            return negar("Conteúdo com formato de token ou chave privada: segredo nunca em arquivo. Use o NOME da variável de ambiente.")
        return 0
    if ferramenta == "Bash":
        cmd = ti.get("command") or ""
        if raiz and DESTRUTIVO.search(cmd):
            for tok in re.findall(r"[\w./~-]+", cmd):
                try:
                    p = Path(tok.replace("\\", "/"))
                    p = (p if p.is_absolute() else cwd / p).resolve()
                except (OSError, ValueError):
                    continue
                if p == raiz or raiz in p.parents:
                    return negar(f"Comando escreve ou apaga dentro da raiz de evidências ({raiz}); anexos são somente leitura.")
        if re.search(r"\bgit\s+(add|commit)\b", cmd) and re.search(r"(^|[\s/])\.env(\.\w+)?(\s|$)", cmd) and ".env.exemplo" not in cmd:
            return negar("git add/commit de .env: segredo nunca entra no repositório.")
        if TOKEN.search(cmd):
            return negar("Comando contém um token em texto: passe-o por variável de ambiente, nunca na linha.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception:  # fail-open: portao de conveniencia, ver docstring
        sys.exit(0)
