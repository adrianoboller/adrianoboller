#!/usr/bin/env python3
"""Proxy HTTP CONNECT minimo, so para a prova do phxvpn por TCP.

Uso: proxy.py PORTA [usuario:senha] [registro]
Com credencial, exige Proxy-Authorization Basic (407 sem ela). O registro
anota cada CONNECT (destino e codigo) -- nunca o cabecalho de credencial,
para a prova poder conferir que a senha nao aparece em lugar nenhum do
lado do phxvpn.
"""
import base64, socket, sys, threading

porta = int(sys.argv[1])
cred = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] else None
registro = open(sys.argv[3], "a", buffering=1) if len(sys.argv) > 3 else sys.stderr
esperado = ("Basic " + base64.b64encode(cred.encode()).decode()) if cred else None


def bombear(de, para):
    try:
        while True:
            d = de.recv(65536)
            if not d:
                break
            para.sendall(d)
    except OSError:
        pass
    for s in (de, para):
        try:
            s.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass


def atender(c):
    cab = b""
    while b"\r\n\r\n" not in cab and len(cab) < 8192:
        d = c.recv(1)
        if not d:
            c.close()
            return
        cab += d
    linhas = cab.decode("latin1").split("\r\n")
    metodo, alvo, _ = linhas[0].split(" ", 2)
    auth = next((l.split(":", 1)[1].strip() for l in linhas[1:]
                 if l.lower().startswith("proxy-authorization:")), None)
    if metodo != "CONNECT":
        c.sendall(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n")
        c.close()
        print(f"{metodo} {alvo} 405", file=registro)
        return
    if esperado and auth != esperado:
        c.sendall(b"HTTP/1.1 407 Proxy Authentication Required\r\n"
                  b"Proxy-Authenticate: Basic realm=\"prova\"\r\n\r\n")
        c.close()
        print(f"CONNECT {alvo} 407", file=registro)
        return
    host, p = alvo.rsplit(":", 1)
    try:
        r = socket.create_connection((host.strip("[]"), int(p)), timeout=5)
        r.settimeout(None)
    except OSError as e:
        c.sendall(b"HTTP/1.1 502 Bad Gateway\r\n\r\n")
        c.close()
        print(f"CONNECT {alvo} 502 {e}", file=registro)
        return
    c.sendall(b"HTTP/1.1 200 Connection established\r\n\r\n")
    print(f"CONNECT {alvo} 200", file=registro)
    for s in (c, r):
        s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    threading.Thread(target=bombear, args=(c, r), daemon=True).start()
    bombear(r, c)


o = socket.socket()
o.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
o.bind(("0.0.0.0", porta))
o.listen(64)
while True:
    c, _ = o.accept()
    threading.Thread(target=atender, args=(c,), daemon=True).start()
