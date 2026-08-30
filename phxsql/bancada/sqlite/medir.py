#!/usr/bin/env python3
"""Bancada PhxSql x SQLite(R) -- a comparacao que decide o caso do celular.

    cargo build --release --examples -p phxsql-store   # a regra do binario velho
    cargo build --release
    python3 bancada/sqlite/medir.py                    # ~8 min
    python3 bancada/sqlite/medir.py 200000 --rodadas 5

O SQLite(R) vem na biblioteca padrao do Python, entao nao ha o que instalar --
e o modulo `sqlite3` e uma extensao em C, nao Python interpretado.

As quatro regras de `bancada/LEIA-ME.md`, aplicadas a ESTE par
-------------------------------------------------------------
1. MESMOS DADOS. O gerador `linha(i)` desta bancada e a traducao literal do
   `linha(i)` de `crates/phxsql-store/examples/carga.rs`. Sem sorteio.
2. MESMO ESQUEMA. Cinco colunas, uma busca por `id` e uma por `cidade`. O
   SQLite(R) roda em DUAS variantes de esquema porque a traducao nao e obvia --
   ver "As duas variantes" mais abaixo -- e as duas sao publicadas.
3. MESMA FORMA DE PERGUNTA. Uma instrucao por operacao dos dois lados: 20.000
   `SELECT ... WHERE id=?` contra 20.000 `buscar`, e nao um `IN (...)` contra
   vinte mil buscas. Foi assim que a primeira bancada desta casa mentiu 41x.
4. MESMA QUANTIDADE DE TRABALHO. A varredura le a faixa INTEIRA e soma o valor
   dos dois lados, e a prova de que esta igual e a SOMA: os dois devolvem o
   mesmo total ate o centavo, por dois codigos sem uma linha em comum. Se
   divergir, a bancada reprova em vez de publicar.

Os tres cuidados que este par exige, e que os outros nao exigiam
---------------------------------------------------------------
a) O SQLite(R) e BIBLIOTECA EM PROCESSO; o PhxSql de hoje e SERVIDOR POR
   SOQUETE. Chamada de funcao contra ida e volta de rede nao e trabalho igual.
   Por isso o PhxSql aparece em DUAS colunas -- `carga` (biblioteca, o mesmo
   binario da bancada do MySQL(R)) e `phxsqld` (soquete) -- e a diferenca entre
   elas e publicada como o CUSTO DO TRANSPORTE, com um piso medido a parte
   (a bancada D: ida e volta de um `ping` com carga util do mesmo tamanho).
b) DURABILIDADE. O SQLite(R) sincroniza por transacao; o PhxSql tem tres modos
   (`por_operacao`, `por_lote`, `sistema`). Agrupar de um lado e nao do outro
   faz o numero mentir -- foi o terceiro erro desta casa, na fase `excluir`.
   Aqui os modos sao casados um a um, e o compromisso vai escrito em cada
   linha do resultado:

       PhxSql por_operacao  <->  SQLite synchronous=FULL, autocommit
       PhxSql por_lote(200) <->  SQLite synchronous=FULL, COMMIT a cada 200
       PhxSql sistema       <->  SQLite synchronous=OFF

   A carga em massa (bancada A) e "uma sincronizacao no fim" dos dois lados.
c) A MAQUINA NAO ESTA QUIETA. Cada medida roda `--rodadas` vezes e o que se
   publica e a MEDIANA, com o menor e o maior ao lado. Numero unico de maquina
   ocupada e chute com aparencia de medida.

As duas variantes do esquema do SQLite(R), e por que as duas ficam
------------------------------------------------------------------
No PhxSql o `.reg` e enderecado por conta (`offset = base + (rowid-1)*slot`),
sem arvore nenhuma, e ha DUAS arvores: `porId` e `porCidade`.

No SQLite(R) a traducao de "chave primaria em id" tem duas leituras honestas:

  * `rowid`   -- `id INTEGER PRIMARY KEY` faz do `id` o proprio rowid: a tabela
                 VIRA a arvore do id. Duas estruturas ao todo. E a forma que
                 qualquer aplicativo escreveria, e a mais rapida.
  * `2ind`    -- `id INTEGER NOT NULL` mais um `UNIQUE INDEX porId`: a tabela
                 tem rowid implicito e MAIS duas arvores. Tres estruturas.

A primeira favorece o SQLite(R) e a segunda o penaliza; nenhuma das duas e "a
certa". Publicar so uma seria escolher o resultado -- entao ficam as duas, e o
leitor ve o tamanho da escolha.

O que esta bancada NAO compara
------------------------------
TRANSACAO. O SQLite(R) tem; o PhxSql nao. Parte do custo de escrita dele paga
uma garantia que o PhxSql ainda nao oferece, e por isso nao se escreve "ACID"
sobre o PhxSql em lugar nenhum.

LIXEIRA. A exclusao fisica do PhxSql copia a linha para o `.trash` e o motivo
para o `.reason` antes de liberar o slot. O SQLite(R) nao faz nada disso. E
trabalho A MAIS do nosso lado, e fica dito em vez de escondido.

COLUNAS DE SISTEMA. Todo esquema do PhxSql ganha `softdeleted` e `rownum` de
brinde (9 bytes por linha). O SQLite(R) nao carrega equivalente.
"""

import json
import os
import shutil
import socket
import sqlite3
import statistics
import subprocess
import sys
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
# O caminho sai DA ARVORE em que este arquivo esta -- com um absoluto escrito a
# mao, uma bancada rodada numa arvore de trabalho mediria o binario de OUTRA.
CARGA = Path(os.environ.get("PHX_CARGA", RAIZ / "target/release/examples/carga"))
PHXSQLD = Path(os.environ.get("PHX_PHXSQLD", RAIZ / "target/release/phxsqld"))
TRABALHO = Path(os.environ.get("PHX_TRABALHO", "/tmp/phx-sqlite"))
RESULTADOS = AQUI / "resultados.json"
PARCIAL = AQUI / "resultados.parcial.json"

PORTA = int(os.environ.get("PORTA", "7450"))
TOKEN = "bancada-sqlite"
SENHA = "bancada-1234"
DB = "bancada"

JIFFY = os.sysconf("SC_CLK_TCK")

CIDADES = [
    "Blumenau",
    "Joinville",
    "Itajai",
    "Curitiba",
    "Chapeco",
    "Lages",
    "Florianopolis",
    "Criciuma",
]
# O salto que espalha as buscas pela tabela inteira, para nao medir so a cache.
# E o mesmo 7.919 do `carga.rs`: primo, entao percorre alvos distintos.
PASSO = 7_919


def linha(i):
    """Traducao literal de `linha(i)` do `carga.rs`. Mesmos dados, sem sorteio."""
    return (
        i,
        "Produto %08d" % i,
        CIDADES[i % len(CIDADES)],
        (i % 900_000) + 100,  # o Decimal(15,2) do PhxSql guarda CENTAVOS
        20_000 + (i % 400),  # o Date do PhxSql guarda o NUMERO DO DIA
    )


def centavos_para_texto(c):
    return "%d.%02d" % divmod(c, 100)


# ---------------------------------------------------------------- contadores


def le_cpu(pid):
    try:
        campos = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        return (int(campos[11]) + int(campos[12])) / JIFFY
    except (FileNotFoundError, ProcessLookupError, IndexError):
        return 0.0


def le_pico_rss(pid):
    try:
        for l in Path(f"/proc/{pid}/status").read_text().splitlines():
            if l.startswith("VmHWM:"):
                return int(l.split()[1]) * 1024
    except (FileNotFoundError, ProcessLookupError):
        pass
    return 0


def tamanho(caminho):
    p = Path(caminho)
    if p.is_file():
        return p.stat().st_size
    return sum(f.stat().st_size for f in p.rglob("*") if f.is_file())


