# Add the colour legend to the grid
# 28/08 22:50

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """.chip-visao button.ativo{background:var(--laranja);color:#10060a}"""
novo = """.chip-visao button.ativo{background:var(--laranja);color:#10060a}
/* A visao das marcadas herda o rosa da acao: a mesma cor do botao que marca. */
.chip-visao button.marcadas.ativo{background:var(--acao-marcar);color:var(--fundo)}"""
assert antigo in s
s = s.replace(antigo, novo)

# a legenda das cinco cores, na barra da grade
antigo = """      <span class="leg" title="${esc(COMO_PAGINOU[r.salto || r.modo] || "")}">${"""
novo = """      <span class="cores-acao" title="a mesma cor vale em toda tela">
        <b><i style="background:var(--acao-incluir)"></i>incluir</b>
        <b><i style="background:var(--acao-alterar)"></i>alterar</b>
        <b><i style="background:var(--acao-marcar)"></i>marcar</b>
        <b><i style="background:var(--acao-excluir)"></i>excluir de vez</b>
        <b><i style="background:var(--acao-consultar)"></i>consultar</b>
      </span>
      <span class="leg" title="${esc(COMO_PAGINOU[r.salto || r.modo] || "")}">${"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
