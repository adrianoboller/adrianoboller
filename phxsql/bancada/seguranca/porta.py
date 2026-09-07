#!/usr/bin/env python3
"""Bateria de ACESSO e SEGURANCA da porta TCP/IP do PhxSql.

    python3 bancada/seguranca/porta.py

Pergunta do dono, 07/09/2026: *«bateria de teste de acesso, seguranca e
tentativas de acessar o banco pela porta TCP/IP relations se esta seguro»*.

Ela nao le o codigo: sobe um `phxsqld` de verdade e **chega pelo soquete**,
como chegaria quem esta de fora. Cada caso traz o que se esperava e o que o
motor devolveu, colado como veio.

# As tres coisas que esta bateria aprendeu na primeira corrida

1. **A bateria bloqueia a si mesma.** Tudo aqui sai de `127.0.0.1`, e a
   politica nao abre excecao para o localhost (SEGURANCA.md secao 3). Provado o
   bloqueio, o proximo caso ja nao conecta -- por isso a bateria chama
   `phxsqld --desbloquear` entre um caso e outro, que e o mesmo caminho do
   operador local.
2. **Fechar o soquete nao fecha o descritor.** `socket.makefile()` segura o fd;
   fechar so o soquete deixa o servidor sem ver o fim da conexao. Os dois se
   fecham, e e o que faz o caso 5 medir a queda de verdade.
3. **Ausencia de resposta e um resultado.** A linha maior que o teto derruba a
   conexao sem responder e sem deixar rastro. Uma sonda que so lesse a resposta
   diria "erro de leitura" e passaria adiante; esta olha o `acessos.log` depois
   e conta a falta.
"""

import glob
import json
import os
import shutil
import socket
import subprocess
import sys
import time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARIO = os.path.join(RAIZ, "target/release/phxsqld")
BASE = f"/tmp/phx-f6-{os.getpid()}"

PORTA = int(os.environ.get("PHX_SEG_PORTA", "6600"))
PORTA_WL = PORTA + 1
PORTA_REP = PORTA + 2
PORTA_FONTE = PORTA + 3
PORTA_TETO = PORTA + 4
PORTA_FECHADA = PORTA + 9

SENHA = "segredo-da-bancada-f6"
TETO_DO_REGISTRO = 128 * 1024 * 1024  # phxsql-core/src/fio.rs

PLACAR = []


def caso(numero, titulo, esperado, real, veredito):
    PLACAR.append((numero, titulo, veredito))
    print(f"\n[{numero}] {titulo}")
    print(f"    esperado: {esperado}")
    for linha in str(real).splitlines() or [""]:
        print(f"    real:     {linha}")
    print(f"    veredito: {veredito}")


def hash_da_senha(senha):
    """A senha em claro NUNCA vai para o config.json -- so o hash do proprio motor."""
    r = subprocess.run([BINARIO, "--senha"], input=senha.encode(), capture_output=True)
    return r.stdout.decode().split('"')[3]


