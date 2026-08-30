# Apply official brand palette and typography to dossier
# 27/08 19:17

import base64, re
p="docs/dossie/dossie-phxsql.html"
s=open(p).read()

# ---------- 1. tipografia da marca: Exo 2 ----------
s=s.replace(
 'href="https://fonts.googleapis.com/css2?family=Archivo:wght@500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap"',
 'href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500;600&display=swap"')
s=s.replace('h1,h2,h3,h4,.desloc,.rotulo{font-family:Archivo,"Helvetica Neue",Arial,sans-serif}',
            'h1,h2,h3,h4,.desloc,.rotulo{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}')
s=s.replace('font-family:Archivo,sans-serif','font-family:"Exo 2",sans-serif')
s=s.replace('font-family:Archivo,"Helvetica Neue",Arial,sans-serif','font-family:"Exo 2","Helvetica Neue",Arial,sans-serif')

# ---------- 2. paleta oficial ----------
claro = '''  --papel:#fbf9f7;
  --papel-2:#f3efec;
  --papel-3:#e9e3de;
  --tinta:#1a1210;
  --tinta-2:#4a3f3a;
  --tinta-3:#7a6d66;
  --linha:#ded6d0;
  --acento:#c63c0a;
  --acento-2:#ff8a1c;
  --reg:#1f5c93;
  --ndx:#6a44a8;
  --bin:#0e7a85;
  --memo:#37702e;
  --log:#b71414;
  --ok:#2f7a3e;
  --pend:#8a6a1f;
  --sombra:0 1px 2px rgba(26,18,16,.06),0 8px 24px rgba(26,18,16,.05);
  --medida:68ch;'''

escuro = '''    --papel:#040814;
    --papel-2:#0a1122;
    --papel-3:#131c31;
    --tinta:#dde2eb;
    --tinta-2:#a8b0c0;
    --tinta-3:#7c8598;
    --linha:#1e2940;
    --acento:#ff8a1c;
    --acento-2:#ffc43d;
    --reg:#5fa6e8;
    --ndx:#b394f0;
    --bin:#3fc8d4;
    --memo:#7bcb6a;
    --log:#ff5f5f;
    --ok:#6cc98c;
    --pend:#ffc43d;
    --sombra:0 1px 2px rgba(0,0,0,.5),0 8px 28px rgba(0,0,0,.38);'''

s=re.sub(r'(:root\{\n).*?(\n\})', lambda m: m.group(1)+claro+m.group(2), s, count=1, flags=re.S)
s=re.sub(r'(@media \(prefers-color-scheme:dark\)\{\n  :root:not\(\[data-theme="light"\]\)\{\n).*?(\n  \}\n\})',
         lambda m: m.group(1)+escuro+m.group(2), s, count=1, flags=re.S)
s=re.sub(r'(:root\[data-theme="dark"\]\{\n).*?(\n\})',
         lambda m: m.group(1)+escuro.replace('\n    ','\n  ')+m.group(2), s, count=1, flags=re.S)

# ---------- 3. o simbolo na capa ----------
b64 = base64.b64encode(open("marca/derivados/phxsql-simbolo-420.png","rb").read()).decode()
s=s.replace('''<header class="capa">
  <div class="selo">Dossiê técnico · versão 0.2.0</div>''',
f'''<header class="capa">
  <img class="marca" width="420" height="277"
       src="data:image/png;base64,{b64}"
       alt="Símbolo do PhxSql: uma fênix de asas abertas envolvendo um cilindro de banco de dados, com trilhas de circuito saindo pelos lados">
  <div class="selo">Dossiê técnico · versão 0.2.0</div>''')

s=s.replace('''header.capa{padding:76px 0 8px;border-bottom:1px solid var(--linha)}''',
'''header.capa{padding:64px 0 8px;border-bottom:1px solid var(--linha)}
.marca{display:block;width:min(300px,62vw);height:auto;margin:0 0 26px -10px}''')

# ---------- 4. o X do logotipo, em cor de marca ----------
s=s.replace('<h1>PhxSql <span class="leve">— cinco arquivos,<br>uma tabela</span></h1>',
            '<h1>Ph<span class="x">x</span>Sql <span class="leve">— cinco arquivos,<br>uma tabela</span></h1>')
s=s.replace('''h1 .leve{color:var(--tinta-3);font-weight:500}''',
'''h1 .leve{color:var(--tinta-3);font-weight:500}
h1 .x{color:var(--acento)}
.assinatura{
  font-family:"Exo 2",sans-serif;font-size:12px;font-weight:500;
  letter-spacing:.24em;text-transform:uppercase;color:var(--tinta-3);
  margin:-8px 0 26px;
}''')
s=s.replace('''  <p class="chamada">Motor de dados em Rust''',
'''  <div class="assinatura">Built to store. Engineered to scale.</div>
  <p class="chamada">Motor de dados em Rust''')
open(p,'w').write(s)
print("dossie rebrandado")
