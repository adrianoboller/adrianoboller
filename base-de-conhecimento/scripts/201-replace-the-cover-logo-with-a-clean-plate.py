# Replace the cover logo with a clean plate
# 27/08 21:28

import base64, pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

b64 = base64.b64encode(pathlib.Path('marca/derivados/phxsql-simbolo-440.png').read_bytes()).decode()

# 1. CSS: a marca passa a morar numa placa com o fundo oficial dela.
velho_css = '.marca{display:block;width:min(300px,62vw);height:auto;margin:0 0 26px -10px}'
novo_css = '''/* A marca vive sobre #010418 -- e o fundo oficial, medido dos originais.
   Sobre papel claro ela precisa levar esse fundo junto, senao o cilindro,
   que e escuro por dentro, vira um fantasma branco. A placa resolve isso
   sendo deliberada: cantos arredondados e um brilho, para ler como
   apresentacao da marca e nao como retangulo perdido. */
.placa{
  display:inline-block;background:#010418;border-radius:12px;
  padding:14px 22px 10px;margin:0 0 26px;
  box-shadow:0 10px 30px rgba(198,60,10,.14), 0 1px 0 rgba(255,255,255,.05) inset;
}
.marca{display:block;width:min(300px,62vw);height:auto}'''
assert s.count(velho_css) == 1
s = s.replace(velho_css, novo_css)

# 2. Marcacao: envolve a imagem e troca o recorte cortado pelo novo.
import re
m = re.search(r'  <img class="marca" width="420" height="277"\n       src="data:image/png;base64,[^"]*"\n       alt="[^"]*">', s)
assert m, 'nao achei a imagem da capa'
novo_img = ('  <div class="placa">\n'
            '    <img class="marca" width="440" height="262"\n'
            f'         src="data:image/png;base64,{b64}"\n'
            '         alt="Símbolo do PhxSql: uma fênix de asas abertas envolvendo um cilindro de banco de dados, com trilhas de circuito saindo pelos lados">\n'
            '  </div>')
s = s[:m.start()] + novo_img + s[m.end():]
p.write_text(s)
print('capa trocada')
