#!/usr/bin/env python3
"""CRUD medido: colmeia (prototipo PSHV) x SQLite(R) x PhxSql Padrao.

Pergunta do dono (16/09/2026): "Compare o tipo colmeia com o sqlite e phxsql -
Insert, update, delete e select."

    cargo build --release --example custo-da-colmeia -p phxsql-store
    python3 bancada/colmeia/medir-crud.py                 # tudo: 2 regimes x 3 N
    python3 bancada/colmeia/medir-crud.py --rapido        # fumaca: 300 ops, 3 reps
    python3 bancada/colmeia/medir-crud.py --regimes sistema --tamanhos 10000

Este script e o MAESTRO: o lado Rust (colmeia viva + Padrao) e o modo `crud`
de `crates/phxsql-store/examples/custo-da-colmeia.rs`, rodado por processo; o
lado SQLite(R) roda AQUI, no `sqlite3` da biblioteca padrao (extensao em C,
zero dependencia). Os tres lados recebem OS MESMOS dados: o exemplo grava o
conjunto num JSON e este script o le -- "mesmos dados" por construcao, nao por
duas implementacoes do gerador que alguem teria de provar iguais.

O contrato de repeticao e o mesmo nos tres lados (`cronometrar`): reps+1
passadas de `ops` operacoes, a primeira e aquecimento e nao entra; ler mede
sobre a mesma base; inserir/atualizar/excluir REFAZEM a base de N antes de
cada passada, para toda passada partir do mesmo estado.

A durabilidade e casada como em `bancada/sqlite/medir.py` (bancada C):

    por_operacao  PhxSql sincronizar() apos cada escrita
                  colmeia fsync das celulas + fsync do bloco base
                  SQLite synchronous=FULL, journal DELETE, autocommit
    sistema       ninguem espera o disco: sem sincronizar, sem fsync,
                  SQLite synchronous=OFF (autocommit -- 1 transacao por op,
                  como 1 write por op dos outros dois)

Portao de maquina parada: antes de cada secao cronometrada exige carga de
1 min < 1,0 e nenhum cargo/rustc vivo; espera ate 20 min; nao mata nada.
A medicao e detectavel pelo `bancada/esta-medindo.sh` pelos dois crivos dele
(este script em `bancada/*.py`, o exemplo em `target/release/examples/`).
"""

import datetime
import json
import os
import platform
import shutil
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]  # phxsql/
EXEMPLO = RAIZ / "target" / "release" / "examples" / "custo-da-colmeia"
SAIDA = AQUI / "resultados-crud.json"
PARCIAL = AQUI / "resultados-crud.parcial.json"

TAMANHOS = [1_000, 10_000, 100_000]
REGIMES = ["por_operacao", "sistema"]
OPS = 4_000
REPS = 15

# O que cada lado FAZ por operacao -- vai no JSON para o numero nunca viajar
# sem o trabalho ao lado dele (regra 4 da bancada, ja errada duas vezes).
O_QUE_CADA_LADO_FAZ = {
    "colmeia": (
        "protótipo PSHV em Rust, arquivo inteiro em memória; ler = descida da "
        "árvore de células por busca binária (sem alocar); escrever = append da "
        "célula nova + cópia de caminho (folha, grupo, raiz nascem de novo no "
        "fim) + bloco base de 128 B reescrito no lugar. SEM índice separado, "
        "SEM diário, SEM transação, SEM lixeira, SEM tipos: célula velha vira "
        "lixo inalcançável até um compactar explícito"
    ),
    "padrao": (
        "phxsql_store::table::Table de verdade (chave Str(64) + valor Str(128), "
        "índice único porChave, cache do .ndx de 1.000.000 páginas); ler = "
        "buscar no .ndx + ler no .reg com decodificação em Vec<Value>; inserir "
        "= .reg (append, ordem de digitação) + .ndx + diário .log + descritor; "
        "atualizar = buscar + regravar a linha no lugar + índice + diário; "
        "excluir = buscar + conferir filhas + copiar a linha para a lixeira "
        ".trash + índice + diário. Em por_operacao, sincronizar() depois de "
        "toda escrita (fsync do que a operação escreveu, mais os dois do "
        ".ndx; até 16/09/2026, de cada arquivo aberto da tabela — pedido 258)"
    ),
    "sqlite": (
        "sqlite3 da biblioteca padrão do Python (extensão em C), tabela rowid "
        "com chave TEXT PRIMARY KEY (b-tree da tabela + b-tree do índice "
        "automático da chave), valor TEXT, journal_mode=DELETE, cache_size de "
        "256 MiB; cada operação é uma transação (autocommit): cria o -journal, "
        "grava, apaga o -journal. O tempo inclui a chamada Python->C; a linha "
        "de base do Python está publicada ao lado"
    ),
}

