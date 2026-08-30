# Add permissions and find the read-only list
# 28/08 17:40

import io
p='crates/phxsql-server/src/usuarios.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            "excluir" => Atividade::Excluir,'''
novo='''            "excluir" => Atividade::Excluir,
            // Restaurar e desfazer uma exclusao: exige o mesmo poder de
            // excluir, e nao mais. Quem pode tirar da lista pode devolver.
            "restaurar" => Atividade::Excluir,
            // O `.trash` e o `.reason` sao dos administradores, e a razao esta
            // no conteudo dos dois. O `.trash` guarda o dado que alguem mandou
            // apagar -- quem so tem `ler` perdeu o direito de ver aquela linha
            // no instante em que ela foi excluida, e a lixeira devolveria o
            // direito por outra porta. O `.reason` costuma ser ainda mais
            // revelador que o registro: "fraude", "pedido de remocao do
            // titular", "duplicidade com o contrato X".
            "lixeira" | "trash" | "motivos" | "reasons" => Atividade::Administrar,
            // Esvaziar a lixeira e a unica operacao do motor que apaga dado
            // sem rede nenhuma embaixo.
            "esvaziar_lixeira" => Atividade::Administrar,'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
