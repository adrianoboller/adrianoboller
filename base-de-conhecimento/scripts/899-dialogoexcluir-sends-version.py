# dialogoExcluir sends version
# 28/08 23:54

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

alvo = '''async function dialogoExcluir(db, tab, rowid, aoTerminar) {'''
novo = '''async function dialogoExcluir(db, tab, rowid, aoTerminar, versao) {'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

alvo = '''      const r = await api("excluir",
        { database: db, tabela: tab, rowid, motivo, fisico: modo === "fisico" });'''
novo = '''      const r = await api("excluir",
        { database: db, tabela: tab, rowid, motivo, fisico: modo === "fisico",
          ...(versao ? { versao } : {}) });'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
