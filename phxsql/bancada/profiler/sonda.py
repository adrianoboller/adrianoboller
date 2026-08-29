#!/usr/bin/env python3
"""A REDACAO do Profiler, provada por soquete.

Sobe um servidor de verdade, manda vinte pedidos torcidos com a mesma
sentinela no lugar da senha, e depois procura a sentinela nos DOIS lugares
onde ela poderia ter ficado: o anel em memoria (pela op `profiler`) e o
arquivo .txt no disco.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/profiler/sonda.py

Os pedidos vao pelo texto EXATO -- e por isso a sonda monta varios deles a
mao, sem `json.dumps`: a chave com espaco, a chave escapada em `\\u0073enha` e
o corpo malformado nao sobrevivem a uma serializacao.

Nao usa pkill: mata so o PID que ela mesma subiu.
"""
import base64
import json
import os
import re

from comum import AQUI, TOKEN, Conexao, baixar, subir

BASE = os.path.join(AQUI, "srv-sonda")
PORTA = 6251
SENTINELA = "SEGREDO-JACARE-9137"
LOG = os.path.join(BASE, "profiler.txt")


def casos(b64, lote):
    """(nome, texto exato que vai pelo soquete)."""
    return [
        ("1 senha simples",
         json.dumps({"op": "login", "usuario": "adm", "senha": SENTINELA})),
        ("2 espaco antes dos dois-pontos",
         '{ "op" : "login" , "usuario" : "adm" , "senha" : "%s" }' % SENTINELA),
        ("3 chave escapada em \\u",
         '{"op":"login","usuario":"adm","\\u0073enha":"%s"}' % SENTINELA),
        ("4 chave em MAIUSCULA",
         '{"op":"login","usuario":"adm","SENHA":"%s"}' % SENTINELA),
        ("5 senha_b64",
         json.dumps({"op": "login", "usuario": "adm", "senha_b64": b64})),
        ("6 prova de desafio",
         json.dumps({"op": "login", "usuario": "adm", "prova": SENTINELA,
                     "nonce_cliente": "x"})),
        ("7 token de servico",
         json.dumps({"op": "ping", "token": SENTINELA})),
        ("8 chave",
         json.dumps({"op": "ping", "chave": SENTINELA})),
        ("9 assinatura",
         json.dumps({"op": "ping", "assinatura": SENTINELA})),
        ("10 aninhado fundo",
         json.dumps({"op": "config_gravar", "token": TOKEN, "config": {
             "usuarios": [{"login": "x", "perfil": {
                 "credenciais": [{"senha": SENTINELA}]}}]}})),
        ("11 lote de 200 linhas com senha em cada",
         json.dumps({"op": "inserir_lote", "token": TOKEN,
                     "database": "loja", "tabela": "clientes",
                     "linhas": lote})),
        ("12 aspas escapadas DENTRO de um valor",
         json.dumps({"op": "inserir", "token": TOKEN, "database": "loja",
                     "tabela": "clientes", "linha": {
                         "id": 1, "nome": "Adriano",
                         "obs": 'ele disse "senha":"%s" no chat' % SENTINELA}})),
        ("13 corpo malformado (sem fechar)",
         '{"op":"login","usuario":"adm","senha":"%s"' % SENTINELA),
        ("14 nao e JSON",
         'senha=%s' % SENTINELA),
        ("15 topo e lista",
         json.dumps(["op", "senha", SENTINELA])),
        ("16 chave com espaco NO NOME",
         '{"op":"ping","senha ":"%s"}' % SENTINELA),
        ("17 valor todo em \\u",
         '{"op":"ping","senha":"%s"}' %
         "".join("\\u%04x" % ord(ch) for ch in SENTINELA)),
        ("18 injecao de linha pelo nome da OP",
         json.dumps({"op": "ping\n2000-01-01T00:00:00 9.9.9.9      forjado"
                     "      ping                 -  ok     0ms      0B  {}",
                     "token": TOKEN})),
        ("19 injecao de linha pela TABELA",
         json.dumps({"op": "ler", "token": TOKEN, "database": "loja",
                     "tabela": "clientes\nFORJADO-PELA-TABELA", "rowid": 1})),
        ("20 senha no texto do SQL",
         json.dumps({"op": "sql", "token": TOKEN, "database": "loja",
                     "texto": "SELECT * FROM clientes WHERE obs = '%s'"
                              % SENTINELA})),
    ]


