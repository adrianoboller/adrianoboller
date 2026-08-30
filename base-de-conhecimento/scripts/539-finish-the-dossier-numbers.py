# Finish the dossier numbers
# 28/08 17:09

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
import re
s2=re.sub(r'PhxSql 0\.11\.0 · [\d.]+ linhas de Rust em 4 crates, mais \d+ KiB de interface ·\s*\n\s*[\d.]+ testes',
          'PhxSql 0.11.0 · 34.156 linhas de Rust em 4 crates, mais 422 KiB de interface ·\n  453 testes', s, count=1)
assert s2!=s, "rodape nao casou"
s=s2
for a,b in [
 ('<p><strong>52 das 55 operações do protocolo têm tela.</strong>','<p><strong>53 das 56 operações do protocolo têm tela.</strong>'),
 ('View Database · grade de tabelas e ficha de edição · 52 das 55 ops na tela','View Database · grade de tabelas e ficha de edição · 53 das 56 ops na tela'),
]:
    assert a in s, a[:50]
    s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
