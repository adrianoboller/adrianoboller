# Probe with login
# 28/08 14:28

p='sisprobe.mjs'
s=open(p).read()
s=s.replace('const a = await api({op:"sistema"});','''console.log("login:", JSON.stringify(await api({op:"login", usuario:"adriano", senha:"senha123"})).slice(0,120));
const a = await api({op:"sistema"});''')
open(p,'w').write(s)
