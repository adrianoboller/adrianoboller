#!/usr/bin/env python3
"""Hook do semaforo: vermelho aguardando voce, amarelo trabalhando, verde pronto.

A ideia veio de um semaforo fisico ao lado do monitor, aceso pelo estado do
Claude Code. Os eventos dos hooks ja dao as tres cores sem ninguem inferir
nada: SessionStart e Stop acendem o VERDE (pronto para tarefa nova),
UserPromptSubmit e PreToolUse acendem o AMARELO (em execucao), e Notification
acende o VERMELHO (o Claude Code parou e espera confirmacao ou resposta).

Desligado custa zero: o primeiro passo e um `stat` no arquivo de configuracao,
e sem ele o hook sai antes de ler o stdin. Ligado, faz no maximo tres coisas,
todas curtas e nenhuma fatal: grava o estado num arquivo, chama uma URL (o
ESP32 ou o painel local) e/ou roda um comando -- o que estiver configurado em
~/.wx-claude-code/semaforo.json:

    {"url": "http://192.168.0.50/luz", "comando": "", "arquivo": true}

A URL recebe GET ?cor=verde|amarelo|vermelho; o comando recebe {cor} no lugar.
Falha de rede e silenciosa e limitada a 1,5 s: semaforo que nao acende nao
pode travar a sessao.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

CASA = Path(os.environ.get("WX_CASA") or Path.home() / ".wx-claude-code")
CONFIG = CASA / "semaforo.json"
ESTADO = CASA / "semaforo.estado"
COR_DO_EVENTO = {
    "SessionStart": "verde",
    "Stop": "verde",
    "UserPromptSubmit": "amarelo",
    "PreToolUse": "amarelo",
    "Notification": "vermelho",
}


def acender(cor: str, cfg: dict, evento: str = "") -> list[str]:
    feito = []
    if cfg.get("arquivo", True):
        try:
            CASA.mkdir(parents=True, exist_ok=True)
            ESTADO.write_text(json.dumps({"cor": cor, "evento": evento, "em": int(time.time())}) + "\n", encoding="utf-8")
            feito.append("arquivo")
        except OSError:
            pass
    url = cfg.get("url")
    if url:
        sep = "&" if "?" in url else "?"
        try:
            with urllib.request.urlopen(url + sep + urllib.parse.urlencode({"cor": cor}), timeout=1.5):
                feito.append("url")
        except Exception:  # noqa: BLE001 -- semaforo apagado nao pode derrubar a sessao
            pass
    cmd = cfg.get("comando")
    if cmd:
        try:
            subprocess.run(cmd.replace("{cor}", cor), shell=True, timeout=3, capture_output=True)
            feito.append("comando")
        except Exception:  # noqa: BLE001
            pass
    return feito


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--estado":
        print(ESTADO.read_text(encoding="utf-8").strip() if ESTADO.is_file() else "apagado (sem estado gravado)")
        return 0
    if not CONFIG.is_file():
        # desligado: um stat e nada mais -- nem o stdin e lido
        if len(sys.argv) > 1 and sys.argv[1] == "--testar":
            print(f"semaforo desligado: crie {CONFIG} com a url do seu semaforo")
            return 1
        return 0
    try:
        cfg = json.loads(CONFIG.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return 0
    if len(sys.argv) > 2 and sys.argv[1] == "--testar":
        cor = sys.argv[2]
        if cor not in ("verde", "amarelo", "vermelho"):
            print("cor: verde, amarelo ou vermelho")
            return 2
        print(f"{cor}: {', '.join(acender(cor, cfg, 'teste')) or 'nada configurado'}")
        return 0
    try:
        entrada = json.load(sys.stdin)
    except ValueError:
        return 0
    evento = entrada.get("hook_event_name", "")
    cor = COR_DO_EVENTO.get(evento)
    if cor:
        acender(cor, cfg, evento)
    return 0


if __name__ == "__main__":
    sys.exit(main())
