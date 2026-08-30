# Verify the figure sequence
# 28/08 19:13

import io,re
s=io.open('docs/dossie/dossie-phxsql.html',encoding='utf-8').read()
nums=[int(m) for m in re.findall(r'<b>Figura (\d+)\.</b>', s)]
print('sequencia:', nums)
print('em ordem e sem falha?', nums == list(range(1, len(nums)+1)))
# e as referencias no texto
print('referencias "Figura N" no corpo:', re.findall(r'[Ff]igura (\d+)(?!\.</b>)', s)[:10])