def tamanho_por_extensao(caminho):
    """Quanto cada arquivo do modelo separado pesa.

    O total sozinho nao ensina nada: saber que o PhxSql ocupa 4x o SQLite(R)
    e util, mas saber QUAL arquivo e o peso decide o que fazer a respeito --
    e num aparelho de bolso "o que fazer a respeito" e a pergunta inteira.
    """
    por = {}
    for f in Path(caminho).rglob("*"):
        if f.is_file():
            por[f.suffix or "(sem)"] = por.get(f.suffix or "(sem)", 0) + f.stat().st_size
    return por


def resumo(amostras):
    """Mediana com o menor e o maior ao lado. Nunca um numero so."""
    return {
        "mediana": round(statistics.median(amostras), 4),
        "min": round(min(amostras), 4),
        "max": round(max(amostras), 4),
        "amostras": [round(x, 4) for x in amostras],
    }


# ------------------------------------------------------------------- PhxSql


def roda_carga(dir_dados, fase, n, ambiente=None):
    """Uma fase do `carga`, em PROCESSO SEPARADO -- os contadores sao dela.

    O tempo que volta e o que o PROPRIO `carga` cronometrou (a linha
    `RESULTADO`), e nao o relogio de fora. A diferenca nao e detalhe: subir um
    processo custa ~6 ms nesta maquina, e uma fase de 20.000 buscas leva 4 ms.
    Medida por fora, a fase que o motor faz em 4 ms apareceria como 10 --
    e o outro lado, que e uma chamada de funcao dentro do mesmo processo, nao
    paga nada disso. Seria trabalho desigual escondido no numero, outra vez.
    """
    env = dict(os.environ)
    if ambiente:
        env.update(ambiente)
    p = subprocess.Popen(
        [str(CARGA), str(dir_dados), fase, str(n)],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=env,
    )
    saida = p.communicate()[0].decode()
    if p.returncode != 0:
        raise SystemExit(f"carga {fase} falhou:\n{saida}")
    marca = saida.rsplit("RESULTADO ", 1)
    segundos = json.loads(marca[1])["segundos"] if len(marca) > 1 else 0.0
    return segundos, saida


