#!/usr/bin/env python3
"""Premissa P-A: quantas das 64 vagas HTTP a INTERFACE consome em repouso?

    python3 bancada/vagas-da-tela/medir.py              # sobe o phxsqld, abre a tela, mede
    python3 bancada/vagas-da-tela/medir.py --sem-tela   # controle: mede sem abrir a tela

A pergunta, e por que ela decide uma sprint
-------------------------------------------
O pedido 333 diz que um long-poll de 50 s ocupa uma das `conexoes_web_max = 64`
vagas por 50 s, e conclui que o chat sob long-poll tem teto de 64 usuarios --
menos o que a propria tela ja consome. **Esse "menos" nunca foi medido**, e o
integrador declarou a aritmetica como «raciocinada, nao medida» no proprio
pedido. Se a tela ja come oito vagas em repouso, o teto nao e 64: e 56.

Sem este numero, o teto do chat e um palpite dentro de um pedido publicado.

O que ela mede, e de onde o numero sai
--------------------------------------
Do proprio servidor, nao de fora: a op `telemetria` devolve o campo `tetos`,
montado por `Servidor::tetos_das_threads` (`crates/phxsql-server/src/servidor.rs`),
com `familia`/`vivo`/`teto` para `dados` e `http`. `vivo` e a ocupacao
instantanea do semaforo. O teste `os_tetos_das_threads_saem_com_a_ocupacao_viva`
trava esse contrato no fonte.

O CONTROLE POSITIVO vem antes do veredito, porque zero sem controle nao vale:
a corrida `--sem-tela` mede com o servidor de pe e NENHUM navegador. Se as duas
corridas derem o mesmo numero, a medicao nao esta vendo a tela -- e o resultado
e «instrumento cego», nao «a tela custa zero».

Como roda
---------
Sobe um `phxsqld` proprio em porta alta e diretorio temporario (nunca encosta
em banco de ninguem), abre a tela pelo Chromium do `playwright`, espera os
`setInterval` da propria pagina baterem algumas vezes, e ai pergunta a
`telemetria` por uma conexao SEPARADA -- a pergunta tambem ocupa uma vaga, e
esse 1 e descontado e dito no relatorio, nunca escondido.
"""
import json, os, socket, subprocess, sys, tempfile, time, pathlib

RAIZ = pathlib.Path(__file__).resolve().parents[2]
BIN = os.environ.get("PHX_PHXSQLD", str(RAIZ / "target/release/phxsqld"))
PORTA = int(os.environ.get("PORTA", "57311"))
PORTA_WEB = int(os.environ.get("PORTA_WEB", "57312"))
BATIDAS = int(os.environ.get("BATIDAS", "6"))   # segundos de tela aberta
TOKEN = "vagas-da-tela"
SENHA = "medida-p-a"


def _uma_linha(s):
    buf = b""
    while not buf.endswith(b"\n"):
        p = s.recv(65536)
        if not p:
            break
        buf += p
    return json.loads(buf.decode())


def falar(porta, pedido):
    """Login e pergunta na MESMA conexao, pelo protocolo JSON, uma por linha.

    O token de servico sozinho NAO abre a `telemetria`: o servidor recusa com
    `[SP000025] acesso negado: faca login antes` -- e a mensagem dele diz o
    remedio por extenso, que e o que uma mensagem de erro tem de fazer. A
    sessao vive na conexao, entao as duas linhas vao pelo mesmo soquete.
    """
    with socket.create_connection(("127.0.0.1", porta), timeout=10) as s:
        s.sendall((json.dumps({"op": "login", "usuario": "root",
                               "senha": SENHA, "token": TOKEN}) + "\n").encode())
        r = _uma_linha(s)
        if not r.get("ok", True):
            raise SystemExit(f"REPROVA: login recusado: {r.get('erro')}")
        s.sendall((json.dumps(dict(pedido, token=TOKEN)) + "\n").encode())
        return _uma_linha(s)


def porta_livre(inicio):
    for p in range(inicio, inicio + 400):
        try:
            s = socket.socket()
            s.bind(("127.0.0.1", p))
            s.close()
            return p
        except OSError:
            pass
    sys.exit("REPROVA: nao achei porta livre")


