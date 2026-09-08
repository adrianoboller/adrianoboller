#!/usr/bin/env python3
"""Painel do semaforo, sem hardware: um servidor HTTP minimo (so std) que recebe
GET /luz?cor=... do hook e mostra a cor no terminal e numa pagina.

Serve para testar a configuracao antes de ter o ESP32, e para quem prefere o
semaforo num segundo monitor. Uso:

    python3 painel.py --porta 8777
    # e em ~/.wx-claude-code/semaforo.json: {"url": "http://127.0.0.1:8777/luz"}
"""
from __future__ import annotations

import argparse
import json
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

ESTADO = {"cor": "apagado", "em": 0}
LUZ = {"vermelho": "\033[31m●\033[0m", "amarelo": "\033[33m●\033[0m", "verde": "\033[32m●\033[0m", "apagado": "○"}
PAGINA = """<!doctype html><meta charset="utf-8"><title>Semáforo WX Claude Code</title>
<style>body{margin:0;background:#010418;color:#EDEDF3;font-family:system-ui;display:flex;flex-direction:column;align-items:center;justify-content:center;height:100vh}
.caixa{background:#121527;border:2px solid #2E3454;border-radius:28px;padding:24px;display:flex;flex-direction:column;gap:18px}
.luz{width:110px;height:110px;border-radius:50%;background:#1B1F33;transition:background .2s,box-shadow .2s}
.on-vermelho .r{background:#E2261C;box-shadow:0 0 40px #E2261C}.on-amarelo .a{background:#F7B733;box-shadow:0 0 40px #F7B733}.on-verde .v{background:#2FBF71;box-shadow:0 0 40px #2FBF71}
p{color:#9AA0B8;margin-top:22px}</style>
<div id="s" class="caixa"><div class="luz r"></div><div class="luz a"></div><div class="luz v"></div></div><p id="t">…</p>
<script>const L={vermelho:'aguardando você',amarelo:'em execução',verde:'pronto para a próxima tarefa',apagado:'sem estado'};
async function f(){const e=await (await fetch('/estado')).json();document.getElementById('s').className='caixa on-'+e.cor;document.getElementById('t').textContent=L[e.cor]||e.cor}
f();setInterval(f,1000)</script>"""


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):  # silencio: o terminal e do semaforo
        pass

    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/luz":
            cor = parse_qs(u.query).get("cor", [""])[0]
            if cor in LUZ and cor != "apagado":
                ESTADO.update(cor=cor, em=int(time.time()))
                print(f"\r{LUZ[cor]} {cor:<9} {time.strftime('%H:%M:%S')}", end="", flush=True)
                self.send_response(204); self.end_headers(); return
            self.send_response(400); self.end_headers(); return
        if u.path == "/estado":
            corpo = json.dumps(ESTADO).encode()
            self.send_response(200); self.send_header("content-type", "application/json"); self.end_headers(); self.wfile.write(corpo); return
        corpo = PAGINA.encode()
        self.send_response(200); self.send_header("content-type", "text/html; charset=utf-8"); self.end_headers(); self.wfile.write(corpo)


def main() -> int:
    ap = argparse.ArgumentParser(description="painel do semaforo, sem hardware")
    ap.add_argument("--porta", type=int, default=8777)
    ap.add_argument("--endereco", default="127.0.0.1")
    a = ap.parse_args()
    print(f"semáforo em http://{a.endereco}:{a.porta}/  · o hook chama /luz?cor=…  · abra a página no navegador")
    HTTPServer((a.endereco, a.porta), H).serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
