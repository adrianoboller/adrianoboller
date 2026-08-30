# Deduplicate TETO and measure the true count
# 30/08 06:43

p='crates/phxsql-server/src/conferidor.rs'
s=open(p,encoding='utf-8').read()
velho='pub const TETO: usize = 1_999;'
assert s.count(velho)==1 and s.count('pub const TETO: usize = 1_996;')==1
# O comentario de historia dos dois lados fica; a constante e uma so, e o valor
# sai da medicao depois do merge -- nao do que cada frente mediu sozinha.
s=s.replace(velho+'\n','',1)
open(p,'w',encoding='utf-8').write(s)
print("uma constante so; o comentario de historia das duas frentes ficou")
