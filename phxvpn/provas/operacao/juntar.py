#!/usr/bin/env python3
"""Grava UMA secao no resultados.json da operacao, sem apagar as outras.

Uso: juntar.py SECAO ARQUIVO_JSON_DA_SECAO

As quatro provas desta pasta (historico, force-cookie, mtu, mlock) rodam em
horas diferentes; cada uma reescreve so a sua chave, com a data dela dentro,
para a pagina de testes nunca juntar corridas de dias diferentes sem dizer.
"""
import json
import os
import sys

secao, arquivo = sys.argv[1], sys.argv[2]
alvo = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resultados.json")
try:
    tudo = json.load(open(alvo))
except (OSError, ValueError):
    tudo = {}
tudo.setdefault("prova", "operacao do phxvpn: historico, force-cookie, MTU do P2P e mlock")
tudo[secao] = json.load(open(arquivo))
json.dump(tudo, open(alvo, "w"), ensure_ascii=False, indent=1)
open(alvo, "a").write("\n")
print("gravado:", alvo, "secao", secao)
