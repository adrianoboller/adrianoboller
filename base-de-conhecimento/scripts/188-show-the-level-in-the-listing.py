# Show the level in the listing
# 27/08 21:19

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''        println!(
            "{:<14} {:<26} {:<9} {:<7}  poder por base",
            "login", "nome", "supervisor", "ativo"
        );''','''        println!(
            "{:<14} {:<24} {:<10} {:<7}  poder por base",
            "login", "nome", "nivel", "ativo"
        );''')
s=s.replace('''            } else if u.bases.is_empty() {
                vec!["(nenhuma)".to_string()]
            } else {''','''            } else if u.bases.is_empty() {
                // Sem regra de base, quem manda e o nivel -- e a listagem tem
                // de dizer isso, senao "(nenhuma)" mente sobre quem pode ler.
                let podem: Vec<&str> = phxsql_server::Atividade::TODAS
                    .iter()
                    .filter(|a| u.nivel.permissoes().pode(**a))
                    .map(|a| a.nome())
                    .collect();
                vec![if podem.is_empty() {
                    "(nada, em base nenhuma)".to_string()
                } else {
                    format!("(pelo nivel, em toda base: {})", podem.join("+"))
                }]
            } else {''')
open(p,'w').write(s)
