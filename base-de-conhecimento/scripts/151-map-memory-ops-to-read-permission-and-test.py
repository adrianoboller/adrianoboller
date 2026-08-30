# Map memory ops to read permission and test
# 27/08 20:49

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
s=s.replace('''            "ping" | "login" | "desafio" | "quem_sou" => return None,''',
            '''            "ping" | "login" | "desafio" | "quem_sou" | "sair" => return None,''')
s=s.replace('''            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "buscar" => Atividade::Ler,''',
'''            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "buscar" => Atividade::Ler,
            // Consultar em memoria e ler: o dado e o mesmo, o caminho e outro.
            // Carregar tambem, porque carregar e varrer a tabela inteira.
            "memoria_carregar" | "memoria" | "SelectMemory" | "selectmemory"
            | "selecionar_memoria" => Atividade::Ler,''')
open(p,'w').write(s)
