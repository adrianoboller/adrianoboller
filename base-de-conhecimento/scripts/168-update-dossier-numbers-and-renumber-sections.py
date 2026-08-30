# Update dossier numbers and renumber sections
# 27/08 21:00

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()

# --- numeros da capa e do rodape, remedidos
s=s.replace('<div><div class="v">14.171</div><div class="r">linhas de Rust</div></div>',
            '<div><div class="v">17.783</div><div class="r">linhas de Rust</div></div>')
s=s.replace('<div><div class="v">214</div><div class="r">testes</div></div>',
            '<div><div class="v">254</div><div class="r">testes</div></div>')
s=s.replace('<div><div class="v">2.418</div><div class="r">linhas de doc</div></div>',
            '<div><div class="v">2.901</div><div class="r">linhas de doc</div></div>')
s=s.replace('<div class="selo">Dossiê técnico · versão 0.1.0</div>',
            '<div class="selo">Dossiê técnico · versão 0.2.0</div>')
s=s.replace('''<p>PhxSql 0.1.0 · 14.171 linhas de Rust em 4 crates, mais 62 KB de interface ·
  214 testes · nenhuma dependência externa.''',
'''<p>PhxSql 0.2.0 · 17.783 linhas de Rust em 4 crates, mais 69 KB de interface ·
  254 testes · nenhuma dependência externa.''')

# --- indice: tres secoes novas
s=s.replace('''    <li><a href="#s11"><span class="n">11</span> Centro de Controle</a></li>
    <li><a href="#s12"><span class="n">12</span> Decisões</a></li>
    <li><a href="#s13"><span class="n">13</span> Estado e roteiro</a></li>''',
'''    <li><a href="#s11"><span class="n">11</span> Centro de Controle</a></li>
    <li><a href="#s12"><span class="n">12</span> Tabela em memória</a></li>
    <li><a href="#s13"><span class="n">13</span> Chave assimétrica</a></li>
    <li><a href="#s14"><span class="n">14</span> Backup</a></li>
    <li><a href="#s15"><span class="n">15</span> Decisões</a></li>
    <li><a href="#s16"><span class="n">16</span> Estado e roteiro</a></li>''')

# --- renumera 13 -> 16 e 12 -> 15
s=s.replace('''<!-- ============================= 13 ============================= -->
<section id="s13">
  <div class="rotulo"><span class="num">13</span><span class="traco"></span></div>
  <h2>Estado e roteiro</h2>''','''<!-- ============================= 16 ============================= -->
<section id="s16">
  <div class="rotulo"><span class="num">16</span><span class="traco"></span></div>
  <h2>Estado e roteiro</h2>''')
s=s.replace('''<!-- ============================= 12 ============================= -->
<section id="s12">
  <div class="rotulo"><span class="num">12</span><span class="traco"></span></div>
  <h2>Decisões tomadas</h2>''','''<!-- ============================= 15 ============================= -->
<section id="s15">
  <div class="rotulo"><span class="num">15</span><span class="traco"></span></div>
  <h2>Decisões tomadas</h2>''')
open(p,'w').write(s)
print('numeros e indice ok')
