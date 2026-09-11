#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Cloudflare FALSO -- imita so o pedaco da API v4 que o provisionador usa,
para PROVAR o mecanismo sem tocar o Cloudflare de verdade nem gastar token.

Rotas imitadas (identicas as reais, conferidas no doc oficial):
  GET  /client/v4/zones/<zona>/dns_records?type=A&name=<fqdn>
  POST /client/v4/zones/<zona>/dns_records            (cria)
  PUT  /client/v4/zones/<zona>/dns_records/<id>       (atualiza)

Envelope identico ao real: {"success":bool,"errors":[...],"result":...}.
Confere o cabecalho Authorization: Bearer <token> -- token errado -> 403,
que e o que permite a prova pegar quando o segredo esta errado.

Rota EXTRA, marcada, so para a prova ler o estado (nao existe no Cloudflare):
  GET  /__estado
"""
import json
import os
import re
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

TOKEN_ESPERADO = os.environ.get("FAKE_CF_TOKEN", "token-de-teste")

# estado em memoria: id -> registro
REGISTROS = {}
_seq = [0]

ROTA = re.compile(r"^/client/v4/zones/([^/]+)/dns_records(?:/([^/?]+))?$")


def novo_id():
    _seq[0] += 1
    return "rec%08x" % _seq[0]


class Mao(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass  # silencio; a prova fala por assercao, nao por log

    # ---- util ----
    def responde(self, codigo, corpo):
        dados = json.dumps(corpo).encode("utf-8")
        self.send_response(codigo)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(dados)))
        self.end_headers()
        self.wfile.write(dados)

    def ok(self, result):
        self.responde(200, {"success": True, "errors": [], "result": result})

    def erro(self, codigo_http, code, msg):
        self.responde(codigo_http, {"success": False,
                                    "errors": [{"code": code, "message": msg}],
                                    "result": None})

    def autorizado(self):
        cab = self.headers.get("Authorization", "")
        if cab != "Bearer " + TOKEN_ESPERADO:
            self.erro(403, 9109, "Invalid access token")
            return False
        return True

    def corpo_json(self):
        n = int(self.headers.get("Content-Length", "0") or "0")
        if n == 0:
            return {}
        return json.loads(self.rfile.read(n).decode("utf-8"))

    # ---- GET ----
    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/__estado":  # rota de prova, nao do Cloudflare
            self.ok(list(REGISTROS.values()))
            return
        if not self.autorizado():
            return
        m = ROTA.match(u.path)
        if not m or m.group(2):
            self.erro(404, 7003, "Could not route to endpoint")
            return
        zona = m.group(1)
        q = parse_qs(u.query)
        nome = (q.get("name") or [None])[0]
        tipo = (q.get("type") or [None])[0]
        achados = [r for r in REGISTROS.values()
                   if r["zone_id"] == zona
                   and (nome is None or r["name"] == nome)
                   and (tipo is None or r["type"] == tipo)]
        self.ok(achados)

    # ---- POST (cria) ----
    def do_POST(self):
        if not self.autorizado():
            return
        m = ROTA.match(urlparse(self.path).path)
        if not m or m.group(2):
            self.erro(404, 7003, "Could not route to endpoint")
            return
        zona = m.group(1)
        b = self.corpo_json()
        for campo in ("type", "name", "content"):
            if not b.get(campo):
                self.erro(400, 1004, "DNS record %s is required" % campo)
                return
        rid = novo_id()
        reg = {"id": rid, "zone_id": zona, "type": b["type"], "name": b["name"],
               "content": b["content"], "ttl": b.get("ttl", 1),
               "proxied": bool(b.get("proxied", False))}
        REGISTROS[rid] = reg
        self.ok(reg)

    # ---- PUT (atualiza) ----
    def do_PUT(self):
        if not self.autorizado():
            return
        m = ROTA.match(urlparse(self.path).path)
        if not m or not m.group(2):
            self.erro(404, 7003, "Could not route to endpoint")
            return
        rid = m.group(2)
        if rid not in REGISTROS:
            self.erro(404, 81044, "Record does not exist")
            return
        b = self.corpo_json()
        reg = REGISTROS[rid]
        reg.update({"type": b.get("type", reg["type"]),
                    "name": b.get("name", reg["name"]),
                    "content": b.get("content", reg["content"]),
                    "ttl": b.get("ttl", reg["ttl"]),
                    "proxied": bool(b.get("proxied", reg["proxied"]))})
        self.ok(reg)


def main():
    porta = int(sys.argv[1]) if len(sys.argv) > 1 else 8787
    srv = ThreadingHTTPServer(("127.0.0.1", porta), Mao)
    print("cloudflare-falso ouvindo em 127.0.0.1:%d (token=%s)" % (porta, TOKEN_ESPERADO), flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
