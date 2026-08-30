# Clean up three rough edges
# 28/08 10:35

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()

# 1. o `return folha(...), ligar...` com virgula: separa em duas linhas
v = """  if (!pag) {
    return folha(`Partições de ${tab}`, `${db} · esta tabela não é paginada`,"""
n = """  if (!pag) {
    folha(`Partições de ${tab}`, `${db} · esta tabela não é paginada`,"""
assert s.count(v) == 1
s = s.replace(v, n)

v = """       <div class="acoes">
         <button class="botao secundario" id="btVoltarGer">← Gestão de ${esc(tab)}</button>
       </div>`), ligarVoltarGestao(db, tab);
  }"""
n = """       <div class="acoes">
         <button class="botao secundario" id="btVoltarGer">← Gestão de ${esc(tab)}</button>
       </div>`);
    return ligarVoltarGestao(db, tab);
  }"""
assert s.count(v) == 1
s = s.replace(v, n)

# 2. o `refazer` que nao e usado
v = """  const refazer = () => { lerForm(); desenharNovaTabela(db); };

"""
assert s.count(v) == 1
s = s.replace(v, "")

# 3. o rascunho declarado junto com o resto do estado
v = """              teto:200, esquemaAtual:null, grade:null, painel:null };"""
n = """              teto:200, esquemaAtual:null, grade:null, painel:null,
              // O formulario de nova tabela sobrevive a um "+ coluna": o
              // rascunho fica aqui e a tela se redesenha a partir dele.
              rascunho:null };"""
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
