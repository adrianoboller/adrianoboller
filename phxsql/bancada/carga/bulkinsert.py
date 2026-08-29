#!/usr/bin/env python3
"""Prova do BULKINSERT pelo soquete: exclusividade, queda e prazo.

    cargo build --release
    python3 bancada/carga/bulkinsert.py [n_linhas]

O que teste unitario nao consegue provar e o que este arquivo existe para
provar: que a QUEDA DA CONEXAO solta a reserva de verdade -- soquete fechado no
meio da carga, sem `bulkinsert(false)`, e a tabela liberada do outro lado.

Tambem mede o que a reserva compra no laco: com a tabela reservada, a janela de
durabilidade fica aberta e a carga inteira vira um `fsync` so.
"""
import json
import os
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = "/tmp/phx-bulk"
PORTA = 5820
TOKEN = "carga"


def subir():
    subprocess.run(["pkill", "-x", "phxsqld"], check=False)
    time.sleep(1)
    subprocess.run(["rm", "-rf", BASE], check=False)
    os.makedirs(BASE, exist_ok=True)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump({"base": "base", "bind": f"127.0.0.1:{PORTA}",
                   "token": TOKEN, "web": {"ligado": False},
                   "recursos": {"carga_prazo_min": 30}}, f, indent=2)
    log = open(os.path.join(BASE, "servidor.log"), "a")
    subprocess.Popen(["setsid", PHXSQLD], cwd=BASE, stdout=log,
                     stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    time.sleep(2)


class Cliente:
    def __init__(self):
        self.s = socket.create_connection(("127.0.0.1", PORTA))
        self.f = self.s.makefile("rwb")

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit(f"{p['op']}: {r.get('erro')}")
        return r["resultado"]

    def matar(self):
        """Mata a conexao na marra, sem soltar nada -- o cliente que morreu.

        SO_LINGER com timeout zero manda RST em vez de FIN: e o que acontece
        quando o processo do outro lado e morto, e nao quando ele se despede.

        E o `self.f.close()` nao e zelo: `makefile` segura o descritor, e
        fechar so o soquete deixa o fd ABERTO -- o servidor nunca veria o fim
        da conexao. Foi assim que a primeira versao deste teste passou por
        engano dizendo que a reserva nao soltava.
        """
        import struct
        self.s.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER,
                          struct.pack("ii", 1, 0))
        self.f.close()
        self.s.close()


def linhas(a, b):
    return [{"id": i, "nome": f"Cliente {i:07d}"} for i in range(a, b + 1)]


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 20_000
    if not os.path.exists(PHXSQLD):
        sys.exit(f"nao achei {PHXSQLD} -- rode `cargo build --release` antes")
    subir()

    a, b = Cliente(), Cliente()
    a.ok({"op": "criar_database", "database": "loja"})
    a.ok({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "nome", "tipo": "Str(40)"}],
          "indices": [{"nome": "porId", "colunas": ["id"], "unico": True,
                       "primario": True}]})
    alvo = {"database": "loja", "tabela": "clientes"}
    print("=== 1. exclusividade ===\n")

    r = a.ok({"op": "bulkinsert", **alvo, "ligado": True})
    print(f"  A reservou; expira em {r['expira_em_s']}s (prazo {r['prazo_min']} min)")

    neg = b.fala({"op": "inserir", **alvo, "linha": {"id": 1, "nome": "x"}})
    print(f"  B tentou gravar: {neg['nome']} ({neg['codigo']}), repetir={neg['repetir']}")
    print(f"    «{neg['erro']}»")
    assert neg["nome"] == "EM_CARGA" and neg["repetir"] is True

    neg = b.fala({"op": "varrer", **alvo})
    print(f"  B tentou LER:    {neg['nome']} — a leitura tambem para")
    assert neg["nome"] == "EM_CARGA"

    print(f"\n=== 2. a carga do dono, {n} linhas ===\n")
    t = time.perf_counter()
    for i in range(0, n, 5_000):
        a.ok({"op": "inserir_lote", **alvo, "linhas": linhas(i + 1, min(i + 5_000, n))})
    s_lote = time.perf_counter() - t
    print(f"  em lote, com a tabela reservada: {s_lote:.2f}s  {n/s_lote:,.0f} linhas/s"
          .replace(",", "."))

    r = a.ok({"op": "bulkinsert", **alvo, "ligado": False})
    print(f"  A soltou: durou {r['durou_ms']} ms, sincronizada={r['sincronizada']}")

    v = b.ok({"op": "varrer", **alvo, "max": 1})
    print(f"  B agora le: {v['registros']} linhas na tabela")
    assert v["registros"] == n

    print("\n=== 3. a queda da conexao solta (a prova que importa) ===\n")
    c = Cliente()
    c.ok({"op": "bulkinsert", **alvo, "ligado": True})
    print("  C reservou")
    neg = b.fala({"op": "varrer", **alvo, "max": 1})
    assert neg["nome"] == "EM_CARGA"
    print("  B barrado, como esperado")

    c.matar()          # sem bulkinsert(false), sem despedida
    time.sleep(0.5)
    print("  C morreu com o soquete fechado, SEM soltar")

    depois = b.fala({"op": "varrer", **alvo, "max": 1})
    print(f"  B agora: {'LIBERADO' if depois.get('ok') else depois['nome']}")
    assert depois.get("ok"), "a reserva sobreviveu a morte do cliente"

    print("\n=== 4. a lista de quem reservou o que ===\n")
    d = Cliente()
    d.ok({"op": "bulkinsert", **alvo, "ligado": True})
    lista = b.ok({"op": "cargas"})
    print(f"  cargas ativas: {lista['total']}")
    for x in lista["cargas"]:
        print(f"    {x['database']}.{x['tabela']}  ligacao {x['ligacao']}"
              f"  ha {x['ha_ms']} ms  expira em {x['expira_em_s']}s")
    d.ok({"op": "bulkinsert", **alvo, "ligado": False})

    print("\nRESULTADO " + json.dumps({
        "linhas": n, "em_lote_s": round(s_lote, 3),
        "em_lote_por_s": round(n / s_lote),
        "exclusividade": True, "leitura_barrada": True,
        "queda_solta": True, "lista_ok": lista["total"] == 1,
    }))
    subprocess.run(["pkill", "-x", "phxsqld"], check=False)
