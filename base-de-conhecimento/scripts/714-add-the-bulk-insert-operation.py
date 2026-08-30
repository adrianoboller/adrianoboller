# Add the bulk insert operation
# 28/08 19:21

import io
p='crates/phxsql-server/src/usuarios.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''            "inserir" => Atividade::Inserir,''',
            '''            "inserir" => Atividade::Inserir,
            // Carga em lote e insercao, e nao mais que isso: quem pode gravar
            // uma linha pode gravar mil. O que muda e o custo, e para isso ha
            // o teto de linhas por carga.
            "inserir_lote" | "importar" | "carga" => Atividade::Inserir,''',1)
io.open(p,'w',encoding='utf-8').write(s)
