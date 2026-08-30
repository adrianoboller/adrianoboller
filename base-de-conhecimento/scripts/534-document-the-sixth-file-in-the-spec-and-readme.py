# Document the sixth file in the spec and readme
# 28/08 17:05

p='README.md'
s=open(p).read()
a='''lógica é a soma de cinco arquivos físicos.'''
b='''lógica é a soma de cinco arquivos físicos — mais um sexto, o espelho `.bkp`,
quando ele está ligado.'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('README ok')
