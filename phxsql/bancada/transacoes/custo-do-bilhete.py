#!/usr/bin/env python3
"""O custo do bilhete posicional do COMMIT (pedido 709), medido -- pedido 724.

    python3 bancada/transacoes/custo-do-bilhete.py --antes BIN_ANTES [--depois BIN]

# A pergunta

Desde o 709 a marca do COMMIT leva a versao do slot antes de cada operacao, e
`Servidor::gravar_a_marca_da_lista` abre cada tabela tocada por alteracao,
exclusao ou restauracao UMA vez a mais, sob a trava global, para ler essas
versoes; a passada a abre de novo. A inclusao nao le nada. Quanto isso custa
no COMMIT?

# O trabalho igual (as quatro regras do `bancada/LEIA-ME.md`)

Os dois binarios recebem a MESMA sequencia de pedidos pelo soquete: um COMMIT
com 100 alteracoes repartidas em K tabelas (K = 1, 4, 16), e o controle, um
COMMIT com 100 INCLUSOES nas mesmas K tabelas, com a durabilidade `sistema`
nos dois (sem o `fsync` das tabelas, que cai num COMMIT qualquer e afoga a
leitura do bilhete). Mede-se so o pedido `commit`
(do envio a resposta): e nele, e so nele, que a marca e a passada acontecem.

O controle existe porque o «depois» e a arvore de trabalho inteira, e nao so
o 709: outras frentes mexem nela ao mesmo tempo. A inclusao nao paga leitura
nenhuma do bilhete, entao uma diferenca nela e de OUTRA coisa -- e a da
alteracao so se atribui ao 709 descontada a do controle.

As corridas se intercalam (antes, depois, depois, antes...) para o ruido da
maquina cair nos dois lados, e cada numero sai com a faixa min-max. O vencedor
so se declara quando as faixas nao se cruzam.

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
import sys
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
TOKEN = "custo-do-bilhete"
OPERACOES = 100
TABELAS = (1, 4, 16)


class Servidor:
    def __init__(self, binario, rotulo):
        self.dir = tempfile.mkdtemp(prefix=f"phx-bilhete-{rotulo}-")
        os.chmod(self.dir, 0o700)
        cfg = {
            "bind": "127.0.0.1:0",
            "token": TOKEN,
            "base": os.path.join(self.dir, "dados"),
            "cifra_fio": {"exigir": False, "arquivo": os.path.join(self.dir, "chave.hex")},
            "web": {"ligado": False},
            # Sem o `fsync` das tabelas na janela: ele cai num COMMIT qualquer
            # quando o lote fecha, custa milissegundos, e afogaria a leitura
            # do bilhete, que e de microssegundos. A marca continua com o
            # `fsync` dela, nos dois lados.
            "recursos": {"durabilidade": "sistema"},
        }
        caminho = os.path.join(self.dir, "config.json")
        with open(caminho, "w") as f:
            json.dump(cfg, f)
        self.erro = open(os.path.join(self.dir, "stderr.txt"), "w+")
        self.proc = subprocess.Popen(
            [binario, "--config", caminho],
            cwd=self.dir,
            stdout=subprocess.DEVNULL,
            stderr=self.erro,
        )
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
                time.sleep(0.05)
        self.sock = socket.create_connection(("127.0.0.1", self.porta))
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.buf = b""

    def pedir(self, pedido):
        pedido = dict(pedido, token=TOKEN)
        self.sock.sendall(json.dumps(pedido).encode() + b"\n")
        while b"\n" not in self.buf:
            pedaco = self.sock.recv(65536)
            if not pedaco:
                raise SystemExit("o servidor fechou a conexao")
            self.buf += pedaco
        linha, self.buf = self.buf.split(b"\n", 1)
        r = json.loads(linha)
        if not r.get("ok"):
            raise SystemExit(f"{pedido.get('op')} recusado: {r}")
        return r

    def fechar(self):
        try:
            self.sock.close()
        finally:
            self.proc.kill()
            self.proc.wait()
            self.erro.close()
            shutil.rmtree(self.dir, ignore_errors=True)


def preparar(s):
    """Uma base por K, com as K tabelas e as linhas que a alteracao vai tocar."""
    for k in TABELAS:
        db = f"b{k}"
        s.pedir({"op": "criar_database", "database": db})
        for t in range(k):
            s.pedir({
                "op": "criar_tabela", "database": db, "tabela": f"t{t}",
                "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                            {"nome": "v", "tipo": "Int8"}],
                "indices": [{"nome": "pk", "colunas": ["id"], "unico": True, "primario": True}],
            })
        for i in range(OPERACOES):
            s.pedir({"op": "inserir", "database": db, "tabela": f"t{i % k}",
                     "linha": {"id": i, "v": 0}})


def um_commit(s, k, tipo, rodada, proximo_id):
    db = f"b{k}"
    s.pedir({"op": "begin", "database": db})
    for i in range(OPERACOES):
        tabela = f"t{i % k}"
        if tipo == "alteracao":
            # A linha i e a (i // k + 1)-esima da tabela dela: rowid na ordem
            # de digitacao.
            s.pedir({"op": "atualizar", "database": db, "tabela": tabela,
                     "rowid": i // k + 1, "valores": {"id": i, "v": rodada}})
        else:
            s.pedir({"op": "inserir", "database": db, "tabela": tabela,
                     "linha": {"id": proximo_id + i, "v": rodada}})
    inicio = time.perf_counter()
    s.pedir({"op": "commit"})
    return (time.perf_counter() - inicio) * 1e6


def resumo(v):
    return {"mediana_us": round(statistics.median(v), 1), "min_us": round(min(v), 1),
            "max_us": round(max(v), 1), "corridas": len(v)}


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--antes", required=True)
    a.add_argument("--depois", default=os.path.join(RAIZ, "target", "release", "phxsqld"))
    a.add_argument("--corridas", type=int, default=30)
    a.add_argument("--antes-rotulo", default="")
    args = a.parse_args()
    lados = {"antes": Servidor(args.antes, "antes"), "depois": Servidor(args.depois, "depois")}
    try:
        for s in lados.values():
            preparar(s)
        tempos = {(l, k, t): [] for l in lados for k in TABELAS for t in ("alteracao", "inclusao")}
        proximo = {l: 1_000_000 for l in lados}
        for r in range(args.corridas):
            ordem = ["antes", "depois"] if r % 2 == 0 else ["depois", "antes"]
            for l in ordem:
                for k in TABELAS:
                    for tipo in ("alteracao", "inclusao"):
                        tempos[(l, k, tipo)].append(um_commit(lados[l], k, tipo, r + 1, proximo[l]))
                        if tipo == "inclusao":
                            proximo[l] += OPERACOES
        saida = {
            "medido_em": time.strftime("%Y-%m-%d %H:%M"),
            "pergunta": "us por COMMIT de 100 operacoes em K tabelas, so o pedido commit, "
                        "antes e depois do bilhete posicional do 709",
            "antes": args.antes_rotulo or args.antes,
            "depois": "arvore de trabalho (inclui frentes paralelas; ver o controle de inclusao)",
            "resultados": {},
        }
        print(f"{'K':>3} {'tipo':<10} {'antes (min-max)':>26} {'depois (min-max)':>26}  faixas")
        for k in TABELAS:
            for tipo in ("alteracao", "inclusao"):
                ra, rd = resumo(tempos[("antes", k, tipo)]), resumo(tempos[("depois", k, tipo)])
                cruzam = not (ra["max_us"] < rd["min_us"] or rd["max_us"] < ra["min_us"])
                saida["resultados"][f"{k}/{tipo}"] = {"antes": ra, "depois": rd,
                                                      "faixas_se_cruzam": cruzam}
                fmt = lambda x: f"{x['mediana_us']:>9.1f} ({x['min_us']:.0f}-{x['max_us']:.0f})"
                print(f"{k:>3} {tipo:<10} {fmt(ra):>26} {fmt(rd):>26}  "
                      f"{'se cruzam' if cruzam else 'NAO se cruzam'}")
        with open(os.path.join(AQUI, "resultados-custo-do-bilhete.json"), "w") as f:
            json.dump(saida, f, indent=1, ensure_ascii=False)
    finally:
        for s in lados.values():
            s.fechar()


if __name__ == "__main__":
    sys.exit(main())
