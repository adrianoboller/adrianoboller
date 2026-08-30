# Add new ops to the manual operations list
# 28/08 18:04

import io
p='MANUAL.txt'
s=io.open(p,encoding='utf-8').read()
velho='''    excluir         database, tabela, rowid'''
novo='''    excluir         database, tabela, rowid, [motivo], [fisico]
    restaurar       database, tabela, rowid, [motivo]   desfaz a exclusao suave
    lixeira         database, tabela, [uuid], [limite], [com_anexos]
    motivos         database, tabela, [rowid], [limite]
    esvaziar_lixeira database, tabela, motivo          nao tem volta'''
assert velho in s
s=s.replace(velho,novo,1)
s=s.replace('''    varrer          database, tabela, [indice], [max]''',
            '''    varrer          database, tabela, [indice], [max], [visao]''',1)
io.open(p,'w',encoding='utf-8').write(s)
