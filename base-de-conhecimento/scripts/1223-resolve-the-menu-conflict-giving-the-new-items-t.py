# Resolve the menu conflict giving the new items translation keys
# 29/08 23:40

import io
M=">>>>>>> worktree-agent-aef6a888055ca3618"
p="phxsql/crates/phxsql-server/ui/index.html"
s=io.open(p,encoding="utf-8").read()

# 1) aditivo: as cores e o repintar do idioma entram os dois
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]; deles=s[b+len("\n=======\n"):c]
s=s[:a]+meu+deles+s[cfim:]

# 2) o menu Ver: a assinatura e as chaves DELES, e os itens do multitela do
#    HEAD -- com chave de traducao propria, porque a regra agora e petrea
a=s.index("<<<<<<< HEAD"); b=s.index("\n=======\n",a); c=s.index(M,b); cfim=s.index("\n",c)+1
meu=s[a+len("<<<<<<< HEAD\n"):b+1]
CH={"Nova aba nesta região":"tela.mi_nova_aba","Fechar esta aba":"tela.mi_fechar_aba",
    "Uma região":"tela.mi_uma_regiao","Duas regiões":"tela.mi_duas_regioes",
    "Três regiões":"tela.mi_tres_regioes","Quatro regiões":"tela.mi_quatro_regioes",
    "Soltar esta tela numa janela":"tela.mi_soltar",
    "Alinhar com as bordas dos monitores":"tela.mi_alinhar",
    "Sobre o modo multitela…":"tela.mi_sobre_multitela"}
itens=meu
for pt,ch in CH.items():
    velho=f'rot:"{pt}",'
    assert itens.count(velho)==1, pt
    itens=itens.replace(velho, f'rot:"{pt}", txt:"{ch}",')
# a cabeca do menu vem DELES (tem a chave do titulo); o corpo, do HEAD
cabeca='  ["Ver", "V", "tela.menu_ver", [\n'
corpo = itens.split("\n",1)[1]           # tira a linha `["Ver", "V", [`
novo = cabeca + corpo
# as duas primeiras entradas ganham as chaves que eles ja tinham
novo = novo.replace('{ rot:"Atualizar", ico:"↻", tecla:"F5", faz:atualizarVista },',
                    '{ rot:"Atualizar", txt:"tela.mi_atualizar", ico:"↻", tecla:"F5", faz:atualizarVista },')
novo = novo.replace('{ rot:"Tema claro / escuro", ico:"🌓", faz:() => $("#btTema").click() },',
                    '{ rot:"Tema claro / escuro", txt:"tela.mi_tema", ico:"🌓", faz:() => $("#btTema").click() },')
s=s[:a]+novo+s[cfim:]
assert "<<<<<<<" not in s and M not in s
io.open(p,"w",encoding="utf-8").write(s)
print("index.html resolvido; os nove itens do multitela ganharam chave")
