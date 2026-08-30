# Wrap page in HTML skeleton, adjust CSP
# 27/08 19:47

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
s = s.replace('''let _ = http::responder(&mut fluxo, 200, "text/html; charset=utf-8", http::PAGINA);''',
              '''let _ = http::responder(
                    &mut fluxo,
                    200,
                    "text/html; charset=utf-8",
                    &http::montar_pagina(),
                );''')
open(p,'w').write(s)