class Servidor:
    """Sobe um `phxsqld` de verdade e o derruba no fim, aconteca o que acontecer.

    Copiada de `bancada/alfanumerica/sonda.py` e adaptada: aqui NAO ha conexao
    guardada, porque um IP bloqueado tem a conexao recusada antes de tudo e uma
    conexao viva mascararia justamente o que se quer medir.
    """

    def __init__(self, nome, porta, seguranca=None, replicacao=None, usuarios=False, extra=None):
        self.nome, self.porta = nome, porta
        self.dir = f"{BASE}/{nome}"
        self.cfg = f"{self.dir}/config.json"
        self.log = f"{self.dir}/acessos.log"
        self.blacklist = f"{self.dir}/blacklist.json"
        self.seguranca = seguranca or {}
        self.replicacao = replicacao
        self.usuarios = usuarios
        self.extra = extra or {}

    def __enter__(self):
        if not os.path.exists(BINARIO):
            raise SystemExit(
                f"{BINARIO} nao existe. Rode antes:\n"
                "  flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server"
            )
        os.makedirs(self.dir + "/dados", exist_ok=True)
        politica = {
            "comandos_proibidos": ["reindexar"],
            "bases_proibidas": ["financeiro"],
            "tentativas_para_bloqueio": 1,
            "tentativas_ate_bloquear": 5,
            "janela_minutos": 10,
            "bloqueio_minutos": 60,
            "whitelist": [],
            "blacklist": self.blacklist,
        }
        politica.update(self.seguranca)
        cfg = {
            "bind": f"127.0.0.1:{self.porta}",
            "base": self.dir + "/dados",
            "token": "t",
            "web": {"ligado": False},
            "max_linhas": 200000,
            "log_acessos": self.log,
            "seguranca": politica,
        }
        if self.replicacao:
            cfg["replicacao"] = self.replicacao
        cfg.update(self.extra)
        if self.usuarios:
            h = hash_da_senha(SENHA)
            cfg["root"] = {"id": 1, "nome": "root", "login": "root", "senha_hash": h}
            cfg["usuarios"] = [
                {
                    "id": 4,
                    "nome": "Carlos Consulta",
                    "login": "carlos",
                    "senha_hash": h,
                    "ativo": True,
                    "bases": {"loja": {"ler": True, "verificar": True}},
                }
            ]
        with open(self.cfg, "w") as f:
            json.dump(cfg, f)
        self.p = subprocess.Popen(
            [BINARIO, "--config", self.cfg],
            stdout=open(self.dir + "/saida.log", "w"),
            stderr=subprocess.STDOUT,
        )
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", self.porta), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)
        else:
            raise SystemExit(f"o servidor nao subiu na porta {self.porta}")
        return self

    def __exit__(self, *_):
        # Pelo PID guardado, nunca por `pkill -f`: o processo ao lado pode ser
        # de outro agente.
        self.p.terminate()
        try:
            self.p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.p.kill()

    # ---------------------------------------------------------- o soquete

    def fala(self, *pedidos, token="t", cru=None, prazo=10):
        """Abre UMA conexao, manda os pedidos e devolve as respostas cruas."""
        respostas = []
        s = socket.create_connection(("127.0.0.1", self.porta), prazo)
        s.settimeout(prazo)
        f = s.makefile("rwb")
        try:
            for p in pedidos:
                if cru is not None:
                    f.write(cru)
                else:
                    if token is not None:
                        p = dict(p, token=token)
                    f.write((json.dumps(p) + "\n").encode())
                f.flush()
                l = f.readline()
                respostas.append(l.decode().strip() if l else "<A CONEXAO FECHOU SEM RESPONDER>")
        finally:
            f.close()
            s.close()
        return respostas

    def json_de(self, *pedidos, **kw):
        return [json.loads(r) if r.startswith("{") else {"cru": r} for r in self.fala(*pedidos, **kw)]

    def desbloquear(self, ip="127.0.0.1"):
        r = subprocess.run(
            [BINARIO, "--desbloquear", ip, "--config", self.cfg], capture_output=True
        )
        return r.stdout.decode().strip()

    def bloqueios(self):
        if not os.path.exists(self.blacklist):
            return []
        return json.load(open(self.blacklist)).get("bloqueios", [])

    def acessos(self):
        if not os.path.exists(self.log):
            return []
        return [json.loads(l) for l in open(self.log) if l.strip()]

    def rss_kb(self):
        for l in open(f"/proc/{self.p.pid}/status"):
            if l.startswith("VmRSS:"):
                return int(l.split()[1])
        return 0

    def portas_escutando(self):
        """As portas em LISTEN DESTE processo, por /proc -- nao ha nmap nem ss aqui."""
        inodes = set()
        for fd in glob.glob(f"/proc/{self.p.pid}/fd/*"):
            try:
                alvo = os.readlink(fd)
            except OSError:
                continue
            if alvo.startswith("socket:["):
                inodes.add(alvo[8:-1])
        achadas = []
        for caminho, v6 in (("/proc/net/tcp", False), ("/proc/net/tcp6", True)):
            if not os.path.exists(caminho):
                continue
            with open(caminho) as f:
                next(f)
                for l in f:
                    c = l.split()
                    if c[3] != "0A" or c[9] not in inodes:
                        continue
                    ip, porta = c[1].split(":")
                    if not v6:
                        ip = ".".join(str(int(ip[i : i + 2], 16)) for i in (6, 4, 2, 0))
                    achadas.append(f"{ip}:{int(porta, 16)}")
        return sorted(achadas)


def corte(texto, n=200):
    t = str(texto)
    return t if len(t) <= n else t[:n] + " …"


