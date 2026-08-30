# Add new write ops to the policy test
# 28/08 17:40

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            // Derrubar conexao alheia nao e leitura: um servidor somente
            // leitura nao deve poder interromper o trabalho de ninguem.
            "encerrar_sessao",
        ] {'''
novo='''            // Derrubar conexao alheia nao e leitura: um servidor somente
            // leitura nao deve poder interromper o trabalho de ninguem.
            "encerrar_sessao",
            // Restaurar desmarca a coluna de sistema, e esvaziar apaga a
            // lixeira inteira: os dois gravam.
            "restaurar",
            "esvaziar_lixeira",
        ] {'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
