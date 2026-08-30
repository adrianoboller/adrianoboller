# Insert DbLink UI, toolbar, menu and CSS
# 28/08 14:53

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/ui_dblink.js').read()

# 1) o codigo, antes da barra de ferramentas
marca='''/* ======================================================= barra de ferramentas'''
assert marca in s
s=s.replace(marca, novo.rstrip()+"\n\n"+marca,1)

# 2) icone
a='''  ajuda: `<circle cx="12" cy="12" r="9"'''
b='''  // Dois elos que se encaixam atravessando uma linha tracejada: a ligacao
  // cruza a fronteira do servidor.
  elo: `<path d="M9.5 14.5l5-5" stroke-width="1.7" stroke-linecap="round"/><path d="M12.8 7.7l1.9-1.9a3.4 3.4 0 014.8 4.8l-1.9 1.9M11.2 16.3l-1.9 1.9a3.4 3.4 0 01-4.8-4.8l1.9-1.9" fill="none" stroke-width="1.7" stroke-linecap="round"/>`,
  ajuda: `<circle cx="12" cy="12" r="9"'''
assert a in s; s=s.replace(a,b,1)

# 3) botao da barra, ao lado de Conexoes
a='''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verConexoes },'''
if a not in s:
    a='''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verConexoes },'''
a2='''  { ico:"tomada",   rot:"Conexões",   cor:"var(--memo)",   faz:verConexoes },'''
alvo=[l for l in s.split("\n") if 'rot:"Conexões"' in l and 'ico:"tomada"' in l]
assert len(alvo)==1, alvo
linha=alvo[0]
s=s.replace(linha, linha+'\n  { ico:"elo",      rot:"DbLink",     cor:"var(--memo)",   faz:() => telaDbLink() },',1)

# 4) menu Configuracoes
a='''    { rot:"Editor de menu…",      ico:"✎", faz:editorDeMenu },'''
b='''    "sep",
    { rot:"Definições do DbLink…", ico:"⛓", faz:telaDbLinkDefinicoes },
    "sep",
    { rot:"Editor de menu…",      ico:"✎", faz:editorDeMenu },'''
assert a in s; s=s.replace(a,b,1)

# 5) CSS
a='''/* O phx-grid traz o proprio tema claro.'''
b='''/* ------------------------------------------------------------------ dblink */
.barra-dbl{display:flex;gap:12px;align-items:end;flex-wrap:wrap;margin-bottom:14px}
.barra-dbl label{display:flex;flex-direction:column;gap:4px;font-size:11px;
                 color:var(--texto-3)}
.barra-dbl select{
  padding:7px 9px;border:1px solid var(--linha);border-radius:5px;
  background:var(--painel);color:var(--texto);font-size:12.5px;min-width:170px;
}
.barra-dbl .botao{width:auto;padding:7px 14px}
/* Lista a esquerda, dado a direita: e a forma do Centro de Controle, e e a
   que deixa trocar de tabela sem perder de vista as outras. */
.dbl-corpo{display:grid;grid-template-columns:270px 1fr;gap:16px;align-items:start}
@media (max-width:900px){.dbl-corpo{grid-template-columns:1fr}}
.dbl-lista{
  border:1px solid var(--linha);border-radius:8px;background:var(--painel);
  max-height:70vh;overflow:auto;
}
.dbl-cab{padding:9px 12px;font-size:11px;color:var(--texto-3);
         border-bottom:1px solid var(--linha);position:sticky;top:0;
         background:var(--painel-2);z-index:1}
.dbl-tab{
  display:block;width:100%;text-align:left;padding:9px 12px;background:none;
  border:none;border-bottom:1px solid var(--linha);color:var(--texto);
  cursor:pointer;font:inherit;
}
.dbl-tab:hover{background:var(--painel-2)}
.dbl-tab.viva{background:var(--realce);box-shadow:inset 3px 0 0 var(--laranja)}
.dbl-tab .n{display:block;font-size:12.5px;font-weight:600}
.dbl-tab .m{display:block;font-size:10.5px;color:var(--texto-3);margin-top:2px;
            font-variant-numeric:tabular-nums}
.dbl-titulo{display:flex;gap:10px;align-items:center;margin-bottom:10px;
            flex-wrap:wrap}
.dbl-titulo .leg{font-size:11px;color:var(--texto-3)}
.dbl-titulo .cresce{flex:1}
.dbl-titulo .botao{width:auto;padding:6px 12px;font-size:11.5px}
.form-dbl{display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));
          gap:12px 16px;text-align:left}
.form-dbl .cmp{display:flex;flex-direction:column;gap:4px;font-size:11px;
               color:var(--texto-3)}
.form-dbl .cmp > span:first-child{letter-spacing:.04em}
.form-dbl input,.form-dbl select{
  padding:8px 10px;border:1px solid var(--linha);border-radius:5px;
  background:var(--painel);color:var(--texto);font-size:12.5px;margin:0;
}
.form-dbl input:focus,.form-dbl select:focus{outline:none;border-color:var(--laranja)}
.form-dbl .linha-chk{grid-column:1/-1;flex-direction:row;align-items:center;
                     gap:8px;font-size:12px;color:var(--texto-2)}
.form-dbl .linha-chk input{width:auto}
#sqlDbl{
  width:100%;padding:10px 12px;border:1px solid var(--linha);border-radius:6px;
  background:var(--painel);color:var(--texto);font-size:12.5px;
  font-family:"IBM Plex Mono",monospace;resize:vertical;margin:0 0 12px;
}

/* O phx-grid traz o proprio tema claro.'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
