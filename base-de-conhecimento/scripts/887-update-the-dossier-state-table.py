# Update the dossier state table
# 28/08 23:11

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
# a secao 23 ganha as linhas novas
antigo = """        <tr><td>Salto por posição com bissecção no <code>rownum</code></td><td><span class="pino ok">pronto</span></td><td class="num">22</td></tr>"""
novo = """        <tr><td>Salto por posição com bissecção no <code>rownum</code></td><td><span class="pino ok">pronto</span></td><td class="num">22</td></tr>
        <tr><td>Profiler · o que chega pela porta antes de virar dado</td><td><span class="pino ok">pronto</span></td><td class="num">9</td></tr>
        <tr><td>Chave composta livre e única · a recusa não consome slot</td><td><span class="pino ok">pronto</span></td><td class="num">4</td></tr>
        <tr><td>Contêiner <code>scratch</code> de 4,7 MB · alvo musl</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Várias instâncias em portas diferentes</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Janela de conflito de escrita · a segunda gravação vence calada</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Direito no nível da <em>tabela</em> · hoje para na base</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Índice de texto completo · índice parcial · ordenação linguística</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Cluster · endereço único, eleição, promoção automática</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Diagrama ER e editor visual de modelo</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.15.0</strong>,"""
novo = """  <p class="chamada" style="margin-top:-6px">Esta é a revisão da <strong>0.16.0</strong>,"""
assert antigo in s
s = s.replace(antigo, novo)
antigo = """  faltava saíram, e as três estão medidas: a <a href="#s9">replicação com quatro
  servidores</a>, o <a href="#s5b">salto para «a página 500»</a> e a
  <a href="#s7">carga em lote</a>.</p>"""
novo = """  faltava saíram, e as três estão medidas: a <a href="#s9">replicação com quatro
  servidores</a>, o <a href="#s5b">salto para «a página 500»</a> e a
  <a href="#s7">carga em lote</a>. Depois dela entraram o <b>Profiler</b>, as
  cores da ação, o contêiner <code>scratch</code> de 4,7 MB — e a leitura da
  documentação do HFSQL(R) contra este código, que está em
  <code>docs/HFSQL.md</code> e que apontou o que ainda falta.</p>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
