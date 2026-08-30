# Make transactions screen read live config
# 28/08 10:44

import pathlib
p = pathlib.Path('crates/phxsql-server/ui/index.html')
s = p.read_text()
v = '''function verTransacoes() {
  folha("Gestão de transações", "o que existe hoje, o que falta e por quê",
    `<div class="fichas">
       <div class="ficha"><div class="v">não</div><div class="r">commit/rollback</div>
         <div class="u">de várias operações</div></div>
       <div class="ficha"><div class="v">sim</div><div class="r">desfazer da inserção</div>
         <div class="u">se um índice recusar</div></div>
       <div class="ficha"><div class="v">sim</div><div class="r">diário .log</div>
         <div class="u">por tabela</div></div>
       <div class="ficha"><div class="v">sim</div><div class="r">espelho .bkp</div>
         <div class="u">se ligado no config</div></div>
     </div>
'''
n = '''async function verTransacoes() {
  // O espelho e por servidor, entao a ficha le a configuracao DESTE em vez de
  // dizer "se ligado" -- que e verdade em toda parte e informacao em lugar
  // nenhum.
  let espelho = null;
  try { espelho = !!(await api("config")).espelho; } catch (_) { /* sem sessão */ }

  folha("Gestão de transações", "o que existe hoje, o que falta e por quê",
    `<div class="fichas">
       <div class="ficha"><div class="v">não</div><div class="r">commit/rollback</div>
         <div class="u">de várias operações</div></div>
       <div class="ficha"><div class="v">sim</div><div class="r">desfazer da inserção</div>
         <div class="u">se um índice recusar</div></div>
       <div class="ficha"><div class="v">sim</div><div class="r">diário .log</div>
         <div class="u">por tabela</div></div>
       <div class="ficha"><div class="v">${espelho === null ? "—" : espelho ? "sim" : "não"}</div>
         <div class="r">espelho .bkp</div>
         <div class="u">neste servidor</div></div>
     </div>
'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
