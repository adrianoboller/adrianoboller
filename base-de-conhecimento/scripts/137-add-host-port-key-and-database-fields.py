# Add host, port, key and database fields
# 27/08 20:35

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

velho = '''    <div class="campo">
      <label for="u">Usuário</label>
      <input id="u" autocomplete="username" value="adriano">
    </div>'''
novo = '''    <div class="onde">
      <div class="campo">
        <label for="h">Servidor</label>
        <input id="h" value="localhost" placeholder="IP ou DNS" autocomplete="off">
      </div>
      <div class="campo estreito">
        <label for="pt">Porta</label>
        <input id="pt" value="5000" inputmode="numeric" autocomplete="off">
      </div>
    </div>
    <div class="campo">
      <label for="u">Usuário</label>
      <input id="u" autocomplete="username" value="adriano">
    </div>'''
assert s.count(velho)==1
s=s.replace(velho,novo)

velho2='''    <div class="campo">
      <label for="t">Token do servidor</label>
      <input id="t" type="password" autocomplete="off">
    </div>'''
novo2='''    <div class="campo">
      <label for="t">Token do servidor</label>
      <input id="t" type="password" autocomplete="off">
    </div>
    <div class="campo" id="campoChave" hidden>
      <label for="k">Chave privada <span class="opc">facultativa</span></label>
      <input id="k" type="password" autocomplete="off"
             placeholder="Ed25519, 64 hexadecimais">
    </div>
    <div class="campo">
      <label for="db">Database <span class="opc">opcional</span></label>
      <input id="db" autocomplete="off" placeholder="abre já neste banco">
    </div>'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# CSS dos campos novos
s=s.replace('''label{display:block;font-size:11px;letter-spacing:.1em;text-transform:uppercase;
  color:var(--texto-3);margin:0 0 6px}''','''label{display:block;font-size:11px;letter-spacing:.1em;text-transform:uppercase;
  color:var(--texto-3);margin:0 0 6px}
label .opc{text-transform:none;letter-spacing:0;font-size:10.5px;opacity:.6;
  font-style:italic}
.onde{display:grid;grid-template-columns:1fr 88px;gap:10px}''')
open(p,'w').write(s)
print('campos ok')
