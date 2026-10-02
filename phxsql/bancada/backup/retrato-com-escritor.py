#!/usr/bin/env python3
"""O BACKUP COM ESCRITOR CONCORRENTE -- pedido 513, passo 2, medido pelo soquete.

    cargo build --release -p phxsql-server --bin phxsqld
    python3 bancada/backup/retrato-com-escritor.py [--mb 1024] [--tabelas 20]
        [--binario target/release/phxsqld] [--rotulo duas_passadas] [--voltas 3]

# O que este medidor existe para decidir

O contrato (`docs/propostas/207-e-513p2-contrato-01-10-2026.md` §513.6, item 6)
diz que o numero da bancada decide o passo 2a -- e dispara o 2b (o rastro
fisico, H-C) se o cenario B reprovar. Nada aqui promete; aqui se mede.

Um banco de N MB em T tabelas; uma thread gravando `inserir`/`atualizar`/
`excluir` SEM PARAR durante todo o backup em arvore; uma thread lendo. Dois
cenarios, no mesmo banco:

  A -- o escritor em DUAS tabelas pequenas (~10% dos bytes);
  B -- o escritor na MAIOR tabela.

De cada um: a duracao do backup, a fase 2 (ms e bytes, do proprio servidor),
a espera MAXIMA e o p99 de um `inserir`, a leitura maxima, e a prova de que o
retrato restaura integro: `conferir_backup`, `restaurar_backup` com outro nome
e o `varrer` de cada tabela restaurada contando o que o indice conta.

# O que aprova e o que dispara

  * aprova o 2a: no cenario A, espera maxima do `inserir` <= fase 2 + 10% e
    <= 1/10 da do passo 1 (o mesmo banco com o binario de ANTES, rotulo
    `passo_1`), faixas min-max sem se cruzar (pedido 155);
  * dispara o 2b (H-C): no cenario B, fase 2 > 50% da copia inteira.

Cada numero traz mediana E faixa min-max das `--voltas`. O `resultados.json`
guarda um retrato por ROTULO (mescla por nome, nunca sobrescreve o vizinho),
com a data e o N medidos -- a pagina dos testes le daqui.

NUNCA usa pkill: o servidor morre pelo PID que este script guardou.
"""
import argparse
import hashlib
import json
import os
import shutil
import socket
import statistics
import subprocess
import sys
import threading
import time

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))
RESULTADOS = os.path.join(AQUI, "resultados.json")
# Faixa 7340-7349 desta bancada: nao colide com a `quorum/` (7330-7339).
PORTA = 7340
TOKEN = "retrato-513"
DB = "loja"
# Cada linha leva ~2 KB de texto: 1 GB sao ~500.000 linhas, que o
# `inserir_lote` carrega em minutos e nao em horas.
TEXTO = "x" * 2000


class Fio:
    """Uma conexao QUENTE, aberta uma vez: medir com conexao nova mediria o
    aperto de mao, que o cliente de verdade paga uma vez so."""

    def __init__(self, prazo=30):
        fim = time.monotonic() + prazo
        while True:
            try:
                self.s = socket.create_connection(("127.0.0.1", PORTA), timeout=3600)
                break
            except OSError:
                if time.monotonic() > fim:
                    raise
                time.sleep(0.1)
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.f = self.s.makefile("rwb")

    def __call__(self, p):
        p.setdefault("token", TOKEN)
        self.f.write((json.dumps(p) + "\n").encode())
        self.f.flush()
        linha = self.f.readline()
        if not linha:
            raise ConnectionError("o servidor fechou a conexao")
        return json.loads(linha.decode())

    def fechar(self):
        # `makefile` segura o descritor: fechar so o soquete deixaria o fd
        # aberto e o servidor nunca veria o fim (licao do BULKINSERT).
        try:
            self.f.close()
        finally:
            self.s.close()


