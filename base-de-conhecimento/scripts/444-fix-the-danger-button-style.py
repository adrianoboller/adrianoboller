# Fix the danger-button style
# 28/08 14:58

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''.botao.perigo{border-color:var(--log);color:var(--log)}'''
b='''/* Sem apagar o fundo, o `.botao` continuava laranja e o texto vermelho ficava
   ilegivel em cima dele -- e como o botao de excluir aparecia. A borda so
   significa alguma coisa sobre fundo transparente. */
.botao.perigo{background:transparent;border:1px solid var(--log);color:var(--log);
              font-weight:500}'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
