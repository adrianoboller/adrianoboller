#!/usr/bin/env python3
"""A replica com credencial RECUSADA nao pode bloquear o proprio IP -- e o
operador que sai do mesmo endereco tem de continuar entrando.

    python3 bancada/replicacao/credencial-recusada.py [diretorio]

Pedido 203, filmado em 07/09/2026: com a origem mal configurada, a replica
tentava login em laco, o master a tratava como ataque e bloqueava o IP por uma
hora. Replica e navegador do operador saiam ambos de 127.0.0.1, entao o
bloqueio derrubou a sessao da tela no meio da gravacao.

O bloqueio do master esta CERTO como defesa e esta bancada NAO o afrouxa: a
regra da casa e que mexer em bloqueio sem medir o alcance e o jeito de abrir a
porta que ele fecha. O que ela mede e o lado da replica, que e onde o defeito
nasce -- uma credencial recusada nao e falha transitoria, e insistir nela so
serve para gastar a tolerancia do master.

# O que ela prova, e como

Tudo pelo soquete, contra servidores de pe -- «o que depende do sistema
operacional se prova contra o sistema operacional». Sobe um master e uma
replica com o `senha_hash` ERRADO, e conta no `acessos.log` do master:

1. quantas vezes a replica tentou entrar, e em quanto tempo;
2. se o master bloqueou o 127.0.0.1, depois de quantas e por quanto tempo;
3. se o operador do mesmo IP, com a senha CERTA, ainda entra.

Depois vem os controles, porque prova sem controle e prova de nada:

4. CONTROLE do comportamento velho do master: cinco logins errados feitos por
   esta propria bateria continuam bloqueando o IP -- a defesa nao afrouxou;
5. CONTROLE da falha de REDE: uma origem que aceita a conexao e fecha (uma
   escuta de mentira) tem de continuar sendo procurada, com o intervalo
   CRESCENDO -- credencial recusada para; origem fora do ar espera mais e
   tenta de novo;
6. RELIGAR: `replicacao_ligar` com a credencial ainda errada rende UMA
   tentativa a mais e para de novo; corrigido o `config.json` da replica e
   reiniciada -- o outro caminho de volta --, as linhas chegam.

O veredito e PASSA so com os seis. Contra o binario de antes do conserto ela
imprime os numeros do «antes» e sai com FALHA: e a mesma corrida que serve de
antes e de depois, e por isso ela nao tem interruptor de «modo antigo».

# As armadilhas que ja custaram aqui

* `socket.makefile()` segura o descritor: fechar so o soquete deixa o servidor
  sem ver o fim da conexao. Os dois se fecham.
* O bloqueio do master vale para o 127.0.0.1 inteiro -- inclusive para esta
  bateria. Entre uma fase e outra ela chama `phxsqld --desbloquear`, que e o
  mesmo caminho do operador local.
* O `acessos.log` e a fonte da contagem, e nao a saida do servidor: o log tem
  carimbo por linha, a saida nao.
"""

import hashlib
import hmac
import json
import os
import secrets
import socket
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
RESULTADO = os.path.join(AQUI, "credencial-recusada.json")

# `PHXSQLD` manda; sem ele, o release; sem release, o debug -- e a saida diz
# qual foi, porque medir com binario velho mede o passado.
PHXSQLD = os.environ.get("PHXSQLD") or next(
    (c for c in (os.path.join(RAIZ, "target", "release", "phxsqld"),
                 os.path.join(RAIZ, "target", "debug", "phxsqld"))
     if os.path.exists(c)), None)

PORTA = int(os.environ.get("PHX_CRED_PORTA", "5870"))
PORTA_MASTER = PORTA
PORTA_REPLICA = PORTA + 1
PORTA_REPLICA_REDE = PORTA + 2
PORTA_ESCUTA_FALSA = PORTA + 9
# A interface web da replica errada, so com `--tela`.
PORTA_WEB_REPLICA = PORTA + 3
# O exercicio da tela, num navegador de verdade: ve o aviso e clica «Religar».
RELIGAR_NA_TELA = os.path.join(RAIZ, "testes-web", "religar-na-tela.mjs")

