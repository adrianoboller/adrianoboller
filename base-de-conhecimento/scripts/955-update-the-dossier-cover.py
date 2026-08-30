# Update the dossier cover
# 29/08 00:45

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
alvo = '''  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.16.0</strong>,
  refeita contra o código. Três coisas que a versão anterior listava como o que
  faltava saíram, e as três estão medidas: a <a href="#s9">replicação com quatro
  servidores</a>, o <a href="#s5b">salto para «a página 500»</a> e a
  <a href="#s7">carga em lote</a>. Depois dela entraram o <b>Profiler</b>, as
  cores da ação, o contêiner <code>scratch</code> de 4,7 MB — e a leitura da
  documentação do HFSQL(R) contra este código, que está em
  <code>docs/HFSQL.md</code> e que apontou o que ainda falta.</p>'''
novo = '''  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.17.0</strong>,
  refeita contra o código. Ela fecha três itens da lista do que faltava, e nenhum
  deles é recurso inventado aqui: a <a href="#s7">janela de conflito de escrita</a>,
  o <a href="#s10">direito no nível da tabela</a> — os dois apontados pela leitura
  do HFSQL(R) — e o ataque ao custo da inserção, que a medição
  <a href="#s17">redirecionou no meio do caminho</a>: o item pedia «ordene as
  chaves do lote», a desordem custava 1,06&#215;, e o que estava caro era o
  <strong>CRC-32 de página inteira</strong> relido a cada descida da árvore. Um
  cache de páginas no <code>.ndx</code> levou a inserção de 44,4 para
  18,5&#8201;µs por linha, e a bancada de dez milhões de 884 para
  <strong>303 segundos</strong>.</p>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
