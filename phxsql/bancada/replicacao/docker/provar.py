#!/usr/bin/env python3
"""Os quatro modos de replicacao do PhxSql, provados em conteineres.

    cargo build --release --target x86_64-unknown-linux-musl --bin phxsqld
    python3 bancada/replicacao/docker/provar.py

A bancada de processos (`bancada/replicacao/montar.py` e `modos.py`) ja prova
que os quatro modos FUNCIONAM. Ela nao prova -- e nao tem como provar -- tres
coisas, e sao elas a razao de esta bancada existir:

1. **ENDERECO.** Com tudo em `127.0.0.1` o `bind` do source e o endereco que a
   replica procura sao o mesmo por acidente. Em conteineres nao sao: o `bind`
   tem de ser `0.0.0.0` e a origem e um NOME de servico. O estagio (0) repoe o
   defeito de proposito e mede o silencio dele.
2. **FIREWALL e ISOLAMENTO.** A secao 7 do REPLICACAO.md desenha um source que
   aceita entrada so do IP da replica e nao alcanca ninguem. No loopback nao ha
   o que trancar. Aqui ha: rede propria, IP fixo, um intruso com a
   configuracao vazada, e `iptables` de verdade dentro do namespace do source.
3. **QUEDA E PARTICAO.** `docker kill` mata o processo sem chance de fechar
   arquivo, e `docker network disconnect` corta o fio SEM matar ninguem -- os
   dois lados continuam vivos e aceitando escrita. E a licao do BULKINSERT
   levada um degrau adiante: teste unitario nao prova queda de conexao,
   soquete prova, e conteiner prova melhor.

Estagios, cada um com o RESULTADO ESPERADO escrito antes de rodar:

    0  endereco: `bind` em 127.0.0.1 dentro do conteiner nao alcanca ninguem
    a  modo A (Primary -> Replica): convergencia, atraso e vazao, com a MESMA
       carga medida tambem em processos, para a comparacao ser de trabalho
       igual e nao so de pergunta igual
    a2 queda do no: `docker kill` na replica, escrita no source, volta
    a3 congelamento: corte SILENCIOSO com o source escrevendo -- `ping` passa
       e `varrer` espera a trava que o laco da replicacao segura atravessando
       uma ida e volta de rede
    b  modo B (Multi-Master): laco morto, conflito, a PARTICAO -- a prova de
       que a identidade e a chave e nao o rowid --, os dois tipos de corte
       cronometrados, e o ABRACO: os dois lados se trancando um ao outro com
       a rede sa
    c  modo C (Spare): recusa de tudo, morte do primario, promocao
    d  modo D (Read Replica): leitura passa, escrita recusa apontando o
       primario -- e o endereco que ela aponta e o achado
    e  firewall da secao 7: intruso, `replicas_autorizadas`, `ips_permitidos`
       e `iptables` no namespace do source

A ultima linha e `RESULTADO <json>`, e o arquivo `resultados.json` fica ao lado
deste script. Portas do hospedeiro: 6801-6853.

Esta bancada so mexe nos conteineres que ela mesma cria (prefixo `phxrep-`) e
nos processos que ela mesma sobe (guarda os `Popen`). Nunca `pkill`.
"""
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", "..", ".."))

# Os tetos do estagio (a3) saem da bancada irma de loopback, e nao daqui: as
# duas medem o MESMO defeito por caminhos diferentes, e numero copiado e
# numero que envelhece de um lado so.
sys.path.insert(0, os.path.dirname(AQUI))
from trava import TETO_PING_MS, TETO_VARRER_MS  # noqa: E402
ALVO_MUSL = os.path.join(
    RAIZ, "target", "x86_64-unknown-linux-musl", "release", "phxsqld"
)
IMAGEM = os.environ.get("PHX_IMAGEM", "phxsql-bancada:local")
BASE = os.environ.get("PHX_BASE", "/tmp/phx-docker-replicacao")

TOKEN = "espelho"
USUARIO = "adm"
SENHA = "segredo1"
TABELA = "clientes"

RESULTADO = {}
FALHAS = []
PROCESSOS = []          # (Popen, rotulo) -- so os que ESTE script subiu
SOQUETES = []           # fechados do lado do cliente antes de derrubar
PIOR_MS = {}            # por porta: a resposta mais demorada ja vista
PIOR_OP = {}            # e qual operacao foi
RECONEXOES = []         # (porta, op) de cada conexao que teve de ser refeita


# --------------------------------------------------------------- utilidades

def sh(*args, checar=True, entrada=None):
    r = subprocess.run(args, capture_output=True, text=True, input=entrada)
    if checar and r.returncode != 0:
        raise SystemExit(f"falhou: {' '.join(args)}\n{r.stdout}\n{r.stderr}")
    return r.stdout.strip()


def hash_da_senha():
    """O hash sai do PROPRIO servidor -- nao ha uma segunda implementacao."""
    saida = sh(ALVO_MUSL, "--senha", entrada=SENHA + "\n")
    return saida.split('": "')[1].split('"')[0]


def permissoes():
    return {"*": {"ler": True, "inserir": True, "alterar": True,
                  "excluir": True, "criar": True, "administrar": True,
                  "diario": True, "verificar": True, "replicar": True}}


def config(h, papel, id_servidor, origens=None, somente_leitura=False,
           bind="0.0.0.0:5000", ips_permitidos=None, replicas_autorizadas=None,
           imagem_da_linha=True):
    rep = {"papel": papel, "id_servidor": id_servidor,
           "imagem_da_linha": imagem_da_linha}
    if origens:
        rep["origens"] = origens
    if replicas_autorizadas is not None:
        rep["replicas_autorizadas"] = replicas_autorizadas
    c = {
        "base": "base",
        "bind": bind,
        "token": TOKEN,
        "web": {"ligado": False},
        "replicacao": rep,
        "usuarios": [{"login": USUARIO, "nome": "Bancada", "id": 10,
                      "senha_hash": h, "bases": permissoes()}],
    }
    if somente_leitura:
        c["somente_leitura"] = True
    if ips_permitidos is not None:
        c["ips_permitidos"] = ips_permitidos
    return c


def origem(nome, host, porta=5000, h=None, reconectar_em=2, extras=None):
    o = {"nome": nome, "host": host, "porta": porta, "token": TOKEN,
         "usuario": USUARIO, "senha_hash": h, "databases": ["loja"],
         "reconectar_em": reconectar_em}
    o.update(extras or {})
    return o


def escrever_config(modo, no, cfg):
    d = os.path.join(BASE, modo, no)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    return d


def limpar_dados(modo, no):
    """Apaga os DADOS e a memoria de acesso -- o `config.json` fica.

    `blacklist.json` entra na lista por uma medicao que quase virou numero
    errado: o estagio (e) bate na porta do source com um intruso, e depois de
    umas dezenas de recusas o servidor **bloqueia o IP** -- que e exatamente o
    que ele deve fazer. So que o arquivo mora no volume e sobrevive a corrida
    seguinte, e ali a fase 1, que e a fase SEM tranca nenhuma, media zero
    evento roubado. O numero estava certo e a conclusao seria errada: quem
    barrou nao era a fase 1, era a lista negra da corrida anterior.
    """
    d = os.path.join(BASE, modo, no)
    for lixo in ["base", "replicacao-posicoes.json", "blacklist.json",
                 "acessos.log", "servidor.log"]:
        caminho = os.path.join(d, lixo)
        if os.path.isdir(caminho):
            shutil.rmtree(caminho, ignore_errors=True)
        elif os.path.exists(caminho):
            os.remove(caminho)


# ------------------------------------------------------- cliente do protocolo

