#!/usr/bin/env python3
"""SOCKS5 minimo (RFC 1928, so CONNECT), com usuario e senha (RFC 1929)
opcionais -- so para a prova do phxvpn.

Uso: socks.py PORTA [usuario:senha] [registro]
Com credencial, recusa quem nao a manda (metodo 0xFF) ou erra. O registro
anota cada pedido (destino e resultado), nunca a senha.
"""
import socket, struct, sys, threading

porta = int(sys.argv[1])
cred = sys.argv[2] if len(sys.argv) > 2 and sys.argv[2] else None
registro = open(sys.argv[3], "a", buffering=1) if len(sys.argv) > 3 else sys.stderr


def ler(c, n):
    b = b""
    while len(b) < n:
        d = c.recv(n - len(b))
        if not d:
            raise OSError("fechou")
        b += d
    return b


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
    try:
        ver, n = ler(c, 2)
        metodos = ler(c, n)
        quer = 2 if cred else 0
        if ver != 5 or quer not in metodos:
            c.sendall(b"\x05\xff")
            print("METODO recusado", file=registro)
            return c.close()
        c.sendall(bytes([5, quer]))
        if cred:
            _, ul = ler(c, 2)
            u = ler(c, ul)
            (pl,) = ler(c, 1)
            p = ler(c, pl)
            if f"{u.decode()}:{p.decode()}" != cred:
                c.sendall(b"\x01\x01")
                print("AUTH recusada", file=registro)
                return c.close()
            c.sendall(b"\x01\x00")
        _, cmd, _, tipo = ler(c, 4)
        if tipo == 1:
            host = socket.inet_ntoa(ler(c, 4))
        elif tipo == 3:
            host = ler(c, ler(c, 1)[0]).decode()
        else:
            host = socket.inet_ntop(socket.AF_INET6, ler(c, 16))
        (p,) = struct.unpack(">H", ler(c, 2))
        if cmd != 1:
            c.sendall(b"\x05\x07\x00\x01" + b"\x00" * 6)
            print(f"CMD {cmd} {host}:{p} recusado", file=registro)
            return c.close()
        r = socket.create_connection((host, p), timeout=5)
        r.settimeout(None)
        c.sendall(b"\x05\x00\x00\x01" + b"\x00" * 6)
        print(f"CONNECT {host}:{p} ok", file=registro)
        threading.Thread(target=bombear, args=(r, c), daemon=True).start()
        bombear(c, r)
    except OSError as e:
        print(f"erro {e}", file=registro)
        c.close()


s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("0.0.0.0", porta))
s.listen(16)
while True:
    c, _ = s.accept()
    threading.Thread(target=atender, args=(c,), daemon=True).start()
