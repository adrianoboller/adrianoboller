#!/usr/bin/env python3
"""A BATERIA INTEIRA, num comando so.

    python3 phxsql/provar.py                 # tudo
    python3 phxsql/provar.py --construir      # compila antes (a regra do binario velho)
    python3 phxsql/provar.py --listar         # o que existe, e o que cada parte prova
    python3 phxsql/provar.py --so tela --so idiomas
    python3 phxsql/provar.py --sem jobs       # a mais demorada fica de fora
    python3 phxsql/provar.py --exigir-tudo    # pular vira reprovar

# Por que ele existe

As baterias sempre estiveram todas aqui -- so que eram OITO comandos, em tres
linguagens, espalhados por seis diretorios, e nenhum relatorio unico. Quem
chegava no projeto nao sabia o que rodar, e ninguem sabia dizer, num so lugar,
se o projeto estava verde.

Este script NAO refaz nenhuma delas. Cada bateria tem dono, ja foi provada e
continua rodando sozinha pelo comando dela; aqui elas sao chamadas, cronometradas
e somadas.

# O que ele imprime, e o que ele nao esconde

Cada parte sai com o veredito, o tempo e o log. **O que foi PULADO aparece no
relatorio com o motivo** -- bateria que esconde o que nao rodou mente por
omissao, e um relatorio que so mostra o verde e um relatorio que nao serve para
decidir nada.

Um pulo nao e uma falha, e o codigo de saida separa os dois:

    0   nada falhou (pode ter pulado; o relatorio diz o que e por que)
    1   alguma parte reprovou
    2   a bateria RECUSOU rodar -- binario velho ou ausente

`--exigir-tudo` transforma pulo em reprovacao, para quem quer o portao apertado.

E ha um quarto veredito por parte, alem de PASSOU / FALHOU / PULADA: **RODOU**,
das SONDAS. Sonda e o que imprime o que achou e sai zero sempre -- chamar isso
de «PASSOU» seria inventar um veredito que ninguem deu. Ver `classe` no
`parte()`.

# A recusa do binario velho, herdada

A pagina da interface esta EMBUTIDA no `phxsqld` (`include_str!`). Mexer em
`ui/` e nao recompilar faz metade destas baterias exercitar a pagina anterior e
passar verde numa correcao que ainda nao existe. Esta casa ja perdeu uma rodada
inteira de ganhos medindo com binario velho. A bateria de frontend ja recusava
rodar nesse caso; aqui a recusa vale para o comando inteiro, e vale tambem para
os `examples`, que o `cargo build --release` NAO recompila sozinho.

# Portas

Cada bateria abre as portas dela, documentadas no cabecalho de cada uma, e mata
so os processos que ela mesma criou, pelo PID. Este script nao abre porta
nenhuma -- ele so CONFERE, antes de comecar, se as portas de cada parte estao
livres. Ocupada, a parte e PULADA com o numero da porta no motivo, em vez de
reprovar por causa de um vizinho: ha outras frentes rodando nesta maquina, e
uma bateria que acusa a vizinha de defeito e pior que uma que nao roda.
"""

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import time

RAIZ = os.path.dirname(os.path.abspath(__file__))
RELEASE = os.path.join(RAIZ, "target", "release")
PHXSQLD = os.path.join(RELEASE, "phxsqld")
UI = os.path.join(RAIZ, "crates", "phxsql-server", "ui")
LOGS = os.path.join("/tmp", "phx-provar")

NODE_PLAYWRIGHT = os.environ.get(
    "PLAYWRIGHT", "/opt/node22/lib/node_modules/playwright/index.mjs")

CORES = {"ok": "\033[32m", "mal": "\033[31m", "pulo": "\033[33m",
         "fraco": "\033[90m", "fim": "\033[0m"}


def cor(nome, texto):
    return (CORES[nome] + texto + CORES["fim"]) if sys.stdout.isatty() else texto


