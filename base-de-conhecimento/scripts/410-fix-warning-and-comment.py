# Fix warning and comment
# 28/08 14:22

p='crates/phxsql-server/src/email.rs'
s=open(p).read()
s=s.replace('''        let mut ultima = String::new();
        loop {''','''        let ultima;
        loop {''',1)
s=s.replace('''        // 2024-02-29T12:34:56Z -- ano bissexto, para pegar erro de calendario.''','''        // 2024-02-29T12:24:56Z -- ano bissexto, para pegar erro de calendario.''',1)
open(p,'w').write(s)
