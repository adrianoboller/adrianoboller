# Atualizar a capa e regerar os numeros
# 29/08 06:59

import io
p='docs/dossie/dossie-phxsql-0.15.html'
s=io.open(p,encoding='utf-8').read()
velho='''  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.17.0</strong>,
  refeita contra o código. Ela fecha quatro itens da lista do que faltava, e nenhum
  deles é recurso inventado aqui: a <a href="#s7">janela de conflito de escrita</a>,
  o <a href="#s10">direito no nível da tabela</a> — os dois apontados pela leitura
  do HFSQL(R) —, o <a href="#s21">BULKINSERT</a> que reserva a tabela para quem
  carrega, e o ataque ao custo da inserção, que a medição
  <a href="#s21">redirecionou no meio do caminho</a>: o item pedia «ordene as
  chaves do lote», a desordem custava 1,06&#215;, e o que estava caro era o
  <strong>CRC-32 de página inteira</strong> relido a cada descida da árvore. Três
  cortes depois a inserção saiu de 44,4 para <strong>15,9&#8201;µs por linha
  (2,79&#215;)</strong>, e a bancada de dez milhões de 884 para
  <strong>303 segundos</strong>.</p>'''
novo='''  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.17.0</strong>,
  refeita contra o código, mais a rodada de desempenho que a seguiu — guiada pela
  leitura dos fontes do InnoDB, do Aria e do Cassandra
  (<code>docs/CONCORRENTES.md</code> e <code>docs/CASSANDRA.md</code>). Seis
  cortes medidos levaram a inserção de 44,4 para <strong>7,5&#8201;µs por
  linha</strong>, e a bancada de dez milhões de 884 para
  <strong>91,5 segundos — a primeira corrida em que o PhxSql insere mais rápido
  que o MySQL(R)</strong> (112,4&#8201;s), ganhando também em buscar, varrer e
  atualizar. A janela de <a href="#s7">conflito de escrita</a>, o
  <a href="#s10">direito por tabela</a> e o <a href="#s21">BULKINSERT</a>
  entraram na mesma versão. Só excluir ainda perde, e está na fila.</p>'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('capa ok')
