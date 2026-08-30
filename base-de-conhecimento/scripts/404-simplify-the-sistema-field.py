# Simplify the sistema field
# 28/08 14:19

import re,io
p='crates/phxsql-server/src/sistema.rs'
s=open(p).read()
velho='''            (
                "sistema",
                Json::texto_de(if disponivel() {
                    std::env::consts::OS
                } else {
                    std::env::consts::OS
                }),
            ),
'''
novo='''            ("sistema", Json::texto_de(std::env::consts::OS)),
'''
assert velho in s
s=s.replace(velho,novo)
open(p,'w').write(s)
print("ok")