# Os mesmos da bateria de frontend (`testes-web/servidor.mjs`), para o
# `entrar()` dela servir ao `--tela` sem uma segunda credencial.
TOKEN = "bateria"
OPERADOR, SENHA_OPERADOR = "adm", "segredo1"
REPLICA, SENHA_REPLICA = "replica", "senha-da-replica"
SENHA_ERRADA = "senha-errada"
# Segundos entre tentativas da replica quando a rodada falha. Um segundo,
# para a bateria caber em minutos; o defeito de origem acontecia com o padrao
# de 10 s, so que mais devagar.
RECONECTAR_EM = 1
# Quanto tempo observar a replica errada. Com a politica padrao (5 tentativas
# em 10 min) e um segundo entre elas, o bloqueio do binario antigo aparece em
# uns 5 s; 12 s deixam margem para contar o que vem depois dele.
OBSERVAR_S = int(os.environ.get("PHX_CRED_OBSERVAR", "12"))
# Quanto tempo escutar a origem de mentira do controle de rede.
ESCUTAR_S = int(os.environ.get("PHX_CRED_ESCUTAR", "20"))

PLACAR = []


def carimbo():
    return datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def caso(numero, titulo, esperado, real, passou):
    PLACAR.append((numero, titulo, passou))
    print(f"\n[{numero}] {titulo}")
    print(f"    esperado: {esperado}")
    for linha in str(real).splitlines() or [""]:
        print(f"    real:     {linha}")
    print(f"    -> {'PASSA' if passou else 'FALHA'}")


def hash_da_senha(senha):
    """O hash sai do proprio servidor -- nao ha uma segunda implementacao."""
    saida = subprocess.run([PHXSQLD, "--senha"], input=senha + "\n",
                           capture_output=True, text=True).stdout
    return saida.split('": "')[1].split('"')[0]


def permissoes_totais():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


# ------------------------------------------------------------------ o soquete


