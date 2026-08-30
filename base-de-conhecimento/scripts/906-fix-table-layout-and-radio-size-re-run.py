# Fix table layout and radio size, re-run
# 29/08 00:02

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
alvo = '''.caixa.larga{max-width:880px}
table.conf td,table.conf th{padding:5px 9px;font-size:12px}
table.conf .col{font-weight:600;white-space:nowrap}
table.conf tr.diverge td{background:rgba(255,196,61,.06)}
/* O `label` da folha nasce em caixa alta -- e ali ele e ROTULO. Aqui dentro
   ele embrulha um VALOR, e valor mostrado em caixa alta e uma mentira: quem
   olha nao sabe se «BLUMENAU» esta gravado assim ou se e a tela gritando. */
table.conf .esc{display:flex;gap:7px;align-items:baseline;cursor:pointer;margin:0;
  text-transform:none;letter-spacing:0;font-size:12px;color:var(--texto)}
table.conf .esc input{margin:0;flex:0 0 auto}
.vazio-nulo{color:var(--texto-3);font-style:italic}'''
novo = '''.caixa.larga{max-width:880px}
/* Largura fixa nas quatro colunas: sem isto, um valor comprido numa delas
   empurra as outras para fora da caixa, e a coluna que some e justamente a
   ultima -- «voce escreve». Com `fixed`, o texto quebra dentro da celula. */
table.conf{table-layout:fixed}
table.conf td,table.conf th{padding:5px 9px;font-size:12px;overflow-wrap:anywhere}
table.conf .col{font-weight:600}
table.conf col.c-nome{width:19%} table.conf col.c-val{width:27%}
table.conf tr.diverge td{background:rgba(255,196,61,.06)}
/* O `label` da folha nasce em caixa alta -- e ali ele e ROTULO. Aqui dentro
   ele embrulha um VALOR, e valor mostrado em caixa alta e uma mentira: quem
   olha nao sabe se «BLUMENAU» esta gravado assim ou se e a tela gritando. */
table.conf .esc{display:flex;gap:7px;align-items:baseline;cursor:pointer;margin:0;
  text-transform:none;letter-spacing:0;font-size:12px;color:var(--texto)}
/* O `input` da folha tem `width:100%` -- num radio isso vira uma bolinha que
   ocupa a celula inteira e joga o valor para o outro lado dela. */
table.conf .esc input{margin:0;flex:0 0 auto;width:14px;height:14px;padding:0}
.vazio-nulo{color:var(--texto-3);font-style:italic}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace(
'''      <div class="rolo"><table class="conf"><thead><tr>
        <th>Coluna</th><th>Valor anterior</th>''',
'''      <div class="rolo"><table class="conf">
        <colgroup><col class="c-nome"><col class="c-val"><col class="c-val"><col></colgroup>
        <thead><tr>
        <th>Coluna</th><th>Valor anterior</th>''', 1)
p.write_text(s)
print("ok")
