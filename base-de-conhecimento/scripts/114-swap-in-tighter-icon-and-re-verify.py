# Swap in tighter icon and re-verify
# 27/08 20:10

import base64, re
p='crates/phxsql-server/ui/index.html'
s=open(p).read()
b64=lambda f: base64.b64encode(open(f,'rb').read()).decode()
ICON, FAVI = b64('marca/derivados/phxsql-icone-64.png'), b64('marca/derivados/phxsql-icone-32.png')

s2, n1 = re.subn(r'(<link rel="icon" type="image/png" href="data:image/png;base64,)[^"]*(">)',
                 lambda m: m.group(1)+FAVI+m.group(2), s)
s2, n2 = re.subn(r'(<span class="marca"><img src="data:image/png;base64,)[^"]*(" width=")64(" height=")38(")',
                 lambda m: m.group(1)+ICON+m.group(2)+'72'+m.group(3)+'62'+m.group(4), s2)
assert (n1, n2) == (1, 1), (n1, n2)
s2 = s2.replace('.barra .marca img{width:34px;height:auto;display:block}',
                '.barra .marca img{width:auto;height:30px;display:block}')
open(p,'w').write(s2)
print('trocado')
