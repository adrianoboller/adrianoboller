# Translate the paging mode labels
# 28/08 19:55

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """      <span class="leg">${fmt(r.devolvidas)} de ${fmt(r.visiveis ?? r.registros)} ·
        <b>${esc(r.modo)}</b>${r.salto ? ` · ${esc(r.salto)}` : ""}</span>
    </div>`;
"""
novo = """      <span class="leg" title="${esc(COMO_PAGINOU[r.salto || r.modo] || "")}">${
        fmt(r.devolvidas)} de ${fmt(r.visiveis ?? r.registros)} ·
        <b>${esc(NOME_DO_MODO[r.salto || r.modo] || r.modo)}</b></span>
    </div>`;
"""
assert antigo in s
s = s.replace(antigo, novo)

# A tabela de nomes, junto da funcao.
antigo = """async function verConteudoEditavel(db, tab, verExcluidos = false, cursor = null) {"""
novo = """/* Como a página foi buscada, em português e com o porquê no `title`. O
   protocolo fala sem acento de propósito -- é chave de JSON --, e a tela é
   quem traduz. */
const NOME_DO_MODO = { cursor:"cursor", bisseccao:"bissecção", passo:"passo a passo",
                       posicao:"posição", rownum:"nº de ordem", indice:"índice" };
const COMO_PAGINOU = {
  cursor:"continuou do rowid onde a página anterior parou: o custo é o da página, não o da tabela",
  bisseccao:"a posição é o nº de ordem, então o início da página saiu de uma busca binária: ~20 leituras",
  passo:"a tabela tem linha excluída, ou é particionada por letra: aqui a posição não é o nº de ordem, e o motor anda até ela",
  rownum:"a página começou no nº de ordem pedido",
};

async function verConteudoEditavel(db, tab, verExcluidos = false, cursor = null) {"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
