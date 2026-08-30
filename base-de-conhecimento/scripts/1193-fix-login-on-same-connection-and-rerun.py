# Fix login on same connection and rerun
# 29/08 18:55

import pathlib
p = pathlib.Path("/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/integra/lgpd.mjs")
t = p.read_text()
# A porta de dados exige login na MESMA conexao: uma conexao so para as duas falas.
t = t.replace(
"""function fala(l){return new Promise(ok=>{const s=net.createConnection(5399,'127.0.0.1',()=>s.write(l+'\\n'));let b='';s.on('data',d=>{b+=d;if(b.includes('\\n')){s.end();ok(JSON.parse(b));}});s.on('error',()=>ok(null));});}""",
"""function sessao(linhas){return new Promise(ok=>{const s=net.createConnection(5399,'127.0.0.1',()=>s.write(linhas.join('\\n')+'\\n'));
  let b='',r=[];s.on('data',d=>{b+=d;const p=b.split('\\n');b=p.pop();for(const l of p) if(l.trim()) r.push(JSON.parse(l));
  if(r.length>=linhas.length){s.end();ok(r);}});s.on('error',()=>ok(r));});}""")
t = t.replace("""const r = await fala(`{${T},"op":"criar_tabela\"""",
              """const r = (await sessao([`{${T},"op":"login","usuario":"adriano","senha":"demo123"}`, `{${T},"op":"criar_tabela\"""")
t = t.replace("""}]}`);
console.log('criar_tabela:'""", """}]}`]))[1];
console.log('criar_tabela:'""")
p.write_text(t); print("script com login na mesma conexao")
