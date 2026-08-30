# Rebuild and check the listing
# 27/08 21:19

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''            println!(
                "{:<14} {:<26} {:<9} {:<7}  {}",
                u.login,
                u.nome,
                if u.supervisor { "sim" } else { "nao" },
                if u.ativo { "sim" } else { "nao" },
                bases.join("  ")
            );''','''            println!(
                "{:<14} {:<24} {:<10} {:<7}  {}",
                u.login,
                u.nome,
                if u.supervisor {
                    "supervisor"
                } else {
                    u.nivel.nome()
                },
                if u.ativo { "sim" } else { "nao" },
                bases.join("  ")
            );''')
open(p,'w').write(s)
