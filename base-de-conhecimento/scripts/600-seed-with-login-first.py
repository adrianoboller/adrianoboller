# Seed with login first
# 28/08 17:52

import io
s=io.open('semear.py',encoding='utf-8').read()
s=s.replace('''pedidos = [
    {"op":"criar_database","database":"loja"},''','''pedidos = [
    {"op":"login","usuario":"adm","senha":"segredo1"},
    {"op":"criar_database","database":"loja"},''',1)
io.open('semear.py','w',encoding='utf-8').write(s)