def corpo(r, o_que=""):
    if not r.get("ok"):
        raise SystemExit(f"pedido recusado {o_que}: {r}")
    return r.get("resultado", {})


def med(x):
    return round(statistics.median(x), 3) if x else None


def faixa(x):
    return [round(min(x), 3), round(max(x), 3)] if x else None


def p99(x):
    if not x:
        return None
    xs = sorted(x)
    return round(xs[min(len(xs) - 1, int(len(xs) * 0.99))], 3)


def subir(binario, base):
    os.makedirs(base, exist_ok=True)
    cfg = {
        "base": os.path.join(base, "dados"),
        # bancada de teste, NAO cliente do produto: fala em claro.
        "bind": f"127.0.0.1:{PORTA}", "cifra_fio": {"exigir": False},
        "token": TOKEN, "web": {"ligado": False},
    }
    caminho = os.path.join(base, "config.json")
    with open(caminho, "w") as f:
        json.dump(cfg, f, indent=2)
    log = open(os.path.join(base, "servidor.log"), "a")
    return subprocess.Popen([binario, "--config", caminho], cwd=base, stdout=log,
                            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)


def matar(p):
    if p is None:
        return
    try:
        p.terminate()
        p.wait(timeout=20)
    except Exception:
        try:
            p.kill()
            p.wait(timeout=5)
        except Exception:
            pass


def tabela(n):
    return f"t{n:02d}"


def criar_banco(fio, tabelas, mb):
    """T tabelas; a maior leva metade dos bytes, duas pequenas ~5% cada, e o
    resto divide o que sobra. Devolve {tabela: linhas}."""
    corpo(fio({"op": "criar_database", "database": DB}), "criar_database")
    linhas_total = (mb * 1024 * 1024) // (len(TEXTO) + 40)
    fatias = {tabela(0): 0.5, tabela(1): 0.05, tabela(2): 0.05}
    resto = (1.0 - sum(fatias.values())) / max(1, tabelas - 3)
    for n in range(3, tabelas):
        fatias[tabela(n)] = resto
    linhas = {}
    for t, fatia in fatias.items():
        corpo(fio({"op": "criar_tabela", "database": DB, "tabela": t,
                   "colunas": [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                               {"nome": "txt", "tipo": "Str(2100)"}],
                   "indices": [{"nome": "porId", "colunas": ["id"], "unico": True}]}),
              f"criar_tabela {t}")
        quantas = max(10, int(linhas_total * fatia))
        feito = 0
        while feito < quantas:
            lote = [{"id": feito + i + 1, "txt": TEXTO}
                    for i in range(min(2000, quantas - feito))]
            r = corpo(fio({"op": "inserir_lote", "database": DB, "tabela": t,
                           "linhas": lote}), f"inserir_lote {t}")
            feito += r.get("gravadas", len(lote))
        linhas[t] = feito
    # A primeira leitura depois da carga cura o cabecalho do `.log` de cada
    # tabela (a janela `por_lote`): sem isso o primeiro `varrer` sob o backup
    # pagaria a exclusiva -- o limite dito do passo 1, que nao e o que se mede.
    for t in linhas:
        corpo(fio({"op": "varrer", "database": DB, "tabela": t, "max": 1}), f"varrer {t}")
    return linhas


def bytes_da_raiz(base):
    total = 0
    for dirpath, _, nomes in os.walk(os.path.join(base, "dados")):
        for n in nomes:
            try:
                total += os.path.getsize(os.path.join(dirpath, n))
            except OSError:
                pass
    return total


