#!/usr/bin/env python3
"""A prova de ABI do driver ODBC, contra um phxsqld de verdade.

Nao passa pelo unixODBC de proposito: o ctypes carrega a MESMA .so que o
gerenciador de driver carregaria e chama as MESMAS funcoes `extern "system"`
-- e prova de ABI no sentido literal, sem depender de pacote instalado.

O que ela prova, na ordem -- cada passo com o resultado esperado escrito
ANTES de rodar, que e o que separa prova de demonstracao:

1. o ciclo de handles ODBC 3.x abre: ENV -> versao -> DBC -> conexao;
2. a connection string devolvida NAO carrega senha nem token;
3. `SELECT *` descreve as quatro colunas com tipo honesto: INT vira
   SQL_INTEGER, Str(40) vira SQL_VARCHAR(40), Decimal(12,2) vira
   SQL_DECIMAL com precisao 12 e escala 2, Date vira SQL_TYPE_DATE;
4. fetch + SQLGetData devolvem os valores INSERIDOS, o decimal com as duas
   casas, o NULL pelo indicador, e SQL_NO_DATA depois da ultima linha;
5. coluna amarrada (SQLBindCol de inteiro) chega pelo SQLFetch;
6. projecao com WHERE pela chave devolve so as colunas pedidas;
7. COUNT(*) vira uma grade de uma celula SQL_BIGINT;
8. erro proposital da SQL_ERROR e o SQLGetDiagRec conta o motivo -- sintaxe
   sai como 42000, e o diagnostico nunca contem a senha;
9. buffer curto no SQLGetData trunca AVISANDO (01004, SUCCESS_WITH_INFO),
   a proxima chamada continua de onde parou e o fim e SQL_NO_DATA. Este e
   o passo do defeito reposto documentado em docs/ODBC.md.

Requisitos: um phxsqld com o banco montado pelo montar-dados.py ao lado
(ver docs/ODBC.md, secao da prova). Uso:

    python3 prova-abi.py <caminho da libphxsql_odbc.so> [host porta token usuario senha]
"""
import ctypes, sys

LIB = sys.argv[1] if len(sys.argv) > 1 else "../../target/release/libphxsql_odbc.so"
HOST = sys.argv[2] if len(sys.argv) > 2 else "127.0.0.1"
PORTA = sys.argv[3] if len(sys.argv) > 3 else "5305"
TOKEN = sys.argv[4] if len(sys.argv) > 4 else "prova-odbc"
USUARIO = sys.argv[5] if len(sys.argv) > 5 else "root"
SENHA = sys.argv[6] if len(sys.argv) > 6 else "prova123"

SUCESSO, COM_INFO, ERRO, INVALIDO, SEM_DADO = 0, 1, -1, -2, 100
ENV, DBC, STMT = 1, 2, 3
NTS = -3
C_CHAR, C_SLONG = 1, -16
NULO = -1  # SQL_NULL_DATA

d = ctypes.CDLL(LIB, mode=ctypes.RTLD_LOCAL)

# SQLRETURN e SQLSMALLINT: 16 bits. Sem declarar o restype o ctypes le 32,
# e os 16 de cima sao lixo LEGITIMO da ABI (o callee so promete os 16 de
# baixo) -- a primeira rodada desta prova viu 1990525028 onde o driver
# devolveu 100, e o defeito era DAQUI. O gerenciador de driver de verdade
# declara short e nunca ve isso.
for nome_fn in ["SQLAllocHandle", "SQLFreeHandle", "SQLFreeStmt",
                "SQLSetEnvAttr", "SQLDriverConnect", "SQLConnect",
                "SQLDisconnect", "SQLExecDirect", "SQLNumResultCols",
                "SQLDescribeCol", "SQLColAttribute", "SQLBindCol",
                "SQLFetch", "SQLGetData", "SQLRowCount", "SQLGetDiagRec",
                "SQLGetInfo", "SQLSetConnectAttr", "SQLSetStmtAttr",
                "SQLPrepare", "SQLExecute"]:
    getattr(d, nome_fn).restype = ctypes.c_short

# Os parametros SQLLEN sao do tamanho do ponteiro; declarar evita depender
# de o libffi estender o int de 32 sozinho.
d.SQLBindCol.argtypes = [ctypes.c_void_p, ctypes.c_ushort, ctypes.c_short,
                         ctypes.c_void_p, ctypes.c_ssize_t, ctypes.c_void_p]
