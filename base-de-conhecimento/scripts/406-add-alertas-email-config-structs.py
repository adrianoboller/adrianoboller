# Add Alertas/Email config structs
# 28/08 14:21

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
marca='''/// Interface web: um servidor HTTP separado, que serve a pagina do Centro de
/// Controle e traduz o clique do navegador no mesmo protocolo da porta 5000.'''
assert marca in s
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/alertas.rs').read()
s=s.replace(marca, novo.strip()+"\n\n"+marca,1)
open(p,'w').write(s)
print('ok')
