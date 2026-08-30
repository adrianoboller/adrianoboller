# Add Debug and re-test
# 28/08 15:26

p='crates/phxsql-server/src/juncao.rs'
s=open(p).read()
for alvo in ['pub struct ColunaSaida {', 'pub struct Resultado {', 'pub struct ResultadoUniao {']:
    s=s.replace(alvo, '#[derive(Debug)]\n'+alvo, 1)
open(p,'w').write(s)
