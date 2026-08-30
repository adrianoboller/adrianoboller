# Recheck table alignment correctly
# 28/08 18:14

import io,re
s=io.open('docs/dossie/dossie-phxsql.html',encoding='utf-8').read()
i=s.index('<th class="num">.trash</th>')
ini=s.rindex('<thead>',0,i)
fim=s.index('</tbody>',i)
bloco=s[ini:fim]
cab=len(re.findall(r'<th[ >]', bloco.split('</thead>')[0]))
corpo=bloco.split('<tbody>')[1]
print('cabecalho:', cab)
ruins=0
for tr in re.findall(r'<tr>(.*?)</tr>', corpo, re.S):
    n=len(re.findall(r'<td[ >]', tr))
    nome=re.search(r'class="dado">([^<]*)', tr)
    marca = 'ok' if n==cab else 'DESALINHADA'
    if n!=cab: ruins+=1
    print(f'  {marca:<12} {n:>2}  {nome.group(1) if nome else "?"}')
print('desalinhadas:', ruins)
