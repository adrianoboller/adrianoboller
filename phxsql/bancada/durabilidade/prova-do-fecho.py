#!/usr/bin/env python3
"""Prova ou derruba, contra o `phxsqld` DE PE (nunca por teste unitario), o
buraco de durabilidade achado por `crates/phxsql-store/examples/
sonda-do-fecho.rs`: o fecho da janela em `recursos.durabilidade: por_lote`
sincroniza o `.reg` de TODA tabela suja, ou so da que estava aberta na mao de
quem gravou por ultimo?

    cargo build --release
    python3 bancada/durabilidade/prova-do-fecho.py

Sobe um `phxsqld` de verdade, ANEXA (nao substitui) um `strace -f -y` no PID
dele -- e e por isso que a prova vale para o artefato que importa, e nao para
o `phxsql-store` isolado. Cada `fsync` que o `strace` capta ja vem com o
caminho do arquivo resolvido (`-y`), entao contar por tabela e por extensao e
so agrupar a saida -- nenhum numero deste relatorio e digitado.

# O mecanismo, para quem le o resultado sem reler o fonte

`Table::abrir` LE o cabecalho do `.reg` com um `std::fs::File::open` cru,
separado do `Volumes` que faz o `fsync` de verdade (comentario em
`reg.rs::abrir`, sobre o custo de trazer o arquivo inteiro para a RAM). Os
outros sete arquivos da tabela (`.ndx`, `.bin`, `.memo`, `.log`, `.trash`,
`.reason`; o `.lgpd` so existe com coluna marcada) leem o cabecalho deles
PELO `Volumes` (`BlobFile::abrir` chama `.cab()`, que chama `Volumes::ler`,
que chama `arquivo()` e DEIXA o descritor no cache) ou guardam um `File`
proprio aberto para sempre (`NdxFile`). Reabrir uma tabela sem escrever nela
sincroniza os sete -- e pula o oitavo, porque o `.reg` dela nunca entrou no
cache de descritores abertos.

`descarregar_sujas_com` (`crates/phxsql-server/src/servidor.rs`, perto da
linha 9100) e QUEM REABRE: para cada tabela suja que NAO e a que acabou de
escrever, ela faz `abrir_database -> abrir_qualificada -> sincronizar` num
`Table` novo. A tabela que DISPAROU o fecho (a que `gravar_de_verdade`
recebe por parametro) sincroniza pelo proprio `Table` que a escreveu -- essa
sim inclui o `.reg`, porque escrever atraves dele deixou o descritor no
cache.

# O controle positivo, e por que dois

A tarefa pede um caso, na MESMA corrida, em que o `.reg` E sincronizado e o
instrumento o ve -- senao "nenhum fsync no .reg" pode ser o defeito ou pode
ser o instrumento cego. Aqui ha DOIS, por motivos diferentes:

1. `rodar_controle_por_operacao()`: um servidor PARTE, `durabilidade:
   por_operacao`, tabela nova, UMA insercao. Prova que o par
   strace+regex+classificador enxerga um `.reg` sincronizado quando o codigo
   pede -- valida o cano inteiro, de ponta a ponta, antes de confiar em
   qualquer numero dos dois cenarios.

2. Dentro do proprio cenario (a): a tabela QUE FECHA a janela (a terceira
   gravacao) e a mesma tabela que o instrumento esta olhando -- entao, na
   MESMA sessao de `strace`, no MESMO regime `por_lote`, ve-se um `.reg`
   sincronizado (o gatilho) ao lado de dois que nao foram (as sujas). Se o
   instrumento fosse cego para aquele PID ou aquela extensao, o gatilho
   tambem sairia zerado -- e nao sai.

No cenario (b) nao ha "tabela que dispara": e o proprio ponto do cenario, e
por isso ele se apoia nos dois controles de fora (1) mais um controle
INTERNO: se o `strace` estivesse simplesmente surdo para aquele momento, os
outros sete arquivos de cada tabela suja tambem sairiam zerados -- e nao
saem, so o `.reg`.

# O que NAO fica medido aqui

Tempo. Este script conta CHAMADAS DE SISTEMA, que -- ao contrario de
latencia -- nao mentem numa maquina ocupada (e o proprio `CLAUDE.md` traca
essa linha). Ainda assim ele confere `bancada/esta-medindo.sh` de cortesia e
avisa se alguem mais estiver competindo pela maquina; nao aborta por causa
disso, porque a garantia deste numero nao depende de a maquina estar quieta.

Mata so o PID que o proprio script sobe. Nunca `pkill`.
"""
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.environ.get("PHX_RAIZ", os.path.abspath(os.path.join(AQUI, "..", "..")))
sys.path.insert(0, os.path.join(RAIZ, "bancada", "profiler"))
from comum import PHXSQLD, TOKEN, hash_da_senha  # noqa: E402