CASAMENTO = {
    "por_operacao": (
        "Padrão: sincronizar() após cada escrita; colmeia: fsync após as "
        "células e fsync após o bloco base; SQLite: synchronous=FULL + "
        "autocommit (fsync do journal e do banco em cada transação)"
    ),
    "sistema": (
        "Padrão: nunca sincroniza (lixeira na janela); colmeia: só write; "
        "SQLite: synchronous=OFF + autocommit (1 transação por operação, "
        "sem fsync)"
    ),
}


# ----------------------------------------------------------------- maquina


def carga_1min():
    with open("/proc/loadavg") as f:
        return float(f.read().split()[0])


def compiladores_vivos():
    """PIDs de cargo/rustc vivos, pelo `comm` -- nunca por `pgrep -f`, que se
    acha (a armadilha que o `esta-medindo.sh` documenta)."""
    vivos = []
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            with open(f"/proc/{d}/comm") as f:
                comm = f.read().strip()
        except OSError:
            continue
        if comm in ("cargo", "rustc"):
            vivos.append(int(d))
    return vivos


SEM_ESPERAR = False  # `--sem-esperar`: so para a prova de fumaca do proprio script


ESPERA_MAX_S = 1200  # `--espera-max <min>`: o limite por secao; 20 min de fabrica


def esperar_slot(limite_s=None, passo_s=30, consultar_portao=True):
    """Espera a maquina parar. Devolve (carga, segundos esperados) ou None se
    o slot nunca ficou livre -- e ai a secao fica NAO MEDIDA, nao estimada.

    `consultar_portao` so na PRIMEIRA secao da corrida: o protocolo da casa e
    "quem chegou primeiro mede, quem chega depois espera no portao". Uma
    bancada que chegou depois desta fica esperando por ela -- e se esta
    voltasse a consultar o portao antes de cada secao, veria a outra (que esta
    so esperando) e esperaria por ela: impasse entre dois processos reais, com
    dois cronometros de 20 min. Pago em 16/09/2026 contra `chutar-a-tomada.py`.
    Carga e compiladores continuam conferidos a cada secao."""
    if limite_s is None:
        limite_s = ESPERA_MAX_S
    inicio = time.monotonic()
    while True:
        carga = carga_1min()
        vivos = compiladores_vivos()
        alheia = medicao_alheia() if consultar_portao else ""
        if SEM_ESPERAR or (carga < 1.0 and not vivos and not alheia):
            return carga, round(time.monotonic() - inicio, 1)
        if time.monotonic() - inicio > limite_s:
            return None
        print(
            f"    [slot] carga {carga:.2f}, compiladores vivos {vivos}, "
            f"outra medicao {alheia or 'nao'} -- espero {passo_s}s",
            flush=True,
        )
        time.sleep(passo_s)


def medicao_alheia():
    """Ha OUTRA bancada medindo? Pergunta ao portao da casa, que exclui a
    propria linhagem (este script nao se acha) e conta o resto. Duas bancadas
    na mesma maquina reprovam uma a outra -- ja aconteceu tres vezes num dia.

    Um refinamento que custou um impasse em 16/09/2026: o crivo 2 do portao
    (caminho do script no cmdline) casa tambem o INVOLUCRO `bash -c` de quem
    ainda vai chamar a bancada -- um laco de `sleep 30` esperando o portao
    ficar livre de "colmeia" tinha `bancada/tomada/...py` no proprio cmdline.
    Ele esperava por mim e eu por ele. Invólucro que so MENCIONA a bancada
    nao e bancada: so conta PID cujo executavel nao e um shell. O portao da
    casa fica como esta (e provado no item 0b da bateria); o crivo a mais e
    daqui."""
    portao = RAIZ / "bancada" / "esta-medindo.sh"
    if not portao.exists():
        return ""
    r = subprocess.run([str(portao)], capture_output=True, text=True)
    if r.returncode != 0:
        return ""
    for linha in r.stdout.splitlines():
        pid = linha.split("\t", 1)[0]
        try:
            exe = os.path.basename(os.readlink(f"/proc/{pid}/exe"))
        except OSError:
            continue
        if exe in ("bash", "sh", "dash", "zsh"):
            continue
        return linha[:80]
    return ""


def agora_utc():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")


def mtime_utc(caminho):
    t = datetime.datetime.fromtimestamp(os.path.getmtime(caminho), datetime.timezone.utc)
    return t.strftime("%Y-%m-%d %H:%M UTC")


# --------------------------------------------------------------- estatistica


def medida(amostras):
    return {
        "mediana_us": round(statistics.median(amostras), 4),
        "faixa_us": [round(min(amostras), 4), round(max(amostras), 4)],
    }


def cronometrar(reps, ops, refazer, preparar, executar, fechar):
    """O MESMO contrato do `cronometrar` do exemplo Rust: reps+1 passadas, a
    primeira descartada; `preparar` de novo antes de cada passada quando
    `refazer`. Devolve (medida, estado da ultima passada)."""
    estado = None
    amostras = []
    for passada in range(reps + 1):
        if estado is None or refazer:
            if estado is not None:
                fechar(estado)
            estado = preparar()
        t0 = time.perf_counter()
        for k in range(ops):
            executar(estado, k)
        dt = time.perf_counter() - t0
        if passada > 0:
            amostras.append(dt * 1e6 / ops)
    return medida(amostras), estado


