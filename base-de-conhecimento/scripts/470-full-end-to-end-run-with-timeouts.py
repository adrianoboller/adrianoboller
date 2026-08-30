# Full end-to-end run with timeouts
# 28/08 15:36

p='juncao.mjs'
s=open(p).read()
s=s.replace('''const api = o => new Promise(r => { fila.push(r); s.write(JSON.stringify({token:"segredo-da-juncao", ...o})+"\\n"); });''',
'''const api = o => new Promise((res, rej) => {
  const t = setTimeout(() => rej(new Error("SEM RESPOSTA: " + JSON.stringify(o).slice(0,90))), 8000);
  fila.push(r => { clearTimeout(t); res(r); });
  s.write(JSON.stringify({token:"segredo-da-juncao", ...o})+"\\n"); });''')
open(p,'w').write(s)