class Ligacao:
    """Uma conexao ao servidor que se refaz sozinha.

    Existe por uma medicao desta bancada, e ela vale a pena escrever: o
    servidor **fecha a conexao ociosa** depois de `timeout_s` (30 s por
    padrao). O laco de atendimento tem `Err(_) => return`, e o prazo de
    leitura conta como erro -- entao ficar calado meio minuto e o mesmo que
    desligar. Isso e razoavel para um servidor, e e mortal para uma bancada
    que mede corte de rede de 45 s: a primeira versao morria com `broken
    pipe` DEPOIS do corte, e o corte parecia nao ter se recuperado quando na
    verdade quem tinha ido embora era o cliente. Um teste que falha por
    engano e tao ruim quanto um que passa por engano.

    Refazer a conexao e repetir o pedido uma vez e seguro aqui: quando o
    servidor ja fechou, o pedido nao chegou a rodar -- o `readline` volta
    vazio ou o `write` leva RST, e nos dois casos nada foi aplicado. Se o
    servidor estiver mesmo fora do ar, a segunda tentativa falha igual e a
    bancada ve a falha de verdade.
    """

    def __init__(self, host, porta):
        self.host, self.porta = host, porta
        self.s = self.f = None

    def abrir(self, prazo=180):
        # Prazo LARGO de proposito: com um prazo curto, um `read` que
        # estourasse deixava o soquete inutilizavel para sempre (`cannot read
        # from timed out object`) e a bancada morria sem dizer quanto tempo o
        # servidor tinha demorado. Com prazo largo e o `PIOR_MS`, a demora
        # vira numero em vez de acidente.
        self.s = socket.create_connection((self.host, self.porta), timeout=prazo)
        self.f = self.s.makefile("rwb")
        SOQUETES.append((self.s, self.f))
        r = self.cru({"op": "login", "usuario": USUARIO, "senha": SENHA})
        if not r.get("ok"):
            raise SystemExit(f"login na porta {self.porta}: {r}")

    def cru(self, pedido):
        pedido.setdefault("token", TOKEN)
        t0 = time.perf_counter()
        self.f.write((json.dumps(pedido) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        ms = (time.perf_counter() - t0) * 1000
        if ms > PIOR_MS.get(self.porta, 0):
            PIOR_MS[self.porta] = ms
            PIOR_OP[self.porta] = pedido.get("op", "?")
        if not linha:
            raise OSError("o servidor fechou a conexao")
        return json.loads(linha.decode())

    def __call__(self, pedido):
        try:
            return self.cru(dict(pedido))
        except OSError:
            RECONEXOES.append((self.porta, pedido.get("op", "?")))
            self.abrir()
            return self.cru(dict(pedido))


def liga(porta, tentativas=150, host="127.0.0.1"):
    ultimo = None
    for _ in range(tentativas):
        try:
            l = Ligacao(host, porta)
            l.abrir()
            return l
        except OSError as e:
            ultimo = e
            time.sleep(0.2)
    raise SystemExit(f"porta {porta} nunca respondeu: {ultimo}")


def tenta_ligar(porta, segundos=6, host="127.0.0.1"):
    """Como `liga`, mas devolve None em vez de morrer -- para provar ausencia."""
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < segundos:
        try:
            l = Ligacao(host, porta)
            l.abrir(prazo=2)
            return l
        except (OSError, SystemExit):
            time.sleep(0.3)
    return None


def fechar_soquetes():
    """Quem fecha primeiro fica com o TIME_WAIT, e ele tem de ficar do lado do
    cliente -- senao a porta do servidor nao volta a aceitar `bind`."""
    for s, f in SOQUETES:
        try:
            f.close()
            s.close()
        except OSError:
            pass
    SOQUETES.clear()


def eventos(fala, tabela=TABELA):
    r = fala({"op": "posicao", "database": "loja"})
    if not r.get("ok"):
        return 0
    t = r["resultado"]["tabelas"].get(tabela)
    return 0 if t is None else t["eventos"]


def soma(fala, tabela=TABELA):
    """A soma de verificacao do PROPRIO servidor: conteudo, linhas e slots.

    `slots` importa tanto quanto a soma: o `.reg` nunca reaproveita slot e o
    rowid e sempre `slots + 1`, entao dois lados com o mesmo numero de slots
    chegaram a mesma numeracao sozinhos.
    """
    r = fala({"op": "checksum", "database": "loja", "tabela": tabela})
    if not r.get("ok"):
        return None
    d = r["resultado"]
    return (d["checksum"], d["linhas"], d["slots"])


def retrato(fala, tabela=TABELA):
    """SHA-256 de cada linha INTEIRA, com rowid e rownum, lido pelo cursor.

    Contar linhas nao prova nada; a soma do servidor nao inclui o rowid. Este
    retrato inclui, e o rowid e justamente o que a replicacao promete
    reproduzir sem transmitir.
    """
    h = hashlib.sha256()
    linhas, depois = 0, 0
    while True:
        r = fala({"op": "varrer", "database": "loja", "tabela": tabela,
                  "max": 2000, "depois": depois, "visao": "todas"})
        if not r.get("ok"):
            return 0, "(sem tabela)"
        d = r["resultado"]
        for l in d["linhas"]:
            h.update(json.dumps(l, sort_keys=True, ensure_ascii=False).encode())
            linhas += 1
        if not d["ha_mais"] or not d["linhas"]:
            break
        depois = d["cursor_fim"]
    return linhas, h.hexdigest()[:16]


def retrato_por_chave(fala, tabela=TABELA):
    """SHA-256 do conteudo ORDENADO PELA CHAVE, sem rowid e sem rownum.

    Existe porque a soma do servidor (`checksum`) e ORDENADA -- ela multiplica
    antes de somar justamente para que trocar duas linhas de lugar mude o
    resultado. Isso e o que se quer no modo A, onde a replica reproduz a ordem
    de digitacao do source. No modo B e o contrario: cada servidor mantem a
    SUA ordem de chegada (a ordem de digitacao e sagrada em cada um), entao
    dois pares bidirecionais convergidos tem, de proposito, somas DIFERENTES.

    Comparar modo B pela soma do servidor da divergencia onde nao ha nenhuma.
    A comparacao certa la e esta: o conteudo, casado pela chave.
    """
    linhas, depois = [], 0
    while True:
        r = fala({"op": "varrer", "database": "loja", "tabela": tabela,
                  "max": 2000, "depois": depois, "visao": "todas"})
        if not r.get("ok"):
            return 0, "(sem tabela)"
        d = r["resultado"]
        for l in d["linhas"]:
            dados = {k: v for k, v in l.items() if k not in ("rowid", "rownum")}
            linhas.append(json.dumps(dados, sort_keys=True, ensure_ascii=False))
        if not d["ha_mais"] or not d["linhas"]:
            break
        depois = d["cursor_fim"]
    linhas.sort()
    h = hashlib.sha256()
    for l in linhas:
        h.update(l.encode())
    return len(linhas), h.hexdigest()[:16]


def esperar(condicao, segundos=60, passo=0.05):
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < segundos:
        try:
            if condicao():
                return time.perf_counter() - t0
        except OSError:
            pass
        time.sleep(passo)
    return None


def criar_tabela(fala, tabela=TABELA, com_chave=True, memo=True):
    fala({"op": "criar_database", "database": "loja"})
    colunas = [{"nome": "id", "tipo": "Int4", "obrigatoria": True},
               {"nome": "nome", "tipo": "Str(40)", "obrigatoria": True},
               {"nome": "cidade", "tipo": "Str(30)"},
               {"nome": "limite", "tipo": "Decimal(12,2)"}]
    if memo:
        colunas.append({"nome": "ficha", "tipo": "Memo"})
    pedido = {"op": "criar_tabela", "database": "loja", "tabela": tabela,
              "motivo_obrigatorio": False, "colunas": colunas}
    if com_chave:
        pedido["indices"] = [{"nome": "porId", "colunas": ["id"],
                              "unico": True, "primario": True}]
    r = fala(pedido)
    if not r.get("ok"):
        raise SystemExit(f"criar {tabela}: {r}")


def inserir(fala, id_, nome, cidade="Blumenau", tabela=TABELA):
    return fala({"op": "inserir", "database": "loja", "tabela": tabela,
                 "linha": {"id": id_, "nome": nome, "cidade": cidade,
                           "limite": "1.00"}})


def por_chave(fala, tabela=TABELA):
    """{id: (nome, rowid)} -- a leitura que casa pela CHAVE, nao pelo rowid."""
    saida, depois = {}, 0
    while True:
        r = fala({"op": "varrer", "database": "loja", "tabela": tabela,
                  "max": 1000, "depois": depois, "visao": "todas"})
        if not r.get("ok"):
            return {}
        d = r["resultado"]
        for l in d["linhas"]:
            saida[l["id"]] = (l["nome"], l["rowid"])
        if not d["ha_mais"] or not d["linhas"]:
            return saida
        depois = d["cursor_fim"]


# ------------------------------------------------------------ palco: docker

def compose(arquivo, *args, checar=True):
    amb = dict(os.environ, PHX_BASE=BASE, PHX_IMAGEM=IMAGEM)
    cmd = ["docker", "compose", "-f", os.path.join(AQUI, arquivo), *args]
    r = subprocess.run(cmd, capture_output=True, text=True, env=amb)
    if checar and r.returncode != 0:
        raise SystemExit(f"falhou: {' '.join(cmd)}\n{r.stdout}\n{r.stderr}")
    return r.stdout.strip()


def ip_de(container):
    f = "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}"
    return sh("docker", "inspect", "-f", f, container)


def pid_de(container):
    return sh("docker", "inspect", "-f", "{{.State.Pid}}", container)


def na_rede_do(container, *cmd):
    """Roda um comando do HOSPEDEIRO dentro do namespace de rede do conteiner.

    A imagem e `scratch`: nao tem shell, nao tem `nc`, nao tem `iptables`.
    Entrar no namespace com `nsenter` da o `iptables` e o `nc` da maquina
    dentro da rede do conteiner sem sujar a imagem com nada.
    """
    return subprocess.run(["nsenter", "-t", pid_de(container), "-n", *cmd],
                          capture_output=True, text=True, timeout=30)


def cortar(container, ip_do_outro, como):
    """Parte a rede entre dois nos VIVOS, nos dois sentidos.

    Por que `iptables` e nao `docker network disconnect`: o disconnect leva
    junto a porta publicada no hospedeiro, e a bancada fica cega justamente no
    momento em que precisa olhar os dois lados. A primeira versao deste estagio
    morreu com «o servidor fechou a conexao» ao tentar ler o no desligado.

    `como` distingue os dois cortes que o mundo tem, e eles NAO sao o mesmo:

      DROP    o pacote some. E o cabo cortado, a regra de firewall, a rota
              que evaporou. Quem chama fica pendurado ate o nucleo desistir.
      REJECT  o pacote leva um RST de volta. E o processo morto, a porta
              fechada. Quem chama sabe na hora.
    """
    regra = ["-j", "DROP"] if como == "DROP" else \
        ["-j", "REJECT", "--reject-with", "tcp-reset"]
    na_rede_do(container, "iptables", "-I", "INPUT", "-s", ip_do_outro, *regra)
    na_rede_do(container, "iptables", "-I", "OUTPUT", "-d", ip_do_outro, *regra)


def religar(container, ip_do_outro, como):
    regra = ["-j", "DROP"] if como == "DROP" else \
        ["-j", "REJECT", "--reject-with", "tcp-reset"]
    na_rede_do(container, "iptables", "-D", "INPUT", "-s", ip_do_outro, *regra)
    na_rede_do(container, "iptables", "-D", "OUTPUT", "-d", ip_do_outro, *regra)


def alcanca(rede, alvo, porta=5000, segundos=4):
    """Um alpine na rede tenta abrir a porta. Devolve (alcancou, saida).

    A distincao que interessa esta no TEXTO: recusa imediata ("Connection
    refused") quer dizer que o pacote chegou e alguem respondeu nao; silencio
    ate o tempo acabar quer dizer DROP -- o pacote nem foi respondido. Sao
    dois desenhos de seguranca diferentes, e so o segundo esconde que ha algo
    ali.
    """
    t0 = time.perf_counter()
    r = subprocess.run(
        ["docker", "run", "--rm", "--network", rede, "alpine:3",
         "timeout", str(segundos + 2), "nc", "-z", "-v", "-w", str(segundos),
         alvo, str(porta)],
        capture_output=True, text=True)
    ms = round((time.perf_counter() - t0) * 1000)
    texto = (r.stdout + r.stderr).strip().replace("\n", " ")
    return r.returncode == 0, f"saida={r.returncode} em {ms} ms: {texto or '(silencio)'}"


# ---------------------------------------------------------- palco: processos

def subir_processo(modo, no, rotulo):
    d = os.path.join(BASE, modo, no)
    log = open(os.path.join(d, "servidor.log"), "a")
    p = subprocess.Popen([ALVO_MUSL], cwd=d, stdout=log,
                         stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    PROCESSOS.append((p, rotulo))
    return p


def derrubar_processos(*rotulos):
    fechar_soquetes()
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


# ------------------------------------------------------------------ relatorio

def estagio(letra, esperado):
    print()
    print(f"--- estagio ({letra})")
    print(f"    esperado: {esperado}")


def medir(letra, ok, texto, extra=None):
    print(f"    medido:   {texto}  [{'ok' if ok else 'FALHOU'}]")
    RESULTADO[letra] = {"ok": ok, "medido": texto}
    if extra:
        RESULTADO[letra].update(extra)
    if not ok:
        FALHAS.append(letra)


# ------------------------------------------------------------------ estagios

def estagio_0_endereco(h):
    estagio("0-endereco",
            "com `bind: 127.0.0.1:5000` DENTRO do conteiner a replica nao "
            "alcanca o source em 20s; trocando para 0.0.0.0 ela alcanca")
    escrever_config(
        "a", "fonte",
        config(h, "source", "fonte", bind="127.0.0.1:5000"))
    escrever_config(
        "a", "replica",
        config(h, "replica", "replica", somente_leitura=True,
               origens=[origem("fonte", "fonte", h=h, reconectar_em=1)]))
    limpar_dados("a", "fonte")
    limpar_dados("a", "replica")
    compose("compose-a-primary-replica.yml", "up", "-d")

    # O source so responde no loopback DELE, entao nem a porta publicada serve:
    # o `docker-proxy` bate no IP da ponte e leva recusa. E a prova por dentro.
    de_dentro, texto = alcanca("phxrep-a-tunel", "fonte")
    replica = liga(6802)
    parado = esperar(lambda: eventos(replica) > 0, 20)
    ok = (not de_dentro) and parado is None
    medir("0-endereco", ok,
          f"vizinho na mesma rede alcancando fonte:5000 = "
          f"{'ALCANCOU' if de_dentro else 'nao'} ({texto!r}); "
          f"eventos na replica depois de 20s = {eventos(replica)}",
          {"bind_loopback_alcancavel": de_dentro})

    # Agora o conserto, que e uma linha do config.
    fechar_soquetes()
    compose("compose-a-primary-replica.yml", "down", "-v")
    escrever_config("a", "fonte", config(h, "source", "fonte"))
    limpar_dados("a", "fonte")
    limpar_dados("a", "replica")
    compose("compose-a-primary-replica.yml", "up", "-d")
    fonte = liga(6801)
    replica = liga(6802)
    criar_tabela(fonte)
    inserir(fonte, 1, "primeira")
    t = esperar(lambda: eventos(replica) >= 1, 30)
    print(f"    conserto: `bind: 0.0.0.0:5000` -- a replica alcancou em "
          f"{t and round(t, 2)}s")
    RESULTADO["0-endereco"]["com_0_0_0_0_s"] = None if t is None else round(t, 2)
    fechar_soquetes()
    compose("compose-a-primary-replica.yml", "down", "-v")


def carga_modo_a(fonte, replica, n, rotulo):
    """A MESMA carga, o MESMO cliente, o MESMO esquema -- so o transporte muda.

    E a regra 4 da bancada levada para ca: comparar conteiner com processo so
    vale se o trabalho for igual, e a unica forma de garantir isso e ser o
    mesmo codigo. Por isso esta funcao nao sabe se esta falando com um
    conteiner ou com um processo.
    """
    saida = {}
    criar_tabela(fonte)
    cid = ["Blumenau", "Joinville", "Itajai", "Curitiba", "Florianopolis"]
    t0 = time.perf_counter()
    i = 0
    while i < n:
        linhas = [{"id": k, "nome": f"Cliente {k:07d}", "cidade": cid[k % 5],
                   "limite": f"{k}.50",
                   "ficha": f"ficha do cliente {k}, com texto que mora no .memo"}
                  for k in range(i + 1, min(i + 5000, n) + 1)]
        r = fonte({"op": "inserir_lote", "database": "loja", "tabela": TABELA,
                   "linhas": linhas})
        if not r.get("ok", True):
            raise SystemExit(f"carga: {r}")
        i += 5000
    t_carga = time.perf_counter() - t0
    saida["fonte_linhas_s"] = round(n / t_carga)
    print(f"    {rotulo}: {n / t_carga:,.0f} linhas/s no source "
          f"(com a imagem no diario)")

    alvo = eventos(fonte)
    t_alcance = esperar(lambda: eventos(replica) >= alvo, 300)
    saida["alcance_s"] = None if t_alcance is None else round(t_alcance, 2)
    saida["replica_eventos_s"] = (
        None if not t_alcance else round(n / t_alcance))
    print(f"    {rotulo}: a replica alcancou {alvo} eventos em "
          f"{t_alcance and round(t_alcance, 2)}s "
          f"({t_alcance and round(n / t_alcance):,} eventos/s)")

    # Atraso por tipo de escrita -- a mesma lista da bancada de processos.
    base = n + 1

    def lote(qtd, inicio):
        linhas = [{"id": k, "nome": f"Cliente {k:07d}", "cidade": "Blumenau",
                   "limite": f"{k}.99", "ficha": f"ficha {k} " * 5}
                  for k in range(inicio, inicio + qtd)]
        return lambda: fonte({"op": "inserir_lote", "database": "loja",
                              "tabela": TABELA, "linhas": linhas})

    escritas = [
        ("1 insercao", lote(1, base)),
        ("1.000 insercoes em lote", lote(1000, base + 10)),
        ("1 alteracao", lambda: fonte({
            "op": "atualizar", "database": "loja", "tabela": TABELA, "rowid": 7,
            "linha": {"id": 7, "nome": "ALTERADO NO SOURCE",
                      "cidade": "Bruxelas", "limite": "999.99",
                      "ficha": "ficha trocada, maior que a de antes"}})),
        ("1 exclusao suave", lambda: fonte({
            "op": "excluir", "database": "loja", "tabela": TABELA,
            "rowid": 11, "motivo": "prova de replicacao"})),
        ("1 restauracao", lambda: fonte({
            "op": "restaurar", "database": "loja", "tabela": TABELA,
            "rowid": 11, "motivo": "voltou"})),
        ("1 exclusao fisica", lambda: fonte({
            "op": "excluir", "database": "loja", "tabela": TABELA,
            "rowid": 13, "fisico": True, "motivo": "prova fisica"})),
        # O anexo e o caso em que copiar o ponteiro daria bloco errado do
        # outro lado.
        ("1 linha com memo de 200 KB", lambda: fonte({
            "op": "inserir", "database": "loja", "tabela": TABELA,
            "linha": {"id": 9_999_999, "nome": "Com anexo", "cidade": "Curitiba",
                      "limite": "1.00", "ficha": "M" * 200_000}})),
    ]
    atrasos = {}
    for nome, fazer in escritas:
        t0 = time.perf_counter()
        fazer()
        alvo = eventos(fonte)
        pronto = esperar(lambda: eventos(replica) >= alvo, 120)
        ms = None if pronto is None else round((time.perf_counter() - t0) * 1000)
        atrasos[nome] = ms
        print(f"      {nome:<28} {alvo:>8} eventos   {ms:>7} ms")
    saida["atraso_ms"] = atrasos

    # UMA amostra de atraso nao diz nada, e a primeira corrida desta bancada
    # provou: a mesma insercao deu 2.035 ms em conteiner e 53 ms em processo,
    # e a diferenca era so ONDE no ciclo de 2 s do `reconectar_em` a escrita
    # caiu. O atraso normal e uniforme dentro desse ciclo, entao o que se
    # publica e a FAIXA de doze amostras, nao a de uma.
    amostras = []
    for k in range(12):
        chave = 30_000_000 + k
        t0 = time.perf_counter()
        fonte({"op": "inserir", "database": "loja", "tabela": TABELA,
               "linha": {"id": chave, "nome": f"amostra {k}",
                         "cidade": "Itajai", "limite": "1.00", "ficha": "a"}})
        alvo = eventos(fonte)
        if esperar(lambda: eventos(replica) >= alvo, 60) is not None:
            amostras.append(round((time.perf_counter() - t0) * 1000))
    amostras.sort()
    saida["atraso_amostras_ms"] = amostras
    saida["atraso_faixa_ms"] = [amostras[0], amostras[len(amostras) // 2],
                                amostras[-1]] if amostras else None
    print(f"    {rotulo}: atraso de 12 insercoes soltas (min/mediana/max): "
          f"{saida['atraso_faixa_ms']} ms")

    s_fonte, s_replica = soma(fonte), soma(replica)
    r_fonte, r_replica = retrato(fonte), retrato(replica)
    saida["soma_fonte"] = s_fonte
    saida["soma_replica"] = s_replica
    saida["retrato_fonte"] = r_fonte
    saida["retrato_replica"] = r_replica
    saida["convergiu"] = s_fonte == s_replica and r_fonte == r_replica
    print(f"    {rotulo}: soma do servidor  source={s_fonte}  replica={s_replica}")
    print(f"    {rotulo}: retrato SHA-256   source={r_fonte}  replica={r_replica}")
    return saida


def estagio_a(h, n):
    estagio("a", f"modo A em conteiner: as {n} linhas atravessam, a soma de "
                 "verificacao, a contagem de linhas, os slots e o retrato "
                 "SHA-256 batem, e a escrita na replica e recusada")
    escrever_config("a", "fonte", config(h, "source", "fonte"))
    escrever_config(
        "a", "replica",
        config(h, "replica", "replica", somente_leitura=True,
               origens=[origem("fonte", "fonte", h=h, reconectar_em=2)]))
    limpar_dados("a", "fonte")
    limpar_dados("a", "replica")
    compose("compose-a-primary-replica.yml", "up", "-d")
    fonte, replica = liga(6801), liga(6802)

    d = carga_modo_a(fonte, replica, n, "docker")

    gravar = inserir(replica, 999999, "nao entra")
    recusou = (not gravar.get("ok")
               and "somente leitura" in gravar.get("erro", "").lower())
    d["escrita_na_replica"] = f"{gravar.get('nome')}: {gravar.get('erro','')}"
    ok = d["convergiu"] and recusou
    medir("a", ok,
          f"{d['fonte_linhas_s']:,} linhas/s no source; a replica alcancou em "
          f"{d['alcance_s']}s ({d['replica_eventos_s']:,} eventos/s); "
          f"convergiu={d['convergiu']}; escrita na replica: "
          f"{gravar.get('nome')}", d)


def estagio_a3_congelamento():
    """O achado que so um corte SILENCIOSO produz -- e por isso so o conteiner.

    O laco da replica SEGURAVA a trava global de dados (`self.dados.lock()` em
    `alcancar_tabela`, e o mesmo em `alcancar_tabela_bidi`) e, DE DENTRO dela,
    fazia a ida e volta de rede que busca o lote (`replica::puxar`). Numa rede
    sa isso e invisivel: a resposta chega em microssegundos. Com o cabo
    cortado -- pacote que some, e nao porta que recusa -- a leitura ficava
    pendurada ate o prazo de 30 s do cliente da replica, e a trava ficava
    presa junto. Todo pedido de cliente que precisasse da trava esperava atras.

    O pedido 147 partiu as duas em tres fases (abrir e ler a posicao com a
    trava; ler o lote do soquete SEM ela; reabrir, reler e aplicar com ela), e
    a regra que saiu dali e o que este estagio afirma hoje: NENHUMA leitura de
    rede acontece com a trava de dados na mao. O numero desta bancada, medido
    dos dois lados da correcao: pior `varrer` 29.456 ms antes, 7 ms depois.

    `docker stop` nao acha isto: matar o processo devolve RST, o `puxar`
    falha na hora e a trava e solta na hora. Processo no mesmo loopback
    tambem nao acha, porque nao ha o que cortar. E preciso um corte que nao
    responde -- e isso e uma regra de firewall dentro de uma rede propria.
    """
    estagio("a3-congelamento",
            "com o source ESCREVENDO SEM PARAR (a replica fica dentro do "
            "laco de puxar, que e onde a trava ESTAVA presa), um corte "
            "SILENCIOSO: na replica, `ping` continua rapido -- e `varrer`, "
            "que precisa da trava, tambem, porque nenhuma leitura de rede "
            f"acontece com a trava na mao (teto {TETO_VARRER_MS} ms)")
    ip_fonte = ip_de("phxrep-a-fonte")
    replica = liga(6802)

    # A PRIMEIRA versao deste estagio cortou com o source PARADO, e nao
    # congelou nada: pior `ping` 8 ms, pior `varrer` 8 ms em 107.365 amostras.
    # A hipotese nao estava errada -- o cenario e que nao a exercitava. Com o
    # source parado a replica passa a vida no `ligar`, que acontece FORA da
    # trava; ela so entra na trava quando ha evento para puxar. Escrever sem
    # parar poe o laco dentro do `puxar` sob a trava quase o tempo todo, e ai
    # o corte cai onde precisa cair.
    parar = threading.Event()

    def escrevendo():
        f = liga(6801)
        k = 50_000_000
        while not parar.is_set():
            linhas = [{"id": k + i, "nome": f"carga {i}", "cidade": "Itajai",
                       "limite": "1.00", "ficha": "x"} for i in range(1000)]
            k += 1000
            try:
                f({"op": "inserir_lote", "database": "loja", "tabela": TABELA,
                   "linhas": linhas})
            except OSError:
                return

    escritor = threading.Thread(target=escrevendo, daemon=True)
    escritor.start()
    time.sleep(3)          # tempo de a replica entrar no laco produtivo
    cortar("phxrep-a-replica", ip_fonte, "DROP")
    # Mais dois segundos de escrita e entao para: depois do corte a replica
    # nao recebe mais nada, entao a fila dela ja nao diminui -- continuar
    # escrevendo so encheria o disco. O que importava era o laco estar DENTRO
    # do puxar sob a trava no instante do corte.
    time.sleep(2)
    parar.set()
    escritor.join(timeout=30)
    pings, varridas = [], []
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < 43:
        t = time.perf_counter()
        replica({"op": "ping"})
        pings.append((time.perf_counter() - t) * 1000)
        t = time.perf_counter()
        replica({"op": "varrer", "database": "loja", "tabela": TABELA, "max": 1})
        varridas.append((time.perf_counter() - t) * 1000)
    religar("phxrep-a-replica", ip_fonte, "DROP")
    pior_ping, pior_varrer = max(pings), max(varridas)
    # O contraste e o diagnostico: se os dois travassem, o servidor estaria
    # fora do ar; so o que precisa da trava travar aponta a trava.
    # ATE 05/09/2026 esta linha era `pior_varrer > 5_000`, e isso era certo
    # ENQUANTO o defeito existia: o estagio nasceu para ACHAR a trava presa, e
    # so passava quando ela estava presa. O pedido 147 soltou a trava e nunca
    # pode refazer esta bancada -- o daemon do Docker estava fora do ar --,
    # entao a afirmacao ficou apontando para tras: com o conserto no lugar, o
    # `varrer` respondeu em 7 ms e o estagio REPROVOU o proprio conserto.
    # Guarda que afirma o defeito vira catraca contra quem o conserta. Hoje
    # ela afirma a GARANTIA, com os mesmos tetos da bancada de loopback.
    ok = pior_varrer < TETO_VARRER_MS and pior_ping < TETO_PING_MS
    medir("a3-congelamento", ok,
          f"em 43 s de corte silencioso, na replica: pior `ping` "
          f"{pior_ping:.0f} ms ({len(pings)} amostras), pior `varrer` "
          f"{pior_varrer:.0f} ms ({len(varridas)} amostras) -- o servidor esta "
          f"no ar e a trava nao ficou na mao do laco",
          {"pior_ping_ms": round(pior_ping),
           "pior_varrer_ms": round(pior_varrer),
           "amostras": len(varridas)})
    fechar_soquetes()
    compose("compose-a-primary-replica.yml", "down", "-v")


def estagio_a2_queda():
    estagio("a2-queda",
            "`docker kill` na replica (SIGKILL, sem chance de fechar arquivo), "
            "4.000 linhas no source com ela morta, e `docker start`: ela volta "
            "a atender e alcanca sozinha, sem retrato divergente")
    sh("docker", "kill", "phxrep-a-replica")
    fechar_soquetes()
    time.sleep(0.5)
    fonte = liga(6801)

    inicio = 20_000_000
    linhas = [{"id": inicio + k, "nome": f"Enquanto caida {k}",
               "cidade": "Itajai", "limite": "10.00", "ficha": "x"}
              for k in range(4000)]
    fonte({"op": "inserir_lote", "database": "loja", "tabela": TABELA,
           "linhas": linhas})
    alvo = eventos(fonte)
    print(f"    o source gravou 4.000 linhas com a replica morta ({alvo} eventos)")

    t0 = time.perf_counter()
    sh("docker", "start", "phxrep-a-replica")
    replica = liga(6802)
    subiu = (time.perf_counter() - t0) * 1000
    # Contado desde o `docker start`, e nao desde o fim do `liga`: a replica
    # comeca a puxar antes de a bancada conseguir a primeira conexao, e medir
    # so a partir dali daria «alcancou em 0,0 s», que e verdade e nao informa
    # nada. O que interessa e quanto o no leva para voltar INTEIRO.
    if esperar(lambda: eventos(replica) >= alvo, 180) is None:
        alcance = None
    else:
        alcance = time.perf_counter() - t0
    s_fonte, s_replica = soma(fonte), soma(replica)
    r_fonte, r_replica = retrato(fonte), retrato(replica)
    ok = alcance is not None and s_fonte == s_replica and r_fonte == r_replica
    medir("a2-queda", ok,
          f"voltou a atender em {subiu:.0f} ms e alcancou os 4.000 eventos em "
          f"{alcance and round(alcance, 2)}s desde o arranque; soma igual="
          f"{s_fonte == s_replica}; retrato igual={r_fonte == r_replica}",
          {"subiu_ms": round(subiu),
           "alcance_s": None if alcance is None else round(alcance, 2)})
    fechar_soquetes()


def estagio_a_processos(h, n):
    estagio("a-processos",
            "a MESMA carga, o MESMO codigo, em processos no 127.0.0.1 -- para "
            "a comparacao com o conteiner ser de trabalho igual")
    escrever_config("proc", "fonte",
                    config(h, "source", "fonte", bind="127.0.0.1:6851"))
    escrever_config(
        "proc", "replica",
        config(h, "replica", "replica", somente_leitura=True,
               bind="127.0.0.1:6852",
               origens=[origem("fonte", "127.0.0.1", porta=6851, h=h,
                               reconectar_em=2)]))
    limpar_dados("proc", "fonte")
    limpar_dados("proc", "replica")
    subir_processo("proc", "fonte", "p-fonte")
    time.sleep(1)
    subir_processo("proc", "replica", "p-replica")
    fonte, replica = liga(6851), liga(6852)
    p = carga_modo_a(fonte, replica, n, "processo")
    medir("a-processos", p["convergiu"],
          f"{p['fonte_linhas_s']:,} linhas/s no source; a replica alcancou em "
          f"{p['alcance_s']}s ({p['replica_eventos_s']:,} eventos/s); "
          f"convergiu={p['convergiu']}", p)
    derrubar_processos("p-fonte", "p-replica")
    return p


def eagain(container):
    """Quantas vezes o laco deste no ja levou EAGAIN -- o prazo de leitura.

    `Resource temporarily unavailable (os error 11)` no diario do servidor e o
    `set_read_timeout(30s)` do cliente da replica estourando: o outro lado
    aceitou a conexao e nao respondeu em 30 s.
    """
    r = subprocess.run(["docker", "logs", container], capture_output=True, text=True)
    return (r.stdout + r.stderr).count("Resource temporarily unavailable")


def estagio_b_abraco():
    """O abraco mortal do bidirecional, SEM corte nenhum.

    Este estagio nasceu de uma leitura de log durante o estagio da particao:
    depois de a rede voltar, os dois lados ficaram cuspindo EAGAIN e a
    convergencia levou 228,9 s -- **sete ciclos de 30 s em cada um**. A
    explicacao que estava escrita («a espera exponencial do SYN do nucleo»)
    era plausivel e estava errada; o log tinha a resposta.

    O mecanismo: `alcancar_tabela_bidi` toma a trava de dados DESTE servidor e,
    de dentro dela, pede `replicar` ao outro. Do outro lado, servir `replicar`
    (e `posicao`) tambem precisa da trava de LA. Se os dois tem fila ao mesmo
    tempo, cada um segura a propria trava esperando a resposta do outro, que
    nao pode vir. Ninguem sai ate o prazo de leitura de 30 s estourar nos
    dois, e ai eles tentam de novo -- e podem reentrar em passo.

    Se isso e verdade, a particao nao e necessaria: basta escrever nos dois
    lados ao mesmo tempo, com a rede perfeitamente sa. E o que este estagio
    faz, e e por isso que ele e a prova e o outro era so o indicio.
    """
    estagio("b-abraco",
            "SEM corte nenhum, a rede sa: 50.000 linhas em cada lado ao mesmo "
            "tempo. Se cada um segura a PROPRIA trava enquanto espera a "
            "resposta do outro, a ESCRITA DO CLIENTE para atras da trava e o "
            "diario dos dois ganha EAGAIN -- o prazo de leitura de 30 s")
    a0, b0 = eagain("phxrep-b-alfa"), eagain("phxrep-b-beta")
    n = 50_000
    # As duas cargas comecam no MESMO instante: o abraco precisa dos dois
    # lacos dentro do puxar ao mesmo tempo, e montar as linhas leva segundos.
    # Sem a barreira, um lado ja teria acabado quando o outro comecasse.
    largada = threading.Barrier(2)

    def carga(porta, inicio, rotulo):
        f = liga(porta)
        lotes = []
        k = inicio
        while k < inicio + n:
            lotes.append([{"id": k + i, "nome": f"{rotulo} {i}",
                           "cidade": "Itajai", "limite": "1.00"}
                          for i in range(min(5000, inicio + n - k))])
            k += 5000
        largada.wait()
        for l in lotes:
            f({"op": "inserir_lote", "database": "loja", "tabela": TABELA,
               "linhas": l})

    fios = [threading.Thread(target=carga, args=(6811, 1_000_000, "de alfa")),
            threading.Thread(target=carga, args=(6812, 2_000_000, "de beta"))]
    for f in fios:
        f.start()
    t0 = time.perf_counter()
    for f in fios:
        f.join()
    escrita_s = time.perf_counter() - t0

    # O `checksum` serve de dois: diz se convergiu E e a sonda da trava, porque
    # ele precisa dela para ler a tabela inteira.
    alfa, beta = liga(6811), liga(6812)
    sondas = []

    def convergiu():
        t = time.perf_counter()
        sa = soma(alfa)
        sondas.append((time.perf_counter() - t) * 1000)
        sb = soma(beta)
        return sa is not None and sb is not None and sa[1] == sb[1] >= 2 * n

    t = esperar(convergiu, 900, passo=0.5)
    a1, b1 = eagain("phxrep-b-alfa"), eagain("phxrep-b-beta")
    pior = max(sondas) if sondas else 0
    travou = (a1 - a0) > 0 or (b1 - b0) > 0
    # O numero que o dono sente e a ESCRITA: as mesmas 100.000 linhas em modo A
    # entram em ~5,6 s. Aqui elas esperam a trava que o laco segura.
    medir("b-abraco", t is not None,
          f"{2 * n} linhas escritas nos dois lados ao mesmo tempo levaram "
          f"{escrita_s:.1f}s (as mesmas 100.000 no modo A levam ~5,6s); "
          f"convergiram {t and round(t, 1)}s depois; EAGAIN novos no diario: "
          f"alfa +{a1 - a0}, beta +{b1 - b0}; pior `checksum` {pior:.0f} ms -- "
          f"{'os dois se trancaram' if travou else 'nao houve travamento'}",
          {"escrita_s": round(escrita_s, 1),
           "convergencia_s": None if t is None else round(t, 1),
           "eagain_alfa": a1 - a0, "eagain_beta": b1 - b0,
           "pior_checksum_ms": round(pior), "linhas": 2 * n})


def estagio_b(h):
    estagio("b", "modo B: ida e volta, laco morto (eventos param), conflito "
                 "pelo carimbo mais recente, e a PARTICAO -- escrita nos dois "
                 "lados cegos, convergencia por CHAVE com rowids DIFERENTES")
    escrever_config("b", "alfa", config(
        h, "multi", "alfa", origens=[origem("beta", "beta", h=h, reconectar_em=1)]))
    escrever_config("b", "beta", config(
        h, "multi", "beta", origens=[origem("alfa", "alfa", h=h, reconectar_em=1)]))
    limpar_dados("b", "alfa")
    limpar_dados("b", "beta")
    compose("compose-b-multi-master.yml", "up", "-d")
    alfa, beta = liga(6811), liga(6812)
    criar_tabela(alfa, memo=False)

    inserir(alfa, 1, "nascida em alfa")
    t1 = esperar(lambda: por_chave(beta).get(1, ("",))[0] == "nascida em alfa", 30)
    inserir(beta, 2, "nascida em beta")
    t2 = esperar(lambda: por_chave(alfa).get(2, ("",))[0] == "nascida em beta", 30)
    ev = (eventos(alfa), eventos(beta))
    time.sleep(6)
    ev2 = (eventos(alfa), eventos(beta))
    parado = ev == ev2
    medir("b-laco", t1 is not None and t2 is not None and parado and ev2 == (2, 2),
          f"alfa->beta {t1 and round(t1, 2)}s; beta->alfa {t2 and round(t2, 2)}s; "
          f"eventos {ev} -> {ev2} "
          f"({'parados' if parado else 'CRESCENDO: LACO VIVO'})",
          {"eventos": list(ev2)})

    # --------- o conflito, com os dois lados cegos: a particao de verdade
    estagio("b-particao",
            "corte SILENCIOSO (DROP) entre os dois, os dois vivos e aceitando "
            "escrita; ao religar, a chave 1 fica com o carimbo mais NOVO nos "
            "dois, as chaves novas de cada lado atravessam, e o rowid delas e "
            "DIFERENTE em cada servidor")
    ip_alfa = ip_de("phxrep-b-alfa")
    cortar("phxrep-b-beta", ip_alfa, "DROP")
    t_corte = time.perf_counter()

    # Cada lado escreve uma chave nova e altera a chave 1. Alfa primeiro,
    # beta depois -- entao beta tem o carimbo mais novo e deve vencer.
    rid_a = por_chave(alfa)[1][1]
    alfa({"op": "atualizar", "database": "loja", "tabela": TABELA,
          "rowid": rid_a, "linha": {"id": 1, "nome": "de alfa, mais velha",
                                    "cidade": "Curitiba", "limite": "1.00"}})
    inserir(alfa, 10, "so de alfa, na cegueira")
    inserir(alfa, 11, "so de alfa, na cegueira, dois")
    inserir(alfa, 12, "so de alfa, na cegueira, tres")
    time.sleep(1.2)
    rid_b = por_chave(beta)[1][1]
    beta({"op": "atualizar", "database": "loja", "tabela": TABELA,
          "rowid": rid_b, "linha": {"id": 1, "nome": "de beta, mais nova",
                                    "cidade": "Bruxelas", "limite": "2.00"}})
    inserir(beta, 20, "so de beta, na cegueira")

    cegos = {"alfa": len(por_chave(alfa)), "beta": len(por_chave(beta))}
    # O que o servidor CONTA de si mesmo enquanto esta cego. Num corte
    # silencioso a resposta e nada: o laco esta pendurado num `connect` que
    # nao volta, entao nao ha rodada nova para registrar nem erro para gravar.
    # Quem olha `replicacao_estado` ve o retrato de antes do corte.
    cego_alfa = alfa({"op": "replicacao_estado"})["resultado"]["origens"].get("beta", {})
    cego_beta = beta({"op": "replicacao_estado"})["resultado"]["origens"].get("alfa", {})
    print(f"    durante a particao: alfa com {cegos['alfa']} linhas, "
          f"beta com {cegos['beta']} -- os dois aceitaram escrita")
    print(f"    durante a particao: `replicacao_estado` em alfa sobre beta: "
          f"ultima_rodada={cego_alfa.get('ultima_rodada')!r} "
          f"ultimo_erro={cego_alfa.get('ultimo_erro')!r}")
    print(f"    durante a particao: `replicacao_estado` em beta sobre alfa: "
          f"ultima_rodada={cego_beta.get('ultima_rodada')!r} "
          f"ultimo_erro={cego_beta.get('ultimo_erro')!r}")

    religar("phxrep-b-beta", ip_alfa, "DROP")
    t_religou = time.perf_counter()
    fora_s = t_religou - t_corte

    def convergiu():
        a, b = por_chave(alfa), por_chave(beta)
        return (set(a) == set(b) == {1, 2, 10, 11, 12, 20}
                and a[1][0] == b[1][0] == "de beta, mais nova")

    t_conv = esperar(convergiu, 300, passo=0.2)
    a, b = por_chave(alfa), por_chave(beta)
    # A prova de que a identidade e a CHAVE: a linha 20 nasceu em beta com um
    # rowid e entrou em alfa com OUTRO -- porque em alfa ja havia tres linhas
    # que beta nunca viu. Se a replicacao casasse por rowid, ela sobrescreveria
    # a linha errada.
    rowids_diferentes = {k: (a[k][1], b[k][1]) for k in sorted(a)
                         if a[k][1] != b[k][1]}
    s_a, s_b = soma(alfa), soma(beta)
    c_a, c_b = retrato_por_chave(alfa), retrato_por_chave(beta)
    # As DUAS comparacoes, e a diferenca entre elas e o achado do estagio: o
    # conteudo casado pela chave e identico; a soma ordenada do servidor NAO
    # e, porque cada servidor guardou as linhas na ordem em que ELE as viu.
    ok = (t_conv is not None and bool(rowids_diferentes)
          and c_a == c_b and s_a[1] == s_b[1] and s_a[2] == s_b[2])
    medir("b-particao", ok,
          f"{fora_s:.1f}s fora da rede; convergiram em "
          f"{t_conv and round(t_conv, 2)}s depois de religar; chave 1 = "
          f"{a.get(1, ('?',))[0]!r} nos dois; rowids diferentes entre os "
          f"servidores: {rowids_diferentes}; conteudo por chave "
          f"alfa={c_a} beta={c_b} ({'IGUAL' if c_a == c_b else 'DIFERENTE'}); "
          f"soma ordenada do servidor alfa={s_a} beta={s_b} "
          f"({'igual' if s_a == s_b else 'DIFERENTE, e esta certo'})",
          {"fora_s": round(fora_s, 1),
           "convergencia_s": None if t_conv is None else round(t_conv, 2),
           "rowids_diferentes": {str(k): list(v)
                                 for k, v in rowids_diferentes.items()},
           "soma_alfa": s_a, "soma_beta": s_b,
           "conteudo_por_chave_alfa": c_a, "conteudo_por_chave_beta": c_b,
           "cego_alfa": cego_alfa, "cego_beta": cego_beta,
           "pior_resposta_ms": {str(p): [round(PIOR_MS[p]), PIOR_OP[p]]
                                for p in sorted(PIOR_MS)}})

    # --------- os DOIS cortes que o mundo tem, cronometrados um contra o outro
    # A HIPOTESE QUE MORREU AQUI, e vale registrar: a primeira redacao deste
    # estagio dizia que a lentidao do corte silencioso vinha da «espera
    # exponencial do SYN do nucleo», porque o `connect` do laco nao tem prazo.
    # Era plausivel e estava errada. O diario dos dois conteineres tinha a
    # resposta: sete `Resource temporarily unavailable` em CADA lado, sete
    # vezes 30 s -- o prazo de LEITURA, nao o de conexao. E o abraco do
    # estagio (b-abraco), que reproduz o mesmo sem corte nenhum.
    # Diagnostico plausivel nao e diagnostico medido.
    estagio("b-cortes",
            "quanto tempo depois de a rede VOLTAR a replicacao volta, por "
            "tipo de corte e por duracao. Com RST (processo morto) a retomada "
            "e imediata sempre; com silencio (cabo cortado) ela e variavel e "
            "nao limitada, porque os dois lados voltam com fila e caem no "
            "abraco do estagio seguinte")
    cortes = {}
    chave = 40
    for como in ["REJECT", "DROP"]:
        for duracao in [3, 20, 45]:
            chave += 1
            cortar("phxrep-b-beta", ip_alfa, como)
            # A escrita entra NO COMECO do corte: assim o laco tem a duracao
            # inteira para tropecar e recuar antes de a rede voltar.
            inserir(alfa, chave, f"escrita no corte {como} de {duracao}s")
            time.sleep(duracao)
            t0 = time.perf_counter()
            religar("phxrep-b-beta", ip_alfa, como)
            t = esperar(lambda c=chave: c in por_chave(beta), 300, passo=0.2)
            cortes[f"{como}-{duracao}s"] = None if t is None else round(t, 1)
            print(f"    corte {como:<7} de {duracao:>2}s: da religacao ate a "
                  f"linha chegar em beta: {cortes[f'{como}-{duracao}s']}s")
            # A retomada tem de acabar antes do proximo corte, senao o numero
            # seguinte herda a espera deste.
            esperar(lambda: por_chave(alfa).keys() == por_chave(beta).keys(),
                    300, passo=0.2)
    faltou = [k for k, v in cortes.items() if v is None]
    medir("b-cortes", not faltou,
          "retomada depois de a rede voltar, em segundos: " +
          "; ".join(f"{k}={v}" for k, v in cortes.items()),
          {"retomada_s": cortes})

    # --------- o abraco mortal, com a rede sa. Entra DEPOIS dos cortes,
    # porque a prova dele exige rede perfeita, e ANTES da tabela sem chave,
    # porque a recusa dela faria ruido em todo laco seguinte.
    estagio_b_abraco()

    # --------- tabela sem chave unica: o bidirecional recusa com o motivo
    estagio("b-sem-chave",
            "tabela SEM chave unica: o bidirecional recusa com o motivo "
            "legivel em `replicacao_estado`, e a linha nao atravessa")
    criar_tabela(alfa, "log_livre", com_chave=False, memo=False)
    inserir(alfa, 1, "nao devia viajar", tabela="log_livre")
    time.sleep(5)
    em_beta = por_chave(beta, "log_livre")
    estado = beta({"op": "replicacao_estado"})["resultado"]
    recusas = estado["origens"].get("alfa", {}).get("recusas", {})
    motivo = recusas.get("loja/log_livre", "")
    ok = (not em_beta) and "chave unica" in motivo
    medir("b-sem-chave", ok,
          f"linhas em beta: {len(em_beta)}; motivo: {motivo!r}")
    fechar_soquetes()
    compose("compose-b-multi-master.yml", "down", "-v")


def estagio_c(h):
    estagio("c", "modo C: o spare recusa ate leitura (SPARE_EM_ESPERA 4004) e "
                 "deixa passar ping/checksum; `docker kill` no primario e "
                 "`spare_promover` faz dele um source com os dados intactos")
    escrever_config("c", "primario", config(h, "source", "primario"))
    escrever_config("c", "spare", config(
        h, "spare", "spare", somente_leitura=True,
        origens=[origem("primario", "primario", h=h, reconectar_em=1)]))
    escrever_config("c", "leitor", config(
        h, "read_replica", "leitor", somente_leitura=True,
        origens=[origem("primario", "primario", h=h, reconectar_em=1)]))
    for no in ["primario", "spare", "leitor"]:
        limpar_dados("c", no)
    compose("compose-c-spare.yml", "up", "-d")
    primario, spare, leitor = liga(6821), liga(6822), liga(6823)
    criar_tabela(primario, memo=False)
    for k in range(1, 201):
        inserir(primario, k, f"Cliente {k:03d}")
    esperar(lambda: eventos(spare) >= 200, 60)
    esperar(lambda: eventos(leitor) >= 200, 60)

    ler = spare({"op": "varrer", "database": "loja", "tabela": TABELA})
    gravar = inserir(spare, 999, "cliente comum")
    ping = spare({"op": "ping"})["resultado"]["papel"]
    conferir = spare({"op": "checksum", "database": "loja", "tabela": TABELA})
    recusou = (not ler.get("ok") and ler.get("nome") == "SPARE_EM_ESPERA"
               and not gravar.get("ok") and gravar.get("codigo") == 4004
               and "spare_promover" in gravar.get("erro", ""))
    print(f"    antes da promocao: varrer={ler.get('nome')} "
          f"({ler.get('codigo')}), inserir={gravar.get('codigo')}, "
          f"ping={ping}, checksum={'passa' if conferir.get('ok') else 'NEGADO'}")

    # A soma do primario ANTES de ele morrer -- e contra ela que a promocao
    # vai ser conferida. Perguntar depois nao daria: ele nao existe mais.
    antes = soma(primario)
    sh("docker", "kill", "phxrep-c-primario")
    morreu = tenta_ligar(6821, segundos=4) is None
    t0 = time.perf_counter()
    promo = spare({"op": "spare_promover", "motivo": "o primario morreu"})
    papel2 = spare({"op": "ping"})["resultado"]["papel"]
    ler2 = spare({"op": "varrer", "database": "loja", "tabela": TABELA})
    gravar2 = inserir(spare, 999, "agora sim")
    ms = (time.perf_counter() - t0) * 1000
    depois = soma(spare)

    ok = (recusou and ping == "spare" and conferir.get("ok")
          and morreu and promo.get("ok") and papel2 == "source"
          and ler2.get("ok") and gravar2.get("ok")
          and antes[1] == depois[1] - 1)
    medir("c", ok,
          f"primario morto={morreu}; promocao em {ms:.0f} ms; papel={papel2}; "
          f"varrer={'ok' if ler2.get('ok') else 'NEGADO'}; "
          f"inserir={'ok' if gravar2.get('ok') else 'NEGADO'}; soma do "
          f"primario antes={antes}, do promovido depois={depois}",
          {"promocao_ms": round(ms), "soma_antes": antes, "soma_depois": depois})

    # O achado do failover manual: para onde a read replica manda o cliente
    # DEPOIS de o primario morrer? Para o primario morto -- o endereco sai da
    # configuracao dela, e ninguem a reconfigurou.
    gravar3 = inserir(leitor, 5000, "depois do failover")
    medir("c-redireciona-orfao",
          gravar3.get("codigo") == 4003 and "primario:5000" in gravar3.get("erro", ""),
          f"a read replica, com o primario ja morto e o spare promovido, "
          f"continua respondendo {gravar3.get('nome')} {gravar3.get('codigo')} "
          f"-> {gravar3.get('erro', '')!r}")
    fechar_soquetes()
    compose("compose-c-spare.yml", "down", "-v")


def estagio_d(h):
    estagio("d", "modo D: a read replica le; a escrita recebe REDIRECIONA "
                 "(4003) com o endereco do primario -- e o endereco e o NOME "
                 "DE SERVICO, que so existe dentro da rede do compose")
    escrever_config("d", "primario", config(h, "source", "primario"))
    escrever_config("d", "leitor", config(
        h, "read_replica", "leitor", somente_leitura=True,
        origens=[origem("primario", "primario", h=h, reconectar_em=1)]))
    limpar_dados("d", "primario")
    limpar_dados("d", "leitor")
    compose("compose-d-read-replica.yml", "up", "-d")
    primario, leitor = liga(6831), liga(6832)
    criar_tabela(primario, memo=False)
    for k in range(1, 501):
        inserir(primario, k, f"Cliente {k:03d}")
    t = esperar(lambda: eventos(leitor) >= 500, 60)

    ler = leitor({"op": "varrer", "database": "loja", "tabela": TABELA,
                  "max": 1000})
    gravar = inserir(leitor, 9999, "nao entra")
    s_p, s_l = soma(primario), soma(leitor)
    endereco = gravar.get("erro", "").split()[1] if gravar.get("erro") else ""
    # O prefixo que o cliente recorta e `REDIRECIONA host:porta`. Aqui ele
    # devolve `primario:5000`, que o hospedeiro nao resolve.
    resolve_no_hospedeiro = True
    try:
        socket.getaddrinfo(endereco.split(":")[0], None)
    except OSError:
        resolve_no_hospedeiro = False

    ok = (ler.get("ok") and len(ler["resultado"]["linhas"]) == 500
          and gravar.get("codigo") == 4003
          and gravar.get("nome") == "REDIRECIONA"
          and s_p == s_l)
    medir("d", ok,
          f"alcancou 500 eventos em {t and round(t, 2)}s; leitura devolveu "
          f"{len(ler.get('resultado', {}).get('linhas', []))} linhas; escrita: "
          f"{gravar.get('nome')} {gravar.get('codigo')} -> "
          f"{gravar.get('erro', '')!r}; soma primario={s_p} leitor={s_l}",
          {"endereco_do_redireciona": endereco,
           "resolve_no_hospedeiro": resolve_no_hospedeiro,
           "alcance_s": None if t is None else round(t, 2)})
    fechar_soquetes()
    compose("compose-d-read-replica.yml", "down", "-v")


def firewall_regras(container, permitidos):
    """As regras da tabela da secao 7, aplicadas no namespace do source.

        Source  ENTRADA  TCP 5000   somente o IP da Replica
        Source  SAIDA    retorno    Replica (e mais ninguem)
    """
    for cmd in [
        ["iptables", "-P", "INPUT", "ACCEPT"],
        ["iptables", "-F"],
        ["iptables", "-A", "INPUT", "-i", "lo", "-j", "ACCEPT"],
        ["iptables", "-A", "INPUT", "-m", "conntrack", "--ctstate",
         "ESTABLISHED,RELATED", "-j", "ACCEPT"],
    ]:
        na_rede_do(container, *cmd)
    for ip in permitidos:
        na_rede_do(container, "iptables", "-A", "INPUT", "-p", "tcp",
                   "--dport", "5000", "-s", ip, "-j", "ACCEPT")
    na_rede_do(container, "iptables", "-A", "INPUT", "-j", "DROP")
    # A saida: so o retorno das conexoes que ENTRARAM. O source nao alcanca
    # ninguem -- e essa e a metade do desenho que so o conteiner prova.
    na_rede_do(container, "iptables", "-A", "OUTPUT", "-o", "lo", "-j", "ACCEPT")
    na_rede_do(container, "iptables", "-A", "OUTPUT", "-m", "conntrack",
               "--ctstate", "ESTABLISHED,RELATED", "-j", "ACCEPT")
    na_rede_do(container, "iptables", "-A", "OUTPUT", "-j", "DROP")
    return na_rede_do(container, "iptables", "-S").stdout.strip()


def repor_intruso():
    """Apaga a base do intruso e o sobe de novo: cada fase mede do zero.

    Sem isto a fase seguinte herdaria os eventos que a anterior deixou passar,
    e o numero diria «ainda tem 200» quando o certo e «nao levou nenhum novo».
    """
    sh("docker", "stop", "-t", "2", "phxrep-e-intruso")
    limpar_dados("e", "intruso")
    sh("docker", "start", "phxrep-e-intruso")
    time.sleep(1)


def eventos_do_intruso(segundos=25):
    """Quantos eventos o intruso conseguiu levar do diario do source."""
    fala = tenta_ligar(6843, segundos=8)
    if fala is None:
        return -1
    esperar(lambda: eventos(fala) >= 200, segundos)
    return eventos(fala)


def estagio_e(h):
    estagio("e", "o desenho da secao 7. Quatro fases, e a resposta de cada "
                 "uma e o numero de eventos que o INTRUSO consegue levar")
    escrever_config("e", "fonte", config(h, "source", "fonte"))
    escrever_config("e", "replica", config(
        h, "replica", "replica", somente_leitura=True,
        origens=[origem("fonte", "fonte", h=h, reconectar_em=1)]))
    # O intruso e uma replica com a configuracao VAZADA: mesmo token, mesmo
    # usuario, mesmo `senha_hash`. E o modelo de ameaca real -- quem rouba um
    # `config.json` de replica rouba tudo o que ele precisa.
    escrever_config("e", "intruso", config(
        h, "replica", "intruso", somente_leitura=True,
        origens=[origem("fonte", "fonte", h=h, reconectar_em=1)]))
    for no in ["fonte", "replica", "intruso"]:
        limpar_dados("e", no)
    compose("compose-e-firewall.yml", "up", "-d")
    fonte, replica = liga(6841), liga(6842)
    criar_tabela(fonte, memo=False)
    for k in range(1, 201):
        inserir(fonte, k, f"Cliente {k:03d}")
    esperar(lambda: eventos(replica) >= 200, 60)

    fases = {}

    # ---- fase 1: nada trancado. A rede do Docker e PLANA.
    roubado = eventos_do_intruso()
    fases["1-sem-tranca"] = roubado
    print(f"    fase 1 (nada trancado):        o intruso levou {roubado} eventos")

    # ---- fase 2: `replicas_autorizadas`, a segunda tranca DOCUMENTADA
    escrever_config("e", "fonte", config(
        h, "source", "fonte", replicas_autorizadas=["172.28.90.20"]))
    sh("docker", "restart", "-t", "2", "phxrep-e-fonte")
    fechar_soquetes()
    fonte = liga(6841)
    repor_intruso()
    roubado2 = eventos_do_intruso()
    fases["2-replicas-autorizadas"] = roubado2
    print(f"    fase 2 (replicas_autorizadas): o intruso levou {roubado2} eventos")

    # ---- fase 3: `ips_permitidos`, a tranca que o codigo LE
    escrever_config("e", "fonte", config(
        h, "source", "fonte",
        ips_permitidos=["172.28.90.20", "172.28.90.1"],
        replicas_autorizadas=["172.28.90.20"]))
    sh("docker", "restart", "-t", "2", "phxrep-e-fonte")
    fechar_soquetes()
    fonte = liga(6841)
    repor_intruso()
    roubado3 = eventos_do_intruso()
    fases["3-ips-permitidos"] = roubado3
    replica = liga(6842)
    inserir(fonte, 900, "depois da tranca")
    replica_ok = esperar(lambda: eventos(replica) >= 201, 30) is not None
    print(f"    fase 3 (ips_permitidos):       o intruso levou {roubado3} "
          f"eventos; a replica autorizada continua = {replica_ok}")

    # A tentativa recusada fica registrada? A secao 7 promete que sim.
    registro = ""
    caminho = os.path.join(BASE, "e", "fonte", "acessos.log")
    if os.path.exists(caminho):
        with open(caminho) as f:
            linhas = [l for l in f if "172.28.90.30" in l]
        registro = f"{len(linhas)} linha(s) do intruso no acessos.log"
        if linhas:
            registro += f"; a ultima: {linhas[-1].strip()[:160]}"
    else:
        registro = "acessos.log nao existe"
    print(f"    fase 3: {registro}")

    # A terceira camada, que nao estava no roteiro e apareceu sozinha: bater na
    # porta recusada dezenas de vezes poe o IP na LISTA NEGRA. O intruso nao so
    # e barrado -- ele e banido, e o banimento sobrevive ao reinicio porque mora
    # no volume.
    bloqueios = fonte({"op": "bloqueios"})
    banido = "172.28.90.30" in json.dumps(bloqueios.get("resultado", {}))
    print(f"    fase 3: o intruso acabou na lista negra do source = {banido}")

    # ---- fase 4: o firewall de verdade, no namespace do source
    regras = firewall_regras("phxrep-e-fonte", ["172.28.90.20", "172.28.90.1"])
    # (a) o intruso nem chega ao TCP: DROP nao devolve recusa, devolve silencio
    chegou, texto_intruso = alcanca("phxrep-e-tunel", "172.28.90.10")
    # (b) a replica autorizada continua replicando
    fechar_soquetes()
    fonte, replica = liga(6841), liga(6842)
    inserir(fonte, 901, "com o firewall no ar")
    alvo = eventos(fonte)
    replica_passa = esperar(lambda: eventos(replica) >= alvo, 30) is not None
    # (c) o source NAO alcanca ninguem: uma conexao de dentro dele para fora
    saida = na_rede_do("phxrep-e-fonte", "timeout", "4", "nc", "-z", "-v",
                       "172.28.90.30", "5000")
    fonte_alcanca = saida.returncode == 0
    fases["4-iptables-intruso-alcanca"] = chegou
    fases["4-iptables-replica-passa"] = replica_passa
    fases["4-source-alcanca-alguem"] = fonte_alcanca
    print(f"    fase 4 (iptables secao 7):     o intruso alcanca a porta = "
          f"{chegou} ({texto_intruso!r})")
    print(f"    fase 4: a replica autorizada continua replicando = {replica_passa}")
    print(f"    fase 4: o source consegue ABRIR conexao para alguem = "
          f"{fonte_alcanca}")

    fases["3-lista-negra"] = banido
    ok = (roubado >= 200 and roubado3 <= 0 and replica_ok
          and not chegou and replica_passa and not fonte_alcanca)
    medir("e", ok,
          f"intruso levou: sem tranca={roubado}, com replicas_autorizadas="
          f"{roubado2}, com ips_permitidos={roubado3}; com iptables ele nem "
          f"abre a porta ({not chegou}); a replica autorizada passa nas "
          f"quatro fases; o source nao alcanca ninguem ({not fonte_alcanca})",
          {"fases": fases, "acessos_log": registro, "iptables": regras})

    # O achado que esta fase existe para achar, dito com todas as letras. Antes
    # do conserto este numero era 200 de 200: o campo existia no config.json,
    # na secao 7 e na tela, e nenhuma linha de codigo o lia.
    medir("e-replicas-autorizadas", roubado2 <= 0,
          f"`replicas_autorizadas` sozinho: o intruso levou {roubado2} de "
          f"{roubado} eventos (antes do conserto levava {roubado})")
    fechar_soquetes()
    compose("compose-e-firewall.yml", "down", "-v")


# ----------------------------------------------------------------------- main

def limpar_tudo():
    fechar_soquetes()
    derrubar_processos()
    for arq in ["compose-a-primary-replica.yml", "compose-b-multi-master.yml",
                "compose-c-spare.yml", "compose-d-read-replica.yml",
                "compose-e-firewall.yml"]:
        compose(arq, "down", "-v", "--remove-orphans", checar=False)


def _bytes_de(texto):
    """«6.41MB», «8.19kB», «0B» -> bytes. O `docker history` so fala assim."""
    unidades = {"B": 1, "kB": 1_000, "MB": 1_000_000, "GB": 1_000_000_000}
    for sufixo, fator in sorted(unidades.items(), key=lambda p: -len(p[0])):
        if texto.endswith(sufixo):
            return float(texto[: -len(sufixo)]) * fator
    return 0.0


def construir_imagem():
    if not os.path.exists(ALVO_MUSL):
        sys.exit(
            f"nao achei {ALVO_MUSL}\n"
            "rode antes:\n"
            "  rustup target add x86_64-unknown-linux-musl\n"
            "  cargo build --release --target x86_64-unknown-linux-musl "
            "--bin phxsqld")
    estagio_dir = os.path.join(BASE, "imagem")
    os.makedirs(estagio_dir, exist_ok=True)
    shutil.copy2(ALVO_MUSL, os.path.join(estagio_dir, "phxsqld"))
    sh("strip", os.path.join(estagio_dir, "phxsqld"), checar=False)
    sh("docker", "build", "-q", "-t", IMAGEM, "-f",
       os.path.join(AQUI, "Dockerfile"), estagio_dir)
    # TRES numeros de tamanho, e eles nao concordam -- por isso os tres saem
    # daqui em vez de um ser digitado num documento:
    #
    #   camada     a soma do `docker history`: o conteudo da imagem
    #   comprimido `docker image inspect .Size` com o snapshotter containerd:
    #              o que se baixa
    #   docker images  maior que os dois, porque conta o manifesto de
    #              atestacao que o BuildKit acrescenta
    #
    # A primeira versao desta bancada publicou o COMPRIMIDO como «a imagem tem
    # 2,7 MB» e o `docker images` mostrava 9,11 -- numero certo, rotulo errado.
    linhas = sh("docker", "history", IMAGEM, "--format", "{{.Size}}").split()
    camada = sum(_bytes_de(t) for t in linhas)
    comprimido = int(sh("docker", "image", "inspect", "-f", "{{.Size}}", IMAGEM))
    visivel = sh("docker", "images", IMAGEM, "--format", "{{.Size}}")
    print(f"imagem {IMAGEM}: {camada / 1_000_000:.2f} MB de camada "
          f"({comprimido / 1_000_000:.2f} MB comprimidos para baixar; o "
          f"`docker images` mostra {visivel}) -- `scratch` + o binario musl, "
          f"sem shell")
    RESULTADO["imagem"] = {"camada_mb": round(camada / 1_000_000, 2),
                           "comprimido_mb": round(comprimido / 1_000_000, 2),
                           "docker_images": visivel}


def main():
    n = int(os.environ.get("PHX_LINHAS", "100000"))
    so = sys.argv[1] if len(sys.argv) > 1 else None
    os.makedirs(BASE, exist_ok=True)
    construir_imagem()
    t0 = time.perf_counter()
    h = hash_da_senha()
    try:
        limpar_tudo()
        if so in (None, "0", "a"):
            estagio_0_endereco(h)
            estagio_a(h, n)
            estagio_a2_queda()
            estagio_a3_congelamento()
            p = estagio_a_processos(h, n)
            d = RESULTADO["a"]
            RESULTADO["comparacao"] = {
                "linhas": n,
                "docker": {k: d.get(k) for k in
                           ["fonte_linhas_s", "replica_eventos_s", "alcance_s",
                            "atraso_ms", "atraso_faixa_ms",
                            "atraso_amostras_ms"]},
                "processo": {k: p.get(k) for k in
                             ["fonte_linhas_s", "replica_eventos_s",
                              "alcance_s", "atraso_ms", "atraso_faixa_ms",
                              "atraso_amostras_ms"]},
            }
        if so in (None, "b"):
            estagio_b(h)
        if so in (None, "c"):
            estagio_c(h)
        if so in (None, "d"):
            estagio_d(h)
        if so in (None, "e"):
            estagio_e(h)
    finally:
        limpar_tudo()
    RESULTADO["minutos"] = round((time.perf_counter() - t0) / 60, 1)
    RESULTADO["reconexoes"] = len(RECONEXOES)
    # So a corrida INTEIRA grava. `provar.py a` e a forma normal de trabalhar
    # num estagio, e ela apagava os outros catorze do `resultados.json` --
    # inclusive o retrato do defeito que a §17 cita. O arquivo se chama "a
    # ultima corrida completa"; ele tem de ser completo por construcao.
    arq = os.path.join(AQUI, "resultados.json")
    if so is None:
        with open(arq, "w") as f:
            json.dump(RESULTADO, f, indent=2, ensure_ascii=False)
        print(f"\ngravado em {arq}")
    else:
        print(f"\ncorrida PARCIAL (estagio {so!r}): {arq} nao foi tocado.\n"
              "Para atualizar o registro, rode a bancada inteira:\n"
              "  python3 bancada/replicacao/docker/provar.py")
    print()
    print("RESULTADO " + json.dumps(RESULTADO, ensure_ascii=False))
    if FALHAS:
        sys.exit(f"estagios com falha: {FALHAS}")


if __name__ == "__main__":
    main()
