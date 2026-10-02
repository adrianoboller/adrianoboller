#!/usr/bin/env python3
"""Sonda VIVA do pedido 238: SQL_C_WCHAR e parametro de SAIDA, ponta a ponta.

    cargo build --release -p phxsql-server --bin phxsqld -p phxsql-odbc
    python3 bancada/odbc/sonda-viva.py [--n 20] [--porta 6958]

# O que a distingue da prova de ABI

`prova-abi.py` carrega a .so por ctypes e chama as funcoes do driver DIRETO.
Esta passa pelo GERENCIADOR DE DRIVER DE VERDADE (unixODBC, `libodbc.so.2`),
que e quem recusaria, reescreveria ou traduziria uma chamada antes de o nosso
driver ve-la. Unitario nao prova isso: a casa ja aprendeu que o que depende de
um terceiro se prova contra o terceiro.

Duas frentes, cada uma repetida N vezes (resultado com N e faixa min-max):

  WCHAR  pyodbc (que liga TODO `str` como SQL_C_WCHAR e le texto por
         SQL_C_WCHAR): insere `Sao Joao + emoji` por parametro, le de volta, e
         compara o texto byte a byte. Ida e volta em UTF-16 pelo soquete.
  SAIDA  unixODBC por ctypes (pyodbc nao liga parametro de saida): `{call p(?,?)}`
         com `SQL_PARAM_INPUT` + `SQL_PARAM_OUTPUT` em SQL_C_SLONG, depois
         OUT em SQL_C_WCHAR (texto fora do BMP) e um INOUT.

Hipoteses (escritas antes de medir; ver a cognicao do pedido 238):
  H1 o gerenciador recusa/reescreve (SQLBindParameter com tipo C que o driver
     ANSI nao anuncia);  H2 o nosso driver tem defeito de borda na conexao real
     (o que o unitario nao viu);  H3 o servidor nao devolve o `saida`/aceita `?`
     no `CALL`. Cada caso abaixo e a leitura de qual delas sobrevive.

Nunca usa `pkill`: mata so o PID do servidor que ele mesmo subiu.
"""

import ctypes
import datetime
import importlib.util
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

AQUI = os.path.dirname(os.path.abspath(__file__))

spec = importlib.util.spec_from_file_location("provar", os.path.join(AQUI, "provar.py"))
provar = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provar)


def arg(nome, padrao):
    return sys.argv[sys.argv.index(nome) + 1] if nome in sys.argv else padrao


N = int(arg("--n", "20"))
PORTA = int(arg("--porta", "6958"))
BASE = "/tmp/phx-odbc-sonda"
TEXTO = "São João \U0001F600 ok"  # BMP acentuado + par substituto

# SQLRETURN e SQLSMALLINT: 16 bits (ver a nota em prova-abi.py).
SQL_SUCCESS, SQL_COM_INFO, SQL_NO_DATA = 0, 1, 100
ENV, DBC, STMT = 1, 2, 3
NTS = -3
C_SLONG, C_WCHAR = -16, -8
SQL_INTEGER, SQL_WVARCHAR = 4, -9
P_IN, P_INOUT, P_OUT = 1, 2, 4
NULO = -1

resultados = []  # (caso, ok, ms, motivo)


def registra(caso, ok, ms, motivo=""):
    resultados.append({"caso": caso, "ok": bool(ok), "ms": ms, "motivo": motivo})


def odbc_do_gerenciador():
    d = ctypes.CDLL("libodbc.so.2", mode=ctypes.RTLD_LOCAL)
    for nome in ["SQLAllocHandle", "SQLFreeHandle", "SQLSetEnvAttr",
                 "SQLDriverConnect", "SQLDisconnect", "SQLExecDirect",
                 "SQLBindParameter", "SQLGetDiagRec", "SQLFreeStmt",
                 "SQLPrepare", "SQLExecute", "SQLNumResultCols"]:
        getattr(d, nome).restype = ctypes.c_short
    return d


def diag(d, tipo, punho):
    estado = ctypes.create_string_buffer(8)
    nativo = ctypes.c_int()
    msg = ctypes.create_string_buffer(1024)
    tam = ctypes.c_short()
    d.SQLGetDiagRec(ctypes.c_short(tipo), ctypes.c_void_p(punho), 1, estado,
                    ctypes.byref(nativo), msg, 1024, ctypes.byref(tam))
    return "%s %s" % (estado.value.decode(), msg.value.decode(errors="replace"))


