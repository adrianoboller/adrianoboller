# Fix the form layout
# 28/08 14:57

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
s=s.replace('`<div class="cartao form-dbl">','`<div class="form-dbl">',1)
a='''    `<div class="cartao">
       <textarea id="sqlDbl" class="campo" rows="5"'''
b='''    `<div class="form-sql">
       <textarea id="sqlDbl" class="campo" rows="5"'''
assert a in s; s=s.replace(a,b,1)
a='''.form-dbl .cmp{display:flex;flex-direction:column;gap:4px;font-size:11px;
               color:var(--texto-3)}
.form-dbl .cmp > span:first-child{letter-spacing:.04em}'''
b='''/* O `label` global e maiusculo e espacado -- bom para o rotulo, ruim para a
   dica embaixo do campo, que virava um grito. */
.form-dbl .cmp{display:flex;flex-direction:column;gap:4px;margin:0;
               text-transform:none;letter-spacing:0;color:var(--texto-3)}
.form-dbl .cmp > span:first-child{font-size:10.5px;letter-spacing:.1em;
                                  text-transform:uppercase}
.form-dbl .cmp .leg{font-size:10.5px;line-height:1.4;color:var(--texto-3);
                    text-transform:none;letter-spacing:0}'''
assert a in s; s=s.replace(a,b,1)
a='''#sqlDbl{'''
b='''.form-sql{margin-bottom:16px}
#sqlDbl{'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
