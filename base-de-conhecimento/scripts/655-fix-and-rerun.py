# Fix and rerun
# 28/08 18:34

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''            let linhas = r.campo("linhas").and_then(Json::lista).unwrap().clone();
            if linhas.is_empty() {''','''            let linhas: Vec<Json> = r.campo("linhas").and_then(Json::lista).unwrap().to_vec();
            if linhas.is_empty() {''',1)
io.open(p,'w',encoding='utf-8').write(s)