def se_cruzam(a, b):
    return a["faixa_us"][1] >= b["faixa_us"][0] and b["faixa_us"][1] >= a["faixa_us"][0]


def julgar(lados):
    """Razoes par a par e o vencedor -- so quando a faixa dele fica INTEIRA
    abaixo das faixas dos outros dois. Dentro do ruido nao se declara nada."""
    nomes = [n for n in ("colmeia", "padrao", "sqlite") if n in lados]
    pares = {}
    for i, a in enumerate(nomes):
        for b in nomes[i + 1 :]:
            pares[f"{b}_sobre_{a}"] = {
                "razao": round(lados[b]["mediana_us"] / lados[a]["mediana_us"], 4),
                "faixas_se_cruzam": se_cruzam(lados[a], lados[b]),
            }
    if len(nomes) < 3:
        return pares, "incompleto"
    melhor = min(nomes, key=lambda n: lados[n]["mediana_us"])
    for outro in nomes:
        if outro != melhor and se_cruzam(lados[melhor], lados[outro]):
            return pares, "dentro_do_ruido"
    return pares, f"{melhor}_ganha"


# ------------------------------------------------------------------ o Rust


def lado_rust(regime, n, ops, reps, dir_tmp, so_operacao="", so_lado=""):
    cmd = [str(EXEMPLO), "crud", regime, str(n), str(ops), str(reps), str(dir_tmp)]
    if so_operacao:
        cmd.append(so_operacao)
        if so_lado:
            cmd.append(so_lado)
    subprocess.run(cmd, cwd=RAIZ, check=True)
    with open(dir_tmp / f"rust-{regime}-{n}.json") as f:
        rust = json.load(f)
    with open(dir_tmp / f"dados-{n}.json") as f:
        dados = json.load(f)
    return rust, dados


# ---------------------------------------------------------------- o SQLite

SEL = "SELECT valor FROM config WHERE chave=?"
INS = "INSERT INTO config (chave, valor) VALUES (?,?)"
UPD = "UPDATE config SET valor=? WHERE chave=?"
DEL = "DELETE FROM config WHERE chave=?"


def abre_sqlite(caminho, regime):
    for extra in ("", "-journal", "-wal", "-shm"):
        Path(str(caminho) + extra).unlink(missing_ok=True)
    con = sqlite3.connect(str(caminho), isolation_level=None)
    con.execute("PRAGMA journal_mode=DELETE")
    con.execute("PRAGMA synchronous=" + ("FULL" if regime == "por_operacao" else "OFF"))
    # Quente como os outros dois: o Padrao tem 1.000.000 paginas de cache e a
    # colmeia o arquivo inteiro em memoria; 2 MiB (o padrao do SQLite) nao
    # cabe a base de 100.000 e mediria o cache, nao o motor.
    con.execute("PRAGMA cache_size=-262144")
    con.execute("CREATE TABLE config (chave TEXT PRIMARY KEY, valor TEXT NOT NULL)")
    return con


def preparar_sqlite(caminho, regime, base):
    con = abre_sqlite(caminho, regime)
    # A carga inicial e UMA transacao -- como a colmeia nasce de um
    # gravar_colmeia em bloco e o Padrao de N inserir + um sincronizar.
    con.execute("BEGIN")
    con.executemany(INS, base)
    con.execute("COMMIT")
    return con


def fechar_sqlite(con):
    con.close()


