# Fix pagination fields in the form
# 28/08 10:41

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''         <label><span>registros por arquivo <em>(0 = arquivo único)</em></span>
           <input id="nt_pag" inputmode="numeric" placeholder="0"
                  value="${esc(String(r.pag ?? 0))}"></label>
         <label><span>teto de volumes <em>(0 = sem teto)</em></span>
           <input id="nt_max" inputmode="numeric" placeholder="0"
                  value="${esc(String(r.max ?? 0))}"></label>'''
n = '''         <label><span>registros por arquivo <em>(0 = arquivo único)</em></span>
           <input id="nt_pag" inputmode="numeric" placeholder="0"
                  value="${esc(String(r.pag ?? 0))}"></label>
         <label><span>dígitos do sufixo <em>(3 dá <code>_001</code>)</em></span>
           <input id="nt_dig" inputmode="numeric" placeholder="3"
                  value="${esc(String(r.dig ?? 3))}"></label>
         <label><span>teto de volumes <em>(0 = o que couber no sufixo)</em></span>
           <input id="nt_max" inputmode="numeric" placeholder="${esc(String(tetoDoSufixo(r.dig ?? 3)))}"
                  value="${esc(String(r.max ?? 0))}"></label>'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    r.pag = +$("#nt_pag").value || 0;
    r.max = +$("#nt_max").value || 0;'''
n = '''    r.pag = +$("#nt_pag").value || 0;
    r.dig = Math.min(9, Math.max(1, +$("#nt_dig").value || 3));
    r.max = +$("#nt_max").value || 0;'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    if (r.pag > 0) { pedido.registros_por_arquivo = r.pag; pedido.max_arquivos = r.max; }'''
n = '''    if (r.pag > 0) {
      pedido.registros_por_arquivo = r.pag;
      pedido.digitos = r.dig;
      pedido.max_arquivos = r.max;   // zero = o que couber no sufixo
    }'''
assert s.count(v) == 1
s = s.replace(v, n)

# a nota do rodape explica o teto que existe de verdade
v = '''       <p><strong>A paginação entra agora e não muda depois.</strong> Ela é o
       divisor que transforma o rowid em endereço; trocá-la mais tarde mudaria
       o endereço de cada registro já gravado.</p>'''
n = '''       <p><strong>A paginação entra agora e não muda depois.</strong> Ela é o
       divisor que transforma o rowid em endereço; trocá-la mais tarde mudaria
       o endereço de cada registro já gravado.</p>
       <p><strong>Não existe “sem teto”.</strong> O sufixo tem largura fixa, e
       com ${esc(String(r.dig ?? 3))} dígitos o volume
       ${esc(String(tetoDoSufixo(r.dig ?? 3) + 1))} não teria nome. Deixando o
       teto em zero, ele vira ${esc(String(tetoDoSufixo(r.dig ?? 3)))} —
       ${esc(String(tetoDoSufixo(r.dig ?? 3) * (r.pag || 0)))} registros de
       capacidade com o divisor de agora.</p>'''
assert s.count(v) == 1
s = s.replace(v, n)

# a funcao do teto
v = '''function desenharNovaTabela(db) {'''
n = '''/** Quantos volumes cabem num sufixo de N digitos: `_999` para tres.
 *
 * Nao e detalhe de tela. Zero volumes de teto seria "infinito", e infinito
 * nao existe aqui: o volume 1000 com tres digitos nao teria nome de arquivo. */
const tetoDoSufixo = digitos => Math.pow(10, Math.min(9, Math.max(1, digitos))) - 1;

function desenharNovaTabela(db) {'''
assert s.count(v) == 1
s = s.replace(v, n)

# e o rascunho ja nasce com digitos
v = '''  est.rascunho = est.rascunho || {
    colunas:'''
n = '''  est.rascunho = est.rascunho || {
    dig: 3,
    colunas:'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')
