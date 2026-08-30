# Insert the join UI for real
# 28/08 15:42

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/ui_juncao.js').read()
marca='''/* ========================================================== DbLink'''
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)

a='''/* ------------------------------------------------------------------ dblink */'''
b='''/* ----------------------------------------------------------------- juncoes */
.vennes{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));
        gap:10px;margin:18px 0}
.venn{
  display:flex;flex-direction:column;align-items:center;gap:5px;padding:12px 8px 10px;
  background:var(--painel-2);border:1px solid var(--linha);border-radius:9px;
  color:var(--texto-2);cursor:pointer;font:inherit;width:auto;text-align:center;
}
.venn:hover{border-color:var(--linha-forte);color:var(--texto)}
.venn.viva{border-color:var(--laranja);background:var(--realce);color:var(--texto)}
.venn svg{width:96px;height:auto;display:block}
.venn .v-rot{font-size:12px;font-weight:600}
.venn .v-sql{font-size:9.5px;font-family:"IBM Plex Mono",monospace;
             color:var(--texto-3);line-height:1.3}
.venn .v-exp{font-size:10px;color:var(--texto-3);line-height:1.35}
.un-lista{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));
          gap:6px;margin-bottom:16px}
.un-item{display:flex;align-items:center;gap:8px;padding:8px 11px;margin:0;
         background:var(--painel-2);border:1px solid var(--linha);border-radius:6px;
         font-size:12.5px;text-transform:none;letter-spacing:0;color:var(--texto-2)}
.un-item input{width:auto;margin:0}
.un-item:hover{border-color:var(--linha-forte)}

/* ------------------------------------------------------------------ dblink */'''
assert a in s; s=s.replace(a,b,1)

alvo=[l for l in s.split("\n") if 'rot:"Pivot"' in l]
assert len(alvo)==1, alvo
s=s.replace(alvo[0], alvo[0]+'\n  { ico:"venn",     rot:"Junção",     cor:"var(--ndx)",    faz:() => telaJuncao() },',1)

a='''  // Dois elos que se encaixam atravessando uma linha tracejada'''
b='''  // Dois circulos que se cruzam, com o meio cheio: e o INNER JOIN, que e a
  // junção que todo mundo desenha quando explica junção.
  venn: `<circle cx="9.5" cy="12" r="6.5" fill="none" stroke-width="1.6"/><circle cx="14.5" cy="12" r="6.5" fill="none" stroke-width="1.6"/><path d="M12 6.2a6.5 6.5 0 000 11.6 6.5 6.5 0 000-11.6z" fill="currentColor" opacity=".55" stroke="none"/>`,
  // Dois elos que se encaixam atravessando uma linha tracejada'''
assert a in s; s=s.replace(a,b,1)

a='''    { rot:"Tabela dinâmica…",     ico:"▦", tecla:"Alt+7", faz:() => telaPivot() },
    "sep",'''
b='''    { rot:"Tabela dinâmica…",     ico:"▦", tecla:"Alt+7", faz:() => telaPivot() },
    { rot:"Junção de tabelas…",   ico:"◉", faz:() => telaJuncao() },
    { rot:"União de tabelas…",    ico:"⊎", faz:() => telaUniao() },
    "sep",'''
assert a in s; s=s.replace(a,b,1)

a='''    { rot:"Tabela dinâmica…",     ico:"▦", faz:() => telaPivot() },'''
b='''    { rot:"Tabela dinâmica…",     ico:"▦", faz:() => telaPivot() },
    { rot:"Junção de tabelas…",   ico:"◉", faz:() => telaJuncao() },
    { rot:"União de tabelas…",    ico:"⊎", faz:() => telaUniao() },'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
