#!/usr/bin/env python3
"""Quanto tempo a trava global fica PRESA, e quanto disso e o `fsync`?

    python3 bancada/concorrencia/quanto-a-trava-fica-presa.py

A pergunta que ficou nomeada e nao medida
-----------------------------------------
O `docs/CONCORRENCIA.md` fecha com uma lista do que continua por medir, e um
dos itens e este:

    «O custo do `fsync` sob a trava, em milissegundos. O mapa prova que ele
     acontece ali; quanto ele dura e medicao, e nao foi feita.»

O mapa estatico responde ONDE: 24 das 76 secoes criticas alcancam `sync_all`
por caminho proprio. Ele nao responde QUANTO, e nao tem como -- ler o fonte
nao cronometra disco.

O que se mede, e o par que separa resultado de palpite
-------------------------------------------------------
A telemetria ja soma, em microssegundos, o tempo que cada tomada da trava
ficou na mao (`contar_trava`, no `Drop` do `TravaMedida`). Entao a conta e
direta: `trava_us` acumulado dividido pelas gravacoes da rodada da o tempo
MEDIO que uma gravacao segura o servidor inteiro.

O experimento e o PAR, e nao um numero solto:

  * `durabilidade: por_lote`     -- o `fsync` acontece a cada 200 operacoes;
  * `durabilidade: por_operacao` -- o `fsync` acontece em TODAS.

Mesmo binario, mesmo codigo, mesmo caminho, mesma trava. A UNICA diferenca e
a frequencia do `fsync` sob a trava, entao a diferenca entre as duas curvas
E o `fsync` sob a trava. Sem esse par, o numero de uma rodada sozinha mediria
tambem o `open` das sete tabelas, o indice e o JSON -- e o `fsync` levaria a
culpa de tudo.

E ha um terceiro ponto, que e o controle de CIMA: a leitura (`varrer`), que
toma a mesma trava e nao sincroniza nada. Se ela subir junto com a escrita
entre as duas rodadas, quem mudou nao foi o `fsync`: foi a maquina.

Por que EFEITO medido por dentro, e nao cronometro por fora
------------------------------------------------------------
Cronometrar o pedido pelo cliente mede o pedido inteiro -- rede, JSON,
despacho, portao de permissao e a fila. A pergunta e outra: quanto a trava
fica PRESA, que e o que a proxima conexao espera. O unico lugar de onde isso
se ve e de dentro do `Drop` da guarda, e a telemetria ja o via.

Nao publica sujo
-----------------
Mesma regra do resto desta pasta: o `quieta.Vigia` mede a maquina nas duas
pontas e RECUSA imprimir quando ela se mexeu. Numa maquina ocupada o `fsync`
demora mais -- e demorar mais e exatamente o sintoma que se procura, entao o
ruido aponta para o mesmo lado da hipotese. `--mesmo-sujo` so para depurar o
proprio arnes.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
sys.path.insert(0, str(AQUI))
import quieta  # noqa: E402

PHXSQLD = Path(os.environ.get("PHX_PHXSQLD", RAIZ / "target/release/phxsqld"))
SENHA = "trava-presa-1"
TOKEN = "t"
GRAVACOES = int(os.environ.get("GRAVACOES", "4000"))
LEITURAS = int(os.environ.get("LEITURAS", "400"))


class Servidor:
    """Sobe um phxsqld com a durabilidade pedida. Morre pelo PID, e so ele."""

    def __init__(self, durabilidade, porta, base):
        self.base = Path(base)
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        self.porta = porta
        cfg = {
            "base": "dados",
            "bind": f"127.0.0.1:{porta}",
            "token": TOKEN,
            "web": {"ligado": False},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": self.hash_da_senha(SENHA)},
            "usuarios": [],
            "recursos": {"durabilidade": durabilidade},
        }
        (self.base / "config.json").write_text(json.dumps(cfg, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
        )
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", porta), timeout=2).close()
                return
            except OSError:
                pass
        raise SystemExit("o phxsqld nao subiu; veja " + str(self.base / "servidor.log"))

    @staticmethod
    def hash_da_senha(senha):
        r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                           capture_output=True, check=True)
        return r.stdout.decode().split('"')[3]

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Cliente:
    def __init__(self, porta):
        self.s = socket.create_connection(("127.0.0.1", porta), timeout=120)
        self.f = self.s.makefile("rwb")

    def call(self, d, exigir=True):
        d.setdefault("token", TOKEN)
        self.s.sendall((json.dumps(d) + "\n").encode())
        r = json.loads(self.f.readline())
        if exigir and not r.get("ok"):
            raise SystemExit(f"{d['op']} recusado: " + json.dumps(r)[:400])
        return r


def acha(no, chave):
    """`trava_ms` esta aninhado no retrato; acha-lo por nome evita depender do
    formato exato, que e de outra frente e muda sem avisar esta."""
    if isinstance(no, dict):
        if chave in no:
            return no[chave]
        for v in no.values():
            achado = acha(v, chave)
            if achado is not None:
                return achado
    elif isinstance(no, list):
        for v in no:
            achado = acha(v, chave)
            if achado is not None:
                return achado
    return None


def uma_bateria(durabilidade, porta, base, vigia):
    """Grava N linhas e le M vezes, e devolve o us de trava por operacao."""
    srv = Servidor(durabilidade, porta, base)
    try:
        c = Cliente(porta)
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        c.call({"op": "criar_database", "database": "t"})
        c.call({"op": "criar_tabela", "database": "t", "tabela": "c",
                "colunas": [{"nome": "id", "tipo": "Sequence", "obrigatoria": True},
                            {"nome": "nome", "tipo": "Str(20)"}],
                "indices": [{"nome": "porId", "colunas": ["id"],
                             "unico": True, "primario": True}]})
        # A telemetria LIGA depois da criacao: o `criar_tabela` toma a trava e
        # sincroniza, e conta-lo junto poria o custo de UMA criacao em cima da
        # media de quatro mil gravacoes.
        c.call({"op": "telemetria_ligar"})

        t0 = time.monotonic()
        for i in range(GRAVACOES):
            c.call({"op": "inserir", "database": "t", "tabela": "c",
                    "linha": {"nome": f"n{i}"}})
        gravou_s = time.monotonic() - t0
        depois_de_gravar = int(acha(c.call({"op": "telemetria"}), "trava_ms") or 0)

        t0 = time.monotonic()
        for _ in range(LEITURAS):
            c.call({"op": "varrer", "database": "t", "tabela": "c", "max": 50})
        leu_s = time.monotonic() - t0
        no_fim = int(acha(c.call({"op": "telemetria"}), "trava_ms") or 0)

        a = vigia.durante_a_rodada(meus=2)
        return {
            "durabilidade": durabilidade,
            "trava_gravando_us": depois_de_gravar * 1000.0 / GRAVACOES,
            "trava_lendo_us": (no_fim - depois_de_gravar) * 1000.0 / LEITURAS,
            "pedido_gravando_us": gravou_s * 1e6 / GRAVACOES,
            "pedido_lendo_us": leu_s * 1e6 / LEITURAS,
            "cpu": a.ocupada,
        }
    finally:
        srv.parar()


def principal():
    if not PHXSQLD.exists():
        print(f"falta {PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    vigia = quieta.Vigia().abrir()
    porta = quieta.porta_livre()
    base = f"/tmp/phx-trava-presa-{os.getpid()}"
    print("=== quanto a trava fica presa, e quanto disso e o fsync ===")
    print(f"    {GRAVACOES} gravacoes e {LEITURAS} leituras por bateria, "
          f"porta {porta}\n")
    linhas = []
    for durab in ("por_lote", "por_operacao"):
        linhas.append(uma_bateria(durab, porta, base, vigia))
    shutil.rmtree(base, ignore_errors=True)
    vigia.fechar()
    vigia.relatar()

    sujo_vale = "--mesmo-sujo" in sys.argv
    if not vigia.publicavel() and not sujo_vale:
        print("Nenhum numero sai desta rodada. Rode com a maquina parada,")
        print("ou use --mesmo-sujo para depurar o proprio arnes.")
        return 1
    if not vigia.publicavel():
        print(">>> NUMEROS SUJOS: a maquina nao estava parada. NAO CITAR. <<<\n")

    print("-- tempo de TRAVA PRESA por operacao (o que a proxima conexao espera)")
    for d in linhas:
        print(f"   {d['durabilidade']:>13}: gravando {d['trava_gravando_us']:9.1f} us   "
              f"lendo {d['trava_lendo_us']:8.1f} us   cpu {d['cpu']:3.0f}%")
    print("\n-- o pedido INTEIRO, para ter com o que comparar")
    for d in linhas:
        print(f"   {d['durabilidade']:>13}: gravando {d['pedido_gravando_us']:9.1f} us   "
              f"lendo {d['pedido_lendo_us']:8.1f} us")

    lote, oper = linhas[0], linhas[1]
    fsync_us = oper["trava_gravando_us"] - lote["trava_gravando_us"]
    print("\n-- o que o par diz")
    print(f"   o `fsync` sob a trava custa {fsync_us:.1f} us por gravacao")
    if lote["trava_gravando_us"] > 0:
        print(f"   ou seja, {oper['trava_gravando_us'] / lote['trava_gravando_us']:.2f}x "
              f"o tempo de trava de uma gravacao sem ele")
    if lote["trava_lendo_us"] > 0:
        deriva = oper["trava_lendo_us"] / lote["trava_lendo_us"]
        print(f"   CONTROLE DE CIMA -- a leitura, que toma a mesma trava e nao")
        print(f"   sincroniza nada, andou {deriva:.2f}x entre as duas baterias.")
        if deriva > 1.25 or deriva < 0.8:
            print("   >>> ela devia ficar parada. Andou: parte da diferenca")
            print("   >>> acima e da MAQUINA, e nao do fsync. <<<")
    if "--json" in sys.argv:
        print(json.dumps(linhas, indent=2))
    return 0 if vigia.publicavel() else 1


if __name__ == "__main__":
    raise SystemExit(principal())
