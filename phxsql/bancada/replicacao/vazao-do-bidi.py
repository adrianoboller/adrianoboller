#!/usr/bin/env python3
"""A vazao do bidirecional, em eventos/s, com faixa min-max -- pedido 722.

    python3 bancada/replicacao/vazao-do-bidi.py --binario A [--binario B ...]

# A pergunta

A E7 do desenho unico poe uma pre-conferencia do grupo inteiro (o ensaio a
seco do `aplicar_por_chave`) antes do primeiro evento de cada grupo do
bidirecional, sob a mesma trava. Quanto ela custa na vazao de quem puxa? A
regra do parecer do papel C e medir ANTES de aceitar.

# O trabalho igual

Para cada binario: um caixa (`multi`) com N vendas ja gravadas -- cada uma um
COMMIT de 1 venda, 5 itens e 1 pagamento, 7 eventos -- e um central novo
(`multi`, puxando do caixa) que sobe e alcanca tudo. Mede-se do instante em
que a porta do central abre ate as tres tabelas mostrarem tudo, e a vazao e
`7 N / tempo`. O caixa e o mesmo binario do central, recriado a cada corrida:
as duas pontas mudam juntas, e nenhuma corrida herda cache da outra.

As corridas se intercalam entre os binarios, e cada numero sai com a faixa
min-max; o vencedor so se declara quando as faixas nao se cruzam.

Mata so os PIDs que ele mesmo subiu.
"""
import argparse
import json
import os
import re
import shutil
import socket
import statistics
import subprocess
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
TOKEN = "vazao-do-bidi"
ITENS = 5


class No:
    def __init__(self, binario, rotulo, papel_id, origem=None):
        self.dir = tempfile.mkdtemp(prefix=f"phx-vazao-bidi-{rotulo}-")
        os.chmod(self.dir, 0o700)
        # O `multi` exige uma origem. A do caixa aponta para uma porta
        # fechada (a 9, «discard»): ele so serve, e quem puxa e o central.
        rep = {"papel": "multi", "id_servidor": papel_id, "imagem_da_linha": True,
               "origens": [{"nome": "caixa01" if origem else "ninguem",
                            "host": "127.0.0.1", "porta": origem or 9, "token": TOKEN,
                            "databases": ["loja"], "reconectar_em": 1 if origem else 3600}]}
        cfg = {"bind": "127.0.0.1:0", "token": TOKEN, "base": os.path.join(self.dir, "dados"),
               "cifra_fio": {"exigir": False, "arquivo": os.path.join(self.dir, "chave.hex")},
               "web": {"ligado": False}, "replicacao": rep}
        caminho = os.path.join(self.dir, "config.json")
        with open(caminho, "w") as f:
            json.dump(cfg, f)
        self.erro = open(os.path.join(self.dir, "stderr.txt"), "w+")
        self.proc = subprocess.Popen([binario, "--config", caminho], cwd=self.dir,
                                     stdout=subprocess.DEVNULL, stderr=self.erro)
        ate = time.time() + 30
        self.porta = None
        while self.porta is None:
            if time.time() > ate or self.proc.poll() is not None:
                raise SystemExit(f"{binario} nao subiu")
            with open(self.erro.name) as f:
                m = re.search(r"porta de dados escutando em 127\.0\.0\.1:(\d+)", f.read())
            if m:
                self.porta = int(m.group(1))
            else:
                time.sleep(0.01)
        self.sock = socket.create_connection(("127.0.0.1", self.porta))
        self.buf = b""

    def pedir(self, pedido, exigir=True):
        self.sock.sendall(json.dumps(dict(pedido, token=TOKEN)).encode() + b"\n")
        while b"\n" not in self.buf:
            pedaco = self.sock.recv(1 << 20)
            if not pedaco:
                raise SystemExit("o servidor fechou a conexao")
            self.buf += pedaco
        linha, self.buf = self.buf.split(b"\n", 1)
        r = json.loads(linha)
        if exigir and not r.get("ok"):
            raise SystemExit(f"{pedido.get('op')} recusado: {r}")
        return r

    def registros(self):
        """`registros` de cada tabela, pelo `sistabelas` -- um pedido so, e sem o
        teto de linhas do `varrer`."""
        r = self.pedir({"op": "sistabelas", "database": "loja"}, exigir=False)
        achadas = {}
        def andar(x):
            if isinstance(x, dict):
                if "tabela" in x and "registros" in x:
                    achadas[x["tabela"]] = x["registros"]
                for v in x.values():
                    andar(v)
            elif isinstance(x, list):
                for v in x:
                    andar(v)
        andar(r.get("resultado"))
        return tuple(achadas.get(t, 0) for t in ("vendas", "itens", "pagamentos"))

    def fechar(self):
        try:
            self.sock.close()
        finally:
            self.proc.kill()
            self.proc.wait()
            self.erro.close()
            shutil.rmtree(self.dir, ignore_errors=True)


