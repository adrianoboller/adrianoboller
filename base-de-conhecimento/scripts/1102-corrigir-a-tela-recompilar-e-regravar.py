# Corrigir a tela, recompilar e regravar
# 29/08 10:33

import io
p='crates/phxsql-server/src/../ui/index.html'
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''       <p><strong>Isto não é SQL.</strong> O PhxSql não tem camada SQL: não há
       parser nem executor. O que existe é o <code>SelectMemory</code>, que
       consulta uma tabela já carregada na RAM.</p>'''
novo='''       <p><strong>Isto não é SQL.</strong> Esta tela é o <code>SelectMemory</code>,
       que consulta uma tabela já carregada na RAM sem tocar o disco. A camada
       SQL de verdade nasceu na 0.18.0: a operação <code>sql</code> do protocolo
       aceita um <code>SELECT</code> simples — pelo <code>phxsqlcmd</code>, pelo
       MCP ou por qualquer cliente da porta de dados.</p>'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
