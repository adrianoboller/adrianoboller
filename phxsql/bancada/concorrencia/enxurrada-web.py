#!/usr/bin/env python3
"""A enxurrada na porta web: N conexoes HTTP ao mesmo tempo contra o `phxsqld`.

    python3 bancada/concorrencia/enxurrada-web.py                    # o binario de hoje, teto de fabrica
    python3 bancada/concorrencia/enxurrada-web.py --sem-teto         # recursos.conexoes_web_max = 0
    PHX_PHXSQLD=/outro/phxsqld python3 bancada/concorrencia/enxurrada-web.py --rotulo antes
    CONEXOES=500 SEGURAR=3 python3 bancada/concorrencia/enxurrada-web.py

A pergunta
----------
Pedido 248. Ate 16/09/2026 a porta web nascia SEM TETO -- o proprio comentario
do laco confessava: «uma enxurrada de pedidos vira uma enxurrada de threads».
O teto entrou (`recursos.conexoes_web_max`, fila `recursos.fila_web_ms`, 503
com `Retry-After`), e esta bancada mede o que ele segura: **threads vivas**
(`Threads:` do `/proc/<pid>/status`), **RSS** e **quantas conexoes receberam
503** -- antes e depois, no mesmo arnes.

As duas ondas, e por que sao duas
---------------------------------
1. **Segurar.** N clientes conectam ao mesmo tempo e mandam o cabecalho SEM a
   linha vazia final: a thread de cada um fica esperando o resto. E o pior
   caso para um servidor de uma-thread-por-pedido, e e o caso que o teto
   existe para segurar. Depois de SEGURAR segundos cada cliente completa o
   pedido e le a resposta. Sem teto: N threads. Com teto: `conexoes_web_max`
   threads, e os outros recebem 503 -- o primeiro depois de `fila_web_ms`, os
   seguintes na hora, pela fila declarada cheia.
2. **Rapida.** Os mesmos N clientes, pedidos completos de uma vez. E o
   comportamento VELHO: abaixo do teto nada muda, e a onda tem de sair com
   zero 503 nos dois arranjos. Sem esta onda a bancada mediria so o ganho e
   nunca o preco.

O que NAO se mede aqui
----------------------
Vazao. Uma enxurrada e um pico, e o numero que interessa num pico e quantas
threads o servidor chegou a ter e quantos clientes ouviram um «volte depois»
em vez de um `connect` que nao responde. O `quieta.Vigia` continua valendo:
maquina ocupada muda o `Threads:` de pico (quem esta lento acumula mais).

O resultado vai para `bancada/concorrencia/resultados.json`, um bloco por
rotulo (`antes`, `depois`, `sem-teto`), cada um com a data -- resultados de
dias diferentes juntos sem a data seriam um retrato que nunca existiu.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quieta  # noqa: E402

RAIZ = Path(__file__).resolve().parents[2]
PHXSQLD = Path(os.environ.get("PHX_PHXSQLD", RAIZ / "target/release/phxsqld"))
TOKEN = "enxurrada"
SENHA = "enxurrada-8765"
CONEXOES = int(os.environ.get("CONEXOES", "500"))
SEGURAR = float(os.environ.get("SEGURAR", "3"))
TETO = int(os.environ.get("TETO", "64"))  # nao-e-catraca: parametro da bancada, ajustavel pelo ambiente
FILA_MS = int(os.environ.get("FILA_MS", "2000"))
RESULTADOS = RAIZ / "bancada/concorrencia/resultados.json"


def duas_portas_livres():
    """Duas portas da faixa da frente, DIFERENTES: `porta_livre` solta o
    soquete ao devolver, entao chama-la duas vezes daria a mesma porta."""
    primeira = quieta.porta_livre()
    guarda = socket.socket()
    guarda.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    guarda.bind(("127.0.0.1", primeira))
    try:
        segunda = quieta.porta_livre((primeira + 1, quieta.FAIXA[1]))
    finally:
        guarda.close()
    return primeira, segunda


class Servidor:
    """Um `phxsqld` proprio com a web ligada, morto pelo PID -- nunca por
    `pkill`, que derrubaria o servidor de outra frente na mesma maquina."""

    def __init__(self, porta_dados, porta_web, teto, fila_ms):
        self.porta_web = porta_web
        self.base = Path(f"/tmp/phx-enxurrada-{os.getpid()}-{porta_web}")
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        cfg = {
            "base": "dados",
            "bind": f"127.0.0.1:{porta_dados}",
            "token": TOKEN,
            # Curto: um cliente que segura o cabecalho por SEGURAR s tem de
            # ser servido quando completa, e nao cortado pelo servidor antes.
            "timeout_s": int(SEGURAR) + 10,
            "web": {"ligado": True, "bind": f"127.0.0.1:{porta_web}"},
            "recursos": {"conexoes_web_max": teto, "fila_web_ms": fila_ms},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": self.hash_da_senha(SENHA)},
            "usuarios": [],
        }
        (self.base / "config.json").write_text(json.dumps(cfg, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", porta_web), timeout=2).close()
                return
            except OSError:
                pass
        raise SystemExit(f"o phxsqld nao subiu; veja {self.base}/servidor.log")

    @staticmethod
    def hash_da_senha(senha):
        r = subprocess.run([str(PHXSQLD), "--senha"], input=senha.encode(),
                           capture_output=True, check=True)
        return r.stdout.decode().split('"')[3]

    def estado(self):
        """(threads vivas, RSS em KiB) do `/proc/<pid>/status`."""
        threads = rss = 0
        try:
            with open(f"/proc/{self.proc.pid}/status") as f:
                for linha in f:
                    if linha.startswith("Threads:"):
                        threads = int(linha.split()[1])
                    elif linha.startswith("VmRSS:"):
                        rss = int(linha.split()[1])
        except OSError:
            pass
        return threads, rss

    def linhas_do_log_de_acessos(self, agulha):
        p = self.base / "dados" / "acessos.log"
        if not p.exists():
            p = self.base / "acessos.log"
        if not p.exists():
            return 0
        return sum(1 for l in p.read_text(errors="replace").splitlines() if agulha in l)

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()
        shutil.rmtree(self.base, ignore_errors=True)


class Amostrador(threading.Thread):
    """Le `Threads:` e `VmRSS:` do servidor a cada 25 ms e guarda o PICO."""

    def __init__(self, srv):
        super().__init__(daemon=True)
        self.srv = srv
        self.pico_threads = 0
        self.pico_rss = 0
        self.parar = threading.Event()

    def run(self):
        while not self.parar.is_set():
            t, r = self.srv.estado()
            self.pico_threads = max(self.pico_threads, t)
            self.pico_rss = max(self.pico_rss, r)
            time.sleep(0.025)


def status_da_resposta(dados):
    """`200`, `503`, ... da primeira linha; `vazio` quando o servidor fechou
    sem escrever (o RST que descarta a resposta em voo, ou o corte)."""
    if not dados:
        return "vazio"
    primeira = dados.split(b"\r\n", 1)[0]
    partes = primeira.split()
    return partes[1].decode(errors="replace") if len(partes) >= 2 else "torto"


def cliente_que_segura(porta, largada, segurar, saida, i):
    """Conecta, manda o cabecalho SEM a linha vazia, espera; completa e le."""
    r = {"i": i, "status": "erro", "primeiro_byte_ms": None}
    try:
        s = socket.create_connection(("127.0.0.1", porta), timeout=segurar + 15)
        largada.wait()
        t0 = time.perf_counter()
        s.sendall(b"GET /saude HTTP/1.1\r\nHost: x\r\n")
        # Enquanto segura, escuta: com o teto, o 503 chega ANTES de o pedido
        # estar completo -- e o cliente tem de estar ouvindo para ve-lo.
        s.settimeout(segurar)
        dados = b""
        try:
            pedaco = s.recv(65536)
            if pedaco:
                dados = pedaco
                r["primeiro_byte_ms"] = (time.perf_counter() - t0) * 1000
                s.settimeout(2)
                while True:
                    pedaco = s.recv(65536)
                    if not pedaco:
                        break
                    dados += pedaco
        except socket.timeout:
            pass
        if not dados:
            # ninguem respondeu enquanto seguravamos: completa o pedido
            s.settimeout(segurar + 15)
            s.sendall(b"\r\n")
            while True:
                try:
                    pedaco = s.recv(65536)
                except socket.timeout:
                    break
                if not pedaco:
                    break
                if r["primeiro_byte_ms"] is None:
                    r["primeiro_byte_ms"] = (time.perf_counter() - t0) * 1000
                dados += pedaco
        s.close()
        r["status"] = status_da_resposta(dados)
        r["retry_after"] = b"Retry-After:" in dados
    except ConnectionResetError:
        r["status"] = "reset"
    except OSError as e:
        r["status"] = f"erro:{e.__class__.__name__}"
    saida[i] = r


def cliente_rapido(porta, largada, saida, i):
    r = {"i": i, "status": "erro", "primeiro_byte_ms": None}
    try:
        s = socket.create_connection(("127.0.0.1", porta), timeout=15)
        largada.wait()
        t0 = time.perf_counter()
        s.sendall(b"GET /saude HTTP/1.1\r\nHost: x\r\n\r\n")
        s.settimeout(15)
        dados = b""
        while True:
            try:
                pedaco = s.recv(65536)
            except socket.timeout:
                break
            if not pedaco:
                break
            if r["primeiro_byte_ms"] is None:
                r["primeiro_byte_ms"] = (time.perf_counter() - t0) * 1000
            dados += pedaco
        s.close()
        r["status"] = status_da_resposta(dados)
        r["retry_after"] = b"Retry-After:" in dados
    except ConnectionResetError:
        r["status"] = "reset"
    except OSError as e:
        r["status"] = f"erro:{e.__class__.__name__}"
    saida[i] = r


def onda(srv, n, alvo, vigia, **kw):
    """N clientes, todos conectados ANTES da largada, soltos de uma vez."""
    largada = threading.Event()
    saida = [None] * n
    fios = [threading.Thread(target=alvo, args=(srv.porta_web, largada) + kw.get("args", ()) + (saida, i),
                             daemon=True) for i in range(n)]
    amostrador = Amostrador(srv)
    amostrador.start()
    t0 = time.perf_counter()
    for f in fios:
        f.start()
    # Espera todo mundo conectar (a fila do `listen` segura quem o `accept`
    # ainda nao pegou), e solta.
    time.sleep(1.0)
    largada.set()
    vigia.durante_a_rodada(meus=2)
    for f in fios:
        f.join()
    duracao = time.perf_counter() - t0
    amostrador.parar.set()
    amostrador.join()
    contagem = {}
    ttfb = []
    for r in saida:
        contagem[r["status"]] = contagem.get(r["status"], 0) + 1
        if r["primeiro_byte_ms"] is not None:
            ttfb.append(r["primeiro_byte_ms"])
    ttfb.sort()
    q = lambda x: ttfb[min(len(ttfb) - 1, int(x * len(ttfb)))] if ttfb else None
    return {
        "conexoes": n,
        "respostas": contagem,
        "com_retry_after": sum(1 for r in saida if r.get("retry_after")),
        "pico_threads": amostrador.pico_threads,
        "pico_rss_kib": amostrador.pico_rss,
        "primeiro_byte_ms": {"p50": q(0.5), "p99": q(0.99), "pior": ttfb[-1] if ttfb else None},
        "duracao_s": round(duracao, 2),
    }


def principal():
    if not PHXSQLD.exists():
        print(f"falta {PHXSQLD} -- rode `cargo build --release` antes")
        return 2
    sem_teto = "--sem-teto" in sys.argv
    rotulo = "sem-teto" if sem_teto else "depois"
    if "--rotulo" in sys.argv:
        rotulo = sys.argv[sys.argv.index("--rotulo") + 1]
    sujo_vale = "--mesmo-sujo" in sys.argv
    teto = 0 if sem_teto else TETO

    print("=== a enxurrada na porta web ===")
    print(f"    {quieta.nucleos()} nucleos | {CONEXOES} conexoes | segurar {SEGURAR:.0f}s | "
          f"teto {teto or 'nenhum'} | fila {FILA_MS} ms | rotulo `{rotulo}`")
    print(f"    binario: {PHXSQLD} ({time.strftime('%Y-%m-%d %H:%M', time.localtime(PHXSQLD.stat().st_mtime))})\n")

    vigia = quieta.Vigia().abrir()
    porta_dados, porta_web = duas_portas_livres()
    srv = Servidor(porta_dados, porta_web, teto, FILA_MS)
    try:
        repouso = srv.estado()
        print(f"-- em repouso: {repouso[0]} threads, {repouso[1] / 1024:.1f} MiB")
        segurando = onda(srv, CONEXOES, cliente_que_segura, vigia, args=(SEGURAR,))
        segurando["recusas_no_log"] = srv.linhas_do_log_de_acessos("porta HTTP cheia")
        # deixa o servidor assentar entre as ondas: threads morrendo, fila zerando
        time.sleep(max(2.0, FILA_MS / 1000 + 0.5))
        rapida = onda(srv, CONEXOES, cliente_rapido, vigia)
        final = srv.estado()
    finally:
        srv.parar()
    vigia.fechar()
    vigia.relatar()

    if not vigia.publicavel() and not sujo_vale:
        print("Nenhum numero sai desta rodada. Rode de novo com a maquina")
        print("parada, ou use --mesmo-sujo para depurar o proprio arnes.")
        return 1
    if not vigia.publicavel():
        print(">>> NUMEROS SUJOS: a maquina nao estava parada. NAO CITAR. <<<\n")

    for nome, o in (("SEGURAR -- cabecalho sem a linha final, por "
                     f"{SEGURAR:.0f}s", segurando), ("RAPIDA -- pedidos completos", rapida)):
        print(f"-- onda {nome}")
        print(f"   respostas: {o['respostas']}  (com Retry-After: {o['com_retry_after']})")
        print(f"   pico de threads: {o['pico_threads']}   pico de RSS: {o['pico_rss_kib'] / 1024:.1f} MiB")
        pb = o["primeiro_byte_ms"]
        if pb["p50"] is not None:
            print(f"   primeiro byte: p50 {pb['p50']:.0f} ms  p99 {pb['p99']:.0f} ms  pior {pb['pior']:.0f} ms")
        if "recusas_no_log" in o:
            print(f"   linhas «porta HTTP cheia» no acessos.log: {o['recusas_no_log']}")
        print(f"   duracao: {o['duracao_s']} s\n")
    print(f"-- no fim: {final[0]} threads, {final[1] / 1024:.1f} MiB")

    bloco = {
        "quando": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "binario_de": time.strftime("%Y-%m-%dT%H:%M", time.localtime(PHXSQLD.stat().st_mtime)),
        "sujo": not vigia.publicavel(),
        "conexoes": CONEXOES,
        "segurar_s": SEGURAR,
        "teto": teto,
        "fila_ms": FILA_MS,
        "nucleos": quieta.nucleos(),
        "repouso": {"threads": repouso[0], "rss_kib": repouso[1]},
        "segurando": segurando,
        "rapida": rapida,
        "fim": {"threads": final[0], "rss_kib": final[1]},
    }
    todos = {}
    if RESULTADOS.exists():
        try:
            todos = json.loads(RESULTADOS.read_text())
        except json.JSONDecodeError:
            todos = {}
    todos.setdefault("bancada", "enxurrada-web")
    todos.setdefault("rotulos", {})
    todos["rotulos"][rotulo] = bloco
    RESULTADOS.write_text(json.dumps(todos, indent=2, ensure_ascii=False) + "\n")
    print(f"\n-> {RESULTADOS.relative_to(RAIZ)} [{rotulo}]")
    return 0 if vigia.publicavel() else 1


if __name__ == "__main__":
    raise SystemExit(principal())
