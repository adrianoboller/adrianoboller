# Update the title and cover intro
# 28/08 20:47

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
s = s.replace("<title>Dossiê PhxSql</title>", "<title>Dossiê PhxSql 0.15</title>", 1)
antigo = """  <p class="chamada">Motor de dados em Rust no modelo de arquivos separados do HFSQL(R), com
  a garantia que o TopSpeed(R) não dava: <strong>o arquivo de dados guarda a ordem em que
  você digitou</strong>, e nada nunca a embaralha.</p>"""
novo = """  <p class="chamada">Motor de dados em Rust no modelo de arquivos separados do HFSQL(R), com
  a garantia que o TopSpeed(R) não dava: <strong>o arquivo de dados guarda a ordem em que
  você digitou</strong>, e nada nunca a embaralha.</p>

  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.15.0</strong>,
  refeita contra o código. Três coisas que a versão anterior listava como o que
  faltava saíram, e as três estão medidas: a <a href="#s9">replicação com quatro
  servidores</a>, o <a href="#s5b">salto para «a página 500»</a> e a
  <a href="#s7">carga em lote</a>.</p>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
