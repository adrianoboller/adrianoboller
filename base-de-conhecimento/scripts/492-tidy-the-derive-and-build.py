# Tidy the derive and build
# 28/08 16:26

p='crates/phxsql-server/src/acesso.rs'
s=open(p).read()
s=s.replace('#[derive(Debug, Clone, PartialEq)]\n#[derive(Default)]\npub struct Acesso {',
            '#[derive(Debug, Clone, PartialEq, Default)]\npub struct Acesso {',1)
open(p,'w').write(s)
