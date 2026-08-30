# Embed brand marks into the interface
# 27/08 20:08

import base64
p='crates/phxsql-server/ui/index.html'
s=open(p).read()

b64 = lambda f: base64.b64encode(open(f,'rb').read()).decode()
SIMB = b64('marca/derivados/phxsql-simbolo-224.png')
ICON = b64('marca/derivados/phxsql-icone-64.png')
FAVI = b64('marca/derivados/phxsql-icone-32.png')

# 1. favicon, logo depois do title
s = s.replace('<meta name="viewport" content="width=device-width, initial-scale=1">',
    '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
    f'<link rel="icon" type="image/png" href="data:image/png;base64,{FAVI}">')

# 2. fundo oficial da marca -- #010418 e o que os originais medem
s = s.replace('--fundo:#040814;', '--fundo:#010418;')

# 3. CSS da marca no cartao e na barra
s = s.replace('''.cartao h1{
  font-size:30px;font-weight:700;letter-spacing:-.02em;margin:0 0 4px;
}''','''/* A fenix vem do logo oficial, com o fundo #010418 retirado para o alfa --
   assim ela assenta em cima do painel sem deixar emenda de retangulo. */
.cartao .simbolo{
  display:block;width:172px;height:auto;margin:0 auto 18px;
  filter:drop-shadow(0 6px 22px rgba(255,77,16,.28));
}
.cartao h1{
  font-size:30px;font-weight:700;letter-spacing:-.02em;margin:0 0 4px;
  text-align:center;
}''')
s = s.replace('''.cartao .assin{
  font-size:10.5px;letter-spacing:.22em;text-transform:uppercase;
  color:var(--texto-3);margin:0 0 26px;
}''','''.cartao .assin{
  font-size:10.5px;letter-spacing:.22em;text-transform:uppercase;
  color:var(--texto-3);margin:0 0 8px;text-align:center;
}
.cartao .lema{
  font-size:9px;letter-spacing:.19em;text-transform:uppercase;
  color:var(--texto-3);opacity:.62;margin:0 0 26px;text-align:center;
}''')
s = s.replace('''.barra .marca{font-size:17px;font-weight:700;letter-spacing:-.01em}''',
'''.barra .marca{display:flex;align-items:center;gap:9px;font-size:17px;font-weight:700;letter-spacing:-.01em}
.barra .marca img{width:34px;height:auto;display:block}''')

# 4. marcacao: simbolo no cartao
s = s.replace('''  <div class="cartao">
    <h1>Ph<span class="x">x</span>Sql</h1>
    <p class="assin">Centro de Controle</p>''',
f'''  <div class="cartao">
    <img class="simbolo" src="data:image/png;base64,{SIMB}"
         width="224" height="133" alt="PhxSql">
    <h1>Ph<span class="x">x</span>Sql</h1>
    <p class="assin">Centro de Controle</p>
    <p class="lema">Built to store. Engineered to scale.</p>''')

# 5. marcacao: icone na barra
s = s.replace('''    <span class="marca">Ph<span class="x">x</span>Sql</span>''',
f'''    <span class="marca"><img src="data:image/png;base64,{ICON}" width="64" height="38"
      alt=""><span>Ph<span class="x">x</span>Sql</span></span>''')

open(p,'w').write(s)