PORTA = 7531
BASE = os.path.join(AQUI, ".base-do-fecho")
DB = "fecho"
RESULTADO = os.path.join(AQUI, "resultado-do-fecho.json")

# As oito extensoes de uma tabela (ver `crates/phxsql-store/src/table.rs`).
# `.lgpd` (a trilha) so nasce com coluna marcada como dado pessoal -- as
# tabelas desta prova nao tem nenhuma, entao ela nunca aparece, do mesmo
# jeito que na sonda original.
EXTENSOES = ("reg", "ndx", "bin", "memo", "log", "trash", "reason", "lgpd")


# ------------------------------------------------------------- infra do servidor

def config(regime, lote_ms, lote_op):
    return {
        "base": "base",
        "bind": "127.0.0.1:%d" % PORTA,
        "token": TOKEN,
        "web": {"ligado": False},
        "recursos": {
            "durabilidade": regime,
            "lote_operacoes": lote_op,
            "lote_milissegundos": lote_ms,
        },
        "usuarios": [
            {
                "login": "adm", "nome": "Adriano", "id": 10, "nivel": "admin",
                "senha_hash": hash_da_senha("senha-do-adm"),
                "bases": {"*": {"ler": True, "inserir": True, "alterar": True,
                                "excluir": True, "criar": True,
                                "administrar": True, "verificar": True}},
            },
        ],
    }


