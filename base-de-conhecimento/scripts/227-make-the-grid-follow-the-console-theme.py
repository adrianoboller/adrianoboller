# Make the grid follow the console theme
# 27/08 21:55

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
s=s.replace('''  if (t === "Date" || t === "DateTime") return "dataHora";''',
'''  if (t === "Date") return "data";           // sem hora, porque nao ha hora
  if (t === "DateTime" || t === "Time") return "dataHora";''')

# o grid segue o tema do console, em vez de trazer o proprio branco
s=s.replace('''.dica{font-size:11.5px;color:var(--texto-3);opacity:.8;font-style:italic;margin-left:auto}''',
'''.dica{font-size:11.5px;color:var(--texto-3);opacity:.8;font-style:italic;margin-left:auto}

/* O phx-grid traz o proprio tema claro. Aqui ele passa a seguir o do
   console: os tokens dele apontam para os nossos, e o sol/lua troca os dois
   ao mesmo tempo. Sem isso, no tema escuro o grid ficaria branco no meio da
   pagina preta. */
.phx-grid{
  --phx-bg:var(--painel); --phx-bg2:var(--painel-2); --phx-fg:var(--texto);
  --phx-meta:var(--texto-3); --phx-borda:var(--linha); --phx-acc:var(--laranja);
  font-family:"Exo 2","Helvetica Neue",Arial,sans-serif;
}
.phx-grid input,.phx-grid select,.phx-grid button{
  background:var(--painel-2);color:var(--texto);border-color:var(--linha-forte);
}''')
open(p,'w').write(s)
