# Add schema field to the new-table form
# 28/08 10:56

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''         <label class="largo"><span>nome da tabela</span>
           <input id="nt_nome" placeholder="cadastroClientes"
                  value="${esc(r.nome || "")}"></label>'''
n = '''         <label class="largo"><span>nome da tabela</span>
           <input id="nt_nome" placeholder="cadastroClientes"
                  value="${esc(r.nome || "")}"></label>
         <label class="largo"><span>schema
             <em>(vazio = na raiz do database; a pasta nasce junto se faltar)</em></span>
           <input id="nt_schema" placeholder="vendas"
                  value="${esc(r.schema || "")}"></label>'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    r.nome = $("#nt_nome").value.trim();'''
n = '''    r.nome = $("#nt_nome").value.trim();
    r.schema = $("#nt_schema").value.trim();'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    const pedido = {
      database: db, tabela: r.nome,'''
n = '''    const pedido = {
      database: db, tabela: r.nome,
      ...(r.schema ? { schema: r.schema } : {}),'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''      avisar(`tabela ${feito.tabela} criada · ${feito.colunas} coluna(s), `
           + `${feito.indices} índice(s)${feito.paginada ? ", paginada" : ""}`);'''
n = '''      avisar(`tabela ${feito.tabela} criada${r.schema ? ` em ${r.schema}` : ""}`
           + ` · ${feito.colunas} coluna(s), ${feito.indices} índice(s)`
           + `${feito.paginada ? ", paginada" : ""}`);'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
