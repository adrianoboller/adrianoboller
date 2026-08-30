# Adapt the draft to existing helpers
# 28/08 14:53

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/ui_dblink.js'
s=open(p).read()
s=s.replace('tabela_html(','tabela(')
# a estrutura vira folha propria, nao janela
a='''  janela(`Estrutura de ${DBL.database}.${tabela}`,
    tabela('''
b='''  folha(`Estrutura de ${DBL.database}.${tabela}`,
    `colunas e índices como o ${DBL.ligacao} os reporta`,
    tabela('''
assert a in s; s=s.replace(a,b,1)
a='''        <td class="num">${esc(linha(l,6))}</td></tr>`));
}'''
b='''        <td class="num">${esc(linha(l,6))}</td></tr>`) +
    `<div class="acoes">
       <button class="botao secundario" id="btVoltaEstr">← ${esc(tabela)}</button>
     </div>`);
  $("#btVoltaEstr").onclick = () => { telaDbLink(DBL.ligacao); };
}'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('class="nota"','class="aviso"')
s=s.replace('`<div class="aviso ok">','`<div class="aviso">')
open(p,'w').write(s)
print('ok')
