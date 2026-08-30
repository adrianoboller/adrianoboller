# Render dossier and capture new figure
# 27/08 20:01

s=open('docs/dossie/dossie-phxsql.html').read()
s=s.replace('<text x="325" y="136" text-anchor="middle" font-size="10.5" opacity=".7">servidor · config · acesso</text>',
            '<text x="325" y="136" text-anchor="middle" font-size="10.5" opacity=".7">servidor · config · acesso · http</text>')
open('docs/dossie/dossie-phxsql.html','w').write(s)
