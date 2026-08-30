# Add trash/reasons to management, inspect menu
# 28/08 17:46

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''    ["excluir", "🗑", "Excluir tabela",
     "Apaga o .reg, o .ndx, o .bin, o .memo e o .log de uma vez. Não há desfazer.",
     () => excluirTabelaDe(db, tab)],'''
novo='''    ["lixeira", "♲", "Lixeira da tabela",
     "As linhas que saíram do .reg, inteiras e com os anexos. Só administrador.",
     () => telaLixeira(db, tab)],
    ["motivos", "✎", "Motivos das exclusões",
     "Por que cada linha foi excluída, por quem e quando. Só administrador.",
     () => telaMotivos(db, tab)],
    ["excluir", "🗑", "Excluir tabela",
     "Apaga os sete arquivos da tabela de uma vez. Não há desfazer.",
     () => excluirTabelaDe(db, tab)],'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
