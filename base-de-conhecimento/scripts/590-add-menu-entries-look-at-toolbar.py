# Add menu entries; look at toolbar
# 28/08 17:46

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''    { rot:"Diário",               ico:"◷", quando:comTabela, faz:() => irAba("diario") },
    { rot:"Integridade",          ico:"⚑", quando:comTabela, faz:() => irAba("integridade") },
    "sep",'''
novo='''    { rot:"Diário",               ico:"◷", quando:comTabela, faz:() => irAba("diario") },
    { rot:"Integridade",          ico:"⚑", quando:comTabela, faz:() => irAba("integridade") },
    "sep",
    // Os tres arquivos que so o administrador le ficam juntos, e o menu diz
    // isso no rotulo: descobrir a restricao so depois de clicar e pior.
    { rot:"Lixeira da tabela",    ico:"♲", quando:comTabela, faz:() => telaLixeira() },
    { rot:"Motivos das exclusões", ico:"✎", quando:comTabela, faz:() => telaMotivos() },
    "sep",'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