def subir(cfg):
    shutil.rmtree(BASE, ignore_errors=True)
    os.makedirs(BASE, exist_ok=True)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump(cfg, f, indent=2)
    log = open(os.path.join(BASE, "servidor.log"), "a")
    p = subprocess.Popen([PHXSQLD], cwd=BASE, stdout=log,
                          stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    for _ in range(120):
        time.sleep(0.1)
        try:
            socket.create_connection(("127.0.0.1", PORTA), 0.3).close()
            return p
        except OSError:
            if p.poll() is not None:
                raise SystemExit("o servidor morreu ao subir -- veja %s" %
                                  os.path.join(BASE, "servidor.log"))
            continue
    p.kill()
    raise SystemExit("o servidor nao subiu na porta %d" % PORTA)


def derrubar(p):
    """SIGTERM no PID que ESTE script criou -- nunca pkill, nunca o PID de
    um agente vizinho."""
    if p is None or p.poll() is not None:
        return
    try:
        p.send_signal(signal.SIGTERM)
        p.wait(10)
    except (ProcessLookupError, subprocess.TimeoutExpired):
        try:
            p.kill()
            p.wait(5)
        except ProcessLookupError:
            pass


class Ligacao:
    def __init__(self, porta=PORTA, prazo=20):
        self.s = socket.create_connection(("127.0.0.1", porta))
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.s.settimeout(prazo)
        self.f = self.s.makefile("rwb")
        r = self.fala({"op": "login", "usuario": "adm", "senha": "senha-do-adm"})
        if not r.get("ok"):
            raise SystemExit("login: %s" % r)

    def fala(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        return json.loads(self.f.readline().decode())

    def ok(self, p):
        r = self.fala(p)
        if not r.get("ok"):
            raise SystemExit("%s: %s" % (p.get("op"), r.get("erro")))
        return r["resultado"]

    def fechar(self):
        # Fecha os DOIS -- `makefile()` segura o descritor por baixo, e
        # fechar so o soquete deixaria o fd aberto (a mesma licao do
        # `BULKINSERT` no CLAUDE.md).
        for c in (self.f, self.s):
            try:
                c.close()
            except OSError:
                pass


def criar_tabela(c, nome):
    c.ok({"op": "criar_tabela", "database": DB, "tabela": nome,
          "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                      {"nome": "v", "tipo": "Str(20)"}],
          "indices": [{"nome": "pk", "colunas": ["id"], "unico": True,
                       "primario": True}]})


def inserir(c, tabela, id_):
    c.ok({"op": "inserir", "database": DB, "tabela": tabela,
          "linha": {"id": id_, "v": "x"}})


def pendentes(c):
    r = c.ok({"op": "telemetria"})
    return r["servidor"]["gravacoes_pendentes"]


# ---------------------------------------------------------------------- strace

def anexar(pid, arquivo):
    """Anexa (NAO substitui) um `strace -f -y` no PID que ja esta de pe.
    `-f` e obrigatorio: sem ele, anexar no PID principal de um processo com
    varias threads so rastreia a thread principal -- e o `fsync` acontece na
    thread da conexao, nao nela."""
    p = subprocess.Popen(
        ["strace", "-f", "-y", "-ttt", "-e", "trace=fsync", "-o", arquivo,
         "-p", str(pid)],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.6)  # o ptrace-attach assentar em todas as threads existentes
    if p.poll() is not None:
        raise SystemExit("strace morreu ao anexar no pid %d" % pid)
    return p


def soltar(p):
    """SIGINT: o `strace` DETACHA de quem ele anexou (nao matou, porque nao
    foi quem criou) e sai. O processo tracado continua vivo e alheio."""
    p.send_signal(signal.SIGINT)
    try:
        p.wait(5)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait(5)
    time.sleep(0.2)  # o arquivo -o terminar de ser descarregado


# A linha COMPLETA: entrou e voltou sem ninguem no meio.
INTEIRA = re.compile(r"^\s*(\d+)\s+([\d.]+)\s+fsync\(\d+<([^>]+)>\)\s*=\s*(-?\d+)")
# A ENTRADA de uma chamada que o `strace` teve de partir em duas, porque outra
# thread entrou num `syscall` antes de esta voltar. So aqui esta o CAMINHO.
ABERTA = re.compile(r"^\s*(\d+)\s+([\d.]+)\s+fsync\(\d+<([^>]+)>\s+<unfinished")
# E a VOLTA dela, que traz o resultado e nao traz caminho nenhum.
FECHADA = re.compile(r"^\s*(\d+)\s+([\d.]+)\s+<\.\.\.\s+fsync\s+resumed>\)?\s*=\s*(-?\d+)")


def eventos_fsync(arquivo):
    """Os `fsync` do traco, INCLUSIVE os que o `strace` partiu em duas linhas.

    # Por que as tres expressoes, e nao uma

    Porque `strace -f` parte uma chamada em `<unfinished ...>` mais
    `<... fsync resumed>` sempre que outra thread entra num `syscall` antes de
    a primeira voltar -- e a partir de 05/09/2026 isso e' o caso NORMAL aqui: o
    fecho da janela sincroniza as K tabelas sujas ao mesmo tempo
    (`docs/CONCORRENCIA.md` 12.6). A expressao antiga exigia o `= 0` na MESMA
    linha do caminho, entao ela perdia toda chamada concorrente **em silencio**,
    e o script acusava «o strace foi solto antes de terminar» quando o strace
    tinha visto tudo.

    Medido no dia do conserto, com `strace -f -y -ttt` sobre um fecho de K=4:
    **480 `fsync` de verdade, 170 partidos em duas linhas, e a expressao antiga
    casava 310.** Um terco do traco sumia.

    A volta e' casada com a ida **pelo pid**, que e' o unico par confiavel: duas
    threads podem ter chamadas abertas ao mesmo tempo, mas cada uma so tem UMA
    aberta por vez -- um `syscall` bloqueia a thread que o chamou.
    """
    eventos = []
    if not os.path.exists(arquivo):
        return eventos
    abertas = {}
    with open(arquivo, "r", errors="replace") as f:
        for linha in f:
            m = INTEIRA.match(linha)
            if m:
                _, ts, caminho, ret = m.groups()
                eventos.append((float(ts), caminho, int(ret) == 0))
                continue
            m = ABERTA.match(linha)
            if m:
                pid, ts, caminho = m.groups()
                abertas[pid] = (float(ts), caminho)
                continue
            m = FECHADA.match(linha)
            if m:
                pid, _, ret = m.groups()
                ida = abertas.pop(pid, None)
                if ida is not None:
                    eventos.append((ida[0], ida[1], int(ret) == 0))
    # Chamada aberta e nunca fechada = o traco terminou no meio dela. Ela
    # ACONTECEU (o nucleo ja estava dentro do `fsync`), mas nao se sabe se
    # voltou bem -- e "nao se sabe" nao entra numa matriz de durabilidade como
    # se fosse sucesso.
    return eventos


def classificar(caminho):
    """(tabela, extensao) a partir do caminho do volume; `None` se nao for
    arquivo de tabela desta prova (ex.: `.pag`, fora do escopo aqui)."""
    base = os.path.basename(caminho)
    if "." not in base:
        return None
    nome, ext = base.rsplit(".", 1)
    if ext not in EXTENSOES:
        return None
    return nome, ext


def matriz(eventos):
    m = {}
    for _, caminho, ok in eventos:
        if not ok:
            continue
        c = classificar(caminho)
        if c is None:
            continue
        m[c] = m.get(c, 0) + 1
    return m


def matriz_json(m):
    """As chaves de `matriz()` sao tuplas `(tabela, extensao)` -- uteis para
    somar em Python, mas o `json` do stdlib nao aceita tupla como chave.
    Achata para `"tabela.extensao"` so na hora de gravar o resultado bruto."""
    return {"%s.%s" % k: v for k, v in m.items()}


def imprime_matriz(titulo, m, tabelas):
    print("\n  %s" % titulo)
    cab = "  %-10s" % "tabela"
    for e in EXTENSOES:
        cab += " %6s" % e
    print(cab)
    for t in tabelas:
        linha = "  %-10s" % t
        for e in EXTENSOES:
            linha += " %6d" % m.get((t, e), 0)
        print(linha)


# --------------------------------------------------------------------- cenarios

def rodar_controle_por_operacao():
    """CONTROLE 1: `por_operacao` sincroniza a cada escrita. Prova que o
    cano strace -> regex -> classificador VE um `.reg` sincronizado quando
    o codigo realmente pede um -- antes de confiar no silencio dos cenarios
    seguintes."""
    print("\n" + "=" * 78)
    print("CONTROLE 1: durabilidade por_operacao (toda escrita sincroniza tudo)")
    print("=" * 78)
    p = subir(config("por_operacao", 3_600_000, 1_000_000))
    try:
        c = Ligacao()
        c.ok({"op": "criar_database", "database": DB})
        criar_tabela(c, "controle")
        arq = os.path.join(AQUI, ".rastro-controle.log")
        tr = anexar(p.pid, arq)
        inserir(c, "controle", 1)
        soltar(tr)
        c.fechar()

        ev = eventos_fsync(arq)
        m = matriz(ev)
        imprime_matriz("fsync por arquivo -- UMA insercao, por_operacao", m,
                        ["controle"])
        reg = m.get(("controle", "reg"), 0)
        ok = reg >= 1
        print("\n  o instrumento VE fsync no .reg quando o codigo pede um? "
              "%s (%d fsync)" % ("SIM" if ok else "NAO -- PARE AQUI", reg))
        os.remove(arq)
        return ok, matriz_json(m)
    finally:
        derrubar(p)


def rodar_cenario_a():
    """(a): duas tabelas sujas ficam na janela, e uma TERCEIRA gravacao (em
    outra tabela) fecha a janela. A terceira tabela e o CONTROLE 2, na
    mesma sessao de strace."""
    print("\n" + "=" * 78)
    print("CENARIO (a): duas tabelas na mesma janela, fechada por uma "
          "gravacao seguinte")
    print("=" * 78)
    p = subir(config("por_lote", 3_600_000, 3))  # so por CONTAGEM, nunca por tempo
    try:
        c = Ligacao()
        c.ok({"op": "criar_database", "database": DB})
        for t in ("a", "b", "c"):
            criar_tabela(c, t)

        arq = os.path.join(AQUI, ".rastro-cenario-a.log")
        tr = anexar(p.pid, arq)

        inserir(c, "a", 1)
        n1 = pendentes(c)
        inserir(c, "b", 1)
        n2 = pendentes(c)
        inserir(c, "c", 1)  # a TERCEIRA gravacao do lote: fecha a janela AQUI
        n3 = pendentes(c)

        soltar(tr)
        c.fechar()

        ev = eventos_fsync(arq)
        m = matriz(ev)
        imprime_matriz("fsync por arquivo -- a e b ficaram sujas, c fechou "
                        "a janela", m, ["a", "b", "c"])
        print("\n  gravacoes_pendentes (a_cada=3): apos a=%d  apos b=%d  "
              "apos c=%d" % (n1, n2, n3))

        # PRE-CONDICAO: se a e b nao ficaram deferidas (1 e 2) antes de c
        # fechar (0), o cenario nao montou o que se pretendia medir -- a
        # mesma armadilha do (b), so que aqui o `ms` de 1 hora ja deveria
        # blindar contra ela. Falhar em silencio aqui seria publicar um
        # numero que nao mede o que diz medir.
        valido = (n1 == 1 and n2 == 2 and n3 == 0)
        if not valido:
            print("\n  *** PRECONDICAO FALHOU: esperava pendentes 1,2,0 e "
                  "veio %d,%d,%d -- o resultado abaixo NAO E CONFIAVEL ***"
                  % (n1, n2, n3))

        reg_a, reg_b, reg_c = (m.get((t, "reg"), 0) for t in "abc")
        outros_a = sum(m.get(("a", e), 0) for e in EXTENSOES if e != "reg")
        outros_b = sum(m.get(("b", e), 0) for e in EXTENSOES if e != "reg")
        print("\n  .reg sincronizado?  a=%s  b=%s  c=%s (c e quem disparou "
              "o fecho -- o controle 2)"
              % (bool(reg_a), bool(reg_b), bool(reg_c)))
        print("  os OUTROS 7 arquivos de a somam %d fsync; os de b somam %d "
              "fsync (prova que a reabertura aconteceu; so o .reg falta)"
              % (outros_a, outros_b))

        os.remove(arq)
        return dict(reg_a=reg_a, reg_b=reg_b, reg_c=reg_c,
                    outros_a=outros_a, outros_b=outros_b, matriz=matriz_json(m),
                    valido=valido)
    finally:
        derrubar(p)


def _tentativa_cenario_b(ms):
    """UMA tentativa do cenario (b), com o `ms` dado. Devolve `None` se a
    PRE-CONDICAO nao se sustentou (contaminacao pelo caminho do cenario a),
    caso em que a chamadora tenta de novo com folga maior -- em vez de
    publicar um numero que mede outra coisa.

    # A armadilha que esta funcao existe para evitar

    `desde` (o relogio da janela) so RESETA quando uma janela FECHA -- nunca
    por estar ocioso. Ele comeca a contar no `Janela::nova()`, ou seja, na
    SUBIDA DO SERVIDOR -- nao na primeira escrita. Se o tempo REAL entre a
    subida e as duas gravacoes de teste (DDL, login, o sono de 0,6 s para o
    `strace` assentar, ida-e-volta de rede sob uma maquina ocupada) passar de
    `lote_milissegundos`, a PRIMEIRA gravacao fecha a propria janela sozinha
    -- pelo MESMO mecanismo do cenario (a) (verificado por dentro do fonte:
    a thread da propria conexao grava o `.reg` cheio, na hora, no meio do
    `write()` da linha). Nesse caso o cenario (b) nunca chega a acontecer: o
    que sai medido e outro cenario (a), com etiqueta errada.

    Foi exatamente o que aconteceu na primeira rodada deste script, com
    `ms=300`: as DUAS tabelas saíram com `.reg` sincronizado, porque as duas
    gravacoes, cada uma, fecharam a PROPRIA janela por tempo decorrido desde
    a subida -- nao pelo relogio de fundo. O `strace` estava certo; a
    premissa do cenario que a rodada montou nao era a que o nome dizia.

    A pre-condicao que separa um cenario do outro: logo apos as DUAS
    gravacoes, sem MAIS NENHUMA chamada entre elas, `gravacoes_pendentes`
    tem de ser exatamente 2 -- as duas ainda sujas, nenhuma fechou nada.
    """
    p = subir(config("por_lote", ms, 1_000_000))  # so por TEMPO, nunca por contagem
    try:
        c = Ligacao()
        c.ok({"op": "criar_database", "database": DB})
        for t in ("d", "e"):
            criar_tabela(c, t)

        arq = os.path.join(AQUI, ".rastro-cenario-b.log")
        tr = anexar(p.pid, arq)

        t_escritas0 = time.time()
        inserir(c, "d", 1)
        inserir(c, "e", 1)
        n_antes = pendentes(c)
        t_escritas1 = time.time()
        print("\n  [ms=%d] as duas gravacoes (d, e) levaram %.0f ms; "
              "gravacoes_pendentes logo depois: %d (esperado: 2)"
              % (ms, (t_escritas1 - t_escritas0) * 1000, n_antes))

        if n_antes != 2:
            print("  *** PRECONDICAO FALHOU: uma ou as duas gravacoes ja "
                  "fecharam a PROPRIA janela por tempo decorrido desde a "
                  "subida do servidor (o mecanismo do cenario a) -- este "
                  "nao e o cenario (b). Tentando de novo com folga maior. "
                  "***")
            soltar(tr)
            c.fechar()
            os.remove(arq)
            return None

        # A pre-condicao segurou: as duas estao sujas, nenhuma fechou nada
        # ainda. Daqui para frente NINGUEM escreve -- so o relogio de fundo,
        # numa thread separada, pode fechar a janela agora. Poll em vez de
        # dormir o maximo: sai assim que fechar, sem depender de acertar um
        # numero de milissegundos.
        prazo = time.time() + ms / 1000.0 * 3 + 2.0
        n_depois = n_antes
        while time.time() < prazo:
            time.sleep(0.25)
            n_depois = pendentes(c)
            if n_depois == 0:
                break
        print("  gravacoes_pendentes apos a espera (sem NINGUEM escrever): "
              "%d" % n_depois)

        # `janela.fechar()` (que zera `pendentes`) roda ANTES do laco que de
        # fato reabre e sincroniza cada tabela suja, em
        # `descarregar_sujas_com` -- os dois na mesma thread do relogio, mas
        # nao no mesmo instante. `pendentes()==0` prova que o relogio
        # DECIDIU fechar; nao prova que ja terminou de sincronizar a
        # segunda tabela da lista. Um buffer aqui, ANTES de soltar o
        # `strace`, e o que separa isso de um falso "e nao sincronizou nada"
        # por termos parado de olhar cedo demais -- foi o que aconteceu
        # numa corrida deste script antes deste ajuste.
        if n_depois == 0:
            time.sleep(1.0)

        soltar(tr)
        c.fechar()

        if n_depois != 0:
            print("  *** o relogio de fundo NAO fechou a janela dentro do "
                  "prazo -- resultado inconclusivo, nao um `.reg` intacto. "
                  "***")
            os.remove(arq)
            return None

        ev = eventos_fsync(arq)
        m = matriz(ev)
        imprime_matriz("fsync por arquivo -- fecho pelo relogio, sem "
                        "escritor", m, ["d", "e"])

        reg_d, reg_e = (m.get((t, "reg"), 0) for t in "de")
        outros_d = sum(m.get(("d", e), 0) for e in EXTENSOES if e != "reg")
        outros_e = sum(m.get(("e", e), 0) for e in EXTENSOES if e != "reg")

        # Controle INTERNO como pre-condicao, nao so como comentario: se
        # QUALQUER uma das duas tabelas nao mostrar os outros sete
        # arquivos, o `strace` foi solto ANTES de `descarregar_sujas_com`
        # terminar a volta dela na lista -- e o "reg=0" dela nao prova nada,
        # porque nada dela foi visto. Refaz, em vez de publicar um zero que
        # pode ser o defeito ou pode ser corte cedo demais.
        if outros_d == 0 or outros_e == 0:
            print("\n  *** CONTROLE INTERNO FALHOU: %s ficou sem NENHUM "
                  "fsync visto (outros_d=%d outros_e=%d) -- o strace foi "
                  "solto antes de descarregar_sujas_com terminar. "
                  "Tentando de novo. ***"
                  % ("d" if outros_d == 0 else "e", outros_d, outros_e))
            os.remove(arq)
            return None

        print("\n  a janela fechou sozinha (pendentes voltou a 0)? True")
        print("  .reg sincronizado?  d=%s  e=%s" %
              (bool(reg_d), bool(reg_e)))
        print("  os OUTROS 7 arquivos de d somam %d fsync; os de e somam %d "
              "fsync (controle INTERNO: se o instrumento estivesse surdo "
              "para este PID/momento, estes tambem sairiam zero)"
              % (outros_d, outros_e))

        os.remove(arq)
        return dict(fechou=True, reg_d=reg_d, reg_e=reg_e,
                    outros_d=outros_d, outros_e=outros_e, matriz=matriz_json(m),
                    valido=True, ms=ms)
    finally:
        derrubar(p)


def rodar_cenario_b():
    """(b): duas tabelas sujas, e NINGUEM grava depois -- so o relogio de
    fundo (`ligar_relogio_de_gravacao`) pode fechar a janela.

    Tenta com `ms` crescente (2s, 5s, 10s) ate a PRE-CONDICAO se sustentar --
    ver `_tentativa_cenario_b` para o porque dela existir. Cada tentativa e
    um servidor NOVO (a janela e por processo)."""
    print("\n" + "=" * 78)
    print("CENARIO (b): janela fechada pelo relogio de fundo, ninguem "
          "gravando")
    print("=" * 78)
    for ms in (2_000, 5_000, 10_000):
        r = _tentativa_cenario_b(ms)
        if r is not None:
            return r
    print("\n  *** NAO CONSEGUI MONTAR O CENARIO (b) LIMPO em nenhuma das "
          "tentativas -- a maquina esta ocupada demais para isolar 'so o "
          "relogio fechou'. Nao ha resultado para publicar aqui. ***")
    return dict(fechou=False, reg_d=None, reg_e=None, outros_d=None,
                outros_e=None, matriz={}, valido=False)


# ------------------------------------------------------------------------ main

def main():
    ocupado = subprocess.run(["sh", os.path.join(RAIZ, "bancada",
                                                   "esta-medindo.sh")],
                              capture_output=True, text=True)
    if ocupado.returncode == 0:
        print("AVISO (cortesia): ha outra medicao em curso nesta maquina:")
        print(ocupado.stdout)
        print("Prosseguindo mesmo assim -- este script conta CHAMADAS DE "
              "SISTEMA, nao tempo, e contagem de syscall nao muda com a "
              "maquina ocupada.\n")

    ok_controle1, m_controle1 = rodar_controle_por_operacao()
    if not ok_controle1:
        print("\n*** O CONTROLE 1 FALHOU: o instrumento nao esta vendo "
              "fsync no .reg nem quando o codigo pede um. NENHUM resultado "
              "dos cenarios abaixo vale enquanto isso nao for corrigido. "
              "Parando. ***")
        sys.exit(1)

    r_a = rodar_cenario_a()
    r_b = rodar_cenario_b()

    print("\n" + "=" * 78)
    print("VEREDITO")
    print("=" * 78)

    defeito_a = (r_a["valido"] and r_a["reg_a"] == 0 and r_a["reg_b"] == 0 and
                 r_a["reg_c"] >= 1 and r_a["outros_a"] > 0 and
                 r_a["outros_b"] > 0)
    defeito_b = (r_b["valido"] and r_b["fechou"] and r_b["reg_d"] == 0 and
                 r_b["reg_e"] == 0 and r_b["outros_d"] > 0 and
                 r_b["outros_e"] > 0)

    print("\n  cenario (a) -- gravacao seguinte fecha a janela:")
    if not r_a["valido"]:
        print("    PRECONDICAO NAO SE SUSTENTOU -- ver aviso acima. Sem "
              "resultado confiavel para este cenario.")
    elif defeito_a:
        print("    CONFIRMADO: as tabelas sujas por ESCRITA ANTERIOR (a, b) "
              "reabrem e sincronizam sete arquivos, SEM o .reg. A tabela "
              "que disparou o fecho (c) sincroniza os oito, .reg incluso.")
    else:
        print("    NAO reproduziu com estes numeros: reg_a=%d reg_b=%d "
              "reg_c=%d" % (r_a["reg_a"], r_a["reg_b"], r_a["reg_c"]))

    print("\n  cenario (b) -- relogio de fundo fecha a janela, sem "
          "escritor:")
    if not r_b["valido"]:
        print("    NAO CONSEGUI MONTAR O CENARIO LIMPO nas tentativas "
              "feitas -- ver avisos acima. Sem resultado confiavel para "
              "este cenario (isto e diferente de 'nao reproduziu': o teste "
              "nao chegou a rodar sob a premissa que o nome promete).")
    elif defeito_b:
        print("    CONFIRMADO: as DUAS tabelas sujas (d, e) reabrem e "
              "sincronizam sete arquivos cada, SEM o .reg de nenhuma das "
              "duas -- e aqui NAO HA tabela-gatilho para se salvar, porque "
              "ninguem estava escrevendo quando o relogio fechou a janela.")
    else:
        print("    NAO reproduziu com estes numeros: fechou=%s reg_d=%s "
              "reg_e=%s" % (r_b["fechou"], r_b["reg_d"], r_b["reg_e"]))

    print()
    if defeito_a and defeito_b:
        print("  alcance medido nesta corrida: em cada janela fechada em "
              "`por_lote`, toda tabela suja que NAO seja a que disparou o "
              "fecho por contagem fica sem `fsync` no `.reg` -- e quando "
              "quem fecha e o relogio de fundo (ninguem escrevendo), "
              "NENHUMA tabela da janela tem o `.reg` sincronizado, "
              "contagem nenhuma incluida.")
    elif defeito_a and not defeito_b:
        print("  alcance medido nesta corrida: confirmado so para o "
              "caminho (a) -- gravacao seguinte fecha a janela e a(s) "
              "tabela(s) suja(s) por escrita ANTERIOR ficam sem `fsync` no "
              "`.reg`. O caminho (b) (relogio de fundo) nao pode ser "
              "confirmado NEM DERRUBADO nesta corrida.")
    else:
        print("  esta corrida NAO confirma o buraco em nenhum dos dois "
              "caminhos com os numeros medidos -- ver os avisos acima "
              "antes de concluir que ele nao existe.")

    resultado = {
        "controle_por_operacao": dict(ok=ok_controle1, matriz=m_controle1),
        "cenario_a": r_a,
        "cenario_b": r_b,
        "defeito_confirmado_a": defeito_a,
        "defeito_confirmado_b": defeito_b,
    }
    with open(RESULTADO, "w") as f:
        json.dump(resultado, f, indent=2, ensure_ascii=False)
    print("\n  resultado bruto salvo em %s" % RESULTADO)

    shutil.rmtree(BASE, ignore_errors=True)


if __name__ == "__main__":
    main()