class Escritor(threading.Thread):
    """Grava sem parar nas tabelas dadas: `inserir`, `atualizar` da linha que
    acabou de entrar, `excluir` de uma mais velha. Mede cada `inserir` E cada
    escrita: a espera da fase 2 cai na operacao que estiver na vez, e medir
    so' o `inserir` deixava o maximo cair no `atualizar` sem ninguem ver
    (achado na primeira corrida: 2.490 ms numa volta, 44 ms na seguinte)."""

    def __init__(self, tabelas, proximo_id):
        super().__init__(daemon=True)
        self.tabelas = tabelas
        self.ids = dict(proximo_id)
        self.parar = threading.Event()
        self.inserir_ms = []
        self.escrita_ms = []
        # Quando cada escrita COMECOU (perf_counter), para separar a espera
        # da fase 1 da espera da fase 2 (pedido 513, disputa de E/S).
        self.escrita_t = []
        self.feitos = 0
        self.erro = None

    def run(self):
        fio = Fio()
        try:
            while not self.parar.is_set():
                for t in self.tabelas:
                    i = self.ids[t] + 1
                    self.ids[t] = i
                    t0 = time.perf_counter()
                    r = fio({"op": "inserir", "database": DB, "tabela": t,
                             "valores": {"id": i, "txt": TEXTO}})
                    gasto = (time.perf_counter() - t0) * 1000
                    self.inserir_ms.append(gasto)
                    self.escrita_ms.append(gasto)
                    self.escrita_t.append(t0)
                    if not r.get("ok"):
                        self.erro = r
                        return
                    rowid = r["resultado"]["rowid"]
                    t0 = time.perf_counter()
                    r = fio({"op": "atualizar", "database": DB, "tabela": t, "rowid": rowid,
                             "valores": {"id": i, "txt": TEXTO[:1000]}})
                    self.escrita_ms.append((time.perf_counter() - t0) * 1000)
                    self.escrita_t.append(t0)
                    if not r.get("ok"):
                        self.erro = r
                        return
                    if rowid > 5:
                        t0 = time.perf_counter()
                        fio({"op": "excluir", "database": DB, "tabela": t, "rowid": rowid - 5})
                        self.escrita_ms.append((time.perf_counter() - t0) * 1000)
                        self.escrita_t.append(t0)
                    self.feitos += 1
        finally:
            fio.fechar()


class Leitor(threading.Thread):
    def __init__(self, t):
        super().__init__(daemon=True)
        self.t = t
        self.parar = threading.Event()
        self.ms = []

    def run(self):
        fio = Fio()
        try:
            while not self.parar.is_set():
                t0 = time.perf_counter()
                fio({"op": "varrer", "database": DB, "tabela": self.t, "max": 10})
                self.ms.append((time.perf_counter() - t0) * 1000)
                time.sleep(0.02)
        finally:
            fio.fechar()


def sha_da_arvore(raiz):
    """O SHA-256 de cada `.reg` debaixo de `raiz`, pelo nome relativo."""
    saida = {}
    for dirpath, _, nomes in os.walk(raiz):
        for n in sorted(nomes):
            if n.endswith(".reg"):
                caminho = os.path.join(dirpath, n)
                h = hashlib.sha256()
                with open(caminho, "rb") as f:
                    for pedaco in iter(lambda: f.read(1 << 20), b""):
                        h.update(pedaco)
                saida[os.path.relpath(caminho, raiz)] = h.hexdigest()
    return saida


def provar_o_retrato(fio, destino, base, linhas, vez):
    """O backup confere, restaura com outro nome, e cada tabela restaurada
    abre e conta pelo indice o que o `varrer` conta -- um retrato consistente.
    Devolve o que falhou (lista vazia = integro)."""
    falhas = []
    c = corpo(fio({"op": "conferir_backup", "destino": destino}), "conferir_backup")
    if not c.get("integro"):
        falhas.append(f"conferir_backup: {c.get('divergencias')}")
    para = f"restaurado{vez}"
    r = fio({"op": "restaurar_backup", "origem": destino, "de": DB, "database": para})
    if not r.get("ok"):
        falhas.append(f"restaurar_backup: {r}")
        return falhas
    for t in linhas:
        v = fio({"op": "varrer", "database": para, "tabela": t, "max": 1})
        if not v.get("ok"):
            falhas.append(f"varrer {para}.{t}: {v}")
            continue
        res = v["resultado"]
        # O que o indice diz que existe tem de bater com o que a varredura
        # enxerga -- e a tabela de uma copia pela metade nao abre.
        if res.get("registros", -1) < 0:
            falhas.append(f"{para}.{t}: sem contagem: {res}")
    return falhas


