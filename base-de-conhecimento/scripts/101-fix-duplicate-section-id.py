# Fix duplicate section id
# 27/08 19:59

s=open('docs/dossie/dossie-phxsql.html').read()
velho = '''<!-- ============================= 10 ============================= -->
<section id="s12">
  <div class="rotulo"><span class="num">12</span><span class="traco"></span></div>
  <h2>Estado e roteiro</h2>'''
novo = '''<!-- ============================= 13 ============================= -->
<section id="s13">
  <div class="rotulo"><span class="num">13</span><span class="traco"></span></div>
  <h2>Estado e roteiro</h2>'''
assert s.count(velho)==1
open('docs/dossie/dossie-phxsql.html','w').write(s.replace(velho,novo))
