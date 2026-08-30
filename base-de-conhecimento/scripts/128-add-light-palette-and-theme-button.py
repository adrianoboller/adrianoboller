# Add light palette and theme button
# 27/08 20:31

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

velho = ''':root{
  --fundo:#010418; --painel:#0a1122; --painel-2:#0f182c; --realce:#152238;
  --linha:#1e2940; --linha-forte:#2b3a56;
  --texto:#dde2eb; --texto-2:#a8b0c0; --texto-3:#7c8598;
  --laranja:#ff8a1c; --ambar:#ffc43d; --vermelhao:#ff4d10; --vermelho:#ff5f5f;
  --reg:#5fa6e8; --ndx:#b394f0; --bin:#3fc8d4; --memo:#7bcb6a; --log:#ff5f5f;
  --ok:#6cc98c; --aviso:#ffc43d;
  --barra:52px; --arvore:268px;
}'''
novo = ''':root{
  --fundo:#010418; --painel:#0a1122; --painel-2:#0f182c; --realce:#152238;
  --linha:#1e2940; --linha-forte:#2b3a56;
  --texto:#dde2eb; --texto-2:#a8b0c0; --texto-3:#7c8598;
  --laranja:#ff8a1c; --ambar:#ffc43d; --vermelhao:#ff4d10; --vermelho:#ff5f5f;
  --reg:#5fa6e8; --ndx:#b394f0; --bin:#3fc8d4; --memo:#7bcb6a; --log:#ff5f5f;
  --ok:#6cc98c; --aviso:#ffc43d;
  --barra:52px; --arvore:268px;
  --sombra-fenix:0 6px 22px rgba(255,77,16,.28);
}

/* Tema claro. Nao e o escuro invertido: sobre papel, o vermelhao #ff4d10 da
   3,5:1 de contraste -- abaixo do minimo para texto. Escurece para #c63c0a,
   que mantem a cor da marca e passa dos 4,5:1. E a mesma adaptacao que o
   dossie ja fazia; agora as duas paginas concordam.

   As cinco cores de arquivo tambem escurecem: elas precisam se distinguir
   entre si E do fundo, e as do tema escuro somem no branco. */
:root[data-tema="claro"]{
  --fundo:#f7f5f2; --painel:#ffffff; --painel-2:#f2efeb; --realce:#e9e4de;
  --linha:#ded7cf; --linha-forte:#c4bab0;
  --texto:#1a1210; --texto-2:#4a3f3a; --texto-3:#7a6d66;
  --laranja:#c63c0a; --ambar:#a06a00; --vermelhao:#c63c0a; --vermelho:#b71414;
  --reg:#1f5c93; --ndx:#6a44a8; --bin:#0e7a85; --memo:#37702e; --log:#b71414;
  --ok:#2f7a3e; --aviso:#8a6a1f;
  --sombra-fenix:0 6px 18px rgba(198,60,10,.18);
}'''
assert s.count(velho)==1
s = s.replace(velho, novo)

# a sombra da fenix passa a vir do token
s = s.replace('filter:drop-shadow(0 6px 22px rgba(255,77,16,.28));',
              'filter:drop-shadow(var(--sombra-fenix));')

# transicao suave + botao do tema
s = s.replace('''.barra .espaco{flex:1}''','''.barra .espaco{flex:1}
.tema{
  background:none;border:1px solid var(--linha-forte);border-radius:99px;
  width:34px;height:26px;display:grid;place-items:center;font-size:13px;
  line-height:1;padding:0;color:var(--texto-2);
}
.tema:hover{border-color:var(--laranja);color:var(--laranja)}
body,.cartao,.barra,.arvore,table,td,th{
  transition:background-color .18s ease,color .18s ease,border-color .18s ease;
}
@media (prefers-reduced-motion:reduce){*{transition:none!important}}''')

s = s.replace('''    <span class="demo" id="selo" hidden>modo demonstração</span>''',
'''    <span class="demo" id="selo" hidden>modo demonstração</span>
    <button class="tema" id="btTema" title="Alternar tema" aria-label="Alternar tema claro e escuro">🌓</button>''')
open(p,'w').write(s)
print('paleta clara ok')
