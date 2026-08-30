# Add trash icon and toolbar button
# 28/08 17:47

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
# icone da lixeira: tampa, corpo e as duas ranhuras
velho='  exportar: `<path d="M12 3v11M8 10.5l4 4 4-4"'
novo='''  lixeira: `<path d="M4 6.5h16M9.5 6.5V4.6A1.1 1.1 0 0110.6 3.5h2.8a1.1 1.1 0 011.1 1.1v1.9" fill="none" stroke-width="1.6" stroke-linecap="round"/><path d="M6.2 6.5l.9 13a1.4 1.4 0 001.4 1.3h7a1.4 1.4 0 001.4-1.3l.9-13" fill="none" stroke-width="1.6"/><path d="M10.2 10.5v6.5M13.8 10.5v6.5" fill="none" stroke-width="1.5" stroke-linecap="round"/>`,
  exportar: `<path d="M12 3v11M8 10.5l4 4 4-4"'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''  { ico:"exportar", rot:"Exportar",   cor:"var(--ok)",     faz:() => telaExportar() },'''
novo2='''  { ico:"lixeira",  rot:"Lixeira",    cor:"var(--log)",    faz:() => telaLixeira() },
  { ico:"exportar", rot:"Exportar",   cor:"var(--ok)",     faz:() => telaExportar() },'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
