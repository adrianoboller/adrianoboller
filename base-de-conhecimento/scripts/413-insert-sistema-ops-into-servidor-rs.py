# Insert sistema ops into servidor.rs
# 28/08 14:24

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
marca='''    // -------------------------------------------------------------- o painel
'''
assert marca in s
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/sistema_op.rs').read()
s=s.replace(marca, novo+"\n"+marca,1)
open(p,'w').write(s)
print('ok')