# =====================================================================
def parte_1_o_token(sv):
    print("\n=== 1-2. O token: a recusa, a contagem e o bloqueio\n")

    r = sv.fala({"op": "ping"}, token=None)[0]
    caso(
        1,
        "pedido SEM token",
        "recusa com ACESSO_NEGADO 4001, sem executar o ping",
        r,
        "PASSOU" if '"codigo":4001' in r and '"ok":false' in r else "FALHOU",
    )
    # O caso 1 JA foi uma violacao leve. Contar a partir de zero aqui daria um
    # limite deslocado -- e a bateria acusaria o motor pelo erro dela.
    leves = 1

    limite = json.load(open(sv.cfg))["seguranca"]["tentativas_ate_bloquear"]
    print(f"\n    a politica em vigor: tentativas_ate_bloquear = {limite}")
    print(f"    o caso 1 ja gastou {leves} tentativa leve; a contagem continua dali")
    linhas = []
    bloqueou_na = None
    for _ in range(limite + 2):
        leves += 1
        r = sv.fala({"op": "ping"}, token="token-errado")[0]
        linhas.append(f"tentativa leve {leves}: {corte(r, 150)}")
        if '"op":"conexao"' in r and "bloqueado" in r and bloqueou_na is None:
            bloqueou_na = leves
    b = sv.bloqueios()
    linhas.append(f"blacklist.json: {json.dumps(b, ensure_ascii=False)}")
    caso(
        2,
        f"token errado ate o limite da politica ({limite}) -> bloqueio do IP",
        f"a {limite}a tentativa leve bloqueia (o blacklist.json grava tentativas={limite}); "
        f"da {limite + 1}a em diante a recusa e da CONEXAO, antes do token",
        "\n".join(linhas),
        "PASSOU"
        if bloqueou_na == limite + 1 and b and b[0]["tentativas"] == limite
        else "FALHOU",
    )

    negadas = [a for a in sv.acessos() if a["op"] == "conexao" and not a["ok"]]
    caso(
        "2b",
        "o bloqueio entra no acessos.log",
        "uma linha por conexao recusada, com ip, quando e ok:false",
        corte(json.dumps(negadas[-1], ensure_ascii=False), 260) if negadas else "NADA NO LOG",
        "PASSOU" if negadas else "FALHOU",
    )
    print(f"\n    --desbloquear 127.0.0.1 -> {sv.desbloquear()}")

    # As tres GRAVES: bloqueiam na primeira.
    graves = [
        ("comando proibido", {"op": "reindexar", "database": "loja", "tabela": "clientes"}),
        ("base proibida", {"op": "varrer", "database": "financeiro", "tabela": "x"}),
        ("travessia de diretorio", {"op": "varrer", "database": "../../etc", "tabela": "passwd"}),
    ]
    linhas = []
    todas = True
    for nome, pedido in graves:
        r = sv.fala(pedido)[0]
        b = sv.bloqueios()
        ok = bool(b) and b[0]["tentativas"] == 1
        todas = todas and ok and '"ok":false' in r
        linhas.append(f"{nome}: {corte(r, 170)}")
        linhas.append(
            f"    -> motivo {b[0]['motivo']!r}, tentativas {b[0]['tentativas']}"
            if b
            else "    -> NAO BLOQUEOU"
        )
        sv.desbloquear()
    caso(
        "2c",
        "gravidade GRAVE bloqueia na PRIMEIRA (secao 3 do SEGURANCA.md)",
        "as tres bloqueiam com tentativas=1, e a resposta diz que o IP foi bloqueado",
        "\n".join(linhas),
        "PASSOU" if todas else "FALHOU",
    )


def parte_3_whitelist(sv):
    print("\n=== 3. A whitelist: quem nunca bloqueia\n")
    linhas = []
    for _ in range(8):
        sv.fala({"op": "ping"}, token="token-errado")
    r = sv.fala({"op": "ping"})[0]
    b = sv.bloqueios()
    linhas.append(f"8 tokens errados -> blacklist.json bloqueios = {json.dumps(b)}")
    linhas.append(f"CONTROLE POSITIVO, pedido legitimo depois: {corte(r, 170)}")
    caso(
        3,
        "IP na whitelist nunca bloqueia",
        "nenhum bloqueio gravado depois de 8 tentativas leves, e o pedido legitimo passa",
        "\n".join(linhas),
        "PASSOU" if not b and '"ok":true' in r else "FALHOU",
    )


