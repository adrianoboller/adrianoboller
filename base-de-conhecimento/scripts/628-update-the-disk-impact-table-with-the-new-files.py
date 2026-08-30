# Update the disk-impact table with the new files
# 28/08 18:14

# confere que toda linha da tabela tem o mesmo numero de celulas do cabecalho
import io,re
s=io.open('docs/dossie/dossie-phxsql.html',encoding='utf-8').read()
i=s.index('<th class="num">.trash</th>')
bloco=s[s.rindex('<table>',0,i):s.index('</table>',i)]
cab=len(re.findall(r'<th', bloco.split('</thead>')[0]))
print('cabecalho:', cab, 'celulas')
for tr in re.findall(r'<tr>(.*?)</tr>', bloco.split('<tbody>')[1], re.S):
    n=len(re.findall(r'<td', tr))
    nome=re.search(r'class="dado">([^<]*)', tr)
    if n!=cab: print('  DESALINHADA:', nome.group(1) if nome else '?', n)
print('conferido')
