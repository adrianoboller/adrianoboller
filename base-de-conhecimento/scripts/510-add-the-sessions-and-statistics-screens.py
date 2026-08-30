# Add the sessions and statistics screens
# 28/08 16:35

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/ui_estat.js').read()
marca='''/* ================================================= Junções e união'''
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)

# Conexoes passa a abrir a tela de sessoes de verdade
a='''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verConexoes },'''
b='''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verSessoes },'''
assert a in s; s=s.replace(a,b,1)
a='''    { rot:"Conexões",             ico:"⇋", faz:verConexoes },'''
b='''    { rot:"Sessões e conexões",   ico:"⇋", faz:verSessoes },
    { rot:"Estatísticas de uso…", ico:"◷", faz:() => verEstatisticas() },'''
assert a in s; s=s.replace(a,b,1)
a='''    { rot:"De onde vêm",   ico:"◎", faz:verIps },'''
b='''    { rot:"Sessões agora", ico:"⇋", faz:verSessoes },
    { rot:"Estatísticas de uso", ico:"◷", faz:() => verEstatisticas() },
    { rot:"De onde vêm",   ico:"◎", faz:verIps },'''
assert a in s; s=s.replace(a,b,1)

# CSS do botao mini
a='''/* ----------------------------------------------------------------- juncoes */'''
b='''.botao.mini{width:auto;padding:4px 10px;font-size:11px;font-weight:500;
            background:transparent;border:1px solid var(--linha-forte);
            color:var(--texto-3)}
.botao.mini:hover{border-color:var(--log);color:var(--log)}
td.num.mal{color:var(--log)}

/* ----------------------------------------------------------------- juncoes */'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
