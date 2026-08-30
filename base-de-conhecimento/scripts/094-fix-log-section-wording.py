# Fix log section wording
# 27/08 19:55

s=open('MANUAL.txt').read()
s = s.replace('''Toda conexao a porta 5000 e registrada, INCLUSIVE as recusadas por IP ou por
token errado - e justamente quem tentou e nao conseguiu que interessa.''',
'''Todo pedido e registrado - pela porta 5000 e pelo Centro de Controle, sem
distincao -, INCLUSIVE os recusados por IP ou por token errado: e justamente
quem tentou e nao conseguiu que interessa.''')
s = s.replace('''
10. LOG DE ACESSOS''','''
10. LOG DE ACESSOS''')
open('MANUAL.txt','w').write(s)
