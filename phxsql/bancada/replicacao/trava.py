#!/usr/bin/env python3
"""A trava de dados presa atras de uma leitura de rede -- medida no soquete.

    python3 bancada/replicacao/trava.py            # os quatro estagios
    python3 bancada/replicacao/trava.py congela    # so o corte silencioso
    python3 bancada/replicacao/trava.py alcance    # so a vazao de aplicacao
    python3 bancada/replicacao/trava.py queda      # so a conexao caindo
    python3 bancada/replicacao/trava.py abraco     # so o bidirecional

Esta bancada existe para responder UMA pergunta com numero: *quanto tempo a
trava global de dados fica na mao do laco da replica enquanto ele espera a
rede*. Ela e a versao de loopback dos dois estagios que o conteiner achou
(`bancada/replicacao/docker/provar.py`, estagios `a3-congelamento` e
`b-abraco`), e existe porque a de conteiner leva 13 minutos e pede um daemon
que nem sempre esta no ar -- esta leva menos de dois e roda em qualquer lugar.

# O que o loopback consegue provar, e o que ele NAO consegue

O achado original precisava de um corte SILENCIOSO: pacote que some, e nao
porta que recusa. Em `127.0.0.1` nao ha cabo para cortar e `iptables` nao
alcanca -- e foi por isso que o conteiner existiu. O que substitui o cabo aqui
e um TUBO em Python entre a replica e o source: ele repassa byte a byte ate
alguem mandar PARAR, e a partir dai fica com os dois soquetes abertos sem
repassar nada. Do ponto de vista da replica e exatamente o mesmo silencio: a
conexao esta de pe, o pedido saiu, e a resposta nunca vem.

O que o loopback nao substitui e a QUEDA de processo e a particao real de
rede; para essas continua valendo a bancada de conteiner.

# O contraste e o diagnostico

Nos dois estagios a sonda e um par: `ping`, que nao toca na trava, e `varrer`
(ou `checksum`), que precisa dela. Se os dois travam, o servidor esta fora do
ar e o problema e outro. So o segundo travar e o que aponta a trava.
"""
import json
import os
import socket
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
PHXSQLD = os.path.join(RAIZ, "target", "release", "phxsqld")
BASE = os.environ.get("PHX_TRAVA", "/tmp/phx-trava")

# Faixa desta bancada -- nada aqui usa pkill, os processos sao os do Popen.
P_FONTE, P_TUBO, P_REPLICA = 7050, 7051, 7052
P_ALFA, P_BETA, P_SOZINHO = 7053, 7054, 7055

TOKEN = "trava"
USUARIO = "adm"
SENHA = "segredo1"
TABELA = "clientes"
LINHAS = int(os.environ.get("PHX_LINHAS", "200000"))
SEGUNDOS_DE_SONDA = int(os.environ.get("PHX_SONDA_S", "40"))

PROCESSOS = []
SOQUETES = []
RESULTADO = {}
FALHAS = []


# ------------------------------------------------------------------- montagem

def hash_da_senha(senha):
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def config_base(porta, h):
    return {
        "base": "base",
        "bind": f"127.0.0.1:{porta}",
        "token": TOKEN,
        "web": {"ligado": False},
        # Supervisor porque a sonda le `telemetria`, e o portao dela exige
        # administrador -- ela mostra login, IP e tabela de toda atividade.
        "usuarios": [{"login": USUARIO, "nome": "Bancada", "id": 10,
                      "senha_hash": h, "supervisor": True,
                      "bases": permissoes()}],
    }


def origem_para(porta, nome, h, extras=None):
    o = {"nome": nome, "host": "127.0.0.1", "porta": porta, "token": TOKEN,
         "usuario": USUARIO, "senha_hash": h, "databases": ["loja"],
         "reconectar_em": 1}
    o.update(extras or {})
    return o


def subir(rotulo, cfg):
    d = os.path.join(BASE, rotulo)
    subprocess.run(["rm", "-rf", d], check=False)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=d, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    PROCESSOS.append((p, rotulo))
    return p


