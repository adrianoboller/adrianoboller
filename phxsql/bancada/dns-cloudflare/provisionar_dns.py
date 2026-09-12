#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
provisionar_dns.py -- cadastra o IP FIXO de um servermail nos dois registros DNS
da empresa, via API do Cloudflare, de forma IDEMPOTENTE (cria ou atualiza).

    <empresa>.phxmail.com.br ->  <ip_fixo>   (A)

Uso:
    CF_API_TOKEN=...  CF_ZONAS='{"phxmail.com.br":"<zid>"}' \\
        python3 provisionar_dns.py <empresa> <ip_fixo>

Regras desta casa que a ferramenta honra:
  - Zero dependencia: so a stdlib do Python (urllib). Nao e o motor Rust -- isto
    e ferramenta de OPS; o motor continua zero-deps. (O "meio" de o proprio
    servidor falar HTTPS fica como decisao do dono; ver docs/CORREIO-DNS.md.)
  - Segredo nunca em texto puro: o token vem SO de CF_API_TOKEN, nunca em argv,
    nunca impresso, nunca no repositorio.
  - Idempotente: rodar de novo nao duplica registro -- confere por nome e
    atualiza o que ja existe. IP mudou? roda de novo e atualiza.
  - So zonas conhecidas: os dois dominios permitidos, nada alem.

Zero saida do token: em erro imprime o codigo/mensagem do Cloudflare, nunca o
cabecalho de autorizacao.
"""
import json
import os
import re
import sys
import urllib.request
import urllib.error

BASE = os.environ.get("CF_API_BASE", "https://api.cloudflare.com/client/v4").rstrip("/")
DOMINIOS = ("phxmail.com.br",)  # so o phxmail (o dono removeu o phxsql.com.br)
TTL = 3600

RE_EMPRESA = re.compile(r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$")  # rotulo DNS valido
RE_IPV4 = re.compile(r"^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$")


class ErroCF(Exception):
    pass


def _ipv4_valido(ip):
    m = RE_IPV4.match(ip)
    return bool(m) and all(0 <= int(g) <= 255 for g in m.groups())


def _pedir(metodo, caminho, token, corpo=None):
    """Uma chamada a API. Devolve o campo 'result'; levanta ErroCF no envelope falso."""
    url = BASE + caminho
    dados = json.dumps(corpo).encode("utf-8") if corpo is not None else None
    req = urllib.request.Request(url, data=dados, method=metodo)
    req.add_header("Authorization", "Bearer " + token)
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            env = json.loads(r.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        # o Cloudflare devolve o envelope tambem em 4xx (ex.: 403 token invalido)
        try:
            env = json.loads(e.read().decode("utf-8"))
        except Exception:
            raise ErroCF("HTTP %s sem envelope JSON" % e.code)
    if not env.get("success"):
        errs = env.get("errors") or [{"message": "erro desconhecido"}]
        raise ErroCF("; ".join("%s(%s)" % (x.get("message"), x.get("code")) for x in errs))
    return env.get("result")


def _zona_de(dominio, zonas):
    zid = zonas.get(dominio)
    if not zid:
        raise ErroCF("zona nao configurada para %s (defina em CF_ZONAS)" % dominio)
    return zid


def cadastrar_um(dominio, empresa, ip, token, zonas):
    """Cria ou atualiza o registro A <empresa>.<dominio> -> ip. Idempotente."""
    zid = _zona_de(dominio, zonas)
    fqdn = "%s.%s" % (empresa, dominio)
    achados = _pedir("GET", "/zones/%s/dns_records?type=A&name=%s" % (zid, fqdn), token)
    corpo = {"type": "A", "name": fqdn, "content": ip, "ttl": TTL, "proxied": False}
    if achados:
        rid = achados[0]["id"]
        res = _pedir("PUT", "/zones/%s/dns_records/%s" % (zid, rid), token, corpo)
        acao = "atualizado"
    else:
        res = _pedir("POST", "/zones/%s/dns_records" % zid, token, corpo)
        acao = "criado"
    return {"fqdn": fqdn, "zona": dominio, "record_id": res["id"],
            "content": res["content"], "acao": acao}


def provisionar(empresa, ip, token, zonas):
    if not RE_EMPRESA.match(empresa):
        raise ErroCF("nome de empresa invalido para DNS: %r" % empresa)
    if not _ipv4_valido(ip):
        raise ErroCF("IP fixo invalido: %r" % ip)
    return [cadastrar_um(d, empresa, ip, token, zonas) for d in DOMINIOS]


def main(argv):
    if len(argv) != 3:
        print("uso: provisionar_dns.py <empresa> <ip_fixo>", file=sys.stderr)
        return 2
    empresa, ip = argv[1], argv[2]
    token = os.environ.get("CF_API_TOKEN", "")
    if not token:
        print("CF_API_TOKEN nao definido (o token nunca vai em argv nem no repo)", file=sys.stderr)
        return 2
    try:
        zonas = json.loads(os.environ.get("CF_ZONAS", "{}"))
    except json.JSONDecodeError:
        print("CF_ZONAS nao e JSON valido", file=sys.stderr)
        return 2
    try:
        saidas = provisionar(empresa, ip, token, zonas)
    except ErroCF as e:
        print("FALHOU: %s" % e, file=sys.stderr)  # nunca imprime o token
        return 1
    for s in saidas:
        print("  %-9s %-28s -> %s   [%s]  id=%s" %
              (s["acao"], s["fqdn"], s["content"], s["zona"], s["record_id"]))
    print("OK: %d registros no IP fixo %s" % (len(saidas), ip))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
