# Move calendar partitioning into section 5
# 28/08 13:10

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
bloco = pathlib.Path('/tmp/bloco_volume.html').read_text().rstrip() + '\n'

v = '''  <p>Passar da capacidade devolve erro explícito de <em>tabela cheia</em>, em vez do
  estouro silencioso de 2 GB que o TopSpeed(R) dava. E um bloco maior que o volume fica
  sozinho no seu volume, em vez de ser recusado — senão uma foto de 2 MB seria impossível
  de gravar num volume de 1 MB.</p>
</section>'''
n = '''  <p>Passar da capacidade devolve erro explícito de <em>tabela cheia</em>, em vez do
  estouro silencioso de 2 GB que o TopSpeed(R) dava. E um bloco maior que o volume fica
  sozinho no seu volume, em vez de ser recusado — senão uma foto de 2 MB seria impossível
  de gravar num volume de 1 MB.</p>

  <p>E <strong>«sem teto» não existe</strong>: o sufixo tem largura fixa, então com três
  dígitos o volume 1000 não teria nome de arquivo. Teto omitido vira o maior que cabe —
  999 com três dígitos.</p>

''' + bloco + '''</section>'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('particao por calendario foi para a secao 5')
