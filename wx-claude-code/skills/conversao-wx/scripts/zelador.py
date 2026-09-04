#!/usr/bin/env python3
"""Zelador: limpa os temporarios do projeto de conversao de tempos em tempos.

O que e temporario, e so isso, sai: execucoes antigas do pre-flight (fica as
tres ultimas e tudo com menos de N dias), logs antigos em .wx-migration/logs,
__pycache__ e worktrees parados em .claude/worktrees. Anexos, matriz, decisoes,
PMO, entregas e codigo nunca sao tocados: nao sao temporarios.

O hook SessionStart chama `zelador.py limpar --se-vencido`: roda no maximo uma
vez por dia (carimbo em .wx-migration/logs/zelador.ultimo) e registra o que
apagou em .wx-migration/logs/zelador.md, com tamanho medido. Sem --executar
so relata.

Uso:
  zelador.py limpar --project-root <p> [--dias 7] [--executar] [--se-vencido]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
import time
from datetime import date, datetime
from pathlib import Path

INTOCAVEIS = ("inputs", "traceability.csv", "gaps.md", "decisions", "pmo", "questionario.json", "respostas_questionario.md")


def tamanho(p: Path) -> int:
    if p.is_file():
        return p.stat().st_size
    return sum(f.stat().st_size for f in p.rglob("*") if f.is_file())


def candidatos(projeto: Path, dias: int) -> list[tuple[Path, str]]:
    wx = projeto / ".wx-migration"
    limite = time.time() - dias * 86400
    itens: list[tuple[Path, str]] = []
    runs = wx / "preflight" / "runs"
    if runs.is_dir():
        pastas = sorted((p for p in runs.iterdir() if p.is_dir()), key=lambda p: p.stat().st_mtime)
        for p in pastas[:-3]:  # sempre ficam as tres ultimas
            if p.stat().st_mtime < limite:
                itens.append((p, "execução antiga do pré-flight"))
    logs = wx / "logs"
    if logs.is_dir():
        for f in logs.iterdir():
            if f.is_file() and f.name not in ("zelador.md", "zelador.ultimo") and f.stat().st_mtime < limite:
                itens.append((f, "log antigo"))
    for pc in projeto.rglob("__pycache__"):
        if ".git" not in pc.parts and "node_modules" not in pc.parts and "target" not in pc.parts:
            itens.append((pc, "bytecode do Python"))
    wt = projeto / ".claude" / "worktrees"
    if wt.is_dir():
        for p in wt.iterdir():
            if p.is_dir() and p.stat().st_mtime < limite:
                itens.append((p, "worktree parado"))
    return itens


def limpar(projeto: Path, dias: int, executar: bool, se_vencido: bool) -> str:
    wx = projeto / ".wx-migration"
    logs = wx / "logs"
    carimbo = logs / "zelador.ultimo"
    if se_vencido and carimbo.is_file() and time.time() - carimbo.stat().st_mtime < 86400:
        return ""  # rodou ha menos de um dia; silencio
    itens = candidatos(projeto, dias)
    # Cinto e suspensorio: nada intocavel entra na lista, mesmo que candidatos() mude.
    itens = [(p, m) for p, m in itens if not set(p.relative_to(projeto).parts[:2]) & set(INTOCAVEIS)]
    total = sum(tamanho(p) for p, _ in itens)
    linhas = [f"| {datetime.now().isoformat(timespec='minutes')} | {'apagou' if executar else 'relatou'} | {len(itens)} itens | {total} bytes |"]
    if executar:
        for p, _ in itens:
            if p.is_dir():
                shutil.rmtree(p, ignore_errors=True)
            else:
                try:
                    p.unlink()
                except OSError:
                    pass
    if wx.is_dir():
        logs.mkdir(parents=True, exist_ok=True)
        md = logs / "zelador.md"
        if not md.is_file():
            md.write_text("# Zelador\n\nUma linha por rodada; tamanhos medidos antes de apagar.\n\n| quando | ação | itens | bytes |\n| --- | --- | ---: | ---: |\n", encoding="utf-8")
        with md.open("a", encoding="utf-8") as f:
            f.write(linhas[0] + "\n")
        if se_vencido or executar:
            carimbo.write_text(date.today().isoformat() + "\n", encoding="utf-8")
    saida = [f"zelador: {len(itens)} temporário(s), {total} bytes" + ("" if executar else " (só relatório; use --executar)")]
    saida += [f"- {p.relative_to(projeto)}: {motivo}" for p, motivo in itens[:20]]
    if len(itens) > 20:
        saida.append(f"- … e mais {len(itens) - 20}")
    return "\n".join(saida)


def espaco(projeto: Path, minimo_mb: int, executar: bool) -> str:
    """Sinal de outro agente: sem espaco. Mede o disco e, abaixo do minimo, limpa tudo que e temporario (dias=0)."""
    st = shutil.disk_usage(projeto)
    livre_mb = st.free // (1024 * 1024)
    if livre_mb >= minimo_mb:
        return f"zelador: {livre_mb} MB livres (mínimo {minimo_mb}); nada a fazer"
    texto = limpar(projeto, 0, executar, False)
    depois = shutil.disk_usage(projeto).free // (1024 * 1024)
    return f"zelador: {livre_mb} MB livres, abaixo de {minimo_mb}; " + texto.splitlines()[0] + f"; agora {depois} MB livres"


def hook_sessao(projeto: Path) -> int:
    if not (projeto / ".wx-migration").is_dir():
        return 0
    texto = limpar(projeto, 7, True, True)
    if texto:
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": "Zelador (limpeza diária de temporários): " + texto.splitlines()[0]}}, ensure_ascii=False))
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--project-root", type=Path, default=Path.cwd())
    sub = ap.add_subparsers(dest="cmd", required=True)
    l = sub.add_parser("limpar")
    l.add_argument("--dias", type=int, default=7)
    l.add_argument("--executar", action="store_true")
    l.add_argument("--se-vencido", action="store_true", help="so roda se a ultima rodada tiver mais de um dia")
    sub.add_parser("hook-sessao")
    e = sub.add_parser("espaco", help="sinal de outro agente: mede o disco e limpa se estiver abaixo do minimo"); e.add_argument("--minimo-mb", type=int, default=500); e.add_argument("--executar", action="store_true")
    a = ap.parse_args()
    projeto = a.project_root.resolve()
    if a.cmd == "hook-sessao":
        return hook_sessao(projeto)
    if a.cmd == "espaco":
        print(espaco(projeto, a.minimo_mb, a.executar)); return 0
    texto = limpar(projeto, a.dias, a.executar, a.se_vencido)
    print(texto or "zelador: já rodou hoje")
    return 0


if __name__ == "__main__":
    sys.exit(main())
