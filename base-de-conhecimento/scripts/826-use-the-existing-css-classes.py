# Use the existing CSS classes
# 28/08 20:44

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()

# usar o `painel` que ja existe, com a linha de unidade que ele nao tinha
antigo = """  <div class="kpis">
    <div class="kpi"><div class="v">18.773</div><div class="r">linhas/s no master</div><div class="u">com a imagem no diário</div></div>
    <div class="kpi"><div class="v">4.273</div><div class="r">eventos/s por réplica</div><div class="u">as três em paralelo</div></div>
    <div class="kpi"><div class="v">1,3–2,1 s</div><div class="r">até as três</div><div class="u">laço de 2 s</div></div>
    <div class="kpi"><div class="v">1,0 s</div><div class="r">retomada</div><div class="u">4.000 eventos após queda</div></div>
  </div>"""
novo = """  <div class="painel">
    <div><div class="v">18.773</div><div class="r">linhas/s no master</div></div>
    <div><div class="v">4.273</div><div class="r">eventos/s por réplica</div></div>
    <div><div class="v">1,3–2,1 s</div><div class="r">até as três réplicas</div></div>
    <div><div class="v">1,0 s</div><div class="r">retomada de 4.000</div></div>
    <div><div class="v">iguais</div><div class="r">retrato das quatro</div></div>
  </div>"""
assert antigo in s
s = s.replace(antigo, novo)

s = s.replace('<div class="nota" data-tom="aviso">', '<div class="nota">')
s = s.replace('''  <p class="fonte">Como refazer: <code>python3 bancada/replicacao/montar.py
  /tmp/phx-replicacao</code> e <code>python3 bancada/replicacao/medir.py
  100000</code>. A bancada não compara «quantas linhas»: compara um SHA-256 de
  <strong>cada linha inteira</strong>, com o <code>rowid</code> e o
  <code>rownum</code> juntos — contar não acharia uma linha que atravessou
  errada.</p>''',
'''  <div class="nota">
    <span class="t">Como refazer, e o que a bancada compara</span>
    <p><code>python3 bancada/replicacao/montar.py /tmp/phx-replicacao</code> sobe os
    quatro; <code>python3 bancada/replicacao/medir.py 100000</code> mede.
    <code>montar.py --cascata</code> põe o Slave03 puxando do Slave01 — o segundo
    salto custou 1.827 ms contra 1.679 ms do primeiro.</p>
    <p>Ela <strong>não compara «quantas linhas»</strong>: compara um SHA-256 de
    <em>cada linha inteira</em>, com o <code>rowid</code> e o <code>rownum</code>
    juntos, lido pelo cursor. Contar não acharia uma linha que atravessou errada — e
    o <code>rowid</code> entrar na conta é o ponto, porque ele não é transmitido.</p>
  </div>''')
p.write_text(s)
print("ok")