def parte_4_pedidos_torcidos(sv):
    print("\n=== 4. Pedido torcido: JSON quebrado, linha gigante, op que nao existe\n")

    crus = [
        ("JSON malformado", b'{"op":"ping","token":"t"\n'),
        ("nao e JSON", b"isto nao e json\n"),
        ("lista em vez de objeto", b'[1,2,3]\n'),
        ("byte nulo no meio", b'{"op":"pi\x00ng","token":"t"}\n'),
    ]
    linhas = []
    todas = True
    for nome, bytes_ in crus:
        r = sv.fala(None, cru=bytes_)[0]
        todas = todas and '"ok":false' in r
        linhas.append(f"{nome}: {corte(r, 160)}")
    caso(
        4,
        "JSON malformado nao derruba o servidor",
        "erro nomeado por linha, a conexao continua, e nada executa",
        "\n".join(linhas),
        "PASSOU" if todas else "FALHOU",
    )

    rss0 = sv.rss_kb()
    cabe = b'{"op":"ping","token":"t","enchimento":"' + b"A" * (1024 * 1024) + b'"}\n'
    r_cabe = sv.fala(None, cru=cabe, prazo=60)[0]
    # O `antes` e lido AQUI, e nao depois. A primeira versao contava as linhas
    # do log DEPOIS de mandar a linha gigante e comparava com 0,3 s mais tarde:
    # o log ja tinha a linha nova dentro do `antes`, e a conta dava zero tanto
    # com rastro quanto sem. *Medir depois do estrago mede o que sobrou.*
    antes = len(sv.acessos())
    t0 = time.time()
    estoura = b'{"op":"ping","token":"t","enchimento":"' + b"A" * (TETO_DO_REGISTRO + 1024) + b'"}\n'
    try:
        r_estoura = sv.fala(None, cru=estoura, prazo=120)[0]
    except OSError as e:
        r_estoura = f"<{type(e).__name__}: {e}>"
    gasto = time.time() - t0
    rss1 = sv.rss_kb()
    time.sleep(0.3)
    novas = [a for a in sv.acessos()[antes:]]
    rastro = [a for a in novas if a.get("op") == "fio" and not a.get("ok")]
    caso(
        "4b",
        f"linha GIGANTE (teto do fio = {TETO_DO_REGISTRO // (1024 * 1024)} MiB)",
        "1 MiB passa; acima do teto o cliente RECEBE um erro com codigo, o "
        "`acessos.log` ganha a recusa com IP, tamanho e teto, e o processo fica de pe",
        f"1 MiB:        {corte(r_cabe, 120)}\n"
        f"{len(estoura)} bytes: {corte(r_estoura, 320)}   ({gasto:.2f}s)\n"
        f"RSS do servidor: {rss0} kB antes, {rss1} kB depois -- o processo continua de pe\n"
        f"linhas novas no acessos.log por causa dela: {len(novas)}\n"
        f"    {corte(json.dumps(rastro, ensure_ascii=False), 320)}\n"
        f"bloqueios apos a tentativa (o interruptor esta DESLIGADO aqui): "
        f"{json.dumps(sv.bloqueios())}",
        "PASSOU"
        if '"nome":"LIMITE_EXCEDIDO"' in r_estoura
        and len(rastro) == 1
        and rastro[0].get("ip") == "127.0.0.1"
        and str(TETO_DO_REGISTRO) in (rastro[0].get("erro") or "")
        and "lidos" in (rastro[0].get("erro") or "")
        and not sv.bloqueios()
        else "FALHOU",
    )

    r = sv.fala({"op": "xyzzy"})[0]
    caso(
        "4c",
        "campo `op` que nao existe",
        "NAO_ENCONTRADO 3001 nomeando a operacao, sem bloquear (nao e violacao)",
        f"{corte(r, 170)}\nbloqueios: {json.dumps(sv.bloqueios())}",
        "PASSOU" if '"codigo":3001' in r and not sv.bloqueios() else "FALHOU",
    )


