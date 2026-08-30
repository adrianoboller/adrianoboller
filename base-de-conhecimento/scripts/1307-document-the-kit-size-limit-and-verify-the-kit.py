# Document the kit size limit and verify the kit
# 30/08 16:38

p='empacotar.sh'
s=open(p,encoding='utf-8').read()
alvo='''# O kit: tudo junto, para quem so quer baixar uma coisa'''
novo='''# O kit: tudo junto, para quem so quer baixar uma coisa
#
# Ele passa de 30 MB, e isso e consequencia de ser a soma de todos os outros --
# nao ha o que apertar sem tirar peca. Canais com limite de anexo (o envio
# desta sessao, correio) nao levam o kit: levam as pecas, que juntas SAO o kit.
# Fica dito aqui para ninguem "consertar" o kit tirando os binarios ARM.'''
assert s.count(alvo)==1
open(p,'w',encoding='utf-8').write(s.replace(alvo,novo))
print("limite do kit documentado no proprio script")
