# Fix verification and rerun
# 29/08 00:17

import pathlib
p = pathlib.Path("bancada/carga/medir.py")
s = p.read_text()
s = s.replace('''    # Conferencia: as duas tabelas tem de ter as mesmas linhas.
    for tab in ("uma_a_uma", "em_lote"):
        r = fala({"op": "info_tabela", "database": "loja", "tabela": tab})
        assert r.get("registros") == n, f"{tab}: {r.get('registros')} de {n}"
''','''    # Conferencia: as duas metades tem de ter gravado o mesmo tanto. Comparar
    # tempo de trabalhos diferentes seria a armadilha que a bancada ja pegou
    # duas vezes.
    contas = {t["nome"]: t.get("registros")
              for t in fala({"op": "tabelas", "database": "loja"})["tabelas"]}
    for tab in ("uma_a_uma", "em_lote"):
        assert contas.get(tab) == n, f"{tab}: {contas.get(tab)} de {n}"
''')
p.write_text(s)
