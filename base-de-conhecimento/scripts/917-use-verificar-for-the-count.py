# Use verificar for the count
# 29/08 00:17

import pathlib
p = pathlib.Path("bancada/carga/medir.py")
s = p.read_text()
s = s.replace('''    contas = {t["nome"]: t.get("registros")
              for t in fala({"op": "tabelas", "database": "loja"})["tabelas"]}
    for tab in ("uma_a_uma", "em_lote"):
        assert contas.get(tab) == n, f"{tab}: {contas.get(tab)} de {n}"''',
'''    for tab in ("uma_a_uma", "em_lote"):
        r = fala({"op": "verificar", "database": "loja", "tabela": tab})
        assert r.get("registros") == n, f"{tab}: {r.get('registros')} de {n}"''')
p.write_text(s)
