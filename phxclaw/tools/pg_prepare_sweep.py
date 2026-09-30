#!/usr/bin/env python3
"""Faz o PostgreSQL PREPARAR cada SQL literal do codigo Rust.

PREPARE analisa tabela, coluna, tipo e funcao sem executar nada: e o jeito barato de
saber se o SQL embutido no motor bate com o esquema que o FULL_INSTALL cria. Sai 1 se
algum literal nao prepara. Uso: python3 tools/pg_prepare_sweep.py "host=/tmp port=55432 dbname=phxclaw_e2e user=postgres"
"""
import glob, re, subprocess, sys

dsn = sys.argv[1]
LIT = re.compile(r'r#"(.*?)"#|"((?:[^"\\]|\\.)*)"', re.S)
SQL = re.compile(r'^\s*(INSERT|UPDATE|SELECT|DELETE|WITH)\b', re.I)

def unescape(s):
    s = re.sub(r'\\\n\s*', '', s)
    return s.replace('\\"', '"').replace('\\n', '\n').replace('\\\\', '\\')

achados = []
for f in sorted(glob.glob('crates/*/src/**/*.rs', recursive=True) + glob.glob('apps/**/src/**/*.rs', recursive=True)):
    txt = open(f, encoding='utf-8').read()
    for m in LIT.finditer(txt):
        s = m.group(1) if m.group(1) is not None else unescape(m.group(2))
        if SQL.match(s) and ('$' in s or ' FROM ' in s.upper() or 'INTO' in s.upper()) and ('{' not in s):
            achados.append((f, txt.count('\n', 0, m.start()) + 1, s))

ok = falha = 0
for f, linha, s in achados:
    q = f"BEGIN; PREPARE p AS {s}; ROLLBACK;"
    r = subprocess.run(["psql", dsn, "-v", "ON_ERROR_STOP=1", "-qtA", "-c", q], capture_output=True, text=True)
    if r.returncode == 0:
        ok += 1
    else:
        falha += 1
        erro = next((l for l in r.stderr.splitlines() if 'ERROR' in l), r.stderr.strip())
        print(f"FALHA {f}:{linha} :: {erro.split('ERROR:')[-1].strip()[:160]}")
print(f"{ok} preparam, {falha} falham, {len(achados)} literais SQL")
sys.exit(1 if falha else 0)