def uma_corrida(binario, vendas):
    caixa = No(binario, "caixa", "caixa01")
    central = None
    try:
        caixa.pedir({"op": "criar_database", "database": "loja"})
        for t in ("vendas", "itens", "pagamentos"):
            caixa.pedir({"op": "criar_tabela", "database": "loja", "tabela": t,
                         "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                                     {"nome": "venda", "tipo": "Int8"}],
                         "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                                      "primario": True}]})
        for n in range(1, vendas + 1):
            caixa.pedir({"op": "begin", "database": "loja"})
            caixa.pedir({"op": "inserir", "database": "loja", "tabela": "vendas",
                         "linha": {"id": n, "venda": n}})
            for i in range(ITENS):
                caixa.pedir({"op": "inserir", "database": "loja", "tabela": "itens",
                             "linha": {"id": (n - 1) * ITENS + i + 1, "venda": n}})
            caixa.pedir({"op": "inserir", "database": "loja", "tabela": "pagamentos",
                         "linha": {"id": n, "venda": n}})
            caixa.pedir({"op": "commit"})
        inicio = time.perf_counter()
        central = No(binario, "central", "central", origem=caixa.porta)
        alvo = (vendas, vendas * ITENS, vendas)
        ate = time.time() + 600
        while True:
            visto = central.registros()
            if visto == alvo:
                return 7 * vendas / (time.perf_counter() - inicio)
            if time.time() > ate:
                raise SystemExit(f"o central nao alcancou: {visto} de {alvo}")
            time.sleep(0.02)
    finally:
        if central:
            central.fechar()
        caixa.fechar()


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--binario", action="append", required=True,
                   help="rotulo=caminho; repetido para comparar")
    a.add_argument("--vendas", type=int, default=1000)
    a.add_argument("--corridas", type=int, default=8)
    args = a.parse_args()
    binarios = [b.split("=", 1) for b in args.binario]
    vazoes = {r: [] for r, _ in binarios}
    for k in range(args.corridas):
        ordem = binarios if k % 2 == 0 else list(reversed(binarios))
        for rotulo, caminho in ordem:
            vazoes[rotulo].append(uma_corrida(caminho, args.vendas))
    saida = {"medido_em": time.strftime("%Y-%m-%d %H:%M"),
             "carga": open("/proc/loadavg").read().split()[:3],
             "pergunta": "eventos/s que o central bidirecional aplica ao alcancar N vendas "
                         "de 7 eventos, da porta aberta ate tudo visivel",
             "vendas": args.vendas, "resultados": {}}
    for rotulo, v in vazoes.items():
        saida["resultados"][rotulo] = {"mediana": round(statistics.median(v)),
                                       "min": round(min(v)), "max": round(max(v)),
                                       "corridas": len(v)}
        print(f"{rotulo:>10}: {statistics.median(v):8.0f} eventos/s "
              f"({min(v):.0f}-{max(v):.0f}, {len(v)} corridas)")
    with open(os.path.join(AQUI, "vazao-do-bidi.json"), "w") as f:
        json.dump(saida, f, indent=1, ensure_ascii=False)


if __name__ == "__main__":
    main()