def conferir_so(fio, destino):
    c = corpo(fio({"op": "conferir_backup", "destino": destino}), "conferir_backup")
    return [] if c.get("integro") else [f"conferir_backup: {c.get('divergencias')}"]


def cenario(nome, fio, base, linhas, escritor_em, leitor_em, voltas, rotulo, guardados):
    medidas = []
    for v in range(voltas):
        destino = os.path.join(base, f"bkp-{nome}-{v}")
        shutil.rmtree(destino, ignore_errors=True)
        proximo = {t: linhas[t] + 1_000_000 * (v + 1) for t in escritor_em}
        esc = Escritor(escritor_em, proximo)
        lei = Leitor(leitor_em)
        esc.start()
        lei.start()
        time.sleep(1.0)  # o escritor ja em regime
        antes = len(esc.inserir_ms)
        antes_escritas = len(esc.escrita_ms)
        t0 = time.perf_counter()
        r = corpo(fio({"op": "backup", "destino": destino}), "backup")
        total_ms = (time.perf_counter() - t0) * 1000
        time.sleep(0.5)
        esc.parar.set()
        lei.parar.set()
        esc.join()
        lei.join()
        if esc.erro:
            raise SystemExit(f"o escritor caiu durante o backup: {esc.erro}")
        durante = esc.inserir_ms[antes:]
        escritas = esc.escrita_ms[antes_escritas:]
        # Aqui so' o `conferir`; a restauracao inteira (uma copia do banco)
        # fica para o FIM, com o backup da primeira volta guardado: nao ha
        # operacao que apague um database, e um database restaurado no meio
        # entraria no backup do cenario seguinte.
        falhas = conferir_so(fio, destino)
        fase_2 = r.get("fase_2") or {}
        # A janela da fase 1 e' [t0, t0 + fase_1_ms]: o que comecou nela
        # disputa E/S com a copia; o que comecou depois pega a fase 2 e o
        # fim. Pelo relogio do cliente (o servidor so' devolve duracao).
        f1 = (r.get("fase_1_ms") or 0) / 1000.0
        ts = esc.escrita_t[antes_escritas:]
        na_f1 = [g for g, t in zip(escritas, ts) if t0 <= t <= t0 + f1]
        pos_f1 = [g for g, t in zip(escritas, ts) if t > t0 + f1]
        m = {
            "escrita_fase1_max_ms": round(max(na_f1), 3) if na_f1 else None,
            "escrita_fase1_p99_ms": p99(na_f1),
            "escrita_fase1_mediana_ms": med(na_f1),
            "escrita_fase1_n": len(na_f1),
            "escrita_pos_fase1_max_ms": round(max(pos_f1), 3) if pos_f1 else None,
            "backup_ms": round(total_ms, 1),
            "servidor_ms": r.get("ms"),
            "modo": r.get("modo", "retrato_inteiro"),
            "fase_1_ms": r.get("fase_1_ms"),
            "fase_2_ms": fase_2.get("ms"),
            "fase_2_arquivos": fase_2.get("arquivos"),
            "fase_2_bytes": fase_2.get("bytes"),
            "fase_2_fora_do_bloqueio": fase_2.get("fora_do_bloqueio"),
            "escrita_max_ms": round(max(escritas), 3) if escritas else None,
            "escrita_p99_ms": p99(escritas),
            "inserir_max_ms": round(max(durante), 3) if durante else None,
            "inserir_p99_ms": p99(durante),
            "inserir_mediana_ms": med(durante),
            "inserir_durante": len(durante),
            "escritas_durante": esc.feitos,
            "leitura_max_ms": round(max(lei.ms), 3) if lei.ms else None,
            "retrato_integro": not falhas,
            "falhas": falhas,
        }
        print(f"  [{rotulo} {nome} volta {v + 1}] backup {m['backup_ms']} ms, modo {m['modo']}, "
              f"fase 2 {m['fase_2_ms']} ms / {m['fase_2_bytes']} B, escrita max "
              f"{m['escrita_max_ms']} ms, inserir max {m['inserir_max_ms']} ms p99 "
              f"{m['inserir_p99_ms']} ms ({len(durante)} durante), "
              f"leitura max {m['leitura_max_ms']} ms, integro {m['retrato_integro']}; "
              f"fase 1: max {m['escrita_fase1_max_ms']} p99 {m['escrita_fase1_p99_ms']} "
              f"med {m['escrita_fase1_mediana_ms']} (n={m['escrita_fase1_n']}), "
              f"pos-fase-1 max {m['escrita_pos_fase1_max_ms']}")
        if falhas:
            print("    FALHAS:", falhas)
        medidas.append(m)
        if v == 0:
            guardados[nome] = destino
        else:
            shutil.rmtree(destino, ignore_errors=True)
    chaves = ["backup_ms", "fase_2_ms", "fase_2_bytes", "escrita_max_ms", "escrita_p99_ms",
              "escrita_fase1_max_ms", "escrita_fase1_p99_ms", "escrita_fase1_mediana_ms",
              "escrita_pos_fase1_max_ms",
              "inserir_max_ms", "inserir_p99_ms", "inserir_mediana_ms", "leitura_max_ms"]
    resumo = {"voltas": medidas}
    for k in chaves:
        xs = [m[k] for m in medidas if m.get(k) is not None]
        resumo[k] = {"mediana": med(xs), "faixa": faixa(xs)}
    resumo["modo"] = medidas[0]["modo"]
    resumo["retrato_integro"] = all(m["retrato_integro"] for m in medidas)
    return resumo