class Servidor:
    """Um `phxsqld` proprio, com a web ligada, morto pelo PID.

    Nunca por `pkill`: matar por padrao derruba o servidor de outra frente na
    mesma maquina -- licao ja paga nesta casa, e escrita na `enxurrada-web.py`.
    """

    def __init__(self, porta_dados, porta_web):
        self.porta_dados, self.porta_web = porta_dados, porta_web
        self.base = pathlib.Path(f"/tmp/phx-vagas-{os.getpid()}-{porta_web}")
        import shutil
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        cfg = {
            "base": "dados",
            # bancada de teste, NAO cliente do produto -- fala em claro para medir "Premissa P-A: quantas das 64 vagas HTTP a INTERFACE consome em repouso?" sem o aperto de mao no meio (servidor exige a cifra por padrao desde o pedido 370)
            "bind": f"127.0.0.1:{porta_dados}", "cifra_fio": {"exigir": False},
            "token": TOKEN,
            "timeout_s": 30,
            "web": {"ligado": True, "bind": f"127.0.0.1:{porta_web}"},
            "root": {"id": 1, "nome": "root", "login": "root",
                     "senha_hash": self.hash_da_senha()},
            "usuarios": [],
        }
        (self.base / "config.json").write_text(json.dumps(cfg, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [BIN], cwd=self.base, stdout=self.log,
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", porta_web), timeout=2).close()
                return
            except OSError:
                pass
        sys.exit(f"REPROVA: o phxsqld nao subiu; veja {self.base}/servidor.log")

    @staticmethod
    def hash_da_senha():
        r = subprocess.run([BIN, "--senha"], input=SENHA.encode(),
                           capture_output=True, check=True)
        return r.stdout.decode().split('"')[3]

    def threads(self):
        try:
            with open(f"/proc/{self.proc.pid}/status") as f:
                for l in f:
                    if l.startswith("Threads:"):
                        return int(l.split()[1])
        except OSError:
            pass
        return 0

    def matar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
        self.log.close()


def vagas_http(porta_dados):
    """`vivo` e `teto` da familia http, pela op `telemetria`.

    A pergunta viaja por uma conexao PROPRIA na porta de DADOS, nao na web --
    por isso ela nao entra na conta das vagas http. Se um dia entrar, o numero
    tem de dizer que entrou.
    """
    r = falar(porta_dados, {"op": "telemetria"})
    if os.environ.get("CRU"):
        print("CRU:", json.dumps(r)[:700])
    # O envelope e `{"ok":…, "op":…, "resultado":{…}}`. Procurar em "resposta"
    # devolvia None sem erro nenhum -- e foi o CONTROLE POSITIVO que pegou:
    # ele recusa quando nao acha a familia, em vez de relatar zero.
    tetos = (r.get("resultado") or r).get("tetos") or []
    for fam in tetos:
        if fam.get("familia") == "http":
            # `em_uso` e a ocupacao; `vivo` e so um booleano que diz se a
            # familia tem semaforo de verdade (o `fixo` do fonte poe false).
            # Confundir os dois foi premissa minha, e o controle a pegou.
            return fam.get("em_uso"), fam.get("teto")
    return None, None


def main():
    sem_tela = "--sem-tela" in sys.argv
    if not os.path.exists(BIN):
        sys.exit(f"REPROVA: nao achei {BIN}. Compile: "
                 f"flock /tmp/phx-cargo.lock cargo build --release --bin phxsqld")
    print(f"binario: {BIN}")
    print(f"  mtime: {time.strftime('%d/%m/%Y %H:%M', time.localtime(os.path.getmtime(BIN)))}")

    pd = porta_livre(PORTA)
    pw = porta_livre(pd + 1)
    s = Servidor(pd, pw)
    try:
        # 1) CONTROLE: servidor de pe, nenhum navegador.
        time.sleep(1.0)
        vivo0, teto = vagas_http(pd)
        thr0 = s.threads()
        print(f"\ncontrole (sem tela): vivo={vivo0} de teto={teto}, threads={thr0}")
        if teto is None:
            sys.exit("REPROVA: a op telemetria nao devolveu a familia http")

        if sem_tela:
            print("\n--sem-tela pedido: so o controle. NAO MEDIDO o custo da tela.")
            return 0

        # 2) A TELA aberta, e o tempo de os setInterval dela baterem.
        # O navegador entra pelo Node, nao pelo Python: o `playwright` desta
        # casa e o do Node e se resolve por caminho absoluto (ver `olhar.mjs`).
        vivos = []
        mjs = str(pathlib.Path(__file__).with_name("abrir-e-segurar.mjs"))
        nav = subprocess.Popen(["node", mjs, f"http://127.0.0.1:{pw}/"],
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True)
        try:
            # Espera o PRONTO: amostrar a pagina ainda subindo mediria menos
            # do que a verdade.
            linha = nav.stdout.readline().strip()
            if linha != "PRONTO":
                erro = nav.stderr.read()[:400]
                sys.exit(f"REPROVA: o navegador nao abriu ({linha!r}). "
                         f"NAO MEDIDO o custo da tela.\n{erro}")
            for _ in range(BATIDAS):
                time.sleep(1.0)
                v, _ = vagas_http(pd)
                vivos.append(v)
            thr1 = s.threads()
        finally:
            nav.terminate()
            try:
                nav.wait(timeout=15)
            except subprocess.TimeoutExpired:
                nav.kill()

        print(f"com a tela aberta, {BATIDAS} amostras de 1 s: {vivos}")
        print(f"  threads do processo: {thr0} -> {thr1}")
        pico = max(v for v in vivos if v is not None)
        mediana = sorted(v for v in vivos if v is not None)[len(vivos) // 2]
        print(f"\nP-A MEDIDO: a tela em repouso ocupa pico={pico} e mediana={mediana} "
              f"das {teto} vagas http")
        if pico == vivo0:
            print("  ATENCAO: igual ao controle. Ou a tela custa zero, ou o "
                  "instrumento esta cego -- nao conclua sem olhar o servidor.log")
        print(f"  teto util para o chat sob long-poll: {teto} - {pico} = {teto - pico}")
        return 0
    finally:
        s.matar()


if __name__ == "__main__":
    sys.exit(main())