def parte_4b2_teto_conta_violacao(sv_teto):
    """O interruptor `seguranca.contar_linha_acima_do_teto`, que nasce desligado.

    Ele mora num servidor PROPRIO porque `config_gravar` nao escreve a secao
    `seguranca` -- e nao escreve de proposito: uma sessao roubada nao abre o
    firewall pela tela.
    """
    print("\n=== 4b-ii. A linha acima do teto contando como violacao (opt-in)\n")
    estoura = b'{"op":"ping","token":"t","enchimento":"' + b"A" * (TETO_DO_REGISTRO + 1024) + b'"}\n'
    try:
        r = sv_teto.fala(None, cru=estoura, prazo=120)[0]
    except OSError as e:
        r = f"<{type(e).__name__}: {e}>"
    time.sleep(0.3)
    bloqueios = sv_teto.bloqueios()
    depois = sv_teto.fala({"op": "ping"})[0]
    caso(
        "4b-ii",
        "com `seguranca.contar_linha_acima_do_teto` LIGADO, a linha gigante bloqueia o IP",
        "a recusa e a mesma; o que muda e o IP entrar na blacklist pela politica "
        "leve que ja existe -- e o `ping` seguinte ser recusado",
        f"resposta a linha gigante: {corte(r, 220)}\n"
        f"bloqueios: {json.dumps(bloqueios)}\n"
        f"o `ping` seguinte: {corte(depois, 160)}",
        "PASSOU"
        if bloqueios
        and bloqueios[0].get("ip") == "127.0.0.1"
        and "teto" in bloqueios[0].get("motivo", "")
        else "FALHOU",
    )
    sv_teto.desbloquear()


def semear(sv, token="t", login=None, unico=True):
    """Cria loja.clientes com uma linha. Uma conexao so, porque o login mora nela."""
    pedidos = []
    if login:
        pedidos.append({"op": "login", "usuario": login[0], "senha": login[1]})
    pedidos += [
        {"op": "criar_database", "database": "loja"},
        {
            "op": "criar_tabela",
            "database": "loja",
            "tabela": "clientes",
            "colunas": [
                {"nome": "id", "tipo": "Int8"},
                {"nome": "nome", "tipo": "Str(60)", "obrigatoria": True},
                {"nome": "cidade", "tipo": "Str(40)"},
            ],
            "indices": [{"nome": "porId", "colunas": ["id"], "unico": unico}],
        },
        {
            "op": "inserir",
            "database": "loja",
            "tabela": "clientes",
            "linha": {"id": 1, "nome": "Alves", "cidade": "Blumenau"},
        },
    ]
    return sv.fala(*pedidos, token=token)