def gravar(rotulo, retrato):
    tudo = {}
    if os.path.exists(RESULTADOS):
        with open(RESULTADOS) as f:
            tudo = json.load(f)
    tudo[rotulo] = retrato
    with open(RESULTADOS, "w") as f:
        json.dump(tudo, f, indent=2, ensure_ascii=False)
    print(f"gravado em {RESULTADOS} ({rotulo})")


def veredito(tudo):
    """O aceite do contrato, lido do `resultados.json` inteiro."""
    d = tudo.get("duas_passadas")
    p1 = tudo.get("passo_1")
    if not d:
        return "sem rotulo duas_passadas: nada a decidir"
    linhas = []
    a, b = d["cenarios"]["A"], d["cenarios"]["B"]
    f2 = a["fase_2_ms"]["mediana"] or 0
    # A espera que conta e a de QUALQUER escrita (a fase 2 cai na operacao que
    # estiver na vez); o `inserir` sozinho fica ao lado, porque e o que o
    # contrato nomeou.
    esp = a["escrita_max_ms"]["mediana"] or 0
    linhas.append(f"A: escrita max {esp} ms (inserir max {a['inserir_max_ms']['mediana']} ms) x "
                  f"fase 2 {f2} ms + 10% = {round(f2 * 1.1, 1)} ms -> "
                  f"{'OK' if esp <= f2 * 1.1 + 1 else 'REPROVA'}")
    if p1:
        pa = p1["cenarios"]["A"]["escrita_max_ms"]
        cruza = a["escrita_max_ms"]["faixa"][1] >= pa["faixa"][0]
        linhas.append(f"A: escrita max {esp} ms x passo 1 {pa['mediana']} ms (1/10 = "
                      f"{round(pa['mediana'] / 10, 1)}) -> "
                      f"{'OK' if esp <= pa['mediana'] / 10 else 'REPROVA'}"
                      f"{' (faixas se cruzam: sem vencedor, pedido 155)' if cruza else ''}")
    else:
        linhas.append("A: sem rotulo passo_1 -- rode com --binario <antes> --rotulo passo_1")
    fb = b["fase_2_ms"]["mediana"] or 0
    tb = b["backup_ms"]["mediana"] or 1
    linhas.append(f"B: fase 2 {fb} ms = {round(100 * fb / tb, 1)}% do backup -> "
                  f"{'DISPARA o 2b (H-C)' if fb > tb * 0.5 else 'nao dispara o 2b'}")
    return "\n".join(linhas)


