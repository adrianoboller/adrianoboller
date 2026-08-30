# Achatar acentos na busca e regerar
# 29/08 03:18

import io
p='docs/dossie/pagina-dos-pedidos.py'
s=io.open(p,encoding='utf-8').read()

velho = """  function aplicar(){{
    const q = busca.value.trim().toLowerCase();
    let vistos = 0;
    for (const tr of linhas) {{
      const okEstado = filtro === 'todos' || tr.dataset.e === filtro;
      const okTexto  = !q || tr.textContent.toLowerCase().includes(q);"""
novo = """  // Sem isto, quem digita «indice» nao acha «indice» com acento -- e em
  // portugues isso e a busca falhando calada, nao uma sutileza.
  const achatar = t => t.normalize('NFD').replace(/[\\u0300-\\u036f]/g, '').toLowerCase();
  linhas.forEach(tr => tr.dataset.busca = achatar(tr.textContent));

  function aplicar(){{
    const q = achatar(busca.value.trim());
    let vistos = 0;
    for (const tr of linhas) {{
      const okEstado = filtro === 'todos' || tr.dataset.e === filtro;
      const okTexto  = !q || tr.dataset.busca.includes(q);"""
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')