d.SQLGetData.argtypes = [ctypes.c_void_p, ctypes.c_ushort, ctypes.c_short,
                         ctypes.c_void_p, ctypes.c_ssize_t, ctypes.c_void_p]

falhas = []

def confere(rotulo, visto, esperado):
    ok = visto == esperado
    print(f"  {'ok ' if ok else 'ERRO'} {rotulo}: {visto!r}" +
          ("" if ok else f" (esperava {esperado!r})"))
    if not ok:
        falhas.append(rotulo)

def diag(tipo, punho):
    estado = ctypes.create_string_buffer(6)
    nativo = ctypes.c_int(0)
    msg = ctypes.create_string_buffer(1024)
    tam = ctypes.c_short(0)
    r = d.SQLGetDiagRec(ctypes.c_short(tipo), punho, ctypes.c_short(1),
                        estado, ctypes.byref(nativo), msg, 1024, ctypes.byref(tam))
    return (r, estado.value.decode(), msg.value.decode(), nativo.value)

print("== 1. ciclo de handles: ENV -> versao ODBC 3 -> DBC -> conexao ==")
env = ctypes.c_void_p()
confere("SQLAllocHandle(ENV)", d.SQLAllocHandle(ENV, None, ctypes.byref(env)), SUCESSO)
confere("SQLSetEnvAttr(ODBC3)",
        d.SQLSetEnvAttr(env, 200, ctypes.c_void_p(3), 0), SUCESSO)
dbc = ctypes.c_void_p()
confere("SQLAllocHandle(DBC)", d.SQLAllocHandle(DBC, env, ctypes.byref(dbc)), SUCESSO)

receita = (f"Driver=PhxSql;Server={HOST};Port={PORTA};Token={TOKEN};"
           f"UID={USUARIO};PWD={SENHA};Database=loja").encode()
volta = ctypes.create_string_buffer(512)
tam_volta = ctypes.c_short(0)
confere("SQLDriverConnect", d.SQLDriverConnect(
    dbc, None, receita, NTS, volta, 512, ctypes.byref(tam_volta), 0), SUCESSO)

print("== 2. a connection string de volta nao vaza segredo ==")
texto_volta = volta.value.decode()
confere("senha fora da volta", SENHA in texto_volta, False)
confere("token fora da volta", TOKEN in texto_volta, False)
confere("usuario continua na volta", f"UID={USUARIO}" in texto_volta, True)

stmt = ctypes.c_void_p()
confere("SQLAllocHandle(STMT)", d.SQLAllocHandle(STMT, dbc, ctypes.byref(stmt)), SUCESSO)

print("== 3. SELECT * descreve as colunas com tipo honesto ==")
confere("SQLExecDirect(SELECT *)",
        d.SQLExecDirect(stmt, b"SELECT * FROM clientes", NTS), SUCESSO)
n_col = ctypes.c_short(0)
confere("SQLNumResultCols", d.SQLNumResultCols(stmt, ctypes.byref(n_col)), SUCESSO)
confere("quatro colunas", n_col.value, 4)

# (nome, tipo SQL, tamanho, decimais, nulavel) -- SQL_INTEGER=4, SQL_VARCHAR=12,
# SQL_DECIMAL=3, SQL_TYPE_DATE=91; id e obrigatoria, o resto aceita NULL.
ESPERADAS = [("id", 4, 10, 0, 0), ("nome", 12, 40, 0, 1),
             ("limite", 3, 12, 2, 1), ("desde", 91, 10, 0, 1)]
for i, (nome_e, tipo_e, tam_e, dec_e, nul_e) in enumerate(ESPERADAS, start=1):
    nome = ctypes.create_string_buffer(64)
    tam_nome, tipo = ctypes.c_short(0), ctypes.c_short(0)
    tamanho = ctypes.c_size_t(0)
    dec, nul = ctypes.c_short(0), ctypes.c_short(0)
    r = d.SQLDescribeCol(stmt, i, nome, 64, ctypes.byref(tam_nome),
                         ctypes.byref(tipo), ctypes.byref(tamanho),
                         ctypes.byref(dec), ctypes.byref(nul))
    confere(f"col {i}", (r, nome.value.decode(), tipo.value,
                          tamanho.value, dec.value, nul.value),
            (SUCESSO, nome_e, tipo_e, tam_e, dec_e, nul_e))