def main():
    ap = argparse.ArgumentParser()
    # 512 e o que cabe nesta maquina com o banco, dois backups e duas
    # restauracoes ao mesmo tempo; o contrato pede 1 GB -- passe --mb 1024
    # onde houver disco.
    ap.add_argument("--mb", type=int, default=512)
    ap.add_argument("--tabelas", type=int, default=20)
    ap.add_argument("--voltas", type=int, default=3)
    ap.add_argument("--binario", default=os.path.join(RAIZ, "target", "release", "phxsqld"))
    ap.add_argument("--rotulo", default="duas_passadas")
    ap.add_argument("--base", default=None, help="onde fica o banco (padrao: pasta temporaria)")
    ap.add_argument("--so-veredito", action="store_true")
    a = ap.parse_args()
    if a.so_veredito:
        with open(RESULTADOS) as f:
            print(veredito(json.load(f)))
        return
    base = a.base or os.path.join("/tmp", f"phx-retrato-513-{os.getpid()}")
    shutil.rmtree(base, ignore_errors=True)
    p = subir(a.binario, base)
    try:
        fio = Fio()
        t0 = time.perf_counter()
        linhas = criar_banco(fio, a.tabelas, a.mb)
        carga_s = time.perf_counter() - t0
        tamanho = bytes_da_raiz(base)
        print(f"banco: {a.tabelas} tabelas, {sum(linhas.values())} linhas, "
              f"{tamanho / 1048576:.0f} MiB em disco, carregado em {carga_s:.0f} s")
        pequenas = [tabela(1), tabela(2)]
        maior = [tabela(0)]
        guardados = {}
        print("cenario A: escritor em duas tabelas pequenas")
        A = cenario("A", fio, base, linhas, pequenas, tabela(3), a.voltas, a.rotulo, guardados)
        print("cenario B: escritor na maior tabela")
        B = cenario("B", fio, base, linhas, maior, tabela(3), a.voltas, a.rotulo, guardados)
        # A prova do retrato, com o backup da primeira volta de cada cenario:
        # restaura com outro nome e abre cada tabela restaurada.
        for nome, resumo in (("A", A), ("B", B)):
            falhas = provar_o_retrato(fio, guardados[nome], base, linhas, nome)
            resumo["restaurado_integro"] = not falhas
            resumo["falhas_da_restauracao"] = falhas
            print(f"  [{a.rotulo} {nome}] restaurado integro: {not falhas}"
                  + (f" FALHAS: {falhas}" if falhas else ""))
            shutil.rmtree(guardados[nome], ignore_errors=True)
        retrato = {
            "quando": time.strftime("%Y-%m-%d %H:%M:%S"),
            "binario": a.binario,
            "mb_pedidos": a.mb,
            "bytes_em_disco": tamanho,
            "tabelas": a.tabelas,
            "linhas": sum(linhas.values()),
            "voltas": a.voltas,
            "cenarios": {"A": A, "B": B},
        }
        gravar(a.rotulo, retrato)
        with open(RESULTADOS) as f:
            print(veredito(json.load(f)))
    finally:
        matar(p)
        shutil.rmtree(base, ignore_errors=True)


if __name__ == "__main__":
    main()
