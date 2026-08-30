# Add the preview operation and permissions
# 28/08 19:27

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
i=s.index('fn o_que_so_le_fica_fora_da_lista')
cab, corpo = s[:i], s[i:]
corpo = corpo.replace('''            "lixeira",
            "motivos",''','''            "lixeira",
            "motivos",
            // Conferir a carga nao grava nada -- e justamente o que se quer
            // poder fazer antes de decidir gravar.
            "importar_conferir",''',1)
s = cab + corpo
i2=s.index('fn tudo_que_grava_esta_na_lista_de_escrita')
cab2, corpo2 = s[:i2], s[i2:]
corpo2 = corpo2.replace('''            "restaurar",
            "esvaziar_lixeira",
        ] {''','''            "restaurar",
            "esvaziar_lixeira",
            // Carga em lote grava, e grava muito.
            "inserir_lote",
        ] {''',1)
io.open(p,'w',encoding='utf-8').write(cab2+corpo2)
