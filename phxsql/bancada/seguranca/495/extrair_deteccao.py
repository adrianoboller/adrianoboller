#!/usr/bin/env python3
"""Corpus de DETECCAO: so o que o repositorio JA tem, lido do codigo.

- ARSENAL de bancada/seguranca/injecao.py (12), rotulado pelo proprio nome
  que o repositorio deu (o que ele chama de DADO/comentario legitimo nao e ataque);
- as strings de `comando_empilhado_acha_o_segundo_comando` (sintaxe.rs).
escapado.jsonl: cada texto do ARSENAL posto como DADO, aspa dobrada -- o que
uma aplicacao SEGURA mandaria. Acusar qualquer um e falso positivo.
"""
import ast, json, os, re
RAIZ = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
AQUI = os.path.dirname(os.path.abspath(__file__))
src = open(f"{RAIZ}/bancada/seguranca/injecao.py", encoding="utf-8").read()
arsenal = None
for no in ast.parse(src).body:
    if isinstance(no, ast.Assign) and any(getattr(t, "id", "") == "ARSENAL" for t in no.targets):
        arsenal = ast.literal_eval(no.value)
# Rotulo: o repositorio classifica estes como NAO ataque em
# sintaxe.rs::comando_empilhado_nao_acusa_o_legitimo ou pelo proprio nome.
NAO_ATAQUE = {"UNION SELECT", "comentario /* */ no meio", "comentario como separador",
              "comentario -- no fim", "aspa escapada como DADO", "DROP direto", "byte nulo no texto"}
det = []
for nome, texto in arsenal:
    det.append({"sql": texto, "origem": f"injecao.py ARSENAL «{nome}»",
                "rotulo": "legitimo" if nome in NAO_ATAQUE else "ataque"})
rs = open(f"{RAIZ}/crates/phxsql-sql/src/sintaxe.rs", encoding="utf-8").read()
k = rs.index("fn comando_empilhado_acha_o_segundo_comando")
bloco = rs[k:rs.index("}\n", rs.index("] {", k))]
for m in re.finditer(r'"((?:\\.|[^"\\])*)"', bloco[:bloco.index("] {")]):
    t = m.group(1).replace('\\"', '"')
    if not any(d["sql"] == t for d in det):
        det.append({"sql": t, "origem": "sintaxe.rs comando_empilhado_acha_o_segundo_comando", "rotulo": "ataque"})
with open(f"{AQUI}/deteccao.jsonl", "w", encoding="utf-8") as f:
    for d in det:
        f.write(json.dumps(d, ensure_ascii=False) + "\n")
with open(f"{AQUI}/escapado.jsonl", "w", encoding="utf-8") as f:
    for nome, texto in arsenal:
        v = texto.replace("\x00", "").replace("'", "''")
        for modelo in ("SELECT * FROM clientes WHERE obs = '{}'",
                       "INSERT INTO comentarios (autor, texto) VALUES ('ana', '{}')",
                       "UPDATE clientes SET obs = '{}' WHERE id = 7"):
            f.write(json.dumps({"sql": modelo.format(v), "origem": f"ARSENAL «{nome}» como dado"}, ensure_ascii=False) + "\n")
print("deteccao:", len(det), "ataque:", sum(d["rotulo"] == "ataque" for d in det),
      "| escapado:", 3 * len(arsenal))