def utf16(texto):
    return texto.encode("utf-16-le")


def montar_gerenciador():
    """odbcinst.ini/odbc.ini proprios: nunca toca o /etc da maquina."""
    pasta = tempfile.mkdtemp(prefix="phx-sonda-odbc-")
    with open(os.path.join(pasta, "odbcinst.ini"), "w") as f:
        f.write("[PhxSql]\nDescription=PhxSql\nDriver=%s\n" % provar.SO)
    open(os.path.join(pasta, "odbc.ini"), "w").close()
    os.environ["ODBCSYSINI"] = pasta
    os.environ["ODBCINI"] = os.path.join(pasta, "odbc.ini")
    return pasta


CONEXAO = ("Driver=PhxSql;Server=127.0.0.1;Port=%d;Token=%s;UID=%s;PWD=%s;"
           "Database=loja" % (PORTA, provar.TOKEN, provar.USUARIO, provar.SENHA))


def sonda_wchar():
    import pyodbc  # so aqui: o resto da sonda nao exige

    cn = pyodbc.connect(CONEXAO, autocommit=True)
    # pyodbc ja liga str como SQL_C_WCHAR e le texto com SQL_C_WCHAR; deixar
    # o padrao e o ponto -- e exatamente o cliente que "so fala UTF-16".
    cur = cn.cursor()
    cur.execute("DELETE FROM clientes WHERE id >= 1000")
    for i in range(N):
        chave = 1000 + i
        t0 = time.perf_counter()
        try:
            cur.execute("INSERT INTO clientes (id, nome) VALUES (?, ?)", chave, TEXTO)
            cur.execute("SELECT nome FROM clientes WHERE id = ?", chave)
            linha = cur.fetchone()
            visto = linha[0] if linha else None
            ms = (time.perf_counter() - t0) * 1000
            registra("wchar_ida_e_volta", visto == TEXTO, ms,
                     "" if visto == TEXTO else "voltou %r" % (visto,))
        except Exception as e:  # noqa: BLE001 -- a sonda REGISTRA a recusa
            registra("wchar_ida_e_volta", False,
                     (time.perf_counter() - t0) * 1000, "excecao: %s" % e)
    cur.execute("DELETE FROM clientes WHERE id >= 1000")
    for _ in range(N):
        # DML nao devolve grade: description None (antes anunciava 4 colunas).
        t0 = time.perf_counter()
        try:
            cur.execute("DELETE FROM clientes WHERE id >= 1000")
            ok = cur.description is None
            registra("dml_sem_colunas", ok, (time.perf_counter() - t0) * 1000,
                     "" if ok else "description=%r" % (cur.description,))
        except Exception as e:  # noqa: BLE001
            registra("dml_sem_colunas", False, (time.perf_counter() - t0) * 1000, str(e))
        # Erro do driver tem de chegar COM SQLSTATE e texto pelo gerenciador.
        t0 = time.perf_counter()
        try:
            cur.execute("SELECT * FROM tabela_que_nao_existe")
            registra("erro_chega_pelo_gerenciador", False, 0, "nao recusou")
        except pyodbc.Error as e:
            ok = e.args[0] == "42S02" and "nao existe" in str(e)
            registra("erro_chega_pelo_gerenciador", ok,
                     (time.perf_counter() - t0) * 1000, "" if ok else str(e))
    cn.close()


def liga(d, stmt, pos, sentido, c_tipo, sql_tipo, buf, cap, ind):
    return d.SQLBindParameter(
        stmt, ctypes.c_ushort(pos), ctypes.c_short(sentido),
        ctypes.c_short(c_tipo), ctypes.c_short(sql_tipo),
        ctypes.c_size_t(cap if sql_tipo == SQL_WVARCHAR else 10), ctypes.c_short(0),
        buf, ctypes.c_ssize_t(cap), ctypes.byref(ind))