def derrubar(*rotulos):
    for s, f in SOQUETES:
        try:
            f.close()
            s.close()
        except OSError:
            pass
    SOQUETES.clear()
    for p, rot in list(PROCESSOS):
        if rotulos and rot not in rotulos:
            continue
        if p.poll() is None:
            p.terminate()
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
        PROCESSOS.remove((p, rot))
    time.sleep(0.4)


class Ligacao:
    """Uma conexao autenticada que se REFAZ -- e as duas razoes disso.

    A primeira e o prazo: toda conexao daqui nasce com um prazo de leitura
    MAIOR que o do defeito que a bancada mede. O cliente que morre de
    `timeout` no meio da medicao devolve "nao respondeu" em vez de "esperou
    30 s", e um teste que falha por engano e tao ruim quanto um que passa por
    engano.

    A segunda e que o servidor FECHA a conexao ociosa em `timeout_s` (30 s):
    o laco de atendimento trata o prazo de leitura como erro e sai. Razoavel
    para um servidor; mortal para uma sonda que fica parada durante os 33 s da
    carga e so entao pergunta. E a mesma armadilha que a bancada de conteiner
    ja pagou, e a saida e a mesma: refazer a conexao e repetir o pedido uma
    vez.
    """

    def __init__(self, porta, tentativas=100, prazo=120):
        self.porta = porta
        self.prazo = prazo
        self.tentativas = tentativas
        self.s = self.f = None
        self._abrir()

    def _abrir(self):
        ultimo = None
        for _ in range(self.tentativas):
            try:
                s = socket.create_connection(("127.0.0.1", self.porta), timeout=self.prazo)
                f = s.makefile("rwb")
                SOQUETES.append((s, f))
                self.s, self.f = s, f
                r = self._cru({"op": "login", "usuario": USUARIO, "senha": SENHA})
                if not r.get("ok"):
                    raise SystemExit(f"login na porta {self.porta}: {r}")
                return
            except OSError as e:
                ultimo = e
                time.sleep(0.2)
        raise SystemExit(f"porta {self.porta} nunca respondeu: {ultimo}")

    def _cru(self, pedido):
        pedido.setdefault("token", TOKEN)
        self.f.write((json.dumps(pedido) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise ConnectionError("o servidor fechou a conexao")
        return json.loads(linha.decode())

    def __call__(self, pedido):
        try:
            return self._cru(dict(pedido))
        except (OSError, ValueError):
            self._abrir()
            return self._cru(dict(pedido))


def liga(porta, tentativas=100, prazo=120):
    return Ligacao(porta, tentativas, prazo)


def criar_tabela(fala):
    fala({"op": "criar_database", "database": "loja"})
    r = fala({"op": "criar_tabela", "database": "loja", "tabela": TABELA,
              "motivo_obrigatorio": False,
              "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                          {"nome": "nome", "tipo": "Str(60)", "obrigatoria": True},
                          {"nome": "cidade", "tipo": "Str(30)"}],
              "indices": [{"nome": "porId", "colunas": ["id"],
                           "unico": True, "primario": True}]})
    if not r.get("ok"):
        raise SystemExit(f"criar tabela: {r}")


def eventos_de(fala):
    r = fala({"op": "posicao", "database": "loja"})
    if not r.get("ok"):
        return None
    t = r["resultado"]["tabelas"].get(TABELA)
    return None if t is None else t["eventos"]


def esperar(condicao, segundos=60, passo=0.2):
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < segundos:
        if condicao():
            return time.perf_counter() - t0
        time.sleep(passo)
    return None


def trava_ms(fala):
    """`totais.trava_ms` -- quanto tempo, somado, alguem segurou a trava.

    Sai do proprio `travar_dados()`, que e o unico lugar que a toma. E a
    testemunha de dentro: nao depende de um cliente de fora reparar na espera.
    """
    r = fala({"op": "telemetria"})
    if not r.get("ok"):
        raise SystemExit(f"telemetria: {r}")
    return r["resultado"]["totais"]["trava_ms"]


def eagain(rotulo):
    """`Resource temporarily unavailable` no diario = o prazo de leitura."""
    caminho = os.path.join(BASE, rotulo, "servidor.log")
    try:
        with open(caminho, "r", errors="replace") as f:
            return f.read().count("Resource temporarily unavailable")
    except OSError:
        return 0


def estagio(nome, esperado):
    print()
    print(f"--- estagio ({nome})")
    print(f"    esperado: {esperado}")


def medir(nome, ok, texto, extra=None):
    print(f"    medido:   {texto}  [{'ok' if ok else 'FALHOU'}]")
    RESULTADO[nome] = {"ok": ok, "medido": texto, **(extra or {})}
    if not ok:
        FALHAS.append(nome)


# ----------------------------------------------------------------------- tubo

class Tubo:
    """Um cano TCP que sabe emudecer -- o corte silencioso do loopback.

    Enquanto `mudo` esta desligado ele repassa byte a byte nos dois sentidos.
    Ligado, ele para de repassar e NAO fecha nada: os dois lados continuam com
    a conexao de pe, o pedido da replica ja saiu, e a resposta nunca chega. E
    o que um `iptables -j DROP` faz num cabo de verdade.
    """

    def __init__(self, porta_entrada, porta_saida):
        self.mudo = threading.Event()
        self.parar = threading.Event()
        self.vivas = []
        self.entrada = socket.socket()
        self.entrada.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.entrada.bind(("127.0.0.1", porta_entrada))
        self.entrada.listen(64)
        self.entrada.settimeout(0.5)
        self.saida = porta_saida
        self.fio = threading.Thread(target=self._aceitar, daemon=True)
        self.fio.start()

    def _aceitar(self):
        while not self.parar.is_set():
            try:
                cliente, _ = self.entrada.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            try:
                servidor = socket.create_connection(("127.0.0.1", self.saida), timeout=5)
            except OSError:
                cliente.close()
                continue
            self.vivas.append((cliente, servidor))
            threading.Thread(target=self._bombear, args=(cliente, servidor),
                             daemon=True).start()
            threading.Thread(target=self._bombear, args=(servidor, cliente),
                             daemon=True).start()

    def _bombear(self, de, para):
        de.settimeout(0.2)
        while not self.parar.is_set():
            try:
                dado = de.recv(65536)
            except socket.timeout:
                continue
            except OSError:
                break
            if not dado:
                break
            # O silencio acontece AQUI: o byte foi lido do cabo e nao segue.
            # Fechar seria um corte barulhento, que o outro lado ve na hora.
            while self.mudo.is_set() and not self.parar.is_set():
                time.sleep(0.05)
            try:
                para.sendall(dado)
            except OSError:
                break
        try:
            de.close()
        except OSError:
            pass
        try:
            para.close()
        except OSError:
            pass

    def cortar(self):
        """Fecha as conexoes vivas -- o corte BARULHENTO, que o outro ve na hora.

        E o corte que interessa para provar a queda ENTRE a leitura do lote e
        a aplicacao dele: o lote ja esta na memoria da replica, a conexao
        morre, e a pergunta e se ele se perde, se dobra, ou se se aplica
        inteiro. Teste unitario nao responde isso -- soquete responde.
        """
        for a, b in self.vivas:
            for lado in (a, b):
                try:
                    lado.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
                try:
                    lado.close()
                except OSError:
                    pass
        n = len(self.vivas)
        self.vivas = []
        return n

    def fechar(self):
        self.parar.set()
        self.mudo.clear()
        try:
            self.entrada.close()
        except OSError:
            pass


# ------------------------------------------------------------------- estagios

def estagio_congela(h):
    estagio("congelamento",
            "com o source ESCREVENDO SEM PARAR (a replica fica dentro do laco "
            "de puxar) e o tubo emudecido: na replica, `ping` continua rapido "
            "e `varrer` -- que precisa da trava -- so responde quando o laco "
            "desistir da leitura")
    subir("fonte", {**config_base(P_FONTE, h), "replicacao": {
        "papel": "source", "id_servidor": "fonte", "imagem_da_linha": True}})
    fonte = liga(P_FONTE)
    criar_tabela(fonte)
    tubo = Tubo(P_TUBO, P_FONTE)
    subir("replica", {**config_base(P_REPLICA, h), "somente_leitura": True,
                      "replicacao": {"papel": "replica", "id_servidor": "replica",
                                     "imagem_da_linha": True,
                                     "origens": [origem_para(P_TUBO, "fonte", h)]}})
    replica = liga(P_REPLICA)
    # A telemetria da replica e a segunda testemunha, e a que fala de DENTRO:
    # `travar_dados()` cronometra a posse da trava, entao `totais.trava_ms`
    # diz quanto tempo alguem a segurou -- sem depender de um cliente de fora
    # perceber. As duas testemunhas tem de contar a mesma historia.
    replica({"op": "telemetria_ligar"})

    parar = threading.Event()

    def escrevendo():
        f = liga(P_FONTE)
        k = 1
        while not parar.is_set():
            linhas = [{"id": k + i, "nome": f"carga {i}", "cidade": "Itajai"}
                      for i in range(1000)]
            k += 1000
            try:
                f({"op": "inserir_lote", "database": "loja", "tabela": TABELA,
                   "linhas": linhas})
            except OSError:
                return

    escritor = threading.Thread(target=escrevendo, daemon=True)
    escritor.start()
    # A tabela tem de existir na replica antes de `varrer` valer alguma coisa.
    esperar(lambda: (eventos_de(replica) or 0) > 0, 30)
    time.sleep(2)          # tempo de o laco entrar no trecho produtivo
    trava0 = trava_ms(replica)
    tubo.mudo.set()
    time.sleep(1)
    parar.set()
    escritor.join(timeout=30)

    pings, varridas = [], []
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < SEGUNDOS_DE_SONDA:
        t = time.perf_counter()
        replica({"op": "ping"})
        pings.append((time.perf_counter() - t) * 1000)
        t = time.perf_counter()
        replica({"op": "varrer", "database": "loja", "tabela": TABELA, "max": 1})
        varridas.append((time.perf_counter() - t) * 1000)
    tubo.mudo.clear()
    posse_s = (trava_ms(replica) - trava0) / 1000.0
    pior_ping, pior_varrer = max(pings), max(varridas)
    ok = pior_varrer < 2_000 and pior_ping < 1_000
    medir("congelamento", ok,
          f"em {SEGUNDOS_DE_SONDA} s de tubo mudo, na replica: pior `ping` "
          f"{pior_ping:.0f} ms ({len(pings)} amostras), pior `varrer` "
          f"{pior_varrer:.0f} ms ({len(varridas)} amostras); a telemetria da "
          f"replica contou {posse_s:.1f} s de trava na mao",
          {"pior_ping_ms": round(pior_ping), "pior_varrer_ms": round(pior_varrer),
           "amostras": len(varridas), "posse_da_trava_s": round(posse_s, 1)})
    tubo.fechar()
    derrubar("fonte", "replica")


def estagio_alcance(h):
    """O preco do conserto, medido -- e nao suposto.

    Separar a leitura de rede do trabalho no dado custa alguma coisa: a tabela
    e ABERTA E FECHADA uma vez por lote, em vez de uma vez por rodada, e cada
    abertura nasce com o cache de paginas do `.ndx` vazio. Se esse preco
    comesse a vazao de aplicacao, o conserto teria trocado um problema por
    outro -- entao ele se mede, com o mesmo trabalho dos dois lados.
    """
    estagio("alcance",
            "com o source ja cheio e a replica subindo do zero, a vazao de "
            "aplicacao (eventos/s) nao pode desabar por causa da abertura de "
            "tabela por lote")
    subir("fonte", {**config_base(P_FONTE, h), "replicacao": {
        "papel": "source", "id_servidor": "fonte", "imagem_da_linha": True}})
    fonte = liga(P_FONTE)
    criar_tabela(fonte)
    for l in montar_lotes(1, LINHAS):
        gravar(fonte, l)
    subir("replica", {**config_base(P_REPLICA, h), "somente_leitura": True,
                      "replicacao": {"papel": "replica", "id_servidor": "replica",
                                     "imagem_da_linha": True,
                                     "origens": [origem_para(P_FONTE, "fonte", h)]}})
    replica = liga(P_REPLICA)
    # A segunda metade do preco: enquanto a replica aplica, quanto tempo um
    # cliente dela espera. E o outro prato da balanca do tamanho do lote --
    # lote maior amortiza a abertura de tabela e prende a trava por mais tempo.
    sonda = {"varrer": [], "parar": threading.Event()}

    def sondando():
        f = liga(P_REPLICA)
        while not sonda["parar"].is_set():
            t = time.perf_counter()
            f({"op": "varrer", "database": "loja", "tabela": TABELA, "max": 1})
            sonda["varrer"].append((time.perf_counter() - t) * 1000)

    olho = threading.Thread(target=sondando, daemon=True)
    olho.start()
    t0 = time.perf_counter()
    t = esperar(lambda: (eventos_de(replica) or 0) >= LINHAS, 900, passo=0.05)
    alcance_s = time.perf_counter() - t0
    sonda["parar"].set()
    olho.join(timeout=130)
    taxa = LINHAS / alcance_s if alcance_s else 0
    pior = max(sonda["varrer"]) if sonda["varrer"] else 0
    medir("alcance", t is not None,
          f"a replica alcancou {LINHAS} eventos em {alcance_s:.2f}s "
          f"({taxa:,.0f} eventos/s); durante a aplicacao, pior `varrer` "
          f"{pior:.0f} ms ({len(sonda['varrer'])} amostras)",
          {"alcance_s": round(alcance_s, 2), "eventos_s": round(taxa),
           "pior_varrer_ms": round(pior), "amostras": len(sonda["varrer"]),
           "linhas": LINHAS})
    derrubar("fonte", "replica")


def soma(fala):
    """A soma de verificacao do PROPRIO servidor: conteudo, linhas e slots.

    `slots` importa tanto quanto a soma: o `.reg` nunca reaproveita slot e o
    rowid e sempre `slots + 1`, entao dois lados com o mesmo numero de slots
    chegaram a mesma numeracao sozinhos. Contar linhas nao acha uma que
    atravessou errada.
    """
    r = fala({"op": "checksum", "database": "loja", "tabela": TABELA})
    if not r.get("ok"):
        return None
    d = r["resultado"]
    return (d["checksum"], d["linhas"], d["slots"])


def estagio_queda(h):
    """A conexao caindo ENTRE a leitura do lote e a aplicacao dele.

    O conserto le o lote inteiro do soquete e SO ENTAO pede a trava. Isso abre
    uma janela que antes nao existia: a conexao pode morrer com o lote ja na
    memoria. As tres saidas possiveis sao perder o lote, aplica-lo duas vezes,
    ou aplica-lo inteiro uma vez -- e so a terceira presta. A posicao local
    nasce do diario DAQUI, entao ela so anda depois de o lote estar gravado, e
    o mesmo lote volta na proxima rodada se a queda foi antes.

    A prova e por SOQUETE, com cortes de verdade no meio de uma carga grande,
    e o julgamento e a soma de verificacao dos dois lados -- nao a contagem.
    """
    estagio("queda",
            "com a conexao caindo repetidas vezes no meio do alcance, a "
            "replica converge com a MESMA soma, as mesmas linhas e os mesmos "
            "slots do source: nada se perde e nada se dobra")
    subir("fonte", {**config_base(P_FONTE, h), "replicacao": {
        "papel": "source", "id_servidor": "fonte", "imagem_da_linha": True}})
    fonte = liga(P_FONTE)
    criar_tabela(fonte)
    for l in montar_lotes(1, LINHAS):
        gravar(fonte, l)
    tubo = Tubo(P_TUBO, P_FONTE)
    subir("replica", {**config_base(P_REPLICA, h), "somente_leitura": True,
                      "replicacao": {"papel": "replica", "id_servidor": "replica",
                                     "imagem_da_linha": True,
                                     "origens": [origem_para(P_TUBO, "fonte", h)]}})
    replica = liga(P_REPLICA)
    cortes = 0
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < 12 and (eventos_de(replica) or 0) < LINHAS:
        time.sleep(0.3)
        cortes += 1 if tubo.cortar() else 0
    t = esperar(lambda: (eventos_de(replica) or 0) >= LINHAS, 300, passo=0.2)
    sf, sr = soma(fonte), soma(replica)
    ok = t is not None and sf is not None and sf == sr and cortes > 0
    medir("queda", ok,
          f"{cortes} corte(s) de conexao durante o alcance; convergiu="
          f"{t is not None}; soma do source={sf} da replica={sr} "
          f"({'IGUAL' if sf == sr else 'DIFERENTE'})",
          {"cortes": cortes, "soma_fonte": sf, "soma_replica": sr,
           "linhas": LINHAS})
    tubo.fechar()
    derrubar("fonte", "replica")


def estagio_abraco(h):
    estagio("abraco",
            "SEM corte nenhum, a rede sa: metade das linhas em cada lado de um "
            "par bidirecional, ao mesmo tempo. Se cada um segura a PROPRIA "
            "trava esperando a resposta do outro, a escrita do cliente para "
            "atras da trava e o diario ganha EAGAIN")
    # A referencia: as MESMAS linhas, o MESMO cliente, num servidor sozinho.
    # Sem ela o numero do bidirecional nao tem com o que ser comparado, e a
    # regra da casa e que a bancada compara trabalho igual.
    subir("sozinho", {**config_base(P_SOZINHO, h), "replicacao": {
        "papel": "source", "id_servidor": "sozinho", "imagem_da_linha": True}})
    so = liga(P_SOZINHO)
    criar_tabela(so)
    lotes = montar_lotes(1, LINHAS)
    t0 = time.perf_counter()
    for l in lotes:
        gravar(so, l)
    simples_s = time.perf_counter() - t0
    if (eventos_de(so) or 0) < LINHAS:
        raise SystemExit(f"o servidor sozinho gravou {eventos_de(so)} de {LINHAS}")
    derrubar("sozinho")

    subir("alfa", {**config_base(P_ALFA, h), "replicacao": {
        "papel": "multi", "id_servidor": "alfa", "imagem_da_linha": True,
        "origens": [origem_para(P_BETA, "beta", h)]}})
    subir("beta", {**config_base(P_BETA, h), "replicacao": {
        "papel": "multi", "id_servidor": "beta", "imagem_da_linha": True,
        "origens": [origem_para(P_ALFA, "alfa", h)]}})
    alfa, beta = liga(P_ALFA), liga(P_BETA)
    criar_tabela(alfa)
    esperar(lambda: eventos_de(beta) is not None, 60)
    a0, b0 = eagain("alfa"), eagain("beta")

    n = LINHAS // 2
    largada = threading.Barrier(2)

    def carga(porta, inicio, rotulo):
        f = liga(porta)
        lotes = montar_lotes(inicio, n, rotulo)
        largada.wait()
        for l in lotes:
            gravar(f, l)

    # A sonda roda DURANTE a carga, e nao depois -- e onde o defeito mora.
    # A primeira versao media so na convergencia e publicou "pior `posicao`
    # 0 ms" com a trava presa por segundos: quando as sondas comecaram, a
    # aplicacao ja tinha acabado. Medir depois do estrago mede o que sobrou.
    sonda = {"ping": [], "varrer": [], "parar": threading.Event()}

    def sondando():
        f = liga(P_ALFA)
        while not sonda["parar"].is_set():
            t = time.perf_counter()
            f({"op": "ping"})
            sonda["ping"].append((time.perf_counter() - t) * 1000)
            t = time.perf_counter()
            f({"op": "varrer", "database": "loja", "tabela": TABELA, "max": 1})
            sonda["varrer"].append((time.perf_counter() - t) * 1000)
            time.sleep(0.02)

    fios = [threading.Thread(target=carga, args=(P_ALFA, 1_000_000, "de alfa")),
            threading.Thread(target=carga, args=(P_BETA, 2_000_000, "de beta"))]
    olho = threading.Thread(target=sondando, daemon=True)
    olho.start()
    for f in fios:
        f.start()
    t0 = time.perf_counter()
    for f in fios:
        f.join()
    escrita_s = time.perf_counter() - t0
    sonda["parar"].set()
    olho.join(timeout=130)

    t = esperar(lambda: (eventos_de(alfa) or 0) >= 2 * n
                and (eventos_de(beta) or 0) >= 2 * n, 900, passo=0.5)
    a1, b1 = eagain("alfa"), eagain("beta")
    pior_ping = max(sonda["ping"]) if sonda["ping"] else 0
    pior_varrer = max(sonda["varrer"]) if sonda["varrer"] else 0
    ok = (t is not None and (a1 - a0) == 0 and (b1 - b0) == 0
          and pior_varrer < 2_000)
    medir("abraco", ok,
          f"{2 * n} linhas nos dois lados ao mesmo tempo levaram "
          f"{escrita_s:.1f}s (as mesmas {LINHAS} num servidor sozinho levam "
          f"{simples_s:.1f}s = {escrita_s / simples_s:.1f}x); convergiram "
          f"{t and round(t, 1)}s depois; EAGAIN novos: alfa +{a1 - a0}, "
          f"beta +{b1 - b0}; durante a carga, em alfa: pior `ping` "
          f"{pior_ping:.0f} ms, pior `varrer` {pior_varrer:.0f} ms "
          f"({len(sonda['varrer'])} amostras)",
          {"escrita_s": round(escrita_s, 1), "simples_s": round(simples_s, 1),
           "vezes": round(escrita_s / simples_s, 2),
           "convergencia_s": None if t is None else round(t, 1),
           "eagain_alfa": a1 - a0, "eagain_beta": b1 - b0,
           "pior_ping_ms": round(pior_ping),
           "pior_varrer_ms": round(pior_varrer),
           "amostras": len(sonda["varrer"]), "linhas": 2 * n})
    derrubar("alfa", "beta")


def gravar(fala, linhas):
    """Um lote, com o `ok` CONFERIDO -- lote recusado em silencio vira vazao.

    Sem esta conferencia, um erro de validacao faria o servidor devolver
    `ok:false` na mesma velocidade e a bancada publicaria a recusa como
    desempenho.
    """
    r = fala({"op": "inserir_lote", "database": "loja", "tabela": TABELA,
              "linhas": linhas})
    if not r.get("ok"):
        raise SystemExit(f"inserir_lote: {r}")
    return r


def montar_lotes(inicio, quantas, rotulo="carga"):
    lotes, k = [], inicio
    while k < inicio + quantas:
        quantos = min(5000, inicio + quantas - k)
        lotes.append([{"id": k + i, "nome": f"{rotulo} {i}", "cidade": "Itajai"}
                      for i in range(quantos)])
        k += quantos
    return lotes


def main():
    if not os.path.exists(PHXSQLD):
        raise SystemExit(f"falta {PHXSQLD} -- cargo build --release --bin phxsqld")
    quais = sys.argv[1:] or ["congela", "alcance", "queda", "abraco"]
    subprocess.run(["rm", "-rf", BASE], check=False)
    os.makedirs(BASE, exist_ok=True)
    h = hash_da_senha(SENHA)
    t0 = time.perf_counter()
    try:
        if "congela" in quais:
            estagio_congela(h)
        if "alcance" in quais:
            estagio_alcance(h)
        if "queda" in quais:
            estagio_queda(h)
        if "abraco" in quais:
            estagio_abraco(h)
    finally:
        derrubar()
    RESULTADO["minutos"] = round((time.perf_counter() - t0) / 60, 1)
    with open(os.path.join(AQUI, "trava.json"), "w") as f:
        json.dump(RESULTADO, f, indent=2)
    print()
    print(f"RESULTADO {json.dumps(RESULTADO)}")
    if FALHAS:
        print(f"FALHARAM: {', '.join(FALHAS)}")
    return 1 if FALHAS else 0


if __name__ == "__main__":
    sys.exit(main())
