#!/usr/bin/env python3
"""Checa a resolucao DNS dos dominios do correio e da WX -- medido, nao citado.

    python3 bancada/servermail/dns.py

Os dominios do correio (phxsql.com.br, phxmail.com.br) sao os que o portao de
dominio do `correio-e2e` exige. A pergunta desta bateria e honesta: eles ja
existem no DNS? A WX (wxsolucoes.com.br) e onde o v7 do usuario seria armazenado
antes de o cadastro liberar -- aqui so se RESOLVE o nome, NUNCA se conecta.

Sai 0 sempre: a ausencia dos dominios do correio no DNS nao e defeito do banco,
e passo de implantacao (registrar + apontar no provedor). O script diz isso.
A ultima linha e `RESULTADO <json>`, para o relatorio nao adivinhar nada.
"""
import json
import socket
import sys

# Os do correio (master/slave do server mail) e o da WX (armazenamento do v7).
DOMINIOS = ["phxsql.com.br", "phxmail.com.br", "wxsolucoes.com.br"]


def resolver(host):
    try:
        infos = socket.getaddrinfo(host, None)
        return sorted({i[4][0] for i in infos})
    except OSError:
        return None


def main():
    resultado = {}
    print("resolucao DNS (medida agora, nesta maquina):")
    for host in DOMINIOS:
        ips = resolver(host)
        resultado[host] = ips
        if ips:
            print(f"  {host:22} RESOLVE   -> {', '.join(ips)}")
        else:
            print(f"  {host:22} NAO RESOLVE")

    correio = [d for d in ("phxsql.com.br", "phxmail.com.br") if not resultado[d]]
    print()
    if correio:
        print(
            "CONCLUSAO (honesta): os dominios do correio "
            + " e ".join(correio)
            + " ainda NAO estao no DNS. Isso e passo de IMPLANTACAO (registrar o\n"
            "  dominio e apontar A/MX no provedor -- ex.: Cloudflare), NAO defeito do\n"
            "  banco. O portao de dominio do correio ja funciona (ver correio-e2e)."
        )
    else:
        print("CONCLUSAO: os dois dominios do correio resolvem.")
    if resultado.get("wxsolucoes.com.br"):
        print(
            "  A WX (wxsolucoes.com.br) resolve, mas esta bateria NAO se conecta nela\n"
            "  -- e producao real do dono. O armazenamento do v7 la e SIMULADO."
        )
    print("\nRESULTADO " + json.dumps(resultado, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