def parte_4d_replicacao(sv, sv_fonte, sv_rep):
    print("\n=== 4d. As operacoes de replicacao pedidas por quem nao e replica\n")

    semear(sv)
    r = sv.json_de({"op": "replicar", "database": "loja", "tabela": "clientes",
                    "desde": 0, "max": 5})[0]
    ev = (r.get("resultado") or {}).get("eventos") or []
    # O aviso do pedido 214(b): a lista vazia continua liberando (comportamento
    # velho, e ha teste que trava isso), e o que mudou e o SILENCIO.
    cfg = sv.json_de({"op": "config"})[0]
    aberta = ((cfg.get("resultado") or {}).get("config") or {}).get("replicacao_aberta") \
        or (cfg.get("resultado") or {}).get("replicacao_aberta")
    arranque = [l for l in open(sv.dir + "/saida.log", errors="replace").read().splitlines()
                if "replicas_autorizadas" in l]
    caso(
        "4d-i",
        "`replicar` num servidor de FABRICA (replicas_autorizadas vazia, imagem desligada)",
        "a lista vazia CONTINUA liberando -- e agora o servidor diz isso no arranque "
        "e na resposta de `config`",
        f"eventos devolvidos: {len(ev)}\n"
        f"{corte(json.dumps(ev[:1], ensure_ascii=False), 200)}\n"
        f"aviso no arranque: {corte(arranque[0] if arranque else '<NENHUM>', 220)}\n"
        f"campo `replicacao_aberta` do `config`: {json.dumps(aberta, ensure_ascii=False)}",
        "PASSOU" if ev and arranque and aberta and aberta.get("aberta") else "FALHOU",
    )

    # Sem indice UNICO de proposito: com ele, aplicar a mesma imagem de volta
    # esbarraria na chave repetida e a bateria mediria a unicidade, nao o portao.
    semear(sv_fonte, unico=False)
    r = sv_fonte.json_de({"op": "replicar", "database": "loja", "tabela": "clientes",
                          "desde": 0, "max": 5})[0]
    ev = (r.get("resultado") or {}).get("eventos") or []
    imagem = ev[0].get("imagem", "") if ev else ""
    # Com a imagem na mao, `aplicar` grava ela de volta -- e `aplicar` NAO esta
    # em OPS_ESCRITA, entao nem o somente-leitura o barra.
    trancar = sv_fonte.fala({"op": "config_gravar", "campos": {"somente_leitura": True}})[0]
    barrado = sv_fonte.fala({"op": "inserir", "database": "loja", "tabela": "clientes",
                             "linha": {"id": 2, "nome": "Nao entra"}})[0]
    aplicou = sv_fonte.fala(
        {"op": "aplicar", "database": "loja", "tabela": "clientes",
         # rowid 2 e o proximo da tabela: com um numero fora de ordem o motor
         # grava a linha do mesmo jeito e AINDA acusa divergencia de replica.
         "eventos": [{"operacao": "inclusao", "rowid": 2, "imagem": imagem}]}
    )[0]
    contagem = sv_fonte.json_de({"op": "varrer", "database": "loja",
                                 "tabela": "clientes", "max": 10})[0]
    linhas = (contagem.get("resultado") or {}).get("linhas") or []
    caso(
        "4d-ii",
        "num Source trancado, o `aplicar` grava a linha que o `replicar` entregou?",
        "`replicar` continua devolvendo a linha em hexadecimal (a lista vazia libera), "
        "mas o `aplicar` RECUSA: o portao 2b-bis so deixa passar quem tem papel de replica",
        f"imagem do evento 1: {imagem[:64]}… ({len(imagem)} caracteres hex)\n"
        f"config_gravar somente_leitura=true: {corte(trancar, 130)}\n"
        f"CONTROLE POSITIVO, `inserir` com o servidor trancado: {corte(barrado, 150)}\n"
        f"`aplicar` com a mesma imagem:  {corte(aplicou, 260)}\n"
        f"linhas na tabela depois: {len(linhas)} -> {corte(json.dumps(linhas, ensure_ascii=False), 200)}",
        "PASSOU" if '"ok":false' in aplicou and len(linhas) == 1 else "FALHOU",
    )

    # E o portao que existe para isso, quando alguem o preenche.
    resp = sv_rep.fala(
        {"op": "login", "usuario": "root", "senha": SENHA},
        {"op": "replicar", "database": "loja", "tabela": "clientes"},
        {"op": "posicao", "database": "loja"},
        {"op": "aplicar", "database": "loja", "tabela": "clientes", "eventos": []},
        {"op": "ping"},
    )
    caso(
        "4d-iii",
        "as tres com `replicas_autorizadas` PREENCHIDA com OUTRO IP",
        "as tres recusadas pelo portao 2a-bis, mesmo com login de supervisor; "
        "o `ping`, que nao e replicacao, continua passando",
        f"login root: {corte(resp[0], 110)}\n"
        f"replicar:   {corte(resp[1], 160)}\n"
        f"posicao:    {corte(resp[2], 160)}\n"
        f"aplicar:    {corte(resp[3], 160)}\n"
        f"CONTROLE POSITIVO ping: {corte(resp[4], 110)}",
        "PASSOU"
        if all("replicas_autorizadas" in x for x in resp[1:4]) and '"ok":true' in resp[4]
        else "FALHOU",
    )
    sv_rep.desbloquear()


def parte_4e_usuario(sv_rep):
    print("\n=== 4e. Com cadastro, o token sozinho nao basta\n")
    semear(sv_rep, login=("root", SENHA))
    sem_login = sv_rep.fala(
        {"op": "ping"},
        {"op": "varrer", "database": "loja", "tabela": "clientes", "max": 1},
    )
    respostas = sv_rep.fala(
        {"op": "login", "usuario": "carlos", "senha": "senha-errada"},
        {"op": "login", "usuario": "carlos", "senha": SENHA},
        {"op": "varrer", "database": "loja", "tabela": "clientes", "max": 1},
        {"op": "replicar", "database": "loja", "tabela": "clientes"},
        {"op": "inserir", "database": "loja", "tabela": "clientes",
         "linha": {"id": 2, "nome": "B"}},
    )
    caso(
        "4e",
        "login obrigatorio havendo cadastro, e a permissao por base",
        "`ping` passa sem login (nao toca dado); `varrer` nao. Senha errada recusa. "
        "carlos LE (controle positivo) e nao replica nem insere",
        f"ping sem login:   {corte(sem_login[0], 110)}\n"
        f"varrer sem login: {corte(sem_login[1], 160)}\n"
        f"senha errada:     {corte(respostas[0], 160)}\n"
        f"senha certa:      {corte(respostas[1], 130)}\n"
        f"CONTROLE POSITIVO varrer: {corte(respostas[2], 130)}\n"
        f"replicar:         {corte(respostas[3], 160)}\n"
        f"inserir:          {corte(respostas[4], 160)}",
        "PASSOU"
        if '"ok":true' in sem_login[0]
        and '"ok":false' in sem_login[1]
        and '"ok":false' in respostas[0]
        and '"ok":true' in respostas[1]
        and '"ok":true' in respostas[2]
        and '"ok":false' in respostas[3]
        and '"ok":false' in respostas[4]
        else "FALHOU",
    )
    return respostas + sem_login