def sonda_saida():
    d = odbc_do_gerenciador()
    env, dbc = ctypes.c_void_p(), ctypes.c_void_p()
    d.SQLAllocHandle(ENV, None, ctypes.byref(env))
    d.SQLSetEnvAttr(env, 200, ctypes.c_void_p(3), 0)  # SQL_ATTR_ODBC_VERSION=3
    d.SQLAllocHandle(DBC, env, ctypes.byref(dbc))
    saida = ctypes.create_string_buffer(1024)
    tam = ctypes.c_short()
    r = d.SQLDriverConnect(dbc, None, CONEXAO.encode(), NTS, saida, 1024,
                           ctypes.byref(tam), 0)
    if r not in (SQL_SUCCESS, SQL_COM_INFO):
        registra("saida_conexao", False, 0, diag(d, DBC, dbc.value))
        return

    def exec_direto(sql):
        st = ctypes.c_void_p()
        d.SQLAllocHandle(STMT, dbc, ctypes.byref(st))
        rr = d.SQLExecDirect(st, sql.encode(), NTS)
        msg = "" if rr in (SQL_SUCCESS, SQL_COM_INFO) else diag(d, STMT, st.value)
        d.SQLFreeHandle(STMT, st)
        return rr, msg

    exec_direto("DROP PROCEDURE sonda_dobro")
    exec_direto("DROP PROCEDURE sonda_eco")
    exec_direto("DROP PROCEDURE sonda_inc")
    for sql in ("CREATE PROCEDURE sonda_dobro(IN x INT, OUT y INT) SET y = x * 2",
                "CREATE PROCEDURE sonda_eco(IN t VARCHAR(40), OUT u VARCHAR(40)) SET u = t",
                "CREATE PROCEDURE sonda_inc(INOUT n INT) SET n = n + 1"):
        rr, msg = exec_direto(sql)
        if rr not in (SQL_SUCCESS, SQL_COM_INFO):
            registra("saida_criar_procedimento", False, 0, msg)
            return

    for i in range(N):
        # (a) OUT inteiro: {call sonda_dobro(?, ?)} com IN e OUT
        t0 = time.perf_counter()
        st = ctypes.c_void_p()
        d.SQLAllocHandle(STMT, dbc, ctypes.byref(st))
        x, y = ctypes.c_int32(21 + i), ctypes.c_int32(-1)
        ix, iy = ctypes.c_ssize_t(0), ctypes.c_ssize_t(0)
        liga(d, st, 1, P_IN, C_SLONG, SQL_INTEGER, ctypes.byref(x), 4, ix)
        liga(d, st, 2, P_OUT, C_SLONG, SQL_INTEGER, ctypes.byref(y), 4, iy)
        rr = d.SQLExecDirect(st, b"{call sonda_dobro(?, ?)}", NTS)
        ms = (time.perf_counter() - t0) * 1000
        if rr in (SQL_SUCCESS, SQL_COM_INFO):
            registra("saida_inteiro", y.value == 2 * (21 + i), ms,
                     "" if y.value == 2 * (21 + i) else "buffer ficou %d" % y.value)
        else:
            registra("saida_inteiro", False, ms, diag(d, STMT, st.value))
        d.SQLFreeHandle(STMT, st)

        # (b) OUT em SQL_C_WCHAR (texto fora do BMP), IN tambem WCHAR
        t0 = time.perf_counter()
        st = ctypes.c_void_p()
        d.SQLAllocHandle(STMT, dbc, ctypes.byref(st))
        entrada = ctypes.create_string_buffer(utf16(TEXTO), 128)
        voltou = ctypes.create_string_buffer(128)
        ie = ctypes.c_ssize_t(len(utf16(TEXTO)))
        iv = ctypes.c_ssize_t(0)
        liga(d, st, 1, P_IN, C_WCHAR, SQL_WVARCHAR, entrada, 128, ie)
        liga(d, st, 2, P_OUT, C_WCHAR, SQL_WVARCHAR, voltou, 128, iv)
        rr = d.SQLExecDirect(st, b"{call sonda_eco(?, ?)}", NTS)
        ms = (time.perf_counter() - t0) * 1000
        if rr in (SQL_SUCCESS, SQL_COM_INFO):
            lido = voltou.raw[:max(iv.value, 0)].decode("utf-16-le", errors="replace")
            ok = lido == TEXTO
            registra("saida_wchar", ok, ms,
                     "" if ok else "indicador=%d buffer=%r" % (iv.value, lido))
        else:
            registra("saida_wchar", False, ms, diag(d, STMT, st.value))
        d.SQLFreeHandle(STMT, st)

        # (c) INOUT inteiro: entra 5+i, o procedimento devolve +1 no MESMO buffer
        t0 = time.perf_counter()
        st = ctypes.c_void_p()
        d.SQLAllocHandle(STMT, dbc, ctypes.byref(st))
        n = ctypes.c_int32(5 + i)
        inn = ctypes.c_ssize_t(0)
        liga(d, st, 1, P_INOUT, C_SLONG, SQL_INTEGER, ctypes.byref(n), 4, inn)
        rr = d.SQLExecDirect(st, b"{call sonda_inc(?)}", NTS)
        ms = (time.perf_counter() - t0) * 1000
        if rr in (SQL_SUCCESS, SQL_COM_INFO):
            registra("saida_inout", n.value == 6 + i, ms,
                     "" if n.value == 6 + i else "buffer ficou %d" % n.value)
        else:
            registra("saida_inout", False, ms, diag(d, STMT, st.value))
        d.SQLFreeHandle(STMT, st)

    exec_direto("DROP PROCEDURE sonda_dobro")
    exec_direto("DROP PROCEDURE sonda_eco")
    exec_direto("DROP PROCEDURE sonda_inc")
    d.SQLDisconnect(dbc)


