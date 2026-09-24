#!/usr/bin/env python3
"""Faz o papel do OpenVPN GUI: responde pela interface de gerencia quando o
openvpn pede usuario e senha do proxy (`http-proxy ... auto`).

Uso: gerencia.py PORTA USUARIO SENHA_ARQ REGISTRO
A senha vem de um ARQUIVO (nunca por argumento, que o `ps` mostra). O
registro anota os pedidos que chegaram (sem a senha).
"""
import socket, sys, time

porta, usuario, senha_arq, registro = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
senha = open(senha_arq).read().strip()
log = open(registro, "a", buffering=1)
fim = time.time() + 120
while time.time() < fim:
    try:
        c = socket.create_connection(("127.0.0.1", int(porta)), timeout=2)
        break
    except OSError:
        time.sleep(0.2)
else:
    sys.exit(1)
c.settimeout(None)
f = c.makefile("rwb", buffering=0)
while True:
    l = f.readline()
    if not l:
        break
    t = l.decode(errors="replace").strip()
    if t.startswith(">PASSWORD:Need"):
        print(t, file=log)
        tipo = t.split("'")[1]
        f.write(f'username "{tipo}" {usuario}\n'.encode())
        f.write(f'password "{tipo}" "{senha}"\n'.encode())
    elif ">STATE:" in t and ",CONNECTED," in t:
        print("CONECTOU", file=log)