def lado_sqlite(regime, n, ops, reps, dados, dir_tmp, so_operacao=""):
    caminho = dir_tmp / f"sqlite-{regime}-{n}.db"
    base = [tuple(p) for p in dados["base"]]
    novos = [tuple(p) for p in dados["novos"]]
    leitura = dados["alvos_leitura"]
    distintos = dados["alvos_distintos"]
    valores_novos = dados["valores_novos"]
    valor_de = dict(base)
    saida = {}

    def preparar():
        return preparar_sqlite(caminho, regime, base)

    for operacao in ("ler", "inserir", "atualizar", "excluir"):
        if so_operacao and operacao != so_operacao:
            continue
        ops_desta = len(distintos) if operacao in ("atualizar", "excluir") else ops
        if operacao == "ler":

            def executar(con, k):
                return con.execute(SEL, (leitura[k],)).fetchone()

            m, con = cronometrar(reps, ops_desta, False, preparar, executar, fechar_sqlite)
            for ch in leitura:
                assert con.execute(SEL, (ch,)).fetchone()[0] == valor_de[ch]
            esperado = n
        elif operacao == "inserir":

            def executar(con, k):
                con.execute(INS, novos[k])

            m, con = cronometrar(reps, ops_desta, True, preparar, executar, fechar_sqlite)
            for ch, v in novos:
                assert con.execute(SEL, (ch,)).fetchone()[0] == v, f"sqlite: inserir nao gravou {ch}"
            esperado = n + ops_desta
        elif operacao == "atualizar":

            def executar(con, k):
                con.execute(UPD, (valores_novos[k], distintos[k]))

            m, con = cronometrar(reps, ops_desta, True, preparar, executar, fechar_sqlite)
            for ch, v in zip(distintos, valores_novos):
                assert con.execute(SEL, (ch,)).fetchone()[0] == v, f"sqlite: atualizar nao trocou {ch}"
            esperado = n
        else:

            def executar(con, k):
                con.execute(DEL, (distintos[k],))

            m, con = cronometrar(reps, ops_desta, True, preparar, executar, fechar_sqlite)
            for ch in distintos:
                assert con.execute(SEL, (ch,)).fetchone() is None, f"sqlite: excluir deixou {ch}"
            esperado = n - ops_desta
        total = con.execute("SELECT count(*) FROM config").fetchone()[0]
        assert total == esperado, f"sqlite: contagem depois de {operacao}: {total} != {esperado}"
        fechar_sqlite(con)
        m["ops_por_repeticao"] = ops_desta
        saida[operacao] = m
        print(
            f"    sqlite   {operacao:9s} mediana {m['mediana_us']:9.3f} us/op"
            f"  faixa [{m['faixa_us'][0]:.3f}; {m['faixa_us'][1]:.3f}]",
            flush=True,
        )
    for extra in ("", "-journal"):
        Path(str(caminho) + extra).unlink(missing_ok=True)
    return saida


def linha_de_base_python(dir_tmp, ops, reps):
    """Quanto do numero do SQLite e Python: o MESMO laco e a MESMA forma de
    chamada, sem tabela nenhuma no meio. Publica-se; nunca se subtrai."""
    caminho = dir_tmp / "base-python.db"

    def preparar():
        return abre_sqlite(caminho, "sistema")

    def execute_select(con, k):
        return con.execute("SELECT ?", (k,)).fetchone()

    def laco_vazio(con, k):
        return k

    m_sel, con = cronometrar(reps, ops, False, preparar, execute_select, fechar_sqlite)
    fechar_sqlite(con)
    m_vazio, con = cronometrar(reps, ops, False, preparar, laco_vazio, fechar_sqlite)
    fechar_sqlite(con)
    for extra in ("", "-journal"):
        Path(str(caminho) + extra).unlink(missing_ok=True)
    return {
        "o_que_e": (
            "custo por chamada do lado Python, com o mesmo laco `cronometrar` "
            "das medidas: `con.execute('SELECT ?', (k,)).fetchone()` nao toca "
            "tabela nenhuma; o laco vazio e so a chamada da funcao por iteracao"
        ),
        "execute_select_param_us": m_sel,
        "laco_vazio_us": m_vazio,
        "ops_por_repeticao": ops,
        "repeticoes": reps,
    }


# ------------------------------------------------------- syscalls por operacao

SYSCALLS = "fsync,fdatasync,write,pwrite64,openat,unlink,unlinkat"