class Fio:
    """Uma conexao crua, JSON por linha. Fecha o soquete E o descritor."""

    def __init__(self, porta, prazo=10):
        self.s = socket.create_connection(("127.0.0.1", porta), prazo)
        self.s.settimeout(prazo)
        self.f = self.s.makefile("rwb")

    def bruto(self, pedido):
        pedido = dict(pedido, token=TOKEN)
        self.f.write((json.dumps(pedido) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            return {"ok": False, "erro": "<A CONEXAO FECHOU SEM RESPONDER>"}
        return json.loads(linha.decode())

    def pedir(self, pedido):
        r = self.bruto(pedido)
        if not r.get("ok"):
            raise RuntimeError(f"{pedido.get('op')}: {r.get('erro')}")
        return r.get("resultado")

    def entrar(self, usuario, senha):
        """O desafio-resposta do docs/SEGURANCA.md Sec.2, so com a std."""
        d = self.pedir({"op": "desafio", "usuario": usuario})
        dk = hashlib.pbkdf2_hmac("sha256", senha.encode(),
                                 bytes.fromhex(d["sal"]), d["iteracoes"], 32)
        nonce_cliente = secrets.token_hex(16)
        msg = f"{d['nonce']},{nonce_cliente},{usuario}".encode()
        prova = hmac.new(dk, msg, hashlib.sha256).hexdigest()
        return self.bruto({"op": "login", "usuario": usuario,
                           "nonce_cliente": nonce_cliente, "prova": prova})

    def fechar(self):
        try:
            self.f.close()
        finally:
            self.s.close()


def tenta_entrar(porta, usuario, senha):
    """Uma conexao nova, um login, e a resposta CRUA -- inclusive a recusa da
    porta, que vem antes de qualquer pedido quando o IP esta bloqueado."""
    try:
        fio = Fio(porta)
    except OSError as e:
        return {"ok": False, "erro": f"<conexao recusada: {e}>"}
    try:
        return fio.entrar(usuario, senha)
    except RuntimeError as e:
        return {"ok": False, "erro": str(e)}
    finally:
        fio.fechar()


def como_operador(porta, f):
    """Abre, entra como operador, roda `f(fio)`, fecha."""
    fio = Fio(porta)
    try:
        r = fio.entrar(OPERADOR, SENHA_OPERADOR)
        if not r.get("ok"):
            raise RuntimeError(f"operador nao entrou em {porta}: {r}")
        return f(fio)
    finally:
        fio.fechar()


# ---------------------------------------------------------------- os servidores


class Servidor:
    def __init__(self, base, nome, porta, cfg, web=None):
        self.dir = os.path.join(base, nome)
        os.makedirs(self.dir, exist_ok=True)
        self.nome, self.porta = nome, porta
        self.cfg = os.path.join(self.dir, "config.json")
        self.log = os.path.join(self.dir, "acessos.log")
        self.blacklist = os.path.join(self.dir, "blacklist.json")
        cfg = dict(cfg)
        cfg["bind"] = f"127.0.0.1:{porta}"
        cfg["base"] = os.path.join(self.dir, "base")
        cfg["token"] = TOKEN
        # A interface so sobe onde a prova pela tela (`--tela`) precisa dela.
        cfg["web"] = ({"ligado": True, "bind": f"127.0.0.1:{web}", "sessao_minutos": 60}
                      if web else {"ligado": False})
        cfg["log_acessos"] = self.log
        cfg.setdefault("seguranca", {})["blacklist"] = self.blacklist
        with open(self.cfg, "w") as f:
            json.dump(cfg, f, indent=2)
        self.p = None

    def subir(self):
        self.p = subprocess.Popen(
            [PHXSQLD, "--config", self.cfg],
            stdout=open(os.path.join(self.dir, "saida.log"), "a"),
            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        self.subiu_em = time.monotonic()
        for _ in range(200):
            try:
                socket.create_connection(("127.0.0.1", self.porta), 0.2).close()
                return self
            except OSError:
                time.sleep(0.05)
        raise SystemExit(f"{self.nome} nao subiu na porta {self.porta}")

    def derrubar(self):
        """Pelo PID guardado, nunca por `pkill`: o processo ao lado pode ser
        de outro agente."""
        if not self.p:
            return
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()
        self.p = None

    def acessos(self):
        if not os.path.exists(self.log):
            return []
        return [json.loads(l) for l in open(self.log) if l.strip()]

    def bloqueios(self):
        if not os.path.exists(self.blacklist):
            return []
        return json.load(open(self.blacklist)).get("bloqueios", [])

    def desbloquear(self, ip="127.0.0.1"):
        r = subprocess.run([PHXSQLD, "--desbloquear", ip, "--config", self.cfg],
                           capture_output=True, text=True)
        return (r.stdout + r.stderr).strip()

    def saida(self, ultimas=6):
        try:
            return "".join(open(os.path.join(self.dir, "saida.log")).readlines()[-ultimas:])
        except OSError:
            return ""


def config_master(h_operador, h_replica):
    return {
        "replicacao": {"papel": "source", "imagem_da_linha": True,
                       "id_servidor": "master"},
        "usuarios": [
            # Supervisor: e a guarda do cadastro que exige um -- sem ele o
            # `usuario_alterar` da fase 8 e recusado por «deixaria o servidor
            # sem nenhum supervisor ativo».
            {"login": OPERADOR, "nome": "Operador", "id": 10, "supervisor": True,
             "senha_hash": h_operador, "bases": permissoes_totais()},
            {"login": REPLICA, "nome": "Replica", "id": 11,
             "senha_hash": h_replica, "bases": permissoes_totais()},
        ],
    }


def config_replica(h_operador, h_origem, porta_origem, id_servidor):
    return {
        "somente_leitura": True,
        "replicacao": {
            "papel": "replica", "id_servidor": id_servidor,
            "imagem_da_linha": True,
            "origens": [{"nome": "master", "host": "127.0.0.1",
                         "porta": porta_origem, "token": TOKEN,
                         "usuario": REPLICA, "senha_hash": h_origem,
                         "databases": ["loja"],
                         "reconectar_em": RECONECTAR_EM}],
        },
        "usuarios": [
            {"login": OPERADOR, "nome": "Operador", "id": 10,
             "senha_hash": h_operador, "bases": permissoes_totais()},
        ],
    }


# ------------------------------------------------------------------ as sondas


def falhas_da_replica(master, desde_ms):
    """As linhas do log do master que sao a replica batendo na porta: o
    `login`/`desafio` recusado (antes do bloqueio) e a `conexao` barrada
    (depois dele). Ambas saem do 127.0.0.1, como o operador -- e e por isso
    que o usuario da linha e o que separa uma da outra."""
    logins, barradas = [], []
    for a in master.acessos():
        if a.get("quando_ms", 0) < desde_ms:
            continue
        if a.get("op") in ("login", "desafio") and not a.get("ok") \
                and a.get("usuario", "") in ("", REPLICA):
            logins.append(a)
        elif a.get("op") == "conexao" and not a.get("ok"):
            barradas.append(a)
    return logins, barradas


def estado_da_origem(porta, origem="master"):
    def ler(fio):
        e = fio.pedir({"op": "replicacao_estado"})
        return (e.get("origens") or {}).get(origem) or {}
    try:
        return como_operador(porta, ler)
    except (OSError, RuntimeError) as e:
        return {"<sem estado>": str(e)}


def religar(porta, origem="master"):
    def pedir(fio):
        return fio.bruto({"op": "replicacao_ligar", "origem": origem})
    try:
        return como_operador(porta, pedir)
    except (OSError, RuntimeError) as e:
        return {"ok": False, "erro": str(e)}


def religar_pela_tela(base):
    """O botao «Religar» do dialogo de acompanhar replica, clicado num
    navegador de verdade. O script de tela devolve JSON numa linha: o que
    viu e o que clicou; a captura fica ao lado dos servidores."""
    captura = os.path.join(base, "religar-na-tela.png")
    r = subprocess.run(["node", RELIGAR_NA_TELA, "--web", str(PORTA_WEB_REPLICA),
                        "--captura", captura], capture_output=True, text=True,
                       timeout=120)
    ultima = (r.stdout.strip().splitlines() or ["{}"])[-1]
    try:
        saida = json.loads(ultima)
    except ValueError:
        saida = {"ok": False, "erro": (r.stdout + r.stderr)[-600:]}
    saida["captura"] = captura if os.path.exists(captura) else None
    return saida


def esperar(condicao, segundos, passo=0.25):
    fim = time.monotonic() + segundos
    while time.monotonic() < fim:
        v = condicao()
        if v:
            return v
        time.sleep(passo)
    return None


class EscutaFalsa:
    """Uma origem que aceita a conexao e fecha na hora: do lado da replica e
    «a origem caiu no meio», que e falha de REDE. Anota o instante de cada
    conexao -- e a sequencia de intervalos e a prova do recuo."""

    def __init__(self, porta):
        self.porta = porta
        self.instantes = []
        self.s = socket.socket()
        self.s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.s.bind(("127.0.0.1", porta))
        self.s.listen(8)
        self.s.settimeout(0.2)
        self.viva = True
        self.t = threading.Thread(target=self.laco, daemon=True)
        self.t.start()

    def laco(self):
        while self.viva:
            try:
                c, _ = self.s.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            self.instantes.append(time.monotonic())
            c.close()

    def parar(self):
        self.viva = False
        self.t.join(timeout=2)
        self.s.close()

    def intervalos(self):
        return [round(b - a, 2) for a, b in zip(self.instantes, self.instantes[1:])]


# ---------------------------------------------------------------------- main


def main():
    if not PHXSQLD:
        sys.exit("nao achei target/release/phxsqld nem target/debug/phxsqld -- "
                 "rode `cargo build --release --bin phxsqld` antes")
    base = next((a for a in sys.argv[1:] if not a.startswith("--")),
                f"/tmp/phx-credencial-{os.getpid()}")
    # `--tela`: o religar do caso 7 acontece pelo BOTAO, num navegador de
    # verdade, em vez de pelo protocolo. Interface so se prova exercitando.
    tela = "--tela" in sys.argv
    os.makedirs(base, exist_ok=True)
    mtime = datetime.fromtimestamp(os.path.getmtime(PHXSQLD), timezone.utc)
    print(f"binario  {PHXSQLD}  (de {mtime:%Y-%m-%d %H:%M} UTC)")
    print(f"pasta    {base}")
    print(f"portas   master {PORTA_MASTER} | replica errada {PORTA_REPLICA} | "
          f"replica de rede {PORTA_REPLICA_REDE} -> escuta falsa {PORTA_ESCUTA_FALSA}")
    print(f"medido em {carimbo()}")

    h_operador = hash_da_senha(SENHA_OPERADOR)
    h_replica = hash_da_senha(SENHA_REPLICA)
    h_errado = hash_da_senha(SENHA_ERRADA)

    resultado = {"medido_em": carimbo(), "binario": os.path.relpath(PHXSQLD, RAIZ),
                 "reconectar_em_s": RECONECTAR_EM, "observado_s": OBSERVAR_S}
    servidores = []
    escuta = None
    try:
        master = Servidor(base, "master", PORTA_MASTER,
                          config_master(h_operador, h_replica)).subir()
        servidores.append(master)

        # A politica em vigor sai do servidor, nao daqui: e o `bloqueios` que
        # diz quantas tentativas ele tolera, em que janela e por quanto tempo.
        politica = como_operador(PORTA_MASTER,
                                 lambda f: f.pedir({"op": "bloqueios"}))["politica"]
        resultado["politica"] = {k: politica[k] for k in
                                 ("tentativas_ate_bloquear", "janela_minutos",
                                  "bloqueio_minutos")}
        print(f"politica do master: {resultado['politica']}")

        # Dado no master para a replica ter o que puxar quando for religada.
        def semear(fio):
            fio.pedir({"op": "criar_database", "database": "loja"})
            fio.pedir({"op": "criar_tabela", "database": "loja", "tabela": "clientes",
                       "motivo_obrigatorio": False,
                       "colunas": [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
                                   {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True}],
                       "indices": [{"nome": "porId", "colunas": ["id"],
                                    "unico": True, "primario": True}]})
            for i, nome in enumerate(["Ana", "Bento", "Carla"], start=1):
                fio.pedir({"op": "inserir", "database": "loja", "tabela": "clientes",
                           "linha": {"id": i, "nome": nome}})
        como_operador(PORTA_MASTER, semear)

        # ---- 1. o operador entra ANTES de a replica existir (controle positivo)
        r = tenta_entrar(PORTA_MASTER, OPERADOR, SENHA_OPERADOR)
        caso(1, "o operador entra no master antes de a replica subir",
             "ok", json.dumps(r, ensure_ascii=False), bool(r.get("ok")))

        # ---- 2. a replica com o hash ERRADO, observada pelo log do master
        replica = Servidor(base, "replica-errada", PORTA_REPLICA,
                           config_replica(h_operador, h_errado, PORTA_MASTER,
                                          "replica-errada"),
                           web=PORTA_WEB_REPLICA if tela else None).subir()
        servidores.append(replica)
        desde_ms = int(time.time() * 1000) - 500
        t0 = time.monotonic()
        bloqueio, bloqueio_apos_s = None, None
        while time.monotonic() - t0 < OBSERVAR_S:
            if bloqueio is None:
                b = [x for x in master.bloqueios() if x["ip"] == "127.0.0.1"]
                if b:
                    bloqueio = b[0]
                    bloqueio_apos_s = round(time.monotonic() - t0, 1)
            time.sleep(0.25)
        logins, barradas = falhas_da_replica(master, desde_ms)
        janela_s = bloqueio_apos_s if bloqueio_apos_s else OBSERVAR_S
        antes_do_bloqueio = [a for a in logins
                             if bloqueio is None or a["quando_ms"] <= bloqueio["desde_ms"]]
        por_minuto = round(len(antes_do_bloqueio) * 60 / max(janela_s, 0.001), 1)
        resultado.update({
            "tentativas_da_replica": len(logins),
            "tentativas_por_minuto": por_minuto,
            "conexoes_barradas": len(barradas),
            "bloqueou": bloqueio is not None,
            "bloqueio_apos_s": bloqueio_apos_s,
            "bloqueio_tentativas": bloqueio and bloqueio.get("tentativas"),
            "bloqueio_minutos_medido": bloqueio and round(
                (bloqueio["ate_ms"] - bloqueio["desde_ms"]) / 60000, 1),
            "bloqueio_motivo": bloqueio and f"{bloqueio['motivo']} ({bloqueio['comando']})",
        })
        real = (f"{len(logins)} login(s)/desafio(s) recusado(s) em {OBSERVAR_S} s "
                f"({por_minuto}/min antes do bloqueio); {len(barradas)} conexao(oes) barrada(s)\n"
                + (f"BLOQUEOU o 127.0.0.1 apos {bloqueio_apos_s} s e "
                   f"{bloqueio['tentativas']} tentativas, por "
                   f"{resultado['bloqueio_minutos_medido']} min: "
                   f"{resultado['bloqueio_motivo']}" if bloqueio
                   else "blacklist.json sem o 127.0.0.1"))
        caso(2, "a replica com credencial recusada tenta UMA vez e nao bloqueia o IP",
             "1 tentativa, blacklist vazia", real,
             len(logins) <= 1 and bloqueio is None)

        # ---- 3. o operador do mesmo IP continua entrando
        r = tenta_entrar(PORTA_MASTER, OPERADOR, SENHA_OPERADOR)
        resultado["operador_derrubado"] = not bool(r.get("ok"))
        caso(3, "o operador do MESMO IP entra durante o que seria o bloqueio",
             "ok", json.dumps(r, ensure_ascii=False), bool(r.get("ok")))

        # ---- 4. a replica publica o motivo de ter parado
        eo = estado_da_origem(PORTA_REPLICA)
        resultado["estado_da_replica"] = {k: eo.get(k) for k in
                                          ("parada", "ultimo_erro", "religadas",
                                           "falhas_de_rede_seguidas", "proxima_tentativa")}
        caso(4, "`replicacao_estado` diz que o laco parou por credencial recusada",
             'parada = "credencial_recusada", com o erro da origem em ultimo_erro',
             json.dumps(resultado["estado_da_replica"], ensure_ascii=False),
             eo.get("parada") == "credencial_recusada" and bool(eo.get("ultimo_erro")))

        # Antes de mexer no master de novo, o IP tem de estar solto -- senao
        # os controles medem o bloqueio da fase 2 em vez do que eles proprios
        # provocam. O binario consertado nao bloqueou, e o `--desbloquear`
        # diz isso.
        print(f"\n    --desbloquear 127.0.0.1 -> {master.desbloquear()}")

        # ---- 5. CONTROLE: o master continua bloqueando N credenciais erradas
        n = int(politica["tentativas_ate_bloquear"])
        respostas = [tenta_entrar(PORTA_MASTER, OPERADOR, SENHA_ERRADA) for _ in range(n)]
        b = [x for x in master.bloqueios() if x["ip"] == "127.0.0.1"]
        depois = tenta_entrar(PORTA_MASTER, OPERADOR, SENHA_OPERADOR)
        resultado["master_bloqueia_n_erradas"] = bool(b) and not depois.get("ok")
        caso(5, f"CONTROLE: {n} logins errados desta bateria continuam bloqueando o IP",
             f"blacklist com o 127.0.0.1 apos {n} recusas, e o operador barrado",
             f"{n} recusas: {respostas[-1].get('erro')}\n"
             f"blacklist: {json.dumps(b, ensure_ascii=False)}\n"
             f"operador depois: {json.dumps(depois, ensure_ascii=False)}",
             resultado["master_bloqueia_n_erradas"])
        print(f"\n    --desbloquear 127.0.0.1 -> {master.desbloquear()}")

        # ---- 6. CONTROLE: origem que cai no meio e falha de REDE, e o
        #         intervalo cresce
        escuta = EscutaFalsa(PORTA_ESCUTA_FALSA)
        rede = Servidor(base, "replica-rede", PORTA_REPLICA_REDE,
                        config_replica(h_operador, h_replica, PORTA_ESCUTA_FALSA,
                                       "replica-rede")).subir()
        servidores.append(rede)
        time.sleep(ESCUTAR_S)
        intervalos = escuta.intervalos()
        eo_rede = estado_da_origem(PORTA_REPLICA_REDE)
        cresce = len(intervalos) >= 2 and all(
            b >= a * 0.9 for a, b in zip(intervalos, intervalos[1:])) \
            and intervalos[-1] >= 2 * intervalos[0]
        resultado["rede_conexoes"] = len(escuta.instantes)
        resultado["rede_intervalos_s"] = intervalos
        resultado["rede_estado"] = {k: eo_rede.get(k) for k in
                                    ("parada", "falhas_de_rede_seguidas", "proxima_tentativa")}
        caso(6, "CONTROLE: a origem fora do ar continua sendo procurada, com recuo crescente",
             f"varias conexoes em {ESCUTAR_S} s, intervalos dobrando; parada vazia",
             f"{len(escuta.instantes)} conexoes; intervalos {intervalos}\n"
             f"estado: {json.dumps(resultado['rede_estado'], ensure_ascii=False)}",
             cresce and not eo_rede.get("parada"))

        # ---- 7. religar com a credencial ainda errada: UMA tentativa a mais
        desde_ms = int(time.time() * 1000) - 200
        if tela:
            r = religar_pela_tela(base)
        else:
            r = religar(PORTA_REPLICA)
        eo = esperar(lambda: (lambda e: e if e.get("parada") == "credencial_recusada"
                              and (e.get("religadas") or 0) >= 1 else None)
                     (estado_da_origem(PORTA_REPLICA)), 6) or estado_da_origem(PORTA_REPLICA)
        time.sleep(2 * RECONECTAR_EM + 1)
        logins, _ = falhas_da_replica(master, desde_ms)
        resultado["religar_com_erro"] = {"resposta": r, "tentativas": len(logins),
                                         "religadas": eo.get("religadas"),
                                         "parada": eo.get("parada")}
        caso(7, "`replicacao_ligar` com a credencial ainda errada: uma tentativa, e para de novo",
             "1 tentativa no log do master, religadas = 1, parada de novo",
             json.dumps(resultado["religar_com_erro"], ensure_ascii=False),
             bool(r.get("ok")) and len(logins) == 1 and eo.get("religadas") == 1
             and eo.get("parada") == "credencial_recusada")
        print(f"\n    --desbloquear 127.0.0.1 -> {master.desbloquear()}")

        # ---- 8. corrigido o config.json da REPLICA e reiniciada, as linhas chegam
        #
        # O conserto e do lado da replica, e por REINICIO -- o caminho de
        # volta que nao e o `replicacao_ligar`. Nao da para consertar pelo
        # master com `usuario_alterar`, e a primeira versao desta bateria
        # tentou: o `senha_hash` da replica tem de ser o MESMO texto do
        # cadastro de la, porque e do sal dele que ela deriva a chave do
        # desafio; `usuario_alterar` recebe a senha e sorteia outro sal, e a
        # replica continua recusada com a senha certa.
        replica.derrubar()
        replica = Servidor(base, "replica-errada", PORTA_REPLICA,
                           config_replica(h_operador, h_replica, PORTA_MASTER,
                                          "replica-errada")).subir()
        servidores[servidores.index(next(x for x in servidores
                                         if x.nome == "replica-errada"))] = replica
        r = {"ok": True, "reiniciada": True}

        def alcancou():
            try:
                p = como_operador(PORTA_REPLICA, lambda f: f.pedir(
                    {"op": "posicao", "database": "loja"}))
            except (OSError, RuntimeError):
                return None
            ev = ((p.get("tabelas") or {}).get("clientes") or {}).get("eventos", 0)
            return ev if ev >= 3 else None
        eventos = esperar(alcancou, 15)
        eo = estado_da_origem(PORTA_REPLICA)
        resultado["religar_corrigida"] = {"resposta": r, "eventos_na_replica": eventos,
                                          "parada": eo.get("parada"),
                                          "aplicados": eo.get("aplicados")}
        caso(8, "config.json da replica corrigido e replica reiniciada: as 3 linhas chegam",
             "3 eventos em loja.clientes na replica, parada vazia",
             json.dumps(resultado["religar_corrigida"], ensure_ascii=False),
             eventos == 3 and not eo.get("parada"))

    finally:
        if escuta:
            escuta.parar()
        for s in servidores:
            s.derrubar()

    passaram = sum(1 for _, _, p in PLACAR if p)
    resultado["casos"] = len(PLACAR)
    resultado["passaram"] = passaram
    resultado["veredito"] = "PASSA" if passaram == len(PLACAR) else "FALHA"
    print("\n" + "=" * 72)
    for numero, titulo, passou in PLACAR:
        print(f"  [{numero}] {'PASSA' if passou else 'FALHA'}  {titulo}")
    print(f"\n{passaram} de {len(PLACAR)} casos -- {resultado['veredito']}")
    with open(RESULTADO, "w") as f:
        json.dump(resultado, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"resultado em {os.path.relpath(RESULTADO, RAIZ)}")
    sys.exit(0 if resultado["veredito"] == "PASSA" else 1)


if __name__ == "__main__":
    main()
