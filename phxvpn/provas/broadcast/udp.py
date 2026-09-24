#!/usr/bin/env python3
"""Emissor e receptores da prova de difusao (so a biblioteca padrao).

  ouvir PORTA IP_DA_PLACA SEGUNDOS [GRUPO...]
      Conta os datagramas que chegam na PORTA (qualquer destino), por
      destino de origem declarado na carga. Imprime JSON {destino: n}.
  mandar DESTINO PORTA N INTERVALO_S IP_DA_PLACA [TAMANHO]
      Manda N datagramas a DESTINO (broadcast ou grupo), saindo pela placa
      (bind no IP dela: o Linux escolhe a interface do broadcast limitado e
      do multicast pela origem). Imprime JSON {enviados, segundos}.
  ssdp-responder IP_DA_PLACA SEGUNDOS
      Responde M-SEARCH de 239.255.255.250:1900 em unicast, como um
      dispositivo UPnP. Imprime quantos respondeu.
  ssdp-buscar IP_DA_PLACA SEGUNDOS
      Manda um M-SEARCH e conta as respostas distintas (por IP).
"""
import json
import socket
import struct
import sys
import time


def receptor(porta, ip, grupos):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4 << 20)
    s.bind(("0.0.0.0", porta))
    for g in grupos:
        mreq = struct.pack("4s4s", socket.inet_aton(g), socket.inet_aton(ip))
        s.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
    return s


def emissor(ip):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
    s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_IF, socket.inet_aton(ip))
    s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 1)
    s.bind((ip, 0))
    return s


def ouvir(porta, ip, segundos, grupos):
    s = receptor(porta, ip, grupos)
    fim = time.time() + segundos
    cont = {}
    while True:
        falta = fim - time.time()
        if falta <= 0:
            break
        s.settimeout(falta)
        try:
            dado, _ = s.recvfrom(65535)
        except socket.timeout:
            break
        chave = dado.split(b"|", 1)[0].decode(errors="replace")
        cont[chave] = cont.get(chave, 0) + 1
    print(json.dumps(cont))


def mandar(destino, porta, n, intervalo, ip, tamanho=64):
    s = emissor(ip)
    carga = (destino + "|").encode()
    carga += b"x" * max(0, tamanho - len(carga))
    t0 = time.time()
    for _ in range(n):
        s.sendto(carga, (destino, porta))
        if intervalo:
            time.sleep(intervalo)
    print(json.dumps({"enviados": n, "segundos": round(time.time() - t0, 4)}))


def ssdp_responder(ip, segundos):
    s = receptor(1900, ip, ["239.255.255.250"])
    fim = time.time() + segundos
    n = 0
    while time.time() < fim:
        s.settimeout(max(0.01, fim - time.time()))
        try:
            dado, de = s.recvfrom(65535)
        except socket.timeout:
            break
        if dado.startswith(b"M-SEARCH * HTTP/1.1"):
            r = (
                "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=60\r\nST: ssdp:all\r\n"
                f"LOCATION: http://{ip}:8080/desc.xml\r\nUSN: uuid:phx-{ip}\r\n\r\n"
            )
            socket.socket(socket.AF_INET, socket.SOCK_DGRAM).sendto(r.encode(), de)
            n += 1
    print(json.dumps({"respondidos": n}))


def ssdp_buscar(ip, segundos):
    s = emissor(ip)
    m = (
        "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n"
        'MAN: "ssdp:discover"\r\nMX: 1\r\nST: ssdp:all\r\n\r\n'
    )
    s.sendto(m.encode(), ("239.255.255.250", 1900))
    fim = time.time() + segundos
    quem = set()
    while time.time() < fim:
        s.settimeout(max(0.01, fim - time.time()))
        try:
            dado, de = s.recvfrom(65535)
        except socket.timeout:
            break
        if dado.startswith(b"HTTP/1.1 200 OK"):
            quem.add(de[0])
    print(json.dumps({"respostas": sorted(quem)}))


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[0] == "ouvir":
        ouvir(int(a[1]), a[2], float(a[3]), a[4:])
    elif a[0] == "mandar":
        mandar(a[1], int(a[2]), int(a[3]), float(a[4]), a[5], *(int(x) for x in a[6:7]))
    elif a[0] == "ssdp-responder":
        ssdp_responder(a[1], float(a[2]))
    elif a[0] == "ssdp-buscar":
        ssdp_buscar(a[1], float(a[2]))