def contar_strace(cmd, cwd):
    """Contagem de chamadas de `strace -c -f` para o comando -- so as contagens,
    o tempo sob strace nao serve para nada."""
    with tempfile.NamedTemporaryFile(prefix="strace-", suffix=".txt", delete=False) as f:
        arq = f.name
    try:
        subprocess.run(
            ["strace", "-f", "-c", "-e", "trace=" + SYSCALLS, "-o", arq] + cmd,
            cwd=cwd,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        contagem = {}
        with open(arq) as f:
            for linha in f:
                partes = linha.split()
                # "% time  seconds  usecs/call  calls  errors  syscall"
                if len(partes) >= 5 and partes[-1] in SYSCALLS.split(","):
                    contagem[partes[-1]] = int(partes[3])
        return contagem
    finally:
        os.unlink(arq)


def syscalls_por_op(regime, dir_tmp, n=1000, pouco=200, muito=1000):
    """Por DIFERENCA entre duas corridas (1000 e 200 ops, reps=1 -> 2
    passadas): o que a montagem da base e o JSON custam cancela, e sobra o
    que UMA operacao chama. E medida, nao leitura de codigo."""
    if not shutil.which("strace"):
        return {"nao_medido": "strace ausente"}
    saida = {}
    for operacao in ("ler", "inserir", "atualizar", "excluir"):
        saida[operacao] = {}
        for lado in ("colmeia", "padrao"):
            contagens = []
            for ops in (pouco, muito):
                d = dir_tmp / f"sys-{lado}-{ops}"
                d.mkdir(exist_ok=True)
                cmd = [str(EXEMPLO), "crud", regime, str(n), str(ops), "1", str(d), operacao, lado]
                contagens.append(contar_strace(cmd, RAIZ))
            saida[operacao][lado] = _diferenca(contagens, muito - pouco, 2)
        contagens = []
        for ops in (pouco, muito):
            # O conjunto de dados e o que o Rust acabou de gravar para ESTE
            # numero de ops (`sys-colmeia-<ops>`): nunca o da corrida
            # principal, que tem outro tamanho de lista.
            cmd = [
                sys.executable,
                str(Path(__file__).resolve()),
                "--sonda-sqlite",
                regime,
                str(n),
                str(ops),
                str(dir_tmp / f"sys-colmeia-{ops}"),
                operacao,
            ]
            contagens.append(contar_strace(cmd, RAIZ))
        saida[operacao]["sqlite"] = _diferenca(contagens, muito - pouco, 2)
    return saida


def _diferenca(contagens, delta_ops, passadas):
    c0, c1 = contagens
    return {
        s: round((c1.get(s, 0) - c0.get(s, 0)) / (delta_ops * passadas), 2)
        for s in SYSCALLS.split(",")
    }


def sonda_sqlite(regime, n, ops, dir_dados, operacao):
    """Modo interno: UMA operacao no SQLite, para o strace de fora contar. Os
    dados vem do `dados-<n>.json` de `dir_dados`, gerado pelo exemplo Rust
    com ESTE `ops`; se ainda nao existe, pede ao exemplo que o gere."""
    d = Path(dir_dados)
    d.mkdir(parents=True, exist_ok=True)
    arq = d / f"dados-{n}.json"
    if not arq.exists():
        subprocess.run(
            [str(EXEMPLO), "crud", regime, str(n), str(ops), "1", str(d), "ler", "colmeia"],
            cwd=RAIZ,
            check=True,
            stdout=subprocess.DEVNULL,
        )
    with open(arq) as f:
        dados = json.load(f)
    assert len(dados["alvos_leitura"]) >= ops, "dados-<n>.json gerado com menos ops que a sonda pede"
    lado_sqlite(regime, n, ops, 1, dados, d, so_operacao=operacao)


# ------------------------------------------------------------------ markdown


def br(x, casas=3):
    """Numero em grafia brasileira, para os documentos."""
    return f"{x:,.{casas}f}".replace(",", "X").replace(".", ",").replace("X", ".")


def celula(lado):
    if not isinstance(lado, dict):
        return "NÃO MEDIDA"
    return f"{br(lado['mediana_us'])} [{br(lado['faixa_us'][0])}; {br(lado['faixa_us'][1])}]"


def markdown(caminho=SAIDA):
    """As tabelas dos documentos, geradas do JSON -- todo numero visivel sai
    daqui, nunca de memoria (`docs/propostas/colmeia.md`, `docs/STATUS-TIPOS.md`)."""
    with open(caminho) as f:
        r = json.load(f)
    linhas = []
    _p = linhas.append
    caminho = Path(caminho).resolve()
    fonte = caminho.relative_to(RAIZ) if caminho.is_relative_to(RAIZ) else caminho
    _p(f"<!-- gerado por `python3 bancada/colmeia/medir-crud.py --markdown` de `{fonte}` ({r['medido_em']}) -->")
    _p("")
    _p(
        f"Medido em {r['medido_em']} ({r['maquina']}; SQLite {r['sqlite']}, Python {r['python']}; "
        f"binário de {r['binario']['modificado_em']}); carga(1 min) no início {br(r['carga_1min_no_inicio'], 2)}; "
        f"{r['ops_por_repeticao']} operações por repetição, {r['repeticoes']} repetições, mediana e faixa min–máx em µs/op."
    )
    b = r.get("linha_de_base_python")
    if b:
        _p("")
        _p(
            f"Linha de base do Python (mesmo laço, sem tabela): `execute('SELECT ?')` + `fetchone()` = "
            f"**{br(b['execute_select_param_us']['mediana_us'])} µs/op** "
            f"[{br(b['execute_select_param_us']['faixa_us'][0])}; {br(b['execute_select_param_us']['faixa_us'][1])}]; "
            f"laço vazio = {br(b['laco_vazio_us']['mediana_us'])} µs/op. Publicada, nunca subtraída."
        )
    for regime in r["regimes"]:
        _p("")
        _p(f"**Regime `{regime}`** — {r['casamento_de_durabilidade'][regime]}.")
        _p("")
        _p("| operação | N | colmeia (µs/op) | Padrão (µs/op) | SQLite (µs/op) | Padrão/colmeia | SQLite/colmeia | SQLite/Padrão | vencedor |")
        _p("|---|---:|---:|---:|---:|---:|---:|---:|---|")
        for c in r["combinacoes"]:
            if c["regime"] != regime:
                continue
            lados = c["lados"]
            pares = c["pares"]

            def razao(nome):
                p = pares.get(nome)
                if not p:
                    return "—"
                return f"{br(p['razao'], 2)}×" + (" (cruza)" if p["faixas_se_cruzam"] else "")

            n_ops = c["ops_por_repeticao"]
            op = c["operacao"] + (f" ({n_ops} distintas)" if c["operacao"] in ("atualizar", "excluir") and n_ops != r["ops_por_repeticao"] else "")
            n_txt = f"{c['n']:,}".replace(",", ".")
            _p(
                f"| {op} | {n_txt} | {celula(lados.get('colmeia'))} | {celula(lados.get('padrao'))} | "
                f"{celula(lados.get('sqlite'))} | {razao('padrao_sobre_colmeia')} | {razao('sqlite_sobre_colmeia')} | "
                f"{razao('sqlite_sobre_padrao')} | {c['vencedor'].replace('_', ' ')} |"
            )
        bytes_linhas = [
            (c["operacao"], c["n"], c["lados"]["colmeia"].get("bytes_anexados_por_op"))
            for c in r["combinacoes"]
            if c["regime"] == regime and isinstance(c["lados"].get("colmeia"), dict) and c["lados"]["colmeia"].get("bytes_anexados_por_op")
        ]
        if bytes_linhas:
            _p("")
            _p("Bytes anexados por operação na colmeia (o preço da cópia de caminho — a raiz cresce com N): " + "; ".join(f"{op} N={n:,}: {br(bb, 0)} B".replace(",", ".") for op, n, bb in bytes_linhas) + ".")
        sc = r.get("syscalls_por_op", {}).get(regime)
        if sc and "nao_medido" not in sc:
            _p("")
            _p(f"Chamadas de sistema por operação em `{regime}` (N = 1.000, `strace -c`, por diferença 1.000 − 200 ops):")
            _p("")
            _p("| operação | lado | fsync | fdatasync | write | pwrite64 | openat | unlink |")
            _p("|---|---|---:|---:|---:|---:|---:|---:|")
            for operacao, lados in sc.items():
                for lado, cont in lados.items():
                    _p(
                        f"| {operacao} | {lado} | {br(cont['fsync'], 1)} | {br(cont['fdatasync'], 1)} | {br(cont['write'], 1)} | "
                        f"{br(cont['pwrite64'], 1)} | {br(cont['openat'], 1)} | {br(cont['unlink'] + cont['unlinkat'], 1)} |"
                    )
    if r.get("nao_medido"):
        _p("")
        _p("NÃO MEDIDO: " + "; ".join(json.dumps(x, ensure_ascii=False) for x in r["nao_medido"]))
    return "\n".join(linhas)


# ------------------------------------------------------------------- principal


def gravar_parcial(resultado):
    with open(PARCIAL, "w") as f:
        json.dump(resultado, f, ensure_ascii=False, indent=2)
        f.write("\n")


def principal(argv):
    if "--sonda-sqlite" in argv:
        i = argv.index("--sonda-sqlite")
        regime, n, ops, dir_dados, operacao = argv[i + 1 : i + 6]
        sonda_sqlite(regime, int(n), int(ops), dir_dados, operacao)
        return 0

    if "--markdown" in argv:
        i = argv.index("--markdown")
        alvo = Path(argv[i + 1]) if len(argv) > i + 1 else SAIDA
        print(markdown(alvo))
        return 0

    regimes = list(REGIMES)
    tamanhos = list(TAMANHOS)
    ops, reps = OPS, REPS
    com_syscalls = True
    continuar = False
    # `--rust-pronto <dir>`: um lado Rust ja medido (o `rust-<regime>-<n>.json`
    # e o `dados-<n>.json` que o exemplo gravou) e reaproveitado em vez de
    # medido de novo -- para uma corrida interrompida entre o lado Rust e o
    # SQLite nao jogar fora os minutos de `por_operacao`. A carga daquele
    # momento vem do proprio JSON.
    rust_pronto = None
    sem_portao = False
    saida = SAIDA
    i = 1
    while i < len(argv):
        a = argv[i]
        if a == "--regimes":
            regimes = argv[i + 1].split(",")
            i += 1
        elif a == "--tamanhos":
            tamanhos = [int(x) for x in argv[i + 1].split(",")]
            i += 1
        elif a == "--ops":
            ops = int(argv[i + 1])
            i += 1
        elif a == "--reps":
            reps = int(argv[i + 1])
            i += 1
        elif a == "--saida":
            saida = Path(argv[i + 1])
            i += 1
        elif a == "--rapido":
            ops, reps = 300, 3
        elif a == "--sem-syscalls":
            com_syscalls = False
        elif a == "--sem-esperar":
            global SEM_ESPERAR
            SEM_ESPERAR = True
        elif a == "--continuar":
            continuar = True
        elif a == "--rust-pronto":
            rust_pronto = Path(argv[i + 1])
            i += 1
        elif a == "--espera-max":
            global ESPERA_MAX_S
            ESPERA_MAX_S = int(argv[i + 1]) * 60
            i += 1
        elif a == "--sem-portao":
            # Para a CONTINUACAO de uma corrida que ja tinha passado pelo
            # portao: quem chegou depois esta esperando por ela, e consultar
            # o portao de novo seria esperar por quem espera. Carga e
            # compiladores continuam conferidos.
            sem_portao = True
        else:
            print(f"argumento desconhecido: {a}", file=sys.stderr)
            return 2
        i += 1

    if not EXEMPLO.exists():
        print(f"falta o binario {EXEMPLO}: cargo build --release --example custo-da-colmeia -p phxsql-store")
        return 2

    inicio_total = time.monotonic()
    resultado = {
        "bancada": "custo-da-colmeia-crud",
        "pergunta_do_dono": "Compare o tipo colmeia com o sqlite e phxsql — Insert, update, delete e select.",
        "o_que_mede": (
            "quatro operacoes de PONTO (ler, inserir, atualizar, excluir) sobre o "
            "mesmo par caminho->valor config-shaped, em tres lados: protótipo PSHV "
            "(colmeia viva, append por operacao), SQLite(R) da biblioteca padrao "
            "do Python, e o motor Padrao do PhxSql (Table com indice unico)"
        ),
        "trabalho_comparado": (
            "1 operacao = 1 chamada nos tres lados; mesma chave, mesmo valor, "
            "mesma ordem; atualizar e excluir sobre min(ops, N) chaves DISTINTAS; "
            "escritas partem sempre de uma base de N refeita antes de cada "
            "passada; o que ficou gravado e conferido no fim (read-back, "
            "contagem) nos tres lados"
        ),
        "o_que_cada_lado_faz": O_QUE_CADA_LADO_FAZ,
        "casamento_de_durabilidade": CASAMENTO,
        "regime_de_cache": (
            "os tres lados QUENTES: colmeia com o arquivo inteiro em memoria; "
            "Padrao com definir_cache_paginas(1_000_000); SQLite com "
            "cache_size=-262144 (256 MiB); a primeira passada de cada medida e "
            "aquecimento e nao entra"
        ),
        "medido_em": agora_utc(),
        "maquina": f"container Linux, {os.cpu_count()} nucleos, kernel {platform.release()}",
        "python": platform.python_version(),
        "sqlite": sqlite3.sqlite_version,
        "binario": {"caminho": str(EXEMPLO.relative_to(RAIZ)), "modificado_em": mtime_utc(EXEMPLO)},
        "carga_1min_no_inicio": carga_1min(),
        "ops_por_repeticao": ops,
        "repeticoes": reps,
        "tamanhos": tamanhos,
        "regimes": regimes,
        "linha_de_base_python": None,
        "combinacoes": [],
        "syscalls_por_op": {},
        "nao_medido": [],
    }

    # `--continuar`: retoma do parcial, sem remedir o que ja esta completo
    # (as tres lados, as quatro operacoes). Uma corrida interrompida no meio
    # nao joga fora vinte minutos de `por_operacao`; o que esta incompleto e
    # refeito inteiro, para os tres lados de uma combinacao serem do mesmo
    # momento.
    feitos = set()
    if continuar and PARCIAL.exists():
        with open(PARCIAL) as f:
            anterior = json.load(f)
        resultado["medido_em"] = anterior.get("medido_em", resultado["medido_em"])
        resultado["retomado_em"] = agora_utc()
        resultado["linha_de_base_python"] = anterior.get("linha_de_base_python")
        completas = [
            c
            for c in anterior.get("combinacoes", [])
            if all(isinstance(v, dict) for v in c["lados"].values()) and len(c["lados"]) == 3
        ]
        por_chave = {}
        for c in completas:
            por_chave.setdefault((c["regime"], c["n"]), []).append(c)
        for chave, lista in por_chave.items():
            if len(lista) == 4:
                feitos.add(chave)
                resultado["combinacoes"].extend(lista)
        resultado["syscalls_por_op"] = {
            k: v for k, v in anterior.get("syscalls_por_op", {}).items() if "nao_medido" not in v
        }
        print(f"== retomando: {len(feitos)} combinacoes (regime, N) ja completas ==", flush=True)

    dir_tmp = Path(tempfile.mkdtemp(prefix="phx-colmeia-crud-"))
    primeira_secao = [not sem_portao]

    def slot_da_secao():
        slot = esperar_slot(consultar_portao=primeira_secao[0])
        if slot is not None:
            primeira_secao[0] = False
        return slot

    try:
        print("== linha de base do Python ==", flush=True)
        slot = slot_da_secao()
        if resultado["linha_de_base_python"]:
            print("    (ja medida)")
        elif slot is None:
            resultado["nao_medido"].append({"secao": "linha_de_base_python", "motivo": f"slot nunca livre em {ESPERA_MAX_S // 60} min"})
        else:
            resultado["linha_de_base_python"] = linha_de_base_python(dir_tmp, ops, reps)
            resultado["linha_de_base_python"]["carga_1min_no_inicio"] = slot[0]
            b = resultado["linha_de_base_python"]
            print(
                f"    execute SELECT ? : {b['execute_select_param_us']['mediana_us']:.3f} us/op"
                f"   laco vazio: {b['laco_vazio_us']['mediana_us']:.3f} us/op"
            )
        gravar_parcial(resultado)

        for regime in regimes:
            for n in tamanhos:
                print(f"\n== regime {regime}, N = {n} ==", flush=True)
                if (regime, n) in feitos:
                    print("    (ja medida)")
                    continue
                pronto = rust_pronto and (rust_pronto / f"rust-{regime}-{n}.json").exists() and (rust_pronto / f"dados-{n}.json").exists()
                if pronto:
                    with open(rust_pronto / f"rust-{regime}-{n}.json") as f:
                        rust = json.load(f)
                    with open(rust_pronto / f"dados-{n}.json") as f:
                        dados = json.load(f)
                    carga_rust, esperou_rust = rust["carga_1min_no_inicio"], None
                    print(f"    lado Rust reaproveitado de {rust_pronto} (medido em {rust['medido_em']}, carga {carga_rust})")
                else:
                    slot = slot_da_secao()
                    if slot is None:
                        resultado["nao_medido"].append({"regime": regime, "n": n, "lado": "colmeia+padrao", "motivo": f"slot nunca livre em {ESPERA_MAX_S // 60} min"})
                        gravar_parcial(resultado)
                        continue
                    carga_rust, esperou_rust = slot
                    rust, dados = lado_rust(regime, n, ops, reps, dir_tmp)

                slot = slot_da_secao()
                if slot is None:
                    resultado["nao_medido"].append({"regime": regime, "n": n, "lado": "sqlite", "motivo": f"slot nunca livre em {ESPERA_MAX_S // 60} min"})
                    sq = {}
                    carga_sq, esperou_sq = None, None
                else:
                    carga_sq, esperou_sq = slot
                    sq = lado_sqlite(regime, n, ops, reps, dados, dir_tmp)

                for secao in rust["secoes"]:
                    operacao = secao["operacao"]
                    lados = {}
                    for lado in ("colmeia", "padrao"):
                        if lado in secao:
                            lados[lado] = secao[lado]
                    if operacao in sq:
                        lados["sqlite"] = {k: v for k, v in sq[operacao].items() if k != "ops_por_repeticao"}
                    else:
                        lados["sqlite"] = "NAO MEDIDA"
                    medidos = {k: v for k, v in lados.items() if isinstance(v, dict)}
                    pares, vencedor = julgar(medidos)
                    resultado["combinacoes"].append(
                        {
                            "regime": regime,
                            "operacao": operacao,
                            "n": n,
                            "ops_por_repeticao": secao["ops_por_repeticao"],
                            "repeticoes": secao["repeticoes"],
                            "carga_1min_no_inicio": {"rust": carga_rust, "sqlite": carga_sq},
                            "esperou_slot_s": {"rust": esperou_rust, "sqlite": esperou_sq},
                            "lados": lados,
                            "pares": pares,
                            "vencedor": vencedor,
                        }
                    )
                    resumo = "  ".join(
                        f"{k} {v['mediana_us']:.3f}" for k, v in medidos.items()
                    )
                    print(f"    -> {operacao:9s} {resumo}   vencedor: {vencedor}")
                gravar_parcial(resultado)

        if com_syscalls:
            for regime in regimes:
                print(f"\n== syscalls por operacao ({regime}, N=1000, por diferenca 1000-200 ops) ==", flush=True)
                if regime in resultado["syscalls_por_op"]:
                    print("    (ja medida)")
                    continue
                slot = slot_da_secao()
                if slot is None:
                    resultado["nao_medido"].append({"secao": f"syscalls_por_op/{regime}", "motivo": f"slot nunca livre em {ESPERA_MAX_S // 60} min"})
                    continue
                resultado["syscalls_por_op"][regime] = syscalls_por_op(regime, dir_tmp)
                for operacao, lados in resultado["syscalls_por_op"][regime].items():
                    for lado, c in lados.items():
                        print(f"    {operacao:9s} {lado:8s} " + "  ".join(f"{s}={v}" for s, v in c.items()))
                gravar_parcial(resultado)
    finally:
        shutil.rmtree(dir_tmp, ignore_errors=True)

    resultado["duracao_total_s"] = round(time.monotonic() - inicio_total, 1)
    resultado["terminado_em"] = agora_utc()
    gravar_parcial(resultado)
    if saida == SAIDA:
        # O arquivo versionado so muda no fim, de uma vez: ou e o antigo
        # inteiro, ou o novo inteiro (a regra da `bancada/LEIA-ME.md`).
        os.replace(PARCIAL, SAIDA)
    else:
        saida.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(PARCIAL), str(saida))
    print(f"\ngravado em {saida}")
    return 0


if __name__ == "__main__":
    sys.exit(principal(sys.argv))