def parte_5_conexao_abandonada(sv):
    print("\n=== 5. Conexao aberta e abandonada: a reserva de carga solta?\n")
    s1 = socket.create_connection(("127.0.0.1", sv.porta), 10)
    f1 = s1.makefile("rwb")
    f1.write(
        (json.dumps({"op": "bulkinsert", "token": "t", "database": "loja",
                     "tabela": "clientes", "ligado": True}) + "\n").encode()
    )
    f1.flush()
    reserva = f1.readline().decode().strip()

    s2 = socket.create_connection(("127.0.0.1", sv.porta), 10)
    f2 = s2.makefile("rwb")

    def por_f2(pedido):
        f2.write((json.dumps(dict(pedido, token="t")) + "\n").encode())
        f2.flush()
        return f2.readline().decode().strip()

    barrado = por_f2({"op": "inserir", "database": "loja", "tabela": "clientes",
                      "linha": {"id": 77, "nome": "Barrado"}})
    # Os DOIS: o makefile segura o fd, e sem isso o servidor nao ve o fim.
    f1.close()
    s1.close()
    time.sleep(0.5)
    solto = por_f2({"op": "inserir", "database": "loja", "tabela": "clientes",
                    "linha": {"id": 78, "nome": "Depois"}})
    f2.close()
    s2.close()
    caso(
        5,
        "conexao com reserva de carga cai -> a reserva solta",
        "antes de cair, outra conexao leva EM_CARGA 4002 (controle positivo); "
        "depois de cair, ela grava",
        f"reserva:          {corte(reserva, 160)}\n"
        f"CONTROLE POSITIVO antes: {corte(barrado, 170)}\n"
        f"depois da queda:  {corte(solto, 150)}",
        "PASSOU" if '"codigo":4002' in barrado and '"ok":true' in solto else "FALHOU",
    )


def parte_6_log(sv):
    print("\n=== 6. O que o acessos.log registra de fato\n")
    linhas = sv.acessos()
    campos = set()
    for a in linhas:
        campos |= set(a.keys())
    ok = [a for a in linhas if a["ok"]]
    falha = [a for a in linhas if not a["ok"]]
    caso(
        6,
        "o acessos.log traz IP, data, hora e o SUCESSO/FALHA de cada tentativa",
        "toda conexao entra, inclusive as recusadas, com ip/quando/op/ok",
        f"linhas: {len(linhas)}   ok:true {len(ok)}   ok:false {len(falha)}\n"
        f"campos vistos: {sorted(campos)}\n"
        f"uma de SUCESSO: {corte(json.dumps(ok[-1], ensure_ascii=False), 220)}\n"
        f"uma de FALHA:   {corte(json.dumps(falha[-1], ensure_ascii=False), 240)}",
        "PASSOU"
        if ok and falha and {"ip", "quando", "op", "ok"} <= campos
        else "FALHOU",
    )


def parte_7_senha(sv_rep, respostas):
    print("\n=== 7. A senha nunca aparece -- nem no log, nem na resposta\n")
    achados = []
    for caminho in glob.glob(f"{BASE}/**/*", recursive=True):
        if not os.path.isfile(caminho):
            continue
        try:
            with open(caminho, "rb") as f:
                if SENHA.encode() in f.read():
                    achados.append(caminho)
        except OSError:
            pass
    nas_respostas = [r for r in respostas if SENHA in r]
    caso(
        7,
        "grep da senha em claro no diretorio inteiro e nas respostas",
        "zero ocorrencias: nem no acessos.log, nem no config.json, nem na resposta do login",
        f"arquivos varridos: {sum(1 for c in glob.glob(f'{BASE}/**/*', recursive=True) if os.path.isfile(c))}\n"
        f"arquivos com a senha em claro: {achados or 'NENHUM'}\n"
        f"respostas do protocolo com a senha: {len(nas_respostas)}\n"
        f"o config.json guarda: {json.load(open(sv_rep.cfg))['usuarios'][0]['senha_hash'][:46]}…",
        "PASSOU" if not achados and not nas_respostas else "FALHOU",
    )


