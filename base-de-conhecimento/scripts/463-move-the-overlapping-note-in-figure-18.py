# Move the overlapping note in Figure 18
# 28/08 15:18

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
a='''          <rect x="60" y="52" width="300" height="22" rx="3" fill="none" stroke="var(--pend)" stroke-width="1.2"/>
          <text x="70" y="67" font-size="10.5" fill="var(--pend)">1ª leitura: só um ponto — não há taxa, e ela diz isso</text>'''
b='''          <rect x="60" y="230" width="316" height="22" rx="3" fill="none" stroke="var(--pend)" stroke-width="1.2"/>
          <text x="70" y="245" font-size="10.5" fill="var(--pend)">1ª leitura: só um ponto — não há taxa, e ela diz isso</text>'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('<svg viewBox="0 0 840 250" role="img" aria-label="A leitura de CPU',
            '<svg viewBox="0 0 840 262" role="img" aria-label="A leitura de CPU',1)
open(p,'w').write(s)
print('ok')
