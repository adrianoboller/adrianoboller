# Locate server label text
# 27/08 20:01

s=open('docs/dossie/dossie-phxsql.html').read()
velho='<text x="325" y="136" text-anchor="middle" font-size="10.5" opacity=".7">servidor &#183; config &#183; acesso</text>'
if velho not in s:
    import re
    m=re.search(r'<text x="325" y="136"[^>]*>([^<]*)</text>', s)
    print("achei:", repr(m.group(0)) if m else None)
else:
    print("casou")