def parte_8_portas(sv, sv_wl, sv_fonte, sv_rep):
    print("\n=== 8. Que portas este processo tem abertas\n")
    faltando = [t for t in ("nmap", "ss", "netstat") if not shutil.which(t)]
    fechada = "conectou (NAO devia)"
    try:
        socket.create_connection(("127.0.0.1", PORTA_FECHADA), 1).close()
    except OSError as e:
        fechada = f"recusada: {type(e).__name__}: {e}"
    caso(
        8,
        "so as portas configuradas estao abertas",
        "cada servidor escuta UMA porta, a que o config.json diz, e so em 127.0.0.1",
        f"ferramentas ausentes nesta maquina: {faltando} -- a lista sai de /proc/net/tcp + /proc/<pid>/fd\n"
        f"servidor {sv.nome} (pid {sv.p.pid}):     {sv.portas_escutando()}\n"
        f"servidor {sv_wl.nome} (pid {sv_wl.p.pid}): {sv_wl.portas_escutando()}\n"
        f"servidor {sv_fonte.nome} (pid {sv_fonte.p.pid}):      {sv_fonte.portas_escutando()}\n"
        f"servidor {sv_rep.nome} (pid {sv_rep.p.pid}): {sv_rep.portas_escutando()}\n"
        f"/proc/net/tcp6 existe? {os.path.exists('/proc/net/tcp6')} (sem IPv6 neste conteiner)\n"
        f"porta {PORTA_FECHADA}, que nenhum config abriu: {fechada}",
        "PASSOU"
        if sv.portas_escutando() == [f"127.0.0.1:{sv.porta}"]
        and "recusada" in fechada
        else "FALHOU",
    )


def main():
    print("=" * 78)
    print("BATERIA DE ACESSO E SEGURANCA DA PORTA TCP/IP -- PhxSql")
    print(f"UTC {time.strftime('%Y-%m-%d %H:%M:%S', time.gmtime())}   "
          f"commit {subprocess.run(['git', '-C', RAIZ, 'rev-parse', '--short', 'HEAD'], capture_output=True).stdout.decode().strip()}")
    print(f"portas {PORTA}, {PORTA_WL}, {PORTA_REP}, {PORTA_FONTE}, {PORTA_TETO}   base {BASE}")
    print("=" * 78)
    shutil.rmtree(BASE, ignore_errors=True)
    try:
        with Servidor("padrao", PORTA) as sv, \
             Servidor("whitelist", PORTA_WL, seguranca={"whitelist": ["127.0.0.1"]}) as sv_wl, \
             Servidor(
                 "fonte",
                 PORTA_FONTE,
                 replicacao={"papel": "source", "id_servidor": "f6-fonte",
                             "imagem_da_linha": True},
             ) as sv_fonte, \
             Servidor(
                 "replica",
                 PORTA_REP,
                 replicacao={"papel": "source", "id_servidor": "f6",
                             "replicas_autorizadas": ["203.0.113.9"]},
                 usuarios=True,
             ) as sv_rep, \
             Servidor(
                 "teto",
                 PORTA_TETO,
                 seguranca={"contar_linha_acima_do_teto": True,
                            "tentativas_ate_bloquear": 1},
             ) as sv_teto:
            parte_1_o_token(sv)
            parte_3_whitelist(sv_wl)
            parte_4_pedidos_torcidos(sv)
            parte_4b2_teto_conta_violacao(sv_teto)
            parte_4d_replicacao(sv, sv_fonte, sv_rep)
            respostas = parte_4e_usuario(sv_rep)
            parte_5_conexao_abandonada(sv)
            parte_6_log(sv)
            parte_7_senha(sv_rep, respostas)
            parte_8_portas(sv, sv_wl, sv_fonte, sv_rep)
    finally:
        pass

    print("\n" + "=" * 78)
    passou = sum(1 for _, _, v in PLACAR if v == "PASSOU")
    falhou = sum(1 for _, _, v in PLACAR if v == "FALHOU")
    achado = sum(1 for _, _, v in PLACAR if v == "ACHADO")
    print(f"PLACAR: {len(PLACAR)} casos -- {passou} passou, {falhou} falhou, {achado} achado")
    for n, t, v in PLACAR:
        print(f"  [{n:>3}] {v:7} {t}")
    print("=" * 78)
    shutil.rmtree(BASE, ignore_errors=True)
    return 1 if falhou else 0


if __name__ == "__main__":
    sys.exit(main())
