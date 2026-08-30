# Use IsTerminal from std instead of TERM heuristic
# 27/08 19:06

p='crates/phxsql-server/src/main.rs'
s=open(p).read()
s=s.replace('''            if atty_provavel() {''','''            if std::io::stdin().is_terminal() {''')
s=s.replace('''    use std::io::Read;
    let i = args''','''    use std::io::{IsTerminal, Read};
    let i = args''')
s=s.replace('''/// Heuristica simples: sem terminal, a entrada costuma vir de um cano.
fn atty_provavel() -> bool {
    std::env::var("TERM").is_ok()
}

''','')
open(p,'w').write(s)