print("== 4/5. fetch devolve o que foi inserido; o NULL vem pelo indicador ==")
INSERIDAS = [(1, "Adriano Boller", "15000.00", "2019-03-12"),
             (2, "Maria Operadora", "4200.50", "2021-07-01"),
             (3, "Carlos Consulta", None, "2024-12-25")]
id_amarrado = ctypes.c_int(0)
ind_id = ctypes.c_ssize_t(0)
confere("SQLBindCol(id como SQL_C_SLONG)",
        d.SQLBindCol(stmt, 1, C_SLONG, ctypes.byref(id_amarrado), 4,
                     ctypes.byref(ind_id)), SUCESSO)

def pega_texto(coluna):
    buf = ctypes.create_string_buffer(128)
    ind = ctypes.c_ssize_t(0)
    r = d.SQLGetData(stmt, coluna, C_CHAR, buf, 128, ctypes.byref(ind))
    if ind.value == NULO:
        return (r, None)
    return (r, buf.value.decode())

for id_e, nome_e, limite_e, desde_e in INSERIDAS:
    confere(f"SQLFetch da linha {id_e}", d.SQLFetch(stmt), SUCESSO)
    confere(f"  id amarrado", id_amarrado.value, id_e)
    confere(f"  nome", pega_texto(2), (SUCESSO, nome_e))
    confere(f"  limite com as duas casas", pega_texto(3), (SUCESSO, limite_e))
    confere(f"  desde", pega_texto(4), (SUCESSO, desde_e))
confere("SQLFetch depois da ultima", d.SQLFetch(stmt), SEM_DADO)
linhas = ctypes.c_ssize_t(0)
confere("SQLRowCount", d.SQLRowCount(stmt, ctypes.byref(linhas)), SUCESSO)
confere("tres linhas", linhas.value, 3)

print("== 6. projecao com WHERE pela chave ==")
confere("SQLFreeStmt(CLOSE)", d.SQLFreeStmt(stmt, 0), SUCESSO)
confere("SQLFreeStmt(UNBIND)", d.SQLFreeStmt(stmt, 2), SUCESSO)
confere("SQLExecDirect(projecao)", d.SQLExecDirect(
    stmt, b"SELECT nome, limite FROM clientes WHERE id = 2", NTS), SUCESSO)
d.SQLNumResultCols(stmt, ctypes.byref(n_col))
confere("duas colunas", n_col.value, 2)
tipo = ctypes.c_short(0)
dec = ctypes.c_short(0)
d.SQLDescribeCol(stmt, 2, ctypes.create_string_buffer(64), 64,
                 ctypes.byref(ctypes.c_short(0)), ctypes.byref(tipo),
                 ctypes.byref(ctypes.c_size_t(0)), ctypes.byref(dec),
                 ctypes.byref(ctypes.c_short(0)))
confere("limite continua SQL_DECIMAL(escala 2)", (tipo.value, dec.value), (3, 2))
confere("fetch", d.SQLFetch(stmt), SUCESSO)
confere("nome da linha 2", pega_texto(1), (SUCESSO, "Maria Operadora"))
confere("limite da linha 2", pega_texto(2), (SUCESSO, "4200.50"))

print("== 7. COUNT(*) vira uma celula SQL_BIGINT ==")
d.SQLFreeStmt(stmt, 0)
confere("SQLExecDirect(COUNT)", d.SQLExecDirect(
    stmt, b"SELECT COUNT(*) FROM clientes", NTS), SUCESSO)
d.SQLNumResultCols(stmt, ctypes.byref(n_col))
tipo = ctypes.c_short(0)
d.SQLDescribeCol(stmt, 1, ctypes.create_string_buffer(64), 64,
                 ctypes.byref(ctypes.c_short(0)), ctypes.byref(tipo),
                 ctypes.byref(ctypes.c_size_t(0)), ctypes.byref(ctypes.c_short(0)),
                 ctypes.byref(ctypes.c_short(0)))
