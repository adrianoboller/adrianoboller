# Add the preview operation and permissions
# 28/08 19:27

import io
p='crates/phxsql-server/src/usuarios.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''            "inserir_lote" | "importar" | "carga" => Atividade::Inserir,''',
'''            "inserir_lote" | "importar" | "carga" => Atividade::Inserir,
            // Conferir LE a carga que o proprio usuario colou e le o esquema
            // da tabela: nao grava nada, e por isso pede so `ler`. Barrar
            // aqui obrigaria a tentar gravar para descobrir se a carga serve.
            "importar_conferir" => Atividade::Ler,''',1)
io.open(p,'w',encoding='utf-8').write(s)