def duracao(s):
    return "%dm%02ds" % (int(s // 60), int(s % 60)) if s >= 60 else "%.1fs" % s


def plural(n, um, muitos):
    """«1 pulada» e «2 puladas». O plural sai do numero, e nao da mao."""
    return "%d %s" % (n, um if n == 1 else muitos)


# --------------------------------------------------------------- requisitos
def porta_livre(*portas):
    """Motivo do pulo quando alguma das portas ja esta ocupada."""
    def confere():
        for p in portas:
            s = socket.socket()
            s.settimeout(0.3)
            try:
                s.connect(("127.0.0.1", p))
                s.close()
                return "a porta %d ja esta ocupada nesta maquina" % p
            except OSError:
                pass
            finally:
                s.close()
        return None
    return confere


def precisa_playwright():
    if not os.path.exists(NODE_PLAYWRIGHT):
        return ("nao achei o Playwright em %s -- ele nao entra no projeto "
                "(zero dependencia), e procurado onde estiver instalado; "
                "aponte a variavel PLAYWRIGHT" % NODE_PLAYWRIGHT)
    return None


def precisa_mysql():
    if not shutil.which("mysql"):
        return "nao ha cliente `mysql` nesta maquina"
    r = subprocess.run(["mysql", "crm", "-e", "SELECT 1"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        return ("precisa de um MySQL(R) vivo com o banco `crm` montado "
                "(docs/DBLINK.md)")
    return None


def precisa_do_odbc():
    """A `.so` do driver. O servidor da prova quem monta e o `bancada/odbc/provar.py`.

    Ate esta rodada esta parte era um PULO permanente: o passo do meio -- subir
    um phxsqld com token e usuario proprios -- estava escrito so em prosa no
    `docs/ODBC.md`, e passo em prosa nao entra em bateria. O `provar.py` de la
    e esse passo, e nada mais: ele chama as duas provas que ja existiam.
    """
    so = os.path.join(RELEASE, "libphxsql_odbc.so")
    if not os.path.exists(so):
        return ("falta %s -- cargo build --release -p phxsql-odbc" %
                os.path.relpath(so, RAIZ))
    return None


def precisa_de_root():
    if os.geteuid() != 0:
        return "monta tmpfs para provar disco cheio, e isso pede root"
    return None


# ------------------------------------------------------------------- partes
def parte(id, prova, cmd, cwd=RAIZ, requisitos=(), prazo=3600, nota="",
          antes=None, ambiente=None, classe="prova"):
    """Uma parte da bateria.

    `antes` e o passo de montagem que a parte exige e que nao e prova de nada --
    o `monta-bancada.py` da telemetria, por exemplo, que recorta o CSS do
    `index.html` para o conferidor exercitar as regras de HOJE. Ele entra no
    mesmo log e no mesmo tempo: montagem que falha reprova a parte, senao a
    parte reprovaria por um motivo que o relatorio nao mostra.

    `classe` separa PROVA de SONDA, e a separacao e honestidade, nao enfeite.
    Uma prova sabe reprovar: ela sai diferente de zero quando o que ela mede
    esta errado. Uma sonda imprime o que achou e sai zero SEMPRE -- e chamar
    isso de «PASSOU» seria inventar um veredito que ninguem deu. Sonda sai como
    RODOU, e o relatorio diz que ninguem julgou; sonda que ESTOURA continua
    reprovando, porque estourar e outra coisa.
    """
    return {"id": id, "prova": prova, "cmd": cmd, "cwd": cwd,
            "requisitos": list(requisitos), "prazo": prazo, "nota": nota,
            "antes": antes, "ambiente": ambiente or {}, "classe": classe}


PARTES = [
    parte("motor", "o motor, o protocolo e os portoes -- os testes de unidade "
          "e de integracao do workspace",
          ["cargo", "test", "--workspace", "--offline"], prazo=2400),

    parte("guardas", "que cada teste ainda PEGA o defeito que o motivou: repoe "
          "o defeito do catalogo e confere que o teste cai",
          [sys.executable, "bancada/guardas/provar-guardas.py"], prazo=3600),

    parte("tela", "a interface contra o servidor de verdade: 120 telas "
          "percorridas, o CSS global, o contraste, a primeira pintura",
          ["node", "testes-web/bateria.mjs", "--porta", "6950"],
          requisitos=[precisa_playwright, porta_livre(6950, 6951)], prazo=2400),

    parte("idiomas", "o caminho do idioma de ponta a ponta -- e o comportamento "
          "velho, que e o que mais importa numa guarda nova",
          ["node", "testes-web/prova-idiomas.mjs", "--porta", "6952"],
          requisitos=[precisa_playwright, porta_livre(6952, 6953)], prazo=1200),

    parte("ponta-a-ponta", "os seis itens do dono pelo SOQUETE: database, "
          "tabelas, UUID v7, gatilhos, procedimentos e carga",
          [sys.executable, "bancada/bateria/prova-bateria.py", "--tela"],
          requisitos=[porta_livre(6300, 6301)], prazo=2400),

    parte("cifra-do-fio", "o aperto de mao da porta de dados contra um cliente "
          "escrito DE NOVO em Python: o cliente velho, o tunel, o pino, o "
          "registro repetido e o fio cortado",
          [sys.executable, "bancada/cifra-do-fio/prova.py"],
          requisitos=[porta_livre(7210, 7211)], prazo=900),
    parte("alter", "acrescentar coluna numa tabela com dado, pelo soquete: o "
          "rowid preservado, o backup, e a replica que ainda nao alterou",
          [sys.executable, "bancada/alter/provar.py"],
          requisitos=[porta_livre(7150, 7152)], prazo=900),

    parte("rotinas", "gatilhos e procedimentos pelo soquete, com o SIGNAL, o "
          "lote, o reinicio e a tabela sem gatilho",
          [sys.executable, "bancada/rotinas/prova-rotinas.py"],
          requisitos=[porta_livre(5301, 5701)], prazo=1200),

    parte("profiler", "a REDACAO do Profiler por soquete: vinte pedidos "
          "torcidos, e a sentinela procurada no anel e no .txt",
          [sys.executable, "bancada/profiler/sonda.py"],
          requisitos=[porta_livre(6251)], prazo=900),

    parte("queda-na-exclusao", "que a janela de durabilidade da exclusao nao "
          "perde linha numa queda do PROCESSO: 150 exclusoes pelo soquete e "
          "um SIGKILL no meio da janela, nos dois modos",
          [sys.executable, "bancada/exclusao/prova-da-queda.py"],
          requisitos=[porta_livre(7100)], prazo=900),

    parte("telemetria-desenho", "o painel de bolhas por MEDIDA: rotulo na "
          "esfera, alvo de clique, contraste nos dois temas",
          ["node", "bancada/telemetria/conferir-desenho.mjs"],
          antes=[sys.executable, "bancada/telemetria/monta-bancada.py"],
          requisitos=[precisa_playwright], prazo=900),

    parte("telemetria-interacao", "clicar na bolha menor com o painel em "
          "movimento, descer de nivel, voltar pela trilha",
          ["node", "bancada/telemetria/conferir-interacao.mjs"],
          antes=[sys.executable, "bancada/telemetria/monta-bancada.py"],
          # As capturas saem do repositorio: retrato guardado envelhece calado.
          ambiente={"TLM_CAPTURAS": os.path.join(LOGS, "telemetria")},
          requisitos=[precisa_playwright], prazo=900),

    parte("telemetria-cores", "as cores configuraveis do painel, exercitando: "
          "paleta escolhida na tela, salva, e conferida no painel",
          ["node", "bancada/telemetria/prova-das-cores.mjs"],
          requisitos=[precisa_playwright, porta_livre(6600, 6601)], prazo=1800),

    parte("cluster", "eleicao e promocao automatica com tres servidores no ar "
          "e um SMTP falso capturando os avisos",
          [sys.executable, "bancada/cluster/provar.py"],
          requisitos=[porta_livre(5310, 5311, 5312, 5316)], prazo=1800),

    parte("replicacao", "os quatro modos de replicacao por soquete, com o "
          "comportamento velho dos Config_exemplo no fim",
          [sys.executable, "bancada/replicacao/modos.py"],
          requisitos=[porta_livre(*range(5330, 5340))], prazo=2400),

    parte("trava", "a trava de dados contra a leitura de rede: corte "
          "silencioso, alcance, queda de conexao e o abraco do bidirecional",
          [sys.executable, "bancada/replicacao/trava.py"],
          requisitos=[porta_livre(*range(7050, 7056))], prazo=1200,
          nota="o estagio do corte silencioso sonda por 40 s de proposito"),

    parte("jobs", "o aviso de jobs por e-mail, com SMTP falso -- e o servidor "
          "SEM bloco de e-mail, que nao pode mandar nada",
          [sys.executable, "bancada/jobs/prova-avisos.py"],
          requisitos=[porta_livre(5303, 5703)], prazo=1200,
          nota="espera de verdade a volta do vigia (60 s): leva uns 3 minutos"),

    # SONDA, e nao prova -- ver `classe` no `parte()`. Ela imprime
    # «ACEITOU -- devia ter recusado» em vez de reprovar, e dar-lhe um codigo
    # de saida exigiria decidir o que conta como falha em cada um dos seis
    # itens: isso e desenho do Profiler, e nao do orquestrador. Fica RODOU, com
    # o buraco declarado, em vez de PASSOU inventado.
    parte("profiler-disco", "o arquivo .txt do Profiler contra o SISTEMA "
          "OPERACIONAL: disco cheio, somente-leitura, reinicio, rotacao",
          [sys.executable, "bancada/profiler/sonda-log.py"],
          requisitos=[precisa_de_root, porta_livre(6253)], prazo=900,
          classe="sonda"),

    parte("dblink", "a sincronia de tabelas primas contra um MySQL(R) de "
          "verdade: quem vence o conflito, e que exclusao nao viaja",
          [sys.executable, "bancada/dblink/prova-sincronia.py"],
          requisitos=[precisa_mysql], prazo=900),

    parte("odbc", "a ABI do driver ODBC pelo ctypes, sem passar pelo "
          "unixODBC -- a mesma .so que o gerenciador carregaria",
          [sys.executable, "bancada/odbc/provar.py", "--porta", "6954"],
          requisitos=[precisa_do_odbc, porta_livre(6954)], prazo=900),
]


# --------------------------------------------------- a recusa do binario velho
def mais_novo(diretorio):
    novo, quem = 0.0, ""
    for pasta, _, arquivos in os.walk(diretorio):
        for a in arquivos:
            p = os.path.join(pasta, a)
            try:
                m = os.stat(p).st_mtime
            except OSError:
                continue
            if m > novo:
                novo, quem = m, p
    return novo, quem


def conferir_binario():
    """Devolve a lista de motivos para RECUSAR a rodada inteira."""
    motivos = []
    if not os.path.exists(PHXSQLD):
        return ["nao achei %s\n     cargo build --release" %
                os.path.relpath(PHXSQLD, RAIZ)]
    bin_ms = os.stat(PHXSQLD).st_mtime
    ui_ms, quem = mais_novo(UI)
    if ui_ms > bin_ms:
        motivos.append(
            "o phxsqld e mais VELHO que %s.\n"
            "     A pagina vem do include_str!, entao a bateria exercitaria a "
            "versao anterior.\n"
            "     cargo build --release -p phxsql-server --bin phxsqld"
            % os.path.relpath(quem, RAIZ))
    # A outra metade da mesma licao: `cargo build --release` NAO recompila os
    # examples, e a bancada chama `target/release/examples/carga` direto. Uma
    # rodada inteira de ganhos ja ficou invisivel por causa disto.
    carga = os.path.join(RELEASE, "examples", "carga")
    fonte, _ = mais_novo(os.path.join(RAIZ, "crates", "phxsql-store", "examples"))
    if os.path.exists(carga) and fonte > os.stat(carga).st_mtime:
        motivos.append(
            "o example `carga` e mais VELHO que o fonte dele.\n"
            "     cargo build --release --examples -p phxsql-store")
    return motivos


def construir():
    passos = [
        ["cargo", "build", "--release", "--offline"],
        ["cargo", "build", "--release", "--offline", "--examples", "-p", "phxsql-store"],
    ]
    for cmd in passos:
        print("  " + " ".join(cmd))
        r = subprocess.run(cmd, cwd=RAIZ)
        if r.returncode != 0:
            return r.returncode
    return 0


# ---------------------------------------------------------------- a rodada
def rodar(p):
    os.makedirs(LOGS, exist_ok=True)
    for d in p["ambiente"].values():
        os.makedirs(d, exist_ok=True)
    amb = dict(os.environ)
    amb.update(p["ambiente"])
    log = os.path.join(LOGS, p["id"] + ".log")
    inicio = time.time()
    with open(log, "w", encoding="utf-8") as f:
        if p["antes"]:
            f.write("$ " + " ".join(p["antes"]) + "\n")
            f.flush()
            r = subprocess.run(p["antes"], cwd=p["cwd"], env=amb, stdout=f,
                               stderr=subprocess.STDOUT)
            if r.returncode != 0:
                return ("FALHOU", time.time() - inicio, log,
                        "o passo de montagem falhou (codigo %d)" % r.returncode)
        f.write("$ " + " ".join(p["cmd"]) + "\n\n")
        f.flush()
        try:
            proc = subprocess.Popen(p["cmd"], cwd=p["cwd"], env=amb, stdout=f,
                                    stderr=subprocess.STDOUT,
                                    start_new_session=True)
        except FileNotFoundError as e:
            return "FALHOU", time.time() - inicio, log, "nao consegui rodar: %s" % e
        try:
            codigo = proc.wait(timeout=p["prazo"])
        except subprocess.TimeoutExpired:
            try:
                os.killpg(os.getpgid(proc.pid), 9)
            except OSError:
                pass
            proc.wait()
            return ("FALHOU", time.time() - inicio, log,
                    "estourou o prazo de %d s" % p["prazo"])
    gasto = time.time() - inicio
    if codigo == 0:
        return ("PASSOU" if p["classe"] == "prova" else "RODOU"), gasto, log, ""
    return "FALHOU", gasto, log, "saiu com codigo %d" % codigo


def ultimas(log, quantas=18):
    try:
        linhas = open(log, encoding="utf-8", errors="replace").read().splitlines()
    except OSError:
        return []
    return linhas[-quantas:]


def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--so", action="append", default=[])
    ap.add_argument("--sem", action="append", default=[])
    ap.add_argument("--listar", action="store_true")
    ap.add_argument("--construir", action="store_true",
                    help="compila release e examples antes de rodar")
    ap.add_argument("--exigir-tudo", action="store_true",
                    help="pular passa a contar como reprovar")
    ap.add_argument("--json", default=None, help="grava o resultado neste arquivo")
    opc = ap.parse_args()

    escolhidas = [p for p in PARTES
                  if (not opc.so or any(s in p["id"] for s in opc.so))
                  and not any(s in p["id"] for s in opc.sem)]

    if opc.listar:
        print("%d partes:\n" % len(PARTES))
        for p in PARTES:
            print("  %-22s %s" % (p["id"], p["prova"]))
            print("      $ %s" % " ".join(p["cmd"]))
            if p["nota"]:
                print("      · %s" % p["nota"])
        return 0

    print("=" * 78)
    print(" PhxSql — a bateria inteira")
    print(" %s" % time.strftime("%Y-%m-%d %H:%M:%S"))
    print("=" * 78)

    if opc.construir:
        print("\ncompilando antes de medir (a regra do binario velho):")
        codigo = construir()
        if codigo != 0:
            print(cor("mal", "\na compilacao falhou; nada foi medido."))
            return 2

    recusas = conferir_binario()
    if recusas:
        print(cor("mal", "\nRECUSADO — a bateria nao roda com binario velho:"))
        for m in recusas:
            print("   · " + m)
        print("\n   (ou rode com --construir, que compila antes)")
        return 2

    # Os pulos sao decididos ANTES de qualquer rodada, para o relatorio poder
    # dizer o tamanho da bateria de verdade ja na primeira linha.
    plano = []
    for p in escolhidas:
        motivo = None
        for r in p["requisitos"]:
            motivo = r()
            if motivo:
                break
        plano.append((p, motivo))
    vao_rodar = [p for p, m in plano if not m]
    print("\n%d partes: %s, %s\n"
          % (len(plano),
             plural(len(vao_rodar), "roda", "rodam"),
             plural(len(plano) - len(vao_rodar), "pulada", "puladas")))

    resultados = []
    comeco = time.time()
    for p, motivo in plano:
        if motivo:
            print("  %-22s %s  %s" % (p["id"], cor("pulo", "PULADA"), motivo))
            resultados.append({"id": p["id"], "veredito": "PULADA",
                               "segundos": 0.0, "motivo": motivo, "log": None})
            continue
        print("  %-22s %s" % (p["id"], cor("fraco", "rodando…")), end="", flush=True)
        veredito, gasto, log, extra = rodar(p)
        pintura = {"PASSOU": "ok", "RODOU": "fraco"}.get(veredito, "mal")
        print("\r  %-22s %-6s %8s   %s"
              % (p["id"], cor(pintura, veredito), duracao(gasto),
                 cor("fraco", log)))
        if extra:
            print("      %s" % cor(pintura, extra))
        if veredito == "FALHOU":
            for l in ultimas(log):
                print("      | " + l[:160])
        resultados.append({"id": p["id"], "veredito": veredito,
                           "segundos": round(gasto, 1), "motivo": extra,
                           "log": log})
    total = time.time() - comeco

    passou = [r for r in resultados if r["veredito"] == "PASSOU"]
    falhou = [r for r in resultados if r["veredito"] == "FALHOU"]
    pulou = [r for r in resultados if r["veredito"] == "PULADA"]
    sondou = [r for r in resultados if r["veredito"] == "RODOU"]

    print("\n" + "=" * 78)
    if pulou:
        print("O QUE NAO RODOU, e por que — bateria que esconde isto mente por omissao:")
        for r in pulou:
            print("  · %-22s %s" % (r["id"], r["motivo"]))
        print()
    if sondou:
        print("SONDA, e nao prova — rodou e imprimiu, e ninguém julgou:")
        for r in sondou:
            print("  · %-22s sai zero sempre; o veredito é de quem lê o log"
                  % r["id"])
        print()
    print(" %d partes: %s, %s, %s, %s   —   %s no total"
          % (len(resultados),
             cor("ok", plural(len(passou), "passou", "passaram")),
             cor("mal", plural(len(falhou), "falhou", "falharam")) if falhou
             else "0 falharam",
             cor("pulo", plural(len(pulou), "pulada", "puladas")) if pulou
             else "0 puladas",
             plural(len(sondou), "sonda", "sondas"),
             duracao(total)))
    if falhou:
        print(" reprovaram: " + ", ".join(r["id"] for r in falhou))
    print(" logs completos em %s" % LOGS)
    print("=" * 78)

    if opc.json:
        with open(opc.json, "w", encoding="utf-8") as f:
            json.dump({"quando": time.strftime("%Y-%m-%dT%H:%M:%S"),
                       "segundos": round(total, 1),
                       "partes": resultados}, f, ensure_ascii=False, indent=2)

    if falhou:
        return 1
    if pulou and opc.exigir_tudo:
        return 1
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        # `provar.py --listar | head` fecha o cano no meio: sair calado e a
        # unica resposta honesta -- o erro seria do `head`, e nao da bateria.
        os._exit(0)
