# Check the roadmap section for stale numbers
# 28/08 15:14

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
s=s.replace('''  e o que ainda falta em <code>docs/PENDENCIAS.md</code>.</p>''',
            '''  o DbLink em <code>docs/DBLINK.md</code>,
  e o que ainda falta em <code>docs/PENDENCIAS.md</code>.</p>''',1)
open(p,'w').write(s)
