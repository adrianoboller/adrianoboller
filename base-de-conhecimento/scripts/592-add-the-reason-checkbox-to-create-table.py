# Add the reason checkbox to create-table
# 28/08 17:47

import io
p='crates/phxsql-server/ui/index.html'
s=io.open(p,encoding='utf-8').read()

velho='''       <h3 class="secao">Partição
         <label class="chk"><input type="checkbox" id="nt_particionada"
           ${r.particionada ? "checked" : ""}> tabela particionada</label></h3>'''
novo='''       <h3 class="secao">Exclusão
         <label class="chk"><input type="checkbox" id="nt_motivo_obrig"
           ${r.motivo_obrigatorio ? "checked" : ""}> exigir motivo escrito</label></h3>
       <p class="leg">Toda tabela nasce com a coluna de sistema
         <code>softdeleted</code>: excluir <b>marca</b> a linha, que some das
         listas e continua inteira no <code>.reg</code>, e dá para restaurar.
         Quem manda apagar de vez tem a linha guardada no <code>.trash</code>
         antes de o slot sair, e o motivo vai para o <code>.reason</code> com a
         data, a hora e quem foi.<br>
         Marcada, esta caixa faz o motor <b>recusar</b> qualquer exclusão desta
         tabela sem uma frase escrita. Vale para tabela cujo apagamento alguém
         vai ter de justificar depois — e não para tabela de rascunho, onde
         obrigar só ensina todo mundo a digitar um ponto.</p>

       <h3 class="secao">Partição
         <label class="chk"><input type="checkbox" id="nt_particionada"
           ${r.particionada ? "checked" : ""}> tabela particionada</label></h3>'''
assert velho in s
s=s.replace(velho,novo,1)

s=s.replace('folha("Nova tabela", `${db} · cinco arquivos nascem juntos`,',
            'folha("Nova tabela", `${db} · sete arquivos nascem juntos`,',1)
io.open(p,'w',encoding='utf-8').write(s)
