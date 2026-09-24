#!/usr/bin/env python3
"""O HTTPS que ja usava a porta: responde uma frase fixa, com o certificado
dado. Uso: https.py PORTA CERT CHAVE [ENDERECO=127.0.0.1]"""
import http.server, ssl, sys


class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        corpo = b"phxvpn-prova-https\n"
        self.send_response(200)
        self.send_header("Content-Length", str(len(corpo)))
        self.end_headers()
        self.wfile.write(corpo)

    def log_message(self, *a):
        pass


onde = sys.argv[4] if len(sys.argv) > 4 else "127.0.0.1"
s = http.server.ThreadingHTTPServer((onde, int(sys.argv[1])), H)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(sys.argv[2], sys.argv[3])
s.socket = ctx.wrap_socket(s.socket, server_side=True)
s.serve_forever()
