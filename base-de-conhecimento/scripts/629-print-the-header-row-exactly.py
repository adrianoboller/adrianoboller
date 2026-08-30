# Print the header row exactly
# 28/08 18:14

import io,re
s=io.open('docs/dossie/dossie-phxsql.html',encoding='utf-8').read()
i=s.index('<th class="num">.trash</th>')
ini=s.rindex('<thead>',0,i)
print(repr(s[ini:s.index('</thead>',i)+8]))
