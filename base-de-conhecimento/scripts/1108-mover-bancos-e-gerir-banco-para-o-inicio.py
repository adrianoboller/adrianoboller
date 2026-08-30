# Mover Bancos e Gerir Banco para o inicio
# 29/08 11:09

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

# Bancos e Gerir Banco saem do grupo 2 e abrem o grupo 1.
ini = s.index('const FERRAMENTAS = [')
fim = s.index('];', ini) + 2
bloco = s[ini:fim]

bancos = '  { ico:"banco",    rot:"Bancos",     cor:"var(--bin)",    faz:verBancos },\n'
gerir  = '  { ico:"gerir",    rot:"Gerir Banco",cor:"var(--bin)",    faz:() => gerirDatabase() },\n'
assert bloco.count(bancos)==1 and bloco.count(gerir)==1
novo = bloco.replace(bancos,'').replace(gerir,'')

alvo = '  { ico:"grade",    rot:"View DB",    cor:"var(--bin)",    faz:viewDatabaseAtual },'
assert novo.count(alvo)==1
novo = novo.replace(alvo, bancos + gerir + alvo)

# e o comentario da ordem passa a dizer o porque do comeco
velho_c = '     1. o dia a dia do dado ...... ver, consultar, cruzar, entrar e sair'
novo_c  = '''     1. o dia a dia do dado ...... abre pelos bancos, a pedido do Adriano
                                   (e o ponto de partida de toda sessao),
                                   depois ver, consultar, cruzar, entrar e sair'''
assert novo.count(velho_c)==1
novo = novo.replace(velho_c, novo_c)

s = s[:ini] + novo + s[fim:]
io.open(p,'w',encoding='utf-8').write(s)
print('Bancos e Gerir Banco na frente')