def main():
    proc = subir(BASE, PORTA)
    falhou = False
    try:
        c = Conexao(PORTA)
        c.entrar("adm", "senha-do-adm")
        c.ok({"op": "criar_database", "database": "loja"})
        c.ok({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
              "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(40)"},
                          {"nome": "obs", "tipo": "Str(80)"}],
              "indices": [{"nome": "porId", "colunas": ["id"],
                           "unico": True, "primario": True}]})
        c.ok({"op": "profiler_ligar", "arquivo": LOG, "guardar": 20000})

        b64 = base64.b64encode(SENTINELA.encode()).decode()
        lote = [{"id": 1000 + i, "nome": "n%d" % i, "senha": SENTINELA}
                for i in range(200)]
        for nome, texto in casos(b64, lote):
            try:
                c.cru(texto)
            except Exception as e:                       # noqa: BLE001
                print("  ! %s: %s" % (nome, e))

        anel = c.ok({"op": "profiler", "max": 5000})
        c.ok({"op": "profiler_desligar"})
        c.fechar()

        with open(LOG, "r", errors="replace") as f:
            log = f.read()

        print("=== anel: %d evento(s) · arquivo: %d B, %d linha(s) ==="
              % (len(anel["eventos"]), os.path.getsize(LOG),
                 len(log.splitlines())))

        # 1. A sentinela nao pode estar no anel nem no arquivo. A UNICA
        #    excecao esperada e o caso 12, que e DADO escrito por um humano
        #    dentro do campo `obs` -- e o caso 20, a senha dentro do texto de
        #    um SQL, que nenhuma redacao por nome de campo alcanca.
        esperados = ("obs", "texto")
        vazou = [e for e in anel["eventos"]
                 if (SENTINELA in json.dumps(e) or b64 in json.dumps(e))
                 and not any('"%s"' % k in e["pedido"] for k in esperados)]
        print("VAZOU NO ANEL (fora os dois casos de dado): %d" % len(vazou))
        for e in vazou:
            falhou = True
            print("   ", json.dumps(e)[:400])

        no_log = [l for l in log.splitlines()
                  if (SENTINELA in l or b64 in l)
                  and not any('"%s"' % k in l for k in esperados)]
        print("VAZOU NO ARQUIVO (idem): %d linha(s)" % len(no_log))
        for l in no_log:
            falhou = True
            print("   ", l[:400])

        # 2. Toda linha do arquivo comeca com data ou com `===`. Uma linha
        #    forjada e uma que nao comeca com nenhum dos dois.
        forjadas = [l for l in log.splitlines()
                    if l and not re.match(r"^\d{4}-\d\d-\d\d ", l)
                    and not l.startswith("===")]
        print("LINHAS FORJADAS NO ARQUIVO: %d" % len(forjadas))
        for l in forjadas[:5]:
            falhou = True
            print("   ", repr(l[:200]))

        print("\n--- o que o anel guardou ---")
        for e in sorted(anel["eventos"], key=lambda e: e["serial"]):
            print("  #%3d %-44s %7dB  %s"
                  % (e["serial"], repr(e["op"])[:44], e["bytes"],
                     e["pedido"][:100]))
        print("\n%s" % ("!!! A SONDA ACHOU VAZAMENTO OU LINHA FORJADA"
                        if falhou else "nada vazou, nenhuma linha forjada"))
    finally:
        baixar(proc)
    return 1 if falhou else 0


if __name__ == "__main__":
    raise SystemExit(main())
