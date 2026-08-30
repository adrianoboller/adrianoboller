# Update test and run full gates
# 29/08 17:28

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = '''        assert_eq!(erro.nome(), "ESCRITA_NA_REPLICA");
        assert_eq!(erro.codigo(), 4003);'''
novo = '''        // O nome era ESCRITA_NA_REPLICA quando esta frente nasceu sozinha. Na
        // integracao com o cluster os dois viraram UM erro -- para quem chama,
        // "escreveu no no errado, va para aquele" e o mesmo evento, e evento
        // so tem um codigo. O 4003 e o resto do teste continuam identicos: e a
        // GARANTIA que importa, e ela nao mudou.
        assert_eq!(erro.nome(), "REDIRECIONA");
        assert_eq!(erro.codigo(), 4003);'''
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("teste atualizado")