confere("uma coluna SQL_BIGINT", (n_col.value, tipo.value), (1, -5))
d.SQLFetch(stmt)
confere("a contagem", pega_texto(1), (SUCESSO, "3"))

print("== 7b. prepare/execute -- o caminho do isql ==")
d.SQLFreeStmt(stmt, 0)
confere("SQLExecute sem prepare e HY010", d.SQLExecute(stmt), ERRO)
confere("SQLPrepare", d.SQLPrepare(
    stmt, b"SELECT nome FROM clientes WHERE id = 3", NTS), SUCESSO)
confere("SQLExecute", d.SQLExecute(stmt), SUCESSO)
confere("fetch do preparado", d.SQLFetch(stmt), SUCESSO)
confere("o nome da linha 3", pega_texto(1), (SUCESSO, "Carlos Consulta"))

print("== 8. erro proposital: SQL_ERROR com diagnostico que nao vaza senha ==")
d.SQLFreeStmt(stmt, 0)
confere("tabela inexistente da erro", d.SQLExecDirect(
    stmt, b"SELECT * FROM nao_existe", NTS), ERRO)
r, estado, msg, nativo = diag(STMT, stmt)
confere("ha um registro de diagnostico", r, SUCESSO)
confere("tabela que nao existe e 42S02", estado, "42S02")
confere("o codigo do servidor vem como erro nativo", nativo, 3001)
confere("a mensagem diz quem falou", msg.startswith("[PhxSql][ODBC]"), True)
confere("a mensagem nao esta vazia", len(msg) > 20, True)
confere("a senha nao esta na mensagem", SENHA in msg, False)
print(f"       ({estado}: {msg[:70]}...)")

confere("erro de sintaxe da erro", d.SQLExecDirect(
    stmt, b"SELECT FROM clientes", NTS), ERRO)
_, estado, _, _ = diag(STMT, stmt)
confere("sintaxe sai como 42000", estado, "42000")

print("== 9. buffer curto trunca AVISANDO e continua -- o passo do defeito reposto ==")
d.SQLFreeStmt(stmt, 0)
d.SQLExecDirect(stmt, b"SELECT * FROM clientes WHERE id = 1", NTS)
confere("fetch", d.SQLFetch(stmt), SUCESSO)
buf = ctypes.create_string_buffer(8)
ind = ctypes.c_ssize_t(0)
# "Adriano Boller" tem 14 bytes; 8 de buffer levam 7 + NUL.
confere("truncou com SUCCESS_WITH_INFO",
        d.SQLGetData(stmt, 2, C_CHAR, buf, 8, ctypes.byref(ind)), COM_INFO)
r, estado, _, _ = diag(STMT, stmt)
confere("o aviso e 01004", estado, "01004")
confere("o pedaco que coube", buf.value.decode(), "Adriano")
confere("o indicador diz quanto havia", ind.value, 14)
confere("a continuacao vem inteira",
        d.SQLGetData(stmt, 2, C_CHAR, buf, 8, ctypes.byref(ind)), SUCESSO)
confere("o resto do nome", buf.value.decode(), " Boller")
confere("depois do fim, SQL_NO_DATA",
        d.SQLGetData(stmt, 2, C_CHAR, buf, 8, ctypes.byref(ind)), SEM_DADO)

print("== 10. desmonta tudo na ordem ==")
confere("SQLFreeHandle(STMT)", d.SQLFreeHandle(STMT, stmt), SUCESSO)
confere("liberar DBC conectado e recusado", d.SQLFreeHandle(DBC, dbc), ERRO)
confere("SQLDisconnect", d.SQLDisconnect(dbc), SUCESSO)
confere("SQLFreeHandle(DBC)", d.SQLFreeHandle(DBC, dbc), SUCESSO)
confere("SQLFreeHandle(ENV)", d.SQLFreeHandle(ENV, env), SUCESSO)
confere("handle liberado vira invalido", d.SQLFreeHandle(ENV, env), INVALIDO)

if falhas:
    print(f"\nPROVA FALHOU: {len(falhas)} conferencia(s): {falhas}")
    sys.exit(1)
print("\nPROVA COMPLETA: a .so carregada por dlopen respondeu a sequencia ODBC")
print("inteira com os valores inseridos, os tipos do esquema e o truncamento")
print("avisado -- cada afirmacao acima foi conferida, nao so impressa.")
