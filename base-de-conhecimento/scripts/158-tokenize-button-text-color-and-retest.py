# Tokenize button text color and retest
# 27/08 20:54

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
# No escuro o botao e laranja vivo e pede tinta escura; no claro ele escurece
# para #c63c0a e a mesma tinta escura some. Vira token.
s=s.replace('  --brilho-entrada:#0d1830;','  --brilho-entrada:#0d1830; --tinta-botao:#160a00;')
s=s.replace('  --brilho-entrada:#efe7dd;','  --brilho-entrada:#efe7dd; --tinta-botao:#fff6f0;')
s=s.replace('background:var(--laranja);color:#160a00;','background:var(--laranja);color:var(--tinta-botao);')
open(p,'w').write(s)
