# Add dialog CSS
# 28/08 17:44

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()
velho='''.botao.mini{width:auto;padding:4px 10px;font-size:11px;font-weight:500;'''
novo='''/* ------------------------------------------------- dialogo de exclusao
   Um sobreposto e nao um `confirm()` do navegador porque a escolha aqui tem
   DUAS partes -- o modo e o motivo --, e `confirm` so sabe perguntar sim ou
   nao. E o modo importa: um dos dois nao tem volta. */
.sobre{position:fixed;inset:0;background:rgba(1,4,24,.62);z-index:90;
       display:flex;align-items:center;justify-content:center;padding:20px}
.caixa{background:var(--painel);border:1px solid var(--linha-forte);
       border-radius:12px;max-width:560px;width:100%;padding:20px 22px;
       box-shadow:0 18px 50px rgba(0,0,0,.45)}
.caixa h3{margin:0 0 4px;font-size:16px}
.caixa .sub{color:var(--texto-3);font-size:11.5px;margin-bottom:14px}
.modos{display:grid;gap:8px;margin-bottom:14px}
.modo{display:flex;gap:10px;align-items:flex-start;padding:11px 13px;
      background:var(--painel-2);border:1px solid var(--linha);border-radius:9px;
      cursor:pointer;text-align:left;font:inherit;color:var(--texto-2);width:auto}
.modo:hover{border-color:var(--linha-forte)}
.modo.escolhido{border-color:var(--laranja);color:var(--texto)}
.modo.escolhido.risco{border-color:var(--vermelhao)}
.modo .m-ico{font-size:16px;line-height:1.2}
.modo .m-rot{font-size:13px;font-weight:600;display:block}
.modo .m-diz{font-size:10.5px;color:var(--texto-3);line-height:1.4;display:block;
             margin-top:2px}
.marca-excluida{color:var(--log);font-weight:600}
tr.linha-excluida td{opacity:.55;text-decoration:line-through}
.chip-visao{display:inline-flex;gap:0;border:1px solid var(--linha-forte);
            border-radius:7px;overflow:hidden;margin-left:8px}
.chip-visao button{padding:3px 10px;font-size:11px;background:transparent;
                   border:0;color:var(--texto-3);cursor:pointer;font:inherit;width:auto}
.chip-visao button.ativo{background:var(--laranja);color:#10060a}

.botao.mini{width:auto;padding:4px 10px;font-size:11px;font-weight:500;'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