def resumo():
    por_caso = {}
    for r in resultados:
        por_caso.setdefault(r["caso"], []).append(r)
    saida = {}
    for caso, rs in por_caso.items():
        ms = [r["ms"] for r in rs]
        saida[caso] = {
            "n": len(rs),
            "ok": sum(1 for r in rs if r["ok"]),
            "ms_min": round(min(ms), 2),
            "ms_mediana": round(statistics.median(ms), 2),
            "ms_max": round(max(ms), 2),
            "motivos": sorted({r["motivo"] for r in rs if r["motivo"]})[:3],
        }
    return saida


def main():
    for caminho in (provar.PHXSQLD, provar.SO):
        if not os.path.exists(caminho):
            sys.exit("nao achei %s -- compile antes (ver o topo)" % caminho)
    try:
        import pyodbc  # noqa: F401
    except ImportError:
        sys.exit("pyodbc ausente: `pip3 install pyodbc` (precisa de unixodbc-dev)")

    provar.PORTA = PORTA
    shutil.rmtree(BASE, ignore_errors=True)
    os.makedirs(BASE)
    with open(os.path.join(BASE, "config.json"), "w") as f:
        json.dump({"base": "base", "bind": "127.0.0.1:%d" % PORTA,
                   "token": provar.TOKEN, "cifra_fio": {"exigir": False},
                   "web": {"ligado": False},
                   "root": {"id": 1, "nome": "root", "login": provar.USUARIO,
                            "senha_hash": provar.hash_da_senha(provar.SENHA)}},
                  f, indent=2)
    log = open(os.path.join(BASE, "servidor.log"), "a")
    proc = subprocess.Popen([provar.PHXSQLD], cwd=BASE, stdout=log,
                            stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    pasta = montar_gerenciador()
    try:
        for _ in range(60):
            time.sleep(0.25)
            if provar.no_ar():
                break
        else:
            sys.exit("o servidor nao subiu; veja %s/servidor.log" % BASE)
        subprocess.run([sys.executable, os.path.join(AQUI, "montar-dados.py"),
                        "127.0.0.1", str(PORTA), provar.TOKEN, provar.USUARIO,
                        provar.SENHA], check=True)
        sonda_wchar()
        sonda_saida()
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=10)
        log.close()
        shutil.rmtree(pasta, ignore_errors=True)

    res = resumo()
    falhou = False
    for caso, r in res.items():
        marca = "OK  " if r["ok"] == r["n"] else "FALHA"
        falhou |= r["ok"] != r["n"]
        print("[%s] %-22s %d/%d  ms min/med/max = %s/%s/%s %s" % (
            marca, caso, r["ok"], r["n"], r["ms_min"], r["ms_mediana"],
            r["ms_max"], r["motivos"] or ""))
    esperados = {"wchar_ida_e_volta", "dml_sem_colunas",
                 "erro_chega_pelo_gerenciador", "saida_inteiro", "saida_wchar",
                 "saida_inout"}
    ausentes = sorted(esperados - set(res))
    if ausentes:
        falhou = True
        print("NAO MEDIDOS (a sonda parou antes):", ausentes)
    with open(os.path.join(AQUI, "resultados.json"), "w") as f:
        json.dump({
            "data": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
            "n": N,
            "gerenciador": "unixODBC 2.3.12 (libodbc.so.2) + pyodbc "
                           + __import__("pyodbc").version,
            "texto": TEXTO,
            "casos": res,
            "ausentes": ausentes,
            "tudo_ok": not falhou,
        }, f, indent=2, ensure_ascii=False)
    return 1 if falhou else 0


if __name__ == "__main__":
    raise SystemExit(main())
