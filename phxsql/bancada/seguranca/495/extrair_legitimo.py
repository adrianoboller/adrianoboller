#!/usr/bin/env python3
"""Extrai o SQL LEGITIMO que o proprio repositorio ja tem, do CODIGO (nunca digitado).

Fontes: literais de string Rust em crates/**/*.rs, constantes de string Python em
bancada/**/*.py e testes-web/**, literais JS em crates/phxsql-server/ui/*.js.
Saida: legitimo.jsonl  ({"sql":..., "origem":"arquivo:linha"}), deduplicado por texto.

Filtro: o literal comeca (apos espaco) por um verbo SQL. Literais com `{`
(modelo de format!/f-string) ficam fora e sao contados.
"""
import ast, json, os, re, sys, glob

RAIZ = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
SAIDA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "legitimo.jsonl")
VERBOS = re.compile(r"^\s*(SELECT|INSERT|UPDATE|DELETE|WITH|CREATE|DROP|ALTER|CALL|SHOW|BEGIN|COMMIT|ROLLBACK|SAVEPOINT|RELEASE|BULKINSERT|GRANT|REVOKE|EXPLAIN|SET|START|LOCK|SIGNAL)\b", re.I)
# precisa parecer um comando e nao uma frase de erro: pelo menos 2 palavras, e
# nenhuma letra acentuada fora de aspas simples seria frase; nao filtra por isso
# (o motor aceita acento em dado) -- quem filtra frase e o crivo abaixo.
FRASE = re.compile(r"^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|DROP|ALTER|SET|SHOW|WITH)\s+(e|é|com|sem|nao|não|que|de|do|da|so|só|sobre|por|para|ou)\b", re.I)

achados = {}
fora_modelo = 0
por_fonte = {}

def guardar(sql, origem):
    global fora_modelo
    if not VERBOS.match(sql) or FRASE.match(sql):
        return
    if "{" in sql or "}" in sql or "${" in sql:
        fora_modelo += 1
        return
    s = sql.strip()
    if len(s) < 6:
        return
    if s not in achados:
        achados[s] = origem
        chave = origem.split("/")[0] if "/" in origem else origem
        por_fonte[chave] = por_fonte.get(chave, 0) + 1

# --- Rust: analisador de literal (normal, raw, continuacao de linha)
def literais_rust(texto):
    i, n, linha = 0, len(texto), 1
    while i < n:
        c = texto[i]
        if c == "\n":
            linha += 1; i += 1; continue
        # comentario de linha
        if texto.startswith("//", i):
            j = texto.find("\n", i)
            i = n if j < 0 else j
            continue
        if texto.startswith("/*", i):
            j = texto.find("*/", i + 2)
            linha += texto.count("\n", i, n if j < 0 else j)
            i = n if j < 0 else j + 2
            continue
        # char literal 'x' ou lifetime: pula simples
        if c == "'":
            m = re.match(r"'(\\.|[^'\\])'", texto[i:i+6])
            i += len(m.group(0)) if m else 1
            continue
        m = re.match(r'(b?)r(#*)"', texto[i:i+10])
        if m and (i == 0 or not (texto[i-1].isalnum() or texto[i-1] == "_")):
            hashes = m.group(2)
            fim = '"' + hashes
            ini = i + len(m.group(0))
            j = texto.find(fim, ini)
            if j < 0: return
            val = texto[ini:j]
            yield linha, val
            linha += val.count("\n")
            i = j + len(fim)
            continue
        if c == '"':
            j = i + 1; buf = []; l0 = linha
            while j < n and texto[j] != '"':
                if texto[j] == "\\":
                    nx = texto[j+1]
                    if nx == "\n":
                        linha += 1
                        j += 2
                        while j < n and texto[j] in " \t\n\r":
                            if texto[j] == "\n": linha += 1
                            j += 1
                        continue
                    mapa = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"', "'": "'", "0": "\0"}
                    if nx in mapa:
                        buf.append(mapa[nx]); j += 2; continue
                    if nx == "u":
                        k = texto.find("}", j)
                        buf.append(chr(int(texto[j+3:k], 16))); j = k + 1; continue
                    if nx == "x":
                        buf.append(chr(int(texto[j+2:j+4], 16))); j += 4; continue
                    buf.append(nx); j += 2; continue
                if texto[j] == "\n": linha += 1
                buf.append(texto[j]); j += 1
            yield l0, "".join(buf)
            i = j + 1
            continue
        i += 1

for arq in sorted(glob.glob(f"{RAIZ}/crates/**/*.rs", recursive=True)):
    if "/target/" in arq: continue
    rel = os.path.relpath(arq, RAIZ)
    try:
        t = open(arq, encoding="utf-8").read()
    except Exception:
        continue
    for linha, val in literais_rust(t):
        guardar(val, f"{rel}:{linha}")

# --- Python
for arq in sorted(glob.glob(f"{RAIZ}/bancada/**/*.py", recursive=True) + glob.glob(f"{RAIZ}/testes-web/**/*.py", recursive=True) + glob.glob(f"{RAIZ}/*.py")):
    rel = os.path.relpath(arq, RAIZ)
    try:
        arv = ast.parse(open(arq, encoding="utf-8").read())
    except Exception:
        continue
    for no in ast.walk(arv):
        if isinstance(no, ast.Constant) and isinstance(no.value, str):
            guardar(no.value, f"{rel}:{no.lineno}")

# --- JS / HTML da interface e testes-web
for arq in sorted(glob.glob(f"{RAIZ}/crates/phxsql-server/ui/**/*.js", recursive=True) + glob.glob(f"{RAIZ}/crates/phxsql-server/ui/*.html") + glob.glob(f"{RAIZ}/testes-web/**/*.js", recursive=True) + glob.glob(f"{RAIZ}/testes-web/**/*.mjs", recursive=True)):
    rel = os.path.relpath(arq, RAIZ)
    t = open(arq, encoding="utf-8").read()
    for m in re.finditer(r"'((?:\\.|[^'\\\n])*)'|\"((?:\\.|[^\"\\\n])*)\"|`((?:\\.|[^`\\])*)`", t):
        val = next(g for g in m.groups() if g is not None)
        val = val.replace("\\n", "\n").replace("\\'", "'").replace('\\"', '"')
        guardar(val, f"{rel}:{t.count(chr(10), 0, m.start()) + 1}")

# bancada/comando.sql
p = f"{RAIZ}/bancada/comando.sql"
if os.path.exists(p):
    for k, l in enumerate(open(p, encoding="utf-8").read().split(";\n")):
        guardar(l, f"bancada/comando.sql:{k}")

with open(SAIDA, "w", encoding="utf-8") as f:
    for s, o in achados.items():
        f.write(json.dumps({"sql": s, "origem": o}, ensure_ascii=False) + "\n")
print(f"legitimo: {len(achados)} comandos distintos; fora por ser modelo com chaves: {fora_modelo}")
print("por raiz de origem:", json.dumps(por_fonte, ensure_ascii=False))
