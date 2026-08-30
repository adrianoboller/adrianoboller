# Corrigir a prova e rodar do zero
# 29/08 11:43

import io
p='bancada/dblink/prova-sincronia.py'
s=io.open(p,encoding='utf-8').read()
velho='''print("== 4. conflito: o dono (aqui) vence ==")
mysql("UPDATE clientes SET cidade='ERRADA' WHERE id=1")
fala({"op": "atualizar", "database": "espelho", "tabela": "clientes", "rowid": 1,
      "valores": {"id": 1, "nome": "Adriano Boller", "cidade": "Curitiba-PR",
                   "limite": "15000.00", "desde": "2019-03-12"}})'''
novo='''print("== 4. conflito: o dono (aqui) vence ==")
mysql("UPDATE clientes SET cidade='ERRADA' WHERE id=1")
# O rowid local se ACHA pela chave -- supor que a ordem da puxada e a ordem
# dos ids foi o primeiro defeito que esta prova pegou, e era da prova.
achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": {"id": 1}})
rowid = achado["linhas"][0]["rowid"] if "linhas" in achado else achado["rowids"][0]
fala({"op": "atualizar", "database": "espelho", "tabela": "clientes", "rowid": rowid,
      "valores": {"id": 1, "nome": "Adriano Boller", "cidade": "Curitiba-PR",
                   "limite": "15000.00", "desde": "2019-03-12"}})'''
assert s.count(velho)==1
s=s.replace(velho,novo)
# e a exclusao do passo 5 tambem: rowid pela chave
velho2='''fala({"op": "excluir", "database": "espelho", "tabela": "clientes", "rowid": 2,
      "fisico": True, "motivo": "prova do limite documentado"})'''
novo2='''achado = fala({"op": "buscar", "database": "espelho", "tabela": "clientes",
               "indice": "porChave", "chave": {"id": 2}})
rowid = achado["linhas"][0]["rowid"] if "linhas" in achado else achado["rowids"][0]
fala({"op": "excluir", "database": "espelho", "tabela": "clientes", "rowid": rowid,
      "fisico": True, "motivo": "prova do limite documentado"})'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
