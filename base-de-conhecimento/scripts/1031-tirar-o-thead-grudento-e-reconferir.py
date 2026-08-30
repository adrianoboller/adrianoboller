# Tirar o thead grudento e reconferir
# 29/08 03:19

import io
p='docs/dossie/pagina-dos-pedidos.py'
s=io.open(p,encoding='utf-8').read()

velho = """thead th{
  font-family:"IBM Plex Mono",monospace;font-weight:500;
  font-size:10px;letter-spacing:.13em;text-transform:uppercase;
  color:var(--tinta-3);text-align:left;
  padding:12px 12px 8px;border-bottom:1px solid var(--linha);
  position:sticky;top:53px;background:var(--papel);z-index:4;
}"""
novo = """/* Cabecalho NAO grudento: `.rolo` tem `overflow-x:auto`, e isso faz dele um
   contexto de rolagem proprio -- o `position:sticky` do `thead` passava a se
   medir por ele e caia POR CIMA da primeira linha. Quem gruda e a barra de
   filtro, que e onde esta a contagem. */
thead th{
  font-family:"IBM Plex Mono",monospace;font-weight:500;
  font-size:10px;letter-spacing:.13em;text-transform:uppercase;
  color:var(--tinta-3);text-align:left;
  padding:12px 12px 8px;border-bottom:1px solid var(--linha);
  background:var(--papel);
}"""
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