class Phxsqld:
    """Um phxsqld nosso. Morre pelo PID, e so ele -- nunca `pkill`."""

    def __init__(self, base, durabilidade="por_lote", lote=200):
        self.base = Path(base)
        shutil.rmtree(self.base, ignore_errors=True)
        (self.base / "dados").mkdir(parents=True)
        cfg = {
            "base": "dados",
            "bind": f"127.0.0.1:{PORTA}",
            "token": TOKEN,
            # A web fica desligada: pagina servida e trabalho que o SQLite(R)
            # nao faz, e ela nem entra na pergunta.
            "web": {"ligado": False},
            # O teto de linhas sobe porque a fase `varrer` devolve a faixa
            # inteira -- com o teto de fabrica (1.000) ela leria 1.000 linhas
            # contra 25.000 do outro lado, que e o erro do trabalho desigual.
            "max_linhas": 5_000_000,
            "recursos": {
                "durabilidade": durabilidade,
                "lote_operacoes": lote,
                "lote_milissegundos": 10_000_000,
                "cache_paginas": 2048,
            },
            "root": {
                "id": 1,
                "nome": "root",
                "login": "root",
                "senha_hash": self.hash_da_senha(SENHA),
            },
            "usuarios": [],
        }
        (self.base / "config.json").write_text(json.dumps(cfg, indent=1))
        self.log = open(self.base / "servidor.log", "a")
        self.proc = subprocess.Popen(
            [str(PHXSQLD)],
            cwd=self.base,
            stdout=self.log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
        )
        for _ in range(80):
            time.sleep(0.25)
            try:
                socket.create_connection(("127.0.0.1", PORTA), timeout=2).close()
                return
            except OSError:
                pass
        raise SystemExit("o phxsqld nao subiu; veja " + str(self.base / "servidor.log"))

    @staticmethod
    def hash_da_senha(senha):
        r = subprocess.run(
            [str(PHXSQLD), "--senha"], input=senha.encode(), capture_output=True, check=True
        )
        return r.stdout.decode().split('"')[3]

    def parar(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait()
        self.log.close()


class Cliente:
    def __init__(self, porta=PORTA):
        self.s = socket.create_connection(("127.0.0.1", porta), timeout=600)
        self.s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.f = self.s.makefile("rwb")

    def call(self, d):
        d.setdefault("token", TOKEN)
        self.s.sendall((json.dumps(d) + "\n").encode())
        r = json.loads(self.f.readline())
        if not r.get("ok"):
            raise SystemExit("pedido recusado: " + json.dumps(r)[:400])
        return r

    def fechar(self):
        self.f.close()
        self.s.close()


def cria_tabela_pelo_soquete(c):
    c.call({"op": "criar_database", "database": DB})
    c.call(
        {
            "op": "criar_tabela",
            "database": DB,
            "tabela": "precos",
            "colunas": [
                {"nome": "id", "tipo": "Int8", "obrigatoria": True},
                {"nome": "produto", "tipo": "Str(40)", "obrigatoria": True},
                {"nome": "cidade", "tipo": "Str(20)"},
                {"nome": "valor", "tipo": "Decimal(15,2)"},
                {"nome": "cadastro", "tipo": "Date"},
            ],
            "indices": [
                {"nome": "porId", "colunas": ["id"], "unico": True},
                {"nome": "porCidade", "colunas": ["cidade"]},
            ],
        }
    )


def valores_do_pedido(i):
    id_, produto, cidade, cent, dia = linha(i)
    return {
        "id": id_,
        "produto": produto,
        "cidade": cidade,
        "valor": centavos_para_texto(cent),
        "cadastro": dia,
    }


# ------------------------------------------------------------------- SQLite


def abre_sqlite(caminho, sincrono, variante):
    con = sqlite3.connect(caminho, isolation_level=None)
    con.execute("PRAGMA journal_mode=DELETE")  # o padrao; WAL seria outro compromisso
    con.execute(f"PRAGMA synchronous={sincrono}")
    if variante == "rowid":
        con.execute(
            "CREATE TABLE precos (id INTEGER PRIMARY KEY, produto TEXT NOT NULL,"
            " cidade TEXT, valor INTEGER, cadastro INTEGER)"
        )
    else:
        con.execute(
            "CREATE TABLE precos (id INTEGER NOT NULL, produto TEXT NOT NULL,"
            " cidade TEXT, valor INTEGER, cadastro INTEGER)"
        )
        con.execute("CREATE UNIQUE INDEX porId ON precos(id)")
    con.execute("CREATE INDEX porCidade ON precos(cidade)")
    return con


INSERE = "INSERT INTO precos (id, produto, cidade, valor, cadastro) VALUES (?,?,?,?,?)"
# O `atualizar` do `carga.rs` regrava a LINHA INTEIRA, entao o UPDATE tambem
# regrava as quatro colunas: trocar so `valor` seria menos trabalho de um lado.
ATUALIZA = "UPDATE precos SET produto=?, cidade=?, valor=?, cadastro=? WHERE id=?"


# ------------------------------------------------------- bancada A: biblioteca


def bancada_a(n, ops, rodadas):
    """Biblioteca contra biblioteca: `carga` (Rust) x `sqlite3` (C).

    E a comparacao que vale para o celular, porque a forma certa do PhxSql no
    aparelho e biblioteca embutida -- nao daemon escutando porta.

    Durabilidade casada: UMA sincronizacao no fim de cada fase dos dois lados.
    """
    fases = ["inserir", "buscar", "varrer", "atualizar", "excluir"]
    saida = {"phxsql": {f: [] for f in fases}, "disco": {"phxsql": []}}
    for v in ("rowid", "2ind"):
        # `varrer_todas` so existe do lado do SQLite(R), e existe porque a
        # varredura e o unico lugar em que os dois lados NAO fazem o mesmo
        # trabalho por dentro: o `carga` decodifica a LINHA INTEIRA de cada
        # rowid da faixa, e o `sum(valor)` do SQLite(R) toca uma coluna so.
        # Em vez de argumentar sobre isso, mede-se: `varrer_todas` soma algo
        # de CADA coluna, obrigando o SQLite(R) a materializar a linha toda.
        # A diferenca entre as duas e o tamanho exato da vantagem que o
        # SQLite(R) tira de saber ler uma coluna sem ler as outras.
        saida[f"sqlite-{v}"] = {f: [] for f in fases + ["varrer_todas"]}
        saida["disco"][f"sqlite-{v}"] = []
    soma_phx = soma_sql = None

    for r in range(rodadas):
        print(f"  rodada {r + 1}/{rodadas}")
        # -------- PhxSql, biblioteca
        d = TRABALHO / "biblioteca"
        shutil.rmtree(d, ignore_errors=True)
        roda_carga(d, "criar", 0)
        for fase in fases:
            quantos = n if fase == "inserir" else ops
            # A exclusao entra na janela porque do outro lado as 20.000
            # exclusoes vao dentro de UMA transacao: um `fsync` para as vinte
            # mil. Sem isto o PhxSql pagaria vinte mil e o numero mentiria --
            # e este e exatamente o terceiro erro de `bancada/LEIA-ME.md`.
            amb = {"PHX_EXCLUSAO_NA_JANELA": "1"} if fase == "excluir" else None
            seg, texto = roda_carga(d, fase, quantos, amb)
            saida["phxsql"][fase].append(seg)
            if fase == "varrer":
                soma_phx = int(texto.split("soma:")[1].split()[0])
            if fase == "inserir":
                saida["disco"]["phxsql"].append(tamanho(d))
                saida["disco_por_arquivo"] = tamanho_por_extensao(d)
            print(f"    phxsql   {fase:10s} {seg:8.3f} s")

        # -------- SQLite, as duas variantes
        for v in ("rowid", "2ind"):
            arq = TRABALHO / f"sqlite-{v}.db"
            for extra in ("", "-journal", "-wal", "-shm"):
                Path(str(arq) + extra).unlink(missing_ok=True)
            con = abre_sqlite(arq, "FULL", v)

            t0 = time.monotonic()
            con.execute("BEGIN")
            con.executemany(INSERE, (linha(i) for i in range(1, n + 1)))
            con.execute("COMMIT")
            saida[f"sqlite-{v}"]["inserir"].append(time.monotonic() - t0)
            saida["disco"][f"sqlite-{v}"].append(
                tamanho(arq) + tamanho_do_diario(arq)
            )

            t0 = time.monotonic()
            cur = con.cursor()
            for k in range(ops):
                alvo = (k * PASSO) % n + 1
                cur.execute("SELECT * FROM precos WHERE id=?", (alvo,))
                cur.fetchone()
            saida[f"sqlite-{v}"]["buscar"].append(time.monotonic() - t0)

            t0 = time.monotonic()
            cur.execute(
                "SELECT count(*), sum(valor) FROM precos WHERE cidade=?", ("Blumenau",)
            )
            quantas, soma_sql = cur.fetchone()
            saida[f"sqlite-{v}"]["varrer"].append(time.monotonic() - t0)

            t0 = time.monotonic()
            cur.execute(
                "SELECT count(*), sum(valor), sum(length(produto)), sum(length(cidade)),"
                " sum(cadastro), sum(id) FROM precos WHERE cidade=?",
                ("Blumenau",),
            )
            cur.fetchone()
            saida[f"sqlite-{v}"]["varrer_todas"].append(time.monotonic() - t0)

            t0 = time.monotonic()
            con.execute("BEGIN")
            for k in range(ops):
                alvo = (k * PASSO) % n + 1
                _, produto, cidade, _, dia = linha(alvo)
                con.execute(ATUALIZA, (produto, cidade, 999_900, dia, alvo))
            con.execute("COMMIT")
            saida[f"sqlite-{v}"]["atualizar"].append(time.monotonic() - t0)

            t0 = time.monotonic()
            con.execute("BEGIN")
            for k in range(ops):
                alvo = (k * PASSO) % n + 1
                con.execute("DELETE FROM precos WHERE id=?", (alvo,))
            con.execute("COMMIT")
            saida[f"sqlite-{v}"]["excluir"].append(time.monotonic() - t0)
            con.close()
            print(f"    sqlite-{v:5s} pronto")

    # A PROVA de que os dois lados fizeram o mesmo trabalho na varredura: a
    # soma bate ate o centavo, por dois codigos sem uma linha em comum.
    if soma_phx != soma_sql:
        raise SystemExit(
            f"as somas da varredura DIVERGEM: phxsql={soma_phx} sqlite={soma_sql}."
            " A bancada nao publica numero de trabalho desigual."
        )
    saida["soma_conferida"] = soma_phx
    saida["linhas_da_faixa"] = quantas
    return saida


def tamanho_do_diario(arq):
    return sum(
        tamanho(Path(str(arq) + e))
        for e in ("-journal", "-wal", "-shm")
        if Path(str(arq) + e).exists()
    )


# ----------------------------------------------- bancada B: o custo do soquete


def bancada_b(n, ops, rodadas):
    """As mesmas fases, agora pelo SOQUETE. A diferenca para a A e o transporte.

    Uma instrucao por operacao, como na A: `inserir` manda UMA linha por ida e
    volta, que e a mesma forma do `t.inserir()` da biblioteca.
    """
    fases = ["inserir", "inserir_lote", "buscar", "varrer", "atualizar", "excluir"]
    saida = {f: [] for f in fases}
    saida["disco"] = []
    for r in range(rodadas):
        print(f"  rodada {r + 1}/{rodadas}")
        s = Phxsqld(TRABALHO / "soquete")
        try:
            c = Cliente()
            c.call({"op": "login", "usuario": "root", "senha": SENHA})
            cria_tabela_pelo_soquete(c)

            t0 = time.monotonic()
            for i in range(1, n + 1):
                c.call(
                    {
                        "op": "inserir",
                        "database": DB,
                        "tabela": "precos",
                        "valores": valores_do_pedido(i),
                    }
                )
            saida["inserir"].append(time.monotonic() - t0)
            saida["disco"].append(tamanho(TRABALHO / "soquete" / "dados"))
            print(f"    inserir      {saida['inserir'][-1]:8.3f} s")

            t0 = time.monotonic()
            for k in range(ops):
                alvo = (k * PASSO) % n + 1
                c.call(
                    {
                        "op": "buscar",
                        "database": DB,
                        "tabela": "precos",
                        "indice": "porId",
                        "chave": [alvo],
                    }
                )
            saida["buscar"].append(time.monotonic() - t0)

            t0 = time.monotonic()
            r_ = c.call(
                {
                    "op": "varrer",
                    "database": DB,
                    "tabela": "precos",
                    "indice": "porCidade",
                    "max": n,
                }
            )
            saida["varrer"].append(time.monotonic() - t0)
            res = r_.get("resultado", r_)
            linhas = next((v for v in res.values() if isinstance(v, list)), [])
            achadas = [l for l in linhas if l.get("cidade") == "Blumenau"]
            if len(achadas) != n // len(CIDADES):
                raise SystemExit(
                    f"a varredura devolveu {len(achadas)} de {n // len(CIDADES)}:"
                    " trabalho desigual, a bancada nao publica"
                )

            t0 = time.monotonic()
            for k in range(ops):
                alvo = (k * PASSO) % n + 1
                v = valores_do_pedido(alvo)
                v["valor"] = "9999.00"
                c.call(
                    {
                        "op": "atualizar",
                        "database": DB,
                        "tabela": "precos",
                        "rowid": alvo,
                        "valores": v,
                    }
                )
            saida["atualizar"].append(time.monotonic() - t0)

            t0 = time.monotonic()
            for k in range(ops):
                alvo = (k * PASSO) % n + 1
                # `fisico` de proposito: a exclusao SUAVE so vira um bit, e o
                # `DELETE` do outro lado tira a linha e conserta os indices.
                # Comparar bit com remocao seria o quarto erro da familia.
                c.call(
                    {
                        "op": "excluir",
                        "database": DB,
                        "tabela": "precos",
                        "rowid": alvo,
                        "fisico": True,
                    }
                )
            saida["excluir"].append(time.monotonic() - t0)
            c.fechar()
        finally:
            s.parar()

        # A carga em LOTE vai numa tabela limpa, para inserir as mesmas n
        # linhas que a fase de cima inseriu uma a uma.
        s = Phxsqld(TRABALHO / "soquete")
        try:
            c = Cliente()
            c.call({"op": "login", "usuario": "root", "senha": SENHA})
            cria_tabela_pelo_soquete(c)
            t0 = time.monotonic()
            bloco = 1_000
            for ini in range(1, n + 1, bloco):
                c.call(
                    {
                        "op": "inserir_lote",
                        "database": DB,
                        "tabela": "precos",
                        "linhas": [
                            valores_do_pedido(i)
                            for i in range(ini, min(ini + bloco, n + 1))
                        ],
                    }
                )
            saida["inserir_lote"].append(time.monotonic() - t0)
            print(f"    inserir_lote {saida['inserir_lote'][-1]:8.3f} s")
            c.fechar()
        finally:
            s.parar()
    return saida


# ------------------------------------------- bancada C: durabilidade por linha


def bancada_c(n_dur, rodadas):
    """O regime que um aplicativo de celular usa de verdade: cada acao grava.

    n_dur e pequeno de proposito -- `fsync` por linha custa ~1 ms, entao
    200.000 linhas seriam tres minutos por corrida, por lado.
    """
    # A quarta coluna e a JANELA do SQLite(R), e ela tem de ser a mesma do
    # PhxSql -- inclusive no modo `sistema`. A primeira versao desta lista
    # punha 0 aqui e caia no ramo do autocommit: o SQLite(R) abria e fechava
    # 20.000 transacoes (com o `-journal` criado e apagado em cada uma) contra
    # 100 janelas do PhxSql. Nenhum dos dois sincronizava, entao o `fsync` nao
    # denunciava nada -- e o numero saiu 1,56x A NOSSO FAVOR por trabalho
    # desigual. E o mesmo erro de sempre, agora na coluna da janela.
    casos = [
        ("por_operacao", 1, "FULL", 1),
        ("por_lote", 200, "FULL", 200),
        ("sistema", 200, "OFF", 200),
    ]
    saida = {}
    for modo, lote, sincrono, commit_a_cada in casos:
        saida[modo] = {"phxsql_soquete": [], "phxsql_lote": [], "sqlite": []}
        for r in range(rodadas):
            s = Phxsqld(TRABALHO / "durabilidade", durabilidade=modo, lote=lote)
            try:
                c = Cliente()
                c.call({"op": "login", "usuario": "root", "senha": SENHA})
                cria_tabela_pelo_soquete(c)
                t0 = time.monotonic()
                for i in range(1, n_dur + 1):
                    c.call(
                        {
                            "op": "inserir",
                            "database": DB,
                            "tabela": "precos",
                            "valores": valores_do_pedido(i),
                        }
                    )
                saida[modo]["phxsql_soquete"].append(time.monotonic() - t0)
                c.fechar()
            finally:
                s.parar()

            # A MESMA durabilidade, mandando as linhas no tamanho da janela --
            # isto e, `lote` linhas por ida e volta, que e exatamente o
            # `COMMIT` a cada `commit_a_cada` do outro lado. Aqui o transporte
            # se dilui e o que sobra e o MOTOR, que e o que a pergunta queria
            # saber: sem esta coluna a bancada compararia o caminho do pedido
            # com uma chamada de funcao e chamaria isso de durabilidade.
            #
            # Com `por_operacao` a janela e de UMA linha, e "lote de um" nao e
            # lote: seria `inserir_lote` pago vinte mil vezes para embrulhar
            # uma linha cada. A coluna nao existe nesse regime, e sai vazia em
            # vez de sair enganando.
            if commit_a_cada > 1:
                s = Phxsqld(TRABALHO / "durabilidade", durabilidade=modo, lote=lote)
                try:
                    c = Cliente()
                    c.call({"op": "login", "usuario": "root", "senha": SENHA})
                    cria_tabela_pelo_soquete(c)
                    t0 = time.monotonic()
                    for ini in range(1, n_dur + 1, commit_a_cada):
                        c.call(
                            {
                                "op": "inserir_lote",
                                "database": DB,
                                "tabela": "precos",
                                "linhas": [
                                    valores_do_pedido(i)
                                    for i in range(
                                        ini, min(ini + commit_a_cada, n_dur + 1)
                                    )
                                ],
                            }
                        )
                    saida[modo]["phxsql_lote"].append(time.monotonic() - t0)
                    c.fechar()
                finally:
                    s.parar()

            arq = TRABALHO / "durabilidade.db"
            for extra in ("", "-journal", "-wal", "-shm"):
                Path(str(arq) + extra).unlink(missing_ok=True)
            con = abre_sqlite(arq, sincrono, "rowid")
            t0 = time.monotonic()
            if commit_a_cada <= 1:
                # autocommit: uma transacao (e um fsync, com FULL) por linha
                for i in range(1, n_dur + 1):
                    con.execute(INSERE, linha(i))
            else:
                for i in range(1, n_dur + 1):
                    if (i - 1) % commit_a_cada == 0:
                        con.execute("BEGIN")
                    con.execute(INSERE, linha(i))
                    if i % commit_a_cada == 0 or i == n_dur:
                        con.execute("COMMIT")
            saida[modo]["sqlite"].append(time.monotonic() - t0)
            con.close()
        lote_txt = (
            "%7.3f s" % statistics.median(saida[modo]["phxsql_lote"])
            if saida[modo]["phxsql_lote"]
            else "       --"
        )
        print(
            f"    {modo:13s} phxsql {statistics.median(saida[modo]['phxsql_soquete']):7.3f} s"
            f"   em lote {lote_txt}"
            f"   sqlite {statistics.median(saida[modo]['sqlite']):7.3f} s"
        )
    return saida


# --------------------------------------------- bancada D: o piso do transporte


def bancada_d(quantos, rodadas):
    """Quanto custa a IDA E VOLTA, sem nenhum trabalho de banco no meio.

    O `ping` passa pelo mesmo caminho de todo pedido -- leitura da linha,
    `Json::analisar` do corpo inteiro, portoes, resposta -- e nao toca em
    disco. Entao o que sobra e o piso: soquete mais protocolo.

    A carga util leva o MESMO tamanho de um `inserir` de verdade, porque
    analisar 40 bytes e analisar 200 nao custam a mesma coisa.

    E o piso se mede TRES vezes, porque "transporte" nao e uma coisa so, e
    atribuir tudo ao soquete seria diagnostico plausivel em vez de medido:

      completo  -- `json.dumps` + `sendall` + `readline` + `json.loads`, com o
                   `phxsqld` do outro lado. E o que um cliente Python paga.
      eco       -- a MESMA ida e volta contra um eco em Python que so analisa
                   o corpo e devolve. Mede o loopback mais o cliente, sem uma
                   linha do nosso servidor no meio.
      cliente   -- exatamente o mesmo `json.dumps`/`json.loads`, SEM soquete
                   nenhum. E o que o Python cobra so por falar JSON.

    Assim `completo - eco` isola o caminho do pedido do `phxsqld`, e
    `eco - cliente` isola o que o nucleo cobra pela ida e volta. Sem essa
    separacao o numero do transporte levaria junto uma lentidao que e da
    linguagem do medidor -- e um cliente nativo nao pagaria.
    """
    enchimento = json.dumps(valores_do_pedido(1234567))
    saida = {
        "pedidos": quantos,
        "bytes_do_pedido": None,
        "segundos": [],
        "so_cliente": [],
        "eco": [],
    }
    s = Phxsqld(TRABALHO / "transporte")
    try:
        c = Cliente()
        c.call({"op": "login", "usuario": "root", "senha": SENHA})
        pedido = {"op": "ping", "enchimento": enchimento}
        corpo = json.dumps({**pedido, "token": TOKEN}) + "\n"
        saida["bytes_do_pedido"] = len(corpo)
        resposta = json.dumps({"ok": True, "op": "ping", "resultado": {"pong": True}}) + "\n"
        for _ in range(rodadas):
            t0 = time.monotonic()
            for _ in range(quantos):
                c.call(dict(pedido))
            saida["segundos"].append(time.monotonic() - t0)
            t0 = time.monotonic()
            for _ in range(quantos):
                d = dict(pedido)
                d.setdefault("token", TOKEN)
                (json.dumps(d) + "\n").encode()
                json.loads(resposta)
            saida["so_cliente"].append(time.monotonic() - t0)
        c.fechar()
    finally:
        s.parar()

    saida["eco"] = eco_puro(corpo.encode(), quantos, rodadas)
    return saida


# O eco vive num PROCESSO, e nao numa thread. A primeira versao usava
# `threading` e o numero desmoronou: com a maquina carregada o eco marcou
# 73,78 us contra 72,75 us do `phxsqld`, e a subtracao deu -1,03 us -- um
# servidor que "custa menos que nada". A causa e o GIL: cliente e eco no mesmo
# interpretador nao rodam ao mesmo tempo, entao cada ida e volta pagava uma
# troca de contexto que o `phxsqld`, que e outro processo, nao paga. Medir o
# piso com uma restricao que o medido nao tem nao mede piso nenhum.
ECO = r"""
import json, socket, sys
porta = int(sys.argv[1])
resposta = (json.dumps({"ok": True, "op": "ping", "resultado": {"pong": True}}) + "\n").encode()
ouvinte = socket.socket()
ouvinte.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
ouvinte.bind(("127.0.0.1", porta))
ouvinte.listen(1)
print("pronto", flush=True)
con, _ = ouvinte.accept()
con.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
f = con.makefile("rb")
while True:
    linha = f.readline()
    if not linha:
        break
    json.loads(linha)          # o eco tambem analisa o corpo, como o servidor
    con.sendall(resposta)
"""


def eco_puro(corpo, quantos, rodadas):
    """A mesma ida e volta contra um eco em Python, em processo separado."""
    porta = PORTA + 5
    p = subprocess.Popen(
        [sys.executable, "-c", ECO, str(porta)],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    try:
        if not p.stdout.readline().startswith(b"pronto"):
            return []
        s = socket.create_connection(("127.0.0.1", porta), timeout=10)
        s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        f = s.makefile("rb")
        tempos = []
        for _ in range(rodadas):
            t0 = time.monotonic()
            for _ in range(quantos):
                s.sendall(corpo)
                json.loads(f.readline())
            tempos.append(time.monotonic() - t0)
        f.close()
        s.close()
    finally:
        p.terminate()
        p.wait(timeout=10)
    return tempos


# ------------------------------------- bancada E: o que custa um pedido a mais


def bancada_e(n_lote, rodadas):
    """Quanto de um `inserir_lote` e a LINHA e quanto e a CHAMADA.

    Nasceu de um numero que nao fechava: na bancada C, `inserir_lote` em blocos
    de 200 saiu a 32 us/linha, e na B, em blocos de 1.000, a 17,8 us/linha --
    a tabela MENOR, com a arvore mais rasa, saiu mais LENTA por linha. O piso
    do transporte (35 us por ida e volta) explica 0,18 us/linha em blocos de
    200: nao e o soquete.

    Entao mede-se em vez de explicar. Varrendo o tamanho do bloco, o custo por
    linha e `motor + chamada/bloco`: a reta que sai daqui da os dois termos
    separados, e o segundo e o que um aplicativo evita escolhendo o bloco.
    """
    saida = {"n": n_lote, "blocos": {}}
    for bloco in (1, 10, 100, 1_000, 5_000):
        saida["blocos"][bloco] = []
        for _ in range(rodadas):
            s = Phxsqld(TRABALHO / "lotes")
            try:
                c = Cliente()
                c.call({"op": "login", "usuario": "root", "senha": SENHA})
                cria_tabela_pelo_soquete(c)
                t0 = time.monotonic()
                for ini in range(1, n_lote + 1, bloco):
                    c.call(
                        {
                            "op": "inserir_lote",
                            "database": DB,
                            "tabela": "precos",
                            "linhas": [
                                valores_do_pedido(i)
                                for i in range(ini, min(ini + bloco, n_lote + 1))
                            ],
                        }
                    )
                saida["blocos"][bloco].append(time.monotonic() - t0)
                c.fechar()
            finally:
                s.parar()
        m = statistics.median(saida["blocos"][bloco])
        print("    bloco %5d: %7.3f s  %7.2f us/linha  (%d chamadas)"
              % (bloco, m, m / n_lote * 1e6, -(-n_lote // bloco)))
    return saida


# ------------------------------------------------------------------ relatorio


def us(segundos, operacoes):
    return round(segundos / operacoes * 1e6, 2)


def principal():
    n = 200_000
    ops = 20_000
    rodadas = 5
    argv = sys.argv[1:]
    if argv and argv[0].isdigit():
        n = int(argv[0])
        argv = argv[1:]
    if "--rodadas" in argv:
        rodadas = int(argv[argv.index("--rodadas") + 1])
    # Refazer UMA parte sem refazer as outras: a bancada inteira leva ~12 min,
    # e uma correcao numa delas nao devia obrigar a remedir as quatro. O que
    # nao se remede vem do `resultados.json` de antes, e o arquivo continua
    # inteiro -- meia medicao publicada e o que este projeto ja pagou uma vez.
    partes = "abcde"
    if "--partes" in argv:
        partes = argv[argv.index("--partes") + 1].lower()
    ops = min(ops, n // 10)

    for exigido in (CARGA, PHXSQLD):
        if not exigido.exists():
            raise SystemExit(
                f"falta {exigido}. Rode:\n"
                "  cargo build --release\n"
                "  cargo build --release --examples -p phxsql-store"
            )
    shutil.rmtree(TRABALHO, ignore_errors=True)
    TRABALHO.mkdir(parents=True)

    anterior = {}
    if partes != "abcde" and RESULTADOS.exists():
        anterior = json.loads(RESULTADOS.read_text())

    # Cada parte carimba a SI MESMA -- data, hora e carga da maquina --, e a
    # parte reaproveitada mantem o carimbo do dia em que foi medida.
    #
    # A primeira versao tinha um carimbo so, do processo. Depois de refazer a
    # parte `d` sozinha, o documento passou a dizer "medido as 16:56" para uma
    # tabela medida as 16:42, com a carga da maquina de outro momento. Nao era
    # mentira grande, e e exatamente por isso que passaria: quem confere um
    # carimbo de proveniencia? Arquivo que descreve a si mesmo errado estraga
    # toda conclusao tirada dele depois.
    carimbos = dict(anterior.get("medido_em", {}))

    def carimbar(parte):
        carimbos[parte] = {
            "quando": time.strftime("%Y-%m-%d %H:%M:%S"),
            "carga": Path("/proc/loadavg").read_text().split()[:3],
        }

    tudo = {
        "n": n,
        "ops": ops,
        "rodadas": rodadas,
        "sqlite_versao": sqlite3.sqlite_version,
        "python": sys.version.split()[0],
        "carregado_em": time.strftime("%Y-%m-%d %H:%M:%S"),
        # A carga da maquina fica gravada junto: numero de bancada rodada com
        # dois compiladores ao lado nao e o mesmo numero, e quem reproduzir
        # precisa saber com o que esta comparando.
        "carga_no_inicio": Path("/proc/loadavg").read_text().split()[:3],
        "nucleos": os.cpu_count(),
        "partes_refeitas": partes,
    }

    if "a" in partes:
        print("A) biblioteca x biblioteca (uma sincronizacao no fim dos dois lados)")
        tudo["a_biblioteca"] = bancada_a(n, ops, rodadas)
        carimbar("a")
    else:
        tudo["a_biblioteca"] = anterior["a_biblioteca"]
    PARCIAL.write_text(json.dumps(tudo, indent=1, ensure_ascii=False))

    if "b" in partes:
        print("B) as mesmas fases pelo SOQUETE (durabilidade por_lote nos dois)")
        tudo["b_soquete"] = bancada_b(n, ops, max(2, rodadas // 2))
        carimbar("b")
    else:
        tudo["b_soquete"] = anterior["b_soquete"]
    PARCIAL.write_text(json.dumps(tudo, indent=1, ensure_ascii=False))

    if "c" in partes:
        print("C) durabilidade casada, tres regimes")
        tudo["c_durabilidade"] = {"n": min(20_000, n // 10)}
        tudo["c_durabilidade"]["casos"] = bancada_c(
            tudo["c_durabilidade"]["n"], max(2, rodadas // 2)
        )
        carimbar("c")
    else:
        tudo["c_durabilidade"] = anterior["c_durabilidade"]
    PARCIAL.write_text(json.dumps(tudo, indent=1, ensure_ascii=False))

    if "d" in partes:
        print("D) o piso do transporte")
        # Nove rodadas, e nao tres, porque esta e a medida mais curta da
        # bancada -- uma ida e volta de dezenas de microssegundos -- e a que
        # o vizinho barulhento mais estraga. Como o que se publica dela e o
        # MENOR, mais rodadas so melhoram a estimativa, e custam segundos.
        tudo["d_transporte"] = bancada_d(20_000, 9)
        carimbar("d")
    else:
        tudo["d_transporte"] = anterior["d_transporte"]
    PARCIAL.write_text(json.dumps(tudo, indent=1, ensure_ascii=False))

    if "e" in partes:
        print("E) o custo de uma chamada a mais")
        tudo["e_tamanho_do_lote"] = bancada_e(min(20_000, n // 10), max(2, rodadas // 2))
        carimbar("e")
    else:
        tudo["e_tamanho_do_lote"] = anterior.get("e_tamanho_do_lote")

    # O arquivo versionado so muda no FIM, e de uma vez: ou e o antigo inteiro,
    # ou o novo inteiro. Meia medicao dentro de um arquivo publicado engana
    # quem olha o numero e nao ve "faltam quatro fases".
    tudo["carga_no_fim"] = Path("/proc/loadavg").read_text().split()[:3]
    tudo["medido_em"] = carimbos
    PARCIAL.write_text(json.dumps(tudo, indent=1, ensure_ascii=False))
    os.replace(PARCIAL, RESULTADOS)
    imprimir(tudo)
    shutil.rmtree(TRABALHO, ignore_errors=True)


def imprimir(t):
    n, ops = t["n"], t["ops"]
    a = t["a_biblioteca"]
    print("\n=== A) biblioteca x biblioteca, %d linhas, mediana de %d rodadas ==="
          % (n, t["rodadas"]))
    print("%-11s %14s %14s %14s" % ("fase", "PhxSql", "SQLite rowid", "SQLite 2ind"))
    for fase in ("inserir", "buscar", "varrer", "atualizar", "excluir"):
        quantos = n if fase == "inserir" else (a["linhas_da_faixa"] if fase == "varrer" else ops)
        cel = []
        for motor in ("phxsql", "sqlite-rowid", "sqlite-2ind"):
            m = statistics.median(a[motor][fase])
            cel.append("%8.3f s %5.2fus" % (m, m / quantos * 1e6))
        print("%-11s %s" % (fase, " ".join("%22s" % c for c in cel)))
    for v in ("rowid", "2ind"):
        m = statistics.median(a[f"sqlite-{v}"]["varrer_todas"])
        print("varrer_todas (SQLite %s, tocando toda coluna): %8.3f s %5.2f us/linha"
              % (v, m, m / a["linhas_da_faixa"] * 1e6))
    print("disco:      " + "  ".join(
        "%s %.1f MiB (%.0f B/linha)" % (k, statistics.median(v) / 1048576,
                                        statistics.median(v) / n)
        for k, v in a["disco"].items()
    ))
    if a.get("disco_por_arquivo"):
        print("  no PhxSql, por arquivo: " + "  ".join(
            "%s %.1f MiB" % (e, b / 1048576)
            for e, b in sorted(a["disco_por_arquivo"].items(), key=lambda x: -x[1])
        ))
    print("soma da faixa conferida nos dois: %d centavos em %d linhas"
          % (a["soma_conferida"], a["linhas_da_faixa"]))

    d = t["d_transporte"]
    # Pelo MENOR, e nao pela mediana -- ver o comentario em `blocos()`: disputa
    # so acrescenta a uma ida e volta, entao a menor corrida e o piso.
    piso = min(d["segundos"]) / d["pedidos"] * 1e6
    piso_cliente = min(d["so_cliente"]) / d["pedidos"] * 1e6

    b = t["b_soquete"]
    print("\n=== B) as mesmas fases pelo soquete (o piso do transporte descontado) ===")
    print("  %-13s %10s %10s %12s" % ("fase", "us/op", "menos piso", "biblioteca"))
    for fase, quantos, na_a in (
        ("inserir", n, "inserir"), ("inserir_lote", n, "inserir"),
        ("buscar", ops, "buscar"), ("varrer", a["linhas_da_faixa"], "varrer"),
        ("atualizar", ops, "atualizar"), ("excluir", ops, "excluir")):
        m = statistics.median(b[fase]) / quantos * 1e6
        # O `inserir_lote` e a `varrer` mandam MIL linhas ou a faixa inteira
        # por ida e volta, entao o piso nao se paga por linha nelas.
        por_linha = piso if fase in ("inserir", "buscar", "atualizar", "excluir") else 0.0
        lib = statistics.median(a["phxsql"][na_a]) / quantos * 1e6
        print("  %-13s %9.2f %10.2f %11.2f" % (fase, m, m - por_linha, lib))

    eco = min(d["eco"]) / d["pedidos"] * 1e6 if d.get("eco") else 0.0
    print("\n=== D) piso do transporte, %d bytes de pedido ===" % d["bytes_do_pedido"])
    print("  ida e volta contra o phxsqld:                    %6.2f us" % piso)
    print("  a mesma ida e volta contra um eco em Python:     %6.2f us" % eco)
    print("  so o JSON do cliente, sem soquete nenhum:        %6.2f us" % piso_cliente)
    print("  -> o caminho do pedido do phxsqld:               %6.2f us" % (piso - eco))
    print("  -> o loopback e as chamadas de sistema:          %6.2f us" % (eco - piso_cliente))

    print("\n=== C) durabilidade casada, %d linhas ===" % t["c_durabilidade"]["n"])
    nd = t["c_durabilidade"]["n"]
    print("  %-13s %10s %11s %10s %9s %8s"
          % ("modo", "phx 1 a 1", "phx s/piso", "phx lote", "sqlite", "razao"))
    for modo, v in t["c_durabilidade"]["casos"].items():
        p = statistics.median(v["phxsql_soquete"])
        s = statistics.median(v["sqlite"])
        # O PhxSql desta bancada fala por SOQUETE e o SQLite(R) e chamada de
        # funcao: sem descontar o piso, o que se compara e o transporte. A
        # coluna "phx lote" e a comparacao limpa -- mesma janela dos dois
        # lados, transporte diluido -- e e a razao publicada quando existe.
        sem = p - piso * nd / 1e6
        if v["phxsql_lote"]:
            pl = statistics.median(v["phxsql_lote"])
            print("  %-13s %9.3fs %10.3fs %9.3fs %8.3fs %7.2fx"
                  % (modo, p, sem, pl, s, pl / s))
        else:
            print("  %-13s %9.3fs %10.3fs %9s %8.3fs %7.2fx"
                  % (modo, p, sem, "--", s, sem / s))
    e = t.get("e_tamanho_do_lote")
    if e:
        print("\n=== E) o custo de uma chamada a mais, %d linhas em lotes ===" % e["n"])
        print("  %8s %10s %13s" % ("bloco", "chamadas", "us/linha"))
        for bloco, v in sorted(e["blocos"].items(), key=lambda x: int(x[0])):
            bloco = int(bloco)
            m = statistics.median(v)
            print("  %8d %10d %12.2f"
                  % (bloco, -(-e["n"] // bloco), m / e["n"] * 1e6))

    print("\nresultados em " + str(RESULTADOS))


def blocos():
    """As tabelas do `docs/MOBILE.md`, prontas, saidas do `resultados.json`.

    Existe pela regra da casa: numero visivel sai de gerador, ou esta errado e
    ninguem percebeu ainda. O selo do dossie passou QUATRO lancamentos dizendo
    a versao errada porque alguem digitou; uma tabela de bancada copiada a mao
    envelhece do mesmo jeito, e mais calada, porque ninguem confere seis
    colunas de microssegundos lendo.

        python3 bancada/sqlite/medir.py --markdown          # mostra
        python3 bancada/sqlite/medir.py --documento docs/MOBILE.md
    """
    saida, atual = {}, []

    def _p(*args):
        atual.append(" ".join(str(x) for x in args) if args else "")

    def _bloco(nome):
        """Fecha o bloco corrente e comeca outro."""
        if atual and saida:
            pass
        saida[_bloco.nome] = "\n".join(atual).strip("\n")
        atual.clear()
        _bloco.nome = nome

    _bloco.nome = "cabecalho"
    t = json.loads(RESULTADOS.read_text())
    n, ops = t["n"], t["ops"]
    a = t["a_biblioteca"]
    faixa = a["linhas_da_faixa"]

    def med(xs):
        return statistics.median(xs)

    def br(x, casas=2):
        """Ponto no milhar, virgula no decimal -- o resto do repositorio escreve
        assim, e tabela que mistura as duas notacoes faz o leitor conferir a
        pontuacao em vez de ler o numero."""
        s = format(float(x), ",.%df" % casas)
        return s.replace(",", "\u0000").replace(".", ",").replace("\u0000", ".")

    # A proveniencia sai VISIVEL, e nao como comentario de HTML: quem le o
    # documento renderizado tem de ver com que maquina o numero foi feito. Uma
    # nota escondida num `<!-- -->` e o mesmo que nao existir para o leitor.
    nomes = {
        "a": "A · motor contra motor",
        "b": "B · as fases pelo soquete",
        "c": "C · durabilidade casada",
        "d": "D · o piso do transporte",
        "e": "E · o custo de uma chamada",
    }
    _p("`%s` linhas, numa maquina de %d nucleos. **Cada parte carrega o carimbo"
       " do dia em que foi medida** -- refazer uma sozinha e comum, e um"
       " carimbo unico faria o documento datar todas pela ultima:"
       % (br(t["n"], 0), t["nucleos"]))
    _p()
    _p("| parte | medida em | carga da maquina (1/5/15 min) |")
    _p("|---|---|---|")
    for letra, rotulo in nomes.items():
        c = t.get("medido_em", {}).get(letra)
        if c:
            _p("| %s | %s | %s |" % (rotulo, c["quando"], " · ".join(c["carga"])))
        else:
            _p("| %s | *(sem carimbo -- corrida anterior a esta conta)* | — |"
               % rotulo)
    _bloco("tabela-a")
    _p("| fase | trabalho | PhxSql | SQLite (rowid) | SQLite (2 índices) | quem ganha |")
    _p("|---|---:|---:|---:|---:|---|")
    rotulos = {
        "inserir": ("inserir em lote", n),
        "buscar": ("ler por chave", ops),
        "varrer": ("varrer faixa", faixa),
        "atualizar": ("atualizar", ops),
        "excluir": ("excluir de vez", ops),
    }
    for fase, (rot, quantos) in rotulos.items():
        p = med(a["phxsql"][fase]) / quantos * 1e6
        r = med(a["sqlite-rowid"][fase]) / quantos * 1e6
        d = med(a["sqlite-2ind"][fase]) / quantos * 1e6
        quem = (
            "**PhxSql %s×**" % br(r / p, 1) if p < r else "SQLite %s×" % br(p / r, 1)
        )
        _p("| %s | %s ops | %s µs | %s µs | %s µs | %s |"
              % (rot, br(quantos, 0), br(p), br(r), br(d), quem))
        if fase == "varrer":
            # A linha seguinte existe porque a de cima NAO compara trabalho
            # igual por dentro: o `carga` decodifica a linha inteira e o
            # `sum(valor)` toca uma coluna. Aqui o SQLite(R) e obrigado a
            # materializar a linha toda, e a diferenca entre as duas linhas e
            # o tamanho exato dessa vantagem dele. Fica na tabela, e nao numa
            # nota de rodape: o leitor tem de tropecar nela.
            rt = med(a["sqlite-rowid"]["varrer_todas"]) / quantos * 1e6
            dt = med(a["sqlite-2ind"]["varrer_todas"]) / quantos * 1e6
            _p("| ↳ *a mesma, o SQLite tocando toda coluna* | %s ops | %s µs "
               "| %s µs | %s µs | SQLite %s× |"
               % (br(quantos, 0), br(p), br(rt), br(dt), br(p / rt, 1)))
    dp = med(a["disco"]["phxsql"])
    dr = med(a["disco"]["sqlite-rowid"])
    dd = med(a["disco"]["sqlite-2ind"])
    _p("| **em disco** | %s linhas | %s MiB | %s MiB | %s MiB | SQLite %s× |"
          % (br(n, 0), br(dp / 2**20, 1), br(dr / 2**20, 1), br(dd / 2**20, 1),
             br(dp / dr, 1)))
    _bloco("dispersao")
    _p("Dispersão das %d rodadas, em segundos por fase:" % t["rodadas"])
    _p()
    _p("| fase | PhxSql (mín · mediana · máx) | SQLite rowid (mín · mediana · máx) |")
    _p("|---|---|---|")
    for fase, (rot, _) in rotulos.items():
        def tres(m):
            xs = sorted(a[m][fase])
            return "%s · **%s** · %s" % (br(xs[0], 3), br(med(xs), 3), br(xs[-1], 3))
        _p("| %s | %s | %s |" % (rot, tres("phxsql"), tres("sqlite-rowid")))
    _bloco("disco")
    _p("Onde os %s MiB do PhxSql estão:" % br(dp / 2**20, 1))
    _p()
    _p("| arquivo | tamanho | por linha | do total |")
    _p("|---|---:|---:|---:|")
    for ext, b in sorted(a["disco_por_arquivo"].items(), key=lambda x: -x[1]):
        if b >= 1024:
            # A fatia entra na tabela porque a frase que ela sustenta -- "o
            # diario de replicacao e um sexto do disco" -- e a unica do
            # documento em que um custo medido E um recurso. Escrita a mao ela
            # envelheceria calada na primeira vez que o formato mudasse.
            _p("| `%s` | %s MiB | %s B | %s%% |"
               % (ext, br(b / 2**20, 1), br(b / n, 0), br(100 * b / dp, 0)))
    _bloco("transporte")
    d = t["d_transporte"]
    # PISO se publica pelo MENOR, e nao pela mediana -- e esta e a unica
    # tabela do documento em que isso vale. Disputa por processador so
    # ACRESCENTA a uma ida e volta; nao ha ruido que a deixe mais rapida que o
    # caminho de codigo permite. Entao a menor corrida e a melhor estimativa do
    # que a maquina cobra, e a mediana aqui mediria a maquina, nao o servidor.
    # Medido: nas corridas desta bancada o mesmo piso saiu entre 32 e 72 us
    # conforme quem mais estava usando a maquina -- 2,2x de diferenca sem uma
    # linha de codigo mudar. As outras tabelas continuam na mediana, porque
    # nelas o trabalho e longo e a disputa se dilui.
    def menor(xs):
        return min(xs) / d["pedidos"] * 1e6

    piso, eco, cli = menor(d["segundos"]), menor(d["eco"]), menor(d["so_cliente"])
    _p("| pedaço da ida e volta | µs |")
    _p("|---|---:|")
    _p("| o JSON do cliente (Python), sem soquete nenhum | %s |" % br(cli))
    _p("| o *loopback* e as chamadas de sistema | %s |" % br(eco - cli))
    _p("| **o caminho do pedido do `phxsqld`** | **%s** |" % br(piso - eco))
    _p("| total, por pedido de %d bytes | %s |" % (d["bytes_do_pedido"], br(piso)))
    _p()
    _p("Este é o único número do documento publicado pelo **menor** e não pela"
       " mediana: disputa por processador só *acrescenta* a uma ida e volta,"
       " então a menor corrida é a melhor estimativa do que o caminho cobra."
       " Nas %d corridas, a ida e volta completa ficou entre **%s** e **%s** µs."
       % (len(d["segundos"]),
          br(min(d["segundos"]) / d["pedidos"] * 1e6),
          br(max(d["segundos"]) / d["pedidos"] * 1e6)))
    _bloco("soquete")
    b = t["b_soquete"]
    _p("| fase | pelo soquete | menos o piso | a biblioteca (A) |")
    _p("|---|---:|---:|---:|")
    for fase, quantos, na_a in (
        ("inserir", n, "inserir"), ("inserir_lote", n, "inserir"),
        ("buscar", ops, "buscar"), ("varrer", faixa, "varrer"),
        ("atualizar", ops, "atualizar"), ("excluir", ops, "excluir")):
        m = med(b[fase]) / quantos * 1e6
        por_linha = piso if fase in ("inserir", "buscar", "atualizar", "excluir") else 0.0
        lib = med(a["phxsql"][na_a]) / quantos * 1e6
        _p("| `%s` | %s µs | %s µs | %s µs |"
              % (fase, br(m), br(m - por_linha), br(lib)))
    _bloco("durabilidade")
    c = t["c_durabilidade"]
    _p("| regime | o que se arrisca | PhxSql (lote da janela) | SQLite | razão |")
    _p("|---|---|---:|---:|---:|")
    risco = {
        "por_operacao": "nada, nem em queda de energia",
        "por_lote": "a janela — 200 gravações ou 200 ms",
        "sistema": "o que o sistema não descarregou",
    }
    for modo, v in c["casos"].items():
        s = med(v["sqlite"])
        if v["phxsql_lote"]:
            p = med(v["phxsql_lote"])
            _p("| `%s` | %s | %s s | %s s | %s× |"
                  % (modo, risco[modo], br(p, 3), br(s, 3), br(p / s)))
        else:
            p = med(v["phxsql_soquete"]) - piso * c["n"] / 1e6
            _p("| `%s` | %s | %s s (uma a uma, menos o piso) | %s s | %s× |"
                  % (modo, risco[modo], br(p, 3), br(s, 3), br(p / s)))
    _bloco("lote")
    e = t.get("e_tamanho_do_lote")
    if e:
        _p("| linhas por chamada | chamadas | µs por linha |")
        _p("|---:|---:|---:|")
        for bloco, v in sorted(e["blocos"].items(), key=lambda x: int(x[0])):
            bloco = int(bloco)
            _p("| %s | %s | %s |"
                  % (br(bloco, 0), br(-(-e["n"] // bloco), 0),
                     br(med(v) / e["n"] * 1e6))) 


    _bloco("fim")
    return saida


def markdown():
    """Mostra os blocos, com o nome de cada um, para conferir antes de gravar."""
    for nome, texto in blocos().items():
        if nome in ("cabecalho", "fim") or not texto:
            continue
        print(f"<!-- mobile:{nome}:inicio -->")
        print(texto)
        print(f"<!-- mobile:{nome}:fim -->")
        print()


def documento(caminho):
    """Reescreve, NO LUGAR, cada bloco marcado do documento.

    O mesmo padrao que o `docs/dossie/numeros-da-bancada.py` usa no
    `PENDENCIAS.md`, e pela mesma razao: numero visivel sai de gerador. A
    diferenca aqui e que o documento tem SETE blocos, e um deles envelhecendo
    calado enquanto os outros seis andam seria pior que todos errados juntos --
    ninguem desconfia de uma tabela cercada de tabelas certas.

    Bloco marcado que o gerador nao conhece, ou bloco que o gerador tem e o
    documento nao marca, PARA a gravacao em vez de gravar metade.
    """
    import re

    alvo = Path(caminho)
    texto = alvo.read_text(encoding="utf-8")
    tabelas = {k: v for k, v in blocos().items() if k not in ("cabecalho", "fim")}
    # `proveniencia` nao e tabela: e a linha de "medido em tal dia, com a
    # maquina assim". Sai do mesmo lugar, mas fora da conferencia de pares.
    marcados = set(re.findall(r"<!-- mobile:([a-z-]+):inicio -->", texto))
    marcados.discard("proveniencia")
    if marcados - set(tabelas):
        sys.exit("o documento marca bloco que o gerador nao tem: "
                 + ", ".join(sorted(marcados - set(tabelas))))
    if set(tabelas) - marcados:
        sys.exit("o gerador tem bloco que o documento nao marca: "
                 + ", ".join(sorted(set(tabelas) - marcados)))
    proveniencia = blocos()["cabecalho"]
    for nome, corpo in tabelas.items():
        texto = re.sub(
            r"(<!-- mobile:%s:inicio -->\n).*?(\n<!-- mobile:%s:fim -->)" % (nome, nome),
            lambda m: m.group(1) + corpo + m.group(2),
            texto,
            flags=re.S,
        )
    texto = re.sub(
        r"(<!-- mobile:proveniencia:inicio -->\n).*?(\n<!-- mobile:proveniencia:fim -->)",
        lambda m: m.group(1) + proveniencia + m.group(2),
        texto,
        flags=re.S,
    )
    alvo.write_text(texto, encoding="utf-8")
    print("gravados %d blocos em %s" % (len(tabelas), alvo))


if __name__ == "__main__":
    if "--markdown" in sys.argv:
        markdown()
    elif "--documento" in sys.argv:
        i = sys.argv.index("--documento")
        documento(sys.argv[i + 1] if len(sys.argv) > i + 1
                  else RAIZ / "docs/MOBILE.md")
    else:
        principal()
