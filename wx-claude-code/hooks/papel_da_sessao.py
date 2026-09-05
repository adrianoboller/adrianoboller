#!/usr/bin/env python3
"""Hook PreToolUse: quem valida nao conserta o que deveria detectar.

O papel independente do QA e o que da valor a evidencia dele. Se o mesmo agente
que testa tambem corrige o defeito, o que sobra e alguem se aprovando -- e a
prova vale zero para quem esta comprando a migracao.

Este hook e a fronteira de UM ESCRITOR: declarado o papel da sessao, ele nega a
escrita fora do que aquele papel pode escrever.

  qa / validador   escreve teste, evidencia e achado; NAO escreve produto
  revisor          igual ao QA
  documentador     escreve documento; nao escreve produto nem teste
  (qualquer outro) nao muda nada

**Entra pedido, nao imposto.** Sem papel declarado o hook nao opina -- projeto
que ja usa o plugin continua exatamente como estava, que e a regra da casa:
proteção que quebra todo cliente antigo nao e proteção, e estrago.

Como declarar, em ordem de precedencia:
  1. variavel de ambiente WX_PAPEL=qa
  2. arquivo .wx-migration/papel-da-sessao (uma palavra)

E como sair: apagar o arquivo, ou WX_PAPEL=dev. A saida e sempre possivel e
barata de proposito -- o hook e disciplina, nao cadeado; quem quiser burlar
apaga o arquivo, e ai a responsabilidade e de quem apagou, registrada no log.
"""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

# O que cada papel PODE escrever. Vazio = escreve o que quiser.
PERMITIDO = {
    "qa": ("tests/", "test/", "testes/", "spec/", ".wx-migration/evidencias/",
           ".wx-migration/gaps.md", ".wx-migration/c-gate.json", ".wx-migration/qa/"),
    "validador": ("tests/", "test/", "testes/", ".wx-migration/evidencias/",
                  ".wx-migration/gaps.md", ".wx-migration/c-gate.json"),
    "revisor": (".wx-migration/gaps.md", ".wx-migration/revisao/", ".wx-migration/evidencias/"),
    "documentador": ("docs/", "README.md", "MANUAL.md", ".wx-migration/documentacao/"),
}
SINONIMOS = {"quality": "qa", "tester": "qa", "teste": "qa", "reviewer": "revisor",
             "validator": "validador", "docs": "documentador", "documentation": "documentador"}


def papel_da_sessao(inicio: Path) -> str:
    do_ambiente = (os.environ.get("WX_PAPEL") or "").strip().lower()
    if do_ambiente:
        return SINONIMOS.get(do_ambiente, do_ambiente)
    for pasta in (inicio, *inicio.parents):
        arq = pasta / ".wx-migration" / "papel-da-sessao"
        if arq.is_file():
            try:
                p = arq.read_text(encoding="utf-8").strip().split()[0].lower()
            except (OSError, IndexError):
                return ""
            return SINONIMOS.get(p, p)
    return ""


def caminho_do_pedido(entrada: dict) -> str:
    ferramenta = entrada.get("tool_name")
    ti = entrada.get("tool_input", {}) or {}
    if ferramenta in {"Write", "Edit", "MultiEdit", "NotebookEdit"}:
        return ti.get("file_path") or ti.get("notebook_path") or ""
    if ferramenta == "Bash":
        # so o alvo de redirecionamento: comando sem > nao escreve arquivo.
        # (a versao anterior deste padrao, no outro hook, bloqueava leitura --
        # o registro de operacoes foi quem mostrou)
        m = re.search(r">>?\s*['\"]?([^\s'\"|;&]+)", ti.get("command") or "")
        return m.group(1) if m else ""
    return ""


def negar(motivo: str) -> None:
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "deny",
        "permissionDecisionReason": motivo}}))


def main() -> int:
    try:
        entrada = json.load(sys.stdin)
    except json.JSONDecodeError:
        return 0
    caminho = caminho_do_pedido(entrada)
    if not caminho:
        return 0
    cwd = Path(entrada.get("cwd", "."))
    papel = papel_da_sessao(cwd)
    permitido = PERMITIDO.get(papel)
    if not permitido:
        return 0  # papel nao declarado ou sem restricao: nada muda
    alvo = Path(caminho.replace("\\", "/"))
    if not alvo.is_absolute():
        alvo = cwd / alvo
    try:
        rel = alvo.resolve().relative_to(cwd.resolve()).as_posix()
    except ValueError:
        rel = alvo.as_posix()
    if any(rel == p.rstrip("/") or rel.startswith(p) for p in permitido):
        return 0
    negar(
        f"Sessão com papel «{papel}»: este papel não escreve o produto que ele mesmo valida "
        f"({rel}). Independência é o que dá valor à evidência — abra o achado em "
        ".wx-migration/gaps.md e devolva para quem escreve. Pode escrever em: "
        + ", ".join(permitido) + ". Para sair do papel: WX_PAPEL=dev ou apague "
        ".wx-migration/papel-da-sessao."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
