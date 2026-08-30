# Fix the comma expression and rebuild
# 28/08 14:53

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''       <div class="acoes"><button class="botao" id="btDef">Cadastrar uma ligação…</button></div>`),
      $("#btDef").onclick = () => telaDbLinkDefinicoes();
  }'''
b='''       <div class="acoes"><button class="botao" id="btDef">Cadastrar uma ligação…</button></div>`);
    $("#btDef").onclick = () => telaDbLinkDefinicoes();
    return;
  }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
